//! The lobby (ADR 0008 §3): admission (version, rules, join code, password), empire slots, ready flags, chat, and
//! the start that turns the links into a lockstep [`Host`] and [`Peer`]s. Lobby messages travel as
//! [`Msg::Lobby`] over any [`Transport`], so the loopback and QUIC links run the same code.
//!
//! Passwords are checked with SPAKE2 plus key confirmation bound to the link ([`Transport::binding`]): the password
//! never crosses the link, and a joiner knows it reached the real host once the host's confirmation checks out.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use ig2_sim::game::GameRules;
use serde::{Deserialize, Serialize};
use spake2::{Ed25519Group, Identity, Password, Spake2};

use crate::lockstep::{Host, Msg, Peer};
use crate::snapshot::{self, Snapshot};
use crate::transport::{loopback, Loopback, Transport};
use crate::wire;

/// The lobby protocol version (checked in the hello).
pub const LOBBY_VERSION: u32 = 1;
/// The host answers a wrong password after this long (one guess per connection).
pub const WRONG_PASSWORD_DELAY: Duration = Duration::from_secs(1);
/// A joiner that hasn't finished the admission by then is dropped.
pub const ADMISSION_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest member name and chat line (characters).
pub const MAX_NAME: usize = 32;
pub const MAX_CHAT: usize = 200;

const ID_JOINER: &[u8] = b"ig2-joiner";
const ID_HOST: &[u8] = b"ig2-host";

/// Why the host turned a joiner away (or the joiner gave up).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refusal {
    Version,
    Rules,
    /// A private session and no or the wrong join code.
    Code,
    Full,
    Started,
    WrongPassword,
    Kicked,
    /// A message out of order.
    Protocol,
    TimedOut,
    /// Joiner side: the host asked for a password and none was given.
    PasswordNeeded,
    /// Joiner side: the host's key confirmation was wrong (not the host, or not the same password).
    HostNotAuthentic,
}

/// Who holds an empire slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlotKind {
    Open,
    Closed,
    Ai,
    Human(u8),
}

/// One of the scenario's playable empires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    pub empire: u8,
    pub race: u8,
    pub kind: SlotKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// The lobby member id = the lockstep slot (the host is 0).
    pub id: u8,
    pub name: String,
    pub ready: bool,
}

/// What every member sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyState {
    pub name: String,
    pub scenario: String,
    pub private: bool,
    pub password: bool,
    pub slots: Vec<Slot>,
    pub members: Vec<Member>,
}

impl LobbyState {
    /// The empire `member` holds.
    pub fn empire_of(&self, member: u8) -> Option<u8> {
        self.slots.iter().find(|s| s.kind == SlotKind::Human(member)).map(|s| s.empire)
    }

    fn member_mut(&mut self, id: u8) -> Option<&mut Member> {
        self.members.iter_mut().find(|m| m.id == id)
    }
}

/// The start of the game, for one member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Start {
    /// The member's lockstep slot and empire.
    pub slot: u8,
    pub empire: u8,
    pub delay: u64,
    pub first_input: u64,
    /// Every human's (slot, empire).
    pub humans: Vec<(u8, u8)>,
    /// The start state (`Snapshot::to_file`).
    pub snapshot: Vec<u8>,
}

impl Start {
    /// This member's lockstep peer.
    pub fn peer<'r>(&self, rules: &'r GameRules) -> Result<Peer<'r>, snapshot::Error> {
        let (snap, _) = Snapshot::from_file(&self.snapshot)?;
        Peer::from_snapshot(self.slot, self.empire, &snap, rules, self.delay, self.first_input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LobbyMsg {
    // joiner → host
    Hello { version: u32, rules: [u8; 32], name: String, code: Option<u64> },
    Pick { empire: Option<u8> },
    Ready(bool),
    Chat(String),
    Leave,
    // both ways (password sessions)
    Pake(Vec<u8>),
    Confirm([u8; 32]),
    // host → joiner
    Refused(Refusal),
    Welcome { member: u8 },
    State(LobbyState),
    Said { member: u8, text: String },
    Start(Box<Start>),
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// HMAC-SHA256 (RFC 2104).
fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(k.map(|b| b ^ 0x36));
    for p in parts {
        inner.update(p);
    }
    let mut outer = Sha256::new();
    outer.update(k.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

/// The key confirmation for `role` (ADR 0008 §3.1).
fn confirmation(key: &[u8], role: &[u8], binding: &[u8; 32]) -> [u8; 32] {
    hmac(key, &[b"ig2-lobby-confirm", role, binding])
}

/// Constant-time comparison.
fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn lobby(m: Msg) -> Option<LobbyMsg> {
    match m {
        Msg::Lobby(l) => Some(l),
        _ => None,
    }
}

// ------------------------------------------------------------------------------------------------ host

/// The host's session settings (the password stays here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub name: String,
    pub scenario: String,
    pub private: bool,
    pub password: Option<String>,
}

enum Stage {
    Hello,
    /// Waiting for the joiner's SPAKE2 message.
    Pake { spake: Spake2<Ed25519Group>, name: String },
    /// Waiting for the joiner's confirmation.
    Confirm { key: Vec<u8>, name: String },
    Refuse { at: Instant, why: Refusal },
}

enum Outcome {
    Keep,
    Drop,
    Admit(String),
}

struct Pending {
    link: Box<dyn Transport>,
    stage: Stage,
    since: Instant,
}

/// Something that happened in the lobby (for the host's UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyEvent {
    Joined(u8),
    Left(u8),
    Refused(Refusal),
    Said(u8, String),
}

/// The host's side of the lobby.
pub struct LobbyHost {
    settings: Settings,
    session: u64,
    rules: [u8; 32],
    state: LobbyState,
    links: BTreeMap<u8, Box<dyn Transport>>,
    pending: Vec<Pending>,
    next_id: u8,
    dirty: bool,
}

/// What [`LobbyHost::start`] returns: the lockstep relay and the host's own peer start and link.
pub struct Started {
    pub host: Host,
    pub own: Start,
    pub link: Loopback,
}

impl LobbyHost {
    /// A lobby for `slots` (the scenario's playable empires, all open); the host is member 0 named `host_name`.
    pub fn new(settings: Settings, session: u64, rules: [u8; 32], slots: &[(u8, u8)], host_name: &str) -> Self {
        let state = LobbyState {
            name: settings.name.clone(),
            scenario: settings.scenario.clone(),
            private: settings.private,
            password: settings.password.is_some(),
            slots: slots.iter().map(|&(empire, race)| Slot { empire, race, kind: SlotKind::Open }).collect(),
            members: vec![Member { id: 0, name: truncate(host_name, MAX_NAME), ready: false }],
        };
        Self { settings, session, rules, state, links: BTreeMap::new(), pending: Vec::new(), next_id: 1, dirty: false }
    }

    pub fn state(&self) -> &LobbyState {
        &self.state
    }

    /// The session id, also the join code of a private session.
    pub fn session(&self) -> u64 {
        self.session
    }

    /// The LAN announcement, or `None` for a private session.
    pub fn announcement(&self, port: u16, fingerprint: [u8; 32]) -> Option<crate::discovery::Announcement> {
        (!self.settings.private).then(|| crate::discovery::Announcement {
            session: self.session,
            name: self.settings.name.clone(),
            port,
            fingerprint,
            password: self.settings.password.is_some(),
            version: LOBBY_VERSION,
            rules: self.rules,
            scenario: self.settings.scenario.clone(),
            humans: self.state.members.len() as u8,
            slots: self.state.slots.iter().filter(|s| s.kind != SlotKind::Closed && s.kind != SlotKind::Ai).count() as u8,
            started: false,
        })
    }

    /// A new connection (`QuicHost::accept`, or a loopback end).
    pub fn admit(&mut self, link: Box<dyn Transport>) {
        self.pending.push(Pending { link, stage: Stage::Hello, since: Instant::now() });
    }

    fn capacity(&self) -> usize {
        self.state.slots.iter().filter(|s| matches!(s.kind, SlotKind::Open | SlotKind::Human(_))).count()
    }

    fn clear_ready(&mut self) {
        for m in &mut self.state.members {
            m.ready = false;
        }
        self.dirty = true;
    }

    /// `member` takes `empire`'s slot (`None`: gives its slot up). Refused unless the slot is open or its own. Any
    /// change clears every ready flag.
    pub fn pick(&mut self, member: u8, empire: Option<u8>) -> bool {
        if self.state.member_mut(member).is_none() {
            return false;
        }
        let old = self.state.empire_of(member);
        if old == empire {
            return true;
        }
        if let Some(e) = empire {
            match self.state.slots.iter_mut().find(|s| s.empire == e) {
                Some(s) if s.kind == SlotKind::Open => s.kind = SlotKind::Human(member),
                _ => return false,
            }
        }
        if let Some(s) = old.and_then(|o| self.state.slots.iter_mut().find(|s| s.empire == o)) {
            s.kind = SlotKind::Open;
        }
        self.clear_ready();
        true
    }

    /// `member`'s ready flag; only a member holding an empire can be ready.
    pub fn set_ready(&mut self, member: u8, ready: bool) -> bool {
        let has = self.state.empire_of(member).is_some();
        match self.state.member_mut(member) {
            Some(m) if has || !ready => {
                m.ready = ready;
                self.dirty = true;
                true
            }
            _ => false,
        }
    }

    /// The host opens, closes or gives `empire`'s slot to the AI (a human in it loses it).
    pub fn set_slot(&mut self, empire: u8, kind: SlotKind) -> bool {
        if matches!(kind, SlotKind::Human(_)) {
            return false;
        }
        let Some(s) = self.state.slots.iter_mut().find(|s| s.empire == empire) else { return false };
        s.kind = kind;
        self.clear_ready();
        true
    }

    /// A chat line from `member` to everyone.
    pub fn say(&mut self, member: u8, text: &str) {
        let text = truncate(text, MAX_CHAT);
        let m = Msg::Lobby(LobbyMsg::Said { member, text });
        for l in self.links.values_mut() {
            l.send(&m);
        }
    }

    /// Drop `member` with `Refused{Kicked}`.
    pub fn kick(&mut self, member: u8) {
        if let Some(mut l) = self.links.remove(&member) {
            l.send(&Msg::Lobby(LobbyMsg::Refused(Refusal::Kicked)));
        }
        self.remove(member);
    }

    fn remove(&mut self, member: u8) {
        self.links.remove(&member);
        self.state.members.retain(|m| m.id != member);
        for s in &mut self.state.slots {
            if s.kind == SlotKind::Human(member) {
                s.kind = SlotKind::Open;
            }
        }
        self.clear_ready();
    }

    /// Every member holds an empire and is ready.
    pub fn can_start(&self) -> bool {
        self.state.members.iter().all(|m| m.ready && self.state.empire_of(m.id).is_some())
    }

    /// Run the admissions, take the members' messages, send the new state. Returns what happened.
    pub fn pump(&mut self) -> Result<Vec<LobbyEvent>, wire::Error> {
        let mut events = Vec::new();
        let now = Instant::now();
        let mut admitted = Vec::new();
        let mut keep = Vec::new();
        for mut p in std::mem::take(&mut self.pending) {
            match self.advance(&mut p, now, &mut events)? {
                Outcome::Keep => keep.push(p),
                Outcome::Drop => {}
                Outcome::Admit(name) => admitted.push((p.link, name)),
            }
        }
        self.pending = keep;
        for (mut link, name) in admitted {
            if self.capacity() <= self.state.members.len() {
                link.send(&Msg::Lobby(LobbyMsg::Refused(Refusal::Full)));
                events.push(LobbyEvent::Refused(Refusal::Full));
                continue;
            }
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1).max(1);
            link.send(&Msg::Lobby(LobbyMsg::Welcome { member: id }));
            self.links.insert(id, link);
            self.state.members.push(Member { id, name, ready: false });
            self.dirty = true;
            events.push(LobbyEvent::Joined(id));
        }

        let mut got = Vec::new();
        let mut gone = Vec::new();
        for (&id, l) in self.links.iter_mut() {
            if l.closed() {
                gone.push(id);
                continue;
            }
            while let Some(m) = l.recv()? {
                if let Some(m) = lobby(m) {
                    got.push((id, m));
                }
            }
        }
        for (id, m) in got {
            match m {
                LobbyMsg::Pick { empire } => {
                    self.pick(id, empire);
                    self.dirty = true;
                }
                LobbyMsg::Ready(r) => {
                    self.set_ready(id, r);
                    self.dirty = true;
                }
                LobbyMsg::Chat(text) => {
                    let text = truncate(&text, MAX_CHAT);
                    self.say(id, &text);
                    events.push(LobbyEvent::Said(id, text));
                }
                LobbyMsg::Leave => gone.push(id),
                _ => {}
            }
        }
        for id in gone {
            if self.state.members.iter().any(|m| m.id == id) {
                self.remove(id);
                events.push(LobbyEvent::Left(id));
            }
        }
        if self.dirty {
            self.dirty = false;
            let m = Msg::Lobby(LobbyMsg::State(self.state.clone()));
            for l in self.links.values_mut() {
                l.send(&m);
            }
        }
        Ok(events)
    }

    /// Run one joiner's admission as far as its messages go.
    fn advance(&self, p: &mut Pending, now: Instant, events: &mut Vec<LobbyEvent>) -> Result<Outcome, wire::Error> {
        loop {
            if let Stage::Refuse { at, why } = p.stage {
                if Instant::now() < at {
                    return Ok(Outcome::Keep);
                }
                p.link.send(&Msg::Lobby(LobbyMsg::Refused(why)));
                events.push(LobbyEvent::Refused(why));
                return Ok(Outcome::Drop);
            }
            if now.duration_since(p.since) > ADMISSION_TIMEOUT || p.link.closed() {
                p.stage = Stage::Refuse { at: now, why: Refusal::TimedOut };
                continue;
            }
            let Some(m) = p.link.recv()? else { return Ok(Outcome::Keep) };
            if let Some(name) = self.admission(p, m) {
                return Ok(Outcome::Admit(name));
            }
        }
    }

    /// One admission message; returns the joiner's name once admitted.
    fn admission(&self, p: &mut Pending, m: Msg) -> Option<String> {
        let refuse = |why| Stage::Refuse { at: Instant::now(), why };
        let Some(m) = lobby(m) else {
            p.stage = refuse(Refusal::Protocol);
            return None;
        };
        match (std::mem::replace(&mut p.stage, Stage::Hello), m) {
            (Stage::Hello, LobbyMsg::Hello { version, rules, name, code }) => {
                let name = truncate(&name, MAX_NAME);
                p.stage = if version != LOBBY_VERSION {
                    refuse(Refusal::Version)
                } else if rules != self.rules {
                    refuse(Refusal::Rules)
                } else if self.settings.private && code != Some(self.session) {
                    refuse(Refusal::Code)
                } else if self.capacity() <= self.state.members.len() {
                    refuse(Refusal::Full)
                } else if let Some(pw) = &self.settings.password {
                    let (spake, out) = Spake2::<Ed25519Group>::start_b(&Password::new(pw.as_bytes()), &Identity::new(ID_JOINER), &Identity::new(ID_HOST));
                    p.link.send(&Msg::Lobby(LobbyMsg::Pake(out)));
                    Stage::Pake { spake, name }
                } else {
                    return Some(name);
                };
            }
            (Stage::Pake { spake, name }, LobbyMsg::Pake(msg)) => {
                p.stage = match spake.finish(&msg) {
                    Ok(key) => Stage::Confirm { key, name },
                    Err(_) => refuse(Refusal::Protocol),
                };
            }
            (Stage::Confirm { key, name }, LobbyMsg::Confirm(mac)) => {
                let binding = p.link.binding();
                if same(&mac, &confirmation(&key, ID_JOINER, &binding)) {
                    p.link.send(&Msg::Lobby(LobbyMsg::Confirm(confirmation(&key, ID_HOST, &binding))));
                    return Some(name);
                }
                p.stage = Stage::Refuse { at: Instant::now() + WRONG_PASSWORD_DELAY, why: Refusal::WrongPassword };
            }
            (_, LobbyMsg::Leave) => p.stage = refuse(Refusal::Protocol),
            _ => p.stage = refuse(Refusal::Protocol),
        }
        None
    }

    /// Start the game from `snapshot` (`Snapshot::to_file` of the start state, already in the canonical view of the
    /// host's empire): every member gets its [`Start`], the links go to a lockstep [`Host`]. Refused (the lobby back)
    /// unless [`LobbyHost::can_start`]. Joiners still being admitted are turned away.
    pub fn start(mut self, snapshot: Vec<u8>, delay: u64) -> Result<Started, Box<Self>> {
        if !self.can_start() {
            return Err(Box::new(self));
        }
        for mut p in std::mem::take(&mut self.pending) {
            p.link.send(&Msg::Lobby(LobbyMsg::Refused(Refusal::Started)));
        }
        let humans: Vec<(u8, u8)> = self.state.members.iter().map(|m| (m.id, self.state.empire_of(m.id).unwrap())).collect();
        let mut host = Host::new(delay);
        let first_input = host.next_turn().max(delay);
        let start = |slot: u8, empire: u8| Start { slot, empire, delay, first_input, humans: humans.clone(), snapshot: snapshot.clone() };
        let (own_end, link) = loopback();
        let own = start(0, humans[0].1);
        host.add_peer(0, own.empire, Box::new(own_end));
        for &(id, empire) in &humans[1..] {
            let mut l = self.links.remove(&id).expect("every member but the host has a link");
            l.send(&Msg::Lobby(LobbyMsg::Start(Box::new(start(id, empire)))));
            let from = host.add_peer(id, empire, l);
            debug_assert_eq!(from, first_input);
        }
        Ok(Started { host, own, link })
    }
}

// ------------------------------------------------------------------------------------------------ joiner

/// Where a joiner stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Joining,
    InLobby,
    Started,
    Refused(Refusal),
}

/// The joiner's side of the lobby.
pub struct LobbyClient {
    link: Box<dyn Transport>,
    password: Option<String>,
    key: Option<Vec<u8>>,
    pub status: Status,
    pub member: Option<u8>,
    pub state: Option<LobbyState>,
    pub chat: Vec<(u8, String)>,
    start: Option<Start>,
}

impl LobbyClient {
    /// Say hello over `link` (`quic::connect_pinned`, or a loopback end). `code` = the join code of a private
    /// session.
    pub fn join(mut link: Box<dyn Transport>, name: &str, rules: [u8; 32], password: Option<String>, code: Option<u64>) -> Self {
        link.send(&Msg::Lobby(LobbyMsg::Hello { version: LOBBY_VERSION, rules, name: truncate(name, MAX_NAME), code }));
        Self { link, password, key: None, status: Status::Joining, member: None, state: None, chat: Vec::new(), start: None }
    }

    fn give_up(&mut self, why: Refusal) {
        self.link.send(&Msg::Lobby(LobbyMsg::Leave));
        self.status = Status::Refused(why);
    }

    /// Take the host's messages (up to the start: lockstep messages after it stay on the link for the peer).
    pub fn pump(&mut self) -> Result<&Status, wire::Error> {
        while matches!(self.status, Status::Joining | Status::InLobby) {
            let Some(m) = self.link.recv()? else { break };
            let Some(m) = lobby(m) else { continue };
            match m {
                LobbyMsg::Pake(msg) if self.status == Status::Joining && self.key.is_none() => {
                    let Some(pw) = self.password.as_deref() else {
                        self.give_up(Refusal::PasswordNeeded);
                        break;
                    };
                    let (spake, out) = Spake2::<Ed25519Group>::start_a(&Password::new(pw.as_bytes()), &Identity::new(ID_JOINER), &Identity::new(ID_HOST));
                    let Ok(key) = spake.finish(&msg) else {
                        self.give_up(Refusal::Protocol);
                        break;
                    };
                    self.link.send(&Msg::Lobby(LobbyMsg::Pake(out)));
                    self.link.send(&Msg::Lobby(LobbyMsg::Confirm(confirmation(&key, ID_JOINER, &self.link.binding()))));
                    self.key = Some(key);
                }
                LobbyMsg::Confirm(mac) if self.status == Status::Joining => {
                    let ok = self.key.as_ref().is_some_and(|k| same(&mac, &confirmation(k, ID_HOST, &self.link.binding())));
                    if !ok {
                        self.give_up(Refusal::HostNotAuthentic);
                        break;
                    }
                    // Confirmed: the key is spent.
                    self.key = Some(Vec::new());
                }
                LobbyMsg::Welcome { member } if self.status == Status::Joining => {
                    // A password session must have confirmed first.
                    if self.password.is_some() && self.key.as_ref().is_none_or(|k| !k.is_empty()) {
                        self.give_up(Refusal::HostNotAuthentic);
                        break;
                    }
                    self.member = Some(member);
                    self.status = Status::InLobby;
                }
                LobbyMsg::State(s) if self.status == Status::InLobby => self.state = Some(s),
                LobbyMsg::Said { member, text } if self.status == Status::InLobby => self.chat.push((member, text)),
                LobbyMsg::Start(s) if self.status == Status::InLobby => {
                    self.start = Some(*s);
                    self.status = Status::Started;
                }
                LobbyMsg::Refused(why) => self.status = Status::Refused(why),
                _ => {}
            }
        }
        Ok(&self.status)
    }

    pub fn pick(&mut self, empire: Option<u8>) {
        self.link.send(&Msg::Lobby(LobbyMsg::Pick { empire }));
    }

    pub fn ready(&mut self, ready: bool) {
        self.link.send(&Msg::Lobby(LobbyMsg::Ready(ready)));
    }

    pub fn say(&mut self, text: &str) {
        self.link.send(&Msg::Lobby(LobbyMsg::Chat(truncate(text, MAX_CHAT))));
    }

    pub fn leave(mut self) {
        self.link.send(&Msg::Lobby(LobbyMsg::Leave));
    }

    /// After the start: this member's [`Start`] and the link the lockstep continues on.
    pub fn into_game(self) -> Option<(Start, Box<dyn Transport>)> {
        Some((self.start?, self.link))
    }
}
