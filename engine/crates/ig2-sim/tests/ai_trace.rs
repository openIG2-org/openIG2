//! Replays the AI empire's daily pass recorded from the running GOG `ig2.exe` (tools/trace/run_ai.py,
//! `re/rules/ai-empire.md`): every `FRISSIT_MI` (exe 0x50a6e0) call of `ai-*.jsonl`, plain and fuzzed. A trace
//! holds byte regions: a full dump, then per call the changes before it and a diff at every checkpoint (entering
//! and leaving `FRISSIT_MI_SZAMOLAS`, and per AI empire `…_GYARTAS`, `…_KEMKEDES`, `…_KOLONIZALAS`, `…_FLOTTAK`,
//! `…_DESIGN` (nested in the two before), the war declaration `ElkuldHaduzenet`, `…_FEJLESZTES`, then the end).
//! Each ported segment runs from the state at one checkpoint, and every recorded region must equal the closing
//! checkpoint's byte for byte:
//! - `szam0 → szam1`: `AiState::bookkeeping`;
//! - previous checkpoint → `gy0`: the previous AI empire's sliders, then `AiState::planet_settings`;
//! - `gy0 → gy1`: `AiState::production`; `kem0 → kem1`: `ai::spy_missions`; `fej0 → fej1`: `ai::research_pick`;
//! - `kol0 → kol1`: `AiState::colonise`; `flo0 → flo1`: `AiState::fleets`; `des0 → des1`: `AiState::design`
//!   (nested segments are also checked as part of the enclosing one);
//! - last checkpoint → `end`: the last sliders, then `AiState::income_shares`.
//!
//! The war declaration is diplomacy's (verified by its own traces): the hook here applies the exe's recorded
//! changes to relations, contacts and contact ages (`war0 → war1`) at the point the port declares, and the replay
//! checks that the port declares the same wars. Localised names (the main fleet's, a new design's) and the bytes of
//! records appended to grown arrays (uninitialised memory in the original) are taken from the exe.
//! Traces hold game state, so they stay in `_local/` (never committed; `IG2_TRACE_DIR` points elsewhere). The rule
//! tables come from the user's exe. The test skips unless `IG2_INSTALL_DIR` and the traces exist.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::ai::{self, AiContext, AiEvent, AiHooks, AiRules, AiState, Category, PodRule, World, CATEGORIES, CATEGORY_SIZE, DESIGN_SLOTS, GRID};
use ig2_sim::colony::{Building, BuildingKind, ColonyRules, EconomyContext, Planet};
use ig2_sim::colony_side::{ColonyWorld, SideRules, TechTable, Treasury, EMPIRES, PLANET_SLOTS};
use ig2_sim::diplomacy::Relation;
use ig2_sim::fleet::{Fleet, Starmap, StarmapContext, Waypoint};
use ig2_sim::research::{Research, ResearchContext, ResearchRules};
use ig2_sim::rng::Streams;
use ig2_sim::spacewar::{CombatFleet, CombatWorld, Design, DesignRules, FleetTotals, Hull, Ship, Vehicle};
use ig2_sim::spy::{self, Held, MissionRule, Spies, Spy, SpyContext, SpyRules, StoryCheck, RECORD, ROSTERS, SLOTS};
use serde_json::Value;

/// `_local/traces` of the checkout, or `IG2_TRACE_DIR` (a git worktree without the gitignored traces).
fn traces(prefix: &str) -> Vec<PathBuf> {
    let dir = std::env::var_os("IG2_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
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
    b.get(o..o + 4).map_or(0, |x| i32::from_le_bytes(x.try_into().unwrap()))
}
fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn f32_at(b: &[u8], o: usize) -> f32 {
    b.get(o..o + 4).map_or(0.0, |x| f32::from_le_bytes(x.try_into().unwrap()))
}
fn f64_at(b: &[u8], o: usize) -> f64 {
    f64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}
fn put(b: &mut [u8], o: usize, v: &[u8]) {
    b[o..o + v.len()].copy_from_slice(v);
}
fn i32s(b: &[u8]) -> [i32; EMPIRES] {
    std::array::from_fn(|e| i32_at(b, 4 * e))
}

type Regions = BTreeMap<String, Vec<u8>>;

/// An empty region for traces recorded before it existed.
fn region<'a>(s: &'a Regions, name: &str) -> &'a [u8] {
    s.get(name).map_or(&[], |v| v.as_slice())
}

fn apply_runs(r: &mut Vec<u8>, d: &Value) {
    r.resize(d["len"].as_u64().unwrap() as usize, 0);
    for run in d["runs"].as_array().unwrap() {
        let (o, bytes) = (run[0].as_u64().unwrap() as usize, unhex(run[1].as_str().unwrap()));
        put(r, o, &bytes);
    }
}

fn apply(state: &mut Regions, diff: &Value) {
    for (name, d) in diff.as_object().unwrap() {
        apply_runs(state.entry(name.clone()).or_default(), d);
    }
}

fn install() -> Option<Install> {
    Install::from_env(Lang::En).map(|i| i.expect("valid install"))
}

struct Rules {
    ai: AiRules,
    spy: SpyRules,
    side: SideRules,
    research: ResearchRules,
}

fn rules(r: &ig2_rules::Rules) -> Rules {
    let colony = ColonyRules {
        buildings: r
            .buildings
            .iter()
            .map(|b| BuildingKind { tech: b.tech, priority: b.priority, build_work: b.build_work, max_hp: b.max_hp, ..Default::default() })
            .collect(),
        race_production: r.race_production_mult.clone(),
        ..Default::default()
    };
    let design = DesignRules {
        hulls: r.hulls.iter().map(|h| Hull { class: h.class, flags: h.flags, tech: h.tech }).collect(),
        tech_price: r.techs.iter().map(|t| t.price).collect(),
        tech_time: r.techs.iter().map(|t| t.build_time).collect(),
        tech_hp: r.techs.iter().map(|t| t.hp).collect(),
        tech_capacity: r.techs.iter().map(|t| t.capacity).collect(),
        firepower: r.techs.iter().map(|t| t.firepower).collect(),
        race_pod_table: r.race_pod_table.clone(),
        pod_slot_types: r.pod_slots.iter().map(|t| t.iter().map(|h| h.iter().map(|s| s.pod_type).collect()).collect()).collect(),
        tank_speeds: r.tank_speeds.clone(),
    };
    Rules {
        ai: AiRules {
            build_mode: r.ai_build_mode.clone(),
            build_min: r.ai_build_min.clone(),
            build_max: r.ai_build_max.clone(),
            production_share: r.race_ai_production_share.clone(),
            military_mode: r.race_ai_military_autobuild.clone(),
            training_cost: r.spy_training_cost.clone(),
            mission_cost_mult: r.spy_race_cost_mult.clone(),
            design,
            pod_slot_max: r.pod_slots.iter().map(|t| t.iter().map(|h| h.iter().map(|s| s.max_count).collect()).collect()).collect(),
            pods: r
                .pod_types
                .iter()
                .map(|p| PodRule { id: p.id, techs: std::array::from_fn(|i| p.techs.get(i).copied().unwrap_or(0)) })
                .collect(),
            ground_priority: r.ai_ground_priority.clone(),
        },
        spy: SpyRules {
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
        },
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

// ------------------------------------------------------------------------------------------------ decoding

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
        field_60: i32_at(r, 0x60),
        protected: r[0x64] != 0,
        autobuild_counter: i32_at(r, 0x68),
        autobuild_mode: r[0x6c],
        autobuild_aggr: r[0x6d],
        ship_capacity: i32_at(r, 0x70),
        vehicle_capacity: i32_at(r, 0x74),
        production: r[0x78..0xb8].try_into().unwrap(),
        epidemic: i16_at(r, 0xb8),
        stations: std::array::from_fn(|k| i16_at(r, 0x100 + 2 * k)),
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
        for (i, x) in region(s, k).chunks(4).enumerate().take(v.len()) {
            v[i] = i32::from_le_bytes(x.try_into().unwrap());
        }
        v
    };
    let (nb, lazad) = (per("nb"), per("lazad"));
    let mut planets = vec![Planet::default(); n];
    let mut o = 0;
    for p in 0..n {
        let len = if p == 0 { 0 } else { nb[p].max(0) as usize * 0x20 };
        planets[p] = colony_planet(&raw[p * 0x188..(p + 1) * 0x188], &s["bld"][o..o + len], lazad[p]);
        o += len;
    }
    let mut w = ColonyWorld {
        planets,
        occupied: per("hanyep"),
        all_techs: g["alltechs"].as_i64().unwrap_or(0) != 0,
        difficulty: g["diff"].as_i64().unwrap_or(0).max(0) as usize,
        ..Default::default()
    };
    w.strength.ground = per("foldi");
    w.strength.fortress = per("erod");
    for e in 0..EMPIRES {
        let six = |k: &str| -> [f64; 6] { std::array::from_fn(|i| f64_at(&s[k], e * 48 + 8 * i)) };
        w.money[e] = Treasury { total: f64_at(&s["money"], 8 * e), pools: six("pools"), reserve: six("hatar"), share: six("szorzo") };
        for i in 0..4 {
            w.empires.labs[e][i] = i32_at(&s["labs"], e * 16 + 4 * i);
            w.empires.lab_buildings[e][i] = i32_at(&s["labsall"], e * 16 + 4 * i);
        }
        w.empires.full_gdp[e] = i32_at(&s["fullgdp"], 4 * e);
    }
    for e in 0..9 {
        w.empires.spy_capacity[e] = i32_at(&s["spycap"], 4 * e);
        w.empires.spy_centres[e] = i32_at(&s["spyall"], 4 * e);
        for p in 0..PLANET_SLOTS {
            w.empires.spy_centres_by_planet[e][p] = i32_at(&s["spyplan"], 4 * (e * PLANET_SLOTS + p));
        }
    }
    w.tech = TechTable { bytes: s["techs"].clone() };
    for a in 0..EMPIRES {
        w.alive[a] = s["el"][a] != 0;
        for b in 0..EMPIRES {
            w.knows[a][b] = s["ismeri"][a * EMPIRES + b];
        }
    }
    w
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
    }
    sp.enabled = s["kuld"].chunks(0x20).map(|m| m[0]).collect();
    sp
}

fn combat_fleet(r: &[u8]) -> CombatFleet {
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
    let mut w = CombatWorld {
        fleets: s["fleets"].chunks(0x80).map(combat_fleet).collect(),
        ships: s["ships"].chunks(6).map(|c| Ship { fleet: i16_at(c, 0), design: i16_at(c, 2) }).collect(),
        vehicles: s["tanks"].chunks(8).map(|c| Vehicle { fleet: i16_at(c, 0), design: i16_at(c, 2), count: i32_at(c, 4) }).collect(),
        designs: vec![Vec::new(); EMPIRES],
        human_controlled: human,
        colony_ship_pool: i32s(&s["cpool"]),
        terraformer_pool: i32s(&s["tpool"]),
        ..Default::default()
    };
    for e in 1..EMPIRES {
        if let Some(d) = s.get(&format!("des{e}")) {
            w.designs[e] = d.chunks(0x84).filter(|c| c.len() == 0x84).map(Design::from_bytes).collect();
        }
    }
    w
}

fn starmap(s: &Regions) -> Starmap {
    let mut m = Starmap {
        fleets: s["fleets"].chunks(0x80).map(Fleet::from_record).collect(),
        waypoints: region(s, "wp").chunks(12).filter(|c| c.len() == 12).map(|c| Waypoint { x: f32_at(c, 0), y: f32_at(c, 4), next: i16_at(c, 8) }).collect(),
        ..Default::default()
    };
    let (r2, sz) = (region(s, "radar2"), region(s, "radarsz"));
    for e in 0..EMPIRES {
        m.radar.range[e] = std::array::from_fn(|k| f32_at(r2, e * 32 + 4 * k));
        m.radar.mult[e] = f32_at(sz, 4 * e);
    }
    m
}

fn ai_state(s: &Regions) -> AiState {
    let mut a = AiState::default();
    for e in 0..EMPIRES {
        a.main_planet[e] = i32_at(&s["fobolygo"], 4 * e);
        a.main_fleet[e] = i32_at(&s["foflotta"], 4 * e);
        a.centre_x[e] = i32_at(&s["kozx"], 4 * e);
        a.centre_y[e] = i32_at(&s["kozy"], 4 * e);
        for k in 0..DESIGN_SLOTS {
            a.designs[ai::BEST][e][k] = i16_at(&s["mutato"], e * 0x32 + 2 * k);
            a.designs[ai::SPECIAL][e][k] = i16_at(&s["mutspec"], e * 0x32 + 2 * k);
        }
        for c in 0..CATEGORIES {
            let o = e * 0x18c + c * CATEGORY_SIZE;
            a.categories[e][c] = Category::from_bytes(&s["cuccok"][o..o + CATEGORY_SIZE]);
        }
        a.empty_planets[e] = s["uresb"][e];
        a.sliders[e] = std::array::from_fn(|i| f64_at(&s["csuszka"], e * 48 + 8 * i));
        a.last_bad_colony[e] = i32_at(region(s, "kolutso"), 4 * e);
        a.no_load[e] = i32_at(region(s, "nopak"), 4 * e);
        a.load_phase[e] = i32_at(region(s, "pakfaz"), 4 * e);
        a.no_upgrade[e] = i32_at(region(s, "noupg"), 4 * e);
        let racs = region(s, "racs");
        for x in 0..GRID {
            for y in 0..GRID {
                a.scout[e][x][y] = racs.get(e * GRID * GRID + x * GRID + y).copied().unwrap_or(0);
            }
        }
    }
    a
}

/// Everything a segment needs, decoded from one checkpoint's regions.
struct Decoded {
    ctx: AiContext,
    sctx: SpyContext,
    econ: EconomyContext,
    rctx: ResearchContext,
    ai: AiState,
    colony: ColonyWorld,
    combat: CombatWorld,
    research: Research,
    relations: Vec<[Relation; EMPIRES]>,
    kidnap: [i32; 18],
    rng: Streams,
    spies: Spies,
    starmap: Starmap,
    star_ctx: StarmapContext,
    fleet_numbers: [i32; EMPIRES],
    power: [f32; EMPIRES],
    planet_count: [i32; EMPIRES],
    danger: [i32; EMPIRES],
    known: [[i32; EMPIRES]; EMPIRES],
}

fn decode(s: &Regions, g: &Value) -> Decoded {
    let gi = |k: &str| g[k].as_i64().unwrap_or(0);
    let ki = unhex(g["ki"].as_str().unwrap());
    let faj = unhex(g["faj"].as_str().unwrap());
    let human: [bool; EMPIRES] = std::array::from_fn(|e| ki[e] == 1);
    let race: [u8; EMPIRES] = faj[..EMPIRES].try_into().unwrap();
    let alive: [bool; EMPIRES] = std::array::from_fn(|e| s["el"][e] != 0);
    let sctx = SpyContext {
        alive,
        human,
        race,
        human_empire: gi("en") as usize,
        program_players: gi("pp") as i32,
        multiplayer: gi("mp") != 0,
        difficulty: gi("diff") as i32,
        day: gi("nap") as i32,
        ..Default::default()
    };
    let spare = region(s, "spare");
    let ctx = AiContext {
        ai: std::array::from_fn(|e| s["mizik"][e] != 0),
        program_players2: gi("pp2") as usize,
        spies_enabled: gi("spyen") != 0,
        story_resale: gi("storyac") != 0,
        may_attack: std::array::from_fn(|e| region(s, "tamad").get(e).is_some_and(|&x| x != 0)),
        story_no_war: gi("storyb0") != 0,
        story_guard: [gi("storyb8") != 0, gi("storybc") != 0],
        spare_human: std::array::from_fn(|e| i32_at(spare, 4 * e) != 0),
        year: gi("ev") as i32,
        month: gi("honap") as i32,
        tick: gi("ido") as i32,
        ai_off: gi("aioff") != 0,
        design_debug: gi("ddebug") != 0,
    };
    let econ = EconomyContext {
        multiplayer: sctx.multiplayer,
        human_empire: sctx.human_empire as u8,
        human_controlled: human,
        empire_race: race,
        players: gi("pp2") as i32,
        ..Default::default()
    };
    let rctx = ResearchContext {
        human_empire: sctx.human_empire as u8,
        human_controlled: human,
        race,
        players: gi("pp2") as i32,
        all_techs: gi("alltechs") != 0,
        ..Default::default()
    };
    let v = &s["visz"];
    let mut rng = Streams::new(21);
    for i in 0..21 {
        rng.set_state(i, i32_at(&s["seeds"], 4 * i) as u32);
    }
    rng.set_choice_state(i32_at(&s["rnd"], 0), i32_at(&s["rndossz"], 0));
    let m = &s["miota"];
    let star_ctx = StarmapContext {
        human_controlled: human,
        default_fleet_name: g["alapnev"].as_str().unwrap_or("Fleet").as_bytes().to_vec(),
        ..Default::default()
    };
    let ero = region(s, "ero");
    Decoded {
        ctx,
        sctx,
        econ,
        rctx,
        ai: ai_state(s),
        colony: colony_world(s, g),
        combat: combat(s, human),
        research: Research { current: i32s(&s["kutat"]), ..Default::default() },
        relations: (0..EMPIRES).map(|a| std::array::from_fn(|b| Relation::from_bytes(&v[a * 0x2d2 + b * Relation::SIZE..]))).collect(),
        kidnap: std::array::from_fn(|i| i32_at(&s["kidnap"], 4 * i)),
        rng,
        spies: spies(s),
        starmap: starmap(s),
        star_ctx,
        fleet_numbers: i32s(region(s, "sajat")),
        power: std::array::from_fn(|e| f32_at(ero, 4 * e)),
        planet_count: i32s(region(s, "hanyb")),
        danger: i32s(&s["veszely"]),
        known: std::array::from_fn(|a| std::array::from_fn(|b| i32_at(m, 4 * (a * EMPIRES + b)))),
    }
}

/// Write the port's state back into the raw regions. `after` (the exe's state at the closing checkpoint) gives
/// the bytes of grown arrays that neither side writes.
fn write_back(s: &mut Regions, d: &Decoded, after: &Regions) {
    let grow = |s: &mut Regions, name: &str, len: usize| {
        let r = s.entry(name.to_string()).or_default();
        if r.len() < len {
            let tail = after.get(name).map_or(&[][..], |a| a.get(r.len()..len.min(a.len())).unwrap_or(&[]));
            r.extend_from_slice(tail);
            r.resize(len, 0);
        }
    };
    let a = &d.ai;
    for e in 0..EMPIRES {
        for (name, x) in [
            ("fobolygo", a.main_planet[e]),
            ("foflotta", a.main_fleet[e]),
            ("kozx", a.centre_x[e]),
            ("kozy", a.centre_y[e]),
            ("kolutso", a.last_bad_colony[e]),
            ("nopak", a.no_load[e]),
            ("pakfaz", a.load_phase[e]),
            ("noupg", a.no_upgrade[e]),
            ("sajat", d.fleet_numbers[e]),
        ] {
            if let Some(r) = s.get_mut(name) {
                put(r, 4 * e, &x.to_le_bytes());
            }
        }
        if let Some(r) = s.get_mut("racs") {
            for x in 0..GRID {
                put(r, e * GRID * GRID + x * GRID, &a.scout[e][x]);
            }
        }
        for k in 0..DESIGN_SLOTS {
            put(s.get_mut("mutato").unwrap(), e * 0x32 + 2 * k, &a.designs[ai::BEST][e][k].to_le_bytes());
            put(s.get_mut("mutspec").unwrap(), e * 0x32 + 2 * k, &a.designs[ai::SPECIAL][e][k].to_le_bytes());
        }
        for c in 0..CATEGORIES {
            put(s.get_mut("cuccok").unwrap(), e * 0x18c + c * CATEGORY_SIZE, &a.categories[e][c].to_bytes());
        }
        s.get_mut("uresb").unwrap()[e] = a.empty_planets[e];
        for i in 0..6 {
            put(s.get_mut("csuszka").unwrap(), e * 48 + 8 * i, &a.sliders[e][i].to_le_bytes());
            let m = &d.colony.money[e];
            put(s.get_mut("pools").unwrap(), e * 48 + 8 * i, &m.pools[i].to_le_bytes());
            put(s.get_mut("hatar").unwrap(), e * 48 + 8 * i, &m.reserve[i].to_le_bytes());
            put(s.get_mut("szorzo").unwrap(), e * 48 + 8 * i, &m.share[i].to_le_bytes());
        }
        put(s.get_mut("money").unwrap(), 8 * e, &d.colony.money[e].total.to_le_bytes());
        for b in 0..EMPIRES {
            put(s.get_mut("miota").unwrap(), 4 * (e * EMPIRES + b), &d.known[e][b].to_le_bytes());
            put(s.get_mut("visz").unwrap(), e * 0x2d2 + b * Relation::SIZE, &d.relations[e][b].to_bytes());
            s.get_mut("ismeri").unwrap()[e * EMPIRES + b] = d.colony.knows[e][b];
        }
    }
    for e in 0..ROSTERS {
        for j in 0..SLOTS {
            put(s.get_mut("kemek").unwrap(), e * SLOTS * RECORD + j * RECORD, &d.spies.roster[e][j].to_bytes());
        }
        put(s.get_mut("ossz").unwrap(), 4 * e, &d.spies.count[e].to_le_bytes());
    }
    let r = s.get_mut("seeds").unwrap();
    for i in 0..21 {
        put(r, 4 * i, &d.rng.state(i).to_le_bytes());
    }
    let (n, total) = d.rng.choice_state();
    put(s.get_mut("rnd").unwrap(), 0, &n.to_le_bytes());
    put(s.get_mut("rndossz").unwrap(), 0, &total.to_le_bytes());
    s.insert("techs".into(), d.colony.tech.bytes.clone());
    for (name, xs) in [("cpool", d.combat.colony_ship_pool), ("tpool", d.combat.terraformer_pool), ("kutat", d.research.current)] {
        let r = s.get_mut(name).unwrap();
        for (e, x) in xs.iter().enumerate() {
            put(r, 4 * e, &x.to_le_bytes());
        }
    }
    let r = s.get_mut("ships").unwrap();
    for (i, x) in d.combat.ships.iter().enumerate() {
        put(r, 6 * i, &x.fleet.to_le_bytes());
        put(r, 6 * i + 2, &x.design.to_le_bytes());
    }
    grow(s, "tanks", 8 * d.combat.vehicles.len());
    let r = s.get_mut("tanks").unwrap();
    for (i, t) in d.combat.vehicles.iter().enumerate() {
        put(r, 8 * i, &t.fleet.to_le_bytes());
        put(r, 8 * i + 2, &t.design.to_le_bytes());
        put(r, 8 * i + 4, &t.count.to_le_bytes());
    }
    grow(s, "fleets", 0x80 * d.starmap.fleets.len());
    let r = s.get_mut("fleets").unwrap();
    for (f, x) in d.starmap.fleets.iter().enumerate() {
        let o = f * 0x80;
        put(r, o, &x.name);
        r[o + 0x15] = x.owner;
        put(r, o + 0x16, &x.story_id.to_le_bytes());
        r[o + 0x18] = x.state;
        put(r, o + 0x19, &x.visible);
        for (off, v) in [(0x30, x.x), (0x34, x.y), (0x38, x.speed), (0x3c, x.speed_override), (0x44, x.tx), (0x48, x.ty), (0x7c, x.radar)] {
            put(r, o + off, &v.to_le_bytes());
        }
        for (off, v) in [(0x40, x.order), (0x42, x.kind), (0x4c, x.colony_ships), (0x4e, x.terraformers), (0x50, x.first_waypoint)] {
            put(r, o + off, &v.to_le_bytes());
        }
        put(r, o + 0x52, &x.flags.to_le_bytes());
        if let Some(c) = d.combat.fleets.get(f) {
            let t = &c.totals;
            for (off, v) in [(0x54, t.ships), (0x58, t.fighters), (0x5c, t.ground), (0x60, t.attack), (0x64, t.fighter_attack), (0x68, t.ground_strength), (0x6c, t.hp), (0x70, t.fighter_hp), (0x74, t.field_74), (0x78, t.capacity)] {
                put(r, o + off, &v.to_le_bytes());
            }
        }
    }
    if !d.starmap.waypoints.is_empty() {
        grow(s, "wp", 12 * d.starmap.waypoints.len());
        let r = s.get_mut("wp").unwrap();
        for (i, w) in d.starmap.waypoints.iter().enumerate() {
            put(r, 12 * i, &w.x.to_le_bytes());
            put(r, 12 * i + 4, &w.y.to_le_bytes());
            put(r, 12 * i + 8, &w.next.to_le_bytes());
        }
    }
    for e in 1..EMPIRES {
        let name = format!("des{e}");
        if !s.contains_key(&name) {
            continue;
        }
        grow(s, &name, 0x84 * d.combat.designs[e].len());
        let r = s.get_mut(&name).unwrap();
        for (k, x) in d.combat.designs[e].iter().enumerate() {
            put(r, 0x84 * k, &x.to_bytes());
        }
    }
    let r = s.get_mut("planets").unwrap();
    for (p, pl) in d.colony.planets.iter().enumerate().take(r.len() / 0x188) {
        let o = p * 0x188;
        r[o + 0x34] = pl.tax_level;
        for (off, v) in [(0x58, pl.field_58), (0x5c, pl.field_5c), (0x60, pl.field_60), (0x114, pl.field_114)] {
            put(r, o + off, &v.to_le_bytes());
        }
        r[o + 0x6c] = pl.autobuild_mode;
        r[o + 0x6d] = pl.autobuild_aggr;
        put(r, o + 0x78, &pl.production);
        for (k, st) in pl.stations.iter().enumerate() {
            put(r, o + 0x100 + 2 * k, &st.to_le_bytes());
        }
    }
}

/// The hooks a replayed segment gets: the exe's war declarations and the exe's bytes for grown arrays.
struct TraceHooks<'a> {
    after: &'a Regions,
    wars: Vec<&'a Value>,
    declared: Vec<(usize, usize)>,
}

impl AiHooks for TraceHooks<'_> {
    fn declare_war(&mut self, w: &mut World<'_>, a: usize, b: usize) {
        let Some(d) = self.wars.get(self.declared.len()).copied() else {
            self.declared.push((a, b));
            return;
        };
        self.declared.push((a, b));
        if let Some(v) = d.get("visz") {
            let mut buf: Vec<u8> = w.spy.relations.iter().flat_map(|row| row.iter().flat_map(|r| r.to_bytes())).collect();
            apply_runs(&mut buf, v);
            for (x, row) in w.spy.relations.iter_mut().enumerate() {
                for (y, r) in row.iter_mut().enumerate() {
                    *r = Relation::from_bytes(&buf[x * 0x2d2 + y * Relation::SIZE..]);
                }
            }
        }
        if let Some(v) = d.get("ismeri") {
            let mut buf: Vec<u8> = w.spy.colony.knows.iter().flatten().copied().collect();
            apply_runs(&mut buf, v);
            for x in 0..EMPIRES {
                w.spy.colony.knows[x].copy_from_slice(&buf[x * EMPIRES..(x + 1) * EMPIRES]);
            }
        }
        if let Some(v) = d.get("miota") {
            let mut buf: Vec<u8> = w.known_since.iter().flatten().flat_map(|x| x.to_le_bytes()).collect();
            apply_runs(&mut buf, v);
            for x in 0..EMPIRES {
                for y in 0..EMPIRES {
                    w.known_since[x][y] = i32_at(&buf, 4 * (x * EMPIRES + y));
                }
            }
        }
    }

    fn fresh_fleet(&mut self, f: usize) -> [u8; 0x80] {
        let r = region(self.after, "fleets");
        r.get(f * 0x80..(f + 1) * 0x80).map_or([0; 0x80], |x| x.try_into().unwrap())
    }

    fn fresh_design(&mut self, e: usize, d: usize) -> [u8; 0x84] {
        let r = region(self.after, &format!("des{e}"));
        r.get(d * 0x84..(d + 1) * 0x84).map_or([0; 0x84], |x| x.try_into().unwrap())
    }
}

#[derive(Default)]
struct Stats {
    records: usize,
    segments: BTreeMap<&'static str, usize>,
    events: BTreeMap<String, usize>,
    missions: BTreeMap<u16, usize>,
    exe_calls: BTreeMap<String, usize>,
    wars: usize,
    orders: BTreeMap<(i16, i16), usize>,
}

/// An open `…0` checkpoint: where a part-2 segment starts.
struct Frame<'a> {
    at: String,
    e: usize,
    b: i64,
    state: Regions,
    /// `StoryAdat+0xb0/+0xb8/+0xbc` as `FRISSIT_MI_FLOTTAK` saw them (the tracer sets them for its duration).
    story: Option<[i64; 3]>,
    wars: Vec<&'a Value>,
    exe_wars: Vec<(usize, usize)>,
}

#[test]
fn ai_matches_every_traced_call() {
    let paths = traces("ai-");
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    if paths.is_empty() {
        eprintln!("no AI trace in _local/traces; run tools/trace/run_ai.py");
        return;
    }
    let r = ig2_rules::Rules::from_install(&inst).expect("catalogued exe");
    let rules = rules(&r);
    let mut st = Stats::default();
    let mut bad = Vec::new();
    for p in paths {
        let file = p.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&p).unwrap();
        let lines: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        let mut state = Regions::new();
        for ev in &lines {
            let k = ev["k"].as_str().unwrap();
            if k == "init" {
                for (n, h) in ev["regions"].as_object().unwrap() {
                    state.insert(n.clone(), unhex(h.as_str().unwrap()));
                }
                continue;
            }
            if k != "mi" {
                continue;
            }
            st.records += 1;
            apply(&mut state, &ev["pre"]);
            let g = &ev["g"];
            // The AI empire whose sliders are still to be written (set after its FEJLESZTES).
            let mut pending: Option<usize> = None;
            let mut frames: Vec<Frame> = Vec::new();
            for cp in ev["cps"].as_array().unwrap() {
                let at = cp["at"].as_str().unwrap();
                let e = cp["e"].as_u64().unwrap() as usize;
                let before = state.clone();
                apply(&mut state, &cp["d"]);
                for c in cp["calls"].as_array().unwrap() {
                    *st.exe_calls.entry(c[0].as_str().unwrap().to_string()).or_default() += 1;
                }
                let what = format!("{file} seq {} {at} e{e}", ev["seq"]);
                match at {
                    "kol0" | "flo0" | "des0" => {
                        let story = cp["story"].as_array().map(|v| std::array::from_fn(|i| v[i].as_i64().unwrap_or(0)));
                        frames.push(Frame { at: at[..3].to_string(), e, b: cp["b"].as_i64().unwrap_or(0), state: state.clone(), story, wars: Vec::new(), exe_wars: Vec::new() });
                        continue;
                    }
                    "war1" => {
                        st.wars += 1;
                        let b = cp["b"].as_u64().unwrap_or(0) as usize;
                        for f in frames.iter_mut() {
                            f.wars.push(&cp["d"]);
                            f.exe_wars.push((e, b));
                        }
                        continue;
                    }
                    _ => {}
                }
                let segment: Option<&'static str> = match at {
                    "szam1" => Some("bookkeeping"),
                    "gy0" => Some("planet settings"),
                    "gy1" => Some("production"),
                    "kem1" => Some("spy missions"),
                    "fej1" => Some("research pick"),
                    "kol1" => Some("colonisation"),
                    "flo1" => Some("fleets"),
                    "des1" => Some("design"),
                    "end" => Some("income shares"),
                    _ => None,
                };
                let Some(segment) = segment else { continue };
                let frame = if matches!(at, "kol1" | "flo1" | "des1") {
                    let f = frames.pop().expect("an open checkpoint");
                    assert_eq!((f.at.as_str(), f.e), (&at[..3], e), "{what}: checkpoints nest");
                    Some(f)
                } else {
                    None
                };
                let start = frame.as_ref().map_or(&before, |f| &f.state);
                let mut d = decode(start, g);
                if let Some(st) = frame.as_ref().and_then(|f| f.story) {
                    d.ctx.story_no_war = st[0] != 0;
                    d.ctx.story_guard = [st[1] != 0, st[2] != 0];
                }
                let mut story = |_: StoryCheck, _: Held| false;
                let mut hooks = TraceHooks { after: &state, wars: frame.as_ref().map_or_else(Vec::new, |f| f.wars.clone()), declared: Vec::new() };
                let events = {
                    let mut w = World {
                        ctx: &d.ctx,
                        rules: &rules.ai,
                        spy: spy::World {
                            ctx: &d.sctx,
                            colony: &mut d.colony,
                            side_rules: &rules.side,
                            econ: &d.econ,
                            combat: &mut d.combat,
                            research: &mut d.research,
                            research_rules: &rules.research,
                            research_ctx: &d.rctx,
                            relations: &mut d.relations,
                            kidnap: &mut d.kidnap,
                            rng: &mut d.rng,
                            rules: &rules.spy,
                            story: &mut story,
                            events: Vec::new(),
                        },
                        spies: &mut d.spies,
                        starmap: &mut d.starmap,
                        star_ctx: &d.star_ctx,
                        fleet_numbers: &mut d.fleet_numbers,
                        danger: &d.danger,
                        power: &d.power,
                        planet_count: &d.planet_count,
                        known_since: &mut d.known,
                        events: Vec::new(),
                    };
                    let a = &mut d.ai;
                    match at {
                        "szam1" => a.bookkeeping(&mut w),
                        "gy0" => {
                            if let Some(prev) = pending.take() {
                                a.set_sliders(prev, w.danger[prev]);
                            }
                            a.planet_settings(&mut w, e);
                        }
                        "gy1" => a.production(&mut w, e),
                        "kem1" => ai::spy_missions(&mut w, e),
                        "fej1" => {
                            ai::research_pick(&mut w, e);
                            pending = Some(e);
                        }
                        "kol1" => a.colonise(&mut w, e, &mut hooks),
                        "flo1" => a.fleets(&mut w, e, &mut hooks),
                        "des1" => {
                            let all = frame.as_ref().is_some_and(|f| f.b != 0);
                            a.design(&mut w, e, all, &mut hooks)
                        }
                        _ => {
                            if let Some(prev) = pending.take() {
                                a.set_sliders(prev, w.danger[prev]);
                            }
                            a.income_shares(&mut w.spy.colony.money);
                        }
                    }
                    w.events
                };
                if let Some(f) = &frame {
                    if hooks.declared != f.exe_wars {
                        bad.push(format!("{what} ({segment}): wars declared {:?}, exe {:?}", hooks.declared, f.exe_wars));
                    }
                }
                *st.segments.entry(segment).or_default() += 1;
                for e in &events {
                    let name = format!("{e:?}");
                    *st.events.entry(name.split([' ', '(']).next().unwrap().to_string()).or_default() += 1;
                }
                if at == "flo1" {
                    for (f, x) in d.starmap.fleets.iter().enumerate() {
                        let old = start.get("fleets").and_then(|r| r.get(f * 0x80..(f + 1) * 0x80)).map(Fleet::from_record);
                        if old.is_none_or(|o| (o.order, o.kind, o.tx, o.ty) != (x.order, x.kind, x.tx, x.ty)) {
                            *st.orders.entry((x.order, x.kind)).or_default() += 1;
                        }
                    }
                }
                if at == "kem1" {
                    for (j, s) in d.spies.roster[e].iter().enumerate() {
                        let old = Spy::from_bytes(&start["kemek"][e * SLOTS * RECORD + j * RECORD..][..RECORD]);
                        if s.state == spy::state::MISSION && (old.state != s.state || old.mission != s.mission) {
                            *st.missions.entry(s.mission).or_default() += 1;
                        }
                    }
                }
                let mut ours = start.clone();
                write_back(&mut ours, &d, &state);
                // Localised names: take the exe's bytes for the renamed fleets and the newly named designs.
                for ev in &events {
                    match ev {
                        AiEvent::MainFleetNamed { fleet, .. } => {
                            let o = fleet * 0x80;
                            let name = state["fleets"][o..o + 0x14].to_vec();
                            put(ours.get_mut("fleets").unwrap(), o, &name);
                        }
                        AiEvent::DesignNamed { empire, design, .. } => {
                            let n = format!("des{empire}");
                            let o = design * 0x84;
                            let name = state[&n][o..o + 0x15].to_vec();
                            put(ours.get_mut(&n).unwrap(), o, &name);
                        }
                        _ => {}
                    }
                }
                for (n, b) in &state {
                    let a = ours.get(n).unwrap_or(b);
                    if a != b {
                        let i = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)).unwrap();
                        bad.push(format!("{what} ({segment}): {n} len {}/{} differs at {i:#x}: ours {:?} exe {:?}", a.len(), b.len(), a.get(i), b.get(i)));
                    }
                }
            }
        }
    }
    eprintln!("{} FRISSIT_MI calls; segments {:?}", st.records, st.segments);
    eprintln!("events {:?}", st.events);
    eprintln!("new spy missions by mission: {:?}", st.missions);
    eprintln!("exe callees: {:?}", st.exe_calls);
    eprintln!("war declarations: {}; fleet orders changed by (order, kind): {:?}", st.wars, st.orders);
    if !bad.is_empty() {
        for b in bad.iter().take(40) {
            eprintln!("{b}");
        }
        panic!("{} mismatches", bad.len());
    }
}
