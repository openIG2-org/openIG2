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
use ig2_net::rendezvous::{Listing, Rendezvous};
use ig2_net::lobby::{LobbyClient, LobbyHost, LobbyState, Refusal, Settings, SlotKind, Start, Status};
use ig2_net::session::{ClientSession, HoldStatus, HostSession, MpSave, SessionEvent, SessionSettings};
use ig2_net::{Peer, Transport};
use ig2_sim::game::new_game::StartData;
use ig2_sim::game::GameRules;

use crate::options::EntryKey;

pub(crate) mod page;

/// The game port a host listens on first (the LAN announcement carries the real one).
pub const GAME_PORT: u16 = ig2_net::discovery::DISCOVERY_PORT + 1;
/// The relay address's file in the save folder.
const RELAY_FILE: &str = "relay.txt";
/// The project's public rendezvous service (ADR 0018), the relay field's value until the player saves another one
/// (an empty field, saved as an empty `relay.txt`, turns it off).
pub const DEFAULT_RELAY: &str = "ig2relay.justdawee.hu:30000";
/// The longest password typed.
const MAX_FIELD: usize = 40;
/// The longest address typed or pasted (a join code with four addresses and its dashes is 84).
const MAX_ADDRESS: usize = 96;
/// The connection page's rows at most (the list scrolls; their ids stay below `spot::ADDRESS`).
const LIST_ROWS: usize = 32;

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
    /// Ask the router to forward the game port (`on`), or remove the forwarding (ADR 0008 §2; on when a room opens, ADR 0020).
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
    /// Put the room on the service's public list (`None`: take it off; ADR 0060).
    fn publish(&mut self, _l: Option<Listing>) {}
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
/// The rendezvous service's public list (each room's host and listing), from a background thread (ADR 0060).
pub type Browsing = Receiver<Result<Vec<(SocketAddrV4, Listing)>, String>>;

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
    /// Ask the rendezvous service set last for its public list (ADR 0060).
    fn browse(&mut self) -> Browsing {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(Err("no internet list here".into()));
        rx
    }
}

/// The client's links: QUIC natively (and WebRTC for browser joiners); WebRTC through the relay server in the browser
/// (ADR 0047 part 4).
#[cfg(not(target_arch = "wasm32"))]
pub type ClientNet = QuicNet;
#[cfg(target_arch = "wasm32")]
pub type ClientNet = WebNet;

/// QUIC links and LAN discovery (ADR 0008), and the rendezvous service the player set (ADR 0018).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct QuicNet {
    browser: Option<ig2_net::discovery::Browser>,
    queried: Option<web_time::Instant>,
    relay: Option<SocketAddrV4>,
}

#[cfg(not(target_arch = "wasm32"))]
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
    /// Browsers' WebRTC links that are not open yet (ADR 0047 part 4).
    browsers: Vec<ig2_net::webrtc::Link>,
}

#[cfg(not(target_arch = "wasm32"))]
impl QuicListener {
    fn new(host: ig2_net::quic::QuicHost, announcer: Option<ig2_net::discovery::Announcer>) -> Self {
        let lan = ig2_net::portmap::local_ipv4(std::net::Ipv4Addr::new(192, 0, 2, 1)).ok();
        Self { host, announcer, asking: None, mapping: None, map: PortMap::Off, lan, browsers: Vec::new() }
    }
}

/// Every protocol's failure, in the order tried.
#[cfg(not(target_arch = "wasm32"))]
fn map_errors(errors: &[(ig2_net::portmap::Protocol, ig2_net::portmap::Error)]) -> String {
    let name = |p| match p {
        ig2_net::portmap::Protocol::Upnp => "UPnP",
        ig2_net::portmap::Protocol::NatPmp => "NAT-PMP",
    };
    errors.iter().map(|(p, e)| format!("{}: {e}", name(*p))).collect::<Vec<_>>().join("; ")
}

#[cfg(not(target_arch = "wasm32"))]
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
        // Browsers' offers through the relay server: answered at once, admitted once their channel opens.
        for (id, offer) in self.host.offers() {
            match ig2_net::webrtc::answer(&offer) {
                Ok((answer, link)) => {
                    self.host.answer(id, &answer);
                    self.browsers.push(link);
                }
                Err(e) => eprintln!("ig2-client: a browser's offer: {e}"),
            }
        }
        self.browsers.retain(|l| !l.closed());
        if let Some(i) = self.browsers.iter().position(|l| l.is_open()) {
            return Some(Box::new(self.browsers.swap_remove(i)));
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

    fn publish(&mut self, l: Option<Listing>) {
        self.host.publish(l.map(|l| l.encode()));
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

#[cfg(not(target_arch = "wasm32"))]
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
            self.queried = Some(web_time::Instant::now());
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

    fn browse(&mut self) -> Browsing {
        let (tx, rx) = mpsc::channel();
        if let Some(r) = self.relay {
            std::thread::spawn(move || {
                let _ = tx.send(ig2_net::rendezvous::list(r, Duration::from_millis(700)).map_err(|e| format!("Relay server: {e}")));
            });
        }
        rx
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

/// The browser's links (ADR 0047 part 4): a join code through the relay server's WebRTC signalling. A page can't
/// listen, so hosting gets a listener nobody reaches, and there is no LAN to search.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct WebNet;

#[cfg(target_arch = "wasm32")]
impl Net for WebNet {
    fn listen(&mut self) -> Result<Box<dyn Listener>, String> {
        // shortcut: a page can't take connections yet (ADR 0047 part 4); browser hosting needs the relay to signal
        // offers to a page over HTTP. Until then say so instead of opening a room nobody reaches.
        Err("the browser version can't host yet: host from the desktop version and join its code here".into())
    }

    fn sessions(&mut self) -> Vec<Found> {
        Vec::new()
    }

    fn connect(&mut self, _addr: &str, _pin: Option<[u8; 32]>) -> Connecting {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(Err("in the browser, join with a join code".into()));
        rx
    }

    fn connect_code(&mut self, code: &JoinCode) -> Connecting {
        let (tx, rx) = mpsc::channel();
        let (session, pin) = (code.session, code.pin);
        wasm_bindgen_futures::spawn_local(async move {
            // The relay is fixed and named, not resolved: the browser looks the host name up (a DDNS name).
            let r = ig2_net::webrtc::connect(&format!("http://{DEFAULT_RELAY}"), session, &pin).await;
            let _ = tx.send(r.map(|l| Box::new(l) as Box<dyn Transport + Send>).map_err(|e| e.to_string()));
        });
        rx
    }
}

/// No network (the headless script runs): hosting gets a listener nobody reaches, the LAN is empty, joins fail.
#[derive(Default)]
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
    /// `story`: `MultiplayerStartStory` (exe 0xF2148C), the story the start begins (0: none).
    Host { lobby: Box<LobbyHost>, listener: Box<dyn Listener>, save: Option<Box<MpSave>>, story: i32 },
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
    /// The room's mods are installed: start the game again joining this address or join code (ADR 0086 §3).
    Restart(String),
    /// The room-mods dialog's Open Mods: the Mods page instead of these screens.
    Mods,
}

/// The page's item ids (ours; [`Act::Spot`]).
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
    /// ADR 0060: the connection page's internet list switch and its refresh, the host's public-list switch.
    pub const BROWSE: u32 = 0x37;
    pub const REFRESH: u32 = 0x38;
    pub const PUBLIC: u32 = 0x75;
    /// Ours (ADR 0033): a computer slot's level.
    pub const LEVEL: u32 = 0x78;
}

/// The options rows (spec §6).
const OPTIONS: usize = 8;

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
    /// Opened from Internet: the browser lists the public rooms (the relay's list) and asks for it at once; from
    /// Local network it lists the LAN's sessions. Saved games is the other tab of either.
    pub inet: bool,
    /// The public list is to be asked for at the next tick (opened from Internet).
    ask: bool,
    pub name: String,
    rules: [u8; 32],
    /// The enabled mods' fingerprint (zero without mods), to name a mods mismatch (ADR 0044 §4).
    mods: [u8; 32],
    /// Ours (ADR 0069 §13): the mods' races a slot may pick besides 1..=8.
    races: Vec<u8>,
    /// The fade 0..100 (+4 a 10 ms tick).
    pub open: f32,
    pub list: Vec<Found>,
    pub selected: Option<usize>,
    pub address: Field,
    pub password: Field,
    /// The rendezvous service's address (ADR 0018), empty for none; kept in the save folder's `relay.txt`
    /// ([`DEFAULT_RELAY`] without one).
    pub relay: Field,
    /// The connection page lists the service's public rooms (ADR 0060) instead of the LAN's sessions.
    pub internet: Option<Vec<(SocketAddrV4, Listing)>>,
    /// The public list being fetched.
    browsing: Option<Browsing>,
    /// The host puts its room on the service's public list (off by default; a private room never is).
    pub public: bool,
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
    /// The password typed for the room this machine joined (kept for a host migration's door).
    joined_password: Option<String>,
    /// The connection page lists the save folder's MP saves instead of the LAN's sessions.
    pub saves: Option<Vec<SavedGame>>,
    /// Where the saves are (`options::slot_dir`).
    pub save_dir: Option<std::path::PathBuf>,
    /// The host's room resumes this save; the members already seated in their saved empires.
    resume: Option<(Box<MpSave>, Vec<u8>)>,
    /// `MultiplayerStartStory` (exe 0xF2148C): set by the room's `story N` / `s N` line; only the host's start sends
    /// it. Starts at the config line `mpstory` ([`MpScreen::with_story`]).
    pub story: i32,
    /// The race heads on the room's player rows (ADR 0041), race 1 first; loaded by [`MpScreen::load_heads`].
    heads: Vec<Option<egui::TextureHandle>>,
    /// A custom game seat's first values (ADR 0085 §4): the skirmish menu's step 2 money and technology 2, from the
    /// install's tables ([`MpScreen::load_heads`]); `None`: no custom game is offered.
    custom_seat: Option<ig2_net::galaxy::Seat>,
    /// The host's Start found someone not ready: the next press starts anyway (ADR 0041).
    force: bool,
    /// Up or Down moved the list's selection: the page scrolls it into view.
    reveal: std::cell::Cell<bool>,
    /// This player's enabled mods (ADR 0086 §3): sent as a host's to joiners refused for other mods, compared as a
    /// joiner's with the room's.
    mod_list: Vec<ig2_net::lobby::RoomMod>,
    /// What was last joined (an address or a join code): joined again after "Download and join"'s restart.
    target: String,
    /// Refused for other mods: the room's mods against ours, and "Download and join" ([`crate::room_mods`]).
    pub room_mods: Option<crate::room_mods::RoomMods>,
}

/// `DoKeyboard`'s room line (exe 0x4db1a0, ~0x4db7bc): `story N` or `s N` (case-insensitive) with `atoi(N) != 0`.
fn story_line(text: &str) -> Option<i32> {
    let lower = text.to_ascii_lowercase();
    let rest = lower.strip_prefix("story ").or_else(|| lower.strip_prefix("s "))?;
    Some(ig2_formats::dialogue::atoi(rest)).filter(|&n| n != 0)
}

/// Our English words where the original has none (ADR 0014 §4).
mod words {
    pub const HOST: &[u8] = b"Host game";
    pub const PASSWORD: &[u8] = b"Password (optional)";
    pub const NONE_FOUND: &[u8] = b"No games found on the local network";
    pub const INET: &[u8] = b"No internet matchmaking: join by address or join code";
    pub const CONNECTING: &[u8] = b"Connecting...";
    pub const COMPUTER: &[u8] = b"computer";
    pub const CLOSED: &[u8] = b"closed";
    pub const SHAPE: &[u8] = b"Galaxy shape:";
    pub const BATTLES: &[u8] = b"Battles:";
    pub const BATTLE: [&[u8]; 4] = [b"real time", b"space real time", b"ground real time", b"automatic"];
    pub const SPEED: &[u8] = b"Speed control:";
    pub const SPEED_BY: [&[u8]; 2] = [b"everyone", b"host"];
    pub const FORMATION: &[u8] = b"Space battle formations:";
    pub const FORMATION_BY: [&[u8]; 2] = [b"off (classic)", b"every player picks"];
    pub const SPECTATORS: &[u8] = b"Defeated players:";
    pub const SPECTATORS_BY: [&[u8]; 2] = [b"leave (classic)", b"stay and watch"];
    pub const AI: &[u8] = b"Computer empires:";
    pub const AI_BY: [&[u8]; 2] = [b"classic", b"enhanced"];
    pub const NEED_TWO: &[u8] = b"Two empires are needed to start";
    pub const WAITING: &[u8] = b"Not everyone is ready: press again to start anyway";
    pub const START_ANYWAY: &[u8] = b"Start anyway";
    pub const NOT_READY: &[u8] = b"Not ready";
    pub const HOST_MARK: &[u8] = b"Host";
    pub const SAVED: &[u8] = b"Saved game";
    pub const LAN: &[u8] = b"LAN games";
    pub const NO_SAVES: &[u8] = b"No multiplayer saves";
    pub const NEED_SAVED: &[u8] = b"Pick one of the saved empires";
    pub const MAP_ASKING: &[u8] = b"Port mapping: asking the router...";
    pub const NO_CODE: &[u8] = b"Join code: none (no network address)";
    pub const COPIED: &[u8] = b"The join code is on the clipboard";
    pub const ADDRESS: &[u8] = b"Address or join code:";
    pub const REFRESH: &[u8] = b"Refresh";
    pub const ASKING: &[u8] = b"Asking the relay server...";
    pub const NO_PUBLIC: &[u8] = b"No public games on the relay server";
    pub const NEED_RELAY: &[u8] = b"Type a relay server to browse internet games";
    // The designed pages (ADR 0064).
    pub const PLAYING_AS: &[u8] = b"Playing as";
    pub const INTERNET: &[u8] = b"Internet games";
    pub const SAVED_GAMES: &[u8] = b"Saved games";
    pub const JOIN_DIRECTLY: &[u8] = b"Join directly";
    pub const RELAY_TITLE: &[u8] = b"Relay server";
    pub const RELAY_ADDRESS: &[u8] = b"Address";
    pub const RELAY_HINT: &[u8] = b"Optional. For internet games: it lists public games, helps players behind a router reach each other and relays when nothing else works.";
    pub const NONE_FOUND_HINT: &[u8] = b"Games hosted on this network show here as they open.";
    pub const NO_PUBLIC_HINT: &[u8] = b"Host one yourself, or press Refresh to ask again.";
    pub const NO_SAVES_HINT: &[u8] = b"A multiplayer game the host saved can be resumed here.";
    pub const HOST_SAVED: &[u8] = b"Host saved game";
    pub const STARTED: &[u8] = b"Started";
    pub const INCOMPATIBLE: &[u8] = b"Incompatible";
    pub const LOCKED: &[u8] = b"Password";
    pub const JOINED: &[u8] = b"Joined";
    pub const PRIVATE_BADGE: &[u8] = b"Private";
    pub const READY_COUNT: &[u8] = b"Ready";
    pub const PLAYERS: &[u8] = b"Players";
    pub const CONNECTION: &[u8] = b"Connection";
    pub const MAP_LABEL: &[u8] = b"Port mapping (UPnP / NAT-PMP)";
    pub const PRIVATE_LABEL: &[u8] = b"Private game";
    pub const PRIVATE_HINT: &[u8] = b"Joined with the join code only";
    pub const PUBLIC_LABEL: &[u8] = b"Public list";
    pub const COPY: &[u8] = b"Copy join code";
    pub const COPY_HINT: &[u8] = b"Send it to the players: they paste it into Join directly (Ctrl+V).";
    pub const OPEN_SEAT: &[u8] = b"Open";
    // Ours (ADR 0080).
    pub const DISCORD_JOINING: &[u8] = b"Joining through Discord...";
    pub const DISCORD_COMING: &[u8] = b"is joining through Discord";
    // Ours (ADR 0086 §3): the room-mods dialog.
    pub const MODS_TITLE: &[u8] = b"This game uses other mods";
    pub const MODS_MISSING: &[u8] = b"Missing:";
    pub const MODS_OTHER: &[u8] = b"Another version (yours / the game's):";
    pub const MODS_EXTRA: &[u8] = b"Not in this game, switch them off in Mods:";
    pub const MODS_UNAVAILABLE: &[u8] = b"Not in the mod index, install them by hand:";
    pub const MODS_ORDER: &[u8] = b"The same mods in another order.";
    pub const MODS_FILES: &[u8] = b"The same mods with other files: compare them in Mods.";
    pub const MODS_CHECKING: &[u8] = b"Looking them up in the mod index (github.com/openIG2-org/mods)...";
    pub const MODS_NO_INDEX: &[u8] = b"The mod index didn't load:";
    pub const MODS_DOWNLOADING: &[u8] = b"Downloading the game's mods...";
    pub const MODS_RESTARTS: &[u8] = b"The game then starts again with these mods and joins.";
    pub const MODS_JOIN: &[u8] = b"Download and join";
    pub const MODS_RESTART: &[u8] = b"Restart and join";
    pub const MODS_OPEN: &[u8] = b"Open Mods";
    pub const CANCEL_WORD: &[u8] = b"Cancel";
    pub const YOU: &[u8] = b"You";
    pub const TAKE_SEAT: &[u8] = b"Take seat";
    pub const CHAT: &[u8] = b"Chat";
    pub const CHAT_HINT: &[u8] = b"Type a message; Enter sends it";

    /// A word in the game's language, as text.
    pub fn text(lang: ig2_formats::text::Lang, w: &[u8]) -> String {
        crate::tr::text(lang, std::str::from_utf8(w).unwrap_or_default()).to_string()
    }
}

/// The relay field's start: the saved `relay.txt`, even an empty one (the player turned the service off), else the
/// default.
fn saved_relay(file: std::io::Result<String>, default: &str) -> String {
    file.unwrap_or_else(|_| default.into())
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
        Refusal::Mods => b"Other mods enabled (mods differ; see Mods)",
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
    /// The enabled mods' fingerprint (`session::mods_fingerprint`): rooms and hellos carry it (ADR 0044 §4).
    pub fn with_mods(mut self, mods: Option<[u8; 32]>) -> Self {
        self.mods = mods.unwrap_or_default();
        self
    }

    /// The enabled mods (`room_mods::list`, ADR 0086 §3).
    pub fn with_mod_list(mut self, mods: Vec<ig2_net::lobby::RoomMod>) -> Self {
        self.mod_list = mods;
        self
    }

    /// The mods' races a slot may pick besides 1..=8 (ADR 0069 §13).
    pub fn with_races(mut self, races: Vec<u8>) -> Self {
        self.races = races;
        self
    }

    /// `Start_Belep` (exe 0x5bf4b0): `MultiplayerStartStory = atoi(GetParam("mpstory", "0"))`, shown as a chat line
    /// when non-zero.
    pub fn with_story(mut self, story: i32) -> Self {
        self.story = story;
        if story != 0 {
            self.chat.push((String::new(), format!("Starting story set to {story}")));
        }
        self
    }

    /// The connection page (`name` = `MultiplayerNev`, `rules` = the rules fingerprint joiners must match).
    pub fn open(inet: bool, name: &str, rules: [u8; 32]) -> Self {
        // The browser build has no LAN: every page is the internet one.
        let inet = inet || cfg!(target_arch = "wasm32");
        Self {
            inet,
            ask: inet,
            name: name.to_string(),
            rules,
            mods: [0; 32],
            races: Vec::new(),
            open: 0.0,
            list: Vec::new(),
            selected: None,
            address: Field::new(MAX_ADDRESS),
            password: Field::new(MAX_FIELD),
            relay: Field::new(MAX_ADDRESS),
            internet: inet.then(Vec::new),
            browsing: None,
            public: false,
            focus: Focus::Address,
            line: Vec::new(),
            lost: false,
            connecting: None,
            role: None,
            chat: Vec::new(),
            entry: Field::new(ig2_net::lobby::MAX_CHAT),
            joined_password: None,
            saves: None,
            // The unit tests never read the player's own files (a saved relay changes the screens' flow).
            save_dir: if cfg!(test) { None } else { crate::options::slot_dir() },
            resume: None,
            story: 0,
            heads: Vec::new(),
            custom_seat: None,
            force: false,
            reveal: Default::default(),
            mod_list: Vec::new(),
            target: String::new(),
            room_mods: None,
        }
        .with_saved_relay()
    }

    fn with_saved_relay(mut self) -> Self {
        let Some(d) = &self.save_dir else { return self };
        // The unit tests never reach the public service; the browser build's relay is fixed.
        let saved = if cfg!(target_arch = "wasm32") { DEFAULT_RELAY.to_owned() } else { saved_relay(ig2_formats::userfs::read_to_string(d.join(RELAY_FILE)), if cfg!(test) { "" } else { DEFAULT_RELAY }) };
        for c in saved.trim().bytes().filter(|c| (0x20..0x7f).contains(c)) {
            self.relay.key(EntryKey::Char(c));
        }
        self
    }

    /// The relay field to `net` (and to `relay.txt` when it changed); false, with the reason on the status line,
    /// when it names no address.
    fn apply_relay(&mut self, net: &mut dyn Net) -> bool {
        // The browser build's relay is fixed in `WebNet` (a page can't resolve a host name here).
        if cfg!(target_arch = "wasm32") {
            return true;
        }
        let typed = self.relay.string();
        match relay_addr(&typed) {
            Ok(a) => {
                net.set_relay(a);
                if let Some(d) = &self.save_dir {
                    let file = d.join(RELAY_FILE);
                    if ig2_formats::userfs::read_to_string(&file).unwrap_or_default().trim() != typed.trim() {
                        let _ = ig2_formats::userfs::write(file, typed.trim());
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
                let lobby = LobbyHost::new_galaxy(settings, ig2_net::discovery::random_u64(), self.rules, &self.name).with_mods(self.mods).with_mod_list(self.mod_list.clone()).with_races(self.races.clone());
                listener.register(lobby.session());
                listener.map_port(true);
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
                let mut lobby = LobbyHost::new(settings, ig2_net::discovery::random_u64(), self.rules, &slots, &self.name).with_mods(self.mods).with_mod_list(self.mod_list.clone()).with_races(self.races.clone());
                let mine = s.humans.iter().find(|h| h.name == lobby.state().members[0].name).or(s.humans.first()).map(|h| h.empire);
                lobby.pick(0, mine);
                listener.register(lobby.session());
                listener.map_port(true);
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

    /// The connection page's list: the LAN's sessions, the service's public rooms or the MP saves.
    fn rows(&self) -> usize {
        match (&self.saves, &self.internet) {
            (Some(s), _) => s.len(),
            (None, Some(i)) => i.len(),
            (None, None) => self.list.len(),
        }
    }

    /// Ask the relay server for its public list (ADR 0060); the rows stay until the answer.
    pub fn refresh(&mut self, net: &mut dyn Net) {
        if !self.apply_relay(net) {
            return;
        }
        if !cfg!(target_arch = "wasm32") && relay_addr(&self.relay.string()).ok().flatten().is_none() {
            self.line = words::NEED_RELAY.to_vec();
            self.focus = Focus::Relay;
            return;
        }
        self.browsing = Some(net.browse());
        self.line = words::ASKING.to_vec();
    }

    /// Join the selected LAN session or public room, else the typed or pasted join code or address. A public room is
    /// joined by its join code (put in the address field): its address, and the relay's hole punch and relay.
    pub fn join(&mut self, net: &mut dyn Net) {
        if let Some(code) = self.selected.and_then(|i| self.internet.as_ref()?.get(i)).map(|(host, l)| l.join_code(*host).encode()) {
            self.address = Field::new(MAX_ADDRESS);
            for c in code.bytes() {
                self.address.key(EntryKey::Char(c));
            }
            self.selected = None;
        }
        let typed = self.address.string();
        self.target = typed.clone();
        let (rx, code) = match self.selected.and_then(|i| self.list.get(i)).filter(|_| self.internet.is_none()) {
            Some(f) => {
                self.target = f.addr.to_string();
                (net.connect(&f.addr.to_string(), Some(f.announcement.fingerprint)), None)
            }
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
        self.role = Some(Role::Joiner(Box::new(LobbyClient::join_with_mods(link, &self.name, self.rules, self.mods, password, code))));
        self.line.clear();
        self.lost = false;
    }

    /// Ours (ADR 0080): join the room of a join code a friend's Discord activity gave (their Join pressed here): the
    /// code in the address field, joined as typed (a password room asks for it as after a typed code). Nothing while
    /// in a room or connecting.
    pub fn join_from_discord(&mut self, code: &str, net: &mut dyn Net) {
        self.join_typed(code, net);
        if self.connecting.is_some() {
            self.line = words::DISCORD_JOINING.to_vec();
        }
    }

    /// `code` (a join code or an address) typed into the address field and joined (Discord's Join, `--mp-join`).
    pub fn join_typed(&mut self, code: &str, net: &mut dyn Net) {
        if self.role.is_some() || self.connecting.is_some() || self.room_mods.is_some() {
            return;
        }
        self.saves = None;
        self.internet = None;
        self.selected = None;
        self.focus = Focus::Address;
        self.address = Field::new(MAX_ADDRESS);
        for c in code.trim().bytes().filter(|c| (0x20..0x7f).contains(c)) {
            self.address.key(EntryKey::Char(c));
        }
        self.join(net);
    }

    /// Ours (ADR 0080): the room as the Discord activity shows it (none on the connection page).
    pub fn discord_room(&self) -> Option<crate::discord::Room> {
        let st = self.state()?;
        let me = self.member()?;
        let seated = st.slots.iter().filter(|s| matches!(s.kind, SlotKind::Human(_))).count();
        let open = st.slots.iter().filter(|s| s.kind == SlotKind::Open).count();
        let humans = st.members.len().max(seated) as u32;
        let host = st.members.iter().find(|m| m.id == 0).map_or("", |m| m.name.as_str());
        Some(crate::discord::Room {
            name: st.name.clone(),
            host: self.is_host(),
            humans,
            seats: (seated + open).max(humans as usize) as u32,
            race: st.slots.iter().find(|s| s.kind == SlotKind::Human(me)).map(|s| s.race).filter(|&r| r != 0),
            ready: st.members.iter().any(|m| m.id == me && m.ready),
            private: st.private,
            saved: self.resume.is_some(),
            party: crate::discord::party_id(&[&st.name, host]),
            code: self.join_code().map(|c| c.encode()),
        })
    }

    /// Ours (ADR 0080): a friend's join request accepted on Discord, told in the room's chat.
    pub fn discord_joining(&mut self, name: &str) {
        self.chat.push((String::new(), format!("{name} {}", words::text(crate::tr::game(), words::DISCORD_COMING))));
    }

    /// The room-mods dialog's action (`yes`) or Cancel (not while it installs).
    pub fn room_mods_answer(&mut self, yes: bool) -> Option<Out> {
        use crate::room_mods::Offer;
        let r = self.room_mods.as_mut()?;
        match (yes, r.offer()) {
            (false, Offer::Wait) if r.installing() => None,
            (false, _) => {
                self.room_mods = None;
                Some(Out::Click)
            }
            (true, Offer::Join(_)) => {
                let dir = ig2_formats::vfs::Mod::exe_folder().or_else(ig2_formats::vfs::Mod::folder)?;
                r.start(dir, ig2_formats::vfs::Mod::folders(), ig2_formats::vfs::Mod::enabled_file(), Box::new(crate::mod_repo::get));
                Some(Out::Click)
            }
            (true, Offer::Mods) => {
                self.room_mods = None;
                Some(Out::Mods)
            }
            (true, Offer::Wait) => None,
        }
    }

    /// Leave the room for the connection page.
    fn leave(&mut self) {
        match self.role.take() {
            Some(Role::Joiner(c)) => (*c).leave(),
            Some(Role::Host { mut listener, .. }) => {
                listener.announce(None);
                listener.publish(None);
            }
            None => {}
        }
    }

    /// One 10 ms tick: the fade, the LAN list, the connection attempt, the room's messages. A started room comes
    /// back as [`Out::Start`].
    pub fn tick(&mut self, net: &mut dyn Net) -> Option<Out> {
        self.open = (self.open + 4.0).min(100.0);
        // The list asked for on opening: without a relay it says so, but the typing stays in the join field.
        if std::mem::take(&mut self.ask) {
            let focus = self.focus;
            self.refresh(net);
            self.focus = focus;
        }
        if let Some(rx) = &self.browsing {
            match rx.try_recv() {
                Ok(Ok(mut rooms)) => {
                    rooms.sort_by(|a, b| a.1.announcement.name.cmp(&b.1.announcement.name));
                    self.line = if rooms.is_empty() { words::NO_PUBLIC.to_vec() } else { Vec::new() };
                    self.internet = Some(rooms);
                    self.browsing = None;
                    self.selected = None;
                }
                Ok(Err(e)) => {
                    self.line = e.into_bytes();
                    self.browsing = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.browsing = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.role.is_none() {
            self.list = net.sessions();
            self.list.sort_by(|a, b| a.announcement.name.cmp(&b.announcement.name));
            if self.selected.is_some_and(|i| i >= self.rows()) {
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
        if self.room_mods.as_mut().is_some_and(|r| r.poll()) {
            return Some(Out::Restart(self.target.clone()));
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
                listener.publish(a.clone().filter(|_| self.public).map(|announcement| Listing { announcement, mods: self.mods }));
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
                        let room = match self.role.take() {
                            Some(Role::Joiner(c)) => c.host_mods,
                            _ => None,
                        };
                        self.line = refusal_text(r).to_vec();
                        if let (Refusal::Mods, Some(room)) = (r, room) {
                            // The unit tests never reach the public index.
                            let fetch = || if cfg!(test) { Err("no mod index in tests".to_string()) } else { crate::mod_repo::load().map(|(i, _)| i) };
                            self.room_mods = Some(crate::room_mods::RoomMods::new(room, &self.mod_list, fetch));
                        }
                        None
                    }
                    _ => None,
                }
            }
        }
    }

    // -------------------------------------------------------------------------------------------- input

    /// The page's press on item `id` (`right`: the right button's; the page's acts, [`page::Act::Spot`]).
    pub fn act(&mut self, id: u32, right: bool, net: &mut dyn Net) -> Option<Out> {
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
                spot::BROWSE => {
                    self.selected = None;
                    if self.internet.take().is_none() {
                        self.internet = Some(Vec::new());
                        self.refresh(net);
                    } else {
                        self.browsing = None;
                        self.line.clear();
                    }
                }
                spot::REFRESH => self.refresh(net),
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
        let slot = |i: u32| st.slots.get(i as usize).copied();
        let force = std::mem::take(&mut self.force);
        match id {
            spot::LEAVE => self.leave(),
            spot::PRIVATE => {
                let Some(Role::Host { lobby, .. }) = self.role.as_mut() else { return None };
                let private = !lobby.state().private;
                lobby.set_private(private);
            }
            spot::CODE => return self.copy(),
            spot::PUBLIC => self.public = !self.public,
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
                        if force {
                            lobby.ready_all();
                        }
                        // A saved empire nobody holds goes to the computer (ADR 0022); others than the saved can't play.
                        let saved = self.resume.as_ref().map(|(s, _)| {
                            st.members.iter().filter_map(|m| st.empire_of(m.id)).all(|e| s.session.humans.iter().any(|h| h.empire == e))
                        });
                        if saved == Some(false) {
                            self.line = words::NEED_SAVED.to_vec();
                            return Some(Out::Click);
                        }
                        if lobby.can_start() {
                            let Some(Role::Host { lobby, mut listener }) = self.role.take() else { unreachable!() };
                            listener.announce(None);
                            listener.publish(None);
                            let save = self.resume.take().map(|r| r.0);
                            return Some(Out::Start(Begun::Host { lobby, listener, save, story: self.story }));
                        }
                        let two = st.members.len() + st.ais().len() >= 2;
                        self.force = two;
                        self.line = if two { words::WAITING } else { words::NEED_TWO }.to_vec();
                    }
                    Role::Joiner(c) => c.ready(!ready),
                }
            }
            i if (spot::RACE..spot::RACE + 8).contains(&i) => {
                let s = slot(i - spot::RACE)?;
                let all: Vec<u8> = (1..=8).chain(self.races.iter().copied()).collect();
                let at = all.iter().position(|&r| r == s.race).unwrap_or(0);
                return self.set_race((i - spot::RACE) as usize, all[if right { at + all.len() - 1 } else { at + 1 } % all.len()]);

            }
            i if (spot::HOLDER..spot::HOLDER + 8).contains(&i) => {
                let s = slot(i - spot::HOLDER)?;
                match (self.role.as_mut()?, right) {
                    // The host's right click: empty → AI → closed → empty.
                    (Role::Host { .. }, true) => {
                        let next = match s.kind {
                            SlotKind::Open => SlotKind::Ai,
                            SlotKind::Ai => SlotKind::Closed,
                            SlotKind::Closed => SlotKind::Open,
                            SlotKind::Human(_) => return None,
                        };
                        return self.set_slot((i - spot::HOLDER) as usize, next);
                    }
                    (Role::Host { lobby, .. }, false) => {
                        lobby.pick(0, Some(s.empire));
                    }
                    (Role::Joiner(c), false) if matches!(s.kind, SlotKind::Open | SlotKind::Ai) => c.pick(Some(s.empire)),
                    _ => return None,
                }
            }
            // The host's click: the next level (right: the previous), easy → normal → hard → easy.
            i if (spot::LEVEL..spot::LEVEL + 8).contains(&i) => {
                let k = (slot(i - spot::LEVEL)?.empire as usize).clamp(1, 8) - 1;
                let level = st.galaxy?.difficulty[k];
                return self.set_level((i - spot::LEVEL) as usize, if right { (level + 1) % 3 + 1 } else { level % 3 + 1 });
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
                    4 => GalaxyOptions { host_time: up, ..o },
                    5 => GalaxyOptions { formation: up, ..o },
                    6 => GalaxyOptions { spectators: up, ..o },
                    _ => GalaxyOptions { enhanced_ai: up, ..o },
                };
                lobby.set_options(n);
            }
            _ => return None,
        }
        Some(Out::Click)
    }

    /// Slot `i`'s race (the host's any slot, a joiner's its own).
    fn set_race(&mut self, i: usize, race: u8) -> Option<Out> {
        let st = self.state()?;
        let s = *st.slots.get(i)?;
        if !self.is_host() && st.empire_of(self.member()?) != Some(s.empire) {
            return None;
        }
        match self.role.as_mut()? {
            Role::Host { lobby, .. } => {
                lobby.set_race(0, s.empire, race);
            }
            Role::Joiner(c) => c.race(s.empire, race),
        }
        Some(Out::Click)
    }

    /// The host opens, closes or gives slot `i` to the computer (not a human's).
    fn set_slot(&mut self, i: usize, kind: SlotKind) -> Option<Out> {
        let s = *self.state()?.slots.get(i)?;
        let Some(Role::Host { lobby, .. }) = self.role.as_mut() else { return None };
        if matches!(s.kind, SlotKind::Human(_)) || matches!(kind, SlotKind::Human(_)) {
            return None;
        }
        lobby.set_slot(s.empire, kind);
        Some(Out::Click)
    }

    /// The host's galaxy options as the page set them (ADR 0085 §4's custom game).
    fn set_options(&mut self, o: GalaxyOptions) -> Option<Out> {
        let Some(Role::Host { lobby, .. }) = self.role.as_mut() else { return None };
        lobby.set_options(o).then_some(Out::Click)
    }

    /// The host sets the computer's level in slot `i` (1..3, ADR 0033).
    fn set_level(&mut self, i: usize, level: u8) -> Option<Out> {
        let k = (self.state()?.slots.get(i)?.empire as usize).clamp(1, 8) - 1;
        let Some(Role::Host { lobby, .. }) = self.role.as_mut() else { return None };
        let mut o = lobby.state().galaxy?;
        o.difficulty[k] = level.clamp(1, 3);
        lobby.set_options(o);
        Some(Out::Click)
    }

    /// Tab: the connection page's next field.
    pub fn next_field(&mut self) {
        if self.role.is_none() {
            self.focus = match self.focus {
                Focus::Address => Focus::Password,
                // The browser build shows no relay field.
                Focus::Password if cfg!(target_arch = "wasm32") => Focus::Address,
                Focus::Password => Focus::Relay,
                Focus::Relay => Focus::Address,
            };
        }
    }

    /// Up or down: the connection page's list selection moves (Enter joins it, or hosts a save).
    pub fn step_list(&mut self, down: bool) {
        let n = self.rows().min(LIST_ROWS);
        if self.role.is_some() || n == 0 {
            return;
        }
        self.reveal.set(true);
        self.selected = Some(match self.selected {
            None if down => 0,
            None => n - 1,
            Some(k) if down => (k + 1).min(n - 1),
            Some(k) => k.saturating_sub(1),
        });
    }

    /// A key: the focused field's (the connection page) or the chat line's (the room); Esc leaves the room, then the
    /// screens.
    pub fn key(&mut self, k: Option<EntryKey>, escape: bool, net: &mut dyn Net) -> Option<Out> {
        // The room-mods dialog takes the keys: Esc its Cancel, Enter its action.
        if self.room_mods.is_some() {
            return match (escape, k) {
                (true, _) => self.room_mods_answer(false),
                (_, Some(EntryKey::Enter)) => self.room_mods_answer(true),
                _ => None,
            };
        }
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
                if let Some(n) = story_line(&text) {
                    // Shown on this machine only, not sent (`Start_NewChatText(.., 2)`).
                    self.story = n;
                    self.chat.push((String::new(), format!("Starting story set to {n}")));
                } else if !text.trim().is_empty() {
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
        let typed = matches!(k, EntryKey::Char(_));
        if !f.key(k) {
            if typed && self.focus == Focus::Address {
                self.selected = None;
            }
            return None;
        }
        match (self.selected, self.saves.is_some()) {
            (Some(i), true) => self.host_save(i, net),
            (Some(_), false) => self.join(net),
            (None, true) => {}
            (None, false) => self.join(net),
        }
        None
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

    /// A joiner's resyncs from the host's state so far (the host's own come as `SessionEvent::Resynced`).
    pub fn resyncs(&self) -> usize {
        match self {
            Session::Client(c, _) => c.resyncs,
            _ => 0,
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
        // Empire 0: a spectator's chat (ADR 0026).
        .unwrap_or_else(|| if empire == 0 { "Spectator".into() } else { format!("Empire {empire}") })
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
        Begun::Host { lobby, listener, save: Some(save), .. } => {
            let s = HostSession::resume(*lobby, &save, rules).map_err(|_| String::from_utf8_lossy(words::NEED_SAVED).into_owned())?;
            Ok(Session::Host { session: Box::new(s), listener })
        }
        Begun::Host { lobby, listener, save: None, story } => {
            // `FRISSIT_Start` (exe 0x5bc590): after `InitNewGame`, `Multi_Send(0x65, EN, story)` → `StartStory(id, 0)`.
            let s = HostSession::start_galaxy(*lobby, data, rules, SessionSettings::default(), ticks, story).map_err(|_| "the room can't start yet".to_string())?;
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

/// `ig2-client --mp-host`: a host without a window to try other clients against (a browser's join, ADR 0047 part 4).
/// It opens the connection page's room through the relay server of `relay.txt` (the save folder), prints the join
/// code once the room is open, starts as soon as every joiner is ready, then runs the game on the wall clock (its own
/// empire idle) and prints the lockstep turn every ten seconds, and any desync, until the joiners leave.
#[cfg(not(target_arch = "wasm32"))]
pub fn headless_host() -> Result<(), String> {
    // The window's install and language: the start's names are in it, and every machine builds the same galaxy.
    let install = crate::open_install(&crate::parse(&[])?)?;
    let (rules, _) = crate::rules::load(&install)?;
    let exe = install.read_raw("ig2.exe").map_err(|e| e.to_string())?;
    let mods = crate::mods_fingerprint(&install);
    let fingerprint = ig2_net::session::modded_fingerprint(ig2_net::session::rules_fingerprint(&exe), mods);
    let mut net = QuicNet::default();
    let mut s = MpScreen::open(true, "Host", fingerprint).with_mods(mods).with_mod_list(crate::room_mods::list(install.mods()));
    s.open = 100.0;
    s.host(&mut net);
    let mut printed = false;
    let begun = loop {
        s.tick(&mut net);
        let Some(Role::Host { listener, .. }) = &s.role else { return Err(String::from_utf8_lossy(&s.line).into_owned()) };
        if !printed && matches!(listener.rendezvous(), Rendezvous::Registered(_)) {
            printed = true;
            println!("join code: {}", s.join_code().map(|c| c.encode()).unwrap_or_default());
        }
        let ready = s.state().is_some_and(|st| st.members.len() >= 2 && st.members.iter().filter(|m| m.id != 0).all(|m| m.ready));
        if ready {
            if let Some(Out::Start(b)) = s.act(spot::READY, false, &mut net) {
                break b;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let texts = crate::new_game::StartTexts::read(&install)?;
    let session = texts.with(&install, |data| launch(begun, data, &rules, crate::new_game::wall_ticks(), Box::new(QuicNet::default())))??;
    println!("started");
    let mut w = crate::world::World::new_mp(session, &rules);
    // As the window's start (`start_world`): the install's design names and the empire names.
    w.game.design_texts = crate::new_game::design_texts(&install);
    w.game.empire_names = crate::ui::empire_names(&crate::ui::names_txt17(&install, false));
    let (start, mut shown) = (web_time::Instant::now(), 0);
    let mut desyncs = 0;
    while !w.net_over {
        w.tick();
        let Some((net, _)) = &w.net else { break };
        let found = match net {
            Session::Host { session, .. } => session.host.desyncs.len(),
            _ => 0,
        } + net.peer().desyncs.len();
        if found > desyncs {
            desyncs = found;
            println!("desync at turn {}", net.peer().turn);
        }
        let secs = start.elapsed().as_secs();
        if secs >= shown + 10 {
            shown = secs;
            println!("turn {}, {} hashes compared, {desyncs} desyncs", net.peer().turn, net.peer().hashes.len());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    println!("the session ended");
    Ok(())
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

    fn click(s: &mut MpScreen, id: u32, right: bool, net: &mut LoopNet) -> Option<Out> {
        s.act(id, right, net)
    }

    /// Tick every screen until `done` (2 s at most); returns the starts that came out.
    fn run(screens: &mut [&mut MpScreen], net: &mut LoopNet, done: impl Fn(&[&mut MpScreen]) -> bool) -> Vec<(usize, Begun)> {
        let t = web_time::Instant::now();
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
        let t = web_time::Instant::now();
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
            // `DoKeyboard`'s room line (exe 0x4db1a0): the host's `story N` picks the start story, shown here only; a
            // joiner's sets its own (never sent: only the host's `FRISSIT_Start` sends 0x65); `story x` is chat.
            if joiners == 1 {
                type_text(&mut host, "Story 1003", &mut net);
                type_text(&mut guests[0], "s 1001", &mut net);
                type_text(&mut host, "story x", &mut net);
                assert_eq!((host.story, guests[0].story), (1003, 1001));
                assert_eq!(host.chat[host.chat.len() - 2], (String::new(), "Starting story set to 1003".to_string()));
                assert!(host.chat.last().is_some_and(|c| c.1 == "story x"), "not a story number: a chat line");
            }
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
            let t = web_time::Instant::now();
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
            if joiners == 1 {
                for w in &worlds {
                    let st = w.game.state.story();
                    assert!(st.is_running(1003) && !st.is_running(1001), "the host's start story runs on every machine");
                }
            }
            let stand_in = &worlds[0].net.as_ref().unwrap().0.peer().game;
            assert!(stand_in.state.fleets.is_empty(), "the peer holds the stand-in between pumps");
        }
    }

    /// ADR 0033: the host clicks a computer slot's level (left: up, right: down); the joiner sees it but can't change
    /// it, and both machines' games give that computer the level's `Nehezseg`.
    #[test]
    fn the_host_sets_a_computer_level() {
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(1, &mut net);
        click(&mut host, spot::OPTION + 3, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().galaxy.is_some_and(|g| g.ais == 1));
        let i = host.state().unwrap().slots.iter().position(|s| s.kind == SlotKind::Ai).expect("an AI slot") as u32;
        let e = host.state().unwrap().slots[i as usize].empire;
        assert!(click(&mut guests[0], spot::LEVEL + i, false, &mut net).is_none(), "a joiner sets no level");
        for right in [false, false, true] {
            assert!(click(&mut host, spot::LEVEL + i, right, &mut net).is_some());
        }
        click(&mut guests[0], spot::READY, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready));
        assert_eq!(guests[0].state().unwrap().galaxy.unwrap().difficulty[e as usize - 1], 2, "easy → normal → hard → normal");
        let Some(Ok(inst)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let rules: &'static GameRules = Box::leak(Box::new(crate::rules::load(&inst).unwrap().0));
        let texts = crate::new_game::StartTexts::read(&inst).unwrap();
        let Some(Out::Start(b)) = click(&mut host, spot::READY, false, &mut net) else { panic!("no start") };
        let hub = net.clone();
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 4242, Box::new(hub.clone()))).unwrap().map(|s| crate::world::World::new_mp(s, rules)).unwrap();
        let mut worlds = vec![mk(b)];
        worlds.extend(started(&mut guests, &mut net).into_iter().map(mk));
        for w in &worlds {
            assert_eq!(w.game.state.difficulties()[e as usize], 2, "empire {e}'s level on every machine");
        }
    }

    /// ADR 0041: the host's Start with a joiner not ready waits and arms "Start anyway"; another press disarms it, and
    /// the armed press readies everyone and starts (the launch itself: the tests above).
    #[test]
    fn the_host_starts_anyway() {
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(2, &mut net);
        click(&mut guests[0], spot::READY, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members[1].ready);
        assert!(matches!(click(&mut host, spot::READY, false, &mut net), Some(Out::Click)), "Guest1 isn't ready");
        assert_eq!(host.line, words::WAITING);
        assert!(host.force);
        click(&mut host, spot::PRIVATE, false, &mut net);
        assert!(!host.force, "another press disarms it");
        click(&mut host, spot::PRIVATE, false, &mut net);
        click(&mut host, spot::READY, false, &mut net);
        let Some(Out::Start(Begun::Host { lobby, .. })) = click(&mut host, spot::READY, false, &mut net) else { panic!("no forced start") };
        assert!(lobby.can_start() && lobby.state().members.iter().all(|m| m.ready), "every member forced ready");
    }

    /// ADR 0026: with the room's "Defeated players" option on, a joiner whose empire dies stays in the session, sees
    /// the whole map, and its screens' orders never reach the lockstep; both machines stay in step.
    #[test]
    fn a_defeated_joiner_watches_without_orders() {
        let Some(Ok(inst)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let rules: &'static GameRules = Box::leak(Box::new(crate::rules::load(&inst).unwrap().0));
        let texts = crate::new_game::StartTexts::read(&inst).unwrap();
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(1, &mut net);
        click(&mut host, spot::OPTION + 2 * 6 + 1, false, &mut net);
        click(&mut guests[0], spot::READY, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready) && h.state().unwrap().galaxy.is_some_and(|g| g.spectators));
        let Some(Out::Start(b)) = click(&mut host, spot::READY, false, &mut net) else { panic!("no start") };
        let hub = net.clone();
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 4242, Box::new(hub.clone()))).unwrap().map(|s| crate::world::World::new_mp(s, rules)).unwrap();
        let mut worlds = vec![mk(b)];
        worlds.extend(started(&mut guests, &mut net).into_iter().map(mk));
        let turn = |w: &crate::world::World| w.net.as_ref().unwrap().0.peer().turn;
        let joiner = worlds[1].net.as_ref().unwrap().0.peer().empire;
        let t = web_time::Instant::now();
        let mut dead = false;
        while worlds.iter().any(|w| turn(w) < 40) {
            for w in &mut worlds {
                w.pump();
                w.tick();
            }
            // The death on both machines at the same turn (as the shared daily check would).
            if !dead && turn(&worlds[0]) >= 5 && turn(&worlds[0]) == turn(&worlds[1]) {
                for w in &mut worlds {
                    w.game.empire_dies(joiner, &[]);
                }
                dead = true;
            }
            if dead && turn(&worlds[1]) == 25 {
                worlds[1].send(ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::Setting { planet: 1, what: 9, value: 3 } });
            }
            assert!(t.elapsed() < Duration::from_secs(60), "stalled");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(worlds[1].game.local.spectating && !worlds[0].game.local.spectating);
        assert!(worlds[1].local_lines.iter().any(|l| l.starts_with(b"Your empire has fallen")));
        assert!(worlds[1].curr.planets.iter().all(|p| p.known), "a spectator sees every planet");
        let Some((Session::Host { session, .. }, _)) = &worlds[0].net else { panic!("the host's world") };
        assert!(!session.host.log.iter().flat_map(|b| &b.commands).any(|c| c.issuer == joiner), "no order from the spectator");
        assert!(session.host.desyncs.is_empty() && !session.status(Duration::from_secs(1)).away.contains(&joiner));
    }

    /// ADR 0026: in a game with the "Defeated players" option a newcomer's Join to the running game makes it a
    /// spectator (empire 0, the host's state now): the players are told, it sees the whole map, orders nothing and
    /// stays in step.
    #[test]
    fn a_newcomer_joins_the_running_game_to_watch() {
        use crate::world::World;
        let Some(Ok(inst)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let rules: &'static GameRules = Box::leak(Box::new(crate::rules::load(&inst).unwrap().0));
        let texts = crate::new_game::StartTexts::read(&inst).unwrap();
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(1, &mut net);
        click(&mut host, spot::OPTION + 2 * 6 + 1, false, &mut net);
        click(&mut guests[0], spot::READY, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready) && h.state().unwrap().galaxy.is_some_and(|g| g.spectators));
        let Some(Out::Start(b)) = click(&mut host, spot::READY, false, &mut net) else { panic!("no start") };
        let hub = net.clone();
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 4242, Box::new(hub.clone()))).unwrap().map(|s| World::new_mp(s, rules)).unwrap();
        let mut worlds = vec![mk(b)];
        worlds.extend(started(&mut guests, &mut net).into_iter().map(mk));
        let turn = |w: &World| w.net.as_ref().unwrap().0.peer().turn;
        let t = web_time::Instant::now();
        let step = |worlds: &mut Vec<World>, until: &dyn Fn(&[World]) -> bool| {
            while !until(worlds) {
                for w in worlds.iter_mut() {
                    w.pump();
                    w.tick();
                }
                assert!(t.elapsed() < Duration::from_secs(60), "stalled");
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        step(&mut worlds, &|w| turn(&w[0]) >= 10);

        let mut watcher = MpScreen::open(false, "Watcher", RULES);
        type_text(&mut watcher, "local", &mut net);
        let b = loop {
            for w in worlds.iter_mut() {
                w.pump();
                w.tick();
            }
            if let Some(Out::Start(b)) = watcher.tick(&mut net) {
                break b;
            }
            assert!(t.elapsed() < Duration::from_secs(20), "no admission");
        };
        assert!(matches!(&b, Begun::Joiner { start, .. } if start.empire == 0 && !start.snapshot.is_empty()));
        worlds.push(mk(b));
        assert!(worlds[2].game.local.spectating);
        let at = turn(&worlds[0]);
        step(&mut worlds, &|w| turn(&w[2]) >= at + 5);
        worlds[2].send(ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::Setting { planet: 1, what: 9, value: 3 } });
        worlds[2].say(b"hello", &[]);
        step(&mut worlds, &|w| w.iter().all(|w| turn(w) >= at + 30));

        assert!(worlds[0].local_lines.iter().any(|l| l == b"Watcher is watching the game."));
        assert!(worlds[1].chat_lines.iter().any(|l| l.0 == b"Spectator: hello"), "a spectator chats");
        assert!(worlds[2].curr.planets.iter().all(|p| p.known), "a spectator sees every planet");
        assert!(worlds[2].game.local.spectating && worlds[2].game.over.is_some(), "still watching, its game never ends");
        let Some((Session::Host { session, .. }, _)) = &worlds[0].net else { panic!("the host's world") };
        assert!(!session.host.log.iter().flat_map(|b| &b.commands).any(|c| c.issuer == 0), "no order from the spectator");
        assert!(session.host.desyncs.is_empty());
        let hashes = |w: &World| w.net.as_ref().unwrap().0.peer().hashes.clone();
        let (h0, h2) = (hashes(&worlds[0]), hashes(&worlds[2]));
        let common: Vec<_> = h2.iter().filter(|x| h0.contains(x)).collect();
        assert!(common.len() >= 25 && h2.iter().all(|(t, x)| h0.iter().find(|(t0, _)| t0 == t).is_none_or(|(_, x0)| x0 == x)), "the spectator is in step");
    }

    /// ADR 0021: a joiner whose game drifts is resynchronised by the host, and both players are told on the message
    /// line.
    #[test]
    fn a_desync_and_its_resync_are_told_to_the_players() {
        use crate::world::World;
        let Some(Ok(inst)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let rules: &'static GameRules = Box::leak(Box::new(crate::rules::load(&inst).unwrap().0));
        let texts = crate::new_game::StartTexts::read(&inst).unwrap();
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(1, &mut net);
        click(&mut guests[0], spot::READY, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready));
        let Some(Out::Start(b)) = click(&mut host, spot::READY, false, &mut net) else { panic!("no start") };
        let hub = net.clone();
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 77, Box::new(hub.clone()))).unwrap().map(|s| World::new_mp(s, rules)).unwrap();
        let mut worlds = vec![mk(b)];
        worlds.extend(started(&mut guests, &mut net).into_iter().map(mk));
        let turn = |w: &World| w.net.as_ref().unwrap().0.peer().turn;
        let t = web_time::Instant::now();
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
        assert!(worlds.iter().all(|w| w.local_lines.is_empty()));
        // The joiner's state drifts (as a stray `rand()` would).
        let money = ig2_sim::save::va::MONEY;
        let m = worlds[1].game.state.mem.i32(money);
        worlds[1].game.state.mem.set_i32(money, m + 12345);
        let host_line: &[u8] = b"Guest0's game was out of step and has been resynchronised.";
        let guest_line: &[u8] = b"This game was out of step with the host and has been resynchronised.";
        step(&mut worlds, &|w| w[0].local_lines.iter().any(|l| l == host_line) && w[1].local_lines.iter().any(|l| l == guest_line));
        let at = turn(&worlds[0]);
        step(&mut worlds, &|w| w.iter().all(|w| turn(w) >= at + 20));
        assert_eq!(worlds[1].local_lines.iter().filter(|l| *l == guest_line).count(), 1, "told once");
        assert_eq!(worlds[1].net.as_ref().unwrap().0.resyncs(), 1);
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
        let t = web_time::Instant::now();
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
        let t2 = web_time::Instant::now();
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

    /// A joiner under Options, Help or a video (`World::run_on`, the window's frame then runs no game screen) for
    /// longer than the keep-alive: both machines' turns go on and nobody is dropped.
    /// A started two-machine game: the host's world and the joiner's.
    fn two_worlds() -> Option<(crate::world::World<'static>, crate::world::World<'static>)> {
        let Some(Ok(inst)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return None };
        let rules: &'static GameRules = Box::leak(Box::new(crate::rules::load(&inst).unwrap().0));
        let texts = crate::new_game::StartTexts::read(&inst).unwrap();
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(1, &mut net);
        click(&mut guests[0], spot::READY, false, &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().members.iter().skip(1).all(|m| m.ready));
        let Some(Out::Start(b)) = click(&mut host, spot::READY, false, &mut net) else { panic!("no start") };
        let hub = net.clone();
        let mk = |b: Begun| texts.with(&inst, |data| launch(b, data, rules, 4242, Box::new(hub.clone()))).unwrap().map(|s| crate::world::World::new_mp(s, rules)).unwrap();
        let a = mk(b);
        Some((a, mk(started(&mut guests, &mut net).pop().unwrap())))
    }

    #[test]
    fn a_joiner_under_options_keeps_the_session_running() {
        let Some((mut a, mut g)) = two_worlds() else { return };
        let t = web_time::Instant::now();
        let ms = || t.elapsed().as_millis() as u32;
        let mut clocks = [0, 1].map(|_| ig2_sim::time::TickClock::new(ig2_sim::time::SIM_TICK_MS, ms()));
        let turn = |w: &crate::world::World| w.net.as_ref().unwrap().0.peer().turn;
        let under = ig2_net::session::DEFAULT_KEEP_ALIVE + Duration::from_secs(2);
        while t.elapsed() < under {
            // The host plays on the starmap at the sim's pace (the frame's pump and ticks due).
            a.pump();
            for _ in 0..clocks[0].advance(ms()) {
                a.tick();
            }
            g.run_on(&mut clocks[1], ms());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(turn(&a) > 100 && turn(&g) > 100, "the turns ran: {} {}", turn(&a), turn(&g));
        for w in [&a, &g] {
            let (net, epoch) = w.net.as_ref().unwrap();
            let s = net.status(epoch.elapsed());
            assert!(s.away.is_empty() && s.waiting.is_empty() && s.migrating.is_empty(), "{s:?}");
        }
    }

    /// Issue #6: a turn runs every two ticks (100 ms, ADR 0007 §2), and on a LAN (input delay 0: a turn's round trip) a joiner's order
    /// runs on both machines within two turns (200 ms) of being given. A tick here is three frames' pumps.
    #[test]
    fn turns_are_100_ms_and_a_joiners_order_lands_soon() {
        let Some((mut a, mut g)) = two_worlds() else { return };
        let turn = |w: &crate::world::World| w.net.as_ref().unwrap().0.peer().turn;
        fn step(a: &mut crate::world::World, g: &mut crate::world::World) {
            for _ in 0..3 {
                a.pump();
                g.pump();
            }
            a.tick();
            g.tick();
        }
        for _ in 0..10 {
            step(&mut a, &mut g);
        }
        let from = (turn(&a), turn(&g));
        for _ in 0..40 {
            step(&mut a, &mut g);
        }
        assert_eq!((turn(&a) - from.0, turn(&g) - from.1), (20, 20), "40 ticks (2 s) run 20 turns");
        let joiner = g.net.as_ref().unwrap().0.peer().empire;
        let home = g.game.state.colony().planets.iter().position(|p| p.owner == joiner).expect("the joiner's planet");
        g.send(ig2_sim::game::ColonyOrder::Build { planet: home as i32, kind: 0, x: 0, y: 0 }.into());
        let landed = |w: &crate::world::World| !w.colony_events.is_empty();
        let mut ticks = 0;
        while !(landed(&a) && landed(&g)) {
            step(&mut a, &mut g);
            ticks += 1;
            assert!(ticks < 40, "the order never ran");
        }
        assert!(ticks <= 4, "the order took {ticks} ticks");
        eprintln!("a joiner's order ran on both machines after {ticks} ticks");
    }

    /// ADR 0063: a joiner's fleet order shows on its own screens in the same frame, as in single player (a prediction:
    /// the shared game and the host's are untouched), and the prediction gives way to the real order once its turn
    /// runs, the same on both machines, which stay in step.
    #[test]
    fn a_joiners_order_shows_at_once_and_the_prediction_clears() {
        let Some((mut a, mut g)) = two_worlds() else { return };
        fn step(a: &mut crate::world::World, g: &mut crate::world::World) {
            for _ in 0..3 {
                a.pump();
                g.pump();
            }
            a.tick();
            g.tick();
        }
        for _ in 0..10 {
            step(&mut a, &mut g);
        }
        let joiner = g.net.as_ref().unwrap().0.peer().empire;
        let f = g.curr.fleets.iter().find(|f| f.owner == joiner).expect("the joiner's fleet").clone();
        let to = [f.pos[0] + 400.0, f.pos[1]];
        let target = |w: &crate::world::World| w.curr.fleets.iter().find(|x| x.slot == f.slot).and_then(|x| x.target);
        assert_ne!(target(&g), Some(to));
        g.send(ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::Move { fleet: f.slot as i32, order: 1, kind: 0, x: to[0].to_bits(), y: to[1].to_bits() } });
        g.pump();
        assert!(g.predicting());
        assert_eq!(target(&g), Some(to), "the order shows in the frame it was given");
        assert_ne!(target(&a), Some(to), "the host hasn't run it yet");
        let mut ticks = 0;
        while g.predicting() || target(&a) != Some(to) {
            step(&mut a, &mut g);
            assert_eq!(target(&g), Some(to), "the prediction holds until the real order replaces it");
            ticks += 1;
            assert!(ticks < 40, "the order never ran");
        }
        for _ in 0..10 {
            step(&mut a, &mut g);
        }
        let hashes = |w: &crate::world::World| w.net.as_ref().unwrap().0.peer().hashes.clone();
        let (ha, hg) = (hashes(&a), hashes(&g));
        assert!(hg.iter().filter(|x| ha.contains(x)).count() >= 10, "the machines stay in step");
        assert!(hg.iter().all(|(t, x)| ha.iter().find(|(t0, _)| t0 == t).is_none_or(|(_, x0)| x0 == x)));
        eprintln!("the real order replaced the prediction after {ticks} ticks");
    }

    /// Issue #33: a fleet on its way glides on both machines, frame by frame as the window draws it (the frame's message
    /// pass, the ticks due, the interpolation's way between the turns' snapshots; none while paused), also through a
    /// pause: it stands still while paused and never hops a turn's way in one frame.
    #[test]
    fn a_moving_fleet_glides_through_a_pause() {
        let Some((mut a, mut g)) = two_worlds() else { return };
        let joiner = g.net.as_ref().unwrap().0.peer().empire;
        let f = g.curr.fleets.iter().find(|f| f.owner == joiner).expect("the joiner's fleet").clone();
        g.send(ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::Move { fleet: f.slot as i32, order: 1, kind: 0, x: (f.pos[0] + 400.0).to_bits(), y: f.pos[1].to_bits() } });
        let pause = |w: &mut crate::world::World, on: bool| w.send(ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::Pause { paused: on } });
        // Half a tick apart, at 60 frames a second.
        let mut clocks = [0, 25].map(|at| ig2_sim::time::TickClock::new(ig2_sim::time::SIM_TICK_MS, at));
        let mut steps = [Vec::new(), Vec::new()];
        let mut last = [f32::NAN; 2];
        // The joiner's own fleet's radar circle (ADR 0063): it glides with the fleet, frame by frame.
        let (mut zone_frames, mut zone_last) = (0, None::<(f32, f32)>);
        for now in (100..3100).step_by(16) {
            match now {
                1092 => pause(&mut a, true),
                2092 => pause(&mut g, false),
                _ => {}
            }
            let [ca, cg] = &mut clocks;
            for (i, (w, c)) in [(&mut a, ca), (&mut g, cg)].into_iter().enumerate() {
                w.pump();
                for _ in 0..c.advance(now) {
                    w.tick();
                }
                let alpha = w.draw_phase(c.phase(now));
                let x = w.curr.fleets_at(&w.prev, alpha).iter().find(|v| v.slot == f.slot).expect("the fleet").pos[0];
                if now > 300 {
                    steps[i].push(x - last[i]);
                }
                last[i] = x;
                if i == 1 {
                    let circles = w.curr.radar_circles_at(&w.prev, alpha).unwrap_or_else(|| w.curr.radar_circles.0.clone());
                    // (A new galaxy's fleets get their range from the first ticks.)
                    let z = circles.iter().find(|c| c.id == -(f.slot as i16)).map(|c| c.centre[0]);
                    if let Some(z) = z {
                        assert!((z - x).abs() < 1e-3, "the zone sits on the fleet: {z} {x}");
                        if let Some((z0, x0)) = zone_last {
                            assert!(((z - z0) - (x - x0)).abs() < 1e-3, "the zone moves as the fleet does");
                            zone_frames += (z != z0) as u32;
                        }
                    }
                    zone_last = z.map(|z| (z, x));
                }
            }
        }
        for (i, s) in steps.iter().enumerate() {
            let moving: Vec<f32> = s.iter().copied().filter(|d| *d != 0.0).collect();
            let step = moving.iter().sum::<f32>() / moving.len() as f32;
            assert!(step > 0.0 && moving.iter().all(|d| *d > 0.0 && *d < 2.0 * step), "machine {i}: the fleet hops {s:?}");
            assert!(s.len() - moving.len() > 40, "machine {i}: the pause stops the fleet");
        }
        assert!(zone_frames > 60, "the fleet's zone moved in {zone_frames} frames");
    }

    /// Issue #32: the host's pause stops the date on both machines (the session keeps turning), and the joiner's
    /// un-pause starts it again.
    #[test]
    fn a_pause_stops_the_date_on_every_machine() {
        let Some((mut a, mut g)) = two_worlds() else { return };
        let step = |a: &mut crate::world::World, g: &mut crate::world::World| {
            a.pump();
            g.pump();
            a.tick();
            g.tick();
        };
        let ido = |w: &crate::world::World| w.game.state.mem.i32(ig2_sim::save::va::IDO);
        let turn = |w: &crate::world::World| w.net.as_ref().unwrap().0.peer().turn;
        for _ in 0..10 {
            step(&mut a, &mut g);
        }
        a.send(ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::Pause { paused: true } });
        for _ in 0..20 {
            step(&mut a, &mut g);
        }
        assert!(a.paused() && g.paused());
        let at = (ido(&a), ido(&g), turn(&a));
        for _ in 0..40 {
            step(&mut a, &mut g);
        }
        assert_eq!((ido(&a), ido(&g)), (at.0, at.1), "the date while paused");
        assert!(turn(&a) > at.2 + 10, "the turns went on");
        g.send(ig2_net::Command::Starmap { order: ig2_net::StarmapCmd::Pause { paused: false } });
        for _ in 0..20 {
            step(&mut a, &mut g);
        }
        assert!(!a.paused() && ido(&a) > at.0 && ido(&g) > at.1, "the date after the un-pause");
    }

    /// Each machine's diplomacy screen shows its own contact list (`AktPlayerek`, multiplayer.md §5.1): the joiner's
    /// is its copy beside the canonical one, the host's the canonical one.
    #[test]
    fn each_machine_shows_its_own_contacts() {
        let Some((mut a, mut g)) = two_worlds() else { return };
        for _ in 0..20 {
            for w in [&mut a, &mut g] {
                w.pump();
                w.tick();
            }
        }
        let joiner = g.net.as_ref().unwrap().0.peer().empire;
        let mut d = g.game.state.diplomacy();
        let own = d.others.iter_mut().find(|c| c.empire == joiner).expect("the joiner's own list");
        own.players.push(ig2_sim::diplomacy::PlayerSlot { empire: 7, ..Default::default() });
        let list: Vec<i32> = own.players.iter().map(|s| s.empire).collect();
        g.game.state.store_diplomacy(&d);
        let v = crate::diplomacy::View::read(&g.game.state);
        assert_eq!(v.en, joiner as usize);
        assert_eq!(v.slots.iter().map(|s| s.empire).collect::<Vec<_>>(), list);
        assert_eq!(v.raw.len(), list.len());
        let host = crate::diplomacy::View::read(&a.game.state);
        assert_eq!(host.slots, a.game.state.diplomacy().players);
    }

    /// A joiner's colony order runs on every machine and its result reaches the joiner's own screens
    /// (`World::colony_events`, as `MULTIGET_Colony` updates the issuer's screen), not the host's.
    #[test]
    fn a_joiners_order_result_reaches_its_screens() {
        let Some((mut a, mut g)) = two_worlds() else { return };
        let joiner = g.net.as_ref().unwrap().0.peer().empire;
        let turn = |w: &crate::world::World| w.net.as_ref().unwrap().0.peer().turn;
        // On its own planet: another's is refused unseen (`Game::foreign_planet`).
        let home = g.game.state.colony().planets.iter().position(|p| p.owner == joiner).expect("the joiner's planet");
        g.send(ig2_sim::game::ColonyOrder::Build { planet: home as i32, kind: 0, x: 0, y: 0 }.into());
        let t = web_time::Instant::now();
        while turn(&a) < 20 || turn(&g) < 20 {
            for w in [&mut a, &mut g] {
                w.pump();
                w.tick();
            }
            assert!(t.elapsed() < Duration::from_secs(60), "stalled");
            std::thread::sleep(Duration::from_millis(1));
        }
        use ig2_sim::game::ColonyOrderEvent::{BuildRefused, Built};
        let mine = |w: &crate::world::World| w.colony_events.iter().filter(|e| matches!(e, Built { empire, .. } | BuildRefused { empire, .. } if *empire == joiner)).count();
        assert_eq!(mine(&g), 1, "{:?}", g.colony_events);
        assert_eq!(mine(&a), 1, "every machine sees it; the screens filter by empire");
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
        let t = web_time::Instant::now();
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
        assert!(matches!(&host.role, Some(Role::Host { listener, .. }) if matches!(listener.port_map(), PortMap::Mapped(_))), "a save's room maps its port too");
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
        let t = web_time::Instant::now();
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
        let t = web_time::Instant::now();
        while !guests.iter().all(|g| g.lost && g.role.is_none()) {
            for g in &mut guests {
                g.tick(&mut net);
            }
            assert!(t.elapsed() < Duration::from_secs(2), "the joiners didn't notice");
        }
    }
    /// Ours (ADR 0064): both pages lay out for the host and a joiner; the room's drop-downs (holder, computer level,
    /// race) act through [`MpScreen::apply`]; a right click is Back when the menu's navigation is on.
    #[test]
    fn the_pages_lay_out_and_their_drop_downs_act() {
        let texts = crate::ui::UiTexts { strings: Default::default(), kinds: Vec::new(), lang: ig2_formats::text::Lang::En };
        let theme = crate::theme::Theme::modern();
        let style = crate::gui::style(&theme);
        let mut gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb);
        let mut net = LoopNet::default();
        let page = |s: &mut MpScreen, flow: bool, gui: &mut crate::gui::Gui| {
            let mut acts = Vec::new();
            gui.run((1024, 768), &style, |ctx| acts = s.page(ctx, &texts, &[], &theme, flow));
            acts
        };
        let mut lone = MpScreen::open(true, "Lone", RULES);
        assert!(page(&mut lone, false, &mut gui).is_empty(), "nothing before the fade ends");
        let (mut host, mut guests) = room(1, &mut net);
        for s in std::iter::once(&mut host).chain(guests.iter_mut()) {
            assert!(page(s, false, &mut gui).is_empty());
        }
        host.apply(vec![page::Act::Slot(7, SlotKind::Ai), page::Act::Level(7, 3), page::Act::Race(0, 4)], &mut net);
        // A joiner can't change another's race nor any holder.
        let outs = guests[0].apply(vec![page::Act::Race(0, 5), page::Act::Slot(6, SlotKind::Closed)], &mut net);
        assert!(outs.is_empty());
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().slots[7].kind == SlotKind::Ai);
        let st = guests[0].state().unwrap();
        assert_eq!((st.slots[0].race, st.slots[6].kind, st.galaxy.unwrap().difficulty[7]), (4, SlotKind::Open, 3));
        // ADR 0085 §4: the host's custom game reaches the seats; a joiner's is refused; both pages draw its rows.
        let seat = ig2_net::galaxy::Seat { money: 30000, tech: 2, planets: 3, aggression: 1, relation: 1 };
        let mut custom = ig2_net::galaxy::RoomCustom::new(90, 77, seat);
        custom.seats[7].aggression = 2;
        let o = GalaxyOptions { custom: Some(custom), ..st.galaxy.unwrap() };
        assert!(guests[0].apply(vec![page::Act::Options(GalaxyOptions { custom: None, ..o })], &mut net).is_empty());
        host.custom_seat = Some(seat);
        host.apply(vec![page::Act::Options(o)], &mut net);
        sync(&mut host, &mut guests, &mut net, |h| h.state().unwrap().galaxy.unwrap().custom.is_some());
        assert_eq!(guests[0].state().unwrap().galaxy.unwrap().custom, Some(custom));
        for s in std::iter::once(&mut host).chain(guests.iter_mut()) {
            assert!(page(s, false, &mut gui).is_empty());
        }
        // The right button: Back with the menu's navigation, nothing without.
        let right = |gui: &mut crate::gui::Gui| {
            gui.pointer(Some([5.0, 5.0]), crate::gui::Gui::scale(768));
            gui.button(1, true);
            gui.button(1, false);
        };
        right(&mut gui);
        assert!(page(&mut host, false, &mut gui).is_empty());
        right(&mut gui);
        assert_eq!(page(&mut host, true, &mut gui), [page::Act::Back]);
        host.apply(vec![page::Act::Back], &mut net);
        assert!(host.role.is_none(), "Back leaves the room");
    }

    #[test]
    fn the_host_maps_its_port_from_the_room() {
        let mut net = LoopNet::default();
        let (mut host, mut guests) = room(1, &mut net);
        let map = |h: &MpScreen| match &h.role {
            Some(Role::Host { listener, .. }) => listener.port_map(),
            _ => unreachable!(),
        };
        assert_eq!(map(&host), PortMap::Mapped("203.0.113.7:4001".parse().unwrap()), "on by default (ADR 0020)");
        assert!(click(&mut guests[0], spot::PORT_MAP, false, &mut net).is_none(), "a joiner has no port mapping");
        assert!(matches!(click(&mut host, spot::PORT_MAP, false, &mut net), Some(Out::Click)));
        assert_eq!(map(&host), PortMap::Off, "a press removes the mapping");
        click(&mut host, spot::PORT_MAP, false, &mut net);
        assert_eq!(map(&host), PortMap::Mapped("203.0.113.7:4001".parse().unwrap()), "a second press maps again");
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

    /// ADR 0086 §3: a joiner without the room's mods is refused and sees which they are; the dialog takes Esc (back to
    /// the connection page, still open) and Enter (no index in tests: Open Mods).
    #[test]
    fn a_joiner_without_the_rooms_mods_sees_them() {
        let mut net = LoopNet::default();
        let list = vec![ig2_net::lobby::RoomMod { id: "armada".into(), version: "1.2.0".into(), sha256: Some("ab".repeat(32)) }];
        let mods = [3; 32];
        let mut host = MpScreen::open(false, "Host", ig2_net::session::modded_fingerprint(RULES, Some(mods))).with_mods(Some(mods)).with_mod_list(list.clone());
        let mut guest = MpScreen::open(false, "Guest", RULES);
        for _ in 0..25 {
            host.tick(&mut net);
            guest.tick(&mut net);
        }
        click(&mut host, spot::HOST, false, &mut net);
        type_text(&mut guest, "local", &mut net);
        let mut all = [&mut host, &mut guest];
        run(&mut all, &mut net, |s| s[1].room_mods.as_ref().is_some_and(|r| !r.busy()));
        assert_eq!(guest.line, refusal_text(Refusal::Mods));
        let r = guest.room_mods.as_ref().unwrap();
        assert_eq!((r.diff.missing.as_slice(), r.offer()), (list.as_slice(), crate::room_mods::Offer::Mods));
        // Typing goes nowhere while it is up; Esc closes it, not the page.
        assert!(guest.key(Some(EntryKey::Char(b'x')), false, &mut net).is_none());
        assert!(matches!(guest.key(None, true, &mut net), Some(Out::Click)));
        assert!(guest.room_mods.is_none() && guest.role.is_none());
        // Again, and Enter: the Mods page.
        type_text(&mut guest, "", &mut net);
        let mut all = [&mut host, &mut guest];
        run(&mut all, &mut net, |s| s[1].room_mods.as_ref().is_some_and(|r| !r.busy()));
        assert!(matches!(guest.key(Some(EntryKey::Enter), false, &mut net), Some(Out::Mods)));
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
        let t = web_time::Instant::now();
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
    fn the_public_relay_is_the_default_until_one_is_saved() {
        let missing = || Err(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert_eq!(saved_relay(missing(), DEFAULT_RELAY), DEFAULT_RELAY);
        assert_eq!(saved_relay(Ok(String::new()), DEFAULT_RELAY), "");
        assert_eq!(saved_relay(Ok("10.0.0.2".into()), DEFAULT_RELAY), "10.0.0.2");
    }

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
        let t = web_time::Instant::now();
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

    /// ADR 0060 through the screens and a service on 127.0.0.1: the host's room goes on the public list only once it
    /// switches Public on, a joiner's "Internet games" tab lists it and two clicks on its row join it.
    #[test]
    fn a_public_room_is_browsed_and_joined_with_a_click() {
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
        let mut guest = open("Guest");
        // Opened from Internet the list is asked for at the first tick; without a relay it asks for one.
        guest.tick(&mut net);
        assert_eq!((guest.line.as_slice(), guest.focus), (words::NEED_RELAY, Focus::Address));
        click_on(&mut guest, spot::RELAY, &mut net);
        guest.paste(&relay);
        let t = web_time::Instant::now();
        let wait = |host: &mut MpScreen, host_net: &mut QuicNet, guest: &mut MpScreen, net: &mut QuicNet, what: &str, done: &dyn Fn(&MpScreen, &MpScreen) -> bool| {
            while !done(host, guest) {
                host.tick(host_net);
                guest.tick(net);
                assert!(t.elapsed() < Duration::from_secs(30), "{what}: {}", String::from_utf8_lossy(&guest.line));
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        click_on(&mut host, spot::RELAY, &mut host_net);
        host.paste(&relay);
        host.host(&mut host_net);
        wait(&mut host, &mut host_net, &mut guest, &mut net, "registered", &|h, _| matches!(&h.role, Some(Role::Host { listener, .. }) if matches!(listener.rendezvous(), Rendezvous::Registered(_))));
        // Private by default: the list is empty.
        click_on(&mut guest, spot::REFRESH, &mut net);
        assert_eq!(guest.line, words::ASKING);
        wait(&mut host, &mut host_net, &mut guest, &mut net, "asked", &|_, g| g.browsing.is_none());
        assert_eq!((guest.rows(), guest.line.as_slice()), (0, words::NO_PUBLIC));
        assert!(matches!(&host.role, Some(Role::Host { listener, .. }) if listener.rendezvous() != Rendezvous::Off), "a host with a relay has the switch");
        click_on(&mut host, spot::PUBLIC, &mut host_net);
        assert!(host.public);
        for _ in 0..5 {
            host.tick(&mut host_net);
        }
        std::thread::sleep(Duration::from_millis(100));
        click_on(&mut guest, spot::REFRESH, &mut net);
        wait(&mut host, &mut host_net, &mut guest, &mut net, "listed", &|_, g| g.browsing.is_none());
        let rooms = guest.internet.clone().unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!((rooms[0].1.announcement.name.as_str(), rooms[0].1.announcement.humans, rooms[0].1.mods), ("Host's galaxy", 1, [0; 32]));
        // Two clicks on the row: the join code's way in, the code left in the address field.
        click_on(&mut guest, spot::LIST, &mut net);
        click_on(&mut guest, spot::LIST, &mut net);
        assert!(JoinCode::decode(&guest.address.string()).is_ok_and(|c| c.addrs == [rooms[0].0]));
        wait(&mut host, &mut host_net, &mut guest, &mut net, "joined", &|_, g| g.state().is_some_and(|st| st.members.len() == 2));
        // Leaving takes the room off the list at once.
        host.key(None, true, &mut host_net);
        let mut other = open("Other");
        click_on(&mut other, spot::RELAY, &mut net);
        other.paste(&relay);
        other.tick(&mut net);
        wait(&mut host, &mut host_net, &mut other, &mut net, "unlisted", &|_, o| o.browsing.is_none());
        assert_eq!(other.rows(), 0);
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn click_on(s: &mut MpScreen, id: u32, net: &mut dyn Net) -> Option<Out> {
        s.act(id, false, net)
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
