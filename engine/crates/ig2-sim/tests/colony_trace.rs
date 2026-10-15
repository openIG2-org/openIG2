//! Replays the colony economy recorded from the running GOG `ig2.exe` (tools/trace/run_colony.py).
//! Each recorded `FRISSIT_Planet(true, 0)` call has a before/after snapshot of every owned planet,
//! its buildings, the RNG streams and the economy globals; the port must turn `before` into `after`.
//!
//! Traces replayed: `_local/traces/colony.jsonl` (quick.scn as played) and `colony-fuzz.jsonl`
//! (`--fuzz`: live state randomised before each call, for branch coverage), or the single file in
//! `IG2_COLONY_TRACE`. They hold game state, so they stay in `_local/` (never committed). The rule
//! tables come from the user's exe. The test skips unless `IG2_INSTALL_DIR` and a trace exist.

use std::path::PathBuf;

use ig2_formats::Install;
use ig2_formats::text::Lang;
use ig2_sim::colony::{self, Building, BuildingKind, ColonyRules, Curve, EconomyContext, Planet, StoryHooks};
use ig2_sim::rng::Streams;
use serde_json::Value;

fn trace_paths() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("IG2_COLONY_TRACE") {
        return vec![PathBuf::from(p)];
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces");
    ["colony.jsonl", "colony-fuzz.jsonl"].iter().map(|f| dir.join(f)).filter(|p| p.exists()).collect()
}

fn colony_rules(r: &ig2_rules::Rules) -> ColonyRules {
    ColonyRules {
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
        curves: r
            .problem_curves
            .iter()
            .map(|c| Curve { a: c.a, b: c.b, c: c.c, d: c.d, rate: c.rate, weight: c.weight })
            .collect(),
        surface_growth: r.race_surface_affinity.iter().map(|v| v[..9].try_into().unwrap()).collect(),
        self_destruct_fear: r.race_fear_threshold.clone(),
        kidnap_morale: r.kidnap_morale.clone(),
        race_production: r.race_production_mult.clone(),
    }
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

/// Decode a 0x188-byte `BolygoTipus` and its building slots (re/rules/colony-economy.md §2).
fn planet(raw: &str, bld: &str, unrest: i32) -> Planet {
    let r = unhex(raw);
    let b = unhex(bld);
    let mut p = Planet {
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
        protected: r[0x64] != 0, // the exe tests the whole byte
        autobuild_counter: i32_at(&r, 0x68),
        ship_capacity: i32_at(&r, 0x70),
        vehicle_capacity: i32_at(&r, 0x74),
        epidemic: i16_at(&r, 0xb8),
        morale_penalty: i32_at(&r, 0x174),
        morale_penalty_rate: i32_at(&r, 0x178),
        loyal: r[0x186] != 0,
        unrest,
        ..Default::default()
    };
    p.spies.copy_from_slice(&r[0xc4..0xc4 + 19]);
    for k in 1..colony::problem::COUNT {
        p.problems[k] = i32_at(&r, 0x118 + 4 * k);
    }
    for k in 0..5 {
        p.events[k] = i16_at(&r, 0x16a + 2 * k);
    }
    p.buildings = b
        .chunks(32)
        .map(|s| Building {
            kind: s[0],
            damage: i32_at(s, 4),
            work_left: u16::from_le_bytes([s[8], s[9]]),
            yield_pct: s[0x18],
            flags: s[0x19],
            shown_energy: i16_at(s, 0x1a),
            shown_workers: i16_at(s, 0x1c),
            efficiency: i16_at(s, 0x1e),
            ..Default::default()
        })
        .collect();
    p
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect()
}

fn context(s: &Value) -> EconomyContext {
    let ki = unhex(s["playerKi"].as_str().unwrap());
    let faj = unhex(s["playerFaj"].as_str().unwrap());
    let story = unhex(s["story"].as_str().unwrap());
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
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Load every planet of a snapshot into a 1-based vector (gaps are unowned placeholders).
fn planets(s: &Value) -> Vec<Planet> {
    let lazad = ints(&s["lazad"]);
    let list = s["planets"].as_array().unwrap();
    let n = list.iter().map(|p| p["i"].as_u64().unwrap() as usize).max().unwrap_or(0);
    let mut out = vec![Planet::default(); n + 1];
    for p in list {
        let i = p["i"].as_u64().unwrap() as usize;
        out[i] = planet(p["raw"].as_str().unwrap(), p["bld"].as_str().unwrap(), lazad[i] as i32);
    }
    out
}

#[test]
fn economy_matches_every_traced_step() {
    let Some(inst) = Install::from_env(Lang::En) else { return };
    let paths = trace_paths();
    if paths.is_empty() {
        eprintln!("no colony trace in _local/traces; run tools/trace/run_colony.py");
        return;
    }
    let rules_all = ig2_rules::Rules::from_install(&inst.expect("valid install")).expect("catalogued exe");
    let rules = colony_rules(&rules_all);
    for path in paths {
        replay(&path, &rules);
    }
}

fn replay(path: &std::path::Path, rules: &ColonyRules) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut mismatches: Vec<String> = Vec::new();
    let (mut steps, mut monthly, mut planet_checks, mut rolls, mut skipped) = (0, 0, 0, 0, 0);
    for line in text.lines() {
        let ev: Value = serde_json::from_str(line).unwrap();
        if ev["k"] != "economy" {
            continue;
        }
        let (before, after) = (&ev["before"], &ev["after"]);
        let seq = ev["seq"].as_i64().unwrap();
        steps += 1;
        monthly += (before["nap"] == 1) as i32;
        rolls += ev["rnds"].as_array().unwrap().iter().filter(|r| r.get("line").is_some()).count();

        // The live problem table equals the exe's, except entry 4's rate/weight (spec §5.3).
        let live = unhex(before["problems"].as_str().unwrap());
        for (k, c) in rules.curves.iter().enumerate() {
            let w: Vec<i32> = (0..6).map(|j| i32_at(&live, 24 * k + 4 * j)).collect();
            let skip = if k == 4 { 4 } else { 6 };
            assert_eq!(&[c.a, c.b, c.c, c.d, c.rate, c.weight][..skip], &w[..skip], "curve {k}");
        }

        let mut ctx = context(before);
        let mut rng = Streams::default();
        for (s, v) in ints(&before["seeds"]).into_iter().enumerate() {
            rng.set_state(s, v as u32);
        }
        let mut ours = planets(before);
        let (reports, totals) = colony::economy_step(&mut ours, rules, &mut ctx, &mut rng);
        let theirs = planets(after);

        let mut miss = |what: String| mismatches.push(format!("step {seq} ({}-{}): {what}", before["honap"], before["nap"]));
        // A rebellion runs BolygoFellazad (not ported; streams 1 and 8) in the middle of the planet
        // and the step continues on the rewritten planet, so from that planet on the step can't be
        // reproduced yet (spec §11). Compare only the planets before it.
        let first_rebel = (0..reports.len()).find(|&i| reports[i].events.contains(&colony::ColonyEvent::Rebellion));
        let rebelled = first_rebel.is_some();
        skipped += first_rebel.map_or(0, |r| ours[r..].iter().filter(|p| p.owner != 0).count());
        if !rebelled && rng.state(3) as i64 != ints(&after["seeds"])[3] {
            miss("stream 3 state".into());
        }
        if ctx.global_morale as i64 != after["gmoral"].as_i64().unwrap() {
            miss("global morale".into());
        }
        let needed = ints(&after["neededMoral"]);
        let unrest = ints(&after["lazad"]);
        for (i, (o, t)) in ours.iter().zip(&theirs).enumerate().skip(1) {
            // A rebellion (BolygoFellazad, not ported) rewrites the planet after our part.
            if o.owner == 0 || first_rebel.is_some_and(|r| i >= r) {
                continue;
            }
            planet_checks += 1;
            let mut o = o.clone();
            // Autobuild (not ported) may place a new building and reset the counter (spec §11).
            if t.buildings.len() > o.buildings.len() || matches!(t.autobuild_counter, 0 | -1000) {
                o.autobuild_counter = t.autobuild_counter;
            }
            let t_slots = &t.buildings[..o.buildings.len().min(t.buildings.len())];
            for (j, (ob, tb)) in o.buildings.iter().zip(t_slots).enumerate() {
                if ob != tb {
                    miss(format!("planet {i} slot {j}: ours {ob:?} game {tb:?}"));
                }
            }
            let mut tc = t.clone();
            tc.buildings = o.buildings.clone();
            if o != tc {
                miss(format!("planet {i}: ours {:?}\n  game {:?}", strip(&o), strip(&tc)));
            }
            if reports[i].needed_morale as i64 != needed[i] {
                miss(format!("planet {i} needed morale ours {} game {}", reports[i].needed_morale, needed[i]));
            }
            if o.unrest as i64 != unrest[i] {
                miss(format!("planet {i} unrest ours {} game {}", o.unrest, unrest[i]));
            }
        }
        if let Some(t) = totals.filter(|_| !rebelled) {
            let (gdp, tax) = (ints(&after["fullGdp"]), ints(&after["fullTax"]));
            for e in 1..19 {
                if t.gdp[e] as i64 != gdp[e] || t.tax[e] as i64 != tax[e] {
                    miss(format!("empire {e} totals ours {}/{} game {}/{}", t.gdp[e], t.tax[e], gdp[e], tax[e]));
                }
            }
        }
    }
    eprintln!(
        "{}: {steps} steps ({monthly} monthly), {planet_checks} planet checks, {rolls} rolls, {skipped} planets skipped after a rebellion",
        path.display()
    );
    for m in mismatches.iter().take(25) {
        eprintln!("{m}");
    }
    assert!(steps >= 30 && monthly >= 5, "trace too short");
    assert!(mismatches.is_empty(), "{}: {} mismatches", path.display(), mismatches.len());
}

/// The planet without its building list, for compact diffs.
fn strip(p: &Planet) -> Planet {
    Planet { buildings: Vec::new(), ..p.clone() }
}
