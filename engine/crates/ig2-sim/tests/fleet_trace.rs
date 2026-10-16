//! Replays fleet movement recorded from the running GOG `ig2.exe` (tools/trace/run_fleet.py):
//! every recorded `FRISSIT_Starmap` tick has before/after snapshots of all fleets, the waypoint pool,
//! wormholes, planets, relations and RNG streams; the port must turn `before` into `after`.
//!
//! The radar pass (spec §6) is computed too: fleet visibility and radar ranges, the stealth bit,
//! planet radar levels, planet visibility and story notices, who-met-whom, the per-empire radar tables
//! and the idle colony ship/terraformer pools are all compared with `after`. Only the space-war
//! in-battle bit (set after the radar block by the battle system) comes from `after`, as the list of
//! fleets in battle. Fleet aggregates built from ships (counts, strength) aren't modelled.
//!
//! Traces: `_local/traces/fleet.jsonl` and `fleet-fuzz.jsonl`, or the file in `IG2_FLEET_TRACE`.
//! They hold game state, so they stay in `_local/` (never committed). Skips without a trace.

use std::path::PathBuf;

use ig2_sim::fleet::{flag, Fleet, StarPlanet, Starmap, StarmapContext, StarmapEvent, Waypoint, Wormhole, DELETED};
use ig2_sim::radar::{RadarRules, RadarTables, ShipDesign};
use ig2_sim::rng::Streams;
use serde_json::Value;

fn trace_paths() -> Vec<PathBuf> {
    if let Some(p) = std::env::var_os("IG2_FLEET_TRACE") {
        return vec![PathBuf::from(p)];
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces");
    ["fleet.jsonl", "fleet-fuzz.jsonl", "fleet-approach.jsonl", "fleet-mpradar.jsonl"].iter().map(|f| dir.join(f)).filter(|p| p.exists()).collect()
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

fn fleet(r: &[u8]) -> Fleet {
    Fleet::from_record(r)
}

fn fleets(s: &Value) -> Vec<Fleet> {
    let n = s["nfleet"].as_u64().unwrap() as usize;
    let mut out = vec![Fleet { owner: DELETED, ..Default::default() }; n + 1];
    for f in s["fleets"].as_array().unwrap() {
        out[f["i"].as_u64().unwrap() as usize] = fleet(&unhex(f["raw"].as_str().unwrap()));
    }
    out
}

fn waypoints(s: &Value) -> Vec<Waypoint> {
    unhex(s["waypoints"].as_str().unwrap())
        .chunks(12)
        .map(|w| Waypoint { x: f32_at(w, 0), y: f32_at(w, 4), next: i16_at(w, 8) })
        .collect()
}

fn wormholes(s: &Value) -> Vec<Wormhole> {
    unhex(s["wormholes"].as_str().unwrap())
        .chunks(0x1c)
        .map(|w| Wormhole {
            ax: f32_at(w, 0),
            ay: f32_at(w, 4),
            revealed_a: w[8] != 0,
            fleet_a: i16_at(w, 10),
            bx: f32_at(w, 0xc),
            by: f32_at(w, 0x10),
            revealed_b: w[0x14] != 0,
            fleet_b: i16_at(w, 0x16),
            tint: [w[0x18], w[0x19], w[0x1a]],
        })
        .collect()
}

fn planets(v: &Value) -> Vec<StarPlanet> {
    let list = v.as_array().unwrap();
    let mut out = vec![StarPlanet::default(); list.len() + 1];
    for p in list {
        out[p["i"].as_u64().unwrap() as usize] = StarPlanet {
            x: p["x"].as_f64().unwrap() as f32,
            y: p["y"].as_f64().unwrap() as f32,
            owner: p["owner"].as_u64().unwrap() as u8,
            epidemic: p["epidemic"].as_i64().unwrap() as i16,
            trade_centres: p["centres"].as_i64().unwrap() as i32,
            banks: p["banks"].as_i64().unwrap() as i32,
            trade_ports: p["ports"].as_i64().unwrap() as i32,
            radar_level: p["radar"].as_u64().unwrap() as u8,
            has_hub: p["hub"].as_i64().unwrap() > 0,
            visible: std::array::from_fn(|e| i16_at(&unhex(p["vis"].as_str().unwrap()), 2 * e)),
            notice: p["notice"].as_u64().unwrap() as u8,
        };
    }
    out
}

fn f32s(s: &Value, key: &str) -> Vec<f32> {
    unhex(s[key].as_str().unwrap()).chunks(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()
}
fn i32s(s: &Value, key: &str) -> [i32; 19] {
    let b = unhex(s[key].as_str().unwrap());
    std::array::from_fn(|e| i32_at(&b, 4 * e))
}

/// `RadarHatotav2`, `…Negyzet`, `RadarHatotavSzorzo`, `…Negyzet` as dumped.
fn radar_tables(s: &Value) -> RadarTables {
    let (r, rs, m, ms) = (f32s(s, "rh2"), f32s(s, "rh2sq"), f32s(s, "rmul"), f32s(s, "rmulsq"));
    RadarTables {
        range: std::array::from_fn(|e| std::array::from_fn(|k| r[8 * e + k])),
        range_sq: std::array::from_fn(|e| std::array::from_fn(|k| rs[8 * e + k])),
        mult: m.try_into().unwrap(),
        mult_sq: ms.try_into().unwrap(),
    }
}

fn met(s: &Value) -> [[u8; 19]; 19] {
    let b = unhex(s["met"].as_str().unwrap());
    std::array::from_fn(|e| std::array::from_fn(|o| b[19 * e + o]))
}

/// Ships with designs, in ship order, for the speed/radar refresh.
fn ships(s: &Value, fl: &[Fleet]) -> Vec<ShipDesign> {
    let Some(raw) = s.get("ships").and_then(Value::as_str) else { return Vec::new() };
    let designs = &s["designs"];
    unhex(raw)
        .chunks(6)
        .skip(1)
        .filter_map(|h| {
            let (f, d) = (i16_at(h, 0), i16_at(h, 2));
            if d == 0x7fff || f < 1 || f as usize >= fl.len() {
                return None;
            }
            let key = format!("{}:{}", fl[f as usize].owner, d);
            let rec = unhex(designs.get(&key)?.as_str()?);
            Some(ShipDesign {
                fleet: f as usize,
                speed: f32_at(&rec, 0x3c),
                design_type: i32_at(&rec, 0x48),
                radar: rec[0x6a],
                stealth: rec[0x6b],
            })
        })
        .collect()
}

fn context(s: &Value, rel: &str, after: &Value) -> StarmapContext {
    let ki = unhex(s["playerKi"].as_str().unwrap());
    let story = unhex(s["story"].as_str().unwrap());
    // Techs 0x21..=0x24: the highest one the empire has developed (state 5).
    let techs: Vec<Vec<u8>> = s["radarTech"].as_array().unwrap().iter().map(|t| unhex(t.as_str().unwrap())).collect();
    let ranges = f32s(s, "radarRanges");
    let tut = s["tutorial"].as_array().unwrap();
    let in_battle = fleets(after).iter().enumerate().skip(1).filter(|(_, f)| f.alive() && f.flags & flag::IN_BATTLE != 0).map(|(i, _)| i).collect();
    let alive = unhex(s["alive"].as_str().unwrap());
    let faj = unhex(s["playerFaj"].as_str().unwrap());
    let r = unhex(rel);
    StarmapContext {
        tick: s["ido2"].as_i64().unwrap() as i32,
        speed_mult: s["mult"].as_i64().unwrap() as i32,
        multiplayer: s["mp"].as_i64().unwrap() != 0,
        // Older traces don't record the SetIdoLelassit inputs (and never slow down).
        speed_locked: s.get("speedLocked").and_then(Value::as_i64).unwrap_or(0) != 0,
        program_speed: s.get("programSpeed").and_then(Value::as_i64).unwrap_or(0) as i32,
        human_empire: s["en"].as_i64().unwrap() as u8,
        human_controlled: std::array::from_fn(|e| ki[e] == 1),
        alive: std::array::from_fn(|e| alive[e] != 0),
        empire_race: faj.try_into().unwrap(),
        debug: s["debug"].as_i64().unwrap() != 0,
        all_fleets_visible: s["allVisible"].as_i64().unwrap() != 0,
        debug_speed: s["debugSpeed"].as_i64().unwrap() != 0,
        // English install language: AlapFlottaNev = "Fleet" (txt17 17/5/46).
        default_fleet_name: b"Fleet".to_vec(),
        relations: (0..19).map(|a| std::array::from_fn(|b| (r[(a * 19 + b) * 2], r[(a * 19 + b) * 2 + 1]))).collect(),
        program_players: s["pp"].as_i64().unwrap() as usize,
        program_players2: s["pp2"].as_i64().unwrap() as usize,
        radar_tech: std::array::from_fn(|e| (1..=4).rev().find(|&c| techs[c - 1][e] == 5).unwrap_or(0) as u8),
        story_tracked_fleet: i32_at(&story, 0x28),
        story_radar_bonus: i32_at(&story, 0x38) != 0,
        story_no_supply: i32_at(&story, 0x48) != 0,
        story_supply_planet: i32_at(&story, 0x50),
        tutorial_sighting: (tut[0].as_i64().unwrap() != 0 || tut[1].as_i64().unwrap() != 0) && tut[2].as_i64().unwrap() == 1,
        radar_rules: RadarRules { ranges: ranges[..6].try_into().unwrap(), planet_ranges: ranges[6..].try_into().unwrap(), ..Default::default() },
        in_battle,
    }
}

/// The fields the port owns (aggregates from ships are left out).
fn owned(f: &Fleet) -> Fleet {
    Fleet { colony_ships: 0, terraformers: 0, ship_count: 0, strength: 0, ..f.clone() }
}

#[test]
fn movement_matches_every_traced_tick() {
    let paths = trace_paths();
    if paths.is_empty() {
        eprintln!("no fleet trace in _local/traces; run tools/trace/run_fleet.py");
        return;
    }
    for path in paths {
        replay(&path);
    }
}

fn replay(path: &std::path::Path) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut mismatches: Vec<String> = Vec::new();
    let (mut ticks, mut moves, mut arrivals, mut rolls, mut skipped, mut refreshes, mut built, mut paid) = (0, 0, 0, 0, 0, 0, 0, 0);
    let (mut sightings, mut planet_sightings, mut met_events, mut story_events, mut radar_fleets, mut colonisations) = (0, 0, 0, 0, 0, 0);
    let mut slowed = 0;
    let mut checked_rules = false;
    for line in text.lines() {
        let ev: Value = serde_json::from_str(line).unwrap();
        if ev["k"] != "tick" {
            continue;
        }
        let (before, after) = (&ev["before"], &ev["after"]);
        let seq = ev["seq"].as_i64().unwrap();
        ticks += 1;
        rolls += ev["rnds"].as_array().unwrap().len();
        let mut ctx = context(before, ev["relations"].as_str().unwrap(), after);
        if !checked_rules {
            check_rules_against_exe(&ctx.radar_rules);
            checked_rules = true;
        }
        let theirs = fleets(after);
        let before_fleets = fleets(before);
        let before_planets = planets(&ev["planets"]);
        let mut map = Starmap {
            fleets: before_fleets.clone(),
            waypoints: waypoints(before),
            wormholes: wormholes(before),
            planets: before_planets.clone(),
            radar: radar_tables(before),
            met: met(before),
            colony_ship_pool: i32s(before, "colonyPool"),
            terraformer_pool: i32s(before, "terraPool"),
        };
        let radar_tick = ctx.tick % 10 == 1;
        refreshes += radar_tick as i32;
        let ship_list = ships(before, &map.fleets);
        let mut rng = Streams::default();
        for (s, v) in before["seeds"].as_array().unwrap().iter().enumerate() {
            rng.set_state(s, v.as_u64().unwrap() as u32);
        }
        let events = map.tick(&ship_list, &mut ctx, &mut rng, &mut ());
        slowed += (ctx.speed_mult != before["mult"].as_i64().unwrap() as i32) as usize;
        arrivals += events.iter().filter(|e| matches!(e, StarmapEvent::Arrived { .. })).count();
        met_events += events.iter().filter(|e| matches!(e, StarmapEvent::EmpireMet { .. })).count();
        story_events += events
            .iter()
            .filter(|e| matches!(e, StarmapEvent::FleetSighted { .. } | StarmapEvent::PlanetSighted { .. } | StarmapEvent::EnemyApproaching { .. }))
            .count();
        if radar_tick {
            for (o, b) in map.fleets.iter().zip(&before_fleets).skip(1).filter(|(o, _)| o.alive()) {
                sightings += o.visible.iter().zip(&b.visible).filter(|(n, p)| n != p).count();
                radar_fleets += (o.radar > 0.0) as usize;
            }
        }

        let mut miss = |what: String| mismatches.push(format!("tick {seq} (ido2 {}): {what}", ctx.tick));
        // Battles and attacks hand over to other systems mid-tick: not reproducible here.
        if events.iter().any(|e| matches!(e, StarmapEvent::SpaceBattle { .. } | StarmapEvent::PlanetAttack { .. })) {
            skipped += 1;
            continue;
        }
        let virus: Vec<usize> = events
            .iter()
            .filter_map(|e| if let StarmapEvent::VirusSpread { fleet, .. } = e { Some(*fleet) } else { None })
            .collect();
        let game_mult = after["mult"].as_i64().unwrap() as i32;
        if ctx.speed_mult != game_mult {
            miss(format!("IdoSpeedSzorzo ours {} game {game_mult}", ctx.speed_mult));
        }
        if rng.state(14) as u64 != after["seeds"][14].as_u64().unwrap() {
            miss("stream 14 state".into());
        }
        let (mut colony, mut terra) = (map.colony_ship_pool, map.terraformer_pool);
        for (i, (o, t)) in map.fleets.iter().zip(&theirs).enumerate().skip(1) {
            if !o.alive() && !t.alive() {
                continue;
            }
            // Production (spec §7, after the movement loop) can put a new fleet into a free slot;
            // `Legyart` counts a new colony ship/terraformer into its owner's pool.
            if !o.alive() && t.alive() && !before_fleets[i].alive() {
                built += 1;
                colony[t.owner as usize] += t.colony_ships as i32;
                terra[t.owner as usize] += t.terraformers as i32;
                continue;
            }
            if o.alive() != t.alive() {
                miss(format!("fleet {i} alive ours {} game {}", o.alive(), t.alive()));
                continue;
            }
            let mut o = owned(o);
            if virus.contains(&i) {
                o.flags = (o.flags & !flag::VIRUS) | (t.flags & flag::VIRUS);
            }
            if o != owned(t) {
                miss(format!("fleet {i}:\n  ours {o:?}\n  game {:?}", owned(t)));
            }
            moves += (o.x != before_fleets[i].x || o.y != before_fleets[i].y) as usize;
        }
        // Trader income (`TradeIncomeThisYear`), when the trace records it.
        if let (Some(b), Some(a)) = (before.get("tradeIncome"), after.get("tradeIncome")) {
            let mut ours = [0i64; 19];
            for e in &events {
                if let StarmapEvent::TraderPaid { empire, amount, .. } = e {
                    ours[*empire as usize] += *amount as i64;
                    paid += 1;
                }
            }
            for (e, our) in ours.iter().enumerate() {
                let delta = a[e].as_i64().unwrap() - b[e].as_i64().unwrap();
                if delta != *our {
                    miss(format!("empire {e} trade income ours +{our} game +{delta}"));
                }
            }
        }
        let tw = waypoints(after);
        if map.waypoints != tw {
            miss(format!("waypoints ours {:?}\n  game {:?}", map.waypoints, tw));
        }
        if map.wormholes != wormholes(after) {
            miss("wormholes".into());
        }
        // Radar state (§6).
        if map.radar != radar_tables(after) {
            miss(format!("radar tables\n  ours {:?}\n  game {:?}", map.radar, radar_tables(after)));
        }
        if map.met != met(after) {
            miss("BirodalomIsmeri".into());
        }
        // Colonisation arrivals (not ported, spec §4.3) take the planet (`BolygoElfoglal`: owner, its
        // visibility 1000) and use up the colony ship: that planet and pool belong to the colony system.
        let mut colonised = Vec::new();
        for e in &events {
            match e {
                StarmapEvent::ColonyShipFreed { empire } => colony[*empire as usize] += 1,
                StarmapEvent::TerraformerFreed { empire } => terra[*empire as usize] += 1,
                StarmapEvent::Colonise { fleet, planet } => colonised.push((before_fleets[*fleet].owner as usize, *planet)),
                _ => {}
            }
        }
        colonisations += colonised.len();
        let (mut game_colony, mut game_terra) = (i32s(after, "colonyPool"), i32s(after, "terraPool"));
        for &(e, _) in &colonised {
            (game_colony[e], game_terra[e], colony[e], terra[e]) = (0, 0, 0, 0);
        }
        if colony != game_colony || terra != game_terra {
            miss(format!("pools ours {colony:?} {terra:?}"));
        }
        if let Some(pa) = ev.get("planetsAfter") {
            let tp = planets(pa);
            for (i, (o, t)) in map.planets.iter().zip(&tp).enumerate().skip(1).filter(|(i, _)| !colonised.iter().any(|c| c.1 == *i)) {
                planet_sightings += o.visible.iter().zip(&before_planets[i].visible).filter(|(n, p)| n != p).count();
                if (o.radar_level, o.visible, o.notice) != (t.radar_level, t.visible, t.notice) {
                    miss(format!("planet {i}: ours {:?} {:?} {} game {:?} {:?} {}", o.radar_level, o.visible, o.notice, t.radar_level, t.visible, t.notice));
                }
            }
        }
    }
    eprintln!(
        "{}: {ticks} ticks ({refreshes} radar refreshes), {moves} fleet moves, {arrivals} arrivals, {rolls} rolls, {skipped} battle ticks skipped, {built} fleets built, {paid} trader payments",
        path.display()
    );
    eprintln!(
        "  radar: {radar_fleets} fleet radars > 0, {sightings} fleet visibility changes, {planet_sightings} planet visibility changes, {met_events} empires met, {story_events} story/warning events ({slowed} slow-downs), {colonisations} colonisations (planet left to the colony system)"
    );
    for m in mismatches.iter().take(20) {
        eprintln!("{m}");
    }
    // `--mp-radar` recordings end at ~380 ticks (a fuzzed attack opens a battle screen, which stops strategic time).
    assert!(ticks >= 300, "trace too short");
    assert!(mismatches.is_empty(), "{}: {} mismatches", path.display(), mismatches.len());
}

/// The radar tables the game holds equal the ones `ig2-rules` reads from the exe file (when the install
/// is configured): `RadarHatotav` and `RadarHatotav2Init` aren't rewritten at runtime.
fn check_rules_against_exe(live: &RadarRules) {
    let Some(inst) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
    let rules = ig2_rules::Rules::from_install(&inst.expect("valid install")).expect("catalogued exe");
    assert_eq!(&live.ranges[..], &rules.radar_ranges[..], "RadarHatotav live vs file");
    assert_eq!(&live.planet_ranges[..], &rules.planet_radar_ranges[..], "RadarHatotav2Init live vs file");
}
