//! The WebRTC transport on 127.0.0.1 (features `quic` and `webrtc`, ADR 0047 part 4): a joiner offers through the
//! rendezvous service's HTTP side as a browser does, the host answers through its QUIC socket's room, the data
//! channel carries messages both ways (one far bigger than a piece), and two such peers play lockstep from
//! `_local/saves/quick-1.sav` with identical hashes.
#![cfg(all(feature = "quic", feature = "webrtc"))]

mod common;

use std::net::{SocketAddr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ig2_net::joincode::PIN_BYTES;
use ig2_net::quic::{QuicHost, Rendezvous};
use ig2_net::rendezvous::{serve_with_http, Service};
use ig2_net::webrtc::{self, Link};
use ig2_net::{Command, Host, Msg, Peer, Transport, DEFAULT_DELAY};
use ig2_sim::game::Game;

fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(start.elapsed() < Duration::from_secs(30), "timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A service on one loopback port, UDP and TCP; stops when dropped.
struct Relay {
    addr: SocketAddrV4,
    stop: Arc<AtomicBool>,
}

impl Relay {
    fn start() -> Self {
        // The free TCP port's UDP twin may be taken or reserved (Windows: os error 10013): try another pair.
        let (tcp, addr, socket) = (0..50)
            .find_map(|_| {
                let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let SocketAddr::V4(addr) = tcp.local_addr().unwrap() else { unreachable!() };
                std::net::UdpSocket::bind(addr).ok().map(|s| (tcp, addr, s))
            })
            .expect("a loopback port free for TCP and UDP");
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        std::thread::spawn(move || serve_with_http(&socket, Some(tcp), &mut Service::default(), |_| s.load(Ordering::Relaxed)));
        Self { addr, stop }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn host_on(relay: &Relay, session: u64) -> (QuicHost, [u8; PIN_BYTES]) {
    let host = QuicHost::bind_relayed("127.0.0.1:0".parse().unwrap(), relay.addr).unwrap();
    host.register(session);
    wait(|| matches!(host.rendezvous(), Rendezvous::Registered(_)).then_some(()));
    let mut pin = [0; PIN_BYTES];
    pin.copy_from_slice(&host.fingerprint()[..PIN_BYTES]);
    (host, pin)
}

/// A joiner's link and the host's end of it: the joiner offers on a thread while the host answers.
fn join(relay: &Relay, host: &QuicHost, session: u64, pin: [u8; PIN_BYTES]) -> (Link, Link) {
    let at = relay.addr;
    let joiner = std::thread::spawn(move || webrtc::connect(at, session, &pin));
    let mut pending = Vec::new();
    let there = wait(|| {
        for (id, offer) in host.offers() {
            let (answer, link) = webrtc::answer(&offer).unwrap();
            host.answer(id, &answer);
            pending.push(link);
        }
        pending.iter().position(|l| l.is_open()).map(|i| pending.swap_remove(i))
    });
    (joiner.join().unwrap().unwrap(), there)
}

#[test]
fn a_joiner_reaches_the_host_over_a_data_channel() {
    let relay = Relay::start();
    let (host, pin) = host_on(&relay, 0x5eed);
    let (mut joiner, mut there) = join(&relay, &host, 0x5eed, pin);
    assert_eq!(joiner.binding(), there.binding());
    assert_ne!(joiner.binding(), [0; 32]);
    joiner.send(&Msg::Hash { turn: 1, hash: 7 });
    assert_eq!(wait(|| there.recv().unwrap()), Msg::Hash { turn: 1, hash: 7 });
    let big = Msg::Input { turn: 3, commands: vec![Command::Buy { delivery: vec![0xab; 300 << 10] }] };
    there.send(&big);
    assert_eq!(wait(|| joiner.recv().unwrap()), big);
    // A wrong pin finds no room; the link closes with its other end.
    let mut wrong = pin;
    wrong[0] ^= 1;
    assert!(webrtc::connect(relay.addr, 0x5eed, &wrong).is_err());
    drop(there);
    wait(|| joiner.closed().then_some(()));
}

#[test]
fn two_peers_in_lockstep_over_webrtc() {
    let Some((rules, state)) = common::setup("quick-1.sav") else { return };
    let relay = Relay::start();
    let (net, pin) = host_on(&relay, 42);
    let mut host = Host::new(DEFAULT_DELAY);
    let mut peers = Vec::new();
    for (slot, empire) in [(1u8, 6u8), (2, 3)] {
        let (link, there) = join(&relay, &net, 42, pin);
        host.add_peer(slot, empire, Box::new(there));
        let mut g = Game::new(state.clone(), &rules);
        g.saved();
        peers.push((Peer::new(slot, empire, g, DEFAULT_DELAY), link));
    }
    let turns = 30;
    let mut issued = false;
    wait(|| {
        host.pump();
        for (p, link) in &mut peers {
            if p.turn >= 4 && p.slot == 1 && !issued {
                p.issue(Command::StoryChoice { id: 10031 });
                issued = true;
            }
            p.poll(link, (turns - p.turn.min(turns)) as usize).unwrap();
        }
        peers.iter().all(|(p, _)| p.turn >= turns).then_some(())
    });
    wait(|| {
        host.pump();
        (host.log.len() as u64 >= turns).then_some(())
    });
    let (a, b) = (&peers[0].0, &peers[1].0);
    assert_eq!(a.hashes.len() as u64, turns);
    assert_eq!(a.hashes, b.hashes);
    assert!(host.desyncs.is_empty(), "{:?}", host.desyncs);
    assert_eq!(host.log.iter().map(|b| b.commands.len()).sum::<usize>(), 1);
}
