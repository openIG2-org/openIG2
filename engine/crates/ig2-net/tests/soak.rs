//! The desync soak (TASKS.md → Multiplayer over the internet): long generated-galaxy games of 2–4 humans and AIs on
//! loopback links, every peer issuing random orders of every kind (fleets, colonies, research, designs, info shares,
//! trade, diplomacy, spies, messages, renames) and fighting the battles that come of it (real-time space battles
//! and ground wars), with an MP save and a drop and rejoin on the way. Every pair of peers must report the same state
//! hash for every turn both ran, and the host must see no desync.
//!
//! `IG2_SOAK_TURNS` and `IG2_SOAK_SEED` scale and vary it; the long variant is `#[ignore]`d (`cargo test --release -p
//! ig2-net --test soak -- --ignored`, `IG2_SOAK_SEEDS` games). `IG2_SOAK_STOP=T` stops every peer at turn T and prints
//! where their states differ (a desync's diagnosis); `SOAK_EVENTS=1` prints the kinds of sim events the games had.

#[allow(dead_code)]
mod common;

use std::time::Duration;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_net::galaxy::GalaxyOptions;
use ig2_net::lobby::{LobbyClient, LobbyHost, Settings, Status};
use ig2_net::session::{ClientSession, HostSession, MpSave, SessionEvent, SessionSettings};
use ig2_net::{loopback, state_hash, BattleOrder, Command, Peer, ResearchCmd, SpyOrder, StarmapCmd, Terms};
use ig2_sim::game::{ColonyOrder, DesignOrder, GameRules, InfoOrder, MessageOrder};
use ig2_sim::ground::GroundOrder;

const RULES: [u8; 32] = [0x5a; 32];
/// `MP_SPACE_BATTLE` (`SpWar_MultiCsata`), `SpWar_MultiCsata_vegetert[e]`, the running ground war and its planet.
const SPACE_RUNNING: u32 = 0xBAD114;
const SPACE_LEFT: u32 = 0xBAD118;
const GROUND_RUNNING: u32 = 0xBAD13C;
const GROUND_PLANET: u32 = 0xBAD140;

/// xorshift64*: each peer's own stream of choices (not the sim's).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }

    fn pick<T: Copy>(&mut self, v: &[T]) -> Option<T> {
        (!v.is_empty()).then(|| v[self.below(v.len())])
    }

    fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1) as usize) as i32
    }
}

/// What the run saw, for the report.
#[derive(Debug, Default)]
struct Seen {
    orders: [u32; 18],
    space_battles: u32,
    ground_wars: u32,
    saves: u32,
    rejoins: u32,
    /// Every sim event's kind (`SOAK_EVENTS=1` prints them): what the games had in them.
    events: std::collections::BTreeMap<String, u32>,
}

// ------------------------------------------------------------------------------------------------ the orders

/// A random order of kind `k` from `p`'s point of view: mostly sensible targets read from its own state, now and then
/// another empire's or out of range (those must be refused alike on every peer).
fn order(p: &Peer, k: usize, r: &mut Rng) -> Option<Command> {
    let st = &p.game.state;
    let e = p.empire;
    let map = st.starmap();
    let fleets: Vec<i32> = (1..map.fleets.len()).filter(|&f| map.fleets[f].alive() && map.fleets[f].owner == e).map(|f| f as i32).collect();
    let others: Vec<i32> = (1..map.fleets.len()).filter(|&f| map.fleets[f].alive() && map.fleets[f].owner != e && (1..9).contains(&map.fleets[f].owner)).map(|f| f as i32).collect();
    let planets: Vec<i32> = (1..map.planets.len()).filter(|&q| map.planets[q].owner == e).map(|q| q as i32).collect();
    let n_planets = st.planet_count() as i32;
    let any_planet = |r: &mut Rng| r.range(1, n_planets.max(1));
    // Mostly own, sometimes anyone's: the refusals must agree too.
    let fleet = |r: &mut Rng| if r.below(8) == 0 { r.range(0, map.fleets.len() as i32) } else { r.pick(&fleets).unwrap_or(1) };
    let planet = |r: &mut Rng| if r.below(8) == 0 { any_planet(r) } else { r.pick(&planets).unwrap_or(1) };
    let empires: Vec<i32> = (1..9).filter(|&o| o != e as i32 && st.mem.u8(ig2_sim::save::va::EMPIRE_ALIVE + o as u32) != 0).collect();
    let starmap = |order: StarmapCmd| Some(Command::Starmap { order });
    match k {
        // Fleets: move somewhere near a planet, attack a fleet or planet, add a waypoint, split, transfer.
        0 => {
            let q = any_planet(r) as usize;
            let (x, y) = map.planets.get(q).map_or((0.0, 0.0), |s| (s.x + r.range(-20, 20) as f32, s.y + r.range(-20, 20) as f32));
            let order = if r.below(4) == 0 { 13 } else { 1 };
            starmap(StarmapCmd::Move { fleet: fleet(r), order, kind: 0, x: x.to_bits(), y: y.to_bits() })
        }
        // Attacks, mostly on the other humans (their fights are the real-time ones).
        1 => {
            let human = |o: u8| o != e && st.mem.u8(ig2_sim::save::va::PLAYER_KI + o as u32) == 1;
            let rivals: Vec<i32> = (1..map.planets.len()).filter(|&q| human(map.planets[q].owner)).map(|q| q as i32).collect();
            let rival_fleets: Vec<i32> = others.iter().copied().filter(|&f| human(map.fleets[f as usize].owner)).collect();
            let (kind, target) = match (r.below(6), r.pick(&rivals), r.pick(&rival_fleets)) {
                (0, _, _) => (1, any_planet(r)),
                (1, _, _) if !others.is_empty() => (2, r.pick(&others).unwrap()),
                (2, _, Some(f)) => (2, f),
                (_, Some(q), _) => (1, q),
                _ => (1, any_planet(r)),
            };
            starmap(StarmapCmd::Move { fleet: fleet(r), order: 2, kind, x: (target as f32).to_bits(), y: 0 })
        }
        2 => match r.below(3) {
            0 => starmap(StarmapCmd::Split { fleet: fleet(r) }),
            1 => {
                let (a, b) = (fleet(r), if r.below(2) == 0 { fleet(r) } else { -planet(r) });
                starmap(StarmapCmd::Transfer { from: a, to: b, design: r.range(0, 12), ship: 0, count: r.range(1, 5) })
            }
            _ => starmap(StarmapCmd::Rally { planet: planet(r), target: r.pick(&fleets).unwrap_or(0) }),
        },
        // Planets: tax, autobuild, aggressiveness; satellites, colonising, selling stations.
        3 => {
            let (what, value) = match r.below(4) {
                0 => (8, r.range(0, 7)),
                1 => (9, r.range(0, 10)),
                2 => (10, r.range(0, 3)),
                _ => (r.range(1, 7), 0),
            };
            starmap(StarmapCmd::Setting { planet: planet(r), what, value })
        }
        4 => match r.below(3) {
            0 => starmap(StarmapCmd::Satellite { planet: planet(r), kind: r.range(0, 2) }),
            1 => starmap(StarmapCmd::Colonise { planet: any_planet(r), kind: r.range(1, 2) }),
            _ => starmap(StarmapCmd::SellStation { planet: planet(r), index: r.range(1, 3) }),
        },
        // Production lines.
        5 => {
            let designs = st.combat().designs.get(e as usize).map_or(1, Vec::len) as i32;
            let op = r.range(1, 5);
            let value = if op == 4 { r.range(0, designs - 1) } else { r.range(1, 10) };
            starmap(StarmapCmd::Production { op, planet: planet(r), line: r.range(0, 1), slot: r.range(0, 3), value })
        }
        // Colony buildings.
        6 => {
            let q = planet(r);
            let kinds = p.game.rules.side.colony.buildings.len() as i32;
            Some(Command::from(match r.below(5) {
                0 | 1 => ColonyOrder::Build { planet: q, kind: r.range(0, kinds - 1), x: r.range(0, 40), y: r.range(0, 40) },
                2 => ColonyOrder::Switch { planet: q, slot: r.range(1, 12) },
                3 => ColonyOrder::Priority { planet: q, slot: r.range(1, 12) },
                _ if r.below(4) == 0 => ColonyOrder::Demolish { planet: q, slot: r.range(1, 12) },
                _ => ColonyOrder::Repair { planet: q, slot: r.range(1, 12) },
            }))
        }
        // Research.
        7 => {
            let tech = st.tech();
            let open: Vec<i32> = (0..ig2_sim::colony_side::TECHS).filter(|&t| tech.status(t, e as usize) == 3).map(|t| t as i32).collect();
            let order = match r.below(4) {
                0 => ResearchCmd::Stop { tech: st.research().current[e as usize] },
                1 => ResearchCmd::Urgent,
                _ => ResearchCmd::Start { tech: r.pick(&open).unwrap_or(0) },
            };
            Some(Command::Research { order })
        }
        // Designs: a copy of one stored as new or edited, units sold or refitted.
        8 => {
            let designs = st.combat().designs.get(e as usize).cloned().unwrap_or_default();
            if designs.len() < 2 {
                return None;
            }
            let d = r.range(1, designs.len() as i32 - 1);
            Some(Command::from(match r.below(4) {
                0 | 1 => DesignOrder::Store { mode: if r.below(2) == 0 { 3 } else { 0 }, design: d, record: designs[d as usize].raw, copy: r.below(3) == 0 },
                2 => DesignOrder::Refit { design: d, holder: 0 },
                _ => DesignOrder::Sell { design: d, mode: 2, holder: r.pick(&fleets).unwrap_or(0) },
            }))
        }
        // The info screen's money shares and limit.
        9 => Some(Command::from(if r.below(3) == 0 {
            InfoOrder::Limit { limit: r.range(0, 50_000) as f64 }
        } else {
            let a = r.range(0, 100) as f64;
            let b = r.range(0, 100 - a as i32) as f64;
            InfoOrder::Shares { sliders: [a / 100.0, b / 100.0, (100.0 - a - b) / 100.0] }
        })),
        // Trade: buy units of a live offer for one of its planets.
        10 => {
            let trade = st.trade();
            let live: Vec<usize> = (0..trade.offers.len()).filter(|&i| ig2_sim::trade::class(&trade.offers[i]) >= 1).collect();
            let i = r.pick(&live)?;
            let qty = r.range(1, 4);
            let mut d = Vec::with_capacity(ig2_sim::trade::DELIVERY);
            for v in [planet(r), r.range(1, 20), i as i32, qty, ig2_sim::trade::price(&trade.offers[i], qty)] {
                d.extend_from_slice(&v.to_le_bytes());
            }
            d.extend_from_slice(&trade.offers[i][..]);
            d.resize(ig2_sim::trade::DELIVERY, 0);
            Some(Command::Buy { delivery: d })
        }
        // Diplomacy: talk, offers, demands, negotiations; answers to whoever.
        11 => {
            let to = r.pick(&empires)?;
            let terms = match r.below(4) {
                0 => Terms { cat: 6, sub: r.pick(&[0, 1, 2, 3, 4, 6, 8, 9])?, value: 0, sub2: -1, value2: -1 },
                1 => Terms { cat: 0, sub: r.range(0, 8), value: r.pick(&empires).unwrap_or(0), sub2: -1, value2: -1 },
                _ => Terms { cat: r.range(1, 3), sub: r.range(0, 6), value: r.range(0, 2000), sub2: -1, value2: -1 },
            };
            Some(if r.below(3) == 0 {
                Command::DiplomacyAnswer { to, terms, counter: r.below(4) == 0, reply: r.below(2) == 0, yes: r.below(2) == 0 }
            } else {
                Command::DiplomacyMessage { to, declaration: r.below(6) == 0, terms }
            })
        }
        // Spies.
        12 => {
            let spies = st.spies();
            let roster = &spies.roster[e as usize];
            let home: Vec<u8> = (1..roster.len()).filter(|&k| roster[k].state == 1 && roster[k].applicant == 0).map(|k| k as u8).collect();
            let away: Vec<u8> = (1..roster.len()).filter(|&k| roster[k].state == 3).map(|k| k as u8).collect();
            let missions: Vec<u16> = (1..=13u16).filter(|&m| spies.enabled.get(m as usize).is_some_and(|&x| x != 0)).collect();
            let order = match (r.below(4), r.pick(&home), r.pick(&away)) {
                (0, _, Some(spy)) => SpyOrder::Recall { spy },
                (1, Some(spy), _) => SpyOrder::Skill { spy, skill: r.range(1, 4) as u8 },
                (_, Some(spy), _) => SpyOrder::Send { spy, mission: r.pick(&missions).unwrap_or(1), target: r.pick(&empires).unwrap_or(1) as u8, param1: 0, param2: 0, days: r.range(1, 30) },
                _ => SpyOrder::Estimate { spy: r.range(1, 12) as u8, mission: r.range(1, 13) as u16, target: r.range(1, 8) as u8 },
            };
            Some(Command::Spy { order })
        }
        // The messages screen.
        13 => Some(Command::from(match r.below(5) {
            0 => MessageOrder::Enter,
            1 => MessageOrder::Auto,
            2 => MessageOrder::Open { row: r.range(0, 3) },
            3 => MessageOrder::Scroll { pointer: r.range(0, 10), row: r.range(-1, 3) },
            _ => MessageOrder::TypedOut,
        })),
        // Renames.
        14 => {
            let is_fleet = r.below(2) == 0;
            let index = if is_fleet { fleet(r) } else { planet(r) }.max(0) as u32;
            Some(Command::Rename { fleet: is_fleet, index, name: format!("Soak{}", r.below(1000)).into_bytes(), custom: r.below(2) == 0 })
        }
        // The colony screen watched.
        15 => Some(Command::from(ColonyOrder::Watch { planet: planet(r) })),
        _ => None,
    }
}

/// The battles `p` takes part in: its space units attack (now and then one retreats), a finished battle's screen is
/// left; its ground units attack the first live enemy or retreat after a while.
fn fight(p: &mut Peer, r: &mut Rng, seen: &mut Seen, ground_turns: &mut u32) {
    for c in battle_orders(p, r, seen, ground_turns) {
        p.issue(c);
    }
}

fn battle_orders(p: &Peer, r: &mut Rng, seen: &mut Seen, ground_turns: &mut u32) -> Vec<Command> {
    let mut out = Vec::new();
    let me = p.empire as i32;
    let m = &p.game.state.mem;
    if m.i32(SPACE_RUNNING) != 0 {
        let b = p.game.realtime.current;
        let Some(battle) = p.game.realtime.slots.get(b) else { return out };
        let left = m.i32(SPACE_LEFT + 4 * me as u32) != 0;
        if !left && (battle.ended || !battle.active || r.below(200) == 0) {
            out.push(Command::SpaceBattleLeft);
        } else if !left && battle.active && r.below(3) == 0 {
            let mine: Vec<usize> = (0..battle.units.len()).filter(|&u| battle.units[u].alive && battle.units[u].owner == me).collect();
            let theirs: Vec<usize> = (0..battle.units.len()).filter(|&u| battle.units[u].alive && battle.units[u].owner > 0 && battle.units[u].owner != me).collect();
            let order = match (r.below(40), r.pick(&mine), r.pick(&theirs)) {
                (0, _, _) => BattleOrder::Retreat,
                (1, Some(u), _) => BattleOrder::Move { unit: u as u16, x: (r.range(-200, 200) as f32).to_bits(), y: 0f32.to_bits(), z: (r.range(-200, 200) as f32).to_bits() },
                (2, _, _) => BattleOrder::ArmAll,
                (_, Some(u), Some(t)) => BattleOrder::Attack { unit: u as u16, target: t as u16 },
                _ => return out,
            };
            out.push(Command::SpaceOrder { battle: b as u8, order });
            seen.orders[16] += 1;
        }
    }
    if m.i32(GROUND_RUNNING) != 0 {
        let q = m.i32(GROUND_PLANET) as usize;
        let Some(f) = p.game.state.ground_field(q) else { return out };
        *ground_turns += 1;
        if r.below(10) != 0 {
            return out;
        }
        let live = |i: usize| f.alive[i] && f.units[i].owner > 0;
        let mine: Vec<usize> = (0..f.units.len()).filter(|&i| live(i) && f.units[i].owner == me).collect();
        if mine.is_empty() {
            return out;
        }
        let order = if *ground_turns > 400 && r.below(4) == 0 {
            GroundOrder::Retreat
        } else {
            let Some(t) = (0..f.units.len()).find(|&i| live(i) && f.units[i].owner != me) else { return out };
            GroundOrder::Target { unit: r.pick(&mine).unwrap() as i32, target: t as i32 }
        };
        out.push(Command::from(ColonyOrder::Ground { planet: q as i32, order }));
        seen.orders[17] += 1;
    }
    out
}

/// One peer's turn: its battles, and an order of a random kind now and then.
fn act(p: &mut Peer, r: &mut Rng, seen: &mut Seen, ground_turns: &mut u32, last: &mut u64) {
    for e in p.events.drain(..) {
        use ig2_sim::game::{GameEvent, RealTimeEvent};
        let name = format!("{e:?}");
        let name: String = name.split([',', '{', ' ']).take(2).collect::<Vec<_>>().join(" ");
        *seen.events.entry(name).or_default() += 1;
        match e {
            GameEvent::RealTime(RealTimeEvent::SpaceStarted { .. }) => seen.space_battles += 1,
            GameEvent::RealTime(RealTimeEvent::GroundStarted { .. }) => {
                seen.ground_wars += 1;
                *ground_turns = 0;
            }
            _ => {}
        }
    }
    if p.turn == *last {
        return;
    }
    *last = p.turn;
    fight(p, r, seen, ground_turns);
    if r.below(3) != 0 {
        return;
    }
    // Kind 1 (attacks) twice as often as the others.
    let k = match r.below(17) {
        16 => 1,
        k => k,
    };
    if let Some(c) = order(p, k, r) {
        seen.orders[k] += 1;
        p.issue(c);
    }
}

/// The even-wars set-up, the same on every machine before the first turn: the exe's debug switch 0xB7723C (lopsided
/// ground wars are fought too), `defender`'s home planet's space guns off, and `attacker`'s first fleet (it carries
/// ground units) beside it with an attack order, so a human-vs-human ground war is fought in real time early on.
fn invade(state: &mut ig2_sim::save::SaveState, attacker: u8, defender: u8) {
    use ig2_sim::save::{va, PLANET_SIZE};
    state.mem.set_i32(0xB7723C, 1);
    let Some(home) = (1..=state.planet_count()).find(|&p| state.mem.u8(va::PLANETS + (p * PLANET_SIZE) as u32 + 0x20) == defender) else { return };
    let at = va::PLANETS + (home * PLANET_SIZE) as u32;
    state.mem.set_i32(at + 0x114, 0);
    for b in state.buildings[home].iter_mut().skip(1) {
        if matches!(b[0], 0x1c..=0x1f | 0x24..=0x26) {
            b[0x19] &= !1;
        }
    }
    let (hx, hy) = (f32::from_le_bytes(state.mem.read(at + 0x18, 4).try_into().unwrap()), f32::from_le_bytes(state.mem.read(at + 0x1c, 4).try_into().unwrap()));
    let mut map = state.starmap();
    let Some(f) = (1..map.fleets.len()).find(|&f| map.fleets[f].alive() && map.fleets[f].owner == attacker) else { return };
    let fl = &mut map.fleets[f];
    (fl.x, fl.y) = (hx + 1.0, hy);
    (fl.order, fl.kind, fl.tx, fl.ty) = (ig2_sim::fleet::order::ATTACK, ig2_sim::fleet::order::PLANET, home as f32, 0.0);
    state.store_starmap(&map);
}

// ------------------------------------------------------------------------------------------------ the session

/// Every turn `a` and `b` both ran has the same hash; returns how many.
fn agree(a: &Peer, b: &Peer) -> usize {
    let mut n = 0;
    for (t, h) in &b.hashes {
        if let Some((_, h0)) = a.hashes.iter().find(|(t0, _)| t0 == t) {
            assert_eq!(h, h0, "turn {t}: empire {} vs empire {}", b.empire, a.empire);
            n += 1;
        }
    }
    n
}

fn started<'r>(mut c: LobbyClient, data: &ig2_sim::game::new_game::StartData, rules: &'r GameRules) -> ClientSession<'r> {
    let t = std::time::Instant::now();
    while c.status != Status::Started {
        c.pump().unwrap();
        assert!(t.elapsed() < Duration::from_secs(5), "no start");
    }
    let (s, link) = c.into_game().unwrap();
    ClientSession::new_galaxy(&s, link, data, rules).unwrap()
}

/// A soak of `humans` players and `ais` computers over `turns` lockstep turns, choices from `seed`; `even_wars`: the
/// exe's debug switch 0xB7723C on every machine, so lopsided ground wars are fought in real time too.
fn soak(humans: usize, ais: u8, turns: u64, seed: u64, even_wars: bool) {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let rules = common::game_rules(&inst);
    common::with_start_data(&inst, |data| {
        let settings = Settings { name: "Soak".into(), scenario: "galaxy".into(), private: false, password: None };
        let mut h = LobbyHost::new_galaxy(settings, seed | 1, RULES, "Host");
        assert!(h.set_options(GalaxyOptions { planets: 20, ais, battle_status: 0, ..GalaxyOptions::default() }));
        let mut joiners: Vec<LobbyClient> = (1..humans)
            .map(|k| {
                let (a, b) = loopback();
                h.admit(Box::new(b));
                LobbyClient::join(Box::new(a), &format!("P{k}"), RULES, None, None)
            })
            .collect();
        let t = std::time::Instant::now();
        while h.state().members.len() < humans || joiners.iter().any(|c| c.state.as_ref() != Some(h.state())) {
            h.pump().unwrap();
            for c in &mut joiners {
                c.pump().unwrap();
            }
            assert!(t.elapsed() < Duration::from_secs(5), "the room didn't fill");
        }
        for c in &mut joiners {
            c.ready(true);
        }
        while !h.state().members.iter().skip(1).all(|m| m.ready) {
            h.pump().unwrap();
            for c in &mut joiners {
                c.pump().unwrap();
            }
        }
        assert!(h.set_ready(0, true));
        let mut hs = HostSession::start_galaxy(h, data, &rules, SessionSettings::default(), seed as u32).ok().expect("the room starts");
        let mut clients: Vec<ClientSession> = joiners.into_iter().map(|c| started(c, data, &rules)).collect();
        if even_wars {
            let (a, d) = (hs.own.empire, clients[0].peer.empire);
            invade(&mut hs.own.game.state, a, d);
            for c in &mut clients {
                invade(&mut c.peer.game.state, a, d);
            }
        }

        let mut seen = Seen::default();
        let n = humans;
        let mut rngs: Vec<Rng> = (0..n).map(|k| Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15).wrapping_add(k as u64 + 1) | 1)).collect();
        let mut ground = vec![0u32; n];
        let mut last = vec![u64::MAX; n];
        let mut now = Duration::ZERO;
        // An MP save at a third, a drop and rejoin of the last joiner at a half.
        let (save_at, drop_at) = (turns / 3, turns / 2);
        let mut saved = false;
        let mut dropped: Option<(u8, u64)> = None;
        let wall = std::time::Instant::now();
        // `IG2_SOAK_STOP=T`: every peer stops at turn T and the states are compared part by part (a desync's turn).
        let stop = env("IG2_SOAK_STOP", u64::MAX);
        let cap = |turn: u64, max: usize| if turn >= stop { 0 } else { max.min((stop - turn) as usize) };
        while hs.own.turn < turns || clients.iter().any(|c| c.peer.turn < turns) {
            now += Duration::from_millis(10);
            for e in hs.pump(now, cap(hs.own.turn, 1)).unwrap() {
                if let SessionEvent::Rejoined { .. } = e {
                    seen.rejoins += 1;
                }
            }
            act(&mut hs.own, &mut rngs[0], &mut seen, &mut ground[0], &mut last[0]);
            for (k, c) in clients.iter_mut().enumerate() {
                c.pump(now, cap(c.peer.turn, 2)).unwrap();
                act(&mut c.peer, &mut rngs[k + 1], &mut seen, &mut ground[k + 1], &mut last[k + 1]);
            }
            if !saved && hs.own.turn >= save_at && !hs.own.game.realtime_running() {
                let save = MpSave::read(&hs.save().unwrap(), RULES).unwrap();
                let (g, pending) = save.snapshot.restore(&rules).unwrap();
                assert_eq!(state_hash(&g, &pending), save.snapshot.hash, "the MP save restores its own state");
                saved = true;
                seen.saves += 1;
            }
            if dropped.is_none() && hs.own.turn >= drop_at && !clients.is_empty() && !hs.own.game.realtime_running() {
                let gone = clients.pop().unwrap();
                let empire = gone.peer.empire;
                for c in &clients {
                    agree(&gone.peer, &c.peer);
                }
                agree(&hs.own, &gone.peer);
                drop(gone);
                dropped = Some((empire, hs.own.turn + 20));
            }
            if let Some((empire, at)) = dropped {
                if at != u64::MAX && hs.own.turn >= at {
                    let (mut ours, theirs) = loopback();
                    hs.rejoin(empire, Box::new(theirs), now).unwrap();
                    let start = ClientSession::wait_start(&mut ours).unwrap().expect("a start");
                    clients.push(ClientSession::new(&start, Box::new(ours), &rules).unwrap());
                    last[n - 1] = u64::MAX;
                    dropped = Some((empire, u64::MAX));
                }
            }
            if hs.own.turn >= stop && clients.iter().all(|c| c.peer.turn >= stop) {
                for c in &clients {
                    diff(&hs.own, &c.peer, &rules);
                }
                for b in hs.host.log.iter().filter(|b| b.turn + 6 >= stop) {
                    eprintln!("bundle {}: {:?}", b.turn, b.commands);
                }
                panic!("stopped at turn {stop}");
            }
            assert!(hs.host.desyncs.is_empty(), "desync at {:?}; seen {seen:?}", hs.host.desyncs);
            assert!(now < Duration::from_secs(turns * 4 + 600), "stalled at turn {}; seen {seen:?}", hs.own.turn);
        }
        let mut agreed = 0;
        for c in &clients {
            agreed += agree(&hs.own, &c.peer);
            for d in &clients {
                agree(&c.peer, &d.peer);
            }
        }
        let events = std::mem::take(&mut seen.events);
        eprintln!("soak {humans} humans + {ais} AIs, {turns} turns, seed {seed}: {agreed} turn hashes agreed, {:.0?}; {seen:?}", wall.elapsed());
        if std::env::var("SOAK_EVENTS").is_ok() {
            eprintln!("events: {events:?}");
        }
        assert!(agreed as u64 >= (turns - 40) * (n as u64 - 1) - turns / 2, "too few turns compared");
        assert!(seen.saves == 1 && seen.rejoins >= 1, "{seen:?}");
    });
}

/// Where `a`'s and `b`'s states differ: the parts, and the first differing memory bytes.
fn diff(a: &Peer, b: &Peer, rules: &GameRules) {
    let (ga, pa) = a.snapshot().restore(rules).unwrap();
    let (gb, pb) = b.snapshot().restore(rules).unwrap();
    let (ha, hb) = (ig2_net::part_hashes(&ga, &pa), ig2_net::part_hashes(&gb, &pb));
    let parts: Vec<&str> = ha.iter().zip(&hb).filter(|(x, y)| x.1 != y.1).map(|(x, _)| x.0).collect();
    eprintln!("turn {} empire {} vs {}: live hashes {:x} {:x}, parts differ {parts:?}", a.turn, a.empire, b.empire, a.hash(), b.hash());
    let (ma, mb) = (ga.state.mem.runs(), gb.state.mem.runs());
    let mut n = 0;
    for (at, x) in ma {
        let Some(y) = mb.get(at) else {
            eprintln!("  run {at:#x} only on empire {}", a.empire);
            continue;
        };
        for i in 0..x.len().max(y.len()) {
            if x.get(i) != y.get(i) && n < 60 {
                eprintln!("  {:#x}: {:?} vs {:?}", at + i as u32, x.get(i), y.get(i));
                n += 1;
            }
        }
    }
    if parts.contains(&"realtime") {
        eprintln!("  realtime: {:?}
  vs        {:?}", ga.realtime.current, gb.realtime.current);
    }
    if parts.contains(&"pending") {
        eprintln!("  pending: {pa:?}
  vs       {pb:?}");
    }
}

fn env(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[test]
fn two_humans_and_two_ais_stay_in_step() {
    soak(2, 2, env("IG2_SOAK_TURNS", 3000), env("IG2_SOAK_SEED", 1), true);
}

#[test]
fn four_humans_stay_in_step() {
    soak(4, 1, env("IG2_SOAK_TURNS", 1500), env("IG2_SOAK_SEED", 2), false);
}

/// The run that found the battlefield hand-offs skipped after the peer emptied the event list (turn 5576, the host's
/// garrison of planet 13 missing on one machine).
#[test]
fn the_seed_one_game_stays_in_step() {
    soak(2, 2, 6000, 1, false);
}

#[test]
#[ignore = "long: run with --ignored"]
fn a_long_soak() {
    for seed in 0..env("IG2_SOAK_SEEDS", 4) {
        soak(2 + (seed % 3) as usize, 2, env("IG2_SOAK_TURNS", 6000), 1000 + seed, seed % 2 == 0);
    }
}
