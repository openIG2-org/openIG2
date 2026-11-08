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

/// ADR 0041: the host's forced start readies every seated member, and every joiner sees it.
#[test]
fn the_host_forces_the_start() {
    let mut h = LobbyHost::new_galaxy(settings(), 9, RULES, "Host");
    let mut a = join(&mut h, "Alice");
    let mut b = join(&mut h, "Bob");
    pump(&mut h, &mut [&mut a, &mut b], |h, c| h.state().members.len() == 3 && synced(h, c));
    a.ready(true);
    pump(&mut h, &mut [&mut a, &mut b], |h, c| h.state().members[1].ready && synced(h, c));
    assert!(h.set_ready(0, true));
    assert!(!h.can_start(), "Bob isn't ready");
    h.ready_all();
    assert!(h.can_start());
    pump(&mut h, &mut [&mut a, &mut b], synced);
    assert!(b.state.as_ref().unwrap().members.iter().all(|m| m.ready), "the joiners see the forced flags");
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

        // With a start story (`mpstory` / `story N`): every machine starts it in the build, right after
        // `InitNewGame` (`FRISSIT_Start` exe 0x5bc590, `Multi_Send(0x65)`), so the joiners' start hashes match.
        let mut hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 123_456, 1003).ok().expect("the room starts");
        let mut clients = vec![started(a, data, &rules).unwrap(), started(b, data, &rules).unwrap()];
        assert!(hs.own.game.state.story().starting.contains(1003), "the start story waits for its first check at turn 0");
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
            assert!(c.peer.game.state.story().is_running(1003));
        }
        assert!(hs.own.game.state.story().is_running(1003));
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
        let _hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 99, 0).ok().unwrap();
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
        let mut hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 4242, 0).ok().expect("the room starts");
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

/// ADR 0026: with the room's spectators option a joiner whose empire dies stays in the session; its orders (a
/// pause here) are ignored on every machine, and the three run on in step; the option survives a snapshot.
#[test]
fn a_defeated_spectator_stays_in_step_and_orders_nothing() {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else { return };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| {
        use ig2_net::{Command, Snapshot, StarmapCmd};
        let mut h = LobbyHost::new_galaxy(settings(), 0xD1F, RULES, "Host");
        assert!(h.set_options(GalaxyOptions { planets: 20, ais: 1, spectators: true, ..GalaxyOptions::default() }));
        let mut a = join(&mut h, "Alice");
        let mut b = join(&mut h, "Bob");
        pump(&mut h, &mut [&mut a, &mut b], |h, c| h.state().members.len() == 3 && synced(h, c));
        a.ready(true);
        b.ready(true);
        pump(&mut h, &mut [&mut a, &mut b], |h, _| h.state().members.iter().skip(1).all(|m| m.ready));
        assert!(h.set_ready(0, true));
        let mut hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 4242, 0).ok().expect("the room starts");
        let mut clients = vec![started(a, data, &rules).unwrap(), started(b, data, &rules).unwrap()];
        let alice = clients[0].peer.empire;
        assert!(hs.own.game.spectators && clients.iter().all(|c| c.peer.game.spectators));

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
        hs.own.game.empire_dies(alice, &[]);
        for c in &mut clients {
            c.peer.game.empire_dies(alice, &[]);
        }
        // Alice stays and (a modified client) pauses: nobody's clock stops.
        clients[0].peer.issue(Command::Starmap { order: StarmapCmd::Pause { paused: true } });
        step(&mut hs, &mut clients, 60, &mut now);
        assert!(!hs.own.game.paused() && clients.iter().all(|c| !c.peer.game.paused()), "a dead empire's pause is ignored");
        assert!(!hs.status(now).away.contains(&alice));
        assert!(agree(&hs.own, &clients[0].peer) >= 60 && agree(&hs.own, &clients[1].peer) >= 60);
        assert!(hs.host.desyncs.is_empty(), "{:?}", hs.host.desyncs);
        // A living player's pause still works.
        clients[1].peer.issue(Command::Starmap { order: StarmapCmd::Pause { paused: true } });
        step(&mut hs, &mut clients, 70, &mut now);
        assert!(hs.own.game.paused() && clients.iter().all(|c| c.peer.game.paused()));

        let (back, _) = Snapshot::capture(&hs.own.game, &[], 60, 0).restore(&rules).unwrap();
        assert!(back.spectators);
    });
}

/// ADR 0026: with the spectators option a newcomer is admitted to the running game as a spectator (empire 0, from
/// the host's state): nobody waits for it, it orders nothing and it stays in step; without the option it is refused.
#[test]
fn a_newcomer_watches_a_running_game() {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else { return };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| {
        use ig2_net::lobby::Refusal;
        use ig2_net::{Command, StarmapCmd};
        for spectators in [false, true] {
            let mut h = LobbyHost::new_galaxy(settings(), 0xD20, RULES, "Host");
            assert!(h.set_options(GalaxyOptions { planets: 20, ais: 1, spectators, ..GalaxyOptions::default() }));
            let mut a = join(&mut h, "Alice");
            pump(&mut h, &mut [&mut a], |h, c| h.state().members.len() == 2 && synced(h, c));
            a.ready(true);
            pump(&mut h, &mut [&mut a], |h, _| h.state().members.iter().skip(1).all(|m| m.ready));
            assert!(h.set_ready(0, true));
            let mut hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 4242, 0).ok().expect("the room starts");
            let mut clients = vec![started(a, data, &rules).unwrap()];
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

            // Carol knocks on the running game's door.
            let (ours, theirs) = loopback();
            hs.admit(Box::new(theirs), now);
            let mut carol = LobbyClient::rejoin(Box::new(ours), "Carol", RULES, None, None);
            while matches!(carol.pump().unwrap(), Status::Joining) {
                now += Duration::from_millis(10);
                hs.pump(now, 0).unwrap();
                clients[0].pump(now, 0).unwrap();
                assert!(now < Duration::from_secs(150), "the admission stalled");
            }
            if !spectators {
                assert_eq!(carol.status, Status::Refused(Refusal::Started), "closed to newcomers");
                continue;
            }
            assert_eq!(carol.status, Status::Admitted);
            let mut link = carol.into_link().unwrap();
            let start = loop {
                if let Some(s) = ClientSession::wait_start(link.as_mut()).unwrap() {
                    break s;
                }
                now += Duration::from_millis(10);
                hs.pump(now, 0).unwrap();
            };
            assert_eq!(start.empire, 0, "a spectator plays no empire");
            let mut watcher = ClientSession::new(&start, link, &rules).unwrap();
            watcher.peer.issue(Command::Starmap { order: StarmapCmd::Pause { paused: true } });

            // The players run on without waiting for the spectator, which then catches up.
            step(&mut hs, &mut clients, 40, &mut now);
            clients.push(watcher);
            step(&mut hs, &mut clients, 60, &mut now);
            assert!(!hs.own.game.paused() && clients.iter().all(|c| !c.peer.game.paused()), "the spectator's pause never went out");
            assert!(agree(&hs.own, &clients[1].peer) >= 30, "the spectator is in step");
            assert!(hs.host.desyncs.is_empty(), "{:?}", hs.host.desyncs);
            assert_eq!(hs.humans().count(), 2, "no new human");
        }
    });
}

/// ADR 0033: the host sets each computer's level in the room; every machine builds the same start with those levels
/// (the AIs' `Nehezseg`, the humans' the game's 1), the state carries them through a snapshot, and two machines run
/// in step; with the room's enhanced computer empires (ADR 0045) too.
#[test]
fn per_computer_levels_travel_and_stay_in_step() {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else { return };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| {
        use ig2_net::snapshot::Snapshot;
        use ig2_sim::save::va;
        let mut h = LobbyHost::new_galaxy(settings(), 0xD1FF, RULES, "Host");
        assert!(h.set_options(GalaxyOptions { planets: 20, ais: 3, ..GalaxyOptions::default() }));
        let mut a = join(&mut h, "Alice");
        pump(&mut h, &mut [&mut a], |h, c| h.state().members.len() == 2 && synced(h, c));
        let ais = h.state().ais();
        assert_eq!(ais.len(), 3);
        let mut o = h.state().galaxy.unwrap();
        for (&e, level) in ais.iter().zip([3, 2, 1]) {
            o.difficulty[e as usize - 1] = level;
        }
        o.enhanced_ai = true;
        assert!(h.set_options(o));
        pump(&mut h, &mut [&mut a], synced);
        assert_eq!(a.state.as_ref().unwrap().galaxy, Some(o), "the joiner sees the levels");
        a.ready(true);
        pump(&mut h, &mut [&mut a], |h, _| h.state().members[1].ready);
        assert!(h.set_ready(0, true));
        let humans: Vec<u8> = (0..2).map(|m| h.state().empire_of(m).unwrap()).collect();

        let mut hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 777, 0).ok().expect("the room starts");
        let mut c = started(a, data, &rules).unwrap();
        for g in [&hs.own.game, &c.peer.game] {
            let d = g.state.difficulties();
            assert_eq!(ais.iter().map(|&e| d[e as usize]).collect::<Vec<_>>(), [4, 2, 1], "the AIs' levels");
            assert!(humans.iter().all(|&e| d[e as usize] == 1), "the humans keep the game's");
            assert_eq!(g.state.mem.i32(va::NEHEZSEG), 1, "the game's level stays the original's");
            assert!(g.enhanced_ai, "the room's enhanced computer empires (ADR 0045)");
            // The start's `BirodalomIsmeriMiotaKi`: a hard computer knows the humans at once, an easy one after 0x708.
            let since = |e: u8, k: u8| g.state.mem.i32(va::EMPIRE_KNOWN_SINCE + 0x4c * e as u32 + 4 * k as u32);
            assert_eq!((since(ais[0], humans[0]), since(ais[2], humans[0])), (0, 0x708));
        }
        let snap = Snapshot::capture(&hs.own.game, &[], 0, 0);
        assert_eq!(snap.restore(&rules).unwrap().0.state.ai_difficulty, hs.own.game.state.ai_difficulty, "the snapshot keeps them");
        assert!(snap.restore(&rules).unwrap().0.enhanced_ai, "and the ruleset");
        let mut classic = snap.restore(&rules).unwrap().0;
        classic.enhanced_ai = false;
        assert_ne!(ig2_net::state_hash(&classic, &[]), ig2_net::state_hash(&hs.own.game, &[]), "the ruleset is in the hash");

        let mut now = Duration::ZERO;
        while hs.own.turn < 200 || c.peer.turn < 200 {
            now += Duration::from_millis(10);
            hs.pump(now, (hs.own.turn < 200) as usize).unwrap();
            hs.own.events.clear();
            c.pump(now, (c.peer.turn < 200) as usize).unwrap();
            c.peer.events.clear();
            assert!(now < Duration::from_secs(300), "stalled at {}", hs.own.turn);
        }
        assert!(agree(&hs.own, &c.peer) >= 200, "every turn from 0");
        assert!(hs.host.desyncs.is_empty(), "{:?}", hs.host.desyncs);
    });
}

#[test]
fn a_custom_game_in_the_start_data_and_the_save() {
    use ig2_net::snapshot::Snapshot;
    use ig2_sim::game::new_game::{CustomGame, NewSkirmish, Side};
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| {
        let sk = NewSkirmish { en: 1, ais: 2, tech: 1, planets: 40, money: 50000, difficulty: 2, population: 1, random_id: 0, seed: 0, ticks: 0 };
        let mut custom = CustomGame::from(&sk);
        custom.drawn_opponents = None;
        custom.empires[3].side = Side::Computer;
        for e in &mut custom.empires[4..] {
            e.side = Side::Off;
        }
        custom.empires[3].level = 4;
        let mut start = GalaxyStart { custom: Some(custom), ..GalaxyStart::new(4242, GalaxyOptions::default(), [1, 2, 3, 4, 5, 6, 7, 8], vec![4]) };
        let host = start.build_hashed(data, &rules, 1, &[1, 2], 99);
        let joiner = start.build(data, &rules, 1, &[1, 2], 99);
        assert_eq!(ig2_net::state_hash(&joiner, &[]), start.hash, "every machine builds the same start");
        let ki = |e: u32| host.state.mem.u8(ig2_sim::save::va::PLAYER_KI + e);
        assert_eq!((ki(1), ki(2), ki(3), ki(4)), (1, 1, 2, 2), "the room's humans");
        assert_eq!(host.state.planet_count(), 40);
        let c = host.state.custom.as_ref().expect("kept");
        assert_eq!((c.seed, c.en, c.empires[1].side), (start.seed, 1, Side::Human));
        let snap = Snapshot::capture(&host, &[], 0, 0);
        let back = snap.restore(&rules).unwrap().0;
        assert_eq!(back.state.custom, host.state.custom, "the save keeps it");
        assert_eq!(ig2_net::state_hash(&back, &[]), ig2_net::state_hash(&host, &[]));
        let mut none = snap.restore(&rules).unwrap().0;
        none.state.custom = None;
        assert_ne!(ig2_net::state_hash(&none, &[]), ig2_net::state_hash(&host, &[]), "it is in the hash");
        // A version 15 state is a version 16 one without the trailing `None`.
        let v16 = Snapshot::capture(&none, &[], 0, 0);
        assert_eq!(v16.state.last(), Some(&0));
        let v15 = Snapshot { state: v16.state[..v16.state.len() - 1].to_vec(), ..v16.clone() };
        assert_eq!(ig2_net::state_hash(&v15.restore(&rules).unwrap().0, &[]), ig2_net::state_hash(&none, &[]), "older saves still load");
    });
}

#[test]
fn a_room_custom_game_reaches_every_seat_and_the_start() {
    use ig2_net::galaxy::{RoomCustom, Seat};
    use ig2_sim::game::new_game::{Aggressiveness, Side, StartRelation};
    use ig2_sim::save::va;
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else { return };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| {
        let mut h = LobbyHost::new_galaxy(settings(), 0xC057, RULES, "Host");
        assert!(h.set_options(GalaxyOptions { ais: 2, ..GalaxyOptions::default() }));
        let mut a = join(&mut h, "Alice");
        pump(&mut h, &mut [&mut a], |h, c| h.state().members.len() == 2 && synced(h, c));
        let ais = h.state().ais();
        let humans: Vec<u8> = (0..2).map(|m| h.state().empire_of(m).unwrap()).collect();
        let mut custom = RoomCustom::new(70, 31337, Seat { money: 40000, tech: 1, planets: 2, aggression: 1, relation: 1 });
        custom.seats[humans[1] as usize - 1].money = 250_000;
        custom.seats[ais[0] as usize - 1] = Seat { money: 9000, tech: 3, planets: 4, aggression: 2, relation: 0 };
        let o = GalaxyOptions { custom: Some(custom), ..h.state().galaxy.unwrap() };
        assert!(h.set_options(o));
        pump(&mut h, &mut [&mut a], synced);
        assert_eq!(a.state.as_ref().unwrap().galaxy.unwrap().custom, Some(custom), "the joiner sees the custom game");
        a.ready(true);
        pump(&mut h, &mut [&mut a], |h, _| h.state().members[1].ready);
        assert!(h.set_ready(0, true));

        let mut hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), 555, 0).ok().expect("the room starts");
        let mut c = started(a, data, &rules).unwrap();
        for g in [&hs.own.game, &c.peer.game] {
            let cg = g.state.custom.as_ref().expect("a custom start");
            assert_eq!((cg.seed, g.state.planet_count()), (31337, 70), "the host's seed and planets");
            assert!(humans.iter().all(|&e| cg.empires[e as usize - 1].side == Side::Human));
            assert!(ais.iter().all(|&e| cg.empires[e as usize - 1].side == Side::Computer));
            let e = cg.empires[ais[0] as usize - 1].clone();
            assert_eq!((e.tech, e.aggressiveness, e.relations[humans[0] as usize]), (3, Aggressiveness::High, StartRelation::Hostile));
            assert_eq!(g.state.mem.f64(va::MONEY + 8 * humans[1] as u32), 250_000.0, "the joiner's seat money");
            assert_eq!(g.state.mem.f64(va::MONEY + 8 * ais[0] as u32), 9000.0);
        }
        let mut now = Duration::ZERO;
        while hs.own.turn < 100 || c.peer.turn < 100 {
            now += Duration::from_millis(10);
            hs.pump(now, (hs.own.turn < 100) as usize).unwrap();
            hs.own.events.clear();
            c.pump(now, (c.peer.turn < 100) as usize).unwrap();
            c.peer.events.clear();
            assert!(now < Duration::from_secs(300), "stalled at {}", hs.own.turn);
        }
        assert!(hs.host.desyncs.is_empty(), "{:?}", hs.host.desyncs);
    });
}
