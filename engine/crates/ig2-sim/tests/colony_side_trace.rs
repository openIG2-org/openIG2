//! Replays the colony side systems recorded from the running GOG `ig2.exe`
//! (tools/trace/run_colony_side.py, `re/rules/colony-side.md`):
//! - `side-economy*.jsonl`: whole economy steps `FRISSIT_Planet(true, 0)` with rebellions,
//!   autobuild, money, per-empire totals, yearly income and the seen-building matrix (plain and fuzzed);
//! - `side-elfoglal.jsonl`, `side-fellazad.jsonl`, `side-colonise.jsonl`: direct calls of
//!   `BolygoElfoglal`, `BolygoFellazad` and the colonisation sequence on fuzzed state;
//! - `side-money.jsonl`: `MoneyMennyi` / `MoneyHozzaadOld` calls.
//!
//! Traces hold game state, so they stay in `_local/` (never committed). Rule tables come from the
//! user's exe and install. The test skips unless `IG2_INSTALL_DIR` and the traces exist.

use std::path::PathBuf;

use ig2_formats::text::{self, Lang};
use ig2_formats::{colony_map, misc_text, Install};
use ig2_sim::placement::{PlacementRules, Terrain};
use ig2_sim::colony::{Building, BuildingKind, ColonyRules, Curve, EconomyContext, Planet, StoryHooks};
use ig2_sim::colony_side::{
    AutobuildMode, AutobuildRules, ColonyWorld, SideEvent, SideRules, TechTable, Treasury, ZeroPick, EMPIRES,
    PLANET_SLOTS,
};
use ig2_sim::colony_side::ColonyArrivals;
use ig2_sim::fleet::{Fleet, StarPlanet, Starmap, StarmapContext, StarmapEvent, Waypoint, DELETED};
use ig2_sim::rng::Streams;
use serde_json::Value;

fn trace(name: &str) -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces");
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with(name) && f.ends_with(".jsonl")));
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
fn ints(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect()
}
fn hexs(v: &Value) -> Vec<u8> {
    unhex(v.as_str().unwrap())
}

/// The colony-map search inputs from the install: mask A of every `gen_maps/<n>.map`, the AI build
/// starts of `terepleiro.txt` and the footprints from the exe.
fn placement_rules(inst: &Install) -> PlacementRules {
    let r = ig2_rules::Rules::from_install(inst).expect("catalogued exe");
    let raw = inst.read("dataraw/colony/terepleiro.txt").expect("terepleiro.txt");
    let surfaces = colony_map::parse_surfaces(&text::decode(&raw, inst.lang())).unwrap();
    let terrains = surfaces
        .iter()
        .enumerate()
        .map(|(n, s)| {
            let m = colony_map::ColonyMap::parse(&inst.read(&format!("dataraw/colony/gen_maps/{n}.map")).unwrap()).unwrap();
            Terrain { mask: m.mask_a, kind: s.kind as u8, start: s.ai_build_start }
        })
        .collect();
    PlacementRules { terrains, footprints: r.building_footprints }
}

fn rules(inst: &Install, statics: &Value) -> SideRules {
    let r = ig2_rules::Rules::from_install(inst).expect("catalogued exe");
    let colony = ColonyRules {
        buildings: r
            .buildings
            .iter()
            .map(|b| BuildingKind {
                tech: b.tech,
                priority: b.priority,
                build_work: b.build_work,
                max_hp: b.max_hp,
                energy: b.energy,
                workers: b.workers,
                output: b.output,
                scaled_effect: b.flags[0] & 1 != 0,
                passive_effect: b.flags[1] & 1 != 0,
            })
            .collect(),
        curves: r.problem_curves.iter().map(|c| Curve { a: c.a, b: c.b, c: c.c, d: c.d, rate: c.rate, weight: c.weight }).collect(),
        surface_growth: r.race_surface_affinity.iter().map(|v| v[..9].try_into().unwrap()).collect(),
        self_destruct_fear: r.race_fear_threshold.clone(),
        kidnap_morale: r.kidnap_morale.clone(),
        race_production: r.race_production_mult.clone(),
    };
    // Autobuild modes from the install's autobldN.txt, names from the exe; checked against the live tables.
    let mut modes = vec![AutobuildMode::default()];
    for m in 1..=10 {
        let raw = inst.read(&format!("dataraw/text/autobld{m}.txt")).expect("autobld file");
        let f = misc_text::parse_autobuild(&text::decode(&raw, inst.lang()));
        modes.push(AutobuildMode::from_entries(f.threshold, f.listed_priority, &f.entries, &r.autobuild_names));
    }
    let live_list = ints(&statics["abEgyeb"]);
    for (m, mode) in modes.iter().enumerate().skip(1) {
        let live: Vec<i32> = (1..=50).map(|j| live_list[m * 51 + j] as i32).take_while(|&c| c != 0).collect();
        assert_eq!(mode.list, live, "autobld{m}.txt list");
        assert_eq!(mode.problem_threshold as i64, ints(&statics["abAlso"])[m], "autobld{m}.txt threshold");
        assert_eq!(mode.listed_priority as i64, ints(&statics["abEgyebSz"])[m], "autobld{m}.txt priority");
    }
    let take29 = |v: &Value| ints(v)[..29].iter().map(|&x| x as i32).collect::<Vec<_>>();
    assert_eq!(r.autobuild_problem, take29(&statics["abProblem"]));
    assert_eq!(r.autobuild_min, take29(&statics["abMin"]));
    assert_eq!(r.autobuild_max, take29(&statics["abMax"]));
    assert_eq!(r.production_default, hexs(&statics["pGyartas0"]));
    let firepower: Vec<i32> = r.techs.iter().map(|t| t.firepower).collect();
    assert_eq!(firepower, ints(&statics["tro"]).iter().map(|&x| x as i32).collect::<Vec<_>>(), "TalalmanyokrEro");
    // TalalmanyEroHP is computed at new game from firepower and hit points.
    let tech_hp = ig2_sim::colony_side::tech_strength(&firepower, &r.techs.iter().map(|t| t.hp).collect::<Vec<_>>());
    assert_eq!(tech_hp, ints(&statics["hp"]).iter().map(|&x| x as i32).collect::<Vec<_>>(), "TalalmanyEroHP");
    SideRules {
        colony,
        firepower,
        tech_hp,
        race_research_time: r.race_research_time_mult.clone(),
        difficulty_money: r.difficulty_money_mult.clone(),
        production_default: r.production_default.clone().try_into().unwrap(),
        autobuild: AutobuildRules { problem: r.autobuild_problem, min: r.autobuild_min, max: r.autobuild_max, modes },
    }
}

fn planet(raw: &str, bld: &str, unrest: i32) -> Planet {
    let r = unhex(raw);
    let b = unhex(bld);
    let mut p = Planet {
        x: f32_at(&r, 0x18),
        y: f32_at(&r, 0x1c),
        owner: r[0x20],
        race: r[0x21],
        surface: r[0x22],
        colony_map: r[0x23],
        richness: i32_at(&r, 0x24),
        population: i32_at(&r, 0x28),
        population_change: i32_at(&r, 0x2c),
        morale: i32_at(&r, 0x30),
        tax_level: r[0x34],
        gdp_acc: i32_at(&r, 0x38),
        gdp_last: i32_at(&r, 0x3c),
        tax_acc: i32_at(&r, 0x40),
        tax_last: i32_at(&r, 0x44),
        energy_produced: i32_at(&r, 0x48),
        energy_demand: i32_at(&r, 0x4c),
        workers_demand: i32_at(&r, 0x50),
        field_54: u16::from_le_bytes([r[0x54], r[0x55]]),
        field_58: i32_at(&r, 0x58),
        field_5c: i32_at(&r, 0x5c),
        protected: r[0x64] != 0,
        autobuild_counter: i32_at(&r, 0x68),
        autobuild_mode: r[0x6c],
        autobuild_aggr: r[0x6d],
        ship_capacity: i32_at(&r, 0x70),
        vehicle_capacity: i32_at(&r, 0x74),
        production: r[0x78..0xb8].try_into().unwrap(),
        epidemic: i16_at(&r, 0xb8),
        field_114: i32_at(&r, 0x114),
        morale_penalty: i32_at(&r, 0x174),
        morale_penalty_rate: i32_at(&r, 0x178),
        loyal: r[0x186] != 0,
        ai_proof: r[0x187] != 0,
        unrest,
        ..Default::default()
    };
    p.spies.copy_from_slice(&r[0xc4..0xc4 + 19]);
    for e in 0..19 {
        p.visibility[e] = i16_at(&r, 0xd8 + 2 * e);
    }
    for k in 1..p.problems.len() {
        p.problems[k] = i32_at(&r, 0x118 + 4 * k);
    }
    for k in 0..5 {
        p.events[k] = i16_at(&r, 0x16a + 2 * k);
    }
    p.buildings = b
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

fn treasury(s: &Value, e: usize) -> Treasury {
    let (m, p, h, z) = (hexs(&s["money"]), hexs(&s["pools"]), hexs(&s["hatar"]), hexs(&s["szorzo"]));
    let six = |b: &[u8]| -> [f64; 6] { std::array::from_fn(|k| f64_at(b, e * 48 + 8 * k)) };
    Treasury { total: f64_at(&m, 8 * e), pools: six(&p), reserve: six(&h), share: six(&z) }
}

fn context(s: &Value) -> EconomyContext {
    let ki = hexs(&s["playerKi"]);
    let faj = hexs(&s["playerFaj"]);
    let story = hexs(&s["story"]);
    let kidnap = ints(&s["kidnap"]);
    EconomyContext {
        year: s["ev"].as_i64().unwrap() as i32,
        month: s["honap"].as_i64().unwrap() as i32,
        day: s["nap"].as_i64().unwrap() as i32,
        multiplayer: s["mp"].as_i64().unwrap() != 0,
        human_empire: s["en"].as_i64().unwrap() as u8,
        human_controlled: std::array::from_fn(|e| ki[e] == 1),
        empire_race: faj.try_into().unwrap(),
        players: s["players2"].as_i64().unwrap() as i32,
        kidnap_state: std::array::from_fn(|e| kidnap[e] as i32),
        global_morale: s["gmoral"].as_i64().unwrap() as i32,
        global_morale_speed: s["gmoralSpeed"].as_i64().unwrap() as i32,
        story: StoryHooks {
            food_halved_planet: i32_at(&story, 0x20),
            lab_bonus_empire: i32_at(&story, 0x24),
            food_lost: i32_at(&story, 0x2c),
            hospital_watch: i32_at(&story, 0x30),
            lab_start: std::array::from_fn(|i| i32_at(&story, 0x9c + 4 * i)),
        },
        story_values: Vec::new(),
    }
}

fn world(s: &Value) -> (ColonyWorld, Streams) {
    let lazad = ints(&s["lazad"]);
    let list = s["planets"].as_array().unwrap();
    let n = list.len();
    let mut planets = vec![Planet::default(); n + 1];
    for p in list {
        let i = p["i"].as_u64().unwrap() as usize;
        planets[i] = planet(p["raw"].as_str().unwrap(), p["bld"].as_str().unwrap(), lazad[i] as i32);
    }
    let per_planet = |k: &str| {
        let mut v = vec![0i32; PLANET_SLOTS.max(n + 1)];
        for (i, x) in ints(&s[k]).into_iter().enumerate() {
            v[i] = x as i32;
        }
        v
    };
    let mut w = ColonyWorld {
        planets,
        occupied: per_planet("hanyEpulet"),
        needed_morale: per_planet("neededMoral"),
        last_near_fleet: per_planet("utolso"),
        tech: TechTable { bytes: hexs(&s["tech"]) },
        all_techs: s["allTechs"].as_i64().unwrap() != 0,
        difficulty: s["nehezseg"].as_i64().unwrap() as usize,
        ..Default::default()
    };
    let arr = |k: &str| ints(&s[k]).into_iter().map(|x| x as i32).collect::<Vec<_>>();
    w.strength.ground = arr("foldi");
    w.strength.fortress = arr("erod");
    w.strength.ion_gun = arr("iongun");
    w.strength.fire_brigade = arr("tuzolto");
    let t = &mut w.empires;
    let (labor, ossz) = (arr("labor"), arr("laborOssz"));
    for e in 0..EMPIRES {
        for i in 0..4 {
            t.labs[e][i] = labor[4 * e + i];
            t.lab_buildings[e][i] = ossz[4 * e + i];
        }
    }
    t.spy_capacity = arr("kemLehet").try_into().unwrap();
    t.spy_centres = arr("kemOssz").try_into().unwrap();
    let per = hexs(&s["kemPer"]);
    for e in 0..9 {
        for p in 0..PLANET_SLOTS {
            t.spy_centres_by_planet[e][p] = i32_at(&per, 4 * (e * PLANET_SLOTS + p));
        }
    }
    t.trade_ports = arr("tradePorts").try_into().unwrap();
    t.full_gdp = arr("fullGdp").try_into().unwrap();
    t.full_tax = arr("fullTax").try_into().unwrap();
    t.tax_this_year = arr("taxThis").try_into().unwrap();
    t.tax_last_year = arr("taxLast").try_into().unwrap();
    t.trade_this_year = arr("tradeThis").try_into().unwrap();
    t.trade_last_year = arr("tradeLast").try_into().unwrap();
    for e in 0..EMPIRES {
        w.money[e] = treasury(s, e);
    }
    let el = hexs(&s["el"]);
    let ismeri = hexs(&s["ismeri"]);
    for a in 0..EMPIRES {
        w.alive[a] = el[a] != 0;
        for b in 0..EMPIRES {
            w.knows[a][b] = ismeri[a * EMPIRES + b];
        }
    }
    let pick = arr("abMitEpit");
    w.autobuild_pick.copy_from_slice(&pick[..29]);
    let mut rng = Streams::default();
    for (i, v) in ints(&s["seeds"]).into_iter().enumerate() {
        rng.set_state(i, v as u32);
    }
    rng.set_choice_state(s["rndN"].as_i64().unwrap() as i32, s["rndTotal"].as_i64().unwrap() as i32);
    (w, rng)
}

/// Fleets (order, kind, target) that `BolygoElfoglal` must stop: alive, `order 2, kind 1, target p`.
fn attackers(s: &Value, p: usize) -> Vec<(usize, i16, i16, f32)> {
    hexs(&s["fleets"])
        .chunks(0x80)
        .enumerate()
        .map(|(i, f)| (i + 1, i16_at(f, 0x40), i16_at(f, 0x42), f32_at(f, 0x44), f[0x15]))
        .filter(|&(_, o, k, t, owner)| owner != 0xff && o == 2 && k == 1 && t == p as f32)
        .map(|(i, o, k, t, _)| (i, o, k, t))
        .collect()
}

/// Compare the whole modelled world with the game's `after` snapshot. `before_slots[p]` is the planet's
/// slot count before the call: a slot appended since comes from `realloc`, and `EpuletNew` keeps its
/// uninitialised flag bit 7 (`&= 0x80`; 0x80 under Frida's debug heap), so that bit is ignored there.
fn compare(tag: &str, ours: &ColonyWorld, rng: &Streams, after: &Value, before_slots: &[usize], miss: &mut Vec<String>) {
    let (mut theirs, trng) = world(after);
    for (t, &n) in theirs.planets.iter_mut().zip(before_slots) {
        for b in t.buildings.iter_mut().skip(n) {
            b.flags &= 0x7f;
        }
    }
    let mut m = |what: String| miss.push(format!("{tag}: {what}"));
    for (i, (o, t)) in ours.planets.iter().zip(&theirs.planets).enumerate().skip(1) {
        if o.buildings != t.buildings {
            for (j, (ob, tb)) in o.buildings.iter().zip(&t.buildings).enumerate() {
                if ob != tb {
                    m(format!("planet {i} slot {}: {}", j + 1, diff(ob, tb)));
                }
            }
            if o.buildings.len() != t.buildings.len() {
                m(format!("planet {i}: {} slots, game {}", o.buildings.len(), t.buildings.len()));
            }
        }
        let strip = |p: &Planet| Planet { buildings: Vec::new(), ..p.clone() };
        if strip(o) != strip(t) {
            m(format!("planet {i}: {}", diff(&strip(o), &strip(t))));
        }
    }
    for s in 0..21 {
        if rng.state(s) != trng.state(s) {
            m(format!("stream {s} state"));
        }
    }
    if rng.choice_state() != trng.choice_state() {
        m(format!("weighted choice state ours {:?} game {:?}", rng.choice_state(), trng.choice_state()));
    }
    let n = ours.planets.len();
    for (name, a, b) in [
        ("occupied", &ours.occupied, &theirs.occupied),
        ("needed morale", &ours.needed_morale, &theirs.needed_morale),
        ("last near fleet", &ours.last_near_fleet, &theirs.last_near_fleet),
    ] {
        if a[..n] != b[..n] {
            m(format!("{name}: ours {:?}\n  game {:?}", &a[..n], &b[..n]));
        }
    }
    if ours.strength != theirs.strength {
        let s = |x: &ig2_sim::colony_side::PlanetStrength| {
            (x.ground[..n].to_vec(), x.fortress[..n].to_vec(), x.ion_gun[..n].to_vec(), x.fire_brigade[..n].to_vec())
        };
        m(format!("strength: ours {:?}\n  game {:?}", s(&ours.strength), s(&theirs.strength)));
    }
    for e in 0..9 {
        for q in 0..PLANET_SLOTS {
            let (a, b) = (ours.empires.spy_centres_by_planet[e][q], theirs.empires.spy_centres_by_planet[e][q]);
            if a != b {
                m(format!("spy centres of empire {e} on planet {q}: ours {a} game {b}"));
            }
        }
    }
    if ours.empires != theirs.empires {
        let (a, b) = (&ours.empires, &theirs.empires);
        m(format!(
            "empire tables: labs {:?}/{:?} lab bld {:?}/{:?} spy {:?}/{:?} centres {:?}/{:?} ports {:?}/{:?} gdp {:?}/{:?} tax {:?}/{:?} year {:?}/{:?} last {:?}/{:?} trade {:?}/{:?} per-planet equal {}",
            a.labs, b.labs, a.lab_buildings, b.lab_buildings, a.spy_capacity, b.spy_capacity, a.spy_centres, b.spy_centres,
            a.trade_ports, b.trade_ports, a.full_gdp, b.full_gdp, a.full_tax, b.full_tax, a.tax_this_year, b.tax_this_year,
            a.tax_last_year, b.tax_last_year, a.trade_last_year, b.trade_last_year,
            a.spy_centres_by_planet == b.spy_centres_by_planet
        ));
    }
    for e in 0..EMPIRES {
        let (a, b) = (&ours.money[e], &theirs.money[e]);
        let bits = |t: &Treasury| (t.total.to_bits(), t.pools.map(f64::to_bits), t.reserve.map(f64::to_bits));
        if bits(a) != bits(b) {
            m(format!("money {e}: {}", diff(a, b)));
        }
    }
    if ours.tech != theirs.tech {
        let diff: Vec<String> = ours
            .tech
            .bytes
            .iter()
            .zip(&theirs.tech.bytes)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .take(8)
            .map(|(i, (a, b))| format!("tech {} +{:#x}: ours {a} game {b}", i / 0x114, i % 0x114))
            .collect();
        m(format!("tech table: {diff:?}"));
    }
    if ours.knows != theirs.knows {
        m("contacts (BirodalomIsmeri)".into());
    }
    if ours.autobuild_pick != theirs.autobuild_pick {
        m(format!("AutoBuildMitEpit ours {:?} game {:?}", ours.autobuild_pick, theirs.autobuild_pick));
    }
}

/// The differing `name: value` fields of two Debug renderings (flat structs and arrays).
fn diff<T: std::fmt::Debug>(a: &T, b: &T) -> String {
    let split = |x: &T| {
        let s = format!("{x:?}");
        let mut out = Vec::new();
        let (mut depth, mut cur) = (0, String::new());
        for ch in s.chars() {
            match ch {
                '[' | '(' => depth += 1,
                ']' | ')' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(std::mem::take(&mut cur));
                    continue;
                }
                _ => {}
            }
            cur.push(ch);
        }
        out.push(cur);
        out
    };
    let (x, y) = (split(a), split(b));
    x.iter().zip(&y).filter(|(p, q)| p != q).map(|(p, q)| format!("{} -> game {}", p.trim(), q.trim())).collect::<Vec<_>>().join("; ")
}

fn install() -> Option<Install> {
    Install::from_env(Lang::En).map(|i| i.expect("valid install"))
}

fn statics(paths: &[PathBuf]) -> Option<Value> {
    paths.iter().find_map(|p| {
        std::fs::read_to_string(p)
            .ok()?
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .find(|v| v["k"] == "static")
    })
}

/// Position of the building the game created on planet `p` in `after` (an empty slot before).
fn placed(before: &Value, after: &Value, p: usize, kind: u8) -> Option<(f32, f32)> {
    let find = |s: &Value| {
        let l = s["planets"].as_array().unwrap();
        let x = l.iter().find(|x| x["i"].as_u64() == Some(p as u64)).unwrap();
        planet(x["raw"].as_str().unwrap(), x["bld"].as_str().unwrap(), 0).buildings
    };
    let (b, a) = (find(before), find(after));
    a.iter()
        .enumerate()
        .find(|&(j, n)| n.kind == kind && b.get(j).is_none_or(|o| o.kind == 0))
        .map(|(_, n)| (n.x, n.y))
}

#[test]
fn money_calls_match_the_game() {
    let Some(inst) = install() else { return };
    let paths = trace("side-money");
    let Some(st) = statics(&paths) else { return };
    let rules = rules(&inst, &st);
    let (mut calls, mut miss) = (0, Vec::new());
    let mut prices_checked = std::collections::BTreeMap::<bool, i32>::new();
    for path in &paths {
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let ev: Value = serde_json::from_str(line).unwrap();
            if ev["k"] != "money" {
                continue;
            }
            let mut tech = TechTable::default();
            if let Some(prices) = ev.get("prices") {
                for (t, x) in ints(prices).into_iter().enumerate() {
                    tech.bytes[t * 0x114 + 0x14..t * 0x114 + 0x18].copy_from_slice(&(x as i32).to_le_bytes());
                }
            }
            for c in ev["calls"].as_array().unwrap() {
                calls += 1;
                if c["f"] == "price" {
                    let (race, t) = (c["race"].as_u64().unwrap() as u8, c["t"].as_u64().unwrap() as usize);
                    let r = ig2_sim::colony_side::price(&tech, race, t);
                    *prices_checked.entry(race == 5).or_insert(0) += 1;
                    if r as i64 != c["r"].as_i64().unwrap() {
                        miss.push(format!("GetGyartasAr(race {race}, tech {t}) ours {r} game {}", c["r"]));
                    }
                    continue;
                }
                let (e, k) = (c["e"].as_u64().unwrap() as usize, c["k"].as_i64().unwrap() as i32);
                let pre = &c["pre"];
                let mut w = ColonyWorld { difficulty: pre["nehezseg"].as_i64().unwrap() as usize, ..Default::default() };
                w.money[e] = treasury(pre, e);
                let ki = hexs(&pre["playerKi"]);
                let ctx = EconomyContext {
                    multiplayer: pre["mp"].as_i64().unwrap() != 0,
                    human_controlled: std::array::from_fn(|i| ki[i] == 1),
                    ..Default::default()
                };
                if c["f"] == "mennyi" {
                    let r = w.money[e].available(k);
                    if r.to_bits() != c["r"].as_f64().unwrap().to_bits() {
                        miss.push(format!("MoneyMennyi({e}, {k}) ours {r} game {}", c["r"]));
                    }
                } else {
                    // A non-finite amount reaches the trace as JSON null: input unknown, skip.
                    let Some(x) = c["x"].as_f64() else { continue };
                    let r = w.add_money(e, k, x, &rules, &ctx) as i64;
                    let post = treasury(&c["post"], e);
                    let bits = |t: &Treasury| (t.total.to_bits(), t.pools.map(f64::to_bits));
                    if r != c["r"].as_i64().unwrap() || bits(&w.money[e]) != bits(&post) {
                        miss.push(format!("MoneyHozzaad({e}, {k}, {x}) ours {r} {:?}\n  game {} {:?}", w.money[e], c["r"], post));
                    }
                }
            }
        }
    }
    eprintln!("money: {calls} calls (prices checked, by race 5: {prices_checked:?})");
    for m in miss.iter().take(10) {
        eprintln!("{m}");
    }
    assert!(calls >= 200, "money trace too short");
    assert!(miss.is_empty(), "{} money mismatches", miss.len());
}

#[test]
fn direct_calls_match_the_game() {
    let Some(inst) = install() else { return };
    let mut all = trace("side-elfoglal");
    all.extend(trace("side-fellazad"));
    all.extend(trace("side-colonise"));
    let Some(st) = statics(&all) else { return };
    let rules = rules(&inst, &st);
    let placement = placement_rules(&inst);
    let mut placements = 0;
    let mut miss = Vec::new();
    let mut counts = std::collections::BTreeMap::<String, i32>::new();
    for path in &all {
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let ev: Value = serde_json::from_str(line).unwrap();
            let kind = ev["k"].as_str().unwrap().to_string();
            if !["elfoglal", "fellazad", "colonise"].contains(&kind.as_str()) {
                continue;
            }
            let (before, after, args) = (&ev["before"], &ev["after"], &ev["args"]);
            let (mut w, mut rng) = world(before);
            let slots: Vec<usize> = w.planets.iter().map(|p| p.buildings.len()).collect();
            let ctx = context(before);
            let p = args["p"].as_u64().unwrap() as usize;
            let mut events = Vec::new();
            let tag = format!("{} {kind} #{}", path.file_name().unwrap().to_string_lossy(), ev["seq"]);
            let old_owner = w.planets[p].owner;
            match kind.as_str() {
                "elfoglal" => w.transfer(p, args["owner"].as_u64().unwrap() as u8, &rules, &ctx, &mut events),
                "fellazad" => {
                    let r = w.rebel(p, args["to"].as_u64().unwrap() as u8, args["force"].as_i64().unwrap() != 0, &rules, &ctx, &mut rng, &mut events);
                    if r as i64 != ev["ret"].as_i64().unwrap() {
                        miss.push(format!("{tag}: result ours {r} game {}", ev["ret"]));
                    }
                    *counts.entry(format!("fellazad result {r}")).or_default() += 1;
                }
                _ => {
                    let owner = args["owner"].as_u64().unwrap() as u8;
                    // The colony-map search (no ground war on a planet being colonised).
                    let mut place = |pl: &Planet, _: usize, k: u8| placement.place(pl, k, &[]);
                    let slot = w.colonise(p, owner, &mut place, &rules, &ctx, &mut rng, &mut events);
                    placements += placed(before, after, p, 1).is_some() as i32;
                    if slot as i64 != ev["ret"].as_i64().unwrap() {
                        miss.push(format!("{tag}: hub slot ours {slot} game {}", ev["ret"]));
                    }
                }
            }
            *counts.entry(kind.clone()).or_default() += 1;
            let captured = events.iter().filter(|e| matches!(e, SideEvent::TechCaptured { .. })).count();
            *counts.entry("techs captured".into()).or_default() += captured as i32;
            // Fleets attacking the planet are recalled when it was owned.
            let recall = events.contains(&SideEvent::RecallAttackers { planet: p });
            let left = attackers(after, p);
            if recall && !left.is_empty() {
                miss.push(format!("{tag}: attackers still ordered {left:?}"));
            }
            if !recall && old_owner != 0 && w.planets[p].owner != old_owner {
                miss.push(format!("{tag}: no recall"));
            }
            compare(&tag, &w, &rng, after, &slots, &mut miss);
        }
    }
    eprintln!("direct calls: {counts:?}, {placements} colony hubs placed by the search");
    for m in miss.iter().take(12) {
        eprintln!("{m}");
    }
    assert!(counts.get("elfoglal").copied().unwrap_or(0) >= 30, "elfoglal trace too short");
    assert!(counts.get("fellazad").copied().unwrap_or(0) >= 30, "fellazad trace too short");
    assert!(counts.get("colonise").copied().unwrap_or(0) >= 20, "colonise trace too short");
    assert!(miss.is_empty(), "{} mismatches", miss.len());
}

#[test]
fn economy_steps_match_the_game() {
    let Some(inst) = install() else { return };
    let paths = trace("side-economy");
    let Some(st) = statics(&paths) else { return };
    let rules = rules(&inst, &st);
    let placement = placement_rules(&inst);
    let mut miss = Vec::new();
    let mut counts = std::collections::BTreeMap::<&str, i32>::new();
    for path in &paths {
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let ev: Value = serde_json::from_str(line).unwrap();
            if ev["k"] != "economy" {
                continue;
            }
            let (before, after) = (&ev["before"], &ev["after"]);
            let tag = format!("{} step #{} ({}-{}-{})", path.file_name().unwrap().to_string_lossy(), ev["seq"], before["ev"], before["honap"], before["nap"]);
            // A pick of building 0 reads an uninitialised stack slot in the original (ZeroPick): accept
            // the step when either outcome reproduces it exactly.
            let mut tried = Vec::new();
            for zero in [ZeroPick::Skip, ZeroPick::Attempt] {
                let (mut w, mut rng) = world(before);
                w.zero_pick = zero;
                let slots: Vec<usize> = w.planets.iter().map(|p| p.buildings.len()).collect();
                let mut ctx = context(before);
                // Autobuild purchases are placed by the colony-map search (traced planets have no ground war).
                let mut place = |pl: &Planet, _: usize, k: u8| placement.place(pl, k, &[]);
                let (_, events) = w.economy_step(&rules, &mut ctx, &mut rng, &mut place);
                let mut m = Vec::new();
                if ctx.global_morale as i64 != after["gmoral"].as_i64().unwrap() {
                    m.push(format!("{tag}: global morale"));
                }
                compare(&tag, &w, &rng, after, &slots, &mut m);
                tried.push((zero, events, m));
            }
            let pick = tried.iter().position(|t| t.2.is_empty()).unwrap_or(0);
            let (zero, events, m) = tried.swap_remove(pick);
            if m.is_empty() && zero == ZeroPick::Attempt {
                *counts.entry("steps needing ZeroPick::Attempt").or_default() += 1;
            }
            miss.extend(m);
            *counts.entry("steps").or_default() += 1;
            *counts.entry("monthly").or_default() += (before["nap"] == 1) as i32;
            for e in &events {
                let k = match e {
                    SideEvent::Rebelled { .. } => "rebellions",
                    SideEvent::AutobuildPlaced { .. } => "autobuild placed",
                    SideEvent::AutobuildNoRoom { .. } => "autobuild no room",
                    SideEvent::Abandoned { .. } => "abandoned",
                    SideEvent::TechCaptured { .. } => "techs captured",
                    SideEvent::YearClosed => "years closed",
                    _ => continue,
                };
                *counts.entry(k).or_default() += 1;
            }
        }
    }
    eprintln!("economy: {counts:?}");
    for m in miss.iter().take(12) {
        eprintln!("{m}");
    }
    assert!(counts.get("steps").copied().unwrap_or(0) >= 30, "economy trace too short");
    assert!(miss.is_empty(), "{} mismatches", miss.len());
}

/// The starmap as far as an arrival tick needs it, from a colony-side snapshot: every fleet record (waypoint
/// chains dropped: only staged arrivals are checked) and the planets' starmap view.
fn starmap(s: &Value, w: &ColonyWorld) -> (Starmap, StarmapContext) {
    let raw = hexs(&s["fleets"]);
    let mut fleets = vec![Fleet { owner: DELETED, ..Default::default() }];
    fleets.extend(raw.chunks(0x80).map(|r| Fleet { first_waypoint: 0, ..Fleet::from_record(r) }));
    let planets = w
        .planets
        .iter()
        .map(|p| StarPlanet {
            x: p.x,
            y: p.y,
            owner: p.owner,
            epidemic: p.epidemic,
            visible: p.visibility,
            has_hub: p.buildings.iter().any(|b| b.kind == 1 && b.work_left == 0),
            ..Default::default()
        })
        .collect();
    let ki = hexs(&s["playerKi"]);
    let (el, faj) = (hexs(&s["el"]), hexs(&s["playerFaj"]));
    let ctx = StarmapContext {
        tick: s["ido2"].as_i64().unwrap() as i32,
        speed_mult: s["mult"].as_i64().unwrap() as i32,
        multiplayer: s["mp"].as_i64().unwrap() != 0,
        human_empire: s["en"].as_i64().unwrap() as u8,
        human_controlled: std::array::from_fn(|e| ki[e] == 1),
        alive: std::array::from_fn(|e| el[e] != 0),
        empire_race: faj.try_into().unwrap(),
        default_fleet_name: b"Fleet".to_vec(),
        ..Default::default()
    };
    (Starmap { fleets, waypoints: vec![Waypoint::default()], planets, ..Default::default() }, ctx)
}

#[test]
fn fleet_arrivals_match_the_game() {
    let Some(inst) = install() else { return };
    let paths = trace("side-arrive");
    let Some(st) = statics(&paths) else { return };
    let rules = rules(&inst, &st);
    let placement = placement_rules(&inst);
    // Terepek[i]+0 in the live game = the [felszin] types the port parses.
    let live: Vec<u8> = st["terrainKinds"].as_array().unwrap().iter().map(|k| k.as_u64().unwrap() as u8).collect();
    assert_eq!(live, placement.terrains.iter().map(|t| t.kind).collect::<Vec<_>>(), "terrain types");
    let mut miss = Vec::new();
    let mut counts = std::collections::BTreeMap::<&str, i32>::new();
    for path in &paths {
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let ev: Value = serde_json::from_str(line).unwrap();
            if ev["k"] != "arrive" {
                continue;
            }
            let (before, after) = (&ev["before"], &ev["after"]);
            let tag = format!("{} #{}", path.file_name().unwrap().to_string_lossy(), ev["seq"]);
            let (mut w, mut rng) = world(before);
            w.map_cursor = hexs(&before["mapCursor"]).try_into().unwrap();
            let ectx = context(before);
            let (mut map, mut sctx) = starmap(before, &w);
            let map_before = map.fleets.clone();
            let mut arrivals = ColonyArrivals { world: &mut w, rules: &rules, ctx: &ectx, placement: &placement, events: Vec::new() };
            let events = map.tick(&[], &mut sctx, &mut rng, &mut arrivals);
            let side_events = arrivals.events.len();
            let (mut theirs, trng) = world(after);
            // Bit 7 of a newly appended building slot is uninitialised memory (spec §6.3): masked like `compare`.
            let slots: Vec<usize> = world(before).0.planets.iter().map(|p| p.buildings.len()).collect();
            for (t, &n) in theirs.planets.iter_mut().zip(&slots) {
                for b in t.buildings.iter_mut().skip(n) {
                    b.flags &= 0x7f;
                }
            }
            let mut checked = Vec::new();
            let game_fleets: Vec<Fleet> = hexs(&after["fleets"]).chunks(0x80).map(Fleet::from_record).collect();
            let mut m = |what: String| miss.push(format!("{tag}: {what}"));
            for s in ev["staged"].as_array().unwrap() {
                let (f, p) = (s["fleet"].as_u64().unwrap() as usize, s["planet"].as_u64().unwrap() as usize);
                let (o, t) = (&w.planets[p], &theirs.planets[p]);
                if !checked.contains(&p) && o != t {
                    let strip = |p: &Planet| Planet { buildings: Vec::new(), ..p.clone() };
                    m(format!("planet {p}: {}", diff(&strip(o), &strip(t))));
                    for (j, (ob, tb)) in o.buildings.iter().zip(&t.buildings).enumerate().filter(|(_, (a, b))| a != b) {
                        m(format!("planet {p} slot {}: {}", j + 1, diff(ob, tb)));
                    }
                    if o.buildings.len() != t.buildings.len() {
                        m(format!("planet {p}: {} slots, game {}", o.buildings.len(), t.buildings.len()));
                    }
                }
                checked.push(p);
                // Production at the end of the tick may build a new fleet into the freed slot (the last free
                // one): a record without the colonist's flags at another position is a reuse, not a survivor.
                let staged_fleet = &map_before[f];
                let gone = game_fleets.get(f - 1).is_none_or(|g| {
                    !g.alive() || (g.flags & 0x18 == 0 && (g.x, g.y) != (staged_fleet.x, staged_fleet.y))
                });
                if map.fleets[f].alive() == gone {
                    m(format!("fleet {f} alive ours {} game {}", map.fleets[f].alive(), !gone));
                }
            }
            for (i, (o, t)) in w.planets.iter().zip(&theirs.planets).enumerate().skip(1) {
                if o.owner != t.owner {
                    m(format!("planet {i} owner ours {} game {}", o.owner, t.owner));
                }
            }
            if rng.state(1) != trng.state(1) {
                m("stream 1 state".into());
            }
            let cursor: [u8; 8] = hexs(&after["mapCursor"]).try_into().unwrap();
            if w.map_cursor != cursor {
                m(format!("map cursor ours {:?} game {cursor:?}", w.map_cursor));
            }
            if w.knows != theirs.knows {
                m("BirodalomIsmeri".into());
            }
            *counts.entry("ticks").or_default() += 1;
            for e in &events {
                match e {
                    StarmapEvent::Colonise { .. } => *counts.entry("colonisations").or_default() += 1,
                    StarmapEvent::Terraform { .. } => *counts.entry("terraformings").or_default() += 1,
                    StarmapEvent::ColonyShipFreed { .. } | StarmapEvent::TerraformerFreed { .. } => {
                        *counts.entry("stood down (target already taken this tick)").or_default() += 1
                    }
                    _ => {}
                }
            }
            *counts.entry("colony-side events").or_default() += side_events as i32;
        }
    }
    eprintln!("arrivals: {counts:?}");
    for m in miss.iter().take(12) {
        eprintln!("{m}");
    }
    assert!(counts.get("ticks").copied().unwrap_or(0) >= 50, "arrival trace too short");
    assert!(miss.is_empty(), "{} mismatches", miss.len());
}
