//! The rendezvous and relay service (ADR 0018): its packets and its state, without sockets. The `ig2-rendezvous`
//! binary feeds it datagrams; the client side is `quic::RelaySocket`.
//!
//! ```text
//! host ── Register{session, pin} ──▶ service ── Registered{public addr} ──▶ host
//! joiner ─ Lookup{session, pin} ──▶ service ── Found{host} ──▶ joiner, Incoming{joiner} ──▶ host (punches)
//! relayed: peer ─ To{dest, QUIC datagram} ─▶ service ─ From{src, datagram} ─▶ dest
//! public list (ADR 0060): host ─ Publish{Listing} ─▶ service; browser ─ List{start} (1200 B) ─▶ service ─ Rooms ─▶
//! ```

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::discovery::{Announcement, MAX_NAME};
use crate::joincode::PIN_BYTES;
use crate::wire;

/// Every packet's first bytes.
pub const MAGIC: &[u8; 4] = b"IG2R";
pub const VERSION: u8 = 1;
/// The service's default UDP port (after the LAN discovery and game ports).
pub const DEFAULT_PORT: u16 = crate::discovery::DISCOVERY_PORT + 2;
/// Magic, version, kind.
const HEAD: usize = 6;
/// The header in front of a relayed datagram (head and an address).
pub const DATA_HEAD: usize = HEAD + 6;
/// A List request's exact size (padding: a Rooms answer is never larger, so the list doesn't amplify).
pub const LIST_LEN: usize = 1200;
/// The longest Listing accepted (its names are at most [`MAX_NAME`] characters).
pub const MAX_LISTING: usize = 512;
/// The pages [`list`] asks for at most (a page holds 2 rooms of the largest listing, 6 or so of usual ones; within
/// the service's control burst of 40).
pub const MAX_PAGES: u16 = 32;

/// A public room's entry in the service's list (ADR 0060): the room's LAN announcement (name, players, slots,
/// scenario, lobby version, rules fingerprint, password, certificate fingerprint) and the enabled mods' fingerprint
/// (zero: none). The host's public address comes from the service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    pub announcement: Announcement,
    pub mods: [u8; 32],
}

impl Listing {
    pub fn encode(&self) -> Vec<u8> {
        wire::to_vec(self).expect("a listing encodes")
    }

    /// A well-formed listing within the size and name limits.
    pub fn decode(b: &[u8]) -> Option<Self> {
        let l: Self = (b.len() <= MAX_LISTING).then(|| wire::from_slice(b).ok()).flatten()?;
        let a = &l.announcement;
        (a.name.chars().count() <= MAX_NAME && a.scenario.chars().count() <= MAX_NAME).then_some(l)
    }

    /// The join code of this room at `host` (its session, the certificate's pin).
    pub fn join_code(&self, host: SocketAddrV4) -> crate::joincode::JoinCode {
        crate::joincode::JoinCode::new(self.announcement.session, &self.announcement.fingerprint, &[host])
    }
}

/// A host's room at the rendezvous service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendezvous {
    /// No service, or not registered yet.
    Off,
    Asking,
    /// The room is open; the service sees this host at this public address.
    Registered(SocketAddrV4),
    Refused(Refusal),
    NoAnswer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No room with that session and pin.
    Unknown = 1,
    /// The session is held by another address.
    Taken = 2,
    /// Too many rooms (in all or from this address) or peers in the room.
    Full = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packet<'a> {
    Register { session: u64, pin: [u8; PIN_BYTES] },
    Registered { seen: SocketAddrV4 },
    Lookup { session: u64, pin: [u8; PIN_BYTES] },
    Found { host: SocketAddrV4 },
    Incoming { joiner: SocketAddrV4 },
    Refused(Refusal),
    Unregister,
    Punch,
    To { peer: SocketAddrV4, data: &'a [u8] },
    From { peer: SocketAddrV4, data: &'a [u8] },
    /// The host's room goes on the public list (an encoded [`Listing`]; repeated with every Register).
    Publish { listing: &'a [u8] },
    /// The host's room leaves the public list now.
    Unlist,
    /// The public list from entry `start` on (padded to [`LIST_LEN`]).
    List { start: u16 },
    /// Entries `start..` of `total`: each the host's address, a u16 LE length and an encoded [`Listing`].
    Rooms { total: u16, start: u16, entries: &'a [u8] },
}

fn put_addr(b: &mut Vec<u8>, a: SocketAddrV4) {
    b.extend_from_slice(&a.ip().octets());
    b.extend_from_slice(&a.port().to_be_bytes());
}

fn addr(b: &[u8]) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(b[0], b[1], b[2], b[3]), u16::from_be_bytes([b[4], b[5]]))
}

impl<'a> Packet<'a> {
    fn kind(&self) -> u8 {
        match self {
            Self::Register { .. } => 1,
            Self::Registered { .. } => 2,
            Self::Lookup { .. } => 3,
            Self::Found { .. } => 4,
            Self::Incoming { .. } => 5,
            Self::Refused(_) => 6,
            Self::Unregister => 7,
            Self::Punch => 8,
            Self::To { .. } => 9,
            Self::From { .. } => 10,
            Self::Publish { .. } => 11,
            Self::Unlist => 12,
            Self::List { .. } => 13,
            Self::Rooms { .. } => 14,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(64);
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&[VERSION, self.kind()]);
        match *self {
            Self::Register { session, pin } | Self::Lookup { session, pin } => {
                b.extend_from_slice(&session.to_le_bytes());
                b.extend_from_slice(&pin);
            }
            Self::Registered { seen: a } | Self::Found { host: a } | Self::Incoming { joiner: a } => put_addr(&mut b, a),
            Self::Refused(r) => b.push(r as u8),
            Self::Unregister | Self::Punch | Self::Unlist => {}
            Self::Publish { listing } => b.extend_from_slice(listing),
            Self::List { start } => {
                b.extend_from_slice(&start.to_le_bytes());
                b.resize(LIST_LEN, 0);
            }
            Self::Rooms { total, start, entries } => {
                b.extend_from_slice(&total.to_le_bytes());
                b.extend_from_slice(&start.to_le_bytes());
                b.extend_from_slice(entries);
            }
            Self::To { peer, data } | Self::From { peer, data } => {
                put_addr(&mut b, peer);
                b.extend_from_slice(data);
            }
        }
        b
    }

    /// A packet of this protocol, or `None` (another version, a wrong length, not ours).
    pub fn decode(b: &'a [u8]) -> Option<Self> {
        if b.len() < HEAD || &b[..4] != MAGIC || b[4] != VERSION {
            return None;
        }
        let body = &b[HEAD..];
        let room = || (body.len() == 8 + PIN_BYTES).then(|| (u64::from_le_bytes(body[..8].try_into().unwrap()), body[8..].try_into().unwrap()));
        let at = || (body.len() == 6).then(|| addr(body));
        Some(match b[5] {
            1 => room().map(|(session, pin)| Self::Register { session, pin })?,
            2 => Self::Registered { seen: at()? },
            3 => room().map(|(session, pin)| Self::Lookup { session, pin })?,
            4 => Self::Found { host: at()? },
            5 => Self::Incoming { joiner: at()? },
            6 => Self::Refused(match body {
                [1] => Refusal::Unknown,
                [2] => Refusal::Taken,
                [3] => Refusal::Full,
                _ => return None,
            }),
            7 if body.is_empty() => Self::Unregister,
            8 if body.is_empty() => Self::Punch,
            9 | 10 if body.len() >= 6 => {
                let (peer, data) = (addr(body), &body[6..]);
                if b[5] == 9 { Self::To { peer, data } } else { Self::From { peer, data } }
            }
            11 if !body.is_empty() && body.len() <= MAX_LISTING => Self::Publish { listing: body },
            12 if body.is_empty() => Self::Unlist,
            13 if b.len() == LIST_LEN && body[2..].iter().all(|&x| x == 0) => Self::List { start: u16::from_le_bytes([body[0], body[1]]) },
            14 if body.len() >= 4 && b.len() <= LIST_LEN => {
                Self::Rooms { total: u16::from_le_bytes([body[0], body[1]]), start: u16::from_le_bytes([body[2], body[3]]), entries: &body[4..] }
            }
            _ => return None,
        })
    }
}

/// The service's abuse limits (ADR 0018 §5).
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_rooms: usize,
    pub rooms_per_ip: usize,
    pub peers_per_room: usize,
    /// A room without a Register or relayed packet from its host for this long is gone.
    pub room_ttl: Duration,
    /// A relayed peer without a packet for this long is dropped from its room.
    pub peer_ttl: Duration,
    /// Control packets per second per source IP, and the burst.
    pub control_rate: f64,
    pub control_burst: f64,
    /// Relayed bytes per second per room, and the burst.
    pub room_rate: f64,
    pub room_burst: f64,
    /// A public room without a Publish for this long leaves the list (the host repeats it every 5 s).
    pub list_ttl: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_rooms: 1024,
            rooms_per_ip: 4,
            peers_per_room: 8,
            room_ttl: Duration::from_secs(30),
            peer_ttl: Duration::from_secs(60),
            control_rate: 20.0,
            control_burst: 40.0,
            room_rate: 256.0 * 1024.0,
            room_burst: 1024.0 * 1024.0,
            list_ttl: Duration::from_secs(15),
        }
    }
}

/// A token bucket.
#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f64,
    at: Duration,
}

impl Bucket {
    fn full(burst: f64, now: Duration) -> Self {
        Self { tokens: burst, at: now }
    }

    fn take(&mut self, n: f64, rate: f64, burst: f64, now: Duration) -> bool {
        self.tokens = (self.tokens + rate * now.saturating_sub(self.at).as_secs_f64()).min(burst);
        self.at = now;
        let ok = self.tokens >= n;
        if ok {
            self.tokens -= n;
        }
        ok
    }
}

#[derive(Debug)]
struct Room {
    host: SocketAddrV4,
    pin: [u8; PIN_BYTES],
    seen: Duration,
    /// Relayed peers (they looked the room up) and when each last sent.
    peers: HashMap<SocketAddrV4, Duration>,
    bytes: Bucket,
    /// On the public list: the encoded [`Listing`] and when it was last published.
    listed: Option<(Vec<u8>, Duration)>,
}

/// What the service did, for its status line.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub relayed_packets: u64,
    pub relayed_bytes: u64,
    pub dropped: u64,
}

/// The service's state: rooms by session, and who may relay to whom.
#[derive(Debug, Default)]
pub struct Service {
    pub limits: Limits,
    rooms: HashMap<u64, Room>,
    hosts: HashMap<SocketAddrV4, u64>,
    peers: HashMap<SocketAddrV4, u64>,
    control: HashMap<Ipv4Addr, Bucket>,
    pub counts: Counts,
}

impl Service {
    pub fn new(limits: Limits) -> Self {
        Self { limits, ..Self::default() }
    }

    pub fn rooms(&self) -> usize {
        self.rooms.len()
    }

    fn close(&mut self, session: u64) {
        if let Some(r) = self.rooms.remove(&session) {
            self.hosts.remove(&r.host);
            for p in r.peers.keys() {
                self.peers.remove(p);
            }
        }
    }

    /// One datagram from `from` at `now`: the datagrams to send go to `out`.
    pub fn handle(&mut self, from: SocketAddrV4, b: &[u8], now: Duration, out: &mut Vec<(SocketAddrV4, Vec<u8>)>) {
        let Some(p) = Packet::decode(b) else {
            self.counts.dropped += 1;
            return;
        };
        let l = self.limits;
        if let Packet::To { peer, data } = p {
            // A host to one of its peers, or a peer to its room's host.
            let session = self.hosts.get(&from).or(self.peers.get(&from)).copied();
            let sent = match session.and_then(|s| self.rooms.get_mut(&s)) {
                Some(r) if (r.host == from && r.peers.contains_key(&peer)) || (r.host == peer && r.peers.contains_key(&from)) => {
                    if r.host == from {
                        r.seen = now;
                    } else {
                        r.peers.insert(from, now);
                    }
                    r.bytes.take(data.len() as f64, l.room_rate, l.room_burst, now)
                }
                _ => false,
            };
            if sent {
                self.counts.relayed_packets += 1;
                self.counts.relayed_bytes += data.len() as u64;
                out.push((peer, Packet::From { peer: from, data }.encode()));
            } else {
                self.counts.dropped += 1;
            }
            return;
        }
        let bucket = self.control.entry(*from.ip()).or_insert_with(|| Bucket::full(l.control_burst, now));
        if !bucket.take(1.0, l.control_rate, l.control_burst, now) {
            self.counts.dropped += 1;
            return;
        }
        match p {
            Packet::Register { session, pin } => {
                if let Some(&old) = self.hosts.get(&from).filter(|&&s| s != session) {
                    self.close(old);
                }
                let full = self.rooms.len() >= l.max_rooms || self.rooms.values().filter(|r| r.host.ip() == from.ip()).count() >= l.rooms_per_ip;
                let reply = match self.rooms.get_mut(&session) {
                    Some(r) if r.host == from && r.pin == pin => {
                        r.seen = now;
                        Packet::Registered { seen: from }
                    }
                    Some(_) => Packet::Refused(Refusal::Taken),
                    None if full => Packet::Refused(Refusal::Full),
                    None => {
                        self.rooms.insert(session, Room { host: from, pin, seen: now, peers: HashMap::new(), bytes: Bucket::full(l.room_burst, now), listed: None });
                        self.hosts.insert(from, session);
                        Packet::Registered { seen: from }
                    }
                };
                out.push((from, reply.encode()));
            }
            Packet::Unregister => {
                if let Some(s) = self.hosts.get(&from).copied() {
                    self.close(s);
                }
            }
            Packet::Lookup { session, pin } => {
                let Some(r) = self.rooms.get(&session).filter(|r| r.pin == pin && r.host != from) else {
                    out.push((from, Packet::Refused(Refusal::Unknown).encode()));
                    return;
                };
                if !r.peers.contains_key(&from) && r.peers.len() >= l.peers_per_room {
                    out.push((from, Packet::Refused(Refusal::Full).encode()));
                    return;
                }
                // A peer is in one room at a time.
                if let Some(old) = self.peers.insert(from, session).filter(|&s| s != session) {
                    if let Some(o) = self.rooms.get_mut(&old) {
                        o.peers.remove(&from);
                    }
                }
                let r = self.rooms.get_mut(&session).expect("looked up");
                r.peers.insert(from, now);
                out.push((from, Packet::Found { host: r.host }.encode()));
                out.push((r.host, Packet::Incoming { joiner: from }.encode()));
            }
            Packet::Publish { listing } => {
                let session = self.hosts.get(&from).copied();
                match (session.and_then(|s| self.rooms.get_mut(&s)), Listing::decode(listing)) {
                    // Only the room's own host lists it, with its own session and certificate.
                    (Some(r), Some(li)) if session == Some(li.announcement.session) && li.announcement.fingerprint.starts_with(&r.pin) => {
                        r.seen = now;
                        r.listed = Some((li.encode(), now));
                    }
                    _ => self.counts.dropped += 1,
                }
            }
            Packet::Unlist => {
                if let Some(r) = self.hosts.get(&from).and_then(|s| self.rooms.get_mut(s)) {
                    r.listed = None;
                }
            }
            Packet::List { start } => out.push((from, self.page(start, now))),
            // Answers and punches are not for the service.
            _ => self.counts.dropped += 1,
        }
    }

    /// The public rooms' hosts and encoded listings, by session id (a stable order for paging).
    pub fn listed(&self, now: Duration) -> Vec<(SocketAddrV4, &[u8])> {
        let mut v: Vec<(u64, SocketAddrV4, &[u8])> = self
            .rooms
            .iter()
            .filter_map(|(&s, r)| r.listed.as_ref().filter(|(_, at)| now.saturating_sub(*at) <= self.limits.list_ttl).map(|(b, _)| (s, r.host, b.as_slice())))
            .collect();
        v.sort_unstable_by_key(|e| e.0);
        v.into_iter().map(|(_, h, b)| (h, b)).collect()
    }

    /// The Rooms answer from entry `start` on: as many entries as fit in [`LIST_LEN`].
    fn page(&self, start: u16, now: Duration) -> Vec<u8> {
        let all = self.listed(now);
        let mut entries = Vec::new();
        for (host, b) in all.iter().skip(start as usize) {
            if HEAD + 4 + entries.len() + 8 + b.len() > LIST_LEN {
                break;
            }
            put_addr(&mut entries, *host);
            entries.extend_from_slice(&(b.len() as u16).to_le_bytes());
            entries.extend_from_slice(b);
        }
        Packet::Rooms { total: all.len().min(u16::MAX as usize) as u16, start, entries: &entries }.encode()
    }

    /// Drop expired rooms and peers and idle rate buckets (call about once a second).
    pub fn sweep(&mut self, now: Duration) {
        let l = self.limits;
        let gone: Vec<u64> = self.rooms.iter().filter(|(_, r)| now.saturating_sub(r.seen) > l.room_ttl).map(|(&s, _)| s).collect();
        for s in gone {
            self.close(s);
        }
        let peers = &mut self.peers;
        for r in self.rooms.values_mut() {
            r.peers.retain(|p, seen| {
                let keep = now.saturating_sub(*seen) <= l.peer_ttl;
                if !keep {
                    peers.remove(p);
                }
                keep
            });
        }
        // A full bucket is the same as none.
        self.control.retain(|_, b| now.saturating_sub(b.at).as_secs_f64() * l.control_rate < l.control_burst);
    }
}

/// A Rooms answer's entries: the hosts' addresses and their listings (`None`: malformed).
pub fn entries(mut b: &[u8]) -> Option<Vec<(SocketAddrV4, Listing)>> {
    let mut v = Vec::new();
    while !b.is_empty() {
        let n = u16::from_le_bytes(b.get(6..8)?.try_into().ok()?) as usize;
        v.push((addr(b), Listing::decode(b.get(8..8 + n)?)?));
        b = &b[8 + n..];
    }
    Some(v)
}

/// The public rooms at the service at `service` (blocking: up to [`MAX_PAGES`] pages, each asked up to three times
/// and waited for `wait`). An error when the first page never comes.
pub fn list(service: SocketAddrV4, wait: Duration) -> std::io::Result<Vec<(SocketAddrV4, Listing)>> {
    let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.set_read_timeout(Some(wait))?;
    let mut rooms = Vec::new();
    let mut buf = [0u8; LIST_LEN];
    'pages: for page in 0..MAX_PAGES {
        let start = rooms.len() as u16;
        for _ in 0..3 {
            socket.send_to(&Packet::List { start }.encode(), service)?;
            let deadline = web_time::Instant::now() + wait;
            while web_time::Instant::now() < deadline {
                let Ok((n, from)) = socket.recv_from(&mut buf) else { break };
                let Some(Packet::Rooms { total, start: s, entries: e }) = (from == service.into()).then(|| Packet::decode(&buf[..n])).flatten() else { continue };
                if s != start {
                    continue;
                }
                let got = entries(e).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "a malformed list"))?;
                let done = got.is_empty() || start as usize + got.len() >= total as usize;
                rooms.extend(got);
                if done {
                    break 'pages;
                }
                continue 'pages;
            }
        }
        if page == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "no answer"));
        }
        break;
    }
    Ok(rooms)
}

/// Serve on `socket` until `stop` says so (asked a few times a second, with the service for its counts). Bad
/// packets and socket errors (an ICMP-reported unreachable peer) are dropped, never fatal.
pub fn serve(socket: &std::net::UdpSocket, service: &mut Service, mut stop: impl FnMut(&Service) -> bool) -> std::io::Result<()> {
    socket.set_read_timeout(Some(Duration::from_millis(200)))?;
    let start = web_time::Instant::now();
    // A relayed datagram is at most QUIC's 1200 bytes plus the header; anything longer is not ours.
    let mut buf = [0u8; 2048];
    let mut out = Vec::new();
    let mut swept = Duration::ZERO;
    while !stop(service) {
        let now = start.elapsed();
        if let Ok((n, std::net::SocketAddr::V4(from))) = socket.recv_from(&mut buf) {
            service.handle(from, &buf[..n], now, &mut out);
        }
        for (to, b) in out.drain(..) {
            let _ = socket.send_to(&b, to);
        }
        if now.saturating_sub(swept) >= Duration::from_secs(1) {
            service.sweep(now);
            swept = now;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> SocketAddrV4 {
        s.parse().unwrap()
    }

    const PIN: [u8; PIN_BYTES] = [9; PIN_BYTES];

    fn send(s: &mut Service, from: &str, p: Packet, now: u64) -> Vec<(SocketAddrV4, Vec<u8>)> {
        let mut out = Vec::new();
        s.handle(a(from), &p.encode(), Duration::from_secs(now), &mut out);
        out
    }

    fn decoded(out: &[(SocketAddrV4, Vec<u8>)]) -> Vec<(SocketAddrV4, Packet<'_>)> {
        out.iter().map(|(to, b)| (*to, Packet::decode(b).unwrap())).collect()
    }

    #[test]
    fn packets_round_trip() {
        let data = [1u8, 2, 3];
        for p in [
            Packet::Register { session: 7, pin: PIN },
            Packet::Registered { seen: a("203.0.113.7:5000") },
            Packet::Lookup { session: u64::MAX, pin: PIN },
            Packet::Found { host: a("1.2.3.4:5") },
            Packet::Incoming { joiner: a("1.2.3.4:6") },
            Packet::Refused(Refusal::Taken),
            Packet::Unregister,
            Packet::Punch,
            Packet::To { peer: a("10.0.0.1:1"), data: &data },
            Packet::From { peer: a("10.0.0.1:1"), data: &[] },
            Packet::Publish { listing: &data },
            Packet::Unlist,
            Packet::List { start: 300 },
            Packet::Rooms { total: 9, start: 2, entries: &data },
        ] {
            assert_eq!(Packet::decode(&p.encode()), Some(p));
        }
        assert_eq!(Packet::To { peer: a("1.1.1.1:1"), data: &data }.encode().len(), DATA_HEAD + 3);
        // QUIC packets, other versions and cut packets are not ours.
        assert_eq!(Packet::decode(&[0xc0, 0, 0, 0, 1, 2, 3]), None);
        let mut b = Packet::Unregister.encode();
        b[4] = 2;
        assert_eq!(Packet::decode(&b), None);
        assert_eq!(Packet::decode(&Packet::Lookup { session: 1, pin: PIN }.encode()[..20]), None);
    }

    #[test]
    fn rooms_are_found_with_session_and_pin_and_relay_between_their_members() {
        let mut s = Service::default();
        let out = send(&mut s, "198.51.100.1:4000", Packet::Register { session: 5, pin: PIN }, 0);
        assert_eq!(decoded(&out), vec![(a("198.51.100.1:4000"), Packet::Registered { seen: a("198.51.100.1:4000") })]);
        // Another address can't take the session; a wrong pin finds nothing.
        assert_eq!(decoded(&send(&mut s, "198.51.100.9:4000", Packet::Register { session: 5, pin: PIN }, 0))[0].1, Packet::Refused(Refusal::Taken));
        assert_eq!(decoded(&send(&mut s, "192.0.2.5:7000", Packet::Lookup { session: 5, pin: [1; PIN_BYTES] }, 0))[0].1, Packet::Refused(Refusal::Unknown));
        // Nobody relays before a lookup.
        assert!(send(&mut s, "192.0.2.5:7000", Packet::To { peer: a("198.51.100.1:4000"), data: b"x" }, 0).is_empty());
        let out = send(&mut s, "192.0.2.5:7000", Packet::Lookup { session: 5, pin: PIN }, 0);
        assert_eq!(decoded(&out), vec![(a("192.0.2.5:7000"), Packet::Found { host: a("198.51.100.1:4000") }), (a("198.51.100.1:4000"), Packet::Incoming { joiner: a("192.0.2.5:7000") })]);
        let out = send(&mut s, "192.0.2.5:7000", Packet::To { peer: a("198.51.100.1:4000"), data: b"hello" }, 1);
        assert_eq!(decoded(&out), vec![(a("198.51.100.1:4000"), Packet::From { peer: a("192.0.2.5:7000"), data: b"hello" })]);
        let out = send(&mut s, "198.51.100.1:4000", Packet::To { peer: a("192.0.2.5:7000"), data: b"back" }, 1);
        assert_eq!(decoded(&out), vec![(a("192.0.2.5:7000"), Packet::From { peer: a("198.51.100.1:4000"), data: b"back" })]);
        // Not to anybody else.
        assert!(send(&mut s, "198.51.100.1:4000", Packet::To { peer: a("8.8.8.8:53"), data: b"x" }, 1).is_empty());
        assert!(send(&mut s, "192.0.2.5:7000", Packet::To { peer: a("8.8.8.8:53"), data: b"x" }, 1).is_empty());
        assert_eq!(s.counts.relayed_packets, 2);
        // Unregister closes the room and its relaying.
        send(&mut s, "198.51.100.1:4000", Packet::Unregister, 2);
        assert_eq!(s.rooms(), 0);
        assert!(send(&mut s, "192.0.2.5:7000", Packet::To { peer: a("198.51.100.1:4000"), data: b"x" }, 2).is_empty());
    }

    #[test]
    fn rooms_and_peers_expire_without_traffic() {
        let mut s = Service::default();
        send(&mut s, "198.51.100.1:4000", Packet::Register { session: 5, pin: PIN }, 0);
        send(&mut s, "192.0.2.5:7000", Packet::Lookup { session: 5, pin: PIN }, 0);
        // Refreshed at 20 s, the room outlives 30 s; the silent peer goes after 60.
        send(&mut s, "198.51.100.1:4000", Packet::Register { session: 5, pin: PIN }, 20);
        s.sweep(Duration::from_secs(45));
        assert_eq!(s.rooms(), 1);
        send(&mut s, "198.51.100.1:4000", Packet::Register { session: 5, pin: PIN }, 50);
        s.sweep(Duration::from_secs(61));
        assert!(send(&mut s, "198.51.100.1:4000", Packet::To { peer: a("192.0.2.5:7000"), data: b"x" }, 61).is_empty(), "peer expired");
        s.sweep(Duration::from_secs(81));
        assert_eq!(s.rooms(), 0, "no refresh for 31 s");
        // Expired: the session is free for anyone.
        assert_eq!(decoded(&send(&mut s, "198.51.100.9:1", Packet::Register { session: 5, pin: PIN }, 82))[0].1, Packet::Registered { seen: a("198.51.100.9:1") });
    }

    fn listing(session: u64, name: &str) -> Listing {
        let mut fingerprint = [7; 32];
        fingerprint[..PIN_BYTES].copy_from_slice(&PIN);
        Listing {
            announcement: Announcement { session, name: name.into(), port: 29171, fingerprint, password: false, version: 11, rules: [3; 32], scenario: "galaxy".into(), humans: 1, slots: 8, started: false },
            mods: [0; 32],
        }
    }

    fn rooms(s: &mut Service, from: &str, start: u16, now: u64) -> (u16, Vec<(SocketAddrV4, Listing)>) {
        let out = send(s, from, Packet::List { start }, now);
        assert_eq!(out.len(), 1);
        assert!(out[0].1.len() <= LIST_LEN, "never larger than the request");
        match Packet::decode(&out[0].1) {
            Some(Packet::Rooms { total, start: s, entries: e }) if s == start => (total, entries(e).unwrap()),
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn public_rooms_are_listed_by_their_hosts_until_they_stop() {
        let mut s = Service::default();
        let l = listing(5, "Alice's galaxy");
        let host = "198.51.100.1:4000";
        // Not without a room, nor for another session or certificate.
        send(&mut s, host, Packet::Publish { listing: &l.encode() }, 0);
        send(&mut s, host, Packet::Register { session: 5, pin: PIN }, 0);
        send(&mut s, host, Packet::Publish { listing: &listing(6, "x").encode() }, 0);
        let other = Listing { announcement: Announcement { fingerprint: [1; 32], ..l.announcement.clone() }, ..l.clone() };
        send(&mut s, host, Packet::Publish { listing: &other.encode() }, 0);
        // Nor by somebody else for this room.
        send(&mut s, "192.0.2.5:7000", Packet::Publish { listing: &l.encode() }, 0);
        assert_eq!(rooms(&mut s, "192.0.2.5:7000", 0, 0), (0, vec![]));
        // A room without a listing is private: unlisted.
        send(&mut s, host, Packet::Publish { listing: &l.encode() }, 1);
        assert_eq!(rooms(&mut s, "192.0.2.5:7000", 0, 1), (1, vec![(a(host), l.clone())]));
        let code = l.join_code(a(host));
        assert_eq!((code.session, code.pin, code.addrs.as_slice()), (5, PIN, &[a(host)][..]), "a click's join code");
        // Unlist takes it off now; a listing not repeated for 15 s goes too (the room stays).
        send(&mut s, host, Packet::Unlist, 2);
        assert_eq!(rooms(&mut s, "192.0.2.5:7000", 0, 2).0, 0);
        send(&mut s, host, Packet::Publish { listing: &l.encode() }, 3);
        send(&mut s, host, Packet::Register { session: 5, pin: PIN }, 18);
        assert_eq!(rooms(&mut s, "192.0.2.5:7000", 0, 18).0, 1);
        assert_eq!(rooms(&mut s, "192.0.2.5:7000", 0, 19).0, 0, "stale");
        // A closed room is off the list.
        send(&mut s, host, Packet::Publish { listing: &l.encode() }, 20);
        send(&mut s, host, Packet::Unregister, 20);
        assert_eq!(rooms(&mut s, "192.0.2.5:7000", 0, 20).0, 0);
        // An unpadded List is not answered.
        let mut short = Packet::List { start: 0 }.encode();
        short.truncate(100);
        let mut out = Vec::new();
        s.handle(a("192.0.2.5:7000"), &short, Duration::from_secs(20), &mut out);
        assert!(out.is_empty(), "no answer to an unpadded list");
    }

    #[test]
    fn the_list_comes_in_pages() {
        let limits = Limits { rooms_per_ip: 64, control_burst: 1000.0, ..Limits::default() };
        let mut s = Service::new(limits);
        let long = "Ǆ".repeat(MAX_NAME);
        let n = 40u64;
        for k in 0..n {
            let host = format!("198.51.100.1:{}", 1000 + k);
            send(&mut s, &host, Packet::Register { session: k, pin: PIN }, 0);
            let mut l = listing(k, &long);
            l.announcement.scenario = long.clone();
            assert!(l.encode().len() <= MAX_LISTING, "{}", l.encode().len());
            send(&mut s, &host, Packet::Publish { listing: &l.encode() }, 0);
        }
        let mut got = Vec::new();
        while got.len() < n as usize {
            let (total, page) = rooms(&mut s, "192.0.2.5:7000", got.len() as u16, 0);
            assert_eq!(total as u64, n);
            assert!(!page.is_empty());
            got.extend(page);
        }
        let sessions: Vec<u64> = got.iter().map(|(_, l)| l.announcement.session).collect();
        assert_eq!(sessions, (0..n).collect::<Vec<_>>(), "by session, each once");
        assert_eq!(rooms(&mut s, "192.0.2.5:7000", n as u16, 0), (n as u16, vec![]));
        // An over-long name is refused.
        let mut l = listing(1, &"x".repeat(MAX_NAME + 1));
        l.announcement.session = 1;
        assert_eq!(Listing::decode(&l.encode()), None);
    }

    /// The blocking browser against a served socket on 127.0.0.1.
    #[test]
    fn the_browser_pages_through_a_running_service() {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let std::net::SocketAddr::V4(at) = socket.local_addr().unwrap() else { unreachable!() };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut s = Service::new(Limits { rooms_per_ip: 64, ..Limits::default() });
            for k in 0..20u64 {
                let host = a(&format!("198.51.100.1:{}", 1000 + k));
                let mut out = Vec::new();
                s.handle(host, &Packet::Register { session: k, pin: PIN }.encode(), Duration::ZERO, &mut out);
                s.handle(host, &Packet::Publish { listing: &listing(k, &format!("Room {k}")).encode() }.encode(), Duration::ZERO, &mut out);
            }
            serve(&socket, &mut s, |_| rx.try_recv().is_ok())
        });
        let got = list(at, Duration::from_millis(500)).unwrap();
        assert_eq!(got.len(), 20);
        assert_eq!(got[19].1.announcement.name, "Room 19");
        tx.send(()).unwrap();
        // Nobody there: an error, not an empty list.
        let dead = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let std::net::SocketAddr::V4(nobody) = dead.local_addr().unwrap() else { unreachable!() };
        drop(dead);
        assert!(list(nobody, Duration::from_millis(100)).is_err());
    }

    #[test]
    fn limits_hold() {
        let limits = Limits { max_rooms: 3, rooms_per_ip: 2, peers_per_room: 1, room_rate: 1000.0, room_burst: 2000.0, ..Limits::default() };
        let mut s = Service::new(limits);
        let reg = |s: &mut Service, from: &str, session| send(s, from, Packet::Register { session, pin: PIN }, 0).remove(0).1;
        let ok = |from: &str| Packet::Registered { seen: a(from) }.encode();
        let full = Packet::Refused(Refusal::Full).encode();
        assert_eq!(reg(&mut s, "198.51.100.1:1", 1), ok("198.51.100.1:1"));
        assert_eq!(reg(&mut s, "198.51.100.1:2", 2), ok("198.51.100.1:2"));
        assert_eq!(reg(&mut s, "198.51.100.1:3", 3), full, "two rooms per IP");
        assert_eq!(reg(&mut s, "198.51.100.2:1", 3), ok("198.51.100.2:1"));
        assert_eq!(reg(&mut s, "198.51.100.3:1", 4), full, "three rooms in all");
        // One peer per room.
        assert_eq!(decoded(&send(&mut s, "192.0.2.1:1", Packet::Lookup { session: 1, pin: PIN }, 0))[0].1, Packet::Found { host: a("198.51.100.1:1") });
        assert_eq!(decoded(&send(&mut s, "192.0.2.2:1", Packet::Lookup { session: 1, pin: PIN }, 0))[0].1, Packet::Refused(Refusal::Full));
        // Bandwidth: a 2000-byte burst, then 1000 bytes a second.
        let big = vec![0u8; 900];
        let to = |s: &mut Service, t: u64| send(s, "192.0.2.1:1", Packet::To { peer: a("198.51.100.1:1"), data: &big }, t).len();
        assert_eq!((to(&mut s, 0), to(&mut s, 0), to(&mut s, 0)), (1, 1, 0));
        assert_eq!(to(&mut s, 1), 1, "refilled");
        // Control packets: a burst of 40, then 20 a second.
        let mut s = Service::default();
        let n = (0..100).filter(|_| !send(&mut s, "192.0.2.7:1", Packet::Lookup { session: 1, pin: PIN }, 0).is_empty()).count();
        assert_eq!(n, 40);
        assert_eq!((0..100).filter(|_| !send(&mut s, "192.0.2.7:1", Packet::Lookup { session: 1, pin: PIN }, 1).is_empty()).count(), 20);
        // Answers never outgrow their requests (no amplification).
        assert!(Packet::Found { host: a("1.2.3.4:5") }.encode().len() < Packet::Lookup { session: 1, pin: PIN }.encode().len());
        assert!(Packet::Registered { seen: a("1.2.3.4:5") }.encode().len() < Packet::Register { session: 1, pin: PIN }.encode().len());
    }
}
