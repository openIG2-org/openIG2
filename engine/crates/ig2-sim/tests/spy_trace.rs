//! Replays espionage recorded from the running GOG `ig2.exe` (tools/trace/run_spy.py, `re/rules/espionage.md`):
//! every `FRISSIT_Spy` (exe 0x584b30) call of `spy-*.jsonl`, plain and fuzzed. A trace holds byte regions: a
//! full dump, then per call the changes made before it (other systems, the fuzzer) and by it. The replay
//! rebuilds the spies, the colony world (planets, buildings, money, techs, contacts), the ships and fleets and
//! the relations, runs the port and compares the result with the recorded "after" state: the spy tables,
//! relations, contacts, techs, kidnappings, stream states, ships and vehicles byte for byte, the planets,
//! money and fleets as parsed records.
//! Traces hold game state, so they stay in `_local/` (never committed). The rule tables come from the
//! user's exe. The test skips unless `IG2_INSTALL_DIR` and the traces exist.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::colony::{Building, BuildingKind, ColonyRules, EconomyContext, Planet};
use ig2_sim::colony_side::{ColonyWorld, SideRules, TechTable, Treasury, EMPIRES, PLANET_SLOTS};
use ig2_sim::diplomacy::Relation;
use ig2_sim::research::{Research, ResearchContext, ResearchRules};
use ig2_sim::rng::Streams;
use ig2_sim::spacewar::{CombatFleet, CombatWorld, Design, FleetTotals, Ship, Vehicle};
use ig2_sim::spy::{Held, MissionRule, Order, Spies, Spy, SpyContext, SpyEvent, SpyRules, StoryCheck, World, RECORD, ROSTERS, SLOTS};
use serde_json::Value;

fn traces(prefix: &str) -> Vec<PathBuf> {
    let dir = std::env::var_os("IG2_TRACE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with(prefix) && f.ends_with(".jsonl")));
    v.sort();
    v
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn f64_at(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
fn put(b: &mut [u8], o: usize, v: &[u8]) {
    b[o..o + v.len()].copy_from_slice(v);
}

type Regions = BTreeMap<String, Vec<u8>>;

fn apply(state: &mut Regions, diff: &Value) {
    for (name, d) in diff.as_object().unwrap() {
        let r = state.entry(name.clone()).or_default();
        r.resize(d["len"].as_u64().unwrap() as usize, 0);
        for run in d["runs"].as_array().unwrap() {
            let (o, bytes) = (run[0].as_u64().unwrap() as usize, unhex(run[1].as_str().unwrap()));
            put(r, o, &bytes);
        }
    }
}

fn install() -> Option<Install> {
    Install::from_env(Lang::En).map(|i| i.expect("valid install"))
}

fn i32s(b: &[u8]) -> [i32; EMPIRES] {
    std::array::from_fn(|e| i32_at(b, 4 * e))
}

fn spy_rules(r: &ig2_rules::Rules) -> SpyRules {
    SpyRules {
        missions: r
            .spy_missions
            .iter()
            .map(|m| MissionRule {
                kind: m.kind,
                xp: m.xp,
                skill_floor: m.skill_floor,
                skill_full: m.skill_full,
                chance: m.chance,
                needs_target: m.needs_target != 0,
                story: m.story != 0,
                no_countdown: m.no_countdown != 0,
                days: m.days,
                cost: m.cost,
                min_rank: m.min_rank,
            })
            .collect(),
        outcome_state: r.spy_outcome_state.clone(),
        rank_points: r.spy_rank_points.clone(),
        templates: r.spy_templates.iter().map(|t| Spy::from_bytes(t)).collect(),
        rank_xp: r.spy_rank_xp.clone(),
        training_xp: r.spy_training_xp.clone(),
        names: r.spy_names.clone(),
        building_tech: r.buildings.iter().map(|b| b.tech).collect(),
        hire_cost_mult: r.spy_hire_cost_mult.clone(),
    }
}

fn spies(s: &Regions) -> Spies {
    let k = &s["kemek"];
    let mut sp = Spies::default();
    for e in 0..ROSTERS {
        for j in 0..SLOTS {
            let o = e * SLOTS * RECORD + j * RECORD;
            sp.roster[e][j] = Spy::from_bytes(&k[o..o + RECORD]);
        }
        sp.count[e] = i32_at(&s["ossz"], 4 * e);
        sp.working[e] = i32_at(&s["van"], 4 * e);
    }
    sp.enabled = s["kuld"].chunks(0x20).map(|m| m[0]).collect();
    sp
}

fn fleet_of(r: &[u8]) -> CombatFleet {
    CombatFleet {
        owner: r[0x15],
        flags: u16::from_le_bytes([r[0x52], r[0x53]]),
        x: f32_at(r, 0x30),
        y: f32_at(r, 0x34),
        totals: FleetTotals::from_record(r),
        colony_ships: i16_at(r, 0x4c),
        terraformers: i16_at(r, 0x4e),
    }
}

fn combat(s: &Regions, human: [bool; EMPIRES]) -> CombatWorld {
    let fl = &s["fleets"];
    let mut w = CombatWorld {
        fleets: fl.chunks(0x80).map(fleet_of).collect(),
        ships: s["ships"].chunks(6).map(|c| Ship { fleet: i16_at(c, 0), design: i16_at(c, 2) }).collect(),
        vehicles: s["tanks"].chunks(8).map(|c| Vehicle { fleet: i16_at(c, 0), design: i16_at(c, 2), count: i32_at(c, 4) }).collect(),
        designs: vec![Vec::new(); EMPIRES],
        human_controlled: human,
        colony_ship_pool: i32s(&s["cpool"]),
        terraformer_pool: i32s(&s["tpool"]),
        ..Default::default()
    };
    for e in 1..=8 {
        if let Some(d) = s.get(&format!("des{e}")) {
            w.designs[e] = d.chunks(0x84).filter(|c| c.len() == 0x84).map(Design::from_bytes).collect();
        }
    }
    for (f, r) in fl.chunks(0x80).enumerate() {
        if i16_at(r, 0x40) > 0 && i16_at(r, 0x42) == 2 {
            w.follow_targets.push((f, f32_at(r, 0x44)));
        }
    }
    w
}

#[derive(Default)]
struct Stats {
    calls: usize,
    events: BTreeMap<String, usize>,
    missions: BTreeMap<(u16, bool), usize>,
    outcomes: BTreeMap<(u8, Option<u8>), usize>,
    exe_calls: BTreeMap<String, usize>,
    orders: BTreeMap<String, usize>,
}

/// Write the port's state back into the raw regions.
#[allow(clippy::too_many_arguments)]
fn write_back(s: &mut Regions, sp: &Spies, relations: &[[Relation; EMPIRES]], colony: &ColonyWorld, kidnap: &[i32; 18], rng: &Streams, cw: &CombatWorld, research: &Research) {
    let k = s.get_mut("kemek").unwrap();
    for e in 0..ROSTERS {
        for j in 0..SLOTS {
            put(k, e * SLOTS * RECORD + j * RECORD, &sp.roster[e][j].to_bytes());
        }
    }
    for (name, xs) in [("ossz", sp.count), ("van", sp.working)] {
        let r = s.get_mut(name).unwrap();
        for (e, x) in xs.iter().enumerate() {
            put(r, 4 * e, &x.to_le_bytes());
        }
    }
    let r = s.get_mut("kuld").unwrap();
    for (m, &x) in sp.enabled.iter().enumerate() {
        r[m * 0x20] = x;
    }
    let r = s.get_mut("visz").unwrap();
    for (a, row) in relations.iter().enumerate() {
        for (b, rel) in row.iter().enumerate() {
            put(r, a * 0x2d2 + b * Relation::SIZE, &rel.to_bytes());
        }
    }
    let r = s.get_mut("ismeri").unwrap();
    for a in 0..EMPIRES {
        for b in 0..EMPIRES {
            r[a * EMPIRES + b] = colony.knows[a][b];
        }
    }
    s.insert("techs".into(), colony.tech.bytes.clone());
    let r = s.get_mut("kidnap").unwrap();
    for (i, x) in kidnap.iter().enumerate() {
        put(r, 4 * i, &x.to_le_bytes());
    }
    let r = s.get_mut("seeds").unwrap();
    for i in 0..21 {
        put(r, 4 * i, &rng.state(i).to_le_bytes());
    }
    let r = s.get_mut("ships").unwrap();
    for (i, x) in cw.ships.iter().enumerate() {
        put(r, 6 * i, &x.fleet.to_le_bytes());
        put(r, 6 * i + 2, &x.design.to_le_bytes());
    }
    let r = s.get_mut("tanks").unwrap();
    for (i, t) in cw.vehicles.iter().enumerate() {
        put(r, 8 * i, &t.fleet.to_le_bytes());
        put(r, 8 * i + 2, &t.design.to_le_bytes());
        put(r, 8 * i + 4, &t.count.to_le_bytes());
    }
    for (name, xs) in [("cpool", cw.colony_ship_pool), ("tpool", cw.terraformer_pool), ("kutat", research.current)] {
        let r = s.get_mut(name).unwrap();
        for (e, x) in xs.iter().enumerate() {
            put(r, 4 * e, &x.to_le_bytes());
        }
    }
    let r = s.get_mut("hanyep").unwrap();
    for (p, x) in colony.occupied.iter().enumerate().take(r.len() / 4) {
        put(r, 4 * p, &x.to_le_bytes());
    }
}

const COMPARED: &[&str] =
    &["kemek", "ossz", "van", "kuld", "visz", "ismeri", "techs", "kidnap", "seeds", "ships", "tanks", "cpool", "tpool", "kutat", "hanyep"];

#[test]
fn spy_matches_every_traced_call() {
    let paths = traces("spy-");
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    if paths.is_empty() {
        eprintln!("no espionage trace in _local/traces; run tools/trace/run_spy.py");
        return;
    }
    let r = ig2_rules::Rules::from_install(&inst).expect("catalogued exe");
    let rules = spy_rules(&r);
    let hr = human_rules(&r);
    let mut st = Stats::default();
    let mut bad = Vec::new();
    for p in paths {
        let mut state = Regions::new();
        for line in std::fs::read_to_string(&p).unwrap().lines() {
            let ev: Value = serde_json::from_str(line).unwrap();
            let k = ev["k"].as_str().unwrap();
            if k == "init" {
                for (n, h) in ev["regions"].as_object().unwrap() {
                    state.insert(n.clone(), unhex(h.as_str().unwrap()));
                }
                continue;
            }
            if k != "spy" && k != "order" {
                continue;
            }
            apply(&mut state, &ev["pre"]);
            let before = state.clone();
            let mut after = state.clone();
            apply(&mut after, &ev["post"]);
            let what = format!("{} seq {}", p.file_name().unwrap().to_string_lossy(), ev["seq"]);
            let g = &ev["g"];
            let gi = |k: &str| g[k].as_i64().unwrap_or(0);
            let ki = unhex(g["ki"].as_str().unwrap());
            let faj = unhex(g["faj"].as_str().unwrap());
            let human: [bool; EMPIRES] = std::array::from_fn(|e| ki[e] == 1);
            let race: [u8; EMPIRES] = faj[..EMPIRES].try_into().unwrap();
            let alive: [bool; EMPIRES] = std::array::from_fn(|e| before["el"][e] != 0);
            let ctx = SpyContext {
                alive,
                human,
                race,
                human_empire: gi("en") as usize,
                program_players: gi("pp") as i32,
                multiplayer: gi("mp") != 0,
                difficulty: gi("diff") as i32,
                debug: gi("debug") != 0,
                tutorial: gi("tut") != 0,
                no_recruits: gi("norecruit") != 0,
                forced: gi("forced") as i32,
                day: gi("nap") as i32,
                senders: None,
                listed: [false; EMPIRES],
            };
            let mut sp = spies(&before);
            let mut colony = colony_world(&before, g);
            colony.tech = TechTable { bytes: before["techs"].clone() };
            colony.alive = alive;
            for a in 0..EMPIRES {
                for b in 0..EMPIRES {
                    colony.knows[a][b] = before["ismeri"][a * EMPIRES + b];
                }
            }
            let econ = EconomyContext {
                multiplayer: ctx.multiplayer,
                human_empire: ctx.human_empire as u8,
                human_controlled: human,
                empire_race: race,
                players: gi("pp2") as i32,
                ..Default::default()
            };
            let mut cw = combat(&before, human);
            let mut research = Research { current: i32s(&before["kutat"]), ..Default::default() };
            let rctx = ResearchContext {
                human_empire: ctx.human_empire as u8,
                human_controlled: human,
                race,
                players: gi("pp2") as i32,
                all_techs: gi("alltechs") != 0,
                ..Default::default()
            };
            let v = &before["visz"];
            let mut relations: Vec<[Relation; EMPIRES]> =
                (0..EMPIRES).map(|a| std::array::from_fn(|b| Relation::from_bytes(&v[a * 0x2d2 + b * Relation::SIZE..]))).collect();
            let mut kidnap: [i32; 18] = std::array::from_fn(|i| i32_at(&before["kidnap"], 4 * i));
            let mut rng = Streams::new(21);
            for i in 0..21 {
                rng.set_state(i, i32_at(&before["seeds"], 4 * i) as u32);
            }
            let mut story_asked = 0;
            let mut story = |_: StoryCheck, _: Held| {
                story_asked += 1;
                false
            };
            let events = {
                let mut w = World {
                    ctx: &ctx,
                    colony: &mut colony,
                    side_rules: &hr.side,
                    econ: &econ,
                    combat: &mut cw,
                    research: &mut research,
                    research_rules: &hr.research,
                    research_ctx: &rctx,
                    relations: &mut relations,
                    kidnap: &mut kidnap,
                    rng: &mut rng,
                    rules: &rules,
                    story: &mut story,
                    events: Vec::new(),
                };
                match ev["order"].as_array() {
                    Some(o) => {
                        let (e, order) = order_of(o);
                        *st.orders.entry(format!("{order:?}").split([' ', '{']).next().unwrap().to_string()).or_default() += 1;
                        sp.order(&mut w, e, order, &r.spy_training_cost, &r.spy_race_cost_mult);
                    }
                    None => sp.daily(&mut w),
                }
                w.events
            };
            st.calls += 1;
            if story_asked > 0 {
                bad.push(format!("{what}: a story mission ran (not traced)"));
            }
            for c in ev["calls"].as_array().unwrap() {
                *st.exe_calls.entry(c[0].as_str().unwrap().to_string()).or_default() += 1;
            }
            for e in &events {
                let name = format!("{e:?}");
                *st.events.entry(name.split([' ', '(']).next().unwrap().to_string()).or_default() += 1;
                match e {
                    SpyEvent::MissionDone { mission, ok, .. } => *st.missions.entry((*mission, *ok)).or_default() += 1,
                    SpyEvent::Caught { outcome, fate, .. } => *st.outcomes.entry((*outcome, *fate)).or_default() += 1,
                    _ => {}
                }
            }

            let mut ours = before.clone();
            write_back(&mut ours, &sp, &relations, &colony, &kidnap, &rng, &cw, &research);
            for &n in COMPARED {
                let (Some(a), Some(b)) = (ours.get(n), after.get(n)) else { continue };
                if a != b {
                    let i = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)).unwrap();
                    bad.push(format!("{what}: {n} len {}/{} differs at {i:#x}: ours {:?} exe {:?}", a.len(), b.len(), a.get(i), b.get(i)));
                }
            }
            let theirs = colony_world(&after, g);
            for p in 1..colony.planets.len() {
                if colony.planets[p] != theirs.planets[p] {
                    bad.push(format!("{what}: planet {p}: ours {:?}\n exe {:?}", colony.planets[p], theirs.planets[p]));
                }
            }
            if colony.money != theirs.money {
                let e = (0..EMPIRES).find(|&e| colony.money[e] != theirs.money[e]).unwrap();
                bad.push(format!("{what}: money of {e}: ours {:?} exe {:?}", colony.money[e], theirs.money[e]));
            }
            for (f, r) in after["fleets"].chunks(0x80).enumerate() {
                let want = fleet_of(r);
                let Some(got) = cw.fleets.get(f) else { continue };
                if got.owner != want.owner || (want.owner != 0xff && got.totals != want.totals) {
                    bad.push(format!("{what}: fleet {f}: ours {:?} exe {:?}", (got.owner, got.totals), (want.owner, want.totals)));
                }
            }
            // Continue from the recorded state either way.
            state = after;
        }
    }
    eprintln!("{} calls; events {:?}", st.calls, st.events);
    eprintln!("staged MULTIGET_Spy orders: {:?}", st.orders);
    eprintln!("missions (mission, ok): {:?}", st.missions);
    eprintln!("catch outcomes (outcome, fate): {:?}", st.outcomes);
    eprintln!("exe callees: {:?}", st.exe_calls);
    if !bad.is_empty() {
        for b in bad.iter().take(30) {
            eprintln!("{b}");
        }
        panic!("{} mismatches", bad.len());
    }
}

/// A staged `MULTIGET_Spy` message `[type, sender, …]` (`espionage.md` §8) as the sender and its order.
fn order_of(o: &[Value]) -> (usize, Order) {
    let v = |i: usize| o[i].as_i64().unwrap();
    let order = match v(0) {
        0x47 => Order::Dismiss { spy: v(2) as u8 },
        0x48 => Order::Recall { spy: v(2) as u8 },
        0x49 => Order::Send { spy: v(2) as u8, mission: v(3) as u16, target: v(4) as u8, param1: v(5) as i16, param2: v(6) as i16, days: v(7) as i32 },
        0x4a => Order::Verdict { fate: v(2) as i32, owner: v(3) as u8, spy: v(4) as u8 },
        0x4b => Order::Applicant { choice: v(2) as i32, spy: v(3) as u8 },
        0x4c => Order::Skill { spy: v(2) as u8, skill: v(3) as u8 },
        t => panic!("order type {t:#x}"),
    };
    (v(1) as usize, order)
}

fn colony_planet(r: &[u8], bld: &[u8], unrest: i32) -> Planet {
    let mut p = Planet {
        x: f32_at(r, 0x18),
        y: f32_at(r, 0x1c),
        owner: r[0x20],
        race: r[0x21],
        surface: r[0x22],
        colony_map: r[0x23],
        richness: i32_at(r, 0x24),
        population: i32_at(r, 0x28),
        population_change: i32_at(r, 0x2c),
        morale: i32_at(r, 0x30),
        tax_level: r[0x34],
        gdp_acc: i32_at(r, 0x38),
        gdp_last: i32_at(r, 0x3c),
        tax_acc: i32_at(r, 0x40),
        tax_last: i32_at(r, 0x44),
        energy_produced: i32_at(r, 0x48),
        energy_demand: i32_at(r, 0x4c),
        workers_demand: i32_at(r, 0x50),
        field_54: u16::from_le_bytes([r[0x54], r[0x55]]),
        field_58: i32_at(r, 0x58),
        field_5c: i32_at(r, 0x5c),
        protected: r[0x64] != 0,
        autobuild_counter: i32_at(r, 0x68),
        autobuild_mode: r[0x6c],
        autobuild_aggr: r[0x6d],
        ship_capacity: i32_at(r, 0x70),
        vehicle_capacity: i32_at(r, 0x74),
        production: r[0x78..0xb8].try_into().unwrap(),
        epidemic: i16_at(r, 0xb8),
        field_114: i32_at(r, 0x114),
        morale_penalty: i32_at(r, 0x174),
        morale_penalty_rate: i32_at(r, 0x178),
        loyal: r[0x186] != 0,
        ai_proof: r[0x187] != 0,
        unrest,
        ..Default::default()
    };
    p.spies.copy_from_slice(&r[0xc4..0xc4 + 19]);
    for e in 0..19 {
        p.visibility[e] = i16_at(r, 0xd8 + 2 * e);
    }
    for k in 1..p.problems.len() {
        p.problems[k] = i32_at(r, 0x118 + 4 * k);
    }
    for k in 0..5 {
        p.events[k] = i16_at(r, 0x16a + 2 * k);
    }
    p.buildings = bld
        .chunks(32)
        .map(|s| Building {
            kind: s[0],
            owner: s[1],
            damage: i32_at(s, 4),
            work_left: u16::from_le_bytes([s[8], s[9]]),
            planet: s[0xa],
            x: f32_at(s, 0xc),
            y: f32_at(s, 0x10),
            yield_pct: s[0x18],
            flags: s[0x19],
            shown_energy: i16_at(s, 0x1a),
            shown_workers: i16_at(s, 0x1c),
            efficiency: i16_at(s, 0x1e),
        })
        .collect();
    p
}

fn colony_world(s: &Regions, g: &Value) -> ColonyWorld {
    let raw = &s["planets"];
    let n = raw.len() / 0x188;
    let per = |k: &str| -> Vec<i32> {
        let mut v = vec![0; PLANET_SLOTS.max(n)];
        for (i, x) in s[k].chunks(4).enumerate().take(v.len()) {
            v[i] = i32::from_le_bytes(x.try_into().unwrap());
        }
        v
    };
    let (nb, lazad) = (per("nb"), per("lazad"));
    let mut planets = vec![Planet::default(); n];
    let mut o = 0;
    for p in 1..n {
        let len = nb[p].max(0) as usize * 0x20;
        planets[p] = colony_planet(&raw[p * 0x188..(p + 1) * 0x188], &s["bld"][o..o + len], lazad[p]);
        o += len;
    }
    let mut w = ColonyWorld {
        planets,
        occupied: per("hanyep"),
        needed_morale: per("moral"),
        last_near_fleet: per("utolso"),
        all_techs: g["alltechs"].as_i64().unwrap_or(0) != 0,
        difficulty: g["diff"].as_i64().unwrap_or(0).max(0) as usize,
        ..Default::default()
    };
    for e in 0..EMPIRES {
        let six = |k: &str| -> [f64; 6] { std::array::from_fn(|i| f64_at(&s[k], e * 48 + 8 * i)) };
        w.money[e] = Treasury { total: f64_at(&s["money"], 8 * e), pools: six("pools"), reserve: six("hatar"), share: six("szorzo") };
    }
    w
}

struct HumanRules {
    side: SideRules,
    research: ResearchRules,
}

fn human_rules(r: &ig2_rules::Rules) -> HumanRules {
    let colony = ColonyRules {
        buildings: r
            .buildings
            .iter()
            .map(|b| BuildingKind { tech: b.tech, priority: b.priority, build_work: b.build_work, max_hp: b.max_hp, ..Default::default() })
            .collect(),
        race_production: r.race_production_mult.clone(),
        ..Default::default()
    };
    HumanRules {
        side: SideRules {
            colony,
            race_research_time: r.race_research_time_mult.clone(),
            difficulty_money: r.difficulty_money_mult.clone(),
            production_default: r.production_default.clone().try_into().unwrap(),
            ..Default::default()
        },
        research: ResearchRules {
            race_time: r.race_research_time_mult.clone(),
            race_money: r.race_research_money_mult.clone(),
            ..Default::default()
        },
    }
}

