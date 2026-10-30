//! Our multiplayer screens (re/screens/start.md §4–§6, ADR 0014): the connection page (the LAN's sessions, host,
//! join from the list or by address, a password) and the room of a generated galaxy (eight slots with races and
//! holders, the host's options, chat, ready and start), drawn in the scenario panel's style over the main menu's
//! scene. They sit on `ig2_net::lobby`; the start hands a [`Begun`] to [`launch`], which builds the game on this
//! machine (`HostSession::start_galaxy` / `ClientSession::new_galaxy`).
//!
//! Links come from a [`Net`]: QUIC and LAN discovery ([`QuicNet`]) in the client, in-process loopback ends in tests.
//! The connection page also hosts a room for an MP save ([`MpScreen::host_save`]); a joiner's [`Session`] takes part in
//! a host migration (rules/multiplayer.md §7.3).

use std::net::SocketAddrV4;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use ig2_net::discovery::{Announcement, Found};
use ig2_net::galaxy::{GalaxyOptions, MAX_AIS, PLANETS};
use ig2_net::joincode::JoinCode;
use ig2_net::quic::Rendezvous;
use ig2_net::lobby::{LobbyClient, LobbyHost, LobbyState, Refusal, Settings, SlotKind, Start, Status};
use ig2_net::session::{ClientSession, HoldStatus, HostSession, MpSave, SessionEvent, SessionSettings};
use ig2_net::{Peer, Transport};
use ig2_sim::game::new_game::StartData;
use ig2_sim::game::GameRules;

use crate::font::Align;
use crate::kit::fader;
use crate::options::{Draw, EntryKey};
use crate::render::{kind, Blend, Frame, Instance};

/// The game port a host listens on first (the LAN announcement carries the real one).
pub const GAME_PORT: u16 = ig2_net::discovery::DISCOVERY_PORT + 1;
/// The relay address's file in the save folder.
const RELAY_FILE: &str = "relay.txt";
/// The longest password typed.
const MAX_FIELD: usize = 40;
/// The longest address typed or pasted (a join code with four addresses and its dashes is 84).
const MAX_ADDRESS: usize = 96;
/// The address characters the field shows (it scrolls with the cursor).
const ADDRESS_SHOWN: usize = 60;
/// The relay address characters shown after the field's label.
const RELAY_SHOWN: usize = 36;
/// Chat lines shown.
const CHAT_LINES: usize = 4;
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
    /// Ask the router to forward the game port (`on`), or remove the forwarding (ADR 0008 §2: the host's opt-in).
    fn map_port(&mut self, _on: bool) {}
    fn port_map(&self) -> PortMap {
        PortMap::Off
    }
    /// Where joiners can reach this listener (its LAN address, the router's mapped one): a join code's addresses.
    fn addrs(&self) -> Vec<std::net::SocketAddrV4> {
        Vec::new()
    }
    /// Open the room of session `session` at the rendezvous service, if one is set (ADR 0018).
    fn register(&mut self, _session: u64) {}
    fn rendezvous(&self) -> Rendezvous {
        Rendezvous::Off
    }
}

/// The host's port mapping, shown in its room.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PortMap {
    #[default]
    Off,
    /// Waiting on the router (`portmap::open_live` searches for a few seconds).
    Asking,
    /// The router's external address and port.
    Mapped(std::net::SocketAddrV4),
    Failed(String),
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
    /// Connect to the host of a join code: every address it carries, the first to answer with the pinned certificate.
    fn connect_code(&mut self, code: &JoinCode) -> Connecting {
        self.connect(&code.addrs.first().map_or_else(String::new, |a| a.to_string()), None)
    }
    /// A running game's listener on a joiner's machine (a host migration's successor takes the survivors on it):
    /// no LAN announcement.
    fn listen_game(&mut self) -> Result<Box<dyn Listener>, String> {
        self.listen()
    }
    /// The rendezvous service hosting and join codes use from now on (ADR 0018; `None`: direct only).
    fn set_relay(&mut self, _relay: Option<SocketAddrV4>) {}
}

/// QUIC links and LAN discovery (ADR 0008), and the rendezvous service the player set (ADR 0018).
#[derive(Default)]
pub struct QuicNet {
    browser: Option<ig2_net::discovery::Browser>,
    queried: Option<std::time::Instant>,
    relay: Option<SocketAddrV4>,
}

struct QuicListener {
    host: ig2_net::quic::QuicHost,
    announcer: Option<ig2_net::discovery::Announcer>,
    /// The router's answer to come (a mapping granted after this listener is gone is dropped, which removes it).
    asking: Option<Receiver<Result<ig2_net::portmap::Mapping, String>>>,
    /// The live mapping: renewed in the background, removed when dropped (leaving the room or the game).
    mapping: Option<ig2_net::portmap::Mapping>,
    map: PortMap,
    /// This machine's address on the LAN (the route towards the internet; nothing is sent).
    lan: Option<std::net::Ipv4Addr>,
}

impl QuicListener {
    fn new(host: ig2_net::quic::QuicHost, announcer: Option<ig2_net::discovery::Announcer>) -> Self {
        let lan = ig2_net::portmap::local_ipv4(std::net::Ipv4Addr::new(192, 0, 2, 1)).ok();
        Self { host, announcer, asking: None, mapping: None, map: PortMap::Off, lan }
    }
}

/// Every protocol's failure, in the order tried.
fn map_errors(errors: &[(ig2_net::portmap::Protocol, ig2_net::portmap::Error)]) -> String {
    let name = |p| match p {
        ig2_net::portmap::Protocol::Upnp => "UPnP",
        ig2_net::portmap::Protocol::NatPmp => "NAT-PMP",
    };
    errors.iter().map(|(p, e)| format!("{}: {e}", name(*p))).collect::<Vec<_>>().join("; ")
}

impl Listener for QuicListener {
    fn accept(&mut self) -> Option<Box<dyn Transport>> {
        if let Some(a) = &mut self.announcer {
            if let Err(e) = a.poll() {
                eprintln!("ig2-client: LAN announcement: {e}");
            }
        }
        if let Some(rx) = &self.asking {
            match rx.try_recv() {
                Ok(Ok(m)) => {
                    self.mapping = Some(m);
                    self.asking = None;
                }
                Ok(Err(e)) => {
                    eprintln!("ig2-client: port mapping: {e}");
                    self.map = PortMap::Failed(e);
                    self.asking = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.map = PortMap::Failed("no answer".into());
                    self.asking = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(m) = &self.mapping {
            // A renewal may move the external address.
            self.map = PortMap::Mapped(m.granted().external);
        }
        self.host.accept().map(|l| Box::new(l) as Box<dyn Transport>)
    }

    fn map_port(&mut self, on: bool) {
        if !on {
            self.asking = None;
            self.mapping = None;
            self.map = PortMap::Off;
            return;
        }
        if self.asking.is_some() || self.mapping.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let port = self.port();
        std::thread::spawn(move || {
            let _ = tx.send(ig2_net::portmap::open_live(port, None).map_err(|e| map_errors(&e)));
        });
        self.asking = Some(rx);
        self.map = PortMap::Asking;
    }

    fn port_map(&self) -> PortMap {
        self.map.clone()
    }

    fn addrs(&self) -> Vec<std::net::SocketAddrV4> {
        let mut v: Vec<std::net::SocketAddrV4> = self.lan.map(|ip| std::net::SocketAddrV4::new(ip, self.port())).into_iter().collect();
        if let PortMap::Mapped(a) = self.map {
            v.push(a);
        }
        // The address the rendezvous service sees (the router's side of a NAT).
        if let Rendezvous::Registered(a) = self.host.rendezvous() {
            v.push(a);
        }
        v
    }

    fn register(&mut self, session: u64) {
        self.host.register(session);
    }

    fn rendezvous(&self) -> Rendezvous {
        self.host.rendezvous()
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
        let bind = |port| match self.relay {
            Some(r) => ig2_net::quic::QuicHost::bind_relayed(any(port), r),
            None => ig2_net::quic::QuicHost::bind(any(port)),
        };
        let host = bind(GAME_PORT).or_else(|_| bind(0)).map_err(|e| e.to_string())?;
        let announcer = ig2_net::discovery::Announcer::bind_lan().map_err(|e| eprintln!("ig2-client: LAN announcement: {e}")).ok();
        Ok(Box::new(QuicListener::new(host, announcer)))
    }

    fn listen_game(&mut self) -> Result<Box<dyn Listener>, String> {
        let any = |port| std::net::SocketAddr::from(([0, 0, 0, 0], port));
        let host = ig2_net::quic::QuicHost::bind(any(GAME_PORT)).or_else(|_| ig2_net::quic::QuicHost::bind(any(0))).map_err(|e| e.to_string())?;
        Ok(Box::new(QuicListener::new(host, None)))
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

    fn set_relay(&mut self, relay: Option<SocketAddrV4>) {
        self.relay = relay;
    }

    fn connect_code(&mut self, code: &JoinCode) -> Connecting {
        let (tx, rx) = mpsc::channel();
        let (each, answers) = mpsc::channel();
        for &a in &code.addrs {
            let (each, pin) = (each.clone(), ig2_net::quic::Pin::Prefix(code.pin.to_vec()));
            std::thread::spawn(move || {
                let _ = each.send(ig2_net::quic::connect_pinned(a.into(), &pin).map_err(|e| format!("{a}: {e}")));
            });
        }
        // And through the rendezvous service: its hole punch, else its relay (ADR 0018 §4).
        if let Some(r) = self.relay {
            let (each, session, pin) = (each.clone(), code.session, code.pin);
            std::thread::spawn(move || {
                let _ = each.send(ig2_net::quic::connect_via(r, session, pin, ig2_net::quic::Via::PunchThenRelay).map_err(|e| format!("relay: {e}")));
            });
        }
        drop(each);
        std::thread::spawn(move || {
            // The first link up wins; the others are dropped as they come.
            let mut errors = Vec::new();
            for r in answers {
                match r {
                    Ok(l) => {
                        let _ = tx.send(Ok(Box::new(l) as Box<dyn Transport + Send>));
                        return;
                    }
                    Err(e) => errors.push(e),
                }
            }
            let _ = tx.send(Err(if errors.is_empty() { "the join code has no address".into() } else { errors.join("; ") }));
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
    Relay,
}

/// Our side of the room.
pub enum Role {
    Host { lobby: Box<LobbyHost>, listener: Box<dyn Listener> },
    Joiner(Box<LobbyClient>),
}

/// The room's start, for [`launch`]: the host's room (`save`: the MP save it resumes), or a joiner's start.
pub enum Begun {
    Host { lobby: Box<LobbyHost>, listener: Box<dyn Listener>, save: Option<Box<MpSave>> },
    Joiner { start: Box<Start>, link: Box<dyn Transport>, me: Me },
}

/// An MP save in the save folder (the connection page's "Saved game" list).
pub struct SavedGame {
    /// The slot's name and date.
    pub title: Vec<u8>,
    pub save: Box<MpSave>,
}

/// The MP saves among `dir`'s slots that `rules` can resume (`options::read_slots`, `MpSave::read`).
pub fn saved_games(dir: Option<&std::path::Path>, rules: [u8; 32]) -> Vec<SavedGame> {
    crate::options::read_slots(dir)
        .into_iter()
        .flatten()
        .filter(|s| s.ours)
        .filter_map(|s| {
            let save = MpSave::read(&std::fs::read(&s.file).ok()?, rules).ok()?;
            let mut title = s.name.clone();
            title.extend_from_slice(format!("  {}.{:02}.{:02}", s.date[0], s.date[1], s.date[2]).as_bytes());
            Some(SavedGame { title, save: Box::new(save) })
        })
        .collect()
}

/// What a press or a key asks of the main menu.
pub enum Out {
    /// The menu click (`audio::CLICK`).
    Click,
    /// The screens are left: back to the multiplayer rings.
    Close,
    /// The room started: build the game ([`launch`]).
    Start(Begun),
    /// Put this text on the clipboard (the host's join code) and click.
    Copy(String),
}

/// Hot-spot ids (ours).
mod spot {
    pub const LIST: u32 = 0x10;
    pub const ADDRESS: u32 = 0x30;
    pub const PASSWORD: u32 = 0x31;
    pub const HOST: u32 = 0x32;
    pub const JOIN: u32 = 0x33;
    pub const CANCEL: u32 = 0x34;
    pub const SAVED: u32 = 0x35;
    pub const RACE: u32 = 0x40;
    pub const HOLDER: u32 = 0x50;
    /// Option k's left arrow = OPTION + 2k, its right arrow + 1.
    pub const OPTION: u32 = 0x60;
    pub const LEAVE: u32 = 0x70;
    pub const READY: u32 = 0x71;
    pub const PORT_MAP: u32 = 0x72;
    pub const PRIVATE: u32 = 0x73;
    pub const CODE: u32 = 0x74;
    pub const RELAY: u32 = 0x36;
}

/// The connection page's relay field.
const RELAY_BOX: [f32; 4] = [130.0, 354.0, 380.0, 18.0];

/// The options rows (spec §6).
const OPTIONS: usize = 5;
/// The connection page's buttons' left edges (Host, Join, Saved game, Cancel).
const BUTTONS: [f32; 4] = [130.0, 227.0, 323.0, 420.0];

/// Seat the room's members whose names are the save's humans in their saved empires, once each (a member may move
/// on afterwards); `seated` = the members done.
fn seat_saved(lobby: &mut LobbyHost, save: &MpSave, seated: &mut Vec<u8>) {
    let st = lobby.state().clone();
    for m in &st.members {
        if seated.contains(&m.id) {
            continue;
        }
        seated.push(m.id);
        let Some(h) = save.session.humans.iter().find(|h| h.name == m.name) else { continue };
        if st.empire_of(m.id).is_none() && st.slots.iter().any(|s| s.empire == h.empire && s.kind == SlotKind::Open) {
            lobby.pick(m.id, Some(h.empire));
        }
    }
}

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
    /// The rendezvous service's address (ADR 0018), empty for none; kept in the save folder's `relay.txt`.
    pub relay: Field,
    pub focus: Focus,
    /// The status line (a refusal, a connect error).
    pub line: Vec<u8>,
    /// The host was lost (`Start_ClientDeleted`'s joiner side: 14/5/3 "Disconnected" on the connection page).
    pub lost: bool,
    /// A connection attempt: the password and the join code's session id to say hello with.
    connecting: Option<(Connecting, Option<String>, Option<u64>)>,
    pub role: Option<Role>,
    pub chat: Vec<(String, String)>,
    pub entry: Field,
    pub at: [f32; 2],
    /// The password typed for the room this machine joined (kept for a host migration's door).
    joined_password: Option<String>,
    /// The connection page lists the save folder's MP saves instead of the LAN's sessions.
    pub saves: Option<Vec<SavedGame>>,
    /// Where the saves are (`options::slot_dir`).
    pub save_dir: Option<std::path::PathBuf>,
    /// The host's room resumes this save; the members already seated in their saved empires.
    resume: Option<(Box<MpSave>, Vec<u8>)>,
}

/// Our English words where the original has none (ADR 0014 §4).
mod words {
    pub const HOST: &[u8] = b"Host game";
    pub const PASSWORD: &[u8] = b"Password (optional)";
    pub const NONE_FOUND: &[u8] = b"No games found on the local network";
    pub const INET: &[u8] = b"No internet matchmaking: join by address or join code";
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
    pub const SAVED: &[u8] = b"Saved game";
    pub const LAN: &[u8] = b"LAN games";
    pub const NO_SAVES: &[u8] = b"No multiplayer saves";
    pub const NEED_SAVED: &[u8] = b"Every saved empire needs its player";
    pub const MAP_OFF: &[u8] = b"Port mapping (UPnP / NAT-PMP): off";
    pub const MAP_ASKING: &[u8] = b"Port mapping: asking the router...";
    pub const PRIVATE: [&[u8]; 2] = [b"Private game: no", b"Private game: yes"];
    pub const NO_CODE: &[u8] = b"Join code: none (no network address)";
    pub const COPIED: &[u8] = b"The join code is on the clipboard";
    pub const RELAY: &[u8] = b"Relay server (optional): ";
}

/// A typed relay address ("host" or "host:port", the default port [`ig2_net::rendezvous::DEFAULT_PORT`]); empty is
/// none. IPv4 only, like join codes.
pub fn relay_addr(typed: &str) -> Result<Option<SocketAddrV4>, String> {
    use std::net::ToSocketAddrs;
    let s = typed.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let with_port = if s.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) { s.to_string() } else { format!("{s}:{}", ig2_net::rendezvous::DEFAULT_PORT) };
    let mut all = with_port.to_socket_addrs().map_err(|e| e.to_string())?;
    all.find_map(|a| match a {
        std::net::SocketAddr::V4(a) => Some(Some(a)),
        _ => None,
    })
    .ok_or_else(|| "no IPv4 address".into())
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
            address: Field::new(MAX_ADDRESS),
            password: Field::new(MAX_FIELD),
            relay: Field::new(MAX_ADDRESS),
            focus: Focus::Address,
            line: if inet { words::INET.to_vec() } else { Vec::new() },
            lost: false,
            connecting: None,
            role: None,
            chat: Vec::new(),
            entry: Field::new(ig2_net::lobby::MAX_CHAT),
            at: [0.0; 2],
            joined_password: None,
            saves: None,
            save_dir: crate::options::slot_dir(),
            resume: None,
        }
        .with_saved_relay()
    }

    fn with_saved_relay(mut self) -> Self {
        let saved = self.save_dir.as_ref().and_then(|d| std::fs::read_to_string(d.join(RELAY_FILE)).ok()).unwrap_or_default();
        for c in saved.trim().bytes().filter(|c| (0x20..0x7f).contains(c)) {
            self.relay.key(EntryKey::Char(c));
        }
        self
    }

    /// The relay field to `net` (and to `relay.txt` when it changed); false, with the reason on the status line,
    /// when it names no address.
    fn apply_relay(&mut self, net: &mut dyn Net) -> bool {
        let typed = self.relay.string();
        match relay_addr(&typed) {
            Ok(a) => {
                net.set_relay(a);
                if let Some(d) = &self.save_dir {
                    let file = d.join(RELAY_FILE);
                    if std::fs::read_to_string(&file).unwrap_or_default().trim() != typed.trim() {
                        let _ = std::fs::write(file, typed.trim());
                    }
                }
                true
            }
            Err(e) => {
                self.line = format!("Relay server: {e}").into_bytes();
                false
            }
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
        if !self.apply_relay(net) {
            return;
        }
        match net.listen() {
            Ok(mut listener) => {
                let settings = Settings { name: format!("{}'s galaxy", self.name), scenario: "galaxy".into(), private: false, password: self.password() };
                let lobby = LobbyHost::new_galaxy(settings, ig2_net::discovery::random_u64(), self.rules, &self.name);
                listener.register(lobby.session());
                self.chat.clear();
                self.role = Some(Role::Host { lobby: Box::new(lobby), listener });
                self.resume = None;
                self.line.clear();
            }
            Err(e) => self.line = format!("Can't host: {e}").into_bytes(),
        }
    }

    /// Host a room for the listed save `i` (rules/multiplayer.md §7.3): a scenario-style room of the saved humans'
    /// empires, the host in the one of its name (else the first).
    pub fn host_save(&mut self, i: usize, net: &mut dyn Net) {
        let Some(save) = self.saves.as_ref().and_then(|s| s.get(i)).map(|s| s.save.clone()) else { return };
        if !self.apply_relay(net) {
            return;
        }
        match net.listen() {
            Ok(mut listener) => {
                let s = &save.session;
                let settings = Settings { name: s.name.clone(), scenario: s.scenario.clone(), private: false, password: self.password() };
                let slots: Vec<(u8, u8)> = s.humans.iter().map(|h| (h.empire, 0)).collect();
                let mut lobby = LobbyHost::new(settings, ig2_net::discovery::random_u64(), self.rules, &slots, &self.name);
                let mine = s.humans.iter().find(|h| h.name == lobby.state().members[0].name).or(s.humans.first()).map(|h| h.empire);
                lobby.pick(0, mine);
                listener.register(lobby.session());
                self.chat.clear();
                self.role = Some(Role::Host { lobby: Box::new(lobby), listener });
                self.resume = Some((save, vec![0]));
                self.saves = None;
                self.selected = None;
                self.line.clear();
            }
            Err(e) => self.line = format!("Can't host: {e}").into_bytes(),
        }
    }

    /// The host's join code (its session, certificate and addresses), once the listener knows an address.
    pub fn join_code(&self) -> Option<JoinCode> {
        let Some(Role::Host { lobby, listener }) = &self.role else { return None };
        let addrs = listener.addrs();
        (!addrs.is_empty()).then(|| JoinCode::new(lobby.session(), &listener.fingerprint(), &addrs))
    }

    /// Clipboard text into the focused field (the connection page's) or the chat line (the room), up to its first
    /// line break.
    pub fn paste(&mut self, text: &str) {
        let f = match (&self.role, self.focus) {
            (Some(_), _) => &mut self.entry,
            (None, Focus::Address) => &mut self.address,
            (None, Focus::Password) => &mut self.password,
            (None, Focus::Relay) => &mut self.relay,
        };
        for c in text.trim().lines().next().unwrap_or("").bytes().filter(|c| (0x20..0x7f).contains(c)) {
            f.key(EntryKey::Char(c));
        }
        if self.role.is_none() && self.focus == Focus::Address {
            self.selected = None;
        }
    }

    /// Ctrl+C or a click on the code: the host's join code to the clipboard.
    pub fn copy(&mut self) -> Option<Out> {
        let code = self.join_code()?.encode();
        self.line = words::COPIED.to_vec();
        Some(Out::Copy(code))
    }

    /// The connection page's list: the LAN's sessions, or the MP saves.
    fn rows(&self) -> usize {
        self.saves.as_ref().map_or(self.list.len(), Vec::len)
    }

    /// Join the selected LAN session, else the typed or pasted join code or address.
    pub fn join(&mut self, net: &mut dyn Net) {
        let typed = self.address.string();
        let (rx, code) = match self.selected.and_then(|i| self.list.get(i)) {
            Some(f) => (net.connect(&f.addr.to_string(), Some(f.announcement.fingerprint)), None),
            None if typed.is_empty() => return,
            None => match JoinCode::decode(&typed) {
                Ok(code) => {
                    if !self.apply_relay(net) {
                        return;
                    }
                    (net.connect_code(&code), Some(code.session))
                }
                // Long and without dots or colons: a mistyped code rather than a host name.
                Err(e) if !typed.contains(['.', ':']) && typed.len() > 20 => {
                    self.line = e.to_string().into_bytes();
                    return;
                }
                Err(_) => (net.connect(&typed, None), None),
            },
        };
        self.connecting = Some((rx, self.password(), code));
        self.line = words::CONNECTING.to_vec();
    }

    /// Join over `link` (a connection that just came up); `code` = the join code's session id.
    pub fn join_link(&mut self, link: Box<dyn Transport>, password: Option<String>, code: Option<u64>) {
        self.chat.clear();
        self.joined_password = password.clone();
        self.role = Some(Role::Joiner(Box::new(LobbyClient::join(link, &self.name, self.rules, password, code))));
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
        if let Some((rx, pw, code)) = &self.connecting {
            match rx.try_recv() {
                Ok(Ok(link)) => {
                    let (pw, code) = (pw.clone(), *code);
                    self.connecting = None;
                    self.join_link(link, pw, code);
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
                if let Some((save, seated)) = &mut self.resume {
                    seat_saved(lobby, save, seated);
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
                        let me = Me { name: self.name.clone(), password: self.joined_password.clone(), rules: self.rules };
                        Some(Out::Start(Begun::Joiner { start: Box::new(start), link, me }))
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
                for i in 0..self.rows().min(LIST_ROWS) {
                    v.push((spot::LIST + i as u32, [130.0, 82.0 + 16.0 * i as f32, 380.0, 16.0]));
                }
                v.extend([(spot::ADDRESS, [130.0, 280.0, 380.0, 22.0]), (spot::PASSWORD, [130.0, 328.0, 380.0, 22.0]), (spot::RELAY, RELAY_BOX)]);
                for (k, id) in [spot::HOST, spot::JOIN, spot::SAVED, spot::CANCEL].into_iter().enumerate() {
                    v.push((id, [BUTTONS[k], 392.0, 90.0, 30.0]));
                }
            }
            Some(_) => {
                for i in 0..8 {
                    let y = 52.0 + 32.0 * i as f32;
                    v.push((spot::RACE + i, [44.0, y, 120.0, 26.0]));
                    v.push((spot::HOLDER + i, [170.0, y, 160.0, 26.0]));
                }
                if self.is_host() {
                    v.extend([(spot::PORT_MAP, [20.0, 318.0, 400.0, 16.0]), (spot::PRIVATE, [430.0, 318.0, 190.0, 16.0]), (spot::CODE, [20.0, 336.0, 600.0, 16.0])]);
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
                spot::RELAY => self.focus = Focus::Relay,
                spot::HOST if self.saves.is_some() => {
                    if let Some(k) = self.selected {
                        self.host_save(k, net);
                    }
                }
                spot::HOST => self.host(net),
                spot::JOIN if self.saves.is_some() => {}
                spot::JOIN => self.join(net),
                spot::SAVED => {
                    self.saves = if self.saves.is_some() { None } else { Some(saved_games(self.save_dir.as_deref(), self.rules)) };
                    self.selected = None;
                }
                spot::CANCEL => return Some(Out::Close),
                i if (spot::LIST..spot::LIST + LIST_ROWS as u32).contains(&i) => {
                    let k = (i - spot::LIST) as usize;
                    let again = self.selected == Some(k);
                    self.selected = Some(k);
                    match (again, self.saves.is_some()) {
                        (true, true) => self.host_save(k, net),
                        (true, false) => self.join(net),
                        _ => {}
                    }
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
            spot::PRIVATE => {
                let Some(Role::Host { lobby, .. }) = self.role.as_mut() else { return None };
                let private = !lobby.state().private;
                lobby.set_private(private);
            }
            spot::CODE => return self.copy(),
            spot::PORT_MAP => {
                let Some(Role::Host { listener, .. }) = self.role.as_mut() else { return None };
                let on = matches!(listener.port_map(), PortMap::Off | PortMap::Failed(_));
                listener.map_port(on);
            }
            spot::READY => {
                let ready = st.members.iter().find(|m| m.id == me).is_some_and(|m| m.ready);
                match self.role.as_mut()? {
                    // The host's press is its ready and the start (exe 0x5c0120's button 0x44).
                    Role::Host { lobby, .. } => {
                        lobby.set_ready(0, true);
                        let saved = self.resume.as_ref().map(|(s, _)| {
                            let mut want: Vec<u8> = s.session.humans.iter().map(|h| h.empire).collect();
                            let mut held: Vec<u8> = st.members.iter().filter_map(|m| st.empire_of(m.id)).collect();
                            want.sort_unstable();
                            held.sort_unstable();
                            want == held
                        });
                        if saved == Some(false) {
                            self.line = words::NEED_SAVED.to_vec();
                            return Some(Out::Click);
                        }
                        if lobby.can_start() {
                            let Some(Role::Host { lobby, mut listener }) = self.role.take() else { unreachable!() };
                            listener.announce(None);
                            let save = self.resume.take().map(|r| r.0);
                            return Some(Out::Start(Begun::Host { lobby, listener, save }));
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
            Focus::Relay => &mut self.relay,
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
        let cursor = |f: &mut Frame, fl: &Field, first: usize, x: f32, y: f32, size: usize| {
            if blink {
                let (w, h) = (d.font.advance(size), d.font.height(size));
                let cx = x + w * (fl.cursor - first) as f32;
                let c = d.col(1, fade);
                f.push(d.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(cx + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
            }
        };
        let Some(st) = self.state() else {
            // The connection page.
            for r in [[120.0, 40.0, 400.0, 400.0], [125.0, 45.0, 390.0, 24.0], [125.0, 76.0, 390.0, 172.0], [130.0, 280.0, 380.0, 22.0], [130.0, 328.0, 380.0, 22.0], RELAY_BOX] {
                box_(f, r);
            }
            for x in BUTTONS {
                box_(f, [x, 392.0, 90.0, 30.0]);
            }
            let title = d.texts.get(15, 1, if self.inet { 30 } else { 29 });
            text(f, &title, 320.0, 50.0, 3, Align::Centre, 1);
            if self.rows() == 0 {
                text(f, if self.saves.is_some() { words::NO_SAVES } else { words::NONE_FOUND }, 320.0, 150.0, 2, Align::Centre, 0x25);
            }
            for (i, s) in self.saves.iter().flatten().take(LIST_ROWS).enumerate() {
                let c = if self.selected == Some(i) { 2 } else { 0x22 };
                text(f, &s.title, 136.0, 84.0 + 16.0 * i as f32, 2, Align::Left, c);
                text(f, format!("{}", s.save.session.humans.len()).as_bytes(), 500.0, 84.0 + 16.0 * i as f32, 2, Align::Right, c);
            }
            for (i, s) in self.list.iter().take(LIST_ROWS).enumerate().filter(|_| self.saves.is_none()) {
                let a = &s.announcement;
                let y = 84.0 + 16.0 * i as f32;
                let c = if self.selected == Some(i) { 2 } else if a.rules != self.rules || a.started { 0x25 } else { 0x22 };
                let lock = if a.password { "* " } else { "" };
                text(f, format!("{lock}{}", a.name).as_bytes(), 136.0, y, 2, Align::Left, c);
                text(f, format!("{}/{}", a.humans, a.slots).as_bytes(), 500.0, y, 2, Align::Right, c);
            }
            text(f, &d.texts.get(14, 5, 2), 130.0, 266.0, 2, Align::Left, 0x25);
            // A long address (a join code) scrolls: its end shows, or from the cursor on when that is further left.
            let first = self.address.text.len().saturating_sub(ADDRESS_SHOWN).min(self.address.cursor);
            let shown = &self.address.text[first..(first + ADDRESS_SHOWN).min(self.address.text.len())];
            text(f, shown, 136.0, 285.0, 2, Align::Left, if self.focus == Focus::Address { 2 } else { 0x22 });
            text(f, words::PASSWORD, 130.0, 314.0, 2, Align::Left, 0x25);
            let stars = vec![b'*'; self.password.text.len()];
            text(f, &stars, 136.0, 333.0, 2, Align::Left, if self.focus == Focus::Password { 2 } else { 0x22 });
            text(f, words::RELAY, 136.0, 357.0, 2, Align::Left, 0x25);
            let relay_x = 136.0 + d.font.advance(2) * words::RELAY.len() as f32;
            let relay_first = self.relay.text.len().saturating_sub(RELAY_SHOWN).min(self.relay.cursor);
            text(f, &self.relay.text[relay_first..(relay_first + RELAY_SHOWN).min(self.relay.text.len())], relay_x, 357.0, 2, Align::Left, if self.focus == Focus::Relay { 2 } else { 0x22 });
            match self.focus {
                Focus::Address => cursor(f, &self.address, first, 136.0, 285.0, 2),
                Focus::Password => cursor(f, &self.password, 0, 136.0, 333.0, 2),
                Focus::Relay => cursor(f, &self.relay, relay_first, relay_x, 357.0, 2),
            }
            let line = if self.lost && self.line.is_empty() { d.texts.get(14, 5, 3) } else { self.line.clone() };
            text(f, &line, 320.0, 377.0, 2, Align::Centre, 2);
            let saved = if self.saves.is_some() { words::LAN } else { words::SAVED };
            let join_c = if self.saves.is_some() { 0x25 } else { hot(spot::JOIN) };
            let labels: [(&[u8], usize); 4] = [(words::HOST, hot(spot::HOST)), (&d.texts.get(16, 3, 3), join_c), (saved, hot(spot::SAVED)), (&d.texts.get(15, 3, 7), hot(spot::CANCEL))];
            for (k, (label, c)) in labels.into_iter().enumerate() {
                text(f, label, BUTTONS[k] + 45.0, 402.0, 2, Align::Centre, c);
            }
            return;
        };
        // The room.
        for r in [[16.0, 8.0, 608.0, 26.0], [16.0, 40.0, 380.0, 272.0], [404.0, 40.0, 220.0, 272.0], [16.0, 316.0, 608.0, 40.0], [16.0, 360.0, 608.0, 80.0], [16.0, 444.0, 120.0, 30.0], [504.0, 444.0, 120.0, 30.0]] {
            box_(f, r);
        }
        text(f, st.name.as_bytes(), 320.0, 13.0, 3, Align::Centre, 1);
        let me = self.member();
        for (i, s) in st.slots.iter().enumerate().take(8) {
            let y = 58.0 + 32.0 * i as f32;
            let own = me.is_some_and(|m| s.kind == SlotKind::Human(m));
            text(f, format!("{}", s.empire).as_bytes(), 26.0, y, 2, Align::Left, if own { 2 } else { 0x25 });
            if st.galaxy.is_some() {
                let race_c = if hover == spot::RACE + i as u32 && (own || self.is_host()) { 2 } else { 0x22 };
                text(f, &d.texts.get(0x11, 1, (s.race.clamp(0, 19) + 5) as u16), 48.0, y, 2, Align::Left, race_c);
            } else if let Some(h) = self.resume.as_ref().and_then(|(sv, _)| sv.session.humans.iter().find(|h| h.empire == s.empire)) {
                // A resumed save: whose empire it was.
                text(f, h.name.as_bytes(), 48.0, y, 2, Align::Left, 0x25);
            }
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
        if let Some(Role::Host { listener, .. }) = &self.role {
            let map = match listener.port_map() {
                PortMap::Off => words::MAP_OFF.to_vec(),
                PortMap::Asking => words::MAP_ASKING.to_vec(),
                PortMap::Mapped(a) => format!("Port mapping: on, reachable at {a}").into_bytes(),
                PortMap::Failed(e) => format!("Port mapping failed: {e}").into_bytes(),
            };
            text(f, &map, 24.0, 320.0, 2, Align::Left, hot(spot::PORT_MAP));
            text(f, words::PRIVATE[st.private as usize], 476.0, 320.0, 2, Align::Left, hot(spot::PRIVATE));
            let code = self.join_code().map_or_else(|| words::NO_CODE.to_vec(), |c| format!("Join code: {}  (click: copy)", c.encode()).into_bytes());
            text(f, &code, 24.0, 338.0, 2, Align::Left, hot(spot::CODE));
            let relay = match listener.rendezvous() {
                Rendezvous::Off => None,
                Rendezvous::Asking => Some("Relay server: opening the room...".to_string()),
                Rendezvous::Registered(a) => Some(format!("Relay server: room open, seen at {a}")),
                Rendezvous::Refused(ig2_net::rendezvous::Refusal::Full) => Some("Relay server: full, try later".into()),
                Rendezvous::Refused(_) => Some("Relay server: refused the room".into()),
                Rendezvous::NoAnswer => Some("Relay server: no answer".into()),
            };
            if let Some(r) = relay {
                text(f, r.as_bytes(), 24.0, 351.0, 2, Align::Left, 0x22);
            }
        } else if st.private {
            text(f, words::PRIVATE[1], 476.0, 320.0, 2, Align::Left, 0x25);
        }
        let first = self.chat.len().saturating_sub(CHAT_LINES);
        for (j, (who, line)) in self.chat[first..].iter().enumerate() {
            text(f, format!("{who}: {line}").as_bytes(), 24.0, 364.0 + 14.0 * j as f32, 2, Align::Left, 0x22);
        }
        text(f, &self.entry.text, 24.0, 424.0, 2, Align::Left, 2);
        cursor(f, &self.entry, 0, 24.0, 424.0, 2);
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
    Client(Box<ClientSession<'r>>, Box<Survivor>),
    /// Only inside [`Session::pump`], while a joiner becomes the host.
    Handover,
}

/// How long a survivor waits for a successor's connection before it tries the next one.
pub const MIGRATE_CONNECT: Duration = Duration::from_secs(5);

/// What a joiner's machine keeps for a host migration (rules/multiplayer.md §7.3): its name in the room, the password it
/// typed and the rules fingerprint (the successor's door checks all three).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Me {
    pub name: String,
    pub password: Option<String>,
    pub rules: [u8; 32],
}

/// A joiner's side of a host migration (ADR 0012 §2): its own listener, where links to the successors come from, and
/// the election as far as it got.
pub struct Survivor {
    net: Box<dyn Net>,
    listener: Option<Box<dyn Listener>>,
    me: Me,
    /// A migration runs: the successors not tried yet.
    left: Option<Vec<u8>>,
    /// The connection to the successor tried now: its slot, the attempt and when it began.
    attempt: Option<(u8, Connecting, Duration)>,
    /// No successor answered: the game ends here.
    failed: bool,
}

/// What the election asks of the session.
enum Step {
    Wait,
    /// This machine is the first successor left: it hosts.
    TakeOver,
}

impl Survivor {
    /// The listener opens and the host is told where it is (`ClientSession::reach`).
    fn new(mut net: Box<dyn Net>, me: Me, c: &mut ClientSession) -> Self {
        let listener = net.listen_game().map_err(|e| eprintln!("ig2-client: migration listener: {e}")).ok();
        if let Some(l) = &listener {
            c.reach(l.port(), l.fingerprint());
        }
        Self { net, listener, me, left: None, attempt: None, failed: false }
    }

    /// One round of the election at `now` (the session's clock).
    fn step(&mut self, c: &mut ClientSession, now: Duration) -> Step {
        if c.kicked() || self.failed {
            return Step::Wait;
        }
        if c.refused.take().is_none() {
            if c.migrating() {
                return Step::Wait;
            }
            if !c.closed() {
                self.left = None;
                return Step::Wait;
            }
        }
        let left = self.left.get_or_insert_with(|| c.successors());
        if let Some((slot, rx, since)) = &self.attempt {
            match rx.try_recv() {
                Ok(Ok(link)) => {
                    let (slot, link) = (*slot, link as Box<dyn Transport>);
                    self.attempt = None;
                    c.migrate(slot, link, &self.me.name, self.me.password.clone(), self.me.rules);
                    return Step::Wait;
                }
                Err(mpsc::TryRecvError::Empty) if now.saturating_sub(*since) < MIGRATE_CONNECT => return Step::Wait,
                _ => self.attempt = None,
            }
        }
        while !left.is_empty() {
            let slot = left.remove(0);
            if slot == c.peer.slot {
                return Step::TakeOver;
            }
            if let Some(to) = c.candidate(slot) {
                self.attempt = Some((slot, self.net.connect(&to.addr, Some(to.cert)), now));
                return Step::Wait;
            }
        }
        self.failed = true;
        Step::Wait
    }
}

impl<'r> Session<'r> {
    /// This machine's player.
    pub fn peer(&self) -> &Peer<'r> {
        match self {
            Session::Host { session, .. } => &session.own,
            Session::Client(c, _) => &c.peer,
            Session::Handover => unreachable!("the handover is inside pump"),
        }
    }

    pub fn peer_mut(&mut self) -> &mut Peer<'r> {
        match self {
            Session::Host { session, .. } => &mut session.own,
            Session::Client(c, _) => &mut c.peer,
            Session::Handover => unreachable!("the handover is inside pump"),
        }
    }

    /// What holds the session now (the host's own view, a joiner's last word from the host).
    pub fn status(&self, now: Duration) -> HoldStatus {
        match self {
            Session::Host { session, .. } => session.status(now),
            Session::Client(c, _) => c.status.clone(),
            Session::Handover => HoldStatus::default(),
        }
    }

    /// A joiner's host is gone (left, the link closed or silent) and no successor took over (ADR 0012, rules/
    /// multiplayer.md §7.3), or the host kicked it: the game ends for it as it did in the original (`MultiCloseServer`).
    pub fn host_gone(&self) -> bool {
        match self {
            Session::Client(c, s) => c.kicked() || s.failed && c.closed(),
            _ => false,
        }
    }

    /// The slot of the machine that hosts the session (a migration changes it).
    pub fn host_slot(&self) -> u8 {
        match self {
            Session::Host { session, .. } => session.info().host,
            Session::Client(c, _) => c.info().host,
            Session::Handover => 0,
        }
    }

    /// This machine runs the session (`SuperVisor`).
    pub fn is_host(&self) -> bool {
        matches!(self, Session::Host { .. })
    }

    /// The host kicked this joiner (`SessionMsg::Kicked`).
    pub fn kicked(&self) -> bool {
        matches!(self, Session::Client(c, _) if c.kicked())
    }

    /// The host's humans still playing (`PlayerKi == 1`, the kick's targets; input.md §5.1), `races` =
    /// `Faj_Nev`. Empty on a joiner (only the host kicks).
    pub fn players(&self, races: &[Vec<u8>]) -> Vec<crate::console::Player> {
        let Session::Host { session, .. } = self else { return Vec::new() };
        let race = |e: u8| races.get(e as usize).cloned().unwrap_or_default();
        session.humans().filter(|h| h.2).map(|(empire, name, _)| crate::console::Player { empire, name: name.as_bytes().to_vec(), race: race(empire) }).collect()
    }

    /// Who the lockstep waits for (the wait overlay's cells, `crate::mp_wait`): the empires the session awaits (dropped
    /// in a battle, a migration's survivors) and, on the host, those whose input for the next turn hasn't come; a
    /// joiner that knows of none waits for the host.
    pub fn late(&self, now: Duration) -> Vec<u8> {
        let s = self.status(now);
        let mut v: Vec<u8> = s.waiting.iter().map(|w| w.0).chain(s.migrating.iter().copied()).collect();
        match self {
            Session::Host { session, .. } => v.extend(session.late()),
            Session::Client(c, _) if v.is_empty() => v.extend(c.humans().iter().find(|h| h.0 == c.info().host).map(|h| h.1)),
            _ => {}
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The host's "Close connection" in the wait overlay (`MultiKickClient(client, 1)`): `empire`'s player takes the
    /// drop path ([`HostSession::drop_player`]: it may come back through the door).
    pub fn drop_player(&mut self, empire: u8, now: Duration) -> Result<(), String> {
        let Session::Host { session, .. } = self else { return Err("only the host closes a connection".into()) };
        session.drop_player(empire, now).map_err(|e| e.to_string())
    }

    /// The host's `/kick` of `empire` ([`HostSession::kick`]).
    pub fn kick(&mut self, empire: u8, now: Duration) -> Result<(), String> {
        let Session::Host { session, .. } = self else { return Err("only the host kicks".into()) };
        session.kick(empire, now).map_err(|e| e.to_string())
    }

    /// The local player's chat line (`ExecuteString`'s 0x3fc): every machine shows it, this one included.
    pub fn say(&mut self, text: &str) {
        match self {
            Session::Host { session, .. } => session.say(text),
            Session::Client(c, _) => c.say(text),
            Session::Handover => {}
        }
    }

    /// The name `empire`'s player had in the room.
    pub fn name_of(&self, empire: u8) -> String {
        match self {
            Session::Host { session, .. } => session.humans().find(|h| h.0 == empire).map(|h| h.1.to_string()),
            Session::Client(c, _) => {
                let slot = c.humans().iter().find(|h| h.1 == empire).map(|h| h.0);
                c.info().names.iter().find(|n| Some(n.0) == slot).map(|n| n.1.clone())
            }
            Session::Handover => None,
        }
        .unwrap_or_else(|| format!("Empire {empire}"))
    }

    /// The name of the player in `slot`.
    pub fn name_in(&self, slot: u8) -> String {
        let info = match self {
            Session::Host { session, .. } => session.info(),
            Session::Client(c, _) => c.info(),
            Session::Handover => return String::new(),
        };
        info.names.iter().find(|n| n.0 == slot).map_or_else(|| format!("Player {slot}"), |n| n.1.clone())
    }

    /// `ExitToMainScreen`'s teardown (game-end.md §2: `MultiCloseServer` / `MultiCloseClient`): the others are told
    /// this player left (a joiner is away at once; the host's survivors migrate), then the links close as the session
    /// drops. A `Leave` lost on the way is the drop path's (closed link, keep-alive).
    pub fn leave(mut self) {
        match &mut self {
            Session::Host { session, .. } => session.leave(),
            Session::Client(c, _) => c.leave(),
            Session::Handover => {}
        }
    }

    /// At `now`: admit rejoiners (host), receive, send, run up to `max_turns` turns; a joiner whose host is gone runs
    /// the election and may become the host. Returns the turns run and the host's session events.
    pub fn pump(&mut self, now: Duration, max_turns: usize) -> Result<(usize, Vec<SessionEvent>), String> {
        if let Session::Client(c, s) = self {
            if let Step::TakeOver = s.step(c, now) {
                let Session::Client(c, mut s) = std::mem::replace(self, Session::Handover) else { unreachable!() };
                let session = HostSession::migrate(*c, s.me.rules, s.me.password.take(), now);
                let listener = s.listener.take().unwrap_or_else(|| Box::new(Unreached));
                *self = Session::Host { session: Box::new(session), listener };
            }
        }
        match self {
            Session::Host { session, listener } => {
                while let Some(l) = listener.accept() {
                    session.admit(l, now);
                }
                let before = session.own.turn;
                let events = session.pump(now, max_turns).map_err(|e| e.to_string())?;
                Ok(((session.own.turn - before) as usize, events))
            }
            Session::Client(c, _) => {
                let run = c.pump(now, max_turns).map_err(|e| e.to_string())?;
                Ok((run, c.chat.drain(..).map(|(empire, text)| SessionEvent::Said { empire, text }).collect()))
            }
            Session::Handover => unreachable!("the handover is inside pump"),
        }
    }
}

/// The room's start on this machine (ADR 0014 §2): the host builds the galaxy and starts the session with `ticks`
/// (its clock), or resumes the room's MP save; a joiner builds the same galaxy from the host's `GalaxyStart` and
/// checks its hash, or, joining a running game under its old name or a resumed save, starts from the host's snapshot
/// (the rejoin, `rules/multiplayer.md` §7.2). A joiner's `net` gives it its migration listener and the links to a
/// successor (§7.3).
pub fn launch<'r>(b: Begun, data: &StartData, rules: &'r GameRules, ticks: u32, net: Box<dyn Net>) -> Result<Session<'r>, String> {
    match b {
        Begun::Host { lobby, listener, save: Some(save) } => {
            let s = HostSession::resume(*lobby, &save, rules).map_err(|_| String::from_utf8_lossy(words::NEED_SAVED).into_owned())?;
            Ok(Session::Host { session: Box::new(s), listener })
        }
        Begun::Host { lobby, listener, save: None } => {
            let s = HostSession::start_galaxy(*lobby, data, rules, SessionSettings::default(), ticks).map_err(|_| "the room can't start yet".to_string())?;
            Ok(Session::Host { session: Box::new(s), listener })
        }
        Begun::Joiner { start, link, me } => {
            // A rejoin to a running game (the host's door matched the name to an away human) or a resumed save: the
            // host's state now.
            let c = if start.galaxy.is_none() { ClientSession::new(&start, link, rules) } else { ClientSession::new_galaxy(&start, link, data, rules) };
            let mut c = Box::new(c.map_err(|e| e.to_string())?);
            let s = Survivor::new(net, me, &mut c);
            Ok(Session::Client(c, Box::new(s)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    const RULES: [u8; 32] = [0x3c; 32];

    /// In-process links: listener `k` (port `k + 1`) takes the loopback ends joiners connect with.
    type Hub = Rc<RefCell<Vec<mpsc::Sender<Box<dyn Transport>>>>>;

    #[derive(Clone, Default)]
    struct LoopNet {
        hub: Hub,
    }

    struct LoopListener(Receiver<Box<dyn Transport>>, u16, PortMap);

    impl Listener for LoopListener {
        fn accept(&mut self) -> Option<Box<dyn Transport>> {
            self.0.try_recv().ok()
        }

        fn port(&self) -> u16 {
            self.1
        }

        /// A router that grants every mapping at a documentation address.
        fn map_port(&mut self, on: bool) {
            self.2 = if on { PortMap::Mapped(std::net::SocketAddrV4::new([203, 0, 113, 7].into(), 4000 + self.1)) } else { PortMap::Off };
        }

        fn port_map(&self) -> PortMap {
            self.2.clone()
        }

        /// "127.0.0.1:n" reaches listener n ([`LoopNet::connect`]).
        fn addrs(&self) -> Vec<std::net::SocketAddrV4> {
            vec![std::net::SocketAddrV4::new([127, 0, 0, 1].into(), self.1)]
        }
    }

    impl Net for LoopNet {
        fn listen(&mut self) -> Result<Box<dyn Listener>, String> {
            let (tx, rx) = mpsc::channel();
            self.hub.borrow_mut().push(tx);
            Ok(Box::new(LoopListener(rx, self.hub.borrow().len() as u16, PortMap::Off)))
        }

        fn sessions(&mut self) -> Vec<Found> {
            Vec::new()
        }

        /// "host:port" reaches that listener, anything else the first.
        fn connect(&mut self, addr: &str, _pin: Option<[u8; 32]>) -> Connecting {
            let (tx, rx) = mpsc::channel();
            let (a, b) = ig2_net::loopback();
            let port = addr.rsplit_once(':').and_then(|(_, p)| p.parse::<usize>().ok()).unwrap_or(1);
            let r = match self.hub.borrow().get(port.wrapping_sub(1)) {
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
            let hub = net.clone();
            let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 4242, Box::new(hub.clone()))).unwrap().map(|s| crate::world::World::new_mp(s, rules)).unwrap();
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
    /// once the host leaves, the last joiner takes the session over alone.
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
        let hub = net.clone();
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 77, Box::new(hub.clone()))).unwrap().map(|s| World::new_mp(s, rules)).unwrap();
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
        worlds[1].say(b"hello there", &[]);
        step(&mut worlds, &|w| w.iter().all(|w| w.chat_lines.iter().any(|l| l.0 == b"Guest0: hello there")));

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

        // The console (input.md §5.1): a joiner's `/kick` is chat; the host's failed kicks are its own red lines; its
        // `/kill guest1` ends Guest1's session, who is away on the others.
        worlds[1].say(b"/kick 3", &[]);
        step(&mut worlds, &|w| w.iter().all(|w| w.chat_lines.iter().any(|l| l.0 == b"Guest0: /kick 3")));
        let red = |w: &World, t: &[u8]| w.chat_lines.iter().any(|l| l.0 == t && l.1 == crate::console::RED);
        worlds[0].say(b"/kick 1", &[]);
        worlds[0].say(b"/kick Nobody", &[]);
        assert!(red(&worlds[0], crate::console::CANNOT_KICK_YOURSELF) && red(&worlds[0], crate::console::NO_SUCH_PLAYER));
        assert!(!worlds[1..].iter().any(|w| w.chat_lines.iter().any(|l| l.1 == crate::console::RED)));
        worlds[0].say(b"/kill guest1", &[]);
        step(&mut worlds, &|w| w[2].net_over && w[..2].iter().all(|w| w.net_seen.away == [3]));
        assert!(worlds[2].local_lines.iter().any(|l| l == b"The host has kicked you from the game."));
        assert!(worlds[2].net.as_ref().unwrap().0.kicked());
        worlds.pop();
        let at = turn(&worlds[0]);
        step(&mut worlds, &|w| w.iter().all(|w| turn(w) >= at + 10));
        let Some((Session::Host { session, .. }, _)) = &worlds[0].net else { panic!() };
        assert!(session.host.desyncs.is_empty(), "the kick caught the real game up: {:?}", session.host.desyncs);

        // The host leaves: Guest0, the only survivor, hosts the session alone and plays on.
        let (h, _) = worlds.remove(0).net.unwrap();
        h.leave();
        step(&mut worlds, &|w| w[0].mp_host() && w[0].local_lines.iter().any(|l| l == b"The host has left the game. Guest0 hosts it now."));
        let at = turn(&worlds[0]);
        step(&mut worlds, &|w| turn(&w[0]) >= at + 10);
        assert!(!worlds[0].net_over);
        assert_eq!(worlds[0].net_seen.away, [1, 3]);
    }

    /// Host migration in the client (ADR 0012, rules/multiplayer.md §7.3) and an MP save resumed from the room: three
    /// machines; every joiner learns where the others listen; the host leaves, Guest0 (the lowest slot left) takes
    /// over on its own listener and Guest1 connects there and resumes from its own state; they agree on every turn.
    /// The new host's MP save, written as a save slot, is listed by a new room's "Saved game"; its humans come back
    /// under their names, are seated in their empires and the game goes on from the saved turn.
    #[test]
    fn survivors_migrate_and_a_saved_game_resumes_from_the_room() {
        use crate::world::World;
        let Some(Ok(inst)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let rules: &'static GameRules = Box::leak(Box::new(crate::rules::load(&inst).unwrap().0));
        let texts = crate::new_game::StartTexts::read(&inst).unwrap();
        let ready_start = |host: &mut MpScreen, guests: &mut Vec<MpScreen>, net: &mut LoopNet| {
            for g in guests.iter_mut() {
                click(g, spot::READY, false, net);
            }
            sync(host, guests, net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready));
            let Some(Out::Start(b)) = click(host, spot::READY, false, net) else { panic!("no start") };
            b
        };
        let turn = |w: &World| w.net.as_ref().unwrap().0.peer().turn;
        let t = std::time::Instant::now();
        let step = |worlds: &mut Vec<World<'static>>, done: &dyn Fn(&[World]) -> bool| {
            while !done(worlds) {
                for w in worlds.iter_mut() {
                    w.pump();
                    w.tick();
                }
                assert!(t.elapsed() < Duration::from_secs(120), "stalled");
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        let agree = |worlds: &[World]| {
            let hashes = |w: &World| w.net.as_ref().unwrap().0.peer().hashes.clone();
            let h0 = hashes(&worlds[0]);
            for w in &worlds[1..] {
                let h = hashes(w);
                assert!(h.iter().filter(|x| h0.contains(x)).count() >= 10, "the peers agree");
                assert!(h.iter().all(|(t, x)| h0.iter().find(|(t0, _)| t0 == t).is_none_or(|(_, x0)| x0 == x)));
            }
            let Some((Session::Host { session, .. }, _)) = &worlds[0].net else { panic!("the host's world first") };
            assert!(session.host.desyncs.is_empty(), "{:?}", session.host.desyncs);
        };

        let mut net = LoopNet::default();
        let hub = net.clone();
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 99, Box::new(hub.clone()))).unwrap().map(|s| World::new_mp(s, rules)).unwrap();
        let (mut host, mut guests) = room(2, &mut net);
        let b = ready_start(&mut host, &mut guests, &mut net);
        let mut worlds = vec![mk(b)];
        worlds.extend(started(&mut guests, &mut net).into_iter().map(mk));
        step(&mut worlds, &|w| w.iter().all(|w| turn(w) >= 10) && w[1..].iter().all(|w| matches!(&w.net, Some((Session::Client(c, _), _)) if c.candidates.len() == 2)));
        let Some((Session::Client(c, _), _)) = &worlds[2].net else { panic!() };
        // The host's listener is port 1, the joiners' migration listeners 2 and 3 (`listen_game`).
        assert_eq!(c.candidates.iter().map(|c| (c.slot, c.addr.as_str())).collect::<Vec<_>>(), [(1, "127.0.0.1:2"), (2, "127.0.0.1:3")]);

        // The host leaves: Guest0 takes over, Guest1 resumes on it (no restart).
        let (h, _) = worlds.remove(0).net.unwrap();
        h.leave();
        let line: &[u8] = b"The host has left the game. Guest0 hosts it now.";
        step(&mut worlds, &|w| w[0].mp_host() && w.iter().all(|w| w.local_lines.iter().any(|l| l == line)) && w.iter().all(|w| w.net_seen.away == [1]));
        let at = turn(&worlds[0]);
        step(&mut worlds, &|w| w.iter().all(|w| turn(w) >= at + 20));
        agree(&worlds);
        let Some((Session::Client(c, _), _)) = &worlds[1].net else { panic!() };
        assert_eq!(c.restarts, 0, "resumed from its own state");
        assert_eq!(c.info().host, 1);
        assert!(!worlds.iter().any(|w| w.net_over));

        // The new host's MP save, as a save slot.
        let save = worlds[0].mp_save().unwrap().unwrap();
        let back = MpSave::read(&save, RULES).unwrap();
        assert_eq!(back.session.humans.iter().map(|h| (h.empire, h.name.as_str())).collect::<Vec<_>>(), [(1, "Host"), (2, "Guest0"), (3, "Guest1")]);
        let dir = std::env::temp_dir().join(format!("ig2-mp-resume-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("save3.ig2s"), crate::options::slot_file(save, b"three of us", [3001, 1, 2])).unwrap();
        for w in worlds.drain(..) {
            w.net.unwrap().0.leave();
        }

        // A new room: Guest0 hosts the save, Host and Guest1 join and are seated in their saved empires.
        let mut net = LoopNet::default();
        let hub = net.clone();
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 5, Box::new(hub.clone()))).unwrap().map(|s| World::new_mp(s, rules)).unwrap();
        let mut host = MpScreen::open(false, "Guest0", RULES);
        host.save_dir = Some(dir.clone());
        let mut guests = vec![MpScreen::open(false, "Host", RULES), MpScreen::open(false, "Guest1", RULES)];
        for _ in 0..25 {
            host.tick(&mut net);
            guests.iter_mut().for_each(|g| drop(g.tick(&mut net)));
        }
        click(&mut host, spot::SAVED, false, &mut net);
        assert_eq!(host.saves.as_ref().map(|s| s.iter().map(|s| s.title.clone()).collect::<Vec<_>>()), Some(vec![b"three of us  3001.01.02".to_vec()]));
        click(&mut host, spot::LIST, false, &mut net);
        click(&mut host, spot::LIST, false, &mut net);
        assert!(host.is_host());
        let st = host.state().unwrap();
        assert!(st.galaxy.is_none());
        assert_eq!(st.slots.iter().map(|s| s.empire).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(st.empire_of(0), Some(2), "the host sits in its saved empire");
        for g in &mut guests {
            type_text(g, "local", &mut net);
        }
        sync(&mut host, &mut guests, &mut net, |h| {
            let st = h.state().unwrap();
            st.members.len() == 3 && st.members.iter().all(|m| st.empire_of(m.id).is_some())
        });
        let st = host.state().unwrap().clone();
        let by_name = |n: &str| st.members.iter().find(|m| m.name == n).and_then(|m| st.empire_of(m.id));
        assert_eq!((by_name("Host"), by_name("Guest1")), (Some(1), Some(3)), "seated by name");
        let b = ready_start(&mut host, &mut guests, &mut net);
        assert!(matches!(&b, Begun::Host { save: Some(_), .. }));
        let mut worlds = vec![mk(b)];
        let begun = started(&mut guests, &mut net);
        assert!(begun.iter().all(|b| matches!(b, Begun::Joiner { start, .. } if start.galaxy.is_none() && !start.snapshot.is_empty())));
        worlds.extend(begun.into_iter().map(mk));
        assert!(worlds.iter().all(|w| turn(w) == back.snapshot.turn), "the game goes on from the saved turn");
        let en = |w: &World| w.game.state.mem.i32(ig2_sim::save::va::EN);
        assert_eq!(worlds.iter().map(en).collect::<Vec<_>>(), [2, 1, 3]);
        let at = back.snapshot.turn;
        step(&mut worlds, &|w| w.iter().all(|w| turn(w) >= at + 20));
        agree(&worlds);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The wait overlay's cells (`Session::late`, `crate::mp_wait`): a joiner that stops sending is late on the host
    /// (its input for the next turn hasn't come) and the host stands still; the joiner, knowing of no one, waits for
    /// the host.
    #[test]
    fn a_silent_joiner_is_late_on_the_host() {
        use crate::world::World;
        let Some(Ok(inst)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let rules: &'static GameRules = Box::leak(Box::new(crate::rules::load(&inst).unwrap().0));
        let texts = crate::new_game::StartTexts::read(&inst).unwrap();
        let mut net = LoopNet::default();
        let hub = net.clone();
        let (mut host, mut guests) = room(1, &mut net);
        click(&mut guests[0], spot::READY, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready));
        let Some(Out::Start(b)) = click(&mut host, spot::READY, false, &mut net) else { panic!("no start") };
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 3, Box::new(hub.clone()))).unwrap().map(|s| World::new_mp(s, rules)).unwrap();
        let mut worlds = vec![mk(b)];
        worlds.extend(started(&mut guests, &mut net).into_iter().map(mk));
        let turn = |w: &World| w.net.as_ref().unwrap().0.peer().turn;
        let t = std::time::Instant::now();
        while worlds.iter().any(|w| turn(w) < 10) {
            for w in &mut worlds {
                w.pump();
                w.tick();
            }
            assert!(t.elapsed() < Duration::from_secs(60), "stalled");
        }
        // Only the host pumps now: it runs out of the joiner's inputs.
        let mut last = turn(&worlds[0]);
        loop {
            worlds[0].pump();
            worlds[0].tick();
            let now = turn(&worlds[0]);
            if now == last && worlds[0].wait_view().unwrap().late.iter().any(|l| l.0 == 2) {
                break;
            }
            last = now;
            assert!(t.elapsed() < Duration::from_secs(60), "the host never waited");
        }
        let v = worlds[0].wait_view().unwrap();
        assert_eq!((v.paused, v.reconnecting, v.host, v.late), (false, false, true, vec![(2, "Guest0".to_string())]));
        let v = worlds[1].wait_view().unwrap();
        assert_eq!((v.host, v.late), (false, vec![(1, "Host".to_string())]));
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
    fn the_host_maps_its_port_from_the_room() {
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(1, &mut net);
        let map = |h: &MpScreen| match &h.role {
            Some(Role::Host { listener, .. }) => listener.port_map(),
            _ => unreachable!(),
        };
        assert_eq!(map(&host), PortMap::Off, "opt-in: nothing asks the router until the host does");
        assert!(!guests[0].boxes().iter().any(|b| b.0 == spot::PORT_MAP), "a joiner has no port mapping");
        assert!(matches!(click(&mut host, spot::PORT_MAP, false, &mut net), Some(Out::Click)));
        assert_eq!(map(&host), PortMap::Mapped("203.0.113.7:4001".parse().unwrap()));
        click(&mut host, spot::PORT_MAP, false, &mut net);
        assert_eq!(map(&host), PortMap::Off, "a second press removes the mapping");
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.len() == 2);
    }

    #[test]
    fn a_private_room_is_joined_with_its_code() {
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(1, &mut net);
        let session = match &host.role {
            Some(Role::Host { lobby, .. }) => lobby.session(),
            _ => unreachable!(),
        };
        assert!(!host.state().unwrap().private);
        click(&mut host, spot::PRIVATE, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().private);
        let fresh = |name: &str, net: &mut LoopNet| {
            let mut s = MpScreen::open(true, name, RULES);
            for _ in 0..25 {
                s.tick(net);
            }
            s
        };
        // By address alone: refused at the door.
        let mut plain = fresh("Plain", &mut net);
        type_text(&mut plain, "127.0.0.1:1", &mut net);
        let mut all = [&mut host, &mut plain];
        run(&mut all, &mut net, |s| s[1].line == refusal_text(Refusal::Code));
        // The host's code (a click copies it) pasted and joined.
        let Some(Out::Copy(text)) = click(&mut host, spot::CODE, false, &mut net) else { panic!("the code row copies") };
        assert_eq!(host.line, words::COPIED);
        let code = JoinCode::decode(&text).unwrap();
        assert_eq!((code.session, code.addrs.as_slice()), (session, &["127.0.0.1:1".parse().unwrap()][..]));
        let mut coded = fresh("Coded", &mut net);
        coded.paste(&format!(" {text}\nsecond line"));
        assert_eq!(coded.address.string(), text, "the first line, trimmed");
        coded.key(Some(EntryKey::Enter), false, &mut net);
        let mut all = [&mut host, &mut coded];
        run(&mut all, &mut net, |s| s[1].state().is_some_and(|st| st.members.len() == 3));
        // A mistyped code is caught before connecting.
        let mut typo = fresh("Typo", &mut net);
        let mut bad = text.into_bytes();
        bad[3] = if bad[3] == b'A' { b'B' } else { b'A' };
        typo.paste(std::str::from_utf8(&bad).unwrap());
        typo.key(Some(EntryKey::Enter), false, &mut net);
        assert!(typo.connecting.is_none() && typo.line.starts_with(b"the join code") || typo.line == b"not a join code", "{}", String::from_utf8_lossy(&typo.line));
        // Open again: the room's members see it.
        click(&mut host, spot::PRIVATE, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| !h.state().unwrap().private);
    }

    /// The real thing on this machine: a private room on a QUIC socket, joined over UDP with its code (the LAN address
    /// the code carries; skipped without one).
    #[test]
    fn a_join_code_works_over_quic() {
        let mut host_net = QuicNet::default();
        let mut host = MpScreen::open(true, "Host", RULES);
        host.open = 100.0;
        host.host(&mut host_net);
        click_on(&mut host, spot::PRIVATE, &mut host_net);
        let Some(Out::Copy(text)) = host.copy() else {
            eprintln!("no LAN address; skipping");
            return;
        };
        let mut net = QuicNet::default();
        let mut guest = MpScreen::open(true, "Guest", RULES);
        guest.open = 100.0;
        guest.paste(&text);
        guest.key(Some(EntryKey::Enter), false, &mut net);
        let t = std::time::Instant::now();
        while !guest.state().is_some_and(|st| st.members.len() == 2 && st.private) {
            host.tick(&mut host_net);
            guest.tick(&mut net);
            assert!(t.elapsed() < Duration::from_secs(20), "not joined: {}", String::from_utf8_lossy(&guest.line));
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A room through a rendezvous service on 127.0.0.1 (ADR 0018): the host types the relay and its room opens there;
    /// a joiner whose code names no reachable address finds the host through the service.
    #[test]
    fn a_room_is_joined_through_the_relay() {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let relay = socket.local_addr().unwrap().to_string();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let s2 = stop.clone();
        std::thread::spawn(move || ig2_net::rendezvous::serve(&socket, &mut ig2_net::rendezvous::Service::default(), |_| s2.load(std::sync::atomic::Ordering::Relaxed)));
        let open = |name: &str| {
            let mut s = MpScreen::open(true, name, RULES);
            s.save_dir = None;
            s.relay = Field::new(MAX_ADDRESS);
            s.open = 100.0;
            s
        };
        let (mut host_net, mut net) = (QuicNet::default(), QuicNet::default());
        let mut host = open("Host");
        click_on(&mut host, spot::RELAY, &mut host_net);
        host.paste("no-such-host.invalid");
        host.host(&mut host_net);
        assert!(host.role.is_none() && host.line.starts_with(b"Relay server: "), "a bad relay address stops hosting");
        host.relay = Field::new(MAX_ADDRESS);
        host.paste(&relay);
        host.host(&mut host_net);
        let t = std::time::Instant::now();
        let seen = loop {
            host.tick(&mut host_net);
            if let Some(Role::Host { listener, .. }) = &host.role {
                if let Rendezvous::Registered(a) = listener.rendezvous() {
                    break a;
                }
            }
            assert!(t.elapsed() < Duration::from_secs(10), "no room at the relay");
            std::thread::sleep(Duration::from_millis(5));
        };
        let code = host.join_code().unwrap();
        assert!(code.addrs.contains(&seen), "the code carries the address the relay sees");
        // Only an unreachable documentation address: the service is the way in.
        let only_relay = JoinCode { addrs: vec!["192.0.2.1:9".parse().unwrap()], ..code };
        let mut guest = open("Guest");
        click_on(&mut guest, spot::RELAY, &mut net);
        guest.paste(&relay);
        click_on(&mut guest, spot::ADDRESS, &mut net);
        guest.paste(&only_relay.encode());
        guest.key(Some(EntryKey::Enter), false, &mut net);
        while !guest.state().is_some_and(|st| st.members.len() == 2) {
            host.tick(&mut host_net);
            guest.tick(&mut net);
            assert!(t.elapsed() < Duration::from_secs(30), "not joined: {}", String::from_utf8_lossy(&guest.line));
            std::thread::sleep(Duration::from_millis(5));
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn click_on(s: &mut MpScreen, id: u32, net: &mut dyn Net) -> Option<Out> {
        let p = at(s, id);
        s.press(p, false, net)
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
