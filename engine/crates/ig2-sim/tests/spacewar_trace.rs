//! Replays space-combat calls recorded from the running GOG `ig2.exe` (tools/trace/run_spacewar.py):
//! - `design`: `TervUjraSzamol` before/after records (plain and fuzzed hulls, pods, races, prices);
//! - `auto`: `SimulateSpBattle` before/after ships, vehicles, fleets, planet defences and RNG streams
//!   (plain and with reshuffled compositions); the port must reproduce every deletion, the winner,
//!   the loser's retreat, the fleet sums and stream 7;
//! - `indit`: `SpaceWarIndit`'s choice between the auto-resolve and the real-time battle;
//! - `mark`: `FRISSIT_Starmap` ticks with frozen real-time battles: the in-battle bit of every fleet.
//!
//! Rules come from the user's exe (`IG2_INSTALL_DIR`); traces from `_local/traces/spacewar*.jsonl`
//! (game state: never committed). Skips without them.

use std::collections::BTreeSet;
use std::io::BufRead;
use std::path::PathBuf;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_rules::Rules;
use ig2_sim::rng::Streams;
use ig2_sim::spacewar::*;
use serde_json::Value;

fn rules() -> Option<Rules> {
    let inst = Install::from_env(Lang::En)?.expect("valid install");
    Some(Rules::from_install(&inst).expect("catalogued exe"))
}

fn traces(prefix: &str) -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces");
    let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
    let mut v: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix))
        .collect();
    v.sort();
    v
}

fn events(prefix: &str, kind: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for p in traces(prefix) {
        for line in std::io::BufReader::new(std::fs::File::open(p).unwrap()).lines() {
            let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
            if v["k"] == kind {
                out.push(v);
            }
        }
    }
    out
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

fn design_rules(r: &Rules) -> DesignRules {
    DesignRules {
        hulls: r.hulls.iter().map(|h| Hull { class: h.class, flags: h.flags, tech: h.tech }).collect(),
        tech_price: r.techs.iter().map(|t| t.price).collect(),
        tech_time: r.techs.iter().map(|t| t.build_time).collect(),
        tech_hp: r.techs.iter().map(|t| t.hp).collect(),
        tech_capacity: r.techs.iter().map(|t| t.capacity).collect(),
        firepower: r.techs.iter().map(|t| t.firepower).collect(),
        race_pod_table: r.race_pod_table.clone(),
        pod_slot_types: r.pod_slots.iter().map(|t| t.iter().map(|h| h.iter().map(|s| s.pod_type).collect()).collect()).collect(),
        tank_speeds: r.tank_speeds.clone(),
    }
}

#[test]
fn design_recompute_matches_trace() {
    let Some(r) = rules() else { return };
    let ev = events("spacewar-design", "design");
    if ev.is_empty() {
        return;
    }
    let dr = design_rules(&r);
    let mut bad = 0;
    for e in &ev {
        let mut d = Design::from_bytes(&unhex(e["before"].as_str().unwrap()));
        d.recompute(&dr, e["race"].as_u64().unwrap() as u8, e["flag"].as_i64().unwrap() != 0);
        let after = unhex(e["after"].as_str().unwrap());
        if d.to_bytes()[..] != after[..] {
            bad += 1;
            if bad <= 5 {
                let got = d.to_bytes();
                let diff: Vec<usize> = (0..0x84).filter(|&i| got[i] != after[i]).collect();
                eprintln!("empire {} design {} race {} flag {}: bytes {diff:x?} differ", e["e"], e["d"], e["race"], e["flag"]);
            }
        }
    }
    eprintln!("design: {} recomputes, {bad} mismatches", ev.len());
    assert_eq!(bad, 0);
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

fn ships_of(s: &Value) -> Vec<Ship> {
    unhex(s["ships"].as_str().unwrap()).chunks(6).map(|c| Ship { fleet: i16_at(c, 0), design: i16_at(c, 2) }).collect()
}
fn vehicles_of(s: &Value) -> Vec<Vehicle> {
    unhex(s["tanks"].as_str().unwrap()).chunks(8).map(|c| Vehicle { fleet: i16_at(c, 0), design: i16_at(c, 2), count: i32_at(c, 4) }).collect()
}

/// (planet, finished gun slots) of a snapshot.
fn planet_of(s: &Value) -> Option<(BattlePlanet, usize)> {
    let p = s.get("planet").filter(|p| !p.is_null())?;
    let raw = unhex(p["raw"].as_str().unwrap());
    let n = i32_at(&raw, 0x114).max(0) as usize;
    let ships = (1..=n).map(|k| i16_at(&raw, 0xfe + 2 * k)).collect();
    let nb = p["nb"].as_i64().unwrap().max(0) as usize;
    let b = unhex(p["buildings"].as_str().unwrap());
    let buildings: Vec<DefenceSlot> = (1..=nb)
        .map(|k| DefenceSlot { slot: k, kind: b[0x20 * k], finished: i16_at(&b, 0x20 * k + 8) == 0 })
        .collect();
    let guns = buildings.iter().filter(|b| b.finished && (28..32).contains(&b.kind)).count();
    Some((BattlePlanet { index: p["i"].as_u64().unwrap() as usize, owner: raw[0x20], ships, buildings }, guns))
}

#[test]
fn auto_resolve_matches_trace() {
    let Some(r) = rules() else { return };
    let ev = events("spacewar-auto", "auto");
    if ev.is_empty() {
        return;
    }
    let guns = GunRules {
        building_tech: r.buildings.iter().map(|b| b.tech).collect(),
        firepower: r.techs.iter().map(|t| t.firepower).collect(),
        tech_hp: r.techs.iter().map(|t| t.hp).collect(),
    };
    let (mut bad, mut kills, mut stale, mut planet_battles, mut wins2, mut deleted) = (0, 0, 0, 0, 0, 0);
    for (n, e) in ev.iter().enumerate() {
        let b = &e["before"];
        let a = &e["after"];
        let f1 = e["f1"].as_u64().unwrap() as usize;
        let f2 = e["f2"].as_u64().unwrap() as usize;
        let nf = b["nfleet"].as_u64().unwrap() as usize;
        let mut w = CombatWorld { fleets: vec![CombatFleet { owner: 0xff, ..Default::default() }; nf + 1], ..Default::default() };
        w.fleets[0] = fleet_of(&unhex(b["fleet0"].as_str().unwrap()));
        w.fleets[f1] = fleet_of(&unhex(b["fleet1"].as_str().unwrap()));
        if f2 != 0 {
            w.fleets[f2] = fleet_of(&unhex(b["fleet2"].as_str().unwrap()));
        }
        w.ships = ships_of(b);
        w.vehicles = vehicles_of(b);
        w.designs = vec![Vec::new(); 19];
        for (k, v) in b["designs"].as_object().unwrap() {
            w.designs[k.parse::<usize>().unwrap()] = unhex(v.as_str().unwrap()).chunks(0x84).map(Design::from_bytes).collect();
        }
        let ki = unhex(b["playerKi"].as_str().unwrap());
        for (h, &k) in w.human_controlled.iter_mut().zip(&ki) {
            *h = k == 1;
        }
        let cx = unhex(b["centreX"].as_str().unwrap());
        let cy = unhex(b["centreY"].as_str().unwrap());
        let centres: Vec<(i32, i32)> = (0..19).map(|i| (i32_at(&cx, 4 * i), i32_at(&cy, 4 * i))).collect();
        let mut rng = Streams::default();
        for (i, s) in b["seeds"].as_array().unwrap().iter().enumerate() {
            rng.set_state(i, s.as_u64().unwrap() as u32);
        }
        let mut planet = planet_of(b);
        let o1 = w.fleets[f1].owner;
        let o2 = if f2 != 0 { w.fleets[f2].owner } else { planet.as_ref().map_or(0, |p| p.0.owner) };
        let out = w.simulate(f1, f2, planet.as_mut().map(|p| &mut p.0), &guns, &centres, &mut rng);

        let mut errs = Vec::new();
        let winner_empire = if out.winner == 1 { o1 } else { o2 } as i64;
        if winner_empire != e["winner"].as_i64().unwrap() {
            errs.push(format!("winner {winner_empire} vs {}", e["winner"]));
        }
        let seeds: Vec<u32> = a["seeds"].as_array().unwrap().iter().map(|s| s.as_u64().unwrap() as u32).collect();
        if (0..21).any(|i| rng.state(i) != seeds[i]) {
            errs.push("rng".into());
        }
        if w.ships != ships_of(a) {
            errs.push("ships".into());
        }
        let va = vehicles_of(a);
        if w.vehicles[..] != va[..w.vehicles.len().min(va.len())] {
            errs.push("vehicles".into());
        }
        let mut cmp = |label: &str, f: usize, raw: &str| {
            let want = fleet_of(&unhex(raw));
            let got = w.fleets[f];
            if got.owner != want.owner || got.x != want.x || got.y != want.y || got.flags != want.flags {
                errs.push(format!("{label} owner/pos/flags {:?} vs {:?}", (got.owner, got.x, got.y, got.flags), (want.owner, want.x, want.y, want.flags)));
            }
            if want.owner != 0xff && (got.totals != want.totals || got.colony_ships != want.colony_ships || got.terraformers != want.terraformers) {
                errs.push(format!("{label} totals {:?} vs {:?}", got.totals, want.totals));
            }
        };
        cmp("fleet1", f1, a["fleet1"].as_str().unwrap());
        if f2 != 0 {
            cmp("fleet2", f2, a["fleet2"].as_str().unwrap());
        }
        let f0 = fleet_of(&unhex(a["fleet0"].as_str().unwrap()));
        if (w.fleets[0].x, w.fleets[0].y) != (f0.x, f0.y) {
            errs.push("fleet0 position".into());
        }
        if let (Some((p, guns_before)), Some((pa, guns_after))) = (planet.as_ref(), planet_of(a)) {
            planet_battles += 1;
            if p.ships != pa.ships {
                errs.push(format!("planet ships {:?} vs {:?}", p.ships, pa.ships));
            }
            let destroyed = w.events.iter().filter(|x| matches!(x, CombatEvent::BuildingDestroyed { .. })).count();
            if guns_before - destroyed != guns_after {
                errs.push(format!("guns {guns_before}-{destroyed} vs {guns_after}"));
            }
        }
        kills += out.sides.iter().flatten().filter(|u| u.hp < 1).count();
        stale += out.stalemate as usize;
        wins2 += (out.winner == 2) as usize;
        deleted += w.events.iter().filter(|x| matches!(x, CombatEvent::FleetDeleted { .. })).count();
        if !errs.is_empty() {
            bad += 1;
            if bad <= 8 {
                eprintln!("battle #{n} ({f1} vs {f2}, planet {}): {errs:?}", e["pl"]);
            }
        }
    }
    eprintln!("auto: {} battles ({planet_battles} with a planet), {kills} units destroyed, {wins2} defender wins, {stale} stalemates, {deleted} fleet deletions, {bad} mismatches", ev.len());
    assert_eq!(bad, 0);
}

#[test]
fn routing_matches_trace() {
    let ev = events("spacewar-indit", "indit");
    if ev.is_empty() {
        return;
    }
    let (mut bad, mut tactical, mut lopsided) = (0, 0, 0);
    for (n, e) in ev.iter().enumerate() {
        let b = &e["before"];
        let rec = |v: &Value| {
            let raw = unhex(v["raw"].as_str().unwrap());
            (Contender { owner: raw[0x15], story_id: i16_at(&raw, 0x16), attack: i32_at(&raw, 0x60), hp: i32_at(&raw, 0x6c) }, f32_at(&raw, 0x30), f32_at(&raw, 0x34))
        };
        let (c1, x, y) = rec(&b["f1"]);
        let mut fleets = vec![c1];
        if !b["f2"].is_null() {
            fleets.push(rec(&b["f2"]).0);
        }
        let planets = b["planets"].as_array().unwrap();
        let pl = e["pl"].as_u64().unwrap() as usize;
        // spKozeliBolygo: the battle planet, or the nearest planet (Manhattan < 20, fleet-movement §5)
        // when it belongs to one of the fleets' owners.
        let near = if pl > 0 {
            pl
        } else {
            let mut best = 20.0f32;
            let mut pick = 0;
            for (i, p) in planets.iter().enumerate() {
                let d = (p[0].as_f64().unwrap() as f32 - x).abs() + (p[1].as_f64().unwrap() as f32 - y).abs();
                if d < best {
                    best = d;
                    pick = i + 1;
                }
            }
            let o = |i: usize| planets[i - 1][2].as_u64().unwrap() as u8;
            if pick > 0 && (o(pick) == fleets[0].owner || fleets.get(1).is_some_and(|f| f.owner == o(pick))) { pick } else { 0 }
        };
        if near as u64 != e["near"].as_u64().unwrap() {
            bad += 1;
            eprintln!("indit #{n}: battle planet {near} vs {}", e["near"]);
            continue;
        }
        let ki = unhex(b["playerKi"].as_str().unwrap());
        let mut inp = RouteInput {
            fleets,
            planet_owner: if near > 0 { planets[near - 1][2].as_u64().unwrap() as u8 } else { 0 },
            ion_gun: if near > 0 { planets[near - 1][3].as_i64().unwrap() as i32 } else { 0 },
            multiplayer: b["mp"].as_u64().unwrap() != 0,
            mp_battle_status: b["mpStatus"].as_i64().unwrap() as i32,
            debug_always_simulate: b["dbgSim"].as_i64().unwrap() != 0,
            debug_never_lopsided: b["dbgNoLop"].as_i64().unwrap() != 0,
            // Story_SpaceBattleStarted clears the story switches inside SpaceWarIndit and lets the
            // campaign script set them per battle; quick.scn has no script, so the effective value is 0
            // whatever the fuzzer wrote before the call (spec §4 step 2).
            story_simulate_ban: false,
            ..Default::default()
        };
        for (h, &k) in inp.human_controlled.iter_mut().zip(&ki) {
            *h = k == 1;
        }
        let got = route(&inp);
        let want_tac = e["tactical"].as_bool().unwrap();
        let want_sim = e["sim"].as_bool().unwrap();
        let ok = match got {
            Route::Tactical => want_tac && !want_sim,
            Route::Simulate { lopsided: l } => {
                lopsided += l as usize;
                want_sim && !want_tac
            }
        };
        tactical += want_tac as usize;
        if !ok {
            bad += 1;
            if bad <= 8 {
                eprintln!("indit #{n}: port {got:?}, exe sim {want_sim} tactical {want_tac}; ratio {}", strength_ratio(&inp.fleets, inp.ion_gun));
            }
        }
    }
    eprintln!("indit: {} calls, {tactical} tactical, {lopsided} lopsided auto-resolves, {bad} mismatches", ev.len());
    assert_eq!(bad, 0);
}

#[test]
fn in_battle_marking_matches_trace() {
    let ev = events("spacewar-indit", "mark");
    if ev.is_empty() {
        return;
    }
    let (mut bad, mut marked) = (0, 0);
    for (n, e) in ev.iter().enumerate() {
        let b = &e["before"];
        let ships: Vec<Ship> = unhex(b["ships"].as_str().unwrap()).chunks(6).map(|c| Ship { fleet: i16_at(c, 0), design: i16_at(c, 2) }).collect();
        let battles: Vec<ActiveBattle> = b["battles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|bt| ActiveBattle {
                units: bt["units"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|u| BattleUnit {
                        alive: u["alive"].as_u64().unwrap() as u8,
                        owner: u["owner"].as_i64().unwrap() as i32,
                        ships: u["ships"].as_array().unwrap().iter().map(|s| s.as_i64().unwrap() as i32).collect(),
                    })
                    .collect(),
            })
            .collect();
        let fl = |s: &Value| -> Vec<(u8, u16)> {
            s["flags"].as_array().unwrap().iter().map(|p| (p[0].as_u64().unwrap() as u8, p[1].as_u64().unwrap() as u16)).collect()
        };
        let before = fl(b);
        let after = fl(&e["after"]);
        let refresh = b["ido2"].as_i64().unwrap() % 10 == 1;
        let set: BTreeSet<usize> = fleets_in_battle(&battles, &ships).into_iter().collect();
        marked += set.len();
        // Bit 0 after the tick: marked fleets have it; on a refresh tick every live fleet 1..=n was
        // cleared first (fleet-movement §6.3 step 2; slot 0 isn't), otherwise it is sticky.
        for (i, (&(o, fb), &(_, fa))) in before.iter().zip(&after).enumerate() {
            let want = if set.contains(&i) { true } else if refresh && i >= 1 && o != 0xff { false } else { fb & 1 != 0 };
            if want != (fa & 1 != 0) {
                bad += 1;
                if bad <= 8 {
                    eprintln!("tick #{n} fleet {i}: port {want}, exe {} (refresh {refresh}, owner {o})", fa & 1);
                }
            }
        }
    }
    eprintln!("mark: {} ticks, {marked} fleet markings, {bad} mismatches", ev.len());
    assert_eq!(bad, 0);
}
