//! Our multiplayer screens (re/screens/start.md §4–§6, ADR 0014): the connection page (the LAN's sessions, host,
//! join from the list or by address, a password) and the room of a generated galaxy (eight slots with races and
//! holders, the host's options, chat, ready and start), drawn in the scenario panel's style over the main menu's
//! scene. They sit on `ig2_net::lobby`; the start hands a [`Begun`] to [`launch`], which builds the game on this
//! machine (`HostSession::start_galaxy` / `ClientSession::new_galaxy`).
//!
//! Links come from a [`Net`]: QUIC and LAN discovery ([`QuicNet`]) in the client, in-process loopback ends in tests.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use ig2_net::discovery::{Announcement, Found};
use ig2_net::galaxy::{GalaxyOptions, MAX_AIS, PLANETS};
use ig2_net::lobby::{LobbyClient, LobbyHost, LobbyState, Refusal, Settings, SlotKind, Start, Status};
use ig2_net::session::{ClientSession, HoldStatus, HostSession, SessionEvent, SessionSettings};
use ig2_net::{Peer, Transport};
use ig2_sim::game::new_game::StartData;
use ig2_sim::game::GameRules;

use crate::font::Align;
use crate::kit::fader;
use crate::options::{Draw, EntryKey};
use crate::render::{kind, Blend, Frame, Instance};

/// The game port a host listens on first (the LAN announcement carries the real one).
pub const GAME_PORT: u16 = ig2_net::discovery::DISCOVERY_PORT + 1;
/// The longest address and password typed.
const MAX_FIELD: usize = 40;
/// Chat lines shown.
const CHAT_LINES: usize = 6;
/// Sessions listed.
const LIST_ROWS: usize = 10;

// ------------------------------------------------------------------------------------------------ links

/// A host's listening end: new connections and the LAN announcement.
pub trait Listener {
    fn accept(&mut self) -> Option<Box<dyn Transport>>;
    /// The announcement to answer LAN queries with (`None`: stop).
    fn announce(&mut self, _a: Option<Announcement>) {}
    /// The game port and the session certificate's fingerprint (for the announcement).
    fn port(&self) -> u16 {
        0
    }
    fn fingerprint(&self) -> [u8; 32] {
        [0; 32]
    }
}

/// A connection attempt's outcome, from a background thread.
pub type Connecting = Receiver<Result<Box<dyn Transport + Send>, String>>;

/// Where links come from.
pub trait Net {
    fn listen(&mut self) -> Result<Box<dyn Listener>, String>;
    /// The sessions the LAN answered with so far (queried now and then).
    fn sessions(&mut self) -> Vec<Found>;
    /// Connect to `addr` ("host" or "host:port"), pinned by `pin` (a LAN announcement's fingerprint).
    fn connect(&mut self, addr: &str, pin: Option<[u8; 32]>) -> Connecting;
}

/// QUIC links and LAN discovery (ADR 0008).
#[derive(Default)]
pub struct QuicNet {
    browser: Option<ig2_net::discovery::Browser>,
    queried: Option<std::time::Instant>,
}

struct QuicListener {
    host: ig2_net::quic::QuicHost,
    announcer: Option<ig2_net::discovery::Announcer>,
}

impl Listener for QuicListener {
    fn accept(&mut self) -> Option<Box<dyn Transport>> {
        if let Some(a) = &mut self.announcer {
            if let Err(e) = a.poll() {
                eprintln!("ig2-client: LAN announcement: {e}");
            }
        }
        self.host.accept().map(|l| Box::new(l) as Box<dyn Transport>)
    }

    fn announce(&mut self, a: Option<Announcement>) {
        if let Some(an) = &mut self.announcer {
            an.set(a);
        }
    }

    fn port(&self) -> u16 {
        self.host.local_addr().map_or(0, |a| a.port())
    }

    fn fingerprint(&self) -> [u8; 32] {
        self.host.fingerprint()
    }
}

impl Net for QuicNet {
    fn listen(&mut self) -> Result<Box<dyn Listener>, String> {
        let any = |port| std::net::SocketAddr::from(([0, 0, 0, 0], port));
        let host = ig2_net::quic::QuicHost::bind(any(GAME_PORT)).or_else(|_| ig2_net::quic::QuicHost::bind(any(0))).map_err(|e| e.to_string())?;
        let announcer = ig2_net::discovery::Announcer::bind_lan().map_err(|e| eprintln!("ig2-client: LAN announcement: {e}")).ok();
        Ok(Box::new(QuicListener { host, announcer }))
    }

    fn sessions(&mut self) -> Vec<Found> {
        if self.browser.is_none() {
            self.browser = ig2_net::discovery::Browser::lan().map_err(|e| eprintln!("ig2-client: LAN browser: {e}")).ok();
        }
        let Some(b) = &mut self.browser else { return Vec::new() };
        if self.queried.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
            self.queried = Some(std::time::Instant::now());
            let _ = b.query();
        }
        let _ = b.poll();
        b.sessions().cloned().collect()
    }

    fn connect(&mut self, addr: &str, pin: Option<[u8; 32]>) -> Connecting {
        let (tx, rx) = mpsc::channel();
        let addr = addr.trim().to_string();
        std::thread::spawn(move || {
            use std::net::ToSocketAddrs;
            let with_port = if addr.parse::<std::net::SocketAddr>().is_ok() || addr.rsplit_once(':').is_some_and(|(h, p)| !h.contains(':') && p.parse::<u16>().is_ok()) { addr.clone() } else { format!("{addr}:{GAME_PORT}") };
            let r = with_port
                .to_socket_addrs()
                .map_err(|e| e.to_string())
                .and_then(|mut a| a.next().ok_or_else(|| "no address".to_string()))
                .and_then(|a| {
                    let pin = pin.map_or(ig2_net::quic::Pin::Any, ig2_net::quic::Pin::Fingerprint);
                    ig2_net::quic::connect_pinned(a, &pin).map_err(|e| e.to_string())
                })
                .map(|l| Box::new(l) as Box<dyn Transport + Send>);
            let _ = tx.send(r);
        });
        rx
    }
}

/// No network (the headless script runs): hosting gets a listener nobody reaches, the LAN is empty, joins fail.
pub struct OfflineNet;

struct Unreached;

impl Listener for Unreached {
    fn accept(&mut self) -> Option<Box<dyn Transport>> {
        None
    }
}

impl Net for OfflineNet {
    fn listen(&mut self) -> Result<Box<dyn Listener>, String> {
        Ok(Box::new(Unreached))
    }

    fn sessions(&mut self) -> Vec<Found> {
        Vec::new()
    }

    fn connect(&mut self, _addr: &str, _pin: Option<[u8; 32]>) -> Connecting {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(Err("offline".into()));
        rx
    }
}

// ------------------------------------------------------------------------------------------------ the screens

/// A text field (an address, a password, a chat line).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    pub text: Vec<u8>,
    pub cursor: usize,
    max: usize,
}

impl Field {
    fn new(max: usize) -> Self {
        Self { text: Vec::new(), cursor: 0, max }
    }

    /// A key; `true` on Enter.
    fn key(&mut self, k: EntryKey) -> bool {
        match k {
            EntryKey::Char(c) if c >= 0x20 && self.text.len() < self.max => {
                self.text.insert(self.cursor.min(self.text.len()), c);
                self.cursor += 1;
            }
            EntryKey::Back if self.cursor > 0 => {
                self.cursor -= 1;
                self.text.remove(self.cursor);
            }
            EntryKey::Delete if self.cursor < self.text.len() => {
                self.text.remove(self.cursor);
            }
            EntryKey::Left => self.cursor = self.cursor.saturating_sub(1),
            EntryKey::Right => self.cursor = (self.cursor + 1).min(self.text.len()),
            EntryKey::Home => self.cursor = 0,
            EntryKey::End => self.cursor = self.text.len(),
            EntryKey::Enter => return true,
            _ => {}
        }
        false
    }

    fn take(&mut self) -> String {
        self.cursor = 0;
        String::from_utf8_lossy(&std::mem::take(&mut self.text)).into_owned()
    }

    fn string(&self) -> String {
        String::from_utf8_lossy(&self.text).trim().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Address,
    Password,
}

/// Our side of the room.
pub enum Role {
    Host { lobby: Box<LobbyHost>, listener: Box<dyn Listener> },
    Joiner(Box<LobbyClient>),
}

/// The room's start, for [`launch`].
pub enum Begun {
    Host { lobby: Box<LobbyHost>, listener: Box<dyn Listener> },
    Joiner { start: Box<Start>, link: Box<dyn Transport> },
}

/// What a press or a key asks of the main menu.
pub enum Out {
    /// The menu click (`audio::CLICK`).
    Click,
    /// The screens are left: back to the multiplayer rings.
    Close,
    /// The room started: build the game ([`launch`]).
    Start(Begun),
}

/// Hot-spot ids (ours).
mod spot {
    pub const LIST: u32 = 0x10;
    pub const ADDRESS: u32 = 0x30;
    pub const PASSWORD: u32 = 0x31;
    pub const HOST: u32 = 0x32;
    pub const JOIN: u32 = 0x33;
    pub const CANCEL: u32 = 0x34;
    pub const RACE: u32 = 0x40;
    pub const HOLDER: u32 = 0x50;
    /// Option k's left arrow = OPTION + 2k, its right arrow + 1.
    pub const OPTION: u32 = 0x60;
    pub const LEAVE: u32 = 0x70;
    pub const READY: u32 = 0x71;
}

/// The options rows (spec §6).
const OPTIONS: usize = 5;

/// The multiplayer screens while they are up.
pub struct MpScreen {
    /// Opened from the "inet" ring (the same screen: no internet matchmaking, ADR 0014 §4).
    pub inet: bool,
    pub name: String,
    rules: [u8; 32],
    /// The fade 0..100 (+4 a 10 ms tick).
    pub open: f32,
    pub list: Vec<Found>,
    pub selected: Option<usize>,
    pub address: Field,
    pub password: Field,
    pub focus: Focus,
    /// The status line (a refusal, a connect error).
    pub line: Vec<u8>,
    /// The host was lost (`Start_ClientDeleted`'s joiner side: 14/5/3 "Disconnected" on the connection page).
    pub lost: bool,
    connecting: Option<(Connecting, Option<String>)>,
    pub role: Option<Role>,
    pub chat: Vec<(String, String)>,
    pub entry: Field,
    pub at: [f32; 2],
}

/// Our English words where the original has none (ADR 0014 §4).
mod words {
    pub const HOST: &[u8] = b"Host game";
    pub const PASSWORD: &[u8] = b"Password (optional)";
    pub const NONE_FOUND: &[u8] = b"No games found on the local network";
    pub const INET: &[u8] = b"No internet matchmaking: join by address";
    pub const CONNECTING: &[u8] = b"Connecting...";
    pub const COMPUTER: &[u8] = b"computer";
    pub const OPEN: &[u8] = b"-";
    pub const CLOSED: &[u8] = b"closed";
    pub const SHAPE: &[u8] = b"Galaxy shape:";
    pub const BATTLES: &[u8] = b"Battles:";
    pub const BATTLE: [&[u8]; 4] = [b"real time", b"space real time", b"ground real time", b"automatic"];
    pub const SPEED: &[u8] = b"Speed control:";
    pub const SPEED_BY: [&[u8]; 2] = [b"everyone", b"host"];
    pub const NEED_TWO: &[u8] = b"Two empires are needed to start";
    pub const WAITING: &[u8] = b"Waiting for every player to be ready";
}

fn refusal_text(r: Refusal) -> &'static [u8] {
    match r {
        Refusal::Version => b"Another game version",
        Refusal::Rules => b"Another game install (rules differ)",
        Refusal::Code => b"Wrong join code",
        Refusal::Full => b"The game is full",
        Refusal::Started => b"The game has started",
        Refusal::WrongPassword => b"Wrong password",
        Refusal::Kicked => b"Removed by the host",
        Refusal::Protocol | Refusal::HostNotAuthentic => b"Connection refused",
        Refusal::TimedOut => b"Timed out",
        Refusal::PasswordNeeded => b"The game needs a password",
        Refusal::Connected => b"That player is connected",
    }
}

impl MpScreen {
    /// The connection page (`name` = `MultiplayerNev`, `rules` = the rules fingerprint joiners must match).
    pub fn open(inet: bool, name: &str, rules: [u8; 32]) -> Self {
        Self {
            inet,
            name: name.to_string(),
            rules,
            open: 0.0,
            list: Vec::new(),
            selected: None,
            address: Field::new(MAX_FIELD),
            password: Field::new(MAX_FIELD),
            focus: Focus::Address,
            line: if inet { words::INET.to_vec() } else { Vec::new() },
            lost: false,
            connecting: None,
            role: None,
            chat: Vec::new(),
            entry: Field::new(ig2_net::lobby::MAX_CHAT),
            at: [0.0; 2],
        }
    }

    /// The room's state as this member sees it.
    pub fn state(&self) -> Option<&LobbyState> {
        match self.role.as_ref()? {
            Role::Host { lobby, .. } => Some(lobby.state()),
            Role::Joiner(c) => c.state.as_ref(),
        }
    }

    /// This member's id (the host is 0).
    pub fn member(&self) -> Option<u8> {
        match self.role.as_ref()? {
            Role::Host { .. } => Some(0),
            Role::Joiner(c) => c.member,
        }
    }

    fn is_host(&self) -> bool {
        matches!(self.role, Some(Role::Host { .. }))
    }

    fn password(&self) -> Option<String> {
        Some(self.password.string()).filter(|p| !p.is_empty())
    }

    /// Host a room (the connection page's Host).
    pub fn host(&mut self, net: &mut dyn Net) {
        match net.listen() {
            Ok(listener) => {
                let settings = Settings { name: format!("{}'s galaxy", self.name), scenario: "galaxy".into(), private: false, password: self.password() };
                let lobby = LobbyHost::new_galaxy(settings, ig2_net::discovery::random_u64(), self.rules, &self.name);
                self.chat.clear();
                self.role = Some(Role::Host { lobby: Box::new(lobby), listener });
                self.line.clear();
            }
            Err(e) => self.line = format!("Can't host: {e}").into_bytes(),
        }
    }

    /// Join the selected LAN session, else the typed address.
    pub fn join(&mut self, net: &mut dyn Net) {
        let (addr, pin) = match self.selected.and_then(|i| self.list.get(i)) {
            Some(f) => (f.addr.to_string(), Some(f.announcement.fingerprint)),
            None if !self.address.string().is_empty() => (self.address.string(), None),
            None => return,
        };
        self.connecting = Some((net.connect(&addr, pin), self.password()));
        self.line = words::CONNECTING.to_vec();
    }

    /// Join over `link` (a connection that just came up).
    pub fn join_link(&mut self, link: Box<dyn Transport>, password: Option<String>) {
        self.chat.clear();
        self.role = Some(Role::Joiner(Box::new(LobbyClient::join(link, &self.name, self.rules, password, None))));
        self.line.clear();
        self.lost = false;
    }

    /// Leave the room for the connection page.
    fn leave(&mut self) {
        match self.role.take() {
            Some(Role::Joiner(c)) => (*c).leave(),
            Some(Role::Host { mut listener, .. }) => listener.announce(None),
            None => {}
        }
    }

    /// One 10 ms tick: the fade, the LAN list, the connection attempt, the room's messages. A started room comes
    /// back as [`Out::Start`].
    pub fn tick(&mut self, net: &mut dyn Net) -> Option<Out> {
        self.open = (self.open + 4.0).min(100.0);
        if self.role.is_none() {
            self.list = net.sessions();
            self.list.sort_by(|a, b| a.announcement.name.cmp(&b.announcement.name));
            if self.selected.is_some_and(|i| i >= self.list.len()) {
                self.selected = None;
            }
        }
        if let Some((rx, pw)) = &self.connecting {
            match rx.try_recv() {
                Ok(Ok(link)) => {
                    let pw = pw.clone();
                    self.connecting = None;
                    self.join_link(link, pw);
                }
                Ok(Err(e)) => {
                    self.connecting = None;
                    self.line = e.into_bytes();
                }
                Err(mpsc::TryRecvError::Disconnected) => self.connecting = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match self.role.as_mut()? {
            Role::Host { lobby, listener } => {
                while let Some(l) = listener.accept() {
                    lobby.admit(l);
                }
                match lobby.pump() {
                    Ok(events) => {
                        for e in events {
                            if let ig2_net::lobby::LobbyEvent::Said(m, text) = e {
                                let who = lobby.state().members.iter().find(|x| x.id == m).map_or_else(String::new, |x| x.name.clone());
                                self.chat.push((who, text));
                            }
                        }
                    }
                    Err(e) => eprintln!("ig2-client: lobby: {e}"),
                }
                let a = lobby.announcement(listener.port(), listener.fingerprint());
                listener.announce(a);
                None
            }
            Role::Joiner(c) => {
                let status = match c.pump() {
                    Ok(s) => s.clone(),
                    Err(e) => {
                        eprintln!("ig2-client: lobby: {e}");
                        Status::Refused(Refusal::Protocol)
                    }
                };
                let names: Vec<(u8, String)> = c.state.as_ref().map(|s| s.members.iter().map(|m| (m.id, m.name.clone())).collect()).unwrap_or_default();
                for (m, text) in c.chat.drain(..) {
                    let who = names.iter().find(|x| x.0 == m).map_or_else(String::new, |x| x.1.clone());
                    self.chat.push((who, text));
                }
                if matches!(status, Status::Joining | Status::InLobby) && c.closed() {
                    self.role = None;
                    self.line.clear();
                    self.lost = true;
                    return None;
                }
                match status {
                    Status::Started => {
                        let Some(Role::Joiner(c)) = self.role.take() else { unreachable!() };
                        let (start, link) = (*c).into_game()?;
                        Some(Out::Start(Begun::Joiner { start: Box::new(start), link }))
                    }
                    Status::Refused(r) => {
                        self.role = None;
                        self.line = refusal_text(r).to_vec();
                        None
                    }
                    _ => None,
                }
            }
        }
    }

    // -------------------------------------------------------------------------------------------- input

    /// The boxes switched on now (id, rect).
    fn boxes(&self) -> Vec<(u32, [f32; 4])> {
        let mut v = Vec::new();
        if self.open < 100.0 {
            return v;
        }
        match self.state() {
            None => {
                for i in 0..self.list.len().min(LIST_ROWS) {
                    v.push((spot::LIST + i as u32, [130.0, 82.0 + 16.0 * i as f32, 380.0, 16.0]));
                }
                v.extend([
                    (spot::ADDRESS, [130.0, 280.0, 380.0, 22.0]),
                    (spot::PASSWORD, [130.0, 328.0, 380.0, 22.0]),
                    (spot::HOST, [130.0, 392.0, 120.0, 30.0]),
                    (spot::JOIN, [260.0, 392.0, 120.0, 30.0]),
                    (spot::CANCEL, [390.0, 392.0, 120.0, 30.0]),
                ]);
            }
            Some(_) => {
                for i in 0..8 {
                    let y = 52.0 + 32.0 * i as f32;
                    v.push((spot::RACE + i, [44.0, y, 120.0, 26.0]));
                    v.push((spot::HOLDER + i, [170.0, y, 160.0, 26.0]));
                }
                if self.is_host() {
                    for k in 0..OPTIONS as u32 {
                        let y = 74.0 + 48.0 * k as f32;
                        v.push((spot::OPTION + 2 * k, [556.0, y, 30.0, 18.0]));
                        v.push((spot::OPTION + 2 * k + 1, [588.0, y, 30.0, 18.0]));
                    }
                }
                v.extend([(spot::LEAVE, [16.0, 444.0, 120.0, 30.0]), (spot::READY, [504.0, 444.0, 120.0, 30.0])]);
            }
        }
        v
    }

    pub fn spot_at(&self, at: [f32; 2]) -> u32 {
        let (x, y) = (at[0].floor(), at[1].floor());
        self.boxes().into_iter().find(|&(_, r)| r[0] <= x && x < r[0] + r[2] && r[1] <= y && y < r[1] + r[3]).map_or(0, |(id, _)| id)
    }

    pub fn hover(&mut self, at: [f32; 2]) {
        self.at = at;
    }

    /// A press at `at` (`right`: the right button).
    pub fn press(&mut self, at: [f32; 2], right: bool, net: &mut dyn Net) -> Option<Out> {
        self.at = at;
        let id = self.spot_at(at);
        if id == 0 {
            return None;
        }
        if self.role.is_none() {
            match id {
                spot::ADDRESS => {
                    self.focus = Focus::Address;
                    self.selected = None;
                }
                spot::PASSWORD => self.focus = Focus::Password,
                spot::HOST => self.host(net),
                spot::JOIN => self.join(net),
                spot::CANCEL => return Some(Out::Close),
                i if (spot::LIST..spot::LIST + LIST_ROWS as u32).contains(&i) => {
                    let k = (i - spot::LIST) as usize;
                    if self.selected == Some(k) {
                        self.join(net);
                    }
                    self.selected = Some(k);
                }
                _ => {}
            }
            return Some(Out::Click);
        }
        let st = self.state()?.clone();
        let me = self.member()?;
        let mine = st.empire_of(me);
        let slot = |i: u32| st.slots.get(i as usize).copied();
        match id {
            spot::LEAVE => self.leave(),
            spot::READY => {
                let ready = st.members.iter().find(|m| m.id == me).is_some_and(|m| m.ready);
                match self.role.as_mut()? {
                    // The host's press is its ready and the start (exe 0x5c0120's button 0x44).
                    Role::Host { lobby, .. } => {
                        lobby.set_ready(0, true);
                        if lobby.can_start() {
                            let Some(Role::Host { lobby, mut listener }) = self.role.take() else { unreachable!() };
                            listener.announce(None);
                            return Some(Out::Start(Begun::Host { lobby, listener }));
                        }
                        let two = st.members.len() + st.ais().len() >= 2;
                        self.line = if two { words::WAITING } else { words::NEED_TWO }.to_vec();
                    }
                    Role::Joiner(c) => c.ready(!ready),
                }
            }
            i if (spot::RACE..spot::RACE + 8).contains(&i) => {
                let s = slot(i - spot::RACE)?;
                if !self.is_host() && mine != Some(s.empire) {
                    return None;
                }
                let race = if right { (s.race + 6) % 8 + 1 } else { s.race % 8 + 1 };
                match self.role.as_mut()? {
                    Role::Host { lobby, .. } => {
                        lobby.set_race(0, s.empire, race);
                    }
                    Role::Joiner(c) => c.race(s.empire, race),
                }
            }
            i if (spot::HOLDER..spot::HOLDER + 8).contains(&i) => {
                let s = slot(i - spot::HOLDER)?;
                match (self.role.as_mut()?, right) {
                    // The host's right click: empty → AI → closed → empty.
                    (Role::Host { lobby, .. }, true) => {
                        let next = match s.kind {
                            SlotKind::Open => SlotKind::Ai,
                            SlotKind::Ai => SlotKind::Closed,
                            SlotKind::Closed => SlotKind::Open,
                            SlotKind::Human(_) => return None,
                        };
                        lobby.set_slot(s.empire, next);
                    }
                    (Role::Host { lobby, .. }, false) => {
                        lobby.pick(0, Some(s.empire));
                    }
                    (Role::Joiner(c), false) if matches!(s.kind, SlotKind::Open | SlotKind::Ai) => c.pick(Some(s.empire)),
                    _ => return None,
                }
            }
            i if (spot::OPTION..spot::OPTION + 2 * OPTIONS as u32).contains(&i) => {
                let Some(Role::Host { lobby, .. }) = self.role.as_mut() else { return None };
                let o = lobby.state().galaxy?;
                let (k, up) = ((i - spot::OPTION) / 2, (i - spot::OPTION) % 2 == 1);
                let step = |v: u8, lo: u8, hi: u8| if up { (v + 1).min(hi) } else { v.saturating_sub(1).max(lo) };
                let n = match k {
                    0 => {
                        let at = PLANETS.iter().position(|&p| p == o.planets).unwrap_or(1) as u8;
                        GalaxyOptions { planets: PLANETS[step(at, 0, 3) as usize], ..o }
                    }
                    1 => GalaxyOptions { ais: step(o.ais, 0, MAX_AIS), ..o },
                    2 => GalaxyOptions { shape: step(o.shape, 1, 2), ..o },
                    3 => GalaxyOptions { battle_status: step(o.battle_status, 0, 3), ..o },
                    _ => GalaxyOptions { host_time: up, ..o },
                };
                lobby.set_options(n);
            }
            _ => return None,
        }
        Some(Out::Click)
    }

    /// A key: the focused field's (the connection page) or the chat line's (the room); Esc leaves the room, then the
    /// screens.
    pub fn key(&mut self, k: Option<EntryKey>, escape: bool, net: &mut dyn Net) -> Option<Out> {
        if escape {
            if self.role.is_some() {
                self.leave();
                return None;
            }
            return Some(Out::Close);
        }
        let k = k?;
        if let Some(role) = &mut self.role {
            if self.entry.key(k) {
                let text = self.entry.take();
                if !text.trim().is_empty() {
                    match role {
                        Role::Host { lobby, .. } => {
                            lobby.say(0, &text);
                            let name = lobby.state().members[0].name.clone();
                            self.chat.push((name, text));
                        }
                        Role::Joiner(c) => c.say(&text),
                    }
                }
            }
            return None;
        }
        let f = match self.focus {
            Focus::Address => &mut self.address,
            Focus::Password => &mut self.password,
        };
        if f.key(k) {
            self.selected = None;
            self.join(net);
        }
        None
    }

    // -------------------------------------------------------------------------------------------- drawing

    pub fn draw(&self, f: &mut Frame, d: &Draw, blink: bool) {
        let v = &d.v;
        let t = 1.0 - self.open / 100.0;
        let (t1, t2) = (t * 155.0 + 100.0, t * 225.0 + 30.0);
        let box_ = |f: &mut Frame, r: [f32; 4]| fader::kirak_fader(f, d.white, v, r, [0.0, 54.0, 90.0], t1, [0.0, 90.0, 130.0], t2, [0.0, 0.0]);
        let fade = self.open;
        let hover = self.spot_at(self.at);
        let text = |f: &mut Frame, s: &[u8], x: f32, y: f32, size: usize, a: Align, c: usize| d.font.text(f, v, s, x, y, 0, size, a, d.col(c, fade), None);
        let hot = |id: u32| if hover == id { 2 } else { 1 };
        let cursor = |f: &mut Frame, fl: &Field, x: f32, y: f32, size: usize| {
            if blink {
                let (w, h) = (d.font.advance(size), d.font.height(size));
                let cx = x + w * fl.cursor as f32;
                let c = d.col(1, fade);
                f.push(d.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(cx + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
            }
        };
        let Some(st) = self.state() else {
            // The connection page.
            for r in [[120.0, 40.0, 400.0, 400.0], [125.0, 45.0, 390.0, 24.0], [125.0, 76.0, 390.0, 172.0], [130.0, 280.0, 380.0, 22.0], [130.0, 328.0, 380.0, 22.0]] {
                box_(f, r);
            }
            for x in [130.0, 260.0, 390.0] {
                box_(f, [x, 392.0, 120.0, 30.0]);
            }
            let title = d.texts.get(15, 1, if self.inet { 30 } else { 29 });
            text(f, &title, 320.0, 50.0, 3, Align::Centre, 1);
            if self.list.is_empty() {
                text(f, words::NONE_FOUND, 320.0, 150.0, 2, Align::Centre, 0x25);
            }
            for (i, s) in self.list.iter().take(LIST_ROWS).enumerate() {
                let a = &s.announcement;
                let y = 84.0 + 16.0 * i as f32;
                let c = if self.selected == Some(i) { 2 } else if a.rules != self.rules || a.started { 0x25 } else { 0x22 };
                let lock = if a.password { "* " } else { "" };
                text(f, format!("{lock}{}", a.name).as_bytes(), 136.0, y, 2, Align::Left, c);
                text(f, format!("{}/{}", a.humans, a.slots).as_bytes(), 500.0, y, 2, Align::Right, c);
            }
            text(f, &d.texts.get(14, 5, 2), 130.0, 266.0, 2, Align::Left, 0x25);
            text(f, &self.address.text, 136.0, 285.0, 2, Align::Left, if self.focus == Focus::Address { 2 } else { 0x22 });
            text(f, words::PASSWORD, 130.0, 314.0, 2, Align::Left, 0x25);
            let stars = vec![b'*'; self.password.text.len()];
            text(f, &stars, 136.0, 333.0, 2, Align::Left, if self.focus == Focus::Password { 2 } else { 0x22 });
            match self.focus {
                Focus::Address => cursor(f, &self.address, 136.0, 285.0, 2),
                Focus::Password => cursor(f, &self.password, 136.0, 333.0, 2),
            }
            let line = if self.lost && self.line.is_empty() { d.texts.get(14, 5, 3) } else { self.line.clone() };
            text(f, &line, 320.0, 364.0, 2, Align::Centre, 2);
            text(f, words::HOST, 190.0, 400.0, 3, Align::Centre, hot(spot::HOST));
            text(f, &d.texts.get(16, 3, 3), 320.0, 400.0, 3, Align::Centre, hot(spot::JOIN));
            text(f, &d.texts.get(15, 3, 7), 450.0, 400.0, 3, Align::Centre, hot(spot::CANCEL));
            return;
        };
        // The room.
        for r in [[16.0, 8.0, 608.0, 26.0], [16.0, 40.0, 380.0, 272.0], [404.0, 40.0, 220.0, 272.0], [16.0, 320.0, 608.0, 120.0], [16.0, 444.0, 120.0, 30.0], [504.0, 444.0, 120.0, 30.0]] {
            box_(f, r);
        }
        text(f, st.name.as_bytes(), 320.0, 13.0, 3, Align::Centre, 1);
        let me = self.member();
        for (i, s) in st.slots.iter().enumerate().take(8) {
            let y = 58.0 + 32.0 * i as f32;
            let own = me.is_some_and(|m| s.kind == SlotKind::Human(m));
            text(f, format!("{}", s.empire).as_bytes(), 26.0, y, 2, Align::Left, if own { 2 } else { 0x25 });
            let race_c = if hover == spot::RACE + i as u32 && (own || self.is_host()) { 2 } else { 0x22 };
            text(f, &d.texts.get(0x11, 1, (s.race.clamp(0, 19) + 5) as u16), 48.0, y, 2, Align::Left, race_c);
            let (who, ready): (Vec<u8>, bool) = match s.kind {
                SlotKind::Human(m) => st.members.iter().find(|x| x.id == m).map_or((Vec::new(), false), |x| (x.name.as_bytes().to_vec(), x.ready)),
                SlotKind::Ai => (words::COMPUTER.to_vec(), false),
                SlotKind::Open => (words::OPEN.to_vec(), false),
                SlotKind::Closed => (words::CLOSED.to_vec(), false),
            };
            let holder_c = if own || (hover == spot::HOLDER + i as u32 && matches!(s.kind, SlotKind::Open | SlotKind::Ai)) { 2 } else { 0x22 };
            text(f, &who, 176.0, y, 2, Align::Left, holder_c);
            if ready {
                text(f, &d.texts.get(14, 5, 6), 386.0, y, 2, Align::Right, 2);
            }
        }
        if let Some(o) = st.galaxy {
            let rows: [(Vec<u8>, Vec<u8>); OPTIONS] = [
                (b"".to_vec(), d.texts.get(14, 2, PLANETS.iter().position(|&p| p == o.planets).unwrap_or(1) as u16 + 1)),
                (d.texts.get(14, 1, 1), format!("{}", o.ais).into_bytes()),
                (words::SHAPE.to_vec(), format!("{}", o.shape).into_bytes()),
                (words::BATTLES.to_vec(), words::BATTLE[o.battle_status.min(3) as usize].to_vec()),
                (words::SPEED.to_vec(), words::SPEED_BY[o.host_time as usize].to_vec()),
            ];
            let mut planets = d.texts.get(15, 2, 8);
            planets.push(b':');
            for (k, (label, value)) in rows.iter().enumerate() {
                let y = 60.0 + 48.0 * k as f32;
                text(f, if k == 0 { &planets } else { label }, 414.0, y, 2, Align::Left, 0x25);
                text(f, value, 414.0, y + 18.0, 2, Align::Left, 0x22);
                if self.is_host() {
                    let id = spot::OPTION + 2 * k as u32;
                    text(f, b"<-", 571.0, y + 18.0, 2, Align::Centre, hot(id));
                    text(f, b"->", 603.0, y + 18.0, 2, Align::Centre, hot(id + 1));
                }
            }
        }
        let first = self.chat.len().saturating_sub(CHAT_LINES);
        for (j, (who, line)) in self.chat[first..].iter().enumerate() {
            text(f, format!("{who}: {line}").as_bytes(), 24.0, 328.0 + 14.0 * j as f32, 2, Align::Left, 0x22);
        }
        text(f, &self.entry.text, 24.0, 418.0, 2, Align::Left, 2);
        cursor(f, &self.entry, 24.0, 418.0, 2);
        text(f, &self.line, 320.0, 452.0, 2, Align::Centre, 2);
        text(f, &d.texts.get(14, 1, 11), 76.0, 452.0, 3, Align::Centre, hot(spot::LEAVE));
        let (label, c) = if self.is_host() {
            (d.texts.get(14, 1, 12), hot(spot::READY))
        } else {
            let ready = st.members.iter().find(|m| Some(m.id) == me).is_some_and(|m| m.ready);
            (d.texts.get(14, 5, 6), if ready { 2 } else { hot(spot::READY) })
        };
        text(f, &label, 564.0, 452.0, 3, Align::Centre, c);
    }
}

// ------------------------------------------------------------------------------------------------ the session

/// The running MP game on this machine.
pub enum Session<'r> {
    Host { session: Box<HostSession<'r>>, listener: Box<dyn Listener> },
    Client(Box<ClientSession<'r>>),
}

impl<'r> Session<'r> {
    /// This machine's player.
    pub fn peer(&self) -> &Peer<'r> {
        match self {
            Session::Host { session, .. } => &session.own,
            Session::Client(c) => &c.peer,
        }
    }

    pub fn peer_mut(&mut self) -> &mut Peer<'r> {
        match self {
            Session::Host { session, .. } => &mut session.own,
            Session::Client(c) => &mut c.peer,
        }
    }

    /// What holds the session now (the host's own view, a joiner's last word from the host).
    pub fn status(&self, now: Duration) -> HoldStatus {
        match self {
            Session::Host { session, .. } => session.status(now),
            Session::Client(c) => c.status.clone(),
        }
    }

    /// A joiner's host is gone (left, the link closed or silent). The joiners don't migrate yet (ADR 0012's glue needs
    /// the survivors' addresses): the game ends for them as it did in the original (`MultiCloseServer`).
    pub fn host_gone(&self) -> bool {
        match self {
            Session::Host { .. } => false,
            Session::Client(c) => c.closed(),
        }
    }

    /// The local player's chat line (`ExecuteString`'s 0x3fc): every machine shows it, this one included.
    pub fn say(&mut self, text: &str) {
        match self {
            Session::Host { session, .. } => session.say(text),
            Session::Client(c) => c.say(text),
        }
    }

    /// The name `empire`'s player had in the room.
    pub fn name_of(&self, empire: u8) -> String {
        match self {
            Session::Host { session, .. } => session.humans().find(|h| h.0 == empire).map(|h| h.1.to_string()),
            Session::Client(c) => {
                let slot = c.humans().iter().find(|h| h.1 == empire).map(|h| h.0);
                c.info().names.iter().find(|n| Some(n.0) == slot).map(|n| n.1.clone())
            }
        }
        .unwrap_or_else(|| format!("Empire {empire}"))
    }

    /// `ExitToMainScreen`'s teardown (game-end.md §2: `MultiCloseServer` / `MultiCloseClient`): the others are told
    /// this player left (a joiner is away at once; the host's survivors migrate), then the links close as the session
    /// drops. A `Leave` lost on the way is the drop path's (closed link, keep-alive).
    pub fn leave(mut self) {
        match &mut self {
            Session::Host { session, .. } => session.leave(),
            Session::Client(c) => c.leave(),
        }
    }

    /// At `now`: admit rejoiners (host), receive, send, run up to `max_turns` turns. Returns the turns run and the
    /// host's session events.
    pub fn pump(&mut self, now: Duration, max_turns: usize) -> Result<(usize, Vec<SessionEvent>), String> {
        match self {
            Session::Host { session, listener } => {
                while let Some(l) = listener.accept() {
                    session.admit(l, now);
                }
                let before = session.own.turn;
                let events = session.pump(now, max_turns).map_err(|e| e.to_string())?;
                Ok(((session.own.turn - before) as usize, events))
            }
            Session::Client(c) => {
                let run = c.pump(now, max_turns).map_err(|e| e.to_string())?;
                Ok((run, c.chat.drain(..).map(|(empire, text)| SessionEvent::Said { empire, text }).collect()))
            }
        }
    }
}

/// The room's start on this machine (ADR 0014 §2): the host builds the galaxy and starts the session with `ticks`
/// (its clock); a joiner builds the same galaxy from the host's `GalaxyStart` and checks its hash, or, joining a
/// running game under its old name, starts from the host's snapshot (the rejoin, `rules/multiplayer.md` §7.2).
pub fn launch<'r>(b: Begun, data: &StartData, rules: &'r GameRules, ticks: u32) -> Result<Session<'r>, String> {
    match b {
        Begun::Host { lobby, listener } => {
            let s = HostSession::start_galaxy(*lobby, data, rules, SessionSettings::default(), ticks).map_err(|_| "the room can't start yet".to_string())?;
            Ok(Session::Host { session: Box::new(s), listener })
        }
        // A rejoin to a running game (the host's door matched the name to an away human): the host's state now.
        Begun::Joiner { start, link } if start.galaxy.is_none() => ClientSession::new(&start, link, rules).map(|c| Session::Client(Box::new(c))).map_err(|e| e.to_string()),
        Begun::Joiner { start, link } => ClientSession::new_galaxy(&start, link, data, rules).map(|c| Session::Client(Box::new(c))).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    const RULES: [u8; 32] = [0x3c; 32];

    /// In-process links: a host's listener takes the loopback ends joiners connect with.
    type Hub = Rc<RefCell<Option<mpsc::Sender<Box<dyn Transport>>>>>;

    #[derive(Clone, Default)]
    struct LoopNet {
        hub: Hub,
    }

    struct LoopListener(Receiver<Box<dyn Transport>>);

    impl Listener for LoopListener {
        fn accept(&mut self) -> Option<Box<dyn Transport>> {
            self.0.try_recv().ok()
        }
    }

    impl Net for LoopNet {
        fn listen(&mut self) -> Result<Box<dyn Listener>, String> {
            let (tx, rx) = mpsc::channel();
            *self.hub.borrow_mut() = Some(tx);
            Ok(Box::new(LoopListener(rx)))
        }

        fn sessions(&mut self) -> Vec<Found> {
            Vec::new()
        }

        fn connect(&mut self, _addr: &str, _pin: Option<[u8; 32]>) -> Connecting {
            let (tx, rx) = mpsc::channel();
            let (a, b) = ig2_net::loopback();
            let r = match self.hub.borrow().as_ref() {
                Some(h) => h.send(Box::new(b)).map(|_| Box::new(a) as Box<dyn Transport + Send>).map_err(|e| e.to_string()),
                None => Err("nobody hosts".into()),
            };
            tx.send(r).unwrap();
            rx
        }
    }

    /// The centre of hot-spot `id` on `s`.
    fn at(s: &MpScreen, id: u32) -> [f32; 2] {
        let r = s.boxes().into_iter().find(|b| b.0 == id).unwrap_or_else(|| panic!("spot {id:#x} is off")).1;
        [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
    }

    fn click(s: &mut MpScreen, id: u32, right: bool, net: &mut LoopNet) -> Option<Out> {
        let p = at(s, id);
        s.press(p, right, net)
    }

    /// Tick every screen until `done` (2 s at most); returns the starts that came out.
    fn run(screens: &mut [&mut MpScreen], net: &mut LoopNet, done: impl Fn(&[&mut MpScreen]) -> bool) -> Vec<(usize, Begun)> {
        let t = std::time::Instant::now();
        let mut begun = Vec::new();
        loop {
            for (i, s) in screens.iter_mut().enumerate() {
                if let Some(Out::Start(b)) = s.tick(net) {
                    begun.push((i, b));
                }
            }
            if done(screens) {
                return begun;
            }
            assert!(t.elapsed() < Duration::from_secs(2), "timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn type_text(s: &mut MpScreen, text: &str, net: &mut LoopNet) {
        for b in text.bytes() {
            s.key(Some(EntryKey::Char(b)), false, net);
        }
        s.key(Some(EntryKey::Enter), false, net);
    }

    /// Host and joiners through the screens' own input: connect, seat, race, move, options, chat, ready, start.
    fn room(joiners: usize, net: &mut LoopNet) -> (MpScreen, Vec<MpScreen>) {
        let mut host = MpScreen::open(false, "Host", RULES);
        let mut guests: Vec<MpScreen> = (0..joiners).map(|k| MpScreen::open(k == 0, &format!("Guest{k}"), RULES)).collect();
        for _ in 0..25 {
            host.tick(net);
            for g in &mut guests {
                g.tick(net);
            }
        }
        assert!(matches!(click(&mut host, spot::HOST, false, net), Some(Out::Click)));
        assert!(host.is_host());
        // No AIs: the planets one step down, the AI count down to 0.
        click(&mut host, spot::OPTION, false, net);
        for _ in 0..2 {
            click(&mut host, spot::OPTION + 2, false, net);
        }
        let o = host.state().unwrap().galaxy.unwrap();
        assert_eq!((o.planets, o.ais), (20, 0));
        for g in &mut guests {
            type_text(g, "local", net);
        }
        let n = joiners + 1;
        let mut all: Vec<&mut MpScreen> = std::iter::once(&mut host).chain(guests.iter_mut()).collect();
        run(&mut all, net, |s| s.iter().all(|s| s.state().is_some_and(|st| st.members.len() == n)) && s.iter().skip(1).all(|g| g.state() == s[0].state()));
        drop(all);
        let st = host.state().unwrap().clone();
        for (k, g) in guests.iter().enumerate() {
            assert_eq!(st.empire_of(g.member().unwrap()), Some(k as u8 + 2), "seated in the first empty slots");
        }
        (host, guests)
    }

    fn sync(host: &mut MpScreen, guests: &mut [MpScreen], net: &mut LoopNet, done: impl Fn(&MpScreen) -> bool) {
        let mut all: Vec<&mut MpScreen> = std::iter::once(host).chain(guests.iter_mut()).collect();
        run(&mut all, net, |s| done(s[0]) && s.iter().skip(1).all(|g| g.state() == s[0].state()));
    }

    /// Tick the guests until each one's start came out.
    fn started(guests: &mut [MpScreen], net: &mut LoopNet) -> Vec<Begun> {
        let t = std::time::Instant::now();
        let mut begun: Vec<Option<Begun>> = guests.iter().map(|_| None).collect();
        while begun.iter().any(Option::is_none) {
            for (g, b) in guests.iter_mut().zip(begun.iter_mut()) {
                if let Some(Out::Start(s)) = g.tick(net) {
                    *b = Some(s);
                }
            }
            assert!(t.elapsed() < Duration::from_secs(2), "no start");
        }
        begun.into_iter().flatten().collect()
    }

    /// Two and three machines: the room's start builds the same galaxy on each (`launch`), the worlds run the
    /// session in step (`World::new_mp`, `World::tick`) and a command from a joiner reaches every machine.
    #[test]
    fn rooms_of_two_and_three_start_the_same_galaxy() {
        let Some(Ok(inst)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let rules: &'static GameRules = Box::leak(Box::new(crate::rules::load(&inst).unwrap().0));
        let texts = crate::new_game::StartTexts::read(&inst).unwrap();
        for joiners in [1, 2] {
            let mut net = LoopNet::default();
            let (mut host, mut guests) = room(joiners, &mut net);
            for g in &mut guests {
                click(g, spot::READY, false, &mut net);
            }
            sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready));
            let Some(Out::Start(b)) = click(&mut host, spot::READY, false, &mut net) else { panic!("no start") };
            let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 4242)).unwrap().map(|s| crate::world::World::new_mp(s, rules)).unwrap();
            // The host builds first: its start sends every joiner its `Start`.
            let mut worlds = vec![mk(b)];
            let begun = started(&mut guests, &mut net);
            assert!(begun.iter().all(|b| matches!(b, Begun::Joiner { start, .. } if start.galaxy.is_some() && start.snapshot.is_empty())));
            worlds.extend(begun.into_iter().map(mk));
            let empires: Vec<u8> = worlds.iter().map(|w| w.net.as_ref().unwrap().0.peer().empire).collect();
            assert_eq!(empires, (1..=joiners as u8 + 1).collect::<Vec<_>>());
            for w in &worlds {
                assert_eq!(w.game.state.mem.i32(ig2_sim::save::va::PLANET_COUNT), 20);
            }
            // The last joiner sets its first planet's tax (a starmap order) through its world's outbox.
            let t = std::time::Instant::now();
            let mut sent = false;
            while worlds.iter().any(|w| w.net.as_ref().unwrap().0.peer().turn < 30) {
                for w in &mut worlds {
                    w.pump();
                    w.tick();
                }
                if !sent && worlds[0].net.as_ref().unwrap().0.peer().turn >= 5 {
                    let w = worlds.last_mut().unwrap();
                    w.send(ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::Setting { planet: 1, what: 9, value: 3 } });
                    sent = true;
                }
                assert!(t.elapsed() < Duration::from_secs(60), "stalled");
                std::thread::sleep(Duration::from_millis(1));
            }
            let hashes = |w: &crate::world::World| w.net.as_ref().unwrap().0.peer().hashes.clone();
            let h0 = hashes(&worlds[0]);
            for w in &worlds[1..] {
                let h = hashes(w);
                let common: Vec<_> = h.iter().filter(|x| h0.contains(x)).collect();
                assert!(common.len() >= 25 && h.iter().all(|(t, x)| h0.iter().find(|(t0, _)| t0 == t).is_none_or(|(_, x0)| x0 == x)), "{joiners} joiners: the peers agree every turn");
            }
            let Some((Session::Host { session, .. }, _)) = &worlds[0].net else { panic!("the host's world") };
            let joiner = *empires.last().unwrap();
            assert!(session.host.log.iter().flat_map(|b| &b.commands).any(|c| c.issuer == joiner && matches!(c.command, ig2_net::Command::Starmap { .. })), "the joiner's order ran in a bundle");
            let stand_in = &worlds[0].net.as_ref().unwrap().0.peer().game;
            assert!(stand_in.state.fleets.is_empty(), "the peer holds the stand-in between pumps");
        }
    }

    /// The game loop on three machines (FX15): each world's screens see its own `EN`; a chat line reaches every
    /// machine; the host's MP save reads back, a joiner has none; a joiner who leaves is away on the others and
    /// comes back through the room's Join under its name (the rejoin's snapshot start); the peers agree throughout;
    /// once the host leaves the joiners' sessions end.
    #[test]
    fn the_mp_game_loop_chats_saves_leaves_and_rejoins() {
        use crate::world::World;
        let Some(Ok(inst)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let rules: &'static GameRules = Box::leak(Box::new(crate::rules::load(&inst).unwrap().0));
        let texts = crate::new_game::StartTexts::read(&inst).unwrap();
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(2, &mut net);
        for g in &mut guests {
            click(g, spot::READY, false, &mut net);
        }
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready));
        let Some(Out::Start(b)) = click(&mut host, spot::READY, false, &mut net) else { panic!("no start") };
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 77)).unwrap().map(|s| World::new_mp(s, rules)).unwrap();
        let mut worlds = vec![mk(b)];
        worlds.extend(started(&mut guests, &mut net).into_iter().map(mk));
        let en = |w: &World| w.game.state.mem.i32(ig2_sim::save::va::EN);
        assert_eq!(worlds.iter().map(en).collect::<Vec<_>>(), [1, 2, 3], "each machine's own EN");
        let turn = |w: &World| w.net.as_ref().unwrap().0.peer().turn;
        let t = std::time::Instant::now();
        let step = |worlds: &mut Vec<World<'static>>, done: &dyn Fn(&[World]) -> bool| {
            while !done(worlds) {
                for w in worlds.iter_mut() {
                    w.pump();
                    w.tick();
                }
                assert!(t.elapsed() < Duration::from_secs(90), "stalled");
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        step(&mut worlds, &|w| w.iter().all(|w| turn(w) >= 10));
        assert_eq!(worlds.iter().map(en).collect::<Vec<_>>(), [1, 2, 3], "the shared EN went back in for every pump");

        // Chat: Guest0's line on every machine, its own included.
        worlds[1].say(b"hello there");
        step(&mut worlds, &|w| w.iter().all(|w| w.chat_lines.iter().any(|l| l == b"Guest0: hello there")));

        // The MP save: the host's reads back with the three humans; a joiner can't.
        let save = worlds[0].mp_save().unwrap().unwrap();
        let back = ig2_net::session::MpSave::read(&save, RULES).unwrap();
        assert_eq!(back.session.humans.iter().map(|h| (h.empire, h.name.as_str())).collect::<Vec<_>>(), [(1, "Host"), (2, "Guest0"), (3, "Guest1")]);
        assert!(worlds[1].mp_save().unwrap().is_err());
        assert!(worlds[2].mp_save().unwrap().is_err());

        // Guest1 leaves: away on the others at once, who play on.
        let (gone, _) = worlds.pop().unwrap().net.unwrap();
        gone.leave();
        step(&mut worlds, &|w| w.iter().all(|w| w.local_lines.iter().any(|l| l == b"Guest1 has left the game.")));
        let at = turn(&worlds[0]);
        step(&mut worlds, &|w| w.iter().all(|w| turn(w) >= at + 10));

        // It joins again by the room's Join under the same name: the host's door sends the state now.
        let mut again = MpScreen::open(false, "Guest1", RULES);
        type_text(&mut again, "local", &mut net);
        let t2 = std::time::Instant::now();
        let b = loop {
            for w in worlds.iter_mut() {
                w.pump();
                w.tick();
            }
            if let Some(Out::Start(b)) = again.tick(&mut net) {
                break b;
            }
            assert!(t2.elapsed() < Duration::from_secs(10), "no rejoin");
        };
        assert!(matches!(&b, Begun::Joiner { start, .. } if start.galaxy.is_none() && !start.snapshot.is_empty()));
        worlds.push(mk(b));
        assert_eq!(en(&worlds[2]), 3);
        step(&mut worlds, &|w| w[0].local_lines.iter().any(|l| l == b"Guest1 is back."));
        let at = turn(&worlds[0]);
        step(&mut worlds, &|w| w.iter().all(|w| turn(w) >= at + 20));
        let hashes = |w: &World| w.net.as_ref().unwrap().0.peer().hashes.clone();
        let h0 = hashes(&worlds[0]);
        for w in &worlds[1..] {
            let h = hashes(w);
            assert!(h.iter().filter(|x| h0.contains(x)).count() >= 10, "the peers agree");
            assert!(h.iter().all(|(t, x)| h0.iter().find(|(t0, _)| t0 == t).is_none_or(|(_, x0)| x0 == x)));
        }
        let Some((Session::Host { session, .. }, _)) = &worlds[0].net else { panic!() };
        assert!(session.host.desyncs.is_empty(), "{:?}", session.host.desyncs);

        // The host leaves: the joiners' sessions end.
        let (h, _) = worlds.remove(0).net.unwrap();
        h.leave();
        step(&mut worlds, &|w| w.iter().all(|w| w.net_over));
        assert!(worlds.iter().all(|w| w.local_lines.iter().any(|l| l == b"The host has left the game.")));
    }

    #[test]
    fn the_room_runs_through_its_screens() {
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(2, &mut net);
        // Guest0's race (slot 2, row 1): right click back from 2 to 1; a race of another slot is refused.
        assert!(click(&mut guests[0], spot::RACE + 1, true, &mut net).is_some());
        assert!(click(&mut guests[0], spot::RACE + 2, false, &mut net).is_none());
        // Guest1 moves to empire 6 (row 5).
        click(&mut guests[1], spot::HOLDER + 5, false, &mut net);
        // The host gives empire 8 to the AI (right click), and chats.
        click(&mut host, spot::HOLDER + 7, true, &mut net);
        type_text(&mut host, "hello", &mut net);
        type_text(&mut guests[0], "hi", &mut net);
        sync(&mut host, &mut guests, &mut net, |h| {
            let st = h.state().unwrap();
            st.slots[1].race == 1 && st.empire_of(2) == Some(6) && st.slots[7].kind == SlotKind::Ai && h.chat.len() == 2
        });
        let st = host.state().unwrap().clone();
        assert_eq!(st.slots[2].kind, SlotKind::Open, "Guest1's old slot is empty again");
        assert_eq!(st.galaxy.unwrap().ais, 1);
        assert_eq!(host.chat, [("Host".to_string(), "hello".to_string()), ("Guest0".to_string(), "hi".to_string())]);
        // Start: the host alone can't; every guest ready, then the host's press starts.
        assert!(matches!(click(&mut host, spot::READY, false, &mut net), Some(Out::Click)));
        assert_eq!(host.line, words::WAITING);
        for g in &mut guests {
            click(g, spot::READY, false, &mut net);
        }
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready));
        let Some(Out::Start(b)) = click(&mut host, spot::READY, false, &mut net) else { panic!("the host's press starts") };
        // The joiners' `Start` goes out when the host builds the game (`launch`: the galaxy test below).
        assert!(matches!(b, Begun::Host { .. }));
        // A host that goes away leaves its joiners on the connection page, "Disconnected".
        drop(b);
        let t = std::time::Instant::now();
        while !guests.iter().all(|g| g.lost && g.role.is_none()) {
            for g in &mut guests {
                g.tick(&mut net);
            }
            assert!(t.elapsed() < Duration::from_secs(2), "the joiners didn't notice");
        }
    }
    #[test]
    fn fields_take_keys() {
        let mut f = Field::new(5);
        for c in b"abcdef" {
            f.key(EntryKey::Char(*c));
        }
        assert_eq!(f.text, b"abcde");
        f.key(EntryKey::Left);
        f.key(EntryKey::Back);
        assert_eq!((f.text.as_slice(), f.cursor), (&b"abce"[..], 3));
        assert!(f.key(EntryKey::Enter));
        assert_eq!(f.take(), "abce");
    }
}
