//! The generated galaxy's room over loopback links (re/screens/start.md §4, ADR 0014): seating, moves, races, the
//! host's options and AI pick, the start condition, and the start every machine builds itself, then runs in step.

#[allow(dead_code)]
mod common;

use std::time::{Duration, Instant};

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_net::galaxy::{GalaxyOptions, GalaxyStart};
use ig2_net::lobby::{LobbyClient, LobbyHost, Settings, SlotKind, Status};
use ig2_net::session::{self, ClientSession, HostSession, SessionSettings};
use ig2_net::{loopback, Peer};
use ig2_sim::game::new_game::StartData;
use ig2_sim::game::GameRules;

const RULES: [u8; 32] = [0x6b; 32];

fn settings() -> Settings {
    Settings { name: "Galaxy".into(), scenario: "galaxy".into(), private: false, password: None }
}

fn join(h: &mut LobbyHost, name: &str) -> LobbyClient {
    let (a, b) = loopback();
    h.admit(Box::new(b));
    LobbyClient::join(Box::new(a), name, RULES, None, None)
}

fn pump(h: &mut LobbyHost, clients: &mut [&mut LobbyClient], done: impl Fn(&LobbyHost, &[&mut LobbyClient]) -> bool) {
    let start = Instant::now();
    loop {
        h.pump().unwrap();
        for c in clients.iter_mut() {
            c.pump().unwrap();
        }
        if done(h, clients) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn kind(h: &LobbyHost, empire: u8) -> SlotKind {
    h.state().slots.iter().find(|s| s.empire == empire).unwrap().kind
}

/// Everyone sees the host's latest state.
fn synced(h: &LobbyHost, c: &[&mut LobbyClient]) -> bool {
    c.iter().all(|c| c.state.as_ref() == Some(h.state()))
}

#[test]
fn the_room_seats_moves_and_picks_ais_like_the_original() {
    let mut h = LobbyHost::new_galaxy(settings(), 0x1234, RULES, "Host");
    let st = h.state();
    assert_eq!(st.galaxy, Some(GalaxyOptions::default()));
    assert_eq!(st.slots.iter().map(|s| (s.empire, s.race)).collect::<Vec<_>>(), (1..=8).map(|e| (e, e)).collect::<Vec<_>>());
    assert_eq!(kind(&h, 1), SlotKind::Human(0));
    assert_eq!(st.ais().len(), 2, "Start_Init's two AIs");
    assert!(!st.ais().contains(&1));

    // No AIs: joiners take the first empty slots.
    assert!(h.set_options(GalaxyOptions { ais: 0, ..GalaxyOptions::default() }));
    assert!(h.state().ais().is_empty());
    let mut a = join(&mut h, "Alice");
    let mut b = join(&mut h, "Bob");
    pump(&mut h, &mut [&mut a, &mut b], |h, c| h.state().members.len() == 3 && synced(h, c));
    assert_eq!((kind(&h, 2), kind(&h, 3)), (SlotKind::Human(1), SlotKind::Human(2)));

    // Races: a member its own slot's, the host any.
    a.race(2, 5);
    a.race(3, 7);
    pump(&mut h, &mut [&mut a, &mut b], |h, c| h.state().slots[1].race == 5 && synced(h, c));
    assert_eq!(h.state().slots[2].race, 3, "not Alice's slot");
    assert!(h.set_race(0, 3, 8));
    assert!(!h.set_race(0, 3, 9));

    // Three AIs over the five empty slots; Bob moves onto one and the AI takes his old slot.
    assert!(h.set_options(GalaxyOptions { ais: 3, planets: 80, battle_status: 2, ..GalaxyOptions::default() }));
    let ais = h.state().ais();
    assert_eq!(ais.len(), 3);
    assert!(ais.iter().all(|e| (4..=8).contains(e)));
    let to = ais[0];
    b.pick(Some(to));
    pump(&mut h, &mut [&mut a, &mut b], |h, c| kind(h, to) == SlotKind::Human(2) && synced(h, c));
    assert_eq!(kind(&h, 3), SlotKind::Ai, "the AI moved to Bob's old slot");
    assert_eq!(h.state().slots[2].race, 8, "the race stays with the slot");
    b.pick(Some(1));
    b.pick(None);
    pump(&mut h, &mut [&mut a, &mut b], synced);
    assert_eq!(kind(&h, to), SlotKind::Human(2), "a human's slot and giving up are refused");

    // AI count above the free slots: all of them, the count recounted.
    assert!(h.set_options(GalaxyOptions { ais: 6, ..h.state().galaxy.unwrap() }));
    assert_eq!(h.state().ais().len(), 5);
    assert_eq!(h.state().galaxy.unwrap().ais, 5);

    // Start: every human ready (a change clears the flags).
    a.ready(true);
    b.ready(true);
    pump(&mut h, &mut [&mut a, &mut b], |h, _| h.state().members.iter().skip(1).all(|m| m.ready));
    assert!(h.set_ready(0, true));
    assert!(h.can_start());
    assert!(h.set_options(GalaxyOptions { shape: 2, ..h.state().galaxy.unwrap() }));
    assert!(!h.can_start());
    assert_eq!(h.state().galaxy.unwrap().planets, 80);

    // Leaving empties the slot.
    b.leave();
    pump(&mut h, &mut [&mut a], |h, _| h.state().members.len() == 2);
    assert_eq!(kind(&h, to), SlotKind::Open);
}

#[test]
fn a_lone_host_needs_an_ai_to_start() {
    let mut h = LobbyHost::new_galaxy(settings(), 7, RULES, "Host");
    assert!(h.set_options(GalaxyOptions { ais: 0, ..GalaxyOptions::default() }));
    assert!(h.set_ready(0, true));
    assert!(!h.can_start(), "one empire");
    assert!(h.set_options(GalaxyOptions { ais: 1, ..GalaxyOptions::default() }));
    assert!(h.set_ready(0, true));
    assert!(h.can_start());
}

fn started<'r>(mut c: LobbyClient, data: &StartData, rules: &'r GameRules) -> Result<ClientSession<'r>, session::Error> {
    let start = Instant::now();
    while c.status != Status::Started {
        c.pump().unwrap();
        assert!(start.elapsed() < Duration::from_secs(5), "no start");
    }
    let (s, link) = c.into_game().unwrap();
    assert!(s.snapshot.is_empty() && s.galaxy.is_some(), "the galaxy travels, not the state");
    ClientSession::new_galaxy(&s, link, data, rules)
}

fn agree(a: &Peer, b: &Peer) -> usize {
    let mut n = 0;
    for (t, h) in &b.hashes {
        if let Some((_, h0)) = a.hashes.iter().find(|(t0, _)| t0 == t) {
            assert_eq!(h, h0, "turn {t}: empire {} vs {}", b.empire, a.empire);
            n += 1;
        }
    }
    n
}

#[test]
fn every_machine_builds_the_galaxy_and_runs_in_step() {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| {
        let mut h = LobbyHost::new_galaxy(settings(), 0xBEEF, RULES, "Host");
        assert!(h.set_options(GalaxyOptions { planets: 20, ais: 1, ..GalaxyOptions::default() }));
        let mut a = join(&mut h, "Alice");
        let mut b = join(&mut h, "Bob");
        pump(&mut h, &mut [&mut a, &mut b], |h, c| h.state().members.len() == 3 && synced(h, c));
        a.race(h.state().empire_of(1).unwrap(), 4);
        a.ready(true);
        b.ready(true);
        pump(&mut h, &mut [&mut a, &mut b], |h, _| h.state().members.iter().skip(1).all(|m| m.ready));
        assert!(h.set_ready(0, true));
        let empires: Vec<u8> = (0..3).map(|m| h.state().empire_of(m).unwrap()).collect();
        let ais = h.state().ais();

        let mut hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 123_456).ok().expect("the room starts");
        let mut clients = vec![started(a, data, &rules).unwrap(), started(b, data, &rules).unwrap()];
        assert_eq!(clients.iter().map(|c| c.peer.empire).collect::<Vec<_>>(), empires[1..]);
        let m = &hs.own.game.state.mem;
        use ig2_sim::save::va;
        assert_eq!(m.i32(va::PLANET_COUNT), 20);
        assert_eq!(m.u8(va::MULTIPLAYER), 1);
        for &e in &empires {
            assert_eq!(m.u8(va::PLAYER_KI + e as u32), 1, "empire {e} is human");
        }
        assert_eq!(m.u8(va::PLAYER_FAJ + empires[1] as u32), 4, "Alice's race");
        for &e in &ais {
            assert_eq!(m.u8(va::EMPIRE_ALIVE + e as u32), 1, "AI {e} plays");
        }

        let mut now = Duration::ZERO;
        while hs.own.turn < 40 || clients.iter().any(|c| c.peer.turn < 40) {
            now += Duration::from_millis(10);
            hs.pump(now, 1).unwrap();
            hs.own.events.clear();
            for c in &mut clients {
                c.pump(now, 2).unwrap();
                c.peer.events.clear();
            }
            assert!(now < Duration::from_secs(120), "stalled at {}", hs.own.turn);
        }
        for c in &clients {
            assert!(agree(&hs.own, &c.peer) >= 40, "every turn from 0");
        }
        assert!(hs.host.desyncs.is_empty(), "{:?}", hs.host.desyncs);
    });
}

#[test]
fn a_different_start_is_refused() {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else { return };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| {
        let mut h = LobbyHost::new_galaxy(settings(), 0xF00D, RULES, "Host");
        let mut a = join(&mut h, "Alice");
        pump(&mut h, &mut [&mut a], |h, c| h.state().members.len() == 2 && synced(h, c));
        a.ready(true);
        pump(&mut h, &mut [&mut a], |h, _| h.state().members[1].ready);
        assert!(h.set_ready(0, true));
        let _hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 99).ok().unwrap();
        let start = Instant::now();
        while a.status != Status::Started {
            a.pump().unwrap();
            assert!(start.elapsed() < Duration::from_secs(5));
        }
        let (mut s, link) = a.into_game().unwrap();
        let g: &mut GalaxyStart = s.galaxy.as_mut().unwrap();
        g.hash ^= 1;
        assert!(matches!(ClientSession::new_galaxy(&s, link, data, &rules), Err(session::Error::StartMismatch { .. })));
        let (l2, _) = loopback();
        assert!(matches!(ClientSession::new(&s, Box::new(l2), &rules), Err(session::Error::StartKind(_))));
    });
}

/// `ExitToMainScreen` in multiplayer (game-end.md §2): a joiner's empire dies, only its machine's game ends; it leaves
/// and is away at once on the host; the other two run on in step; then the host leaves and the survivor sees it gone.
#[test]
fn a_lost_player_leaves_and_the_others_play_on() {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else { return };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| {
        use ig2_net::session::SessionEvent;
        use ig2_sim::game::{GameEvent, GameOver};
        let mut h = LobbyHost::new_galaxy(settings(), 0xD1E, RULES, "Host");
        assert!(h.set_options(GalaxyOptions { planets: 20, ais: 1, ..GalaxyOptions::default() }));
        let mut a = join(&mut h, "Alice");
        let mut b = join(&mut h, "Bob");
        pump(&mut h, &mut [&mut a, &mut b], |h, c| h.state().members.len() == 3 && synced(h, c));
        a.ready(true);
        b.ready(true);
        pump(&mut h, &mut [&mut a, &mut b], |h, _| h.state().members.iter().skip(1).all(|m| m.ready));
        assert!(h.set_ready(0, true));
        let mut hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 4242).ok().expect("the room starts");
        let mut clients = vec![started(a, data, &rules).unwrap(), started(b, data, &rules).unwrap()];
        let alice = clients[0].peer.empire;

        // Every machine at turn 20, then Alice's empire dies on each (as the same turn's death would).
        let mut now = Duration::ZERO;
        let step = |hs: &mut HostSession, clients: &mut [ClientSession], until: u64, now: &mut Duration| {
            while hs.own.turn < until || clients.iter().any(|c| c.peer.turn < until) {
                *now += Duration::from_millis(10);
                hs.pump(*now, (hs.own.turn < until) as usize).unwrap();
                for c in clients.iter_mut() {
                    let n = (c.peer.turn < until) as usize;
                    c.pump(*now, n).unwrap();
                }
                assert!(*now < Duration::from_secs(120), "stalled at {}", hs.own.turn);
            }
        };
        step(&mut hs, &mut clients, 20, &mut now);
        hs.own.game.take_events();
        hs.own.game.empire_dies(alice, &[]);
        for c in &mut clients {
            c.peer.game.take_events();
            c.peer.game.empire_dies(alice, &[]);
        }
        let game_over = |p: &Peer| p.game.events.iter().filter_map(|e| if let GameEvent::GameOver(o) = e { Some(*o) } else { None }).collect::<Vec<_>>();
        assert_eq!(game_over(&clients[0].peer), [GameOver::Lost], "Alice's machine");
        assert!(game_over(&hs.own).is_empty() && game_over(&clients[1].peer).is_empty(), "nobody else's");

        // Alice leaves: away at once, no hold.
        let mut alice_session = clients.remove(0);
        alice_session.leave();
        drop(alice_session);
        let mut left = false;
        while !left {
            now += Duration::from_millis(10);
            left = hs.pump(now, 0).unwrap().contains(&SessionEvent::Left { empire: alice });
            assert!(now < Duration::from_secs(130));
        }
        assert!(hs.status(now).away.contains(&alice) && !hs.status(now).held());
        assert_eq!(hs.humans().filter(|h| h.2).count(), 2);
        step(&mut hs, &mut clients, 60, &mut now);
        assert!(agree(&hs.own, &clients[0].peer) >= 60);
        assert!(hs.host.desyncs.is_empty(), "{:?}", hs.host.desyncs);

        // The host leaves: Bob sees it gone at once (no keep-alive wait).
        hs.leave();
        assert!(!clients[0].closed());
        now += Duration::from_millis(10);
        clients[0].pump(now, 0).unwrap();
        assert!(clients[0].closed());
        assert_eq!(clients[0].successors(), vec![clients[0].peer.slot], "Bob takes over");
    });
}
