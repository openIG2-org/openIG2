//! The multiplayer core on a real save (`_local/saves/quick-1.sav`, ADR 0007): our save format round-trips the
//! state, two peers on loopback links run the same ticks from the same commands with identical hashes, a third
//! rejoins from a snapshot and the command log, a corrupted peer is reported as desynced, and peers playing
//! different empires and looking at different planets stay identical in the canonical view (ADR 0007 §5), each with
//! its own contact list and spy report queue (`re/rules/multiplayer.md` §5).

mod common;

use ig2_net::{loopback, state_hash, Command, Host, Loopback, Peer, Snapshot, SpyOrder, Terms, DEFAULT_DELAY, TURN_TICKS};
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
        (1, 7) => p.issue(Command::Spy { order: SpyOrder::Send { spy: 1, mission: 2, target: 3, param1: 0, param2: 0, days: 5 } }),
        (2, 7) => p.issue(Command::Spy { order: SpyOrder::Skill { spy: 1, skill: 1 } }),
        (1, 8) => p.issue(Command::StoryStart { id: 1004 }),
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
    assert_eq!(sent.len(), 9, "{sent:?}");
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

/// The slot of `owner`'s contact list for `e` (`None`: no slot): the canonical `EN`'s list or another human's.
fn contact(g: &Game, owner: u8, e: i32) -> Option<ig2_sim::diplomacy::PlayerSlot> {
    let d = g.state.diplomacy();
    let list = if owner == 6 { d.players } else { d.others.into_iter().find(|c| c.empire == owner)?.players };
    list.into_iter().find(|s| s.empire == e)
}

#[test]
fn each_human_keeps_its_own_contacts_and_reports() {
    let Some((rules, mut state)) = common::setup(SAVE) else { return };
    // Empire 3 is played by a second human.
    state.mem.set_u8(va::PLAYER_KI + 3, 1);
    let classic = game(state.clone(), &rules);
    assert!(classic.state.humans.is_empty(), "single player keeps no copies");
    let canonical = |state: SaveState| {
        let mut g = game(state, &rules);
        g.canonical_view(6);
        g
    };

    // The copy starts from what 3 has met; `EN`'s own list is the save's.
    let g = canonical(state.clone());
    assert_eq!(g.state.humans.keys().copied().collect::<Vec<_>>(), vec![3]);
    let races = state.races();
    let knows = |a: usize, b: usize| state.mem.u8(va::EMPIRE_KNOWS + (a * 19 + b) as u32) != 0;
    let met: Vec<i32> = (1..9).filter(|&e| e != 3 && knows(3, e) && races[e] != 5).map(|e| e as i32).collect();
    let d = g.state.diplomacy();
    assert_eq!(d.others[0].players.iter().map(|s| s.empire).collect::<Vec<_>>(), met);
    assert!(d.others[0].players.iter().all(|s| s.open == 1 && s.waiting == 0));
    assert_eq!(d.players, classic.state.diplomacy().players);
    eprintln!("3 starts knowing {met:?}");

    // Each human's letters and answers land in its own list, and both peers agree.
    let trade = Terms { cat: 0, sub: 8, value: 0, sub2: -1, value2: -1 };
    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, canonical(state.clone()), DEFAULT_DELAY));
    s.join(2, 3, |_| Peer::new(2, 3, canonical(state.clone()), DEFAULT_DELAY));
    let turns = 20;
    s.run_to(turns, |p| match (p.slot, p.turn) {
        (2, 3) => p.issue(Command::DiplomacyMessage { to: 6, declaration: false, terms: trade }),
        (1, 5) => p.issue(Command::DiplomacyMessage { to: 3, declaration: false, terms: trade }),
        (2, 8) => p.issue(Command::DiplomacyAnswer { to: 6, terms: trade, counter: false, reply: false, yes: true }),
        (1, 10) => p.issue(Command::DiplomacyAnswer { to: 3, terms: trade, counter: false, reply: false, yes: false }),
        _ => {}
    });
    assert_eq!(agree(&s.peers) as u64, turns);
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    let (p1, p2) = (&s.peers[0].0, &s.peers[1].0);
    assert_eq!(p1.game.state.humans, p2.game.state.humans);
    for g in [&p1.game, &p2.game] {
        // 3's letter waits in 6's (the exe's) list and got 3's answer; 6's letter waits in 3's own list.
        let at6 = contact(g, 6, 3).expect("6 knows 3");
        assert_eq!((at6.waiting, at6.letter.from, at6.letter.to, at6.answered, at6.answer.yes), (1, 3, 6, 1, true));
        let at3 = contact(g, 3, 6).expect("3 knows 6");
        assert_eq!((at3.waiting, at3.letter.from, at3.letter.to, at3.answered, at3.answer.yes), (1, 6, 3, 1, false));
    }
    // The classic run of the same letter to 3 leaves no trace in any list but `EN`'s.
    let mut c = game(state.clone(), &rules);
    c.diplomacy_message(ig2_sim::diplomacy::Letter { from: 6, to: 3, declaration: false, msg: trade.into() });
    assert!(c.state.humans.is_empty());

    // Spy reports to another human go to its own queue (50 entries, the oldest dropped).
    let mut g = canonical(state.clone());
    for k in 0..51 {
        g.state.push_spy_report_to(3, &ig2_sim::spy::Report { kind: 2, a: k, ..Default::default() });
    }
    let q = &g.state.humans[&3].reports;
    assert_eq!(q.len(), 50 * ig2_sim::spy::REPORT);
    assert_eq!(i32::from_le_bytes(q[4..8].try_into().unwrap()), 1);

    // The copies are synced state: a snapshot carries them.
    let snap = p2.snapshot();
    let (back, pending) = snap.restore(&rules).unwrap();
    assert_eq!(back.state.humans, p2.game.state.humans);
    assert_eq!(Snapshot::capture(&back, &pending, snap.turn, snap.tick).hash, snap.hash);
}

/// Two humans' fleets meet: the real-time battle runs on the sim clock in canonical MP (`re/rules/multiplayer.md`
/// §7, `space-combat.md` §7.12–§7.13): both peers set it up in the same slot, the strategic clock waits, the orders
/// travel as lockstep commands, and once both players leave the finished battle's screen `SpaceWarVege` ends it and
/// the clock runs on. Every turn the peers' hashes agree; a snapshot taken mid-battle restores to the same hash.
#[test]
fn two_humans_fight_a_real_time_battle_in_lockstep() {
    use ig2_net::BattleOrder;
    use ig2_sim::game::{GameEvent, RealTimeEvent};
    let Some((rules, mut state)) = common::setup(SAVE) else { return };
    // Empire 3 is the second human; the session runs the MP rules (two human sides fight in real time).
    state.mem.set_u8(va::PLAYER_KI + 3, 1);
    state.mem.set_u8(va::MULTIPLAYER, 1);
    state.mem.set_i32(0xBAE558, 0);
    // MP normal speed (×4; the save holds 0): the battles step once per tick (`SetIdoSpeed`).
    state.mem.set_i32(va::IDO_SPEED, 4);
    let combat = state.combat();
    let warship = |e: usize| {
        (1..combat.designs[e].len())
            .find(|&d| {
                let x = &combat.designs[e][d];
                x.class == 1 && !x.is_fighter() && x.attack > 0 && x.hp > 0
            })
            .unwrap_or_else(|| panic!("empire {e} has a warship design"))
    };
    let (d6, d3) = (warship(6), warship(3));
    // A fleet for each next to one of 6's planets (records like fleet 1's, a plain fleet), 3's ordered to attack 6's.
    let home = (1..=state.planet_count()).find(|&p| state.mem.u8(va::PLANETS + (p * PLANET_SIZE) as u32 + 0x20) == 6).expect("a planet of 6");
    let at = va::PLANETS + (home * PLANET_SIZE) as u32;
    let (hx, hy) = (f32::from_le_bytes(state.mem.read(at + 0x18, 4).try_into().unwrap()), f32::from_le_bytes(state.mem.read(at + 0x1c, 4).try_into().unwrap()));
    for owner in [6u8, 3] {
        let mut rec = state.fleets[1];
        rec[0x15] = owner;
        state.fleets.push(rec);
    }
    let (a, b) = (state.fleets.len() - 2, state.fleets.len() - 1);
    state.mem.set_i32(va::FLEET_COUNT, b as i32);
    for (f, d, n) in [(a, d6, 6), (b, d3, 6)] {
        for _ in 0..n {
            let mut s = [0u8; 6];
            s[0..2].copy_from_slice(&(f as i16).to_le_bytes());
            s[2..4].copy_from_slice(&(d as i16).to_le_bytes());
            state.ships.push(s);
        }
    }
    state.mem.set_i32(va::SHIP_COUNT, state.ships.len() as i32 - 1);
    let mut map = state.starmap();
    for f in [a, b] {
        (map.fleets[f].x, map.fleets[f].y, map.fleets[f].order, map.fleets[f].flags) = (hx + 3.0, hy + 3.0, 0, 0);
        map.fleets[f].name = [0; 21];
        map.fleets[f].name[..5].copy_from_slice(b"Probe");
    }
    map.fleets[b].x += 1.0;
    (map.fleets[b].order, map.fleets[b].kind, map.fleets[b].tx) = (ig2_sim::fleet::order::ATTACK, ig2_sim::fleet::order::FLEET, a as f32);
    state.store_starmap(&map);
    let canonical = |state: SaveState| {
        let mut g = game(state, &rules);
        g.canonical_view(6);
        g
    };

    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, canonical(state.clone()), DEFAULT_DELAY));
    s.join(2, 3, |_| Peer::new(2, 3, canonical(state.clone()), DEFAULT_DELAY));
    let started = |p: &Peer| p.events.iter().find_map(|e| if let GameEvent::RealTime(RealTimeEvent::SpaceStarted { slot }) = e { Some(*slot) } else { None });
    // Each player's orders: the first own unit attacks the first enemy unit, the others move, one fires directly,
    // weapons and the fighter stance are toggled; later empire 3 retreats.
    let mut slot = None;
    let mut ordered = [false; 2];
    let mut over = None;
    let mut left = [false; 2];
    let mut mid = None;
    let mut clock_at_start = 0;
    for _ in 0..40 {
        let target = s.peers[0].0.turn + 50;
        s.run_to(target, |p| {
            if slot.is_none() {
                slot = started(p);
                if slot.is_some() {
                    clock_at_start = p.game.state.mem.i32(va::IDO);
                }
            }
            let Some(b) = slot else { return };
            let k = (p.slot - 1) as usize;
            let me = p.empire as i32;
            let battle = &p.game.realtime.slots[b];
            let (active, finished, tick) = (battle.active, battle.finished, battle.tick);
            let mine: Vec<usize> = (0..battle.units.len()).filter(|&u| battle.units[u].alive && battle.units[u].owner == me).collect();
            let theirs: Vec<usize> = (0..battle.units.len()).filter(|&u| battle.units[u].alive && battle.units[u].owner > 0 && battle.units[u].owner != me).collect();
            let armed = mine.first().is_some_and(|&u| !battle.units[u].mounts.is_empty());
            if !ordered[k] && active {
                ordered[k] = true;
                assert!(!mine.is_empty() && !theirs.is_empty(), "both sides on the field");
                let o = |order| Command::SpaceOrder { battle: b as u8, order };
                p.issue(o(BattleOrder::Attack { unit: mine[0] as u16, target: theirs[0] as u16 }));
                for &u in &mine[1..] {
                    p.issue(o(BattleOrder::Move { unit: u as u16, x: 0f32.to_bits(), y: (10.0 * me as f32).to_bits(), z: (-20f32).to_bits() }));
                }
                if armed {
                    p.issue(o(BattleOrder::DirectFire { unit: mine[0] as u16, target: theirs[0] as u16, mount: 0 }));
                }
                // Weapon bit 0 off (it comes back on below).
                p.issue(o(BattleOrder::Weapons { unit: mine[0] as u16, keep: !1, set: 0 }));
                p.issue(o(BattleOrder::FighterStance { side: p.empire, value: 80 }));
                // Someone else's unit: refused by every peer alike.
                p.issue(o(BattleOrder::Move { unit: theirs[0] as u16, x: 0, y: 0, z: 0 }));
            }
            if ordered[k] && tick == 200 + 50 * k as i32 && active && !mine.is_empty() {
                p.issue(Command::SpaceOrder { battle: b as u8, order: BattleOrder::Weapons { unit: mine[0] as u16, keep: !0, set: !0 } });
            }
            if p.empire == 3 && ordered[k] && tick > 300 && active && !finished {
                p.issue(Command::SpaceOrder { battle: b as u8, order: BattleOrder::Retreat });
            }
            if mid.is_none() && active && tick > 100 {
                mid = Some(p.snapshot());
            }
            if over.is_none() {
                over = p.events.iter().find_map(|e| if let GameEvent::RealTime(RealTimeEvent::SpaceOver { winner, .. }) = e { Some(*winner) } else { None });
                if over.is_some() {
                    // The strategic clock stood still while the battle ran.
                    assert_eq!(p.game.state.mem.i32(va::IDO), clock_at_start);
                }
            }
            if over.is_some() && !left[k] {
                left[k] = true;
                p.issue(Command::SpaceBattleLeft);
            }
            p.events.clear();
        });
        if left.iter().all(|&l| l) && s.peers.iter().all(|(p, _)| p.game.state.mem.i32(va::MP_SPACE_BATTLE) == 0) {
            break;
        }
    }
    let b = slot.expect("the attack started a real-time battle");
    assert!(over.is_some(), "the battle ended");
    assert!(s.peers.iter().all(|(p, _)| !p.game.realtime.slots[b].active), "SpaceWarVege closed the slot");
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    let turns = agree(&s.peers);
    eprintln!("battle in slot {b}, winner {over:?}, {turns} turns agreed, clock {clock_at_start} -> {}", s.peers[0].0.game.state.mem.i32(va::IDO));
    // The strategic clock stood still during the battle and runs again after it.
    let end = s.peers[0].0.turn + 20;
    s.run_to(end, |p| p.events.clear());
    assert!(s.peers[0].0.game.state.mem.i32(va::IDO) > clock_at_start);
    assert_eq!(agree(&s.peers), turns + 20);
    // The battle is synced state: the mid-battle snapshot restores to the same hash.
    let snap = mid.expect("a snapshot mid-battle");
    let (back, pending) = snap.restore(&rules).unwrap();
    assert!(back.realtime.slots.iter().any(|b| b.active));
    assert_eq!(Snapshot::capture(&back, &pending, snap.turn, snap.tick).hash, snap.hash);
}

/// A human lands on another human's planet: the ground war runs by the session's rules (`re/rules/multiplayer.md`
/// §7, `ground-combat.md` §11.17): `WF::MI` and `WF::Refresh` of that planet every tick whatever the peers show, the
/// strategic clock waits, the peers stay hash-identical. Two humans' units only fight on their players' orders
/// (`MULTIGET_Colony`, not ported yet), so a defended planet stays contested; an undefended one falls at once
/// (`StartGrWar` finds the field held).
#[test]
fn two_humans_fight_a_ground_war_in_lockstep() {
    use ig2_sim::game::{GameEvent, RealTimeEvent};
    let Some((rules, base)) = common::setup(SAVE) else { return };
    let canonical = |state: SaveState| {
        let mut g = game(state, &rules);
        g.canonical_view(6);
        g
    };
    for defended in [true, false] {
        let (state, home) = landing(base.clone(), defended);
        let mut s = Session::new();
        s.join(1, 6, |_| Peer::new(1, 6, canonical(state.clone()), DEFAULT_DELAY));
        s.join(2, 3, |_| Peer::new(2, 3, canonical(state.clone()), DEFAULT_DELAY));
        let (mut started, mut ended, mut clock, mut turns_running) = (None, None, None, 0);
        s.run_to(150, |p| {
            for e in p.events.drain(..) {
                match e {
                    GameEvent::RealTime(RealTimeEvent::GroundStarted { planet, holder }) if p.slot == 1 => {
                        started = Some((planet, holder));
                        clock = Some(p.game.state.mem.i32(va::IDO));
                    }
                    GameEvent::RealTime(RealTimeEvent::GroundEnded { planet, holder }) if p.slot == 1 => ended = Some((planet, holder)),
                    _ => {}
                }
            }
            if p.slot == 1 && p.game.state.mem.i32(0xBAD13C) != 0 {
                turns_running += 1;
                // The strategic clock stands still while the war runs.
                assert_eq!(Some(p.game.state.mem.i32(va::IDO)), clock);
            }
        });
        assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
        assert_eq!(agree(&s.peers), 150);
        let (planet, holder) = started.expect("the landing started a real-time ground war");
        assert_eq!(planet, home);
        let g = &s.peers[0].0.game;
        let owner = g.state.mem.u8(va::PLANETS + (home * PLANET_SIZE) as u32 + 0x20);
        eprintln!("defended {defended}: holder at start {holder:?}, ended {ended:?}, {turns_running} turns running, owner {owner}");
        if defended {
            assert_eq!((holder, ended), (None, None));
            assert!(turns_running > 100);
            // `WF::Refresh` ran every tick: the call-in timers of every empire reached their caps.
            let f = g.state.ground_field(home).unwrap();
            assert!(f.timers.iter().all(|t| *t == (200, 300)), "{:?}", f.timers);
            assert_eq!(g.state.mem.i32(va::IDO), clock.unwrap());
        } else {
            assert_eq!(holder, Some(3));
            assert_eq!(ended, Some((home, 3)));
            assert_eq!((owner, g.state.mem.i32(0xBAD13C)), (3, 0), "captured, the MP flag cleared");
            assert!(g.state.mem.i32(va::IDO) > clock.unwrap(), "the clock runs on");
        }
    }
}

/// Empire 3 (a second human) lands six tanks on one of 6's planets under the MP rules; `defended == false` takes the
/// planet's turrets and garrison away first.
fn landing(mut state: SaveState, defended: bool) -> (SaveState, usize) {
    state.mem.set_u8(va::PLAYER_KI + 3, 1);
    state.mem.set_u8(va::MULTIPLAYER, 1);
    state.mem.set_i32(0xBAE558, 0);
    state.mem.set_i32(va::IDO_SPEED, 4);
    // Debug switch 0xB7723C: no lopsided ground war (it would be auto-resolved).
    state.mem.set_i32(0xB7723C, 1);
    let combat = state.combat();
    let design = |e: usize, f: &dyn Fn(&ig2_sim::spacewar::Design) -> bool| {
        (1..combat.designs[e].len()).find(|&d| f(&combat.designs[e][d])).unwrap_or_else(|| panic!("a design of {e}"))
    };
    let carrier = design(3, &|x| x.class == 1 && !x.is_fighter() && x.capacity > 0);
    let tank = design(3, &|x| x.class == 2 && x.attack > 0);
    // 6's planet with no space defence (no ships, guns switched off), 3's fleet beside it, ordered to attack it.
    let home = (1..=state.planet_count()).find(|&p| state.mem.u8(va::PLANETS + (p * PLANET_SIZE) as u32 + 0x20) == 6).expect("a planet of 6");
    let at = va::PLANETS + (home * PLANET_SIZE) as u32;
    state.mem.set_i32(at + 0x114, 0);
    for b in state.buildings[home].iter_mut().skip(1) {
        if matches!(b[0], 0x1c..=0x1f | 0x24..=0x26) {
            b[0x19] &= !1;
        }
        if !defended && matches!(b[0], 0x1c..=0x23) {
            b[0] = 0;
        }
    }
    if !defended {
        let mut f = state.take_ground_field(home);
        for a in f.alive.iter_mut() {
            *a = false;
        }
        state.store_ground_field(home, f);
        for v in 1..=state.mem.i32(va::VEHICLE_COUNT).max(0) as u32 {
            if state.mem.i16(va::VEHICLES + 8 * v) == -(home as i16) {
                state.mem.write(va::VEHICLES + 8 * v + 2, &0x7fffi16.to_le_bytes());
            }
        }
        state.mem.set_i32(at + 0x58, 0);
        state.mem.set_i32(at + 0x5c, 0);
    }
    let (hx, hy) = (f32::from_le_bytes(state.mem.read(at + 0x18, 4).try_into().unwrap()), f32::from_le_bytes(state.mem.read(at + 0x1c, 4).try_into().unwrap()));
    let rec = state.fleets[1];
    state.fleets.push(rec);
    let f = state.fleets.len() - 1;
    state.mem.set_i32(va::FLEET_COUNT, f as i32);
    for _ in 0..4 {
        let mut s = [0u8; 6];
        s[0..2].copy_from_slice(&(f as i16).to_le_bytes());
        s[2..4].copy_from_slice(&(carrier as i16).to_le_bytes());
        state.ships.push(s);
    }
    state.mem.set_i32(va::SHIP_COUNT, state.ships.len() as i32 - 1);
    let nv = state.mem.i32(va::VEHICLE_COUNT).max(0) as u32 + 1;
    let mut v = [0u8; 8];
    v[0..2].copy_from_slice(&(f as i16).to_le_bytes());
    v[2..4].copy_from_slice(&(tank as i16).to_le_bytes());
    v[4..8].copy_from_slice(&6i32.to_le_bytes());
    state.mem.write(va::VEHICLES + 8 * nv, &v);
    state.mem.set_i32(va::VEHICLE_COUNT, nv as i32);
    let mut map = state.starmap();
    let fl = &mut map.fleets[f];
    (fl.owner, fl.flags, fl.x, fl.y) = (3, 0, hx + 1.0, hy);
    (fl.order, fl.kind, fl.tx, fl.ty) = (ig2_sim::fleet::order::ATTACK, ig2_sim::fleet::order::PLANET, home as f32, 0.0);
    state.store_starmap(&map);
    (state, home)
}
