//! The rendezvous and relay service (ADR 0018): its packets and its state, without sockets. The `ig2-rendezvous`
//! binary feeds it datagrams; the client side is `quic::RelaySocket`.
//!
//! ```text
//! host ── Register{session, pin} ──▶ service ── Registered{public addr} ──▶ host
//! joiner ─ Lookup{session, pin} ──▶ service ── Found{host} ──▶ joiner, Incoming{joiner} ──▶ host (punches)
//! relayed: peer ─ To{dest, QUIC datagram} ─▶ service ─ From{src, datagram} ─▶ dest
//! ```

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

use crate::joincode::PIN_BYTES;

/// Every packet's first bytes.
pub const MAGIC: &[u8; 4] = b"IG2R";
pub const VERSION: u8 = 1;
/// The service's default UDP port (after the LAN discovery and game ports).
pub const DEFAULT_PORT: u16 = crate::discovery::DISCOVERY_PORT + 2;
/// Magic, version, kind.
const HEAD: usize = 6;
/// The header in front of a relayed datagram (head and an address).
pub const DATA_HEAD: usize = HEAD + 6;

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
            Self::Unregister | Self::Punch => {}
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
                        self.rooms.insert(session, Room { host: from, pin, seen: now, peers: HashMap::new(), bytes: Bucket::full(l.room_burst, now) });
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
            // Answers and punches are not for the service.
            _ => self.counts.dropped += 1,
        }
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

/// Serve on `socket` until `stop` says so (asked a few times a second, with the service for its counts). Bad
/// packets and socket errors (an ICMP-reported unreachable peer) are dropped, never fatal.
pub fn serve(socket: &std::net::UdpSocket, service: &mut Service, mut stop: impl FnMut(&Service) -> bool) -> std::io::Result<()> {
    socket.set_read_timeout(Some(Duration::from_millis(200)))?;
    let start = std::time::Instant::now();
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
