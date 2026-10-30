//! The rendezvous service on 127.0.0.1 (feature `quic`, ADR 0018): a host registers, joiners find it by session
//! and pin, by the punch path and relay-only, and two peers play lockstep from `_local/saves/quick-1.sav` through the
//! relay alone with identical hashes.
#![cfg(feature = "quic")]

mod common;

use std::net::{SocketAddr, SocketAddrV4};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ig2_net::joincode::PIN_BYTES;
use ig2_net::quic::{connect_via, QuicHost, QuicLink, Rendezvous, Via};
use ig2_net::rendezvous::{serve, Counts, Service};
use ig2_net::{Command, Host, Msg, Peer, Transport, DEFAULT_DELAY};
use ig2_sim::game::Game;

fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A service on a loopback port, its counts, and its stop switch (stops when dropped).
struct Relay {
    addr: SocketAddrV4,
    counts: Arc<Mutex<Counts>>,
    stop: Arc<AtomicBool>,
}

impl Relay {
    fn start() -> Self {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let SocketAddr::V4(addr) = socket.local_addr().unwrap() else { unreachable!() };
        let (counts, stop) = (Arc::new(Mutex::new(Counts::default())), Arc::new(AtomicBool::new(false)));
        let (c, s) = (counts.clone(), stop.clone());
        std::thread::spawn(move || {
            serve(&socket, &mut Service::default(), |svc| {
                *c.lock().unwrap() = svc.counts;
                s.load(Ordering::Relaxed)
            })
        });
        Self { addr, counts, stop }
    }

    fn relayed(&self) -> u64 {
        self.counts.lock().unwrap().relayed_packets
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn host_on(relay: &Relay, session: u64) -> (QuicHost, [u8; PIN_BYTES]) {
    let host = QuicHost::bind_relayed("127.0.0.1:0".parse().unwrap(), relay.addr).unwrap();
    assert_eq!(host.rendezvous(), Rendezvous::Off, "nothing is sent before the room opens");
    host.register(session);
    let seen = wait(|| match host.rendezvous() {
        Rendezvous::Registered(a) => Some(a),
        Rendezvous::Asking => None,
        r => panic!("{r:?}"),
    });
    assert_eq!(SocketAddr::V4(seen), host.local_addr().unwrap(), "the service sees the host's address");
    let mut pin = [0; PIN_BYTES];
    pin.copy_from_slice(&host.fingerprint()[..PIN_BYTES]);
    (host, pin)
}

fn exchange(joiner: &mut QuicLink, there: &mut QuicLink) {
    joiner.send(&Msg::Hash { turn: 1, hash: 7 });
    assert_eq!(wait(|| there.recv().unwrap()), Msg::Hash { turn: 1, hash: 7 });
    // A frame much bigger than one datagram, back the other way.
    let big = Msg::Input { turn: 3, commands: vec![Command::Buy { delivery: vec![0xab; 300 << 10] }] };
    there.send(&big);
    assert_eq!(wait(|| joiner.recv().unwrap()), big);
}

#[test]
fn joiners_find_the_host_by_punch_or_relay() {
    let relay = Relay::start();
    let (host, pin) = host_on(&relay, 0x5eed);
    // The punch path: on one machine the direct try always gets through, so nothing is relayed.
    let mut direct = connect_via(relay.addr, 0x5eed, pin, Via::PunchThenRelay).unwrap();
    let mut there = wait(|| host.accept());
    exchange(&mut direct, &mut there);
    assert_eq!(relay.relayed(), 0);
    // Relay-only: the same session over the service, encrypted end to end (both ends share the TLS binding).
    let mut relayed = connect_via(relay.addr, 0x5eed, pin, Via::RelayOnly).unwrap();
    let mut there = wait(|| host.accept());
    assert_eq!(relayed.binding(), there.binding());
    exchange(&mut relayed, &mut there);
    assert!(relay.relayed() > 200, "{} packets", relay.relayed());
    // A wrong pin or session finds nothing.
    let mut wrong = pin;
    wrong[0] ^= 1;
    assert!(connect_via(relay.addr, 0x5eed, wrong, Via::RelayOnly).is_err());
    assert!(connect_via(relay.addr, 0x5eee, pin, Via::RelayOnly).is_err());
    // The room closes with the host.
    drop(host);
    wait(|| relayed.closed().then_some(()));
}

#[test]
fn two_peers_in_lockstep_through_the_relay_only() {
    let Some((rules, state)) = common::setup("quick-1.sav") else { return };
    let relay = Relay::start();
    let (net, pin) = host_on(&relay, 42);
    let mut host = Host::new(DEFAULT_DELAY);
    let mut peers = Vec::new();
    for (slot, empire) in [(1u8, 6u8), (2, 3)] {
        let link = connect_via(relay.addr, 42, pin, Via::RelayOnly).unwrap();
        let there = wait(|| net.accept());
        host.add_peer(slot, empire, Box::new(there));
        let mut g = Game::new(state.clone(), &rules);
        g.saved();
        peers.push((Peer::new(slot, empire, g, DEFAULT_DELAY), link));
    }
    let turns = 30;
    let mut issued = false;
    wait(|| {
        host.pump().unwrap();
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
        host.pump().unwrap();
        (host.log.len() as u64 >= turns).then_some(())
    });
    let (a, b) = (&peers[0].0, &peers[1].0);
    assert_eq!(a.hashes.len() as u64, turns);
    assert_eq!(a.hashes, b.hashes);
    assert!(host.desyncs.is_empty(), "{:?}", host.desyncs);
    assert_eq!(host.log.iter().map(|b| b.commands.len()).sum::<usize>(), 1);
    assert!(relay.relayed() > 0);
}
