//! LAN discovery (ADR 0008 §1): a browser broadcasts a padded query on the LAN, every host's [`Announcer`] answers
//! with its [`Announcement`] by unicast. LAN only: both sides drop packets from addresses outside the private,
//! link-local and loopback ranges. Private sessions never answer.
//!
//! ```text
//! Browser ── Query{nonce} (256 B, broadcast :29170) ──▶ Announcer
//!         ◀──────── Answer{nonce, Announcement} (unicast) ──
//! ```

use std::collections::BTreeMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::wire;

/// The UDP port hosts listen on for queries.
pub const DISCOVERY_PORT: u16 = 29170;
/// The discovery packets' version.
pub const VERSION: u8 = 1;
/// A query's exact size (padding; an answer is always smaller, so it never amplifies).
pub const QUERY_LEN: usize = 256;
/// The largest answer sent.
pub const MAX_ANSWER: usize = QUERY_LEN - 1;
/// The longest session or scenario name announced (characters).
pub const MAX_NAME: usize = 32;
/// A session not heard from for this long is forgotten.
pub const FORGET: Duration = Duration::from_secs(3);

const MAGIC: &[u8; 4] = b"IG2D";
const QUERY: u8 = 0;
const ANSWER: u8 = 1;
/// The browser accepts answers to its last few queries.
const NONCES: usize = 8;

/// What a host tells the LAN about its session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announcement {
    /// The session id (also the join code of a private session, which isn't announced).
    pub session: u64,
    pub name: String,
    /// The QUIC game port; the address is the answer's source.
    pub port: u16,
    /// SHA-256 of the session certificate (DER), pinned by joiners.
    pub fingerprint: [u8; 32],
    pub password: bool,
    /// The lobby protocol version.
    pub version: u32,
    /// The rules fingerprint (ADR 0005 §4).
    pub rules: [u8; 32],
    pub scenario: String,
    pub humans: u8,
    pub slots: u8,
    /// The game has started (rejoin only).
    pub started: bool,
}

/// A session a browser heard of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub announcement: Announcement,
    /// Where to connect (the answer's source address with the announced port).
    pub addr: SocketAddr,
    pub seen: Instant,
}

/// Loopback, RFC 1918, link-local or IPv6 ULA: the addresses discovery talks to.
pub fn is_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_lan(IpAddr::V4(v4));
            }
            let s = v6.segments()[0];
            v6.is_loopback() || s & 0xffc0 == 0xfe80 || s & 0xfe00 == 0xfc00
        }
    }
}

fn truncate(s: &str) -> String {
    s.chars().take(MAX_NAME).collect()
}

/// A query packet with `nonce`.
pub fn query_packet(nonce: u64) -> [u8; QUERY_LEN] {
    let mut p = [0u8; QUERY_LEN];
    p[..4].copy_from_slice(MAGIC);
    p[4] = VERSION;
    p[5] = QUERY;
    p[6..14].copy_from_slice(&nonce.to_le_bytes());
    p
}

/// The nonce of a well-formed query.
fn parse_query(p: &[u8]) -> Option<u64> {
    (p.len() == QUERY_LEN && &p[..4] == MAGIC && p[4] == VERSION && p[5] == QUERY && p[14..].iter().all(|&b| b == 0))
        .then(|| u64::from_le_bytes(p[6..14].try_into().unwrap()))
}

/// The answer packet to `nonce`, or `None` if it would be too big even with the names truncated.
pub fn answer_packet(nonce: u64, a: &Announcement) -> Option<Vec<u8>> {
    let a = Announcement { name: truncate(&a.name), scenario: truncate(&a.scenario), ..a.clone() };
    let mut p = Vec::with_capacity(MAX_ANSWER);
    p.extend_from_slice(MAGIC);
    p.extend_from_slice(&[VERSION, ANSWER]);
    p.extend_from_slice(&nonce.to_le_bytes());
    wire::to_sink(&a, &mut p).ok()?;
    (p.len() <= MAX_ANSWER).then_some(p)
}

fn parse_answer(p: &[u8]) -> Option<(u64, Announcement)> {
    if p.len() < 14 || p.len() > MAX_ANSWER || &p[..4] != MAGIC || p[4] != VERSION || p[5] != ANSWER {
        return None;
    }
    let nonce = u64::from_le_bytes(p[6..14].try_into().unwrap());
    let a: Announcement = wire::from_slice(&p[14..]).ok()?;
    (a.name.chars().count() <= MAX_NAME && a.scenario.chars().count() <= MAX_NAME).then_some((nonce, a))
}

/// A random `u64` (nonces, session ids).
pub fn random_u64() -> u64 {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("OS random source");
    u64::from_le_bytes(b)
}

// ------------------------------------------------------------------------------------------------ host

/// The host's side: answers queries with the current announcement.
pub struct Announcer {
    socket: UdpSocket,
    announcement: Option<Announcement>,
}

impl Announcer {
    /// Listen on `0.0.0.0:DISCOVERY_PORT`, shared with other hosts on this machine (`SO_REUSEADDR`).
    pub fn bind_lan() -> io::Result<Self> {
        let s = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, Some(socket2::Protocol::UDP))?;
        s.set_reuse_address(true)?;
        s.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT)).into())?;
        Self::with_socket(s.into())
    }

    /// Listen on `addr` (tests: `127.0.0.1:0`).
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        Self::with_socket(UdpSocket::bind(addr)?)
    }

    fn with_socket(socket: UdpSocket) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self { socket, announcement: None })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// What to answer with; `None` (a private session, or none hosted) answers nothing.
    pub fn set(&mut self, a: Option<Announcement>) {
        self.announcement = a;
    }

    /// Answer every query waiting. Returns how many were answered.
    pub fn poll(&mut self) -> io::Result<usize> {
        let mut buf = [0u8; QUERY_LEN + 1];
        let mut n = 0;
        loop {
            let (len, from) = match self.socket.recv_from(&mut buf) {
                Ok(r) => r,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(n),
                // Windows reports an earlier send's ICMP "port unreachable" on the next receive.
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(e) => return Err(e),
            };
            let (Some(nonce), Some(a)) = (parse_query(&buf[..len]), &self.announcement) else { continue };
            if !is_lan(from.ip()) {
                continue;
            }
            if let Some(p) = answer_packet(nonce, a) {
                if self.socket.send_to(&p, from).is_ok() {
                    n += 1;
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ browser

/// The LAN tab's side: queries and collects the answers.
pub struct Browser {
    socket: UdpSocket,
    targets: Vec<SocketAddr>,
    nonces: Vec<u64>,
    found: BTreeMap<(SocketAddr, u64), Found>,
}

impl Browser {
    /// Query the LAN by limited broadcast (`255.255.255.255:DISCOVERY_PORT`).
    pub fn lan() -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_broadcast(true)?;
        Self::with_socket(socket, vec![SocketAddr::from((Ipv4Addr::BROADCAST, DISCOVERY_PORT))])
    }

    /// Query `targets` from a socket bound to `bind` (tests: announcers on `127.0.0.1`).
    pub fn with_targets(bind: SocketAddr, targets: Vec<SocketAddr>) -> io::Result<Self> {
        Self::with_socket(UdpSocket::bind(bind)?, targets)
    }

    fn with_socket(socket: UdpSocket, targets: Vec<SocketAddr>) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self { socket, targets, nonces: Vec::new(), found: BTreeMap::new() })
    }

    /// Send a query (once a second while the list is shown).
    pub fn query(&mut self) -> io::Result<()> {
        let nonce = random_u64();
        if self.nonces.len() == NONCES {
            self.nonces.remove(0);
        }
        self.nonces.push(nonce);
        let p = query_packet(nonce);
        for t in &self.targets {
            self.socket.send_to(&p, t)?;
        }
        Ok(())
    }

    /// Take the answers waiting and forget sessions not heard from for [`FORGET`].
    pub fn poll(&mut self) -> io::Result<()> {
        let mut buf = [0u8; QUERY_LEN];
        let now = Instant::now();
        loop {
            let (len, from) = match self.socket.recv_from(&mut buf) {
                Ok(r) => r,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                Err(e) => return Err(e),
            };
            if !is_lan(from.ip()) {
                continue;
            }
            let Some((nonce, a)) = parse_answer(&buf[..len]) else { continue };
            if !self.nonces.contains(&nonce) {
                continue;
            }
            let addr = SocketAddr::new(from.ip(), a.port);
            self.found.insert((from, a.session), Found { announcement: a, addr, seen: now });
        }
        self.found.retain(|_, f| now.duration_since(f.seen) < FORGET);
        Ok(())
    }

    /// The sessions heard of, by address.
    pub fn sessions(&self) -> impl Iterator<Item = &Found> {
        self.found.values()
    }
}
