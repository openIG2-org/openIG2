//! The QUIC transport on 127.0.0.1 (feature `quic`): messages both ways, a wrong pinned certificate refused, and
//! two peers in lockstep over real UDP from `_local/saves/quick-1.sav` with identical hashes.
#![cfg(feature = "quic")]

mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use ig2_net::quic::{connect, connect_pinned, Pin, QuicHost, QuicLink};
use ig2_net::{Bundle, Command, Host, Msg, Peer, Transport, DEFAULT_DELAY};
use ig2_sim::game::Game;

fn localhost() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

/// Wait for `f` to return something (up to 10 s).
fn wait<T>(mut f: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn recv(link: &mut QuicLink) -> Msg {
    wait(|| link.recv().unwrap())
}

#[test]
fn messages_cross_both_ways() {
    let host = QuicHost::bind(localhost()).unwrap();
    let addr = host.local_addr().unwrap();
    let mut joiner = connect(addr, host.cert()).unwrap();
    joiner.send(&Msg::Input { turn: 2, commands: vec![Command::StoryChoice { id: 7 }] });
    let mut there = wait(|| host.accept());
    assert_eq!(recv(&mut there), Msg::Input { turn: 2, commands: vec![Command::StoryChoice { id: 7 }] });
    // A big message (a snapshot-sized frame) and ordering.
    let big = Msg::Bundle(Bundle { turn: 9, commands: vec![] });
    there.send(&Msg::Hash { turn: 1, hash: 42 });
    there.send(&big);
    there.send(&Msg::Input { turn: 3, commands: vec![Command::Buy { delivery: vec![0xab; 3 << 20] }] });
    assert_eq!(recv(&mut joiner), Msg::Hash { turn: 1, hash: 42 });
    assert_eq!(recv(&mut joiner), big);
    let Msg::Input { commands, .. } = recv(&mut joiner) else { panic!() };
    assert_eq!(commands, vec![Command::Buy { delivery: vec![0xab; 3 << 20] }]);
    there.close();
    wait(|| joiner.is_closed().then_some(()));
}

#[test]
fn fingerprint_pins_and_the_channel_binding_match() {
    let host = QuicHost::bind(localhost()).unwrap();
    let other = QuicHost::bind(localhost()).unwrap();
    let addr = host.local_addr().unwrap();
    assert!(connect_pinned(addr, &Pin::Fingerprint(other.fingerprint())).is_err());
    let joiner = connect_pinned(addr, &Pin::Fingerprint(host.fingerprint())).unwrap();
    let there = wait(|| host.accept());
    assert_eq!(joiner.binding(), there.binding());
    // An unpinned join connects too, on a different TLS session (different binding).
    let open = connect_pinned(addr, &Pin::Any).unwrap();
    let there2 = wait(|| host.accept());
    assert_eq!(open.binding(), there2.binding());
    assert_ne!(open.binding(), joiner.binding());
}

#[test]
fn a_wrong_certificate_is_refused() {
    let host = QuicHost::bind(localhost()).unwrap();
    let other = QuicHost::bind(localhost()).unwrap();
    assert!(connect(host.local_addr().unwrap(), other.cert()).is_err());
}

#[test]
fn two_peers_in_lockstep_over_udp() {
    let Some((rules, state)) = common::setup("quick-1.sav") else { return };
    let net = QuicHost::bind(localhost()).unwrap();
    let addr = net.local_addr().unwrap();
    let mut host = Host::new(DEFAULT_DELAY);
    let mut peers = Vec::new();
    for (slot, empire) in [(1u8, 6u8), (2, 3)] {
        let link = connect(addr, net.cert()).unwrap();
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
    // The last hashes reach the host too.
    wait(|| {
        host.pump().unwrap();
        (host.log.len() as u64 >= turns).then_some(())
    });
    let (a, b) = (&peers[0].0, &peers[1].0);
    assert_eq!(a.hashes.len() as u64, turns);
    assert_eq!(a.hashes, b.hashes);
    assert!(host.desyncs.is_empty(), "{:?}", host.desyncs);
    assert_eq!(host.log.iter().map(|b| b.commands.len()).sum::<usize>(), 1);
}

#[test]
fn discover_then_join_a_password_lobby_over_quic() {
    use ig2_net::discovery::{Announcer, Browser};
    use ig2_net::lobby::{LobbyClient, LobbyHost, Settings, Status};

    let rules = [3u8; 32];
    let server = QuicHost::bind(localhost()).unwrap();
    let settings = Settings { name: "LAN game".into(), scenario: "quick".into(), private: false, password: Some("pw".into()) };
    let mut lobby = LobbyHost::new(settings, 77, rules, &[(1, 0), (6, 5)], "Host");
    let mut announcer = Announcer::bind(localhost()).unwrap();
    announcer.set(lobby.announcement(server.local_addr().unwrap().port(), server.fingerprint()));
    let mut browser = Browser::with_targets(localhost(), vec![announcer.local_addr().unwrap()]).unwrap();
    browser.query().unwrap();
    let found = wait(|| {
        announcer.poll().unwrap();
        browser.poll().unwrap();
        browser.sessions().next().cloned()
    });
    assert!(found.announcement.password);
    assert_eq!(found.addr, server.local_addr().unwrap());

    // The wrong password, then the right one; each over its own pinned connection.
    for (pw, want) in [("nope", Status::Refused(ig2_net::lobby::Refusal::WrongPassword)), ("pw", Status::InLobby)] {
        let link = connect_pinned(found.addr, &Pin::Fingerprint(found.announcement.fingerprint)).unwrap();
        let mut c = LobbyClient::join(Box::new(link), "Joiner", rules, Some(pw.into()), None);
        let got = wait(|| {
            if let Some(l) = server.accept() {
                lobby.admit(Box::new(l));
            }
            lobby.pump().unwrap();
            let s = c.pump().unwrap().clone();
            (s != Status::Joining).then_some(s)
        });
        assert_eq!(got, want);
    }
    assert_eq!(lobby.state().members.len(), 2);
}
