//! The session layer (ADR 0011 §4–§6, ADR 0007 §3–§4): what drives a lockstep game from the lobby's start to its
//! end, without UI. Time is the caller's (`now`: any monotonic origin), so the policy is deterministic under test.
//!
//! - [`HostSession`] starts the game (`Game::start_multiplayer`, the start snapshot, `LobbyHost::start`), runs the
//!   host's own [`Peer`] and applies the policy: a human who drops while a real-time battle runs is waited for
//!   (`Host::hold`) for the grace period, then marked away (`Command::PlayerAway`) and the session runs on; outside a
//!   battle a drop is away at once. A rejoin gets a fresh [`Start`] from the host's state (snapshot + `send_log`) and
//!   `Command::PlayerBack`; a rejoiner comes back through the lobby's admission ([`HostSession::admit`]: hello,
//!   join code, password), matched to its empire by name. A human silent for the keep-alive is dropped like a closed
//!   link (both ends send `SessionMsg::Alive`, so a held session isn't silence). A desynced peer is restarted the same way on its own link (the host's state wins). A pause
//!   is the same hold. An MP save is the host's snapshot followed by the session's humans ([`SavedSession`]).
//! - [`ClientSession`] runs a joiner's peer, takes the host's restarts (rejoin, resync) and its hold status.
//! - Host migration (ADR 0012): when the host is gone, the survivors elect the lowest connected slot
//!   ([`ClientSession::successors`]); it becomes the host from its own state ([`HostSession::migrate`]), the others
//!   come back through its door and resume theirs ([`ClientSession::migrate`], `SessionMsg::Resume`), the bundles
//!   only some of them got adopted; the old host's player takes the drop path.
//!
//! ```text
//! drop ─▶ battle running? ─yes─▶ Waiting (hold) ─grace─▶ Away (inject PlayerAway, resume)
//!                         └─no──▶ Away (PlayerAway whenever a battle starts)
//! admit(link) ─▶ Hello / password ─▶ the human of that name, if away ─▶ Welcome ─▶ rejoin
//! rejoin(empire, link) ─▶ Start{snapshot at Host::next_turn} + send_log + PlayerBack (if away) ─▶ Here
//! desync ─▶ the peers whose hash differs from the host's: restart on their link
//! host gone ─▶ successor: HostSession::migrate (hold) ◀─ door ─ Resume{turn, bundles} ─ each survivor
//!            ─▶ adopt the bundles past its frontier ─▶ Resumed{from} + send_log  (or a Start) ─▶ old host: drop path
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use ig2_sim::game::{Game, GameRules, Libc};
use serde::{Deserialize, Serialize};

use crate::command::Command;
use crate::lobby::{Door, LobbyClient, LobbyHost, LobbyMsg, Refusal, SessionInfo, Settings, Start, Status};
use crate::lockstep::{Bundle, Host, Msg, Peer, DEFAULT_DELAY};
use crate::snapshot::{self, Header, Snapshot};
use crate::transport::{loopback, Loopback, Transport};
use crate::wire;

/// How long a human who dropped during a battle is waited for (ADR 0011 §4).
pub const DEFAULT_GRACE: Duration = Duration::from_secs(60);
/// How long a connected peer may be silent before it counts as dropped (either end; [`SessionSettings::keep_alive`]).
pub const DEFAULT_KEEP_ALIVE: Duration = Duration::from_secs(10);
/// The marker of the session block after an MP save's state.
pub const SESSION_MAGIC: &[u8; 8] = b"IG2RSESS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSettings {
    /// The input delay in turns.
    pub delay: u64,
    pub grace: Duration,
    /// Silence after which a peer is dropped; `SessionMsg::Alive` goes out every quarter of it. Zero: off (the
    /// transport's own timeout only).
    pub keep_alive: Duration,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self { delay: DEFAULT_DELAY, grace: DEFAULT_GRACE, keep_alive: DEFAULT_KEEP_ALIVE }
    }
}

/// Send `SessionMsg::Alive` at `now`? (every quarter of `keep_alive` since `last`).
fn alive_due(keep_alive: Duration, last: Option<Duration>, now: Duration) -> bool {
    !keep_alive.is_zero() && last.is_none_or(|t| now.saturating_sub(t) >= keep_alive / 4)
}

/// The session layer's messages ([`Msg::Session`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionMsg {
    /// Peer → host: the player pauses (`true`) or resumes.
    Pause(bool),
    /// Host → peers: what holds the session now (whenever it changes, and to a rejoiner).
    Status(HoldStatus),
    /// Either way: still here (the keep-alive; sent even while the session holds).
    Alive,
    /// Survivor → new host, once admitted (ADR 0012 §3): the next turn it runs and the bundles it holds
    /// ([`Peer::held_bundles`]).
    Resume { turn: u64, bundles: Vec<Bundle> },
    /// New host → survivor: go on from your own state; send inputs from turn `from` on (the log follows).
    Resumed { from: u64 },
}

/// Why the session stands still, and who is gone.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoldStatus {
    /// The empires whose players paused.
    pub paused: Vec<u8>,
    /// Dropped during a battle and awaited: `(empire, grace left in ms when sent)`.
    pub waiting: Vec<(u8, u64)>,
    /// Gone: their empires run on without orders.
    pub away: Vec<u8>,
    /// A host migration waits for these to reconnect (the old host's among them until it ends).
    pub migrating: Vec<u8>,
}

impl HoldStatus {
    /// The turn clock stands still.
    pub fn held(&self) -> bool {
        !self.paused.is_empty() || !self.waiting.is_empty() || !self.migrating.is_empty()
    }

    fn key(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        (self.paused.clone(), self.waiting.iter().map(|w| w.0).collect(), self.away.clone(), self.migrating.clone())
    }
}

/// What the host's session did, for its UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    /// `empire`'s player dropped; `waiting`: a battle runs, the session holds for the grace.
    Dropped { empire: u8, waiting: bool },
    /// The grace ran out: `empire` is away, the session runs on.
    Away { empire: u8 },
    /// `empire`'s player is back, from the state at `turn`.
    Rejoined { empire: u8, turn: u64 },
    /// After a host migration, `empire`'s player goes on from its own state; the log from `turn` on was sent.
    Resumed { empire: u8, turn: u64 },
    /// `empire`'s peer diverged at `turn` and was restarted from the host's state at `at`.
    Resynced { empire: u8, turn: u64, at: u64 },
    Paused { empire: u8, on: bool },
    /// A connection was turned away at the admission.
    Refused(Refusal),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Wire(#[from] wire::Error),
    #[error(transparent)]
    Snapshot(#[from] snapshot::Error),
    #[error("empire {0} isn't a human of this session")]
    NotInSession(u8),
    #[error("empire {0}'s player is still connected")]
    Connected(u8),
    #[error("not a multiplayer save")]
    NotMultiplayer,
    #[error("the save was made with other rules")]
    Rules,
    #[error("corrupt session block: {0}")]
    Corrupt(&'static str),
}

/// The rules fingerprint as a save header stores it (its first 8 bytes).
pub fn rules_id(fingerprint: [u8; 32]) -> u64 {
    u64::from_le_bytes(fingerprint[..8].try_into().unwrap())
}

// ------------------------------------------------------------------------------------------------ MP saves

/// The session part of an MP save (ADR 0007 §4): who played, and the settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSession {
    pub name: String,
    pub scenario: String,
    pub delay: u64,
    pub grace_ms: u64,
    pub humans: Vec<SavedHuman>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedHuman {
    pub empire: u8,
    pub name: String,
}

/// An MP save read back.
#[derive(Debug, Clone)]
pub struct MpSave {
    pub snapshot: Snapshot,
    pub header: Header,
    pub session: SavedSession,
}

impl MpSave {
    /// The file: `Snapshot::to_file(.., true)`, then [`SESSION_MAGIC`], the block's `u32` length and the session
    /// ([`wire`] encoding).
    pub fn write(snapshot: &Snapshot, fingerprint: [u8; 32], session: &SavedSession) -> Vec<u8> {
        let mut out = snapshot.to_file(rules_id(fingerprint), true);
        let block = wire::to_vec(session).expect("the session encodes");
        out.extend_from_slice(SESSION_MAGIC);
        out.extend_from_slice(&(block.len() as u32).to_le_bytes());
        out.extend_from_slice(&block);
        out
    }

    /// An MP save made with the rules of `fingerprint`.
    pub fn read(b: &[u8], fingerprint: [u8; 32]) -> Result<Self, Error> {
        let (snapshot, header) = Snapshot::from_file(b)?;
        if header.flags & 1 == 0 {
            return Err(Error::NotMultiplayer);
        }
        if header.rules != rules_id(fingerprint) {
            return Err(Error::Rules);
        }
        let rest = &b[Snapshot::file_len(b)?..];
        if rest.len() < 12 || &rest[..8] != SESSION_MAGIC {
            return Err(Error::Corrupt("no session block"));
        }
        let len = u32::from_le_bytes(rest[8..12].try_into().unwrap()) as usize;
        let block = rest.get(12..12 + len).ok_or(Error::Corrupt("truncated"))?;
        Ok(Self { snapshot, header, session: wire::from_slice(block)? })
    }
}

// ------------------------------------------------------------------------------------------------ host

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Presence {
    Here,
    /// Dropped during a battle at `since`; the session holds.
    Waiting { since: Duration },
    Away,
    /// A host migration began at `since` and this survivor hasn't reconnected yet; the session holds.
    Migrating { since: Duration },
    /// The old host's player during a migration: dropped once it ends.
    Lost,
}

struct Human {
    slot: u8,
    empire: u8,
    name: String,
    presence: Presence,
    /// `Host::heard` for its slot, and when that last changed (`None`: (re)started, noted at the next pump).
    heard: Option<(u64, Duration)>,
}

/// The host's side of a running game: the lockstep relay, the host's own player and the session policy.
pub struct HostSession<'r> {
    pub host: Host,
    /// The host's own player (its UI issues and shows through it).
    pub own: Peer<'r>,
    link: Loopback,
    fingerprint: [u8; 32],
    settings: SessionSettings,
    /// The lobby's admission, for rejoiners.
    door: Door,
    /// The session as every member knows it (`Start::session`).
    info: SessionInfo,
    humans: Vec<Human>,
    /// Survivors admitted during a migration, until their `Resume` (index into `humans`).
    resuming: Vec<(usize, Box<dyn Transport>)>,
    /// When the host migration began, while it runs.
    migration: Option<Duration>,
    paused: BTreeSet<u8>,
    /// A real-time battle ran at the last look (PlayerAway goes out for the away empires as one starts).
    in_battle: bool,
    /// Desync reports handled so far.
    desyncs_seen: usize,
    /// The hold status last sent.
    sent: HoldStatus,
    /// When the last `SessionMsg::Alive` went out.
    alive_sent: Option<Duration>,
    queued: Vec<SessionEvent>,
}

impl<'r> HostSession<'r> {
    /// Start a new game: `game` (the scenario's or a save's state) becomes the session's start state with the lobby's
    /// humans (`Game::start_multiplayer`, the host's empire as `EN`) and the session id as its libc seed (ADR 0007
    /// §5). Refused (the lobby back) unless `LobbyHost::can_start`.
    pub fn start(lobby: LobbyHost, mut game: Game<'r>, rules: &'r GameRules, settings: SessionSettings) -> Result<Self, Box<LobbyHost>> {
        if !lobby.can_start() {
            return Err(Box::new(lobby));
        }
        let st = lobby.state();
        let humans: Vec<u8> = st.members.iter().filter_map(|m| st.empire_of(m.id)).collect();
        game.start_multiplayer(humans[0], &humans);
        game.libc = Libc(lobby.session() as u32);
        let file = Snapshot::capture(&game, &[], 0, 0).to_file(rules_id(lobby.rules()), true);
        Self::launch(lobby, file, rules, settings)
    }

    /// Resume an MP save: the lobby's members must hold exactly the saved humans' empires. The state goes on from the
    /// saved turn with the saved settings. Refused (the lobby back) otherwise.
    pub fn resume(lobby: LobbyHost, save: &MpSave, rules: &'r GameRules) -> Result<Self, Box<LobbyHost>> {
        let st = lobby.state();
        let mut theirs: Vec<u8> = st.members.iter().filter_map(|m| st.empire_of(m.id)).collect();
        let mut saved: Vec<u8> = save.session.humans.iter().map(|h| h.empire).collect();
        theirs.sort_unstable();
        saved.sort_unstable();
        if !lobby.can_start() || theirs != saved {
            return Err(Box::new(lobby));
        }
        let file = save.snapshot.to_file(rules_id(lobby.rules()), true);
        let settings = SessionSettings { delay: save.session.delay, grace: Duration::from_millis(save.session.grace_ms), ..SessionSettings::default() };
        Self::launch(lobby, file, rules, settings)
    }

    fn launch(lobby: LobbyHost, file: Vec<u8>, rules: &'r GameRules, settings: SessionSettings) -> Result<Self, Box<LobbyHost>> {
        let fingerprint = lobby.rules();
        let started = lobby.start_with(file, &settings)?;
        let own = started.own.peer(rules).expect("our own start state loads");
        let info = started.own.session.clone();
        let names: BTreeMap<u8, String> = info.names.iter().cloned().collect();
        let humans = started.own.humans.iter().map(|&(slot, empire)| Human { slot, empire, name: names[&slot].clone(), presence: Presence::Here, heard: None }).collect();
        Ok(Self {
            host: started.host,
            own,
            link: started.link,
            fingerprint,
            settings,
            door: started.door,
            info,
            humans,
            resuming: Vec::new(),
            migration: None,
            paused: BTreeSet::new(),
            in_battle: false,
            desyncs_seen: 0,
            sent: HoldStatus::default(),
            alive_sent: None,
            queued: Vec::new(),
        })
    }

    /// Take over a session whose host is gone (ADR 0012): `client` is the elected successor (first of its
    /// [`ClientSession::successors`]). Its peer goes on from its own state; the relay continues at its frontier with
    /// its bundles as the log; the other survivors come back through the door (`fingerprint`: these rules; `password`:
    /// the session's, as the glue kept it) and resume. The session holds until each has, or the grace since `now` has
    /// passed; the old host's player then takes the drop path.
    pub fn migrate(client: ClientSession<'r>, fingerprint: [u8; 32], password: Option<String>, now: Duration) -> Self {
        let ClientSession { mut peer, status, mut info, humans, delay, .. } = client;
        let next = peer.frontier();
        let mut host = Host::resumed(delay, next, peer.held_bundles());
        let (own_end, link) = loopback();
        let from = host.add_peer(peer.slot, peer.empire, Box::new(own_end));
        peer.resume(from);
        let settings = SessionSettings { delay, grace: Duration::from_millis(info.grace_ms), keep_alive: Duration::from_millis(info.keep_alive_ms) };
        let door = Door::new(Settings { name: info.name.clone(), scenario: info.scenario.clone(), private: info.private, password }, info.id, fingerprint);
        let names: BTreeMap<u8, String> = info.names.iter().cloned().collect();
        let old = info.host;
        let gone = humans.iter().find(|h| h.0 == old).map(|h| h.1);
        let humans: Vec<Human> = humans
            .iter()
            .map(|&(slot, empire)| {
                let presence = if slot == peer.slot {
                    Presence::Here
                } else if slot == old || status.migrating.contains(&empire) {
                    Presence::Lost
                } else if status.away.contains(&empire) {
                    Presence::Away
                } else if status.waiting.iter().any(|w| w.0 == empire) {
                    Presence::Waiting { since: now }
                } else {
                    Presence::Migrating { since: now }
                };
                Human { slot, empire, name: names.get(&slot).cloned().unwrap_or_default(), presence, heard: None }
            })
            .collect();
        info.host = peer.slot;
        let in_battle = peer.game.realtime_running();
        let mut s = Self {
            host,
            own: peer,
            link,
            fingerprint,
            settings,
            door,
            info,
            humans,
            resuming: Vec::new(),
            migration: Some(now),
            paused: status.paused.iter().copied().filter(|&e| Some(e) != gone).collect(),
            in_battle,
            desyncs_seen: 0,
            sent: HoldStatus::default(),
            alive_sent: None,
            queued: Vec::new(),
        };
        s.update_hold(now);
        s
    }

    pub fn settings(&self) -> SessionSettings {
        self.settings
    }

    /// The session as every member knows it.
    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    /// A host migration waits for survivors to come back.
    pub fn migrating(&self) -> bool {
        self.migration.is_some()
    }

    /// The session's humans: `(empire, name, connected)`.
    pub fn humans(&self) -> impl Iterator<Item = (u8, &str, bool)> {
        self.humans.iter().map(|h| (h.empire, h.name.as_str(), h.presence == Presence::Here))
    }

    /// What holds the session at `now`.
    pub fn status(&self, now: Duration) -> HoldStatus {
        HoldStatus {
            paused: self.paused.iter().copied().collect(),
            waiting: self
                .humans
                .iter()
                .filter_map(|h| match h.presence {
                    Presence::Waiting { since } => Some((h.empire, self.settings.grace.saturating_sub(now.saturating_sub(since)).as_millis() as u64)),
                    _ => None,
                })
                .collect(),
            away: self.humans.iter().filter(|h| h.presence == Presence::Away).map(|h| h.empire).collect(),
            migrating: self.humans.iter().filter(|h| matches!(h.presence, Presence::Migrating { .. } | Presence::Lost)).map(|h| h.empire).collect(),
        }
    }

    /// A connection at `now` (`QuicHost::accept`, or a loopback end): a player coming back, admitted like a joiner
    /// of the lobby (version, rules, join code, password) and matched to the session's human of its name.
    pub fn admit(&mut self, link: Box<dyn Transport>, now: Duration) {
        self.door.admit(link, now);
    }

    /// Connections still being admitted.
    pub fn admitting(&self) -> usize {
        self.door.pending()
    }

    /// One round: relay the turns, notice drops (closed or silent links) and desyncs, admit rejoiners, run up to `max_turns` of the host's own
    /// turns, apply the grace and hold. Returns what happened since the last round.
    pub fn pump(&mut self, now: Duration, max_turns: usize) -> Result<Vec<SessionEvent>, Error> {
        self.host.pump()?;
        for (slot, m) in self.host.take_inbox() {
            if let (SessionMsg::Pause(on), Some(empire)) = (m, self.host.empire_of(slot)) {
                self.set_pause(empire, on);
            }
        }
        for slot in self.host.closed() {
            self.dropped(slot, now)?;
        }
        for slot in self.silent(now) {
            self.dropped(slot, now)?;
        }
        if alive_due(self.settings.keep_alive, self.alive_sent, now) {
            self.host.broadcast(&Msg::Session(SessionMsg::Alive));
            self.alive_sent = Some(now);
        }
        self.admit_rejoiners(now)?;
        self.resume_survivors()?;
        self.end_migration(now)?;
        self.own.poll(&mut self.link, max_turns)?;
        self.resync_diverged()?;
        for h in &mut self.humans {
            if let Presence::Waiting { since } = h.presence {
                if now.saturating_sub(since) >= self.settings.grace {
                    self.host.inject(h.empire, Command::PlayerAway);
                    h.presence = Presence::Away;
                    self.queued.push(SessionEvent::Away { empire: h.empire });
                }
            }
        }
        let running = self.own.game.realtime_running();
        if running && !self.in_battle {
            for h in self.humans.iter().filter(|h| h.presence == Presence::Away) {
                self.host.inject(h.empire, Command::PlayerAway);
            }
        }
        self.in_battle = running;
        self.update_hold(now);
        Ok(std::mem::take(&mut self.queued))
    }

    /// The host's own player pauses or resumes.
    pub fn pause(&mut self, on: bool) {
        self.set_pause(self.own.empire, on);
    }

    /// `empire`'s player is gone (the glue saw its keep-alive time out, or the host kicked it); a closed link is
    /// noticed by [`HostSession::pump`] itself.
    pub fn drop_player(&mut self, empire: u8, now: Duration) -> Result<(), Error> {
        let h = self.humans.iter().find(|h| h.empire == empire).ok_or(Error::NotInSession(empire))?;
        let slot = h.slot;
        self.dropped(slot, now)
    }

    /// `empire`'s player is back on `link` (admitted by the glue): it gets the host's state now, the bundles since,
    /// and, if it was away, `PlayerBack`.
    pub fn rejoin(&mut self, empire: u8, link: Box<dyn Transport>, now: Duration) -> Result<(), Error> {
        let i = self.humans.iter().position(|h| h.empire == empire).ok_or(Error::NotInSession(empire))?;
        let was = self.humans[i].presence;
        if was == Presence::Here {
            return Err(Error::Connected(empire));
        }
        let turn = self.restart(self.humans[i].slot, link)?;
        if was == Presence::Away {
            self.host.inject(empire, Command::PlayerBack);
        }
        self.humans[i].presence = Presence::Here;
        self.queued.push(SessionEvent::Rejoined { empire, turn });
        self.update_hold(now);
        Ok(())
    }

    /// An MP save of the state now (the host's own peer at the next turn) with the session's humans.
    pub fn save(&mut self) -> Result<Vec<u8>, Error> {
        self.catch_up()?;
        let session = SavedSession {
            name: self.info.name.clone(),
            scenario: self.info.scenario.clone(),
            delay: self.settings.delay,
            grace_ms: self.settings.grace.as_millis() as u64,
            humans: self.humans.iter().map(|h| SavedHuman { empire: h.empire, name: h.name.clone() }).collect(),
        };
        Ok(MpSave::write(&self.own.snapshot(), self.fingerprint, &session))
    }

    /// The connected humans' slots heard from last `keep_alive` or longer ago (the host's own peer aside).
    fn silent(&mut self, now: Duration) -> Vec<u8> {
        let keep_alive = self.settings.keep_alive;
        let mut silent = Vec::new();
        for h in self.humans.iter_mut().filter(|h| h.presence == Presence::Here && h.slot != self.own.slot) {
            let Some(count) = self.host.heard(h.slot) else { continue };
            match h.heard {
                Some((c, since)) if c == count => {
                    if !keep_alive.is_zero() && now.saturating_sub(since) >= keep_alive {
                        silent.push(h.slot);
                    }
                }
                _ => h.heard = Some((count, now)),
            }
        }
        silent
    }

    /// Run the admissions; an admitted player gets `Welcome` with its slot and the rejoin.
    fn admit_rejoiners(&mut self, now: Duration) -> Result<(), Error> {
        let humans = &self.humans;
        let mut refused = Vec::new();
        let admitted = self.door.pump(now, |name| returning(humans, name).err(), &mut refused)?;
        self.queued.extend(refused.into_iter().map(SessionEvent::Refused));
        for (mut link, name) in admitted {
            // The door checked it last.
            let i = returning(&self.humans, &name).expect("an away human of that name");
            link.send(&Msg::Lobby(LobbyMsg::Welcome { member: self.humans[i].slot }));
            if matches!(self.humans[i].presence, Presence::Migrating { .. }) {
                // A survivor of the migration: its `Resume` follows.
                self.resuming.retain(|r| r.0 != i);
                self.resuming.push((i, link));
            } else {
                self.rejoin(self.humans[i].empire, link, now)?;
            }
        }
        Ok(())
    }

    /// Take the admitted survivors' `Resume`s (ADR 0012 §3).
    fn resume_survivors(&mut self) -> Result<(), Error> {
        for (i, mut link) in std::mem::take(&mut self.resuming) {
            let mut resume = None;
            while let Some(m) = link.recv()? {
                if let Msg::Session(SessionMsg::Resume { turn, bundles }) = m {
                    resume = Some((turn, bundles));
                    break;
                }
            }
            match resume {
                Some((turn, bundles)) => self.resume_survivor(i, link, turn, bundles)?,
                None if link.closed() => {}
                None => self.resuming.push((i, link)),
            }
        }
        Ok(())
    }

    /// A survivor that held the bundles up to `turn`'s and beyond: adopt those past the relay's frontier, then let it
    /// go on from its own state if the log reaches its frontier, else restart it from ours.
    fn resume_survivor(&mut self, i: usize, link: Box<dyn Transport>, turn: u64, mut bundles: Vec<Bundle>) -> Result<(), Error> {
        bundles.sort_by_key(|b| b.turn);
        bundles.dedup_by_key(|b| b.turn);
        let mut frontier = turn;
        for b in bundles.iter().filter(|b| b.turn >= turn) {
            if b.turn != frontier {
                break;
            }
            frontier += 1;
        }
        let (slot, empire) = (self.humans[i].slot, self.humans[i].empire);
        let migrating = matches!(self.humans[i].presence, Presence::Migrating { .. });
        if migrating {
            let next = self.host.next_turn();
            for b in bundles.into_iter().filter(|b| b.turn >= next) {
                if !self.host.adopt(b) {
                    break;
                }
            }
        }
        self.humans[i].heard = None;
        self.humans[i].presence = Presence::Here;
        if migrating && frontier <= self.host.next_turn() && self.host.log_reaches(frontier) {
            let from = self.host.add_peer(slot, empire, link);
            self.host.send_to(slot, &Msg::Session(SessionMsg::Resumed { from }));
            self.host.send_log(slot, frontier);
            self.host.send_to(slot, &Msg::Session(SessionMsg::Status(self.sent.clone())));
            self.queued.push(SessionEvent::Resumed { empire, turn: frontier });
        } else {
            let turn = self.restart(slot, link)?;
            self.queued.push(SessionEvent::Rejoined { empire, turn });
        }
        Ok(())
    }

    /// End the migration once no survivor is awaited (each resumed, or the grace since it began passed: those are
    /// dropped); the old host's player then takes the drop path.
    fn end_migration(&mut self, now: Duration) -> Result<(), Error> {
        let Some(began) = self.migration else { return Ok(()) };
        let late = now.saturating_sub(began) >= self.settings.grace;
        if !late && self.humans.iter().any(|h| matches!(h.presence, Presence::Migrating { .. })) {
            return Ok(());
        }
        self.migration = None;
        self.resuming.clear();
        for i in 0..self.humans.len() {
            if matches!(self.humans[i].presence, Presence::Migrating { .. } | Presence::Lost) {
                self.mark_dropped(i, now)?;
            }
        }
        Ok(())
    }

    fn set_pause(&mut self, empire: u8, on: bool) {
        let changed = if on { self.paused.insert(empire) } else { self.paused.remove(&empire) };
        if changed {
            self.queued.push(SessionEvent::Paused { empire, on });
        }
    }

    fn dropped(&mut self, slot: u8, now: Duration) -> Result<(), Error> {
        self.host.remove_peer(slot);
        let Some(i) = self.humans.iter().position(|h| h.slot == slot && h.presence == Presence::Here) else { return Ok(()) };
        self.mark_dropped(i, now)
    }

    /// The drop path for `humans[i]`: waiting during a battle, else away.
    fn mark_dropped(&mut self, i: usize, now: Duration) -> Result<(), Error> {
        let empire = self.humans[i].empire;
        if self.paused.remove(&empire) {
            self.queued.push(SessionEvent::Paused { empire, on: false });
        }
        // Decide on the state the hold would stop at: every bundle sent so far run.
        self.catch_up()?;
        let waiting = self.own.game.realtime_running();
        self.humans[i].presence = if waiting { Presence::Waiting { since: now } } else { Presence::Away };
        self.queued.push(SessionEvent::Dropped { empire, waiting });
        Ok(())
    }

    /// Run the host's own peer through every bundle sent (so its state is the one at `Host::next_turn`).
    fn catch_up(&mut self) -> Result<(), Error> {
        while self.own.turn < self.host.next_turn() {
            if self.own.poll(&mut self.link, usize::MAX)? == 0 {
                break;
            }
        }
        debug_assert_eq!(self.own.turn, self.host.next_turn());
        Ok(())
    }

    /// (Re)start `slot` on `link` from the host's state now; returns the snapshot's turn.
    fn restart(&mut self, slot: u8, mut link: Box<dyn Transport>) -> Result<u64, Error> {
        self.catch_up()?;
        let h = self.humans.iter_mut().find(|h| h.slot == slot).expect("a session slot");
        h.heard = None;
        let empire = h.empire;
        let snap = self.own.snapshot();
        let delay = self.settings.delay;
        let first_input = self.host.next_turn().max(delay);
        let humans = self.humans.iter().map(|h| (h.slot, h.empire)).collect();
        let session = self.info.clone();
        let start = Start { slot, empire, delay, first_input, humans, session, snapshot: snap.to_file(rules_id(self.fingerprint), true) };
        link.send(&Msg::Lobby(LobbyMsg::Start(Box::new(start))));
        let from = self.host.add_peer(slot, empire, link);
        debug_assert_eq!(from, first_input);
        self.host.send_log(slot, snap.turn);
        self.host.send_to(slot, &Msg::Session(SessionMsg::Status(self.sent.clone())));
        Ok(snap.turn)
    }

    /// ADR 0007 §3: on a new desync report, restart every peer whose hash for a reported turn differs from the host's
    /// own (the host's state wins; reports from before a peer's (re)start don't count).
    fn resync_diverged(&mut self) -> Result<(), Error> {
        let total: usize = self.host.desyncs.iter().map(|d| d.slots.len()).sum();
        if total == self.desyncs_seen {
            return Ok(());
        }
        self.desyncs_seen = total;
        self.catch_up()?;
        let mut diverged = BTreeMap::new();
        for d in &self.host.desyncs {
            let Some(&(_, mine)) = self.own.hashes.iter().rev().find(|(t, _)| *t == d.turn) else { continue };
            for &(slot, hash) in self.host.reported(d.turn) {
                if slot != self.own.slot && hash != mine && self.host.joined_at(slot).is_some_and(|j| j <= d.turn) {
                    diverged.entry(slot).or_insert(d.turn);
                }
            }
        }
        for (slot, turn) in diverged {
            let link = self.host.remove_peer(slot).expect("a reporting slot");
            let at = self.restart(slot, link)?;
            let empire = self.host.empire_of(slot).unwrap();
            self.queued.push(SessionEvent::Resynced { empire, turn, at });
        }
        Ok(())
    }

    fn update_hold(&mut self, now: Duration) {
        let status = self.status(now);
        self.host.hold(status.held());
        if status.key() != self.sent.key() {
            self.host.broadcast(&Msg::Session(SessionMsg::Status(status.clone())));
            self.sent = status;
        }
    }
}

/// The session's human a rejoiner named `name` comes back as: the first of that name who isn't connected.
fn returning(humans: &[Human], name: &str) -> Result<usize, Refusal> {
    let mut named = humans.iter().enumerate().filter(|(_, h)| h.name == name).peekable();
    if named.peek().is_none() {
        return Err(Refusal::Started);
    }
    named.find(|(_, h)| h.presence != Presence::Here).map(|(i, _)| i).ok_or(Refusal::Connected)
}

// ------------------------------------------------------------------------------------------------ joiner

/// A joiner's side of a running game: its peer, restarted whenever the host sends a new [`Start`].
pub struct ClientSession<'r> {
    pub peer: Peer<'r>,
    link: Box<dyn Transport>,
    rules: &'r GameRules,
    /// From the last `Start`: the session, its humans' `(slot, empire)` and the input delay.
    info: SessionInfo,
    humans: Vec<(u8, u8)>,
    delay: u64,
    /// A host migration in progress on this side (ADR 0012).
    migration: Option<Migration>,
    /// The last migration's door turned this survivor away (the glue tries the next successor).
    pub refused: Option<Refusal>,
    /// The host's last hold status.
    pub status: HoldStatus,
    /// Restarts from the host's state (resyncs) since this session began.
    pub restarts: usize,
    /// The host's silence after which it counts as gone ([`SessionSettings::keep_alive`]; zero: off).
    pub keep_alive: Duration,
    /// When the host was last heard from, and when `SessionMsg::Alive` last went out.
    heard: Option<Duration>,
    alive_sent: Option<Duration>,
    /// The host was silent for `keep_alive` at the last pump.
    silent: bool,
}

impl<'r> ClientSession<'r> {
    /// From the lobby's start (`LobbyClient::into_game`), or a rejoin's ([`ClientSession::wait_start`]).
    pub fn new(start: &Start, link: Box<dyn Transport>, rules: &'r GameRules) -> Result<Self, Error> {
        Ok(Self {
            peer: start.peer(rules)?,
            link,
            rules,
            info: start.session.clone(),
            humans: start.humans.clone(),
            delay: start.delay,
            migration: None,
            refused: None,
            status: HoldStatus::default(),
            restarts: 0,
            keep_alive: Duration::from_millis(start.session.keep_alive_ms),
            heard: None,
            alive_sent: None,
            silent: false,
        })
    }

    /// A rejoiner's first message: the host's [`Start`] (anything before it is dropped).
    pub fn wait_start(link: &mut dyn Transport) -> Result<Option<Box<Start>>, Error> {
        while let Some(m) = link.recv()? {
            if let Msg::Lobby(LobbyMsg::Start(s)) = m {
                return Ok(Some(s));
            }
        }
        Ok(None)
    }

    /// At `now` (the caller's clock): receive, send the due inputs and keep-alive, run up to `max_turns` turns.
    /// Returns the turns run.
    pub fn pump(&mut self, now: Duration, max_turns: usize) -> Result<usize, Error> {
        if self.migration.is_some() && !self.migrate_pump()? {
            return Ok(0);
        }
        if alive_due(self.keep_alive, self.alive_sent, now) {
            self.link.send(&Msg::Session(SessionMsg::Alive));
            self.alive_sent = Some(now);
        }
        let mut run = 0;
        loop {
            let mut tap = Tap { link: &mut *self.link, start: None, status: None, heard: false };
            run += self.peer.poll(&mut tap, max_turns.saturating_sub(run))?;
            let (start, status) = (tap.start, tap.status);
            if tap.heard || self.heard.is_none() {
                self.heard = Some(now);
            }
            self.silent = !self.keep_alive.is_zero() && self.heard.is_some_and(|t| now.saturating_sub(t) >= self.keep_alive);
            if let Some(s) = status {
                self.status = s;
            }
            let Some(start) = start else { return Ok(run) };
            self.restart(&start)?;
        }
    }

    fn restart(&mut self, start: &Start) -> Result<(), Error> {
        self.peer = start.peer(self.rules)?;
        self.info = start.session.clone();
        self.humans = start.humans.clone();
        self.delay = start.delay;
        self.restarts += 1;
        Ok(())
    }

    /// The session as every member knows it.
    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    /// Who takes over if the host is gone (ADR 0012 §2), in the order to try: the session's humans by slot, without
    /// the host and the empires the last status lists away, awaited or reconnecting. The first is the elected one;
    /// if that's this peer's slot, the glue calls [`HostSession::migrate`], else [`ClientSession::migrate`] with a link
    /// to it.
    pub fn successors(&self) -> Vec<u8> {
        let s = &self.status;
        let gone = |e: u8| s.away.contains(&e) || s.migrating.contains(&e) || s.waiting.iter().any(|w| w.0 == e);
        let mut slots: Vec<u8> = self.humans.iter().filter(|&&(slot, e)| slot != self.info.host && !gone(e)).map(|h| h.0).collect();
        slots.sort_unstable();
        slots
    }

    /// Reconnect to the successor `host` (a slot of [`ClientSession::successors`]) on `link` (ADR 0012 §3): the door's
    /// hello as `name` with the session's join code, `password` and `fingerprint`, then `Resume` with this peer's
    /// bundles; the new host answers `Resumed` (go on) or a `Start` (restart from its state). [`ClientSession::pump`]
    /// runs it; [`ClientSession::refused`] reports a door that turned it away.
    pub fn migrate(&mut self, host: u8, link: Box<dyn Transport>, name: &str, password: Option<String>, fingerprint: [u8; 32]) {
        self.migration = Some(Migration::Door(host, Box::new(LobbyClient::rejoin(link, name, fingerprint, password, Some(self.info.id)))));
        self.refused = None;
        self.heard = None;
        self.silent = false;
    }

    /// A migration in progress (the door, or waiting for the new host's answer).
    pub fn migrating(&self) -> bool {
        self.migration.is_some()
    }

    /// Run the migration as far as its messages go; true once this peer goes on (resumed or restarted).
    fn migrate_pump(&mut self) -> Result<bool, Error> {
        if let Some(Migration::Door(host, door)) = &mut self.migration {
            let host = *host;
            match door.pump()?.clone() {
                Status::Admitted => {
                    let Some(Migration::Door(_, door)) = self.migration.take() else { unreachable!() };
                    self.link = door.into_link().expect("admitted");
                    self.link.send(&Msg::Session(SessionMsg::Resume { turn: self.peer.turn, bundles: self.peer.held_bundles() }));
                    self.migration = Some(Migration::Answer(host));
                }
                Status::Refused(why) => {
                    self.migration = None;
                    self.refused = Some(why);
                    return Ok(false);
                }
                _ => return Ok(false),
            }
        }
        let Some(Migration::Answer(host)) = self.migration else { return Ok(false) };
        while let Some(m) = self.link.recv()? {
            match m {
                Msg::Session(SessionMsg::Resumed { from }) => {
                    self.peer.resume(from);
                    self.info.host = host;
                    self.migration = None;
                    return Ok(true);
                }
                Msg::Lobby(LobbyMsg::Start(s)) => {
                    self.restart(&s)?;
                    self.migration = None;
                    return Ok(true);
                }
                Msg::Session(SessionMsg::Status(s)) => self.status = s,
                _ => {}
            }
        }
        Ok(false)
    }

    /// The local player pauses or resumes (the host holds every peer).
    pub fn pause(&mut self, on: bool) {
        self.link.send(&Msg::Session(SessionMsg::Pause(on)));
    }

    /// The host is gone: the link closed, or the host was silent for `keep_alive` at the last pump (never during a
    /// migration: [`ClientSession::refused`] and the glue's connect timeout tell that one).
    pub fn closed(&self) -> bool {
        self.migration.is_none() && (self.link.closed() || self.silent)
    }
}

/// A survivor's side of a host migration to the slot it names.
enum Migration {
    Door(u8, Box<LobbyClient>),
    /// Admitted, `Resume` sent: waiting for `Resumed` or a `Start`.
    Answer(u8),
}

/// The joiner's link as its peer sees it: the session messages taken out, and nothing after a restart (the next
/// messages are the new peer's).
struct Tap<'a> {
    link: &'a mut dyn Transport,
    start: Option<Box<Start>>,
    status: Option<HoldStatus>,
    /// Anything came from the host.
    heard: bool,
}

impl Transport for Tap<'_> {
    fn send(&mut self, msg: &Msg) {
        self.link.send(msg);
    }

    fn recv(&mut self) -> Result<Option<Msg>, wire::Error> {
        if self.start.is_some() {
            return Ok(None);
        }
        loop {
            let m = self.link.recv()?;
            self.heard |= m.is_some();
            match m {
                Some(Msg::Lobby(LobbyMsg::Start(s))) => {
                    self.start = Some(s);
                    return Ok(None);
                }
                Some(Msg::Session(SessionMsg::Status(s))) => self.status = Some(s),
                other => return Ok(other),
            }
        }
    }

    fn closed(&self) -> bool {
        self.link.closed()
    }
}
