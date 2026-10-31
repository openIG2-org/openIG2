//! The multiplayer core on a real save (`_local/saves/quick-1.sav`, ADR 0007): our save format round-trips the
//! state, two peers on loopback links run the same ticks from the same commands with identical hashes, a third
//! rejoins from a snapshot and the command log, a corrupted peer is reported as desynced, and peers playing
//! different empires and looking at different planets stay identical in the canonical view (ADR 0007 §5), each with
//! its own contact list and spy report queue (`re/rules/multiplayer.md` §5), two humans' movement orders and
//! sightings agree (`fleet-movement.md` §6.4). The real-time battles (§7, ADR 0011): two
//! humans fight in space and on the ground with orders, a player drops and rejoins mid-battle, a battle desync is
//! reported and resynced, and the host's log replayed into one game gives the peers' hashes.

mod common;

use ig2_net::{loopback, state_hash, Command, Host, Loopback, Peer, Snapshot, SpyOrder, StarmapCmd, Terms, DEFAULT_DELAY, TURN_TICKS};
use ig2_sim::game::{Game, GameRules, LocalView, MessageOrder, ViewMode};
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
    g.take_events();
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

    /// Pump `rounds` times without asking for progress (a held session stands still).
    fn idle(&mut self, rounds: usize) {
        for _ in 0..rounds {
            self.host.pump().unwrap();
            for (p, link) in &mut self.peers {
                p.poll(link, 3).unwrap();
            }
        }
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

/// Human 6 sends a letter to human 3 and takes it back with the diplomacy screen's "cancel last message" (menu 0x2e,
/// packet 0x40, `re/screens/diplomacy.md` §7 step 5): 6's slot is free to compose again and 3's waiting letter is
/// gone, on both peers.
#[test]
fn a_human_cancels_its_last_message_to_another_human() {
    let Some((rules, mut state)) = common::setup(SAVE) else { return };
    state.mem.set_u8(va::PLAYER_KI + 3, 1);
    let canonical = |state: SaveState| {
        let mut g = game(state, &rules);
        g.canonical_view(6);
        g
    };
    let trade = Terms { cat: 0, sub: 8, value: 0, sub2: -1, value2: -1 };
    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, canonical(state.clone()), DEFAULT_DELAY));
    s.join(2, 3, |_| Peer::new(2, 3, canonical(state.clone()), DEFAULT_DELAY));
    let screen = |op| Command::DiplomacyScreen { empire: 3, op };
    s.run_to(6, |p| {
        if (p.slot, p.turn) == (1, 2) {
            p.issue(screen(ig2_net::DiplomacyScreenOp::Sent));
            p.issue(Command::DiplomacyMessage { to: 3, declaration: false, terms: trade });
        }
    });
    for (p, _) in &s.peers {
        assert_eq!(contact(&p.game, 6, 3).map(|c| c.asked), Some(1));
        assert_eq!(contact(&p.game, 3, 6).map(|c| c.waiting), Some(1));
    }
    s.run_to(12, |p| {
        if (p.slot, p.turn) == (1, 7) {
            p.issue(screen(ig2_net::DiplomacyScreenOp::Cancelled));
        }
    });
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    for (p, _) in &s.peers {
        let (at6, at3) = (contact(&p.game, 6, 3).unwrap(), contact(&p.game, 3, 6).unwrap());
        assert_eq!((at6.asked, at6.screen, at3.waiting), (0, 0, 0));
    }
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

/// The entries' block ids of human `h`'s own inbox (`HumanSide::inbox`: three bytes, then `INBOX`, `INBOX_PLAYBACK`,
/// `INBOX_RANDOM`, then 0xDABDA0.. with the count at +0x30), or of the state's when `h` is `EN`.
fn inbox_ids(g: &Game, h: u8) -> Vec<i32> {
    let b = match g.state.humans.get(&h) {
        Some(s) => s.inbox.clone(),
        None => g.state.inbox_regions(),
    };
    let base = 3 + 100 * 0x88 + 100 * 0x54 + 400;
    let n = i32::from_le_bytes(b[base + 0x30..base + 0x34].try_into().unwrap());
    (0..n.max(0) as usize).map(|k| i32::from_le_bytes(b[3 + k * 0x88..7 + k * 0x88].try_into().unwrap())).collect()
}

/// `re/rules/multiplayer.md` §5.3: in story 1003 every human gets the radar offer (message 0x2347)
/// in its own inbox; the second human's answers run in its inbox and buy the radar for it (special 15 with it as the
/// sender: its 20000 paid, its bit in `RADAR_BUYERS`), and both peers agree on every turn.
#[test]
fn every_human_gets_its_own_radar_offer() {
    let Some((rules, mut state)) = common::setup(SAVE) else { return };
    state.mem.set_u8(va::PLAYER_KI + 3, 1);
    // `MessagesVege` (0xD909BA, unsaved): set once a dialogue has ended; a fresh save has none, and `Messages_Main`
    // opens a row only with it.
    state.mem.set_u8(0xD909BA, 1);
    // The save's clock stands (speed 0): the fastest speed on both, before the start.
    let canonical = |state: SaveState| {
        let mut g = game(state, &rules);
        g.canonical_view(6);
        g.set_speed(3, false);
        g
    };
    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, canonical(state.clone()), DEFAULT_DELAY));
    s.join(2, 3, |_| Peer::new(2, 3, canonical(state.clone()), DEFAULT_DELAY));
    s.run_to(4, |p| {
        if (p.slot, p.turn) == (1, 2) {
            p.issue(Command::StoryStart { id: 1003 });
        }
    });
    let offered = |g: &Game| inbox_ids(g, 6).contains(&0x2347) && inbox_ids(g, 3).contains(&0x2347);
    let mut turns = 4;
    while !offered(&s.peers[0].0.game) {
        turns += 50;
        s.run_to(turns, |_| {});
        assert!(turns < 3_000, "no offer after {turns} turns: 6 {:?}, 3 {:?}", inbox_ids(&s.peers[0].0.game, 6), inbox_ids(&s.peers[0].0.game, 3));
    }
    eprintln!("both offers by turn {turns}");
    let money = |g: &Game, e: usize| g.state.treasuries()[e].total;
    let before = (money(&s.peers[0].0.game, 6), money(&s.peers[0].0.game, 3));
    // 3 opens its messages screen and takes answer 0 (part 2: buy); 6 does nothing.
    let order = |o: MessageOrder| {
        let (kind, values) = o.to_message();
        Command::Message { kind, values }
    };
    let t = turns;
    let mut script = vec![order(MessageOrder::Enter), order(MessageOrder::Open { row: 0 }), order(MessageOrder::TypedOut)];
    script.extend([order(MessageOrder::Answer { k: 0 }), order(MessageOrder::TypedOut)]);
    s.run_to(t + 2 * script.len() as u64 + 20, |p| {
        if p.slot == 2 && p.turn > t && (p.turn - t) % 2 == 0 {
            if let Some(c) = script.get(((p.turn - t) / 2 - 1) as usize) {
                p.issue(c.clone());
            }
        }
    });
    let g = &s.peers[0].0.game;
    let after = (money(g, 6), money(g, 3));
    let mut st = g.state.story();
    let buyers = st.v_get(10039);
    assert_eq!(buyers, 1 << 3, "only 3 bought");
    assert!(before.1 - after.1 >= 20000.0 - 1e-6, "3 paid: {before:?} → {after:?}");
    assert_eq!(before.0, after.0, "6 paid nothing");
    assert_eq!(inbox_ids(g, 6), [0x2347], "6's own offer waits, unanswered");
    // The client's view: 3's peer shows its own inbox while it holds the game, then the canonical state again.
    let p2 = &mut s.peers[1].0;
    let h0 = state_hash(&p2.game, &[]);
    let own = p2.game.state.humans[&3].inbox.clone();
    p2.game.show_inbox(Some(3));
    assert_eq!(p2.game.state.inbox_regions(), own, "3's own inbox on its screen");
    p2.game.show_inbox(Some(3));
    p2.game.show_inbox(None);
    assert_eq!(state_hash(&p2.game, &[]), h0, "the canonical state is back");
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    assert!(agree(&s.peers) as u64 >= turns);
    assert_eq!(s.peers[0].0.game.state.humans, s.peers[1].0.game.state.humans);
    // The own inbox is synced state: a snapshot carries it.
    let snap = s.peers[1].0.snapshot();
    let (back, pending) = snap.restore(&rules).unwrap();
    assert_eq!(back.state.humans, s.peers[1].0.game.state.humans);
    assert_eq!(Snapshot::capture(&back, &pending, snap.turn, snap.tick).hash, snap.hash);
}

/// Deviation 395: a rename in multiplayer goes through the command stream (`Command::Rename`): the second human renames its
/// own fleet (the name and the custom-name bit land on both peers, the hashes agree); its rename of another's fleet is
/// refused. The client's local edit before the command (`Game::edit_local_name`) is swapped out of the shared state
/// whenever the game goes into the lockstep (`show_local_names(false)`): the hash doesn't see it.
#[test]
fn a_rename_goes_through_the_lockstep() {
    let Some((rules, mut state)) = common::setup(SAVE) else { return };
    // quick-1 has the pirates' fleets only: two of them become empire 3's (the second human's) and 6's.
    let (mine, theirs, other) = (11, 12, 3u8);
    state.fleets[mine][0x15] = other;
    state.fleets[theirs][0x15] = 6;
    state.mem.set_u8(va::PLAYER_KI + other as u32, 1);
    let canonical = |state: SaveState| {
        let mut g = game(state, &rules);
        g.canonical_view(6);
        g
    };
    // The local edit: shown while held, gone from the hash.
    let mut g = canonical(state.clone());
    let h0 = state_hash(&g, &[]);
    g.edit_local_name(true, mine, b"Raiders", true);
    assert_eq!(&g.state.fleets[mine][..8], b"Raiders\0");
    g.show_local_names(false);
    assert_eq!(state_hash(&g, &[]), h0, "the shared state is untouched");
    g.show_local_names(true);
    g.show_local_names(true);
    assert_eq!(g.state.fleets[mine][0x53] & 0x08, 0x08);

    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, canonical(state.clone()), DEFAULT_DELAY));
    s.join(2, other, |_| Peer::new(2, other, canonical(state.clone()), DEFAULT_DELAY));
    let rename = |index: usize, name: &[u8]| Command::Rename { fleet: true, index: index as u32, name: name.to_vec(), custom: true };
    s.run_to(20, |p| match (p.slot, p.turn) {
        (2, 3) => p.issue(rename(mine, b"Raiders")),
        (2, 4) => p.issue(rename(theirs, b"Stolen")),
        _ => {}
    });
    assert!(agree(&s.peers) as u64 >= 20);
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    for (p, _) in &s.peers {
        let f = &p.game.state.fleets;
        assert_eq!(&f[mine][..8], b"Raiders\0");
        assert_eq!(f[mine][0x53] & 0x08, 0x08, "the custom-name bit");
        assert_eq!(f[theirs][..0x14], state.fleets[theirs][..0x14], "another empire's fleet keeps its name");
    }
    // Once the shared state has caught up, the local edit is dropped.
    g = std::mem::replace(&mut s.peers[1].0.game, canonical(state.clone()));
    g.local_names.iter_mut().for_each(|n| n.sent = true);
    g.show_local_names(false);
    g.show_local_names(true);
    assert!(g.local_names.is_empty());
}

/// Two humans steer their fleets (`MULTIGET_Starmap` 0x15, `re/rules/fleet-movement.md` §6.4): the orders cross the
/// wire stamped with their issuer and both peers carry them out; a revealed wormhole is shown to both humans, as each
/// one's own machine would (the canonical view gives every human its local sightings), and the hashes agree.
#[test]
fn two_humans_give_movement_orders_in_lockstep() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    // 6's and 3's fleets of the battle test, 3's sent far away and idle (no battle).
    let mut state = common::space_clash(state);
    let (f6, f3) = (state.fleets.len() - 2, state.fleets.len() - 1);
    let mut map = state.starmap();
    (map.fleets[f3].x, map.fleets[f3].y, map.fleets[f3].order) = (250.0, -250.0, 0);
    state.store_starmap(&map);
    let e2 = 3u8;
    let (x6, y6) = (map.fleets[f6].x, map.fleets[f6].y);
    let (x3, y3) = (map.fleets[f3].x, map.fleets[f3].y);
    let (f6, f3) = (f6 as i32, f3 as i32);
    let canonical = |state: SaveState| {
        let mut g = game(state, &rules);
        g.canonical_view(6);
        let w = g.add_wormhole(x6 as i32 + 5, y6 as i32 + 5, -250, 250);
        assert_ne!(w, 0);
        g.reveal_wormhole(w);
        g
    };
    let to = |x: f32| (x * 0.5).to_bits();
    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, canonical(state.clone()), DEFAULT_DELAY));
    s.join(2, e2, |_| Peer::new(2, e2, canonical(state.clone()), DEFAULT_DELAY));
    let turns = 20;
    s.run_to(turns, |p| match (p.slot, p.turn) {
        (1, 2) => p.issue(Command::Starmap { order: StarmapCmd::Move { fleet: f6, order: 1, kind: 0, x: to(x6), y: to(y6) } }),
        (2, 3) => p.issue(Command::Starmap { order: StarmapCmd::Move { fleet: f3, order: 1, kind: 0, x: to(x3), y: to(y3) } }),
        // Appending a waypoint to the other human's fleet is carried out too (0x15 has no owner check).
        (2, 4) => p.issue(Command::Starmap { order: StarmapCmd::Move { fleet: f6, order: 13, kind: 0, x: 0f32.to_bits(), y: 0f32.to_bits() } }),
        _ => {}
    });
    assert_eq!(agree(&s.peers) as u64, turns);
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    for (p, _) in &s.peers {
        let m = p.game.state.starmap();
        let (a, b) = (&m.fleets[f6 as usize], &m.fleets[f3 as usize]);
        assert_eq!((a.order, a.tx, a.ty), (1, x6 * 0.5, y6 * 0.5), "6's move");
        assert!(a.first_waypoint > 0, "the appended waypoint");
        assert_eq!((b.order, b.tx, b.ty), (1, x3 * 0.5, y3 * 0.5), "the second human's move");
        let w = m.wormholes.last().expect("the wormhole");
        let ends = [&m.fleets[w.fleet_a as usize], &m.fleets[w.fleet_b as usize]];
        assert!(ends.iter().all(|f| f.visible[6] == 1 && f.visible[e2 as usize] == 1), "both humans see the revealed ends");
    }
}

/// Two humans' fleets meet: the real-time battle runs on the sim clock in canonical MP (`re/rules/multiplayer.md`
/// §7, `space-combat.md` §7.12–§7.13): both peers set it up in the same slot, the strategic clock waits, the orders
/// travel as lockstep commands, and once both players leave the finished battle's screen `SpaceWarVege` ends it and
/// the clock runs on. Every turn the peers' hashes agree; a snapshot taken mid-battle restores to the same hash.
#[test]
fn two_humans_fight_a_real_time_battle_in_lockstep() {
    use ig2_net::BattleOrder;
    use ig2_sim::game::{GameEvent, RealTimeEvent};
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
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
/// (`MULTIGET_Colony`, `Command::Colony` with `ColonyOrder::Ground`): without them a defended planet stays contested;
/// with them the war ends with one side holding the field. An order for another empire's unit is refused (ADR 0011
/// §2). An undefended planet falls at once (`StartGrWar` finds the field held).
#[test]
fn two_humans_fight_a_ground_war_in_lockstep() {
    use ig2_sim::game::{ColonyOrder, GameEvent, RealTimeEvent};
    use ig2_sim::ground::GroundOrder;
    let Some((rules, base)) = common::setup(SAVE) else { return };
    let canonical = |state: SaveState| {
        let mut g = game(state, &rules);
        g.canonical_view(6);
        g
    };
    for (defended, ordered) in [(true, false), (true, true), (false, false)] {
        let (state, home) = landing(base.clone(), defended);
        let mut s = Session::new();
        s.join(1, 6, |_| Peer::new(1, 6, canonical(state.clone()), DEFAULT_DELAY));
        s.join(2, 3, |_| Peer::new(2, 3, canonical(state.clone()), DEFAULT_DELAY));
        let (mut started, mut ended, mut clock, mut turns_running, mut refused) = (None, None, None, 0, false);
        let order = |unit: usize, target: i32| Command::from(ColonyOrder::Ground { planet: home as i32, order: GroundOrder::Target { unit: unit as i32, target } });
        let limit = if ordered { 4000 } else { 150 };
        while s.peers[0].0.turn < limit && ended.is_none() {
            let target = s.peers[0].0.turn + 50;
            s.run_to(target.min(limit), |p| {
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
                if p.game.state.mem.i32(0xBAD13C) == 0 {
                    return;
                }
                if p.slot == 1 {
                    turns_running += 1;
                    // The strategic clock stands still while the war runs.
                    assert_eq!(Some(p.game.state.mem.i32(va::IDO)), clock);
                }
                if !ordered || p.turn % 10 != 0 {
                    return;
                }
                // Each player's live units attack the first live enemy unit, or else an enemy defence building.
                let f = p.game.state.ground_field(home).unwrap();
                let me = p.empire as i32;
                let live = |i: usize| f.alive[i] && f.units[i].owner > 0;
                let enemy = (0..f.units.len()).find(|&i| live(i) && f.units[i].owner != me).map(|i| i as i32).or_else(|| {
                    let owner = p.game.state.mem.u8(va::PLANETS + (home * PLANET_SIZE) as u32 + 0x20) as i32;
                    let slots = p.game.state.mem.i32(va::BUILDING_SLOTS + 4 * home as u32).max(0) as usize;
                    (owner != me).then(|| (1..=slots).find(|&k| matches!(p.game.state.buildings[home][k][0], 0x1c..=0x23)).map(|k| -(k as i32))).flatten()
                });
                let Some(target) = enemy else { return };
                for i in (0..f.units.len()).filter(|&i| live(i) && f.units[i].owner == me) {
                    p.issue(order(i, target));
                }
                // An order for the other side's unit changes nothing on any peer.
                if !refused && p.slot == 1 {
                    if let Some(theirs) = (0..f.units.len()).find(|&i| live(i) && f.units[i].owner != me) {
                        let (mut g, pending) = p.snapshot().restore(&rules).unwrap();
                        let before = state_hash(&g, &pending);
                        order(theirs, -1).apply(&mut g, p.empire);
                        Command::from(ColonyOrder::Ground { planet: home as i32, order: GroundOrder::Move { unit: theirs as i32, x: 1, y: 1 } }).apply(&mut g, p.empire);
                        assert_eq!(state_hash(&g, &pending), before, "refused");
                        refused = true;
                    }
                }
            });
        }
        assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
        assert_eq!(agree(&s.peers) as u64, s.peers.iter().map(|(p, _)| p.turn).min().unwrap());
        let (planet, holder) = started.expect("the landing started a real-time ground war");
        assert_eq!(planet, home);
        let g = &s.peers[0].0.game;
        let owner = g.state.mem.u8(va::PLANETS + (home * PLANET_SIZE) as u32 + 0x20);
        eprintln!("defended {defended}, ordered {ordered}: holder at start {holder:?}, ended {ended:?}, {turns_running} turns running, owner {owner}");
        match (defended, ordered) {
            (true, false) => {
                assert_eq!((holder, ended), (None, None));
                assert!(turns_running > 100);
                // `WF::Refresh` ran every tick: the call-in timers of every empire reached their caps.
                let f = g.state.ground_field(home).unwrap();
                assert!(f.timers.iter().all(|t| *t == (200, 300)), "{:?}", f.timers);
                assert_eq!(g.state.mem.i32(va::IDO), clock.unwrap());
            }
            (true, true) => {
                assert!(refused);
                let (p, h) = ended.expect("the ordered war ended");
                assert_eq!((p, holder), (home, None));
                assert!(h == 3 || h == 6);
                assert_eq!((owner, g.state.mem.i32(0xBAD13C)), (h as u8, 0), "the holder keeps or takes the planet, the MP flag cleared");
                assert!(g.state.mem.i32(va::IDO) > clock.unwrap(), "the clock runs on");
            }
            _ => {
                assert_eq!(holder, Some(3));
                assert_eq!(ended, Some((home, 3)));
                assert_eq!((owner, g.state.mem.i32(0xBAD13C)), (3, 0), "captured, the MP flag cleared");
                assert!(g.state.mem.i32(va::IDO) > clock.unwrap(), "the clock runs on");
            }
        }
    }
}

/// Single player (`ground-combat.md` §11.6): an AI fleet lands on the human's planet, the war goes real time
/// (`GroundWarIndit`: the owner is human-controlled, no lopsided war) and `StartGrWar` runs. `FRISSIT_Grwar` runs
/// `WF::MI` and `WF::Refresh` only for the planet on the colony screen (`AktKep == 3`, `AktPlanet`): away from it the war
/// stands still (the call-in timers stay where `StartGrWar` set them, 140/240); on it the battle runs to
/// `GroundWarFinished` from `WF::Refresh`'s end countdown, the holder keeps or takes the planet. An undefended planet
/// falls at once (`StartGrWar` finds the field held), screen or not.
#[test]
fn single_player_ground_war_runs_on_its_colony_screen() {
    use ig2_sim::game::{GameEvent, RealTimeEvent};
    let Some((rules, base)) = common::setup(SAVE) else { return };
    for defended in [true, false] {
        let (mut state, home) = landing(base.clone(), defended);
        state.mem.set_u8(va::PLAYER_KI + 3, 0);
        state.mem.set_u8(va::MULTIPLAYER, 0);
        state.mem.set_i32(va::EN, 6);
        let mut g = game(state, &rules);
        g.sp_battles.live = true;
        let won = |g: &Game| g.state.mem.i32(va::BATTLES_WON + 4 * 3);
        let won_before = won(&g);
        let (mut started, mut ended, mut ticks) = (None, None, 0);
        while ticks < 6000 && ended.is_none() {
            // The colony screen opens 300 ticks after the war started.
            g.sp_battles.colony = started.filter(|&(_, at)| ticks >= at + 300).map(|_| home);
            // `StartGrWar` paused the game (the player unpauses on the colony screen).
            g.set_pause(false, false);
            for _ in 0..5 {
                g.colony_ui_tick();
            }
            g.advance_clock();
            g.run_tick();
            for e in g.take_events() {
                match e {
                    GameEvent::RealTime(RealTimeEvent::GroundStarted { planet, holder }) => {
                        assert_eq!(planet, home);
                        if holder.is_some() {
                            ended = holder.map(|h| (h, ticks));
                        }
                        started = Some((holder, ticks));
                    }
                    GameEvent::RealTime(RealTimeEvent::GroundEnded { planet, holder }) => {
                        assert_eq!(planet, home);
                        ended = Some((holder, ticks));
                    }
                    _ => {}
                }
            }
            if let Some((None, at)) = started {
                if ticks == at + 299 {
                    let f = g.state.ground_field(home).unwrap();
                    assert!(f.timers.iter().all(|t| *t == (0x8c, 0xf0)), "no WF::Refresh away from the screen: {:?}", f.timers);
                    assert!(ended.is_none());
                }
            }
            ticks += 1;
        }
        let (holder, at) = started.expect("the landing started a real-time ground war");
        let (h, end) = ended.expect("the war ended");
        let owner = g.state.mem.u8(va::PLANETS + (home * PLANET_SIZE) as u32 + 0x20) as i32;
        eprintln!("defended {defended}: started at {at}, holder {holder:?}, ended at {end} held by {h}, owner {owner}");
        assert!(h == 3 || h == 6);
        assert_eq!(owner, h, "the holder keeps or takes the planet");
        assert_eq!(won(&g), won_before + (h == 3) as i32);
        assert_eq!(g.state.mem.u8(va::PLANETS + (home * PLANET_SIZE) as u32 + 0x64), 0, "the war is over");
        assert_eq!(g.state.mem.i32(0xBAD13C), 0, "no MP flag in single player");
        if defended {
            assert_eq!(holder, None);
            assert!(end > at + 300, "it ran on the colony screen only");
        } else {
            assert_eq!((holder, end, h), (Some(3), at, 3));
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

// ------------------------------------------------------------------------------------------------ ADR 0011

/// The battle slot a peer's events say started, if any.
fn space_started(p: &Peer) -> Option<usize> {
    use ig2_sim::game::{GameEvent, RealTimeEvent};
    p.events.iter().find_map(|e| if let GameEvent::RealTime(RealTimeEvent::SpaceStarted { slot }) = e { Some(*slot) } else { None })
}

fn space_over(p: &Peer) -> bool {
    use ig2_sim::game::{GameEvent, RealTimeEvent};
    p.events.iter().any(|e| matches!(e, GameEvent::RealTime(RealTimeEvent::SpaceOver { .. })))
}

/// Each of the peer's units in battle `b` attacks the first enemy unit.
fn attack_all(p: &mut Peer, b: usize) {
    use ig2_net::BattleOrder;
    let battle = &p.game.realtime.slots[b];
    let me = p.empire as i32;
    let mine: Vec<usize> = (0..battle.units.len()).filter(|&u| battle.units[u].alive && battle.units[u].owner == me).collect();
    let Some(target) = (0..battle.units.len()).find(|&u| battle.units[u].alive && battle.units[u].owner > 0 && battle.units[u].owner != me) else { return };
    for u in mine {
        p.issue(Command::SpaceOrder { battle: b as u8, order: BattleOrder::Attack { unit: u as u16, target: target as u16 } });
    }
}

/// Replays the host's bundle log into one `Game` from the start state, as single player runs it (ADR 0011 §7), and
/// checks every turn's hash against the ones a peer reported. Returns the turns replayed.
fn replay_log(start: Game, log: &[ig2_net::Bundle], hashes: &[(u64, u64)]) -> usize {
    let mut g = start;
    let mut pending: Vec<Command> = Vec::new();
    for (n, b) in log.iter().enumerate() {
        assert_eq!(b.turn, n as u64);
        for k in 0..TURN_TICKS {
            for c in std::mem::take(&mut pending) {
                c.apply(&mut g, 0);
            }
            if k == 0 {
                for s in &b.commands {
                    s.command.apply(&mut g, s.issuer);
                }
            }
            let from = g.events.len();
            g.advance_clock();
            g.run_tick();
            let events = g.take_events();
            pending.extend(events[from..].iter().filter_map(Command::from_event));
        }
        if let Some((_, h)) = hashes.iter().find(|(t, _)| *t == b.turn) {
            assert_eq!(state_hash(&g, &pending), *h, "turn {}", b.turn);
        }
    }
    log.len()
}

/// `SpWar_MultiCsata_vegetert[e]`: `e` has left the battle screen.
fn left_screen(g: &Game, e: u32) -> i32 {
    g.state.mem.i32(0xBAD118 + 4 * e)
}

/// Two humans (6, 3) in a session started by `Game::start_multiplayer`, run until their space battle has run a few
/// ticks; `act` sees each peer at each turn with the battle's slot once it started.
fn into_space_battle<'r>(rules: &'r GameRules, state: &SaveState, mut act: impl FnMut(&mut Peer<'r>, Option<usize>)) -> (Session<'r>, usize) {
    let start = |state: SaveState| {
        let mut g = game(state, rules);
        g.start_multiplayer(6, &[6, 3]);
        g
    };
    let mut s = Session::new();
    s.join(1, 6, |_| Peer::new(1, 6, start(state.clone()), DEFAULT_DELAY));
    s.join(2, 3, |_| Peer::new(2, 3, start(state.clone()), DEFAULT_DELAY));
    let mut slot = None;
    for _ in 0..40 {
        let target = s.peers[0].0.turn + 25;
        s.run_to(target, |p| {
            slot = slot.or(space_started(p));
            p.events.clear();
            act(p, slot);
        });
        if slot.is_some_and(|b| s.peers.iter().all(|(p, _)| p.game.realtime.slots[b].active && p.game.realtime.slots[b].tick > 20)) {
            return (s, slot.unwrap());
        }
    }
    panic!("no real-time space battle");
}

/// A human drops in the middle of a space battle (ADR 0011 §4–§5): the host holds the turn clock (nothing moves),
/// the grace runs out and `PlayerAway` lets the battle go on, the player rejoins from a snapshot and the log while it
/// still runs and `PlayerBack` puts them on its screen again, so it ends only when both have left. A peer's own
/// `PlayerAway` is dropped by the host. Every turn agrees, and the host's log replayed into one game gives the same
/// hashes (§7).
#[test]
fn a_player_drops_mid_battle_and_rejoins() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let mut ordered = [false; 2];
    let (mut s, b) = into_space_battle(&rules, &state, |p, slot| {
        let Some(b) = slot else { return };
        let k = p.slot as usize - 1;
        if !ordered[k] && p.game.realtime.slots[b].active {
            ordered[k] = true;
            attack_all(p, b);
            if p.slot == 1 {
                p.issue(Command::PlayerAway);
            }
        }
    });
    let start_state = |state: SaveState| {
        let mut g = game(state, &rules);
        g.start_multiplayer(6, &[6, 3]);
        g
    };
    s.run_to(s.peers[0].0.turn + 5, |p| p.events.clear());
    assert!(s.host.log.iter().flat_map(|b| &b.commands).all(|c| c.command != Command::PlayerAway), "a peer's PlayerAway is dropped");
    assert_eq!(left_screen(&s.peers[0].0.game, 6), 0);

    // Slot 2 drops; a battle runs, so the host holds.
    assert!(s.peers[0].0.game.realtime_running());
    s.host.remove_peer(2);
    let (gone, _) = s.peers.remove(1);
    s.host.hold(true);
    s.idle(10);
    let held_at = s.peers[0].0.turn;
    s.idle(50);
    assert_eq!(s.peers[0].0.turn, held_at, "a held session stands still");
    // The grace runs out: 3 is away, the battle goes on.
    s.host.inject(3, Command::PlayerAway);
    s.host.hold(false);
    let away_at = s.peers[0].0.turn;
    s.run_to(away_at + 10, |p| p.events.clear());
    assert_eq!(left_screen(&s.peers[0].0.game, 3), 1, "away counts as having left the screen");
    assert!(s.peers[0].0.game.realtime.slots[b].active, "the battle still runs");

    // 3 comes back while it runs: a snapshot from 6's machine, the bundles since, and `PlayerBack`.
    let file = s.peers[0].0.snapshot().to_file(0, true);
    let (snap, _) = Snapshot::from_file(&file).unwrap();
    let rules_ref = &rules;
    s.join(2, 3, |first| Peer::from_snapshot(2, 3, &snap, rules_ref, DEFAULT_DELAY, first).unwrap());
    s.host.send_log(2, snap.turn);
    s.host.inject(3, Command::PlayerBack);
    // Both fight on; each leaves the screen once the battle is over, 6 first: the battle waits for 3.
    let (mut over, mut sent, mut waited) = ([false; 2], [false; 2], false);
    for _ in 0..60 {
        let target = s.peers[0].0.turn + 50;
        s.run_to(target, |p| {
            let k = p.slot as usize - 1;
            over[k] |= space_over(p);
            p.events.clear();
            if over[k] && !sent[k] && (k == 0 || waited) {
                sent[k] = true;
                p.issue(Command::SpaceBattleLeft);
            }
            if k == 1 && left_screen(&p.game, 6) == 1 && left_screen(&p.game, 3) == 0 {
                assert_eq!(p.game.state.mem.i32(va::MP_SPACE_BATTLE), 1, "6 has left, 3 is back: the battle waits");
                waited = true;
            }
        });
        if sent.iter().all(|&l| l) && s.peers.iter().all(|(p, _)| p.game.state.mem.i32(va::MP_SPACE_BATTLE) == 0) {
            break;
        }
    }
    assert!(waited, "the battle waited for the player who came back");
    assert!(s.peers.iter().all(|(p, _)| !p.game.realtime.slots[b].active), "SpaceWarVege closed the slot");
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    let n = agree(&s.peers);
    assert!(n > 0);
    // The log is the single-player path: one game replaying it reproduces 6's hashes, and 3's before it dropped.
    let turns = replay_log(start_state(state.clone()), &s.host.log, &s.peers[0].0.hashes);
    assert_eq!(turns as u64, s.host.next_turn());
    replay_log(start_state(state.clone()), &s.host.log, &gone.hashes);
    eprintln!("battle slot {b}: held at {held_at}, away at {away_at}, rejoined at {}, {n} turns agreed, {turns} replayed", snap.turn);
}

/// A player who rejoins mid-battle has no battle screen up (no `SpaceStarted` reaches it), so only
/// `Game::space_battle_waits_on` tells its machine that the strategic clock waits for its 0x44d. Each machine leaving
/// on that once the battle is decided (the client's `mp_battle_check`) ends it on every peer and the clock runs again.
#[test]
fn a_rejoined_player_without_the_screen_lets_the_battle_end() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let mut ordered = [false; 2];
    let (mut s, b) = into_space_battle(&rules, &state, |p, slot| {
        let Some(b) = slot else { return };
        let k = p.slot as usize - 1;
        if !ordered[k] && p.game.realtime.slots[b].active {
            ordered[k] = true;
            attack_all(p, b);
        }
    });
    s.host.remove_peer(2);
    s.peers.remove(1);
    let file = s.peers[0].0.snapshot().to_file(0, true);
    let (snap, _) = Snapshot::from_file(&file).unwrap();
    let rules_ref = &rules;
    s.join(2, 3, |first| Peer::from_snapshot(2, 3, &snap, rules_ref, DEFAULT_DELAY, first).unwrap());
    s.host.send_log(2, snap.turn);
    s.host.inject(3, Command::PlayerBack);
    let clock = s.peers[0].0.game.state.mem.i32(va::IDO);
    let mut sent = [false; 2];
    for _ in 0..60 {
        let target = s.peers[0].0.turn + 50;
        s.run_to(target, |p| {
            p.events.clear();
            let k = p.slot as usize - 1;
            if let Some(slot) = p.game.space_battle_waits_on(p.empire) {
                if p.game.realtime.slots[slot].ended && !sent[k] {
                    sent[k] = true;
                    p.issue(Command::SpaceBattleLeft);
                }
            }
        });
        if s.peers.iter().all(|(p, _)| p.game.state.mem.i32(va::MP_SPACE_BATTLE) == 0) {
            break;
        }
    }
    assert!(sent.iter().all(|&l| l), "both machines saw the battle wait on them: {sent:?}");
    assert!(s.peers.iter().all(|(p, _)| p.game.state.mem.i32(va::MP_SPACE_BATTLE) == 0 && !p.game.realtime.slots[b].active));
    s.run_to(s.peers[0].0.turn + 20, |p| p.events.clear());
    assert!(s.peers.iter().all(|(p, _)| p.game.state.mem.i32(va::IDO) > clock), "the strategic clock runs again");
    assert!(s.host.desyncs.is_empty(), "{:?}", s.host.desyncs);
    assert!(agree(&s.peers) > 0);
}

/// A peer's battle state drifts (as a stray `rand()` in its battle tick would): the host reports it at that turn, and
/// the rejoin path resyncs it mid-battle (ADR 0011 §6).
#[test]
fn a_desync_inside_a_battle_is_reported_and_resynced() {
    let Some((rules, state)) = common::setup(SAVE) else { return };
    let state = common::space_clash(state);
    let (mut s, b) = into_space_battle(&rules, &state, |_, _| {});
    // Both at the same turn, then slot 2's battle drifts.
    let at = s.peers.iter().map(|(p, _)| p.turn).max().unwrap();
    s.run_to(at, |p| p.events.clear());
    let peer2 = &mut s.peers[1].0;
    let drift_turn = peer2.turn;
    let u = peer2.game.realtime.slots[b].units.iter().position(|u| u.alive).unwrap();
    peer2.game.realtime.slots[b].units[u].alive = false;
    s.run_to(drift_turn + 5, |p| p.events.clear());
    s.host.pump().unwrap();
    let d = s.host.desyncs.first().expect("a desync").clone();
    assert!(d.turn == drift_turn && d.slots == [2], "{d:?}");
    // Resync: slot 2 reloads 6's state and the bundles since.
    s.host.remove_peer(2);
    s.peers.remove(1);
    let snap = s.peers[0].0.snapshot();
    assert!(snap.restore(&rules).unwrap().0.realtime.slots[b].active, "mid-battle");
    let rules_ref = &rules;
    s.join(2, 3, |first| Peer::from_snapshot(2, 3, &snap, rules_ref, DEFAULT_DELAY, first).unwrap());
    s.host.send_log(2, snap.turn);
    let desyncs = s.host.desyncs.len();
    let end = s.peers[0].0.turn + 60;
    s.run_to(end, |p| p.events.clear());
    assert_eq!(s.host.desyncs.len(), desyncs, "in step again: {:?}", s.host.desyncs);
    let (p1, p2) = (&s.peers[0].0, &s.peers[1].0);
    let mut n = 0;
    for (t, h) in &p2.hashes {
        if let Some((_, h1)) = p1.hashes.iter().find(|(t1, _)| t1 == t) {
            assert_eq!(h, h1, "turn {t}");
            n += 1;
        }
    }
    assert!(n >= 50, "{n}");
}

/// Every ground-war message recorded from the exe's own `MULTIGET_Colony` (`_local/traces/grt-orders*.jsonl`, the
/// `gorder` events `ground_rt_trace.rs` replays exactly through `War::order`) crosses the lockstep unchanged: as a
/// `Command::Colony` in a peer's input and in the host's bundle, it decodes to the same `ColonyOrder`.
#[test]
fn recorded_ground_orders_cross_the_wire() {
    use ig2_net::{wire, Bundle, Msg, Stamped};
    use ig2_sim::game::ColonyOrder;
    let dir = std::env::var_os("IG2_TRACE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("{} missing; skipping", dir.display());
        return;
    };
    // `"key": value` of a flat JSON number or number array (the trace's own layout, one event per line).
    let field = |line: &str, key: &str| -> Option<Vec<i32>> {
        let at = line.find(&format!("\"{key}\": "))? + key.len() + 4;
        let rest = &line[at..];
        let end = if rest.starts_with('[') { rest.find(']')? + 1 } else { rest.find([',', '}'])? };
        Some(rest[..end].trim_matches(['[', ']']).split(',').filter(|s| !s.trim().is_empty()).map(|s| s.trim().parse().unwrap()).collect())
    };
    let mut n = 0;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("grt-orders") || !name.ends_with(".jsonl") {
            continue;
        }
        for line in std::fs::read_to_string(e.path()).unwrap().lines().filter(|l| l.starts_with("{\"k\": \"gorder\"")) {
            let (t, v) = (field(line, "type").unwrap()[0], field(line, "vals").unwrap());
            let order = ColonyOrder::from_message(t, &v[1..]).unwrap_or_else(|| panic!("{name}: message {t:#x} {v:?}"));
            assert!(matches!(order, ColonyOrder::Ground { .. }));
            let c = Command::from(order);
            let input: Msg = wire::from_slice(&wire::to_vec(&Msg::Input { turn: 7, commands: vec![c.clone()] }).unwrap()).unwrap();
            let bundle: Msg = wire::from_slice(&wire::to_vec(&Msg::Bundle(Bundle { turn: 7, commands: vec![Stamped { issuer: v[0] as u8, command: c.clone() }] })).unwrap()).unwrap();
            let Msg::Input { commands, .. } = input else { panic!() };
            let Msg::Bundle(b) = bundle else { panic!() };
            assert_eq!((&commands[0], &b.commands[0].command, b.commands[0].issuer as i32), (&c, &c, v[0]));
            let Command::Colony { kind, values } = &b.commands[0].command else { panic!() };
            assert_eq!(ColonyOrder::from_message(*kind, values), Some(order));
            n += 1;
        }
    }
    eprintln!("{n} recorded ground-war messages crossed the wire");
}
