//! Replays production and research recorded from the running GOG `ig2.exe`
//! (tools/trace/run_prodres.py, `re/rules/production-research.md`):
//! - `prod-research-*.jsonl`: every `FRISSIT_Research` call (exe 0x51ddd0), plain and fuzzed;
//! - `prod-production-*.jsonl`: the production block of `FRISSIT_Starmap` (exe 0x597322..0x597aac).
//!
//! A trace holds byte regions: a full dump, then per record the changes made before the call (other
//! systems, the fuzzer) and by the call. The replay rebuilds the state, runs the port, writes its result
//! back into the raw records and compares them byte for byte with the recorded "after" state.
//! Traces hold game state, so they stay in `_local/` (never committed). Rule tables come from the
//! user's exe. The test skips unless `IG2_INSTALL_DIR` and the traces exist.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::colony_side::{TechTable, Treasury, EMPIRES};
use ig2_sim::fleet::Fleet;
use ig2_sim::production::{ProductionContext, ProductionEvent, Queue, Shipyard, YardFleet, YardPlanet, STATION_SLOTS};
use ig2_sim::research::{self, Research, ResearchContext, ResearchEvent, ResearchRules};
use ig2_sim::spacewar::{Design, FleetTotals, Ship, Vehicle};
use serde_json::Value;

fn traces(prefix: &str) -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces");
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

fn treasuries(s: &Regions) -> [Treasury; EMPIRES] {
    let (m, p, h, z) = (&s["money"], &s["pools"], &s["hatar"], &s["szorzo"]);
    std::array::from_fn(|e| {
        let six = |b: &[u8]| -> [f64; 6] { std::array::from_fn(|k| f64_at(b, e * 48 + 8 * k)) };
        Treasury { total: f64_at(m, 8 * e), pools: six(p), reserve: six(h), share: six(z) }
    })
}
fn put_treasuries(s: &mut Regions, money: &[Treasury; EMPIRES]) {
    for (e, t) in money.iter().enumerate() {
        put(s.get_mut("money").unwrap(), 8 * e, &t.total.to_le_bytes());
        for k in 0..6 {
            put(s.get_mut("pools").unwrap(), e * 48 + 8 * k, &t.pools[k].to_le_bytes());
            put(s.get_mut("hatar").unwrap(), e * 48 + 8 * k, &t.reserve[k].to_le_bytes());
            put(s.get_mut("szorzo").unwrap(), e * 48 + 8 * k, &t.share[k].to_le_bytes());
        }
    }
}

fn install() -> Option<Install> {
    Install::from_env(Lang::En).map(|i| i.expect("valid install"))
}

/// Compare every region the port models; report the first differing byte of each.
fn compare(ours: &Regions, theirs: &Regions, names: &[&str], what: &str, out: &mut Vec<String>) {
    for &n in names {
        let (a, b) = (&ours[n], &theirs[n]);
        if a != b {
            let i = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)).unwrap();
            out.push(format!("{what}: region {n} len {}/{} differs at {i:#x}: ours {:?} exe {:?}", a.len(), b.len(), a.get(i), b.get(i)));
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Research

#[test]
fn research_matches_every_traced_tick() {
    let paths = traces("prod-research");
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    if paths.is_empty() {
        eprintln!("no research trace in _local/traces; run tools/trace/run_prodres.py --mode research");
        return;
    }
    let r = ig2_rules::Rules::from_install(&inst).expect("catalogued exe");
    let rules = ResearchRules {
        race_time: r.race_research_time_mult.clone(),
        race_money: r.race_research_money_mult.clone(),
        difficulty_speed: r.difficulty_development_mult.clone(),
        difficulty_money: r.difficulty_money_mult.clone(),
    };
    for p in paths {
        replay_research(&p, &rules);
    }
}

fn research_context(g: &Value, s: &Regions) -> ResearchContext {
    let n = |k: &str| g[k].as_i64().unwrap() as i32;
    ResearchContext {
        speed_mult: n("speed"),
        day: n("nap"),
        difficulty: n("diff") as usize,
        multiplayer: n("mp") != 0,
        human_empire: n("en") as u8,
        human_controlled: std::array::from_fn(|e| s["ki"][e] == 1),
        race: s["faj"][..EMPIRES].try_into().unwrap(),
        story_bonus: n("bonus"),
        cheat: n("cheat") != 0,
        all_techs: n("all") != 0,
        players: n("pp2"),
        speed_locked: g["locked"].as_i64().unwrap_or(0) != 0,
        program_speed: g["progSpeed"].as_i64().unwrap_or(0) as i32,
    }
}

fn replay_research(path: &std::path::Path, rules: &ResearchRules) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut state = Regions::new();
    let mut bad = Vec::new();
    let (mut ticks, mut done, mut started, mut spent, mut auto) = (0, 0, 0, 0, 0);
    for line in text.lines() {
        let ev: Value = serde_json::from_str(line).unwrap();
        match ev["k"].as_str().unwrap() {
            "init" => {
                for (n, h) in ev["regions"].as_object().unwrap() {
                    state.insert(n.clone(), unhex(h.as_str().unwrap()));
                }
                continue;
            }
            "res" => {}
            _ => continue,
        }
        apply(&mut state, &ev["pre"]);
        let mut ctx = research_context(&ev["g"], &state);
        let mut techs = TechTable { bytes: state["techs"].clone() };
        let mut res = Research {
            current: std::array::from_fn(|e| i32_at(&state["raf"], 4 * e)),
            urgent: state["urgent"][..EMPIRES].try_into().unwrap(),
        };
        let labs: [[i32; 4]; EMPIRES] = std::array::from_fn(|e| std::array::from_fn(|i| i32_at(&state["labs"], 16 * e + 4 * i)));
        let mut money = treasuries(&state);
        let money0 = money;
        let events = research::tick(&mut techs, &mut res, &labs, &mut money, rules, &mut ctx);
        ticks += 1;
        spent += (1..EMPIRES).filter(|&e| money[e].pools[3] != money0[e].pools[3]).count();
        for e in &events {
            match e {
                ResearchEvent::Completed { .. } => done += 1,
                ResearchEvent::Started { .. } => started += 1,
                ResearchEvent::AutoDesign { .. } => auto += 1,
            }
        }
        let mut ours = state.clone();
        ours.insert("techs".into(), techs.bytes);
        for e in 0..EMPIRES {
            put(ours.get_mut("raf").unwrap(), 4 * e, &res.current[e].to_le_bytes());
        }
        put_treasuries(&mut ours, &money);
        apply(&mut state, &ev["post"]);
        let seq = ev["seq"].as_i64().unwrap();
        compare(&ours, &state, &["techs", "raf", "urgent", "labs", "money", "pools", "hatar", "szorzo"], &format!("res {seq}"), &mut bad);
    }
    eprintln!("{}: {ticks} research ticks, {spent} payments, {done} completed, {started} AI picks, {auto} auto designs",
        path.file_name().unwrap().to_string_lossy());
    assert!(bad.is_empty(), "{} mismatches in {}:\n{}", bad.len(), path.display(), bad[..bad.len().min(20)].join("\n"));
    assert!(ticks > 0);
}

// ------------------------------------------------------------------------------------------------
// Production

#[test]
fn production_matches_every_traced_tick() {
    let paths = traces("prod-production");
    let Some(inst) = install() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    if paths.is_empty() {
        eprintln!("no production trace in _local/traces; run tools/trace/run_prodres.py --mode production");
        return;
    }
    let r = ig2_rules::Rules::from_install(&inst).expect("catalogued exe");
    for p in paths {
        replay_production(&p, &r);
    }
}

const PLANET: usize = 0x188;
const FLEET: usize = 0x80;

fn yard_planet(r: &[u8]) -> YardPlanet {
    YardPlanet {
        owner: r[0x20],
        x: f32_at(r, 0x18),
        y: f32_at(r, 0x1c),
        halted: r[0x64],
        capacity: [i32_at(r, 0x70), i32_at(r, 0x74)],
        queue: Queue::from_bytes(&r[0x78..0xb8]),
        garrison: i32_at(r, 0x58),
        garrison_strength: i32_at(r, 0x5c),
        stations: std::array::from_fn(|k| i16_at(r, 0x100 + 2 * k)),
        station_count: i32_at(r, 0x114),
    }
}
fn put_planet(r: &mut [u8], p: &YardPlanet) {
    put(r, 0x78, &p.queue.to_bytes());
    put(r, 0x58, &p.garrison.to_le_bytes());
    put(r, 0x5c, &p.garrison_strength.to_le_bytes());
    for k in 0..STATION_SLOTS {
        put(r, 0x100 + 2 * k, &p.stations[k].to_le_bytes());
    }
    put(r, 0x114, &p.station_count.to_le_bytes());
}

fn yard_fleet(r: &[u8]) -> YardFleet {
    YardFleet {
        fleet: Fleet {
            name: r[..21].try_into().unwrap(),
            owner: r[0x15],
            story_id: i16_at(r, 0x16),
            state: r[0x18],
            visible: r[0x19..0x19 + 19].try_into().unwrap(),
            x: f32_at(r, 0x30),
            y: f32_at(r, 0x34),
            speed: f32_at(r, 0x38),
            speed_override: f32_at(r, 0x3c),
            order: i16_at(r, 0x40),
            kind: i16_at(r, 0x42),
            tx: f32_at(r, 0x44),
            ty: f32_at(r, 0x48),
            colony_ships: i16_at(r, 0x4c),
            terraformers: i16_at(r, 0x4e),
            first_waypoint: i16_at(r, 0x50),
            flags: u16::from_le_bytes([r[0x52], r[0x53]]),
            ship_count: i32_at(r, 0x54),
            strength: i32_at(r, 0x60),
            radar: f32_at(r, 0x7c),
        },
        totals: FleetTotals::from_record(r),
    }
}
fn put_fleet(r: &mut [u8], y: &YardFleet) {
    let f = &y.fleet;
    put(r, 0, &f.name);
    r[0x15] = f.owner;
    put(r, 0x16, &f.story_id.to_le_bytes());
    r[0x18] = f.state;
    put(r, 0x19, &f.visible);
    for (o, v) in [(0x30, f.x), (0x34, f.y), (0x38, f.speed), (0x3c, f.speed_override), (0x44, f.tx), (0x48, f.ty), (0x7c, f.radar)] {
        put(r, o, &v.to_le_bytes());
    }
    for (o, v) in [(0x40, f.order), (0x42, f.kind), (0x4c, f.colony_ships), (0x4e, f.terraformers), (0x50, f.first_waypoint)] {
        put(r, o, &v.to_le_bytes());
    }
    put(r, 0x52, &f.flags.to_le_bytes());
    let t = &y.totals;
    for (o, v) in [(0x54, t.ships), (0x58, t.fighters), (0x5c, t.ground), (0x60, t.attack), (0x64, t.fighter_attack),
        (0x68, t.ground_strength), (0x6c, t.hp), (0x70, t.fighter_hp), (0x78, t.capacity)] {
        put(r, o, &v.to_le_bytes());
    }
    assert_eq!((f.ship_count, f.strength), (t.ships, t.attack), "mirrored sums");
}

fn i32s<const N: usize>(b: &[u8]) -> [i32; N] {
    std::array::from_fn(|i| i32_at(b, 4 * i))
}

fn replay_production(path: &std::path::Path, r: &ig2_rules::Rules) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut state = Regions::new();
    let mut bad = Vec::new();
    let (mut ticks, mut built, mut fleets_made, mut ships_made, mut garrisons, mut paid, mut deleted) = (0, 0, 0, 0, 0, 0, 0);
    for line in text.lines() {
        let ev: Value = serde_json::from_str(line).unwrap();
        match ev["k"].as_str().unwrap() {
            "init" => {
                for (n, h) in ev["regions"].as_object().unwrap() {
                    state.insert(n.clone(), unhex(h.as_str().unwrap()));
                }
                continue;
            }
            "prod" => {}
            _ => continue,
        }
        apply(&mut state, &ev["pre"]);
        let g = &ev["g"];
        let n = |k: &str| g[k].as_i64().unwrap() as i32;
        let diff = n("diff") as usize;
        let name = g["fleetName"].as_str().unwrap().as_bytes().to_vec();
        let ctx = ProductionContext {
            speed_mult: n("speed"),
            day: n("nap"),
            multiplayer: n("mp") != 0,
            human_controlled: std::array::from_fn(|e| state["ki"][e] == 1),
            difficulty_production: r.difficulty_production_mult[diff],
            difficulty_money: r.difficulty_money_mult[diff],
            default_fleet_name: name.clone(),
            numbered_fleet_name: name,
        };
        let pl = &state["planets"];
        let fl = &state["fleets"];
        let mut yard = Shipyard {
            planets: pl.chunks(PLANET).map(yard_planet).collect(),
            fleets: fl.chunks(FLEET).map(yard_fleet).collect(),
            ships: state["ships"].chunks(6).map(|c| Ship { fleet: i16_at(c, 0), design: i16_at(c, 2) }).collect(),
            vehicles: state["tanks"].chunks(8).take(n("ntanks") as usize + 1)
                .map(|c| Vehicle { fleet: i16_at(c, 0), design: i16_at(c, 2), count: i32_at(c, 4) }).collect(),
            designs: (0..EMPIRES).map(|e| state.get(&format!("d{e}")).map_or(Vec::new(), |b| b.chunks(0x84).map(Design::from_bytes).collect())).collect(),
            last_fleet: i32s::<201>(&state["last"]).to_vec(),
            fleet_numbers: i32s(&state["names"]),
            colony_ship_pool: i32s(&state["cpool"]),
            terraformer_pool: i32s(&state["tpool"]),
        };
        let nf0 = yard.fleets.len();
        let ns0 = yard.ships.len();
        let mut money = treasuries(&state);
        let money0 = money;
        let events = yard.tick(&mut money, &ctx);
        ticks += 1;
        paid += (1..EMPIRES).filter(|&e| money[e].pools[1] != money0[e].pools[1]).count();
        let seq = ev["seq"].as_i64().unwrap();
        let before = state.clone();
        apply(&mut state, &ev["post"]);
        // Write the port's result into the "before" records.
        let mut ours = before;
        put_treasuries(&mut ours, &money);
        let planets = ours.get_mut("planets").unwrap();
        for (p, y) in yard.planets.iter().enumerate() {
            put_planet(&mut planets[p * PLANET..(p + 1) * PLANET], y);
        }
        let mut fleets = vec![0u8; yard.fleets.len() * FLEET];
        let theirs = &state["fleets"];
        for (i, y) in yard.fleets.iter().enumerate() {
            let rec = &mut fleets[i * FLEET..(i + 1) * FLEET];
            if i < nf0 {
                rec.copy_from_slice(&ours["fleets"][i * FLEET..(i + 1) * FLEET]);
            } else if let Some(t) = theirs.get(i * FLEET..(i + 1) * FLEET) {
                rec.copy_from_slice(t); // a slot past the old end: unwritten bytes are whatever memory held
            }
            put_fleet(rec, y);
        }
        let created: Vec<usize> = events.iter().filter_map(|e| if let ProductionEvent::FleetCreated { fleet } = e { Some(*fleet) } else { None }).collect();
        for &f in &created {
            let rec = &mut fleets[f * FLEET..(f + 1) * FLEET];
            put(rec, 0x2c, &[0, 0]);
            put(rec, 0x74, &[0; 4]);
            if f >= nf0 {
                // Never written by FlottaNew: the name bytes after its NUL (strcpy), +0x18 bits 2-7, visibility of
                // empire 0, +0x2e/+0x2f, flag bit 15.
                let t = &theirs[f * FLEET..(f + 1) * FLEET];
                let end = rec[..21].iter().position(|&c| c == 0).map_or(21, |i| i + 1);
                rec[end..21].copy_from_slice(&t[end..21]);
                rec[0x18] = (rec[0x18] & 3) | (t[0x18] & !3);
                rec[0x19] = t[0x19];
                put(rec, 0x2e, &t[0x2e..0x30]);
                rec[0x53] = (rec[0x53] & 0x7f) | (t[0x53] & 0x80);
            }
        }
        ours.insert("fleets".into(), fleets);
        let mut ships = vec![0u8; yard.ships.len() * 6];
        for (i, s) in yard.ships.iter().enumerate() {
            let rec = &mut ships[i * 6..(i + 1) * 6];
            if i < ns0 {
                rec.copy_from_slice(&ours["ships"][i * 6..(i + 1) * 6]);
            } else if let Some(t) = state["ships"].get(i * 6..(i + 1) * 6) {
                rec.copy_from_slice(t);
            }
            put(rec, 0, &s.fleet.to_le_bytes());
            put(rec, 2, &s.design.to_le_bytes());
        }
        for e in &events {
            if let ProductionEvent::ShipCreated { ship } = e {
                ships[ship * 6 + 4] = 0;
            }
        }
        ours.insert("ships".into(), ships);
        let tanks = ours.get_mut("tanks").unwrap();
        tanks.resize(state["tanks"].len().max(yard.vehicles.len() * 8), 0);
        for (i, v) in yard.vehicles.iter().enumerate() {
            put(tanks, i * 8, &v.fleet.to_le_bytes());
            put(tanks, i * 8 + 2, &v.design.to_le_bytes());
            put(tanks, i * 8 + 4, &v.count.to_le_bytes());
        }
        tanks.truncate(state["tanks"].len());
        let put_i32s = |s: &mut Regions, k: &str, v: &[i32]| {
            let b = s.get_mut(k).unwrap();
            for (i, x) in v.iter().enumerate() {
                put(b, 4 * i, &x.to_le_bytes());
            }
        };
        put_i32s(&mut ours, "last", &yard.last_fleet);
        put_i32s(&mut ours, "names", &yard.fleet_numbers);
        put_i32s(&mut ours, "cpool", &yard.colony_ship_pool);
        put_i32s(&mut ours, "tpool", &yard.terraformer_pool);
        let mut names = vec!["money", "pools", "hatar", "szorzo", "planets", "fleets", "ships", "tanks", "last", "names", "cpool", "tpool"];
        let dn: Vec<String> = (1..EMPIRES).map(|e| format!("d{e}")).filter(|k| ours.contains_key(k)).collect();
        names.extend(dn.iter().map(String::as_str));
        compare(&ours, &state, &names, &format!("prod {seq}"), &mut bad);
        for e in &events {
            match e {
                ProductionEvent::Built { .. } => built += 1,
                ProductionEvent::FleetCreated { .. } => fleets_made += 1,
                ProductionEvent::ShipCreated { .. } => ships_made += 1,
                ProductionEvent::GarrisonPlaced { .. } => garrisons += 1,
                ProductionEvent::FleetDeleted { .. } => deleted += 1,
                _ => {}
            }
        }
    }
    eprintln!("{}: {ticks} production ticks, {paid} payments, {built} units, {fleets_made} new fleets, {ships_made} ships, {garrisons} garrisons, {deleted} deletions",
        path.file_name().unwrap().to_string_lossy());
    assert!(bad.is_empty(), "{} mismatches in {}:\n{}", bad.len(), path.display(), bad[..bad.len().min(20)].join("\n"));
    assert!(ticks > 0);
}
