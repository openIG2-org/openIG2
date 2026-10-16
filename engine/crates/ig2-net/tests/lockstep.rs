//! The multiplayer core on a real save (`_local/saves/quick-1.sav`, ADR 0007): our save format round-trips the
//! state, two peers on loopback links run the same ticks from the same commands with identical hashes, a third
//! rejoins from a snapshot and the command log, a corrupted peer is reported as desynced, and peers playing
//! different empires and looking at different planets stay identical in the canonical view (ADR 0007 §5).

mod common;

use ig2_net::{loopback, state_hash, Command, Host, Loopback, Peer, Snapshot, Terms, DEFAULT_DELAY, TURN_TICKS};
use ig2_sim::game::{Game, GameRules, LocalView, ViewMode};
use ig2_sim::save::{va, SaveState, PLANET_SIZE};
use ig2_sim::trade::DELIVERY;

const SAVE: &str = "quick-1.sav";

/// The game as a load leaves it (`LoadSave` ends with `FlottakUjraSzamol`).
fn game<'r>(state: SaveState, rules: &'r GameRules) -> Game<'r> {
    let mut g = Game::new(state, rules);
    g.saved();
    g
}

fn tick(g: &mut Game) {
    g.advance_clock();
    g.run_tick();
    g.events.clear();
}

#[test]
fn save_format_round_trips_the_state() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let mut a = game(state, &rules);
    for _ in 0..20 {
        tick(&mut a);
    }
    let snap = Snapshot::capture(&a, &[], 10, 20);
    let file = snap.to_file(0x1234, false);
    eprintln!("state {} bytes, file {} bytes", snap.state.len(), file.len());
    let (back, header) = Snapshot::from_file(&file).unwrap();
    assert_eq!(back, snap);
    assert_eq!((header.turn, header.tick, header.rules, header.flags), (10, 20, 0x1234, 0));
    let (mut b, pending) = back.restore(&rules).unwrap();
    assert!(pending.is_empty());
    // The same state: memory image, records, queues, the original save's chunks.
    assert_eq!(b.state.mem, a.state.mem);
    assert_eq!((&b.state.fleets, &b.state.ships, &b.state.waypoints), (&a.state.fleets, &a.state.ships, &a.state.waypoints));
    assert_eq!((&b.state.designs, &b.state.buildings), (&a.state.designs, &a.state.buildings));
    assert_eq!(b.state.story, a.state.story);
    assert_eq!(b.state.chunks(), a.state.chunks());
    // The battlefields hold loaded floats that can be NaN (uninitialised in the original): compare them as printed.
    assert_eq!(b.libc, a.libc);
    assert_eq!(format!("{:?}", b.state.ground_fields()), format!("{:?}", a.state.ground_fields()));
    assert_eq!(Snapshot::capture(&b, &[], 10, 20).state, snap.state);
    // And it runs on identically.
    for k in 0..40 {
        tick(&mut a);
        tick(&mut b);
        assert_eq!(state_hash(&a, &[]), state_hash(&b, &[]), "tick {k} after the load");
    }
    // A damaged file is refused.
    let mut bad = file.clone();
    bad[8] = 99;
    assert!(Snapshot::from_file(&bad).is_err());
    let mut bad = file.clone();
    bad[28] ^= 1;
    assert!(Snapshot::from_file(&bad).is_err());
    assert!(Snapshot::from_file(&file[..file.len() - 1]).is_err());
    assert!(Snapshot::from_file(b"IG2RSA").is_err());
}

/// A session on loopback links: the host and its peers' ends of the links.
struct Session<'r> {
    host: Host,
    peers: Vec<(Peer<'r>, Loopback)>,
}

impl<'r> Session<'r> {
    fn new() -> Self {
        Self { host: Host::new(DEFAULT_DELAY), peers: Vec::new() }
    }

    fn join(&mut self, slot: u8, empire: u8, make: impl FnOnce(u64) -> Peer<'r>) {
        let (ours, theirs) = loopback();
        let first = self.host.add_peer(slot, empire, Box::new(theirs));
        self.peers.push((make(first), ours));
    }

    /// Pump everything until every peer has run `turns` turns, letting `act` issue commands as turns go by.
    fn run_to(&mut self, turns: u64, mut act: impl FnMut(&mut Peer<'r>)) {
        for _ in 0..100_000 {
            if self.peers.iter().all(|(p, _)| p.turn >= turns) {
                return;
            }
            self.host.pump().unwrap();
            for (p, link) in &mut self.peers {
                // Uneven speeds: peers drift apart within the input delay.
                let max = if p.slot % 2 == 0 { 1 } else { 3 };
                p.poll(link, 0).unwrap();
                for _ in 0..max {
                    if p.turn >= turns || p.poll(link, 1).unwrap() == 0 {
                        break;
                    }
                    act(p);
                }
            }
        }
        panic!("the session stalled");
    }
}

/// Every peer's hash for every turn they both ran agrees.
fn agree(peers: &[(Peer, Loopback)]) -> usize {
    let (first, _) = &peers[0];
    let mut n = 0;
    for (p, _) in &peers[1..] {
        for (t, h) in &p.hashes {
            if let Some((_, h0)) = first.hashes.iter().find(|(t0, _)| t0 == t) {
                assert_eq!(h, h0, "turn {t}: slot {} vs slot {}", p.slot, first.slot);
                n += 1;
            }
        }
    }
    n
}

/// Some commands of each kind, issued by the peers at a few turns (valid and invalid ones alike).
fn act(p: &mut Peer) {
    match (p.slot, p.turn) {
        (1, 3) => p.issue(Command::DiplomacyMessage { to: 3, declaration: false, terms: Terms { cat: 0, sub: 1, value: 0, sub2: -1, value2: -1 } }),
        (2, 4) => p.issue(Command::Buy { delivery: vec![0; DELIVERY] }),
        (1, 6) => p.issue(Command::StoryChoice { id: 10031 }),
        (2, 6) => p.issue(Command::DiplomacyAnswer { to: 6, terms: Terms { cat: 0, sub: 1, value: 0, sub2: -1, value2: -1 }, counter: false, reply: false, yes: true }),
        (1, 9) => p.issue(Command::Buy { delivery: vec![1; 3] }),
        (2, 12) => p.issue(Command::FleetEmptied { fleet: 1 }),
        _ => {}
    }
}

#[test]
fn two_peers_stay_in_lockstep() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let mut s = Session::new();
    // The human of quick-1 (empire 6) and the AI empire 3 played by a second player.
    s.join(1, 6, |_| Peer::new(1, 6, game(state.clone(), &rules), DEFAULT_DELAY));
    s.join(2, 3, |_| Peer::new(2, 3, game(state.clone(), &rules), DEFAULT_DELAY));
    let turns = 60;
    s.run_to(turns, act);
    let n = agree(&s.peers);
    assert_eq!(n as u64, turns);
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    for (p, _) in &s.peers {
        assert_eq!(p.tick, turns * TURN_TICKS as u64);
        assert!(p.desyncs.is_empty());
    }
    // The bundles carried every command, stamped with its sender's empire.
    let sent: Vec<(u64, u8)> = s.host.log.iter().flat_map(|b| b.commands.iter().map(move |c| (b.turn, c.issuer))).collect();
    assert_eq!(sent.len(), 6, "{sent:?}");
    assert!(sent.iter().all(|&(t, _)| t >= DEFAULT_DELAY));
    // Commands changed the game: a session without them ends elsewhere.
    let mut solo = Session::new();
    solo.join(1, 6, |_| Peer::new(1, 6, game(state.clone(), &rules), DEFAULT_DELAY));
    solo.run_to(turns, |_| {});
    assert_ne!(solo.peers[0].0.hashes.last(), s.peers[0].0.hashes.last());
}

#[test]
fn a_peer_rejoins_from_a_snapshot_and_the_log() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, game(state.clone(), &rules), DEFAULT_DELAY));
    s.join(2, 3, |_| Peer::new(2, 3, game(state.clone(), &rules), DEFAULT_DELAY));
    s.run_to(10, act);
    // Slot 2 drops; the others go on without it.
    s.host.remove_peer(2);
    s.peers.remove(1);
    s.run_to(20, act);
    // It comes back: the host's player sends a snapshot (as a save file) and the host the bundles since.
    let file = s.peers[0].0.snapshot().to_file(0, true);
    let (snap, _) = Snapshot::from_file(&file).unwrap();
    let rules_ref = &rules;
    s.join(2, 3, |first| Peer::from_snapshot(2, 3, &snap, rules_ref, DEFAULT_DELAY, first).unwrap());
    s.host.send_log(2, snap.turn);
    s.run_to(45, act);
    let n = agree(&s.peers);
    assert_eq!(n as u64, 45 - snap.turn);
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
}

#[test]
fn a_diverged_peer_is_reported() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, game(state.clone(), &rules), DEFAULT_DELAY));
    s.join(2, 3, |_| Peer::new(2, 3, game(state.clone(), &rules), DEFAULT_DELAY));
    s.run_to(5, |_| {});
    // Slot 2's libc stream drifts (as an unsynchronised `rand()` would).
    s.peers[1].0.game.libc.0 ^= 0x5555;
    s.run_to(12, |_| {});
    s.host.pump().unwrap();
    let d = s.host.desyncs.first().expect("a desync");
    assert!(d.turn >= 5 && d.turn <= 6, "{d:?}");
    for (p, link) in &mut s.peers {
        p.poll(link, 0).unwrap();
        assert!(!p.desyncs.is_empty(), "slot {} heard of it", p.slot);
    }
}

/// A planet whose battlefield weather draws libc `rand()` (surfaces 2, 5, 7): showing it skips that draw in the
/// classic view (`FRISSIT_Planet`'s prologue, `Game::battlefield_weather`).
fn weather_planet(state: &SaveState) -> i32 {
    (1..=state.planet_count())
        .find(|&p| matches!(state.mem.u8(va::PLANETS + (p * PLANET_SIZE) as u32 + 0x22), 2 | 5 | 7))
        .expect("a planet with weather") as i32
}

#[test]
fn local_views_stay_out_of_the_synced_state() {
    let Some((rules, mut state)) = common::setup(SAVE) else { return };
    // quick-1 is saved paused: run the clock at ×16 so economy days come.
    state.mem.set_i32(va::IDO_SPEED, 16);
    let shown = weather_planet(&state);
    let look = |empire: u8| LocalView { empire, starmap_mode: 1, planet: shown, fleet: 1 };

    // The classic view: the same two looks are game state, and the shown planet changes gameplay (the libc stream
    // every later draw of the economy day uses), not just the bytes of the globals.
    let (mut a, mut b) = (game(state.clone(), &rules), game(state.clone(), &rules));
    b.set_view(look(3));
    let mut diverged = None;
    for k in 0..2000 {
        tick(&mut a);
        tick(&mut b);
        if a.libc != b.libc {
            diverged = Some(k + 1);
            break;
        }
    }
    let diverged = diverged.expect("an economy day within 2000 ticks");
    eprintln!("classic: the shown planet moved libc after {diverged} ticks");

    // The canonical view: both peers' sim reads `EN` = 6 and nothing on screen, whatever each one shows.
    let canonical = |state: SaveState| {
        let mut g = game(state, &rules);
        g.canonical_view(6);
        g
    };
    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, canonical(state.clone()), DEFAULT_DELAY));
    s.join(2, 3, |_| Peer::new(2, 3, canonical(state.clone()), DEFAULT_DELAY));
    s.peers[1].0.show(look(3));
    let turns = (diverged as u64).div_ceil(TURN_TICKS as u64) + 10;
    s.run_to(turns, |p| {
        act(p);
        // Peer 1 browses: a planet, then the starmap, a fleet.
        match (p.slot, p.turn % 7) {
            (1, 0) => p.show(LocalView { empire: 6, starmap_mode: 1, planet: shown + 1, fleet: 2 }),
            (1, 3) => p.show(LocalView { empire: 6, starmap_mode: 0, planet: 0, fleet: 3 }),
            _ => {}
        }
    });
    assert_eq!(agree(&s.peers) as u64, turns);
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    let (p1, p2) = (&s.peers[0].0, &s.peers[1].0);
    assert_eq!((p1.game.local_view().empire, p2.game.local_view()), (6, look(3)));
    assert_ne!(p1.game.cosmetic, p2.game.cosmetic, "per-peer cosmetic streams");
    // The view is synced state: a snapshot carries it, the per-peer parts it leaves out.
    let snap = p2.snapshot();
    assert_eq!(snap.hash, p1.snapshot().hash);
    let (back, pending) = snap.restore(&rules).unwrap();
    assert_eq!(back.view, ViewMode::Canonical { en: 6 });
    assert_eq!(Snapshot::capture(&back, &pending, snap.turn, snap.tick).hash, snap.hash);
}
