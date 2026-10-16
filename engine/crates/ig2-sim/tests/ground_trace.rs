//! Replays ground wars recorded from the running GOG `ig2.exe` (tools/trace/run_ground.py):
//! - `war`: a staged landing (`AddTanksToPlanet(f, p, 1)`) and the auto-resolved war (`WF::Simulate`,
//!   which ends in `GroundWarFinished(p, s, true)`), each from the exe's own state before the call; the
//!   port must reproduce the battlefield (every unit group's members, origins, flags, weapons, targets,
//!   countdowns), the vehicle table, the fleets' sums, the planet (owner, garrison, buildings), stream 2
//!   and the number of libc `rand()` draws (pinned with `srand` by the tracer);
//! - `indit`: `GroundWarIndit`'s route (real-time or simulated) and, for simulated wars, the outcome.
//!
//! Rules come from the user's exe (`IG2_INSTALL_DIR`); traces from `_local/traces/ground-*.jsonl`
//! (game state: never committed). Skips without them.

use std::io::BufRead;
use std::path::PathBuf;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_rules::Rules;
use ig2_sim::colony::Building;
use ig2_sim::ground::*;
use ig2_sim::rng::Streams;
use ig2_sim::spacewar::{CombatFleet, CombatWorld, Design, DesignRules, FleetTotals, Hull, Vehicle, NO_DESIGN};
use serde_json::Value;

fn rules() -> Option<Rules> {
    let inst = Install::from_env(Lang::En)?.expect("valid install");
    Some(Rules::from_install(&inst).expect("catalogued exe"))
}

fn events(kind: &str) -> Vec<Value> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces");
    let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("ground-"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for p in files {
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
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn ints(v: &Value) -> Vec<i32> {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap() as i32).collect()
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

fn ground_rules(r: &Rules) -> GroundRules {
    GroundRules {
        weapons: r.ground_weapons.clone(),
        hull_hp: r.ground_hull_hp.clone(),
        building_hp: r.buildings.iter().map(|b| b.max_hp).collect(),
        map_size: r.ground_map_size,
        designs: design_rules(r),
        rt_weapons: r.ground_rt_weapons.clone(),
        fortress: r.ground_fortress_weapons.clone(),
        turn_speeds: r.ground_turn_speeds.clone(),
        armour: r.ground_armour.clone(),
    }
}

/// A unit record (exe 0x290 bytes) with its member arrays.
fn unit(v: &Value) -> Group {
    let r = unhex(v["raw"].as_str().unwrap());
    Group {
        design: i32_at(&r, 0x15c),
        owner: i32_at(&r, 0x194),
        side: i32_at(&r, 0x198),
        max_hp: i32_at(&r, 0x158),
        hp: ints(&v["hp"]),
        origin: ints(&v["origin"]),
        flags: u32_at(&r, 0x188),
        flags2: u32_at(&r, 0x18c),
        weapons: u32_at(&r, 0x1d0),
        pods: i32_at(&r, 0x11c),
        ammo: i32_at(&r, 0x120),
        regen: i32_at(&r, 0x1cc),
        air: i32_at(&r, 0x288),
        has_gun: r[0x130] != 0,
        hull: i32_at(&r, 0x134),
        prev_size: i32_at(&r, 0x138),
        cell: (i32_at(&r, 0x1a0), i32_at(&r, 0x1a4)),
        last_cell: (i32_at(&r, 0x1a8), i32_at(&r, 0x1ac)),
        dest: (i32_at(&r, 0x1b0), i32_at(&r, 0x1b4)),
        move_state: (i32_at(&r, 0x1b8), i32_at(&r, 0x1bc)),
        pos: (f32_at(&r, 0x1d4), f32_at(&r, 0x1d8)),
        carrier: i32_at(&r, 0x1e0),
        partner: i32_at(&r, 0x1e4),
        paralysed: i32_at(&r, 0x1e8),
        seized: i32_at(&r, 0x1ec),
        building: i32_at(&r, 0x238),
        target: i32_at(&r, 0x23c),
        index: i32_at(&r, 0x244),
        last_hit: Some((i32_at(&r, 0x25c), i32_at(&r, 0x260))),
        aim: i32_at(&r, 0x27c),
        transport: u32_at(&r, 0x278),
        ..Default::default()
    }
}

fn field(s: &Value) -> Field {
    let w = unhex(s["raw"].as_str().unwrap());
    let mut f = Field {
        attack_fleet: i32_at(&w, 0x338),
        self_destruct: i32_at(&w, 0x334),
        countdown: i32_at(&w, 0x148),
        simulating: w[0x88c] != 0,
        last_status: i32_at(&w, 0x8c8),
        status_valid: i32_at(&w, 0x8cc) != 0,
        ..Default::default()
    };
    f.present.copy_from_slice(&w[0x8b1..0x8b1 + 19]);
    f.radar.copy_from_slice(&w[0x8d8..0x8d8 + 19]);
    for u in s["units"].as_array().unwrap() {
        let alive = u["alive"].as_u64().unwrap() != 0;
        f.alive.push(alive);
        f.units.push(if alive { unit(u) } else { Group::default() });
    }
    f
}

fn planet(s: &Value, index: i32) -> GroundPlanet {
    let r = unhex(s["raw"].as_str().unwrap());
    let b = unhex(s["buildings"].as_str().unwrap());
    GroundPlanet {
        index,
        owner: r[0x20],
        garrison: i32_at(&r, 0x58),
        garrison_strength: i32_at(&r, 0x5c),
        in_war: r[0x64],
        buildings: b
            .chunks(32)
            .map(|x| Building {
                kind: x[0],
                damage: i32_at(x, 4),
                work_left: u16::from_le_bytes([x[8], x[9]]),
                flags: x[0x19],
                ..Default::default()
            })
            .collect(),
        building_count: s["count"].as_i64().unwrap() as i32,
        building_z: b.chunks(32).map(|x| f32_at(x, 0x14)).collect(),
    }
}

fn vehicles(s: &Value) -> Vec<Vehicle> {
    unhex(s["tanks"].as_str().unwrap())
        .chunks(8)
        .map(|x| Vehicle { fleet: i16_at(x, 0), design: i16_at(x, 2), count: i32_at(x, 4) })
        .collect()
}

fn fleet(r: &[u8]) -> CombatFleet {
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

fn world(ev: &Value, fleets_hex: &str, tanks: &Value) -> CombatWorld {
    let g = &ev["g"];
    let mut w = CombatWorld {
        fleets: unhex(fleets_hex).chunks(0x80).map(fleet).collect(),
        vehicles: vehicles(tanks),
        ..Default::default()
    };
    w.designs = vec![Vec::new(); 19];
    for (e, d) in g["designs"].as_object().unwrap() {
        let e: usize = e.parse().unwrap();
        w.designs[e] = unhex(d.as_str().unwrap()).chunks(0x84).map(Design::from_bytes).collect();
    }
    let ki = unhex(g["playerKi"].as_str().unwrap());
    for (h, k) in w.human_controlled.iter_mut().zip(&ki) {
        *h = *k == 1;
    }
    w
}

fn context(ev: &Value) -> GroundContext {
    let g = &ev["g"];
    let bonus = ints(&g["bonus"]);
    GroundContext {
        mp_war: false,
        human: g["en"].as_i64().unwrap() as i32,
        multiplayer: g["mp"].as_u64().unwrap() != 0,
        story_planet: g["storyPlanet"].as_i64().unwrap() as i32,
        story_bonus: bonus[0].wrapping_add(bonus[1]),
        players: g["players2"].as_i64().unwrap() as i32,
        prev_planet_owner: g["prevOwner"].as_u64().unwrap() as u8,
        on_screen: false,
        player_ki: [0; 19],
    }
}

fn streams(s: &Value) -> Streams {
    let mut r = Streams::default();
    for (i, v) in s["seeds"].as_array().unwrap().iter().enumerate() {
        r.set_state(i, v.as_u64().unwrap() as u32);
    }
    r
}

/// The number of MSVC `rand()` draws between `srand(seed)` and the recorded pair `r1, r2`.
fn libc_draws(v: &Value) -> Option<usize> {
    let mut h = v["seed"].as_u64().unwrap() as u32;
    let (r1, r2) = (v["r1"].as_u64().unwrap() as u32, v["r2"].as_u64().unwrap() as u32);
    let mut prev = None;
    for k in 0..20_000_000usize {
        h = h.wrapping_mul(214013).wrapping_add(2531011);
        let x = (h >> 16) & 0x7fff;
        if prev == Some(r1) && x == r2 {
            return Some(k - 1);
        }
        prev = Some(x);
    }
    None
}

/// The unit fields the strategic port owns, as one comparable record. Positions are left out: the
/// landing places attackers at cell (100, 100) and `LocateTankSuitablePosition` (not ported) places
/// partners and split groups.
fn key(u: &Group) -> String {
    let target = if u.flags & flag::HAS_TARGET != 0 { u.target } else { NONE };
    let partner_link = if u.flags & flag::PARTNER != 0 { u.carrier } else { 0 };
    format!(
        "d{} o{} s{} max{} hp{:?} or{:?} fl{:#x} f2{:#x} w{:#x} pods{} ammo{} regen{} air{} gun{} hull{} prev{} \
         partner{} carrier{} par{} sei{} bld{} tgt{} idx{} aim{}",
        u.design, u.owner, u.side, u.max_hp, u.hp, u.origin, u.flags & !(flag::MESH | flag::MESH2), u.flags2, u.weapons,
        u.pods, u.ammo, u.regen, u.air, u.has_gun as u8, u.hull, u.prev_size, u.partner, partner_link, u.paralysed,
        u.seized, u.building, target, u.index, u.aim
    )
}

fn compare_fields(tag: &str, got: &Field, want: &Field, fails: &mut Vec<String>) {
    let n = got.units.len().max(want.units.len());
    for i in 0..n {
        let ga = got.alive.get(i).copied().unwrap_or(false);
        let wa = want.alive.get(i).copied().unwrap_or(false);
        if ga != wa {
            fails.push(format!("{tag}: unit {i} alive port {ga} exe {wa}"));
            continue;
        }
        if ga {
            let (g, w) = (key(&got.units[i]), key(&want.units[i]));
            if g != w {
                fails.push(format!("{tag}: unit {i}\n  port {g}\n  exe  {w}"));
            }
        }
    }
    let gl = (got.attack_fleet, got.self_destruct, got.countdown, got.simulating, got.last_status, got.status_valid);
    let wl = (want.attack_fleet, want.self_destruct, want.countdown, want.simulating, want.last_status, want.status_valid);
    if gl != wl {
        fails.push(format!("{tag}: field globals port {gl:?} exe {wl:?}"));
    }
    if got.present[1..] != want.present[1..] || got.radar[1..] != want.radar[1..] {
        fails.push(format!("{tag}: present/radar port {:?}/{:?} exe {:?}/{:?}", got.present, got.radar, want.present, want.radar));
    }
}

fn compare_world(tag: &str, got: &CombatWorld, want_tanks: &[Vehicle], want_fleets: Option<&[CombatFleet]>, fails: &mut Vec<String>) {
    let live = |v: &[Vehicle]| -> Vec<(usize, Vehicle)> {
        v.iter().enumerate().skip(1).filter(|(_, x)| x.design != NO_DESIGN).map(|(i, x)| (i, *x)).collect()
    };
    if live(&got.vehicles) != live(want_tanks) {
        fails.push(format!("{tag}: vehicles\n  port {:?}\n  exe  {:?}", live(&got.vehicles), live(want_tanks)));
    }
    if let Some(wf) = want_fleets {
        for (i, (g, w)) in got.fleets.iter().zip(wf).enumerate() {
            if g.owner != w.owner || g.totals != w.totals || (g.flags & 1) != (w.flags & 1) {
                fails.push(format!("{tag}: fleet {i} port {:?} {:?} {:#x} exe {:?} {:?} {:#x}", g.owner, g.totals, g.flags, w.owner, w.totals, w.flags));
            }
        }
    }
}

struct Run {
    field: Field,
    planet: GroundPlanet,
    world: CombatWorld,
    rng: Streams,
    draws: usize,
    events: Vec<GroundEvent>,
    timing: i32,
    stats: WarStats,
}

fn run(ev: &Value, gr: &GroundRules, start: &Value, fleets_hex: &str, f: impl FnOnce(&mut War)) -> Run {
    let p = ev["p"].as_i64().unwrap() as i32;
    let ctx = context(ev);
    let mut field = field(&start["field"]);
    let mut planet = planet(&start["planet"], p);
    let mut world = world(ev, fleets_hex, start);
    let mut rng = streams(start);
    let mut draws = 0usize;
    let mut libc = || {
        draws += 1;
        0
    };
    let mut place = |_s: Option<(i32, i32)>| (true, (0, 0));
    let (events, timing, stats) = {
        let mut war = War::new(&mut field, &mut planet, &mut world, gr, &ctx, &mut rng, &mut libc, &mut place);
        f(&mut war);
        (war.events.clone(), war.timing, war.stats)
    };
    Run { field, planet, world, rng, draws, events, timing, stats }
}

#[test]
fn ground_war_matches_trace() {
    let Some(r) = rules() else { return };
    let gr = ground_rules(&r);
    let evs = events("war");
    if evs.is_empty() {
        return;
    }
    let mut fails = Vec::new();
    let (mut units, mut kills, mut draws2, mut libc_total, mut captures, mut buildings_lost) = (0, 0, 0u64, 0, 0, 0);
    let mut total = WarStats::default();
    let add = |t: &mut WarStats, s: &WarStats| {
        for k in 0..14 {
            t.shots[k] += s.shots[k];
        }
        t.paralysed += s.paralysed;
        t.seized += s.seized;
        t.kamikazes += s.kamikazes;
        t.splits += s.splits;
        t.joins += s.joins;
        t.partner_hits += s.partner_hits;
        t.dodges += s.dodges;
    };
    for (n, ev) in evs.iter().enumerate() {
        let f = ev["f"].as_i64().unwrap() as i32;
        let before = &ev["before"];
        let landed = &ev["landed"];
        let after = &ev["after"];
        let tag = format!("war {n} (p {} f {f})", ev["p"]);
        // 1. The landing, from the state before it.
        let a = run(ev, &gr, before, before["fleets"].as_str().unwrap(), |w| w.land(f, true));
        let want = field(&landed["field"]);
        compare_fields(&format!("{tag} land"), &a.field, &want, &mut fails);
        let mut fl: Vec<CombatFleet> = unhex(before["fleets"].as_str().unwrap()).chunks(0x80).map(fleet).collect();
        fl[f as usize] = fleet(&unhex(landed["fleet"].as_str().unwrap()));
        compare_world(&format!("{tag} land"), &a.world, &vehicles(landed), Some(&fl), &mut fails);
        add(&mut total, &a.stats);
        let lw = libc_draws(&ev["rand1"]);
        if lw != Some(a.draws) {
            fails.push(format!("{tag} land: libc draws port {} exe {lw:?}", a.draws));
        }
        if a.rng != streams(landed) {
            fails.push(format!("{tag} land: streams moved"));
        }
        // 2. The war, from the exe's state after the landing.
        let mut fleets_hex = String::new();
        for (i, c) in unhex(before["fleets"].as_str().unwrap()).chunks(0x80).enumerate() {
            let rec = if i == f as usize { unhex(landed["fleet"].as_str().unwrap()) } else { c.to_vec() };
            fleets_hex.extend(rec.iter().map(|b| format!("{b:02x}")));
        }
        let b = run(ev, &gr, landed, &fleets_hex, |w| w.simulate());
        let want = field(&after["field"]);
        compare_fields(&format!("{tag} war"), &b.field, &want, &mut fails);
        let wf: Vec<CombatFleet> = unhex(after["fleets"].as_str().unwrap()).chunks(0x80).map(fleet).collect();
        compare_world(&format!("{tag} war"), &b.world, &vehicles(after), Some(&wf), &mut fails);
        let wp = planet(&after["planet"], ev["p"].as_i64().unwrap() as i32);
        let gp = &b.planet;
        let bk = |p: &GroundPlanet| p.buildings.iter().map(|x| (x.kind, x.damage)).collect::<Vec<_>>();
        if (gp.owner, gp.garrison, gp.garrison_strength, gp.in_war, gp.building_count, bk(gp))
            != (wp.owner, wp.garrison, wp.garrison_strength, wp.in_war, wp.building_count, bk(&wp))
        {
            fails.push(format!(
                "{tag} war: planet port {:?} exe {:?}",
                (gp.owner, gp.garrison, gp.garrison_strength, gp.in_war, gp.building_count, bk(gp)),
                (wp.owner, wp.garrison, wp.garrison_strength, wp.in_war, wp.building_count, bk(&wp))
            ));
        }
        if b.rng != streams(after) {
            fails.push(format!("{tag} war: stream 2 port {} exe {}", b.rng.state(2), streams(after).state(2)));
        }
        if b.timing != after["timing"].as_i64().unwrap() as i32 {
            fails.push(format!("{tag} war: timing port {} exe {}", b.timing, after["timing"]));
        }
        add(&mut total, &b.stats);
        let lw = libc_draws(&ev["rand2"]);
        if lw != Some(b.draws) {
            fails.push(format!("{tag} war: libc draws port {} exe {lw:?}", b.draws));
        }
        units += want.alive.len();
        kills += field(&landed["field"]).alive.iter().filter(|&&x| x).count() - want.alive.iter().filter(|&&x| x).count();
        draws2 += b.timing as u64;
        libc_total += b.draws;
        captures += b.events.iter().filter(|e| matches!(e, GroundEvent::Abandoned)).count();
        buildings_lost += b.events.iter().filter(|e| matches!(e, GroundEvent::BuildingDestroyed { .. })).count();
    }
    eprintln!(
        "ground wars: {} replayed, {units} unit slots, {kills} groups gone, {draws2} passes, {libc_total} libc draws, \
         {buildings_lost} buildings fell, {captures} planets abandoned",
        evs.len()
    );
    eprintln!("  {total:?}");
    if !fails.is_empty() {
        for x in fails.iter().take(25) {
            eprintln!("{x}");
        }
        panic!("{} mismatches", fails.len());
    }
}

#[test]
fn ground_war_routing_matches_trace() {
    let Some(r) = rules() else { return };
    let gr = ground_rules(&r);
    let evs = events("indit");
    if evs.is_empty() {
        return;
    }
    let mut fails = Vec::new();
    let (mut real, mut sims, mut fell, mut compared) = (0, 0, 0, 0);
    for (n, ev) in evs.iter().enumerate() {
        let (f, p) = (ev["f"].as_i64().unwrap() as usize, ev["p"].as_i64().unwrap() as usize);
        let before = &ev["before"];
        let g = &ev["g"];
        let fr = &unhex(before["fleets"].as_str().unwrap())[0x80 * f..0x80 * (f + 1)];
        let pr = unhex(before["planet"]["raw"].as_str().unwrap());
        let ki = unhex(g["playerKi"].as_str().unwrap());
        let dbg = ints(&g["dbg"]);
        let inp = RouteInput {
            fleet_ground: i32_at(fr, 0x5c),
            fleet_strength: i32_at(fr, 0x68),
            garrison_strength: i32_at(&pr, 0x5c),
            fortress: g["erod"].as_i64().unwrap() as i32,
            attacker_human: ki[fr[0x15] as usize] == 1,
            owner_human: ki[pr[0x20] as usize] == 1,
            multiplayer: g["mp"].as_u64().unwrap() != 0,
            mp_battle_status: g["mpStatus"].as_i64().unwrap() as i32,
            debug_no_lopsided: dbg[0] != 0,
            debug_lopsided_off: dbg[1] != 0,
            debug_always_simulate: dbg[2] != 0,
            // `Story_GroundBattleSimulateCheck` recomputes the ban first (clears it; quick.scn runs no
            // campaign script), so the fuzzed value before the call doesn't count.
            story_ban: false,
        };
        let _ = dbg[3];
        let (want_real, want_sim) = (ev["realtime"].as_bool().unwrap(), ev["sim"].as_bool().unwrap());
        let got = route(&inp);
        let ok = match got {
            GroundRoute::None => !want_real && !want_sim,
            GroundRoute::RealTime => want_real && !want_sim,
            GroundRoute::Simulate { .. } => want_sim && !want_real,
        };
        if !ok {
            fails.push(format!("indit {n}: route port {got:?} exe realtime {want_real} sim {want_sim} ({inp:?})"));
            continue;
        }
        if !want_sim {
            real += want_real as usize;
            continue;
        }
        sims += 1;
        // The whole simulated war from the state before `GroundWarIndit`: landing, battle, capture.
        let after = &ev["after"];
        let out = run(ev, &gr, before, before["fleets"].as_str().unwrap(), |w| {
            w.auto_resolve(f as i32);
            w.battle_status();
        });
        let wp = planet(&after["planet"], p as i32);
        let fell_port = out.events.iter().any(|e| matches!(e, GroundEvent::Captured { .. }));
        fell += fell_port as usize;
        if out.planet.owner != wp.owner {
            fails.push(format!("indit {n}: planet owner port {} exe {}", out.planet.owner, wp.owner));
        }
        // A capture runs `BolygoElfoglal` (colony side), which this replay doesn't: compare the battlefield
        // and the vehicles only when the planet kept its owner.
        if !fell_port {
            compared += 1;
            compare_fields(&format!("indit {n}"), &out.field, &field(&after["field"]), &mut fails);
            compare_world(&format!("indit {n}"), &out.world, &vehicles(after), None, &mut fails);
        }
        if out.rng.state(2) != streams(after).state(2) {
            fails.push(format!("indit {n}: stream 2 port {} exe {}", out.rng.state(2), streams(after).state(2)));
        }
    }
    eprintln!("ground routing: {} calls, {real} real-time, {sims} simulated ({fell} planets fell, {compared} battlefields compared)", evs.len());
    if !fails.is_empty() {
        for x in fails.iter().take(25) {
            eprintln!("{x}");
        }
        panic!("{} mismatches", fails.len());
    }
}

/// Fuzzed inputs on which the exe itself faulted inside `WF::Simulate` (a split partner group's
/// uninitialised carrier index, spec §8): the port must finish them without panicking.
#[test]
fn ground_war_survives_original_crashes() {
    let Some(r) = rules() else { return };
    let gr = ground_rules(&r);
    let wars: std::collections::BTreeSet<(i64, i64, String)> = events("war")
        .iter()
        .map(|e| (e["p"].as_i64().unwrap(), e["f"].as_i64().unwrap(), e["landed"]["seeds"].to_string()))
        .collect();
    let mut crashed = 0;
    for ev in events("pre") {
        let k = (ev["p"].as_i64().unwrap(), ev["f"].as_i64().unwrap(), ev["landed"]["seeds"].to_string());
        if wars.contains(&k) {
            continue;
        }
        crashed += 1;
        let landed = &ev["landed"];
        let f = ev["f"].as_i64().unwrap() as usize;
        let mut fleets_hex = String::new();
        let n = ev["g"]["nfleets"].as_u64().unwrap() as usize;
        for i in 0..=n {
            let rec = if i == f { unhex(landed["fleet"].as_str().unwrap()) } else { vec![0u8; 0x80] };
            fleets_hex.extend(rec.iter().map(|b| format!("{b:02x}")));
        }
        let out = run(&ev, &gr, landed, &fleets_hex, |w| w.simulate());
        assert!(!out.field.simulating);
    }
    eprintln!("ground wars the exe faulted on: {crashed}, finished by the port");
}
