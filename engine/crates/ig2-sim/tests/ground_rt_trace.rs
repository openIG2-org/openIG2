//! Replays the real-time ground war recorded from the running GOG `ig2.exe` (tools/trace/run_ground_rt.py):
//! - `place`: a real-time landing (`AddTanksToPlanet(f, p, 0)`: `LocateTankSuitablePosition` with the
//!   landing zone, `HalveGroupC` splits) and sometimes `StartGrWar(p)`;
//! - `turret`: a defence building (`AddBuildingTurrets`, `AddTurret`, `GetHeight`, `BuildingBuilt`).
//!
//! Each call runs from the exe's state before it; the port must reproduce every unit record field it
//! models (cells, map positions, heights, turning/driving speeds, fire range, reloads), the landing zone,
//! the battlefield globals, stream 2 and the libc `rand()` sequence (the MSVC LCG from the recorded seed).
//! Rules and colony maps come from the user's install (`IG2_INSTALL_DIR`); traces from
//! `_local/traces/grt-*.jsonl` (`IG2_TRACE_DIR` overrides; game state: never committed). Skips without them.

use std::io::BufRead;
use std::path::PathBuf;

use ig2_formats::colony_map::{self, ColonyMap};
use ig2_formats::text::{self, Lang};
use ig2_formats::Install;
use ig2_rules::Rules;
use ig2_sim::colony::Building;
use ig2_sim::game::ColonyOrder;
use ig2_sim::ground::*;
use ig2_sim::placement::Terrain;
use ig2_sim::rng::Streams;
use ig2_sim::spacewar::{CombatFleet, CombatWorld, Design, DesignRules, FleetTotals, Hull, Vehicle};
use serde_json::Value;

struct Env {
    rules: Rules,
    maps: Vec<GroundMap>,
    /// Each colony map's obstacle mask (mask B) as loaded.
    masks_b: Vec<[u8; 650]>,
}

fn env() -> Option<Env> {
    let inst = Install::from_env(Lang::En)?.expect("valid install");
    let rules = Rules::from_install(&inst).expect("catalogued exe");
    let raw = inst.read("dataraw/colony/terepleiro.txt").unwrap();
    let surfaces = colony_map::parse_surfaces(&text::decode(&raw, inst.lang())).unwrap();
    let footprints = rules.building_footprints.clone();
    let (maps, masks_b) = surfaces
        .iter()
        .enumerate()
        .map(|(n, s)| {
            let m = ColonyMap::parse(&inst.read(&format!("dataraw/colony/gen_maps/{n}.map")).unwrap()).unwrap();
            let map = GroundMap {
                terrain: Terrain { mask: m.mask_a, kind: s.kind as u8, start: s.ai_build_start },
                heights: m.heights.to_vec(),
                base: -(m.base - 200.0),
                footprints: footprints.clone(),
            };
            (map, m.mask_b)
        })
        .unzip();
    Some(Env { rules, maps, masks_b })
}

fn events(kind: &str) -> Vec<Value> {
    let dir = std::env::var_os("IG2_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("grt-"))
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
        kind0_width: r.building_footprint0.0,
    }
}

fn mat(r: &[u8], o: usize) -> [f32; 9] {
    std::array::from_fn(|k| f32_at(r, o + 4 * k))
}

/// A unit record (exe 0x290 bytes) with its member arrays.
fn unit(v: &Value) -> Group {
    unit_bytes(&unhex(v["raw"].as_str().unwrap()), ints(&v["hp"]), ints(&v["origin"]), Vec::new())
}

fn unit_bytes(r: &[u8], hp: Vec<i32>, origin: Vec<i32>, waypoints: Vec<(i32, i32)>) -> Group {
    let r = r.to_vec();
    let mut reload = [0f32; 14];
    let mut reload_left = [0f32; 14];
    for k in 0..14 {
        reload[k] = f32_at(&r, 0x1f0 + 4 * k);
        reload_left[k] = f32_at(&r, 0xa8 + 4 * k);
    }
    Group {
        design: i32_at(&r, 0x15c),
        owner: i32_at(&r, 0x194),
        side: i32_at(&r, 0x198),
        max_hp: i32_at(&r, 0x158),
        hp,
        origin,
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
        rt: TankRt {
            z: f32_at(&r, 0x1dc),
            turn: (f32_at(&r, 0xe0), f32_at(&r, 0xe4)),
            max_speed: f32_at(&r, 0x108),
            velocity: f32_at(&r, 0x10c),
            accel: f32_at(&r, 0x110),
            decel: f32_at(&r, 0x114),
            speed: f32_at(&r, 0x150),
            range: f32_at(&r, 0x148),
            reload,
            reload_left,
            angle: f32_at(&r, 0x1c0),
            turret_angle: f32_at(&r, 0x1c4),
            elevation: f32_at(&r, 0x1c8),
            f228: u32_at(&r, 0x228),
            ai_group: i32_at(&r, 0x248),
            ai_home: i32_at(&r, 0x24c),
            alive_cached: r[0x125],
            alive_n: i32_at(&r, 0x128),
            drop: [f32_at(&r, 0x264), f32_at(&r, 0x268), f32_at(&r, 0x26c)],
            planet: i32_at(&r, 0x280),
            basis: mat(&r, 0x1c),
            body: mat(&r, 0x40),
            turret: mat(&r, 0x64),
            step: (f32_at(&r, 0xc), f32_at(&r, 0x10)),
            snap: r[0x284],
            moved: r[0x285],
            aim: f32_at(&r, 0x18),
            dir: (f32_at(&r, 0x98), f32_at(&r, 0x9c)),
            want_angle: f32_at(&r, 0x12c),
            blocked: r[0x104],
            path_timer: i32_at(&r, 0x118),
            bld_range: f32_at(&r, 0x14c),
            waypoints,
            wp_cursor: i32_at(&r, 0x230),
            laying: r[0x250],
            mine_cell: (i32_at(&r, 0x254), i32_at(&r, 0x258)),
            fired: r[0x28c],
        },
        tail: Default::default(),
    }
}

/// The ground AI's per-empire state (`WF + 0x33c + 0x40·e`, ground-ai.md §2.1).
fn ai_blocks(w: &[u8]) -> [GroundAi; 19] {
    std::array::from_fn(|e| {
        if e == 0 {
            return GroundAi::default();
        }
        let o = 0x33c + 0x40 * e;
        GroundAi {
            strategy: i32_at(w, o),
            mode: i32_at(w, o + 4),
            phase: i32_at(w, o + 8),
            a: (f32_at(w, o + 0xc), f32_at(w, o + 0x10)),
            b: (f32_at(w, o + 0x14), f32_at(w, o + 0x18)),
            start: (f32_at(w, o + 0x1c), f32_at(w, o + 0x20)),
            timer: i32_at(w, o + 0x24),
            counter: i32_at(w, o + 0x28),
            targets: std::array::from_fn(|g| i32_at(w, o + 0x2c + 4 * g)),
        }
    })
}

fn field(s: &Value) -> Field {
    let w = unhex(s["field"]["raw"].as_str().unwrap());
    let mut f = Field {
        attack_fleet: i32_at(&w, 0x338),
        self_destruct: i32_at(&w, 0x334),
        countdown: i32_at(&w, 0x148),
        simulating: w[0x88c] != 0,
        last_status: i32_at(&w, 0x8c8),
        status_valid: i32_at(&w, 0x8cc) != 0,
        finished_flag: w[0x190] != 0,
        ..Default::default()
    };
    f.present.copy_from_slice(&w[0x8b1..0x8b1 + 19]);
    f.radar.copy_from_slice(&w[0x8d8..0x8d8 + 19]);
    f.not_watching.copy_from_slice(&w[0x88d..0x88d + 18]);
    f.retreat.copy_from_slice(&w[0x89f..0x89f + 18]);
    for e in 0..18 {
        f.timers[e] = (i32_at(&w, 0x7fc + 8 * e), i32_at(&w, 0x800 + 8 * e));
    }
    let l = &s["landing"];
    if l["made"].as_u64().unwrap() != 0 {
        f.landing = Some(unhex(l["grid"].as_str().unwrap()).iter().map(|&b| b as i8).collect());
    }
    for u in s["field"]["units"].as_array().unwrap() {
        let alive = u["alive"].as_u64().unwrap() != 0;
        f.alive.push(alive);
        f.units.push(if alive { unit(u) } else { Group::default() });
    }
    f
}

fn planet(s: &Value, index: i32) -> (GroundPlanet, u8) {
    let r = unhex(s["raw"].as_str().unwrap());
    let b = unhex(s["buildings"].as_str().unwrap());
    let p = GroundPlanet {
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
                x: f32_at(x, 0xc),
                y: f32_at(x, 0x10),
                flags: x[0x19],
                ..Default::default()
            })
            .collect(),
        building_count: s["count"].as_i64().unwrap() as i32,
        building_z: b.chunks(32).map(|x| f32_at(x, 0x14)).collect(),
    };
    (p, r[0x23])
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

fn world(ev: &Value, s: &Value) -> CombatWorld {
    let g = &ev["g"];
    let mut w = CombatWorld {
        fleets: s["fleets"].as_str().map_or_else(Vec::new, |h| unhex(h).chunks(0x80).map(fleet).collect()),
        vehicles: vehicles(s),
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
        on_screen: g["aktKep"].as_i64() == Some(3)
            && g["aktPlanet"] == ev["p"]
            && g["belep"].as_u64().unwrap_or(0) == 0
            && g["atlepes"].as_u64().unwrap_or(0) == 0,
        player_ki: g["playerKi"].as_str().map_or([0; 19], |h| std::array::from_fn(|k| unhex(h).get(k).copied().unwrap_or(0))),
    }
}

fn streams(s: &Value) -> Streams {
    let mut r = Streams::default();
    for (i, v) in s["seeds"].as_array().unwrap().iter().enumerate() {
        r.set_state(i, v.as_u64().unwrap() as u32);
    }
    r
}

/// The MSVC `rand()` from a seed.
struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as i32
    }
}

/// The tracer's `voices` region: `Lang`, the mute byte, then (speaker, kind, samples, cooldown) per line.
fn voices_of(b: &[u8]) -> Voices {
    let v = i32s(b);
    Voices {
        silent: v[0] == 3,
        muted: v[1] != 0,
        lines: v[2..].chunks(4).map(|c| VoiceLine { speaker: c[0], kind: c[1], samples: c[2], cooldown: c[3] }).collect(),
    }
}

/// The libc sequence after the call must continue with the recorded pair.
fn check_libc(tag: &str, lcg: &mut Lcg, v: &Value, fails: &mut Vec<String>) {
    let (r1, r2) = (v["r1"].as_i64().unwrap() as i32, v["r2"].as_i64().unwrap() as i32);
    let (a, b) = (lcg.next(), lcg.next());
    if (a, b) != (r1, r2) {
        fails.push(format!("{tag}: libc after the call port ({a}, {b}) exe ({r1}, {r2})"));
    }
}

fn bits(x: f32) -> u32 {
    x.to_bits()
}
fn m(a: &[f32; 9]) -> Vec<u32> {
    a.iter().map(|x| x.to_bits()).collect()
}

/// Every field the port owns, as one comparable record.
fn key(u: &Group) -> String {
    let target = if u.flags & flag::HAS_TARGET != 0 { u.target } else { NONE };
    let partner_link = if u.flags & flag::PARTNER != 0 { u.carrier } else { 0 };
    let rt = &u.rt;
    let reload: Vec<u32> = rt.reload.iter().map(|&x| bits(x)).collect();
    format!(
        "d{} o{} s{} max{} hp{:?} or{:?} fl{:#x} f2{:#x} w{:#x} pods{} ammo{} regen{} air{} gun{} hull{} prev{} \
         partner{} carrier{} par{} sei{} bld{} tgt{} idx{} aim{} tr{:#x} cell{:?} last{:?} mv{:?} pos{:x}/{:x} z{:x} \
         turn{:x}/{:x} m{:x}/{:x}/{:x}/{:x} spd{:x} rng{:x} rel{:x?} ang{:x}/{:x} elevation{} f228{} group{} alive_cached{} \
         drop{:x?} pl{} hit{:?} basis{:x?} body{:x?} tur{:x?} rp{:x}/{:x} snap{} moved{}",
        u.design, u.owner, u.side, u.max_hp, u.hp, u.origin, u.flags & !(flag::MESH | flag::MESH2), u.flags2, u.weapons,
        u.pods, u.ammo, u.regen, u.air, u.has_gun as u8, u.hull, u.prev_size, u.partner, partner_link, u.paralysed,
        u.seized, u.building, target, u.index, u.aim, u.transport, u.cell, u.last_cell, u.move_state, bits(u.pos.0),
        bits(u.pos.1), bits(rt.z), bits(rt.turn.0), bits(rt.turn.1), bits(rt.max_speed), bits(rt.velocity), bits(rt.accel),
        bits(rt.decel), bits(rt.speed), bits(rt.range), reload, bits(rt.angle), bits(rt.turret_angle), bits(rt.elevation), rt.f228, rt.ai_group,
        rt.alive_cached, rt.drop.map(bits), rt.planet, u.last_hit, m(&rt.basis), m(&rt.body), m(&rt.turret), bits(rt.step.0),
        bits(rt.step.1), rt.snap, rt.moved
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
            // Fields the exe leaves uninitialised: the last hit of a unit never hit (`None` in the port) and
            // a turret's render position (`SetPosG` never runs for turrets).
            let mut w = want.units[i].clone();
            if got.units[i].last_hit.is_none() {
                w.last_hit = None;
            }
            if w.flags & flag::TURRET != 0 {
                w.rt.step = got.units[i].rt.step;
            }
            let (g, w) = (key(&got.units[i]), key(&w));
            if g != w {
                let d: Vec<String> =
                    g.split(' ').zip(w.split(' ')).filter(|(a, b)| a != b).map(|(a, b)| format!("port {a} exe {b}")).collect();
                fails.push(format!("{tag}: unit {i}: {}", d.join("; ")));
            }
        }
    }
    let gl = (got.attack_fleet, got.countdown, got.last_status, got.status_valid, got.finished_flag);
    let wl = (want.attack_fleet, want.countdown, want.last_status, want.status_valid, want.finished_flag);
    if gl != wl {
        fails.push(format!("{tag}: field globals port {gl:?} exe {wl:?}"));
    }
    if (got.timers, got.not_watching, got.retreat) != (want.timers, want.not_watching, want.retreat) {
        fails.push(format!("{tag}: timers/flags port {:?} {:?} {:?} exe {:?} {:?} {:?}", got.timers, got.not_watching,
            got.retreat, want.timers, want.not_watching, want.retreat));
    }
    if got.landing != want.landing {
        let diff = match (&got.landing, &want.landing) {
            (Some(a), Some(b)) => a.iter().zip(b).position(|(x, y)| x != y).map(|i| format!("first diff at cell {i}")),
            _ => None,
        };
        fails.push(format!("{tag}: landing zone port {} exe {} {diff:?}", got.landing.is_some(), want.landing.is_some()));
    }
}

struct Run {
    field: Field,
    rng: Streams,
    lcg: Lcg,
}

fn run(env: &Env, gr: &GroundRules, ev: &Value, start: &Value, seed: u32, f: impl FnOnce(&mut War)) -> Run {
    let p = ev["p"].as_i64().unwrap() as i32;
    let ctx = context(ev);
    let mut field = field(start);
    let (mut planet, map) = planet(&start["planet"], p);
    let mut world = world(ev, start);
    let mut rng = streams(start);
    let mut lcg = Lcg(seed);
    {
        let mut libc = || lcg.next();
        let mut place = |_s: Option<(i32, i32)>| (false, (0, 0));
        let view = ev["g"]["aktMap"].as_u64().map(|m| &env.maps[m as usize]);
        let mut war = War::new(&mut field, &mut planet, &mut world, gr, &ctx, &mut rng, &mut libc, &mut place)
            .with_map(&env.maps[map as usize])
            .with_view_map(view);
        f(&mut war);
    }
    Run { field, rng, lcg }
}

#[test]
fn realtime_landing_matches_trace() {
    let Some(env) = env() else { return };
    let gr = ground_rules(&env.rules);
    let evs = events("place");
    if evs.is_empty() {
        return;
    }
    let mut fails = Vec::new();
    let (mut units, mut starts) = (0, 0);
    for (n, ev) in evs.iter().enumerate() {
        let f = ev["f"].as_i64().unwrap() as i32;
        let tag = format!("place {n} (p {} f {f})", ev["p"]);
        let seed = ev["rand"]["seed"].as_u64().unwrap() as u32;
        let mut a = run(&env, &gr, ev, &ev["before"], seed, |w| w.land(f, false));
        let want = field(&ev["after"]);
        compare_fields(&tag, &a.field, &want, &mut fails);
        check_libc(&tag, &mut a.lcg, &ev["rand"], &mut fails);
        if a.rng != streams(&ev["after"]) {
            fails.push(format!("{tag}: streams moved"));
        }
        units += want.alive.iter().filter(|&&x| x).count();
        let st = &ev["start"];
        if st.is_object() {
            starts += 1;
            let seed = st["rand"]["seed"].as_u64().unwrap() as u32;
            let mut b = run(&env, &gr, ev, &ev["after"], seed, |w| {
                w.start();
            });
            compare_fields(&format!("{tag} start"), &b.field, &field(&st["after"]), &mut fails);
            check_libc(&format!("{tag} start"), &mut b.lcg, &st["rand"], &mut fails);
        }
    }
    eprintln!("real-time landings: {} replayed ({units} groups placed), {starts} StartGrWar", evs.len());
    if !fails.is_empty() {
        for x in fails.iter().take(20) {
            eprintln!("{x}");
        }
        panic!("{} mismatches", fails.len());
    }
}

#[test]
fn turrets_match_trace() {
    let Some(env) = env() else { return };
    let gr = ground_rules(&env.rules);
    let evs = events("turret");
    if evs.is_empty() {
        return;
    }
    let mut fails = Vec::new();
    let mut turrets = 0;
    for (n, ev) in evs.iter().enumerate() {
        let slot = ev["slot"].as_i64().unwrap() as i32;
        if slot < 1 {
            continue;
        }
        let kind = ev["kind"].as_u64().unwrap() as u8;
        let done = ev["done"].as_u64().unwrap() != 0;
        let tag = format!("turret {n} (p {} kind {kind:#x} slot {slot})", ev["p"]);
        let seed = ev["rand"]["seed"].as_u64().unwrap() as u32;
        // The building itself comes from the colony side (`EpuletNew`): take the exe's planet after the call.
        let mut start = ev["before"].clone();
        start["planet"] = ev["after"]["planet"].clone();
        let mut a = run(&env, &gr, ev, &start, seed, |w| {
            let side = w.planet.owner as i32;
            w.add_building_turrets(kind, side, slot);
            if done {
                w.building_built(slot);
            }
        });
        let want = field(&ev["after"]);
        compare_fields(&tag, &a.field, &want, &mut fails);
        let _ = &mut a.lcg;
        turrets += want.alive.iter().filter(|&&x| x).count() - field(&ev["before"]).alive.iter().filter(|&&x| x).count();
    }
    eprintln!("defence buildings: {} replayed, {turrets} turrets", evs.len());
    if !fails.is_empty() {
        for x in fails.iter().take(20) {
            eprintln!("{x}");
        }
        panic!("{} mismatches", fails.len());
    }
}

// ------------------------------------------------------------------------------------------------ battle

/// Region state rebuilt from `init` + the `pre`/`post` diffs.
/// `PlanetAtjarasMap::GenerateMap(p − 1, 0)` for a build the trace saw happen inside a call: the port
/// generates the maps from the live record of colony map `m` (the battle's `g.terrain`, else the file) and
/// they must equal the exe's (`st`, after the call). The heights past the grid are the head of record
/// `m + 1` (a heap pointer and leftover bytes: per-run, spec §11.13); with that record in the trace the
/// maps must be exact, else they only reach the strip `y ≥ 195`, which is taken from the exe's maps and
/// everything outside it must match. Returns the maps `[0..6]` as the field holds them.
fn generated_maps(env: &Env, meta: &Value, m: usize, st: &Regions, tag: &str, fails: &mut Vec<String>, gen_tail: &mut (usize, usize)) -> Option<Vec<Vec<u16>>> {
    let terrain = &meta["g"]["terrain"];
    let record = terrain[m.to_string()].as_str().map(unhex);
    let next = terrain[(m + 1).to_string()].as_str().map(unhex);
    let (mut map, obstacles) = match &record {
        Some(r) => {
            let mut map = env.maps[m].clone();
            map.heights = r[0x138c..0x2714].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
            map.base = f32::from_le_bytes(r[0x84..0x88].try_into().unwrap());
            (map, r[0x330..0x330 + 650].to_vec())
        }
        None => (env.maps[m].clone(), env.masks_b[m].to_vec()),
    };
    if let Some(n) = &next {
        map.heights.extend(n[..102].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])));
    }
    let mut built = generate_map(&map, &obstacles, &env.rules.ground_slope_limits).maps;
    let want: Vec<Vec<u16>> = (0..6)
        .map(|k| st.get(&format!("atj{k}")).map(|b| b.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()).unwrap_or_default())
        .collect();
    let mut strip = false;
    for k in 1..=5 {
        for i in 0..40000 {
            if built[k][i] == want[k][i] {
                continue;
            }
            if next.is_none() && i % 200 >= 195 {
                built[k][i] = want[k][i];
                strip = true;
            } else {
                fails.push(format!("{tag}: GenerateMap layer {k} cell ({}, {}) port {} exe {}", i / 200, i % 200, built[k][i], want[k][i]));
                return None;
            }
        }
    }
    gen_tail.0 += strip as usize;
    gen_tail.1 += next.is_some() as usize;
    Some(built)
}

type Regions = std::collections::BTreeMap<String, Vec<u8>>;

fn apply(state: &mut Regions, d: &Value) {
    for (n, v) in d.as_object().unwrap() {
        let len = v["len"].as_i64().unwrap();
        if len < 0 {
            // A dead unit's record is freed by `RemoveTankGroup` but its pointer stays: a shooter that picked
            // it as its auto-aim after `RemoveTarget` ran keeps reading it. Keep the last bytes as the freed
            // block's content.
            if let Some(b) = state.remove(n) {
                if ["u", "hp", "or"].iter().any(|p| n.strip_prefix(p).is_some_and(|d| d.bytes().all(|c| c.is_ascii_digit()))) {
                    state.insert(format!("freed-{n}"), b);
                }
            }
            continue;
        }
        let b = state.entry(n.clone()).or_default();
        b.resize(len as usize, 0);
        for run in v["runs"].as_array().unwrap() {
            let off = run[0].as_u64().unwrap() as usize;
            let raw = unhex(run[1].as_str().unwrap());
            b[off..off + raw.len()].copy_from_slice(&raw);
        }
    }
}

fn f3(r: &[u8], o: usize) -> [f32; 3] {
    [f32_at(r, o), f32_at(r, o + 4), f32_at(r, o + 8)]
}

fn i32s(b: &[u8]) -> Vec<i32> {
    b.chunks(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect()
}

fn ptrs(st: &Regions, list: &str) -> Vec<u32> {
    st.get(list).map(|b| b.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect()).unwrap_or_default()
}

/// The objects of one projectile list: slot `k` holds an object when its pointer is set.
fn objects(st: &Regions, w: &[u8], name: &str, list: &str, count_off: usize) -> Vec<Option<Vec<u8>>> {
    let cnt = i32_at(w, count_off).max(0) as usize;
    let p = ptrs(st, list);
    (0..cnt).map(|k| if p.get(k).copied().unwrap_or(0) != 0 { st.get(&format!("{name}{k}")).cloned() } else { None }).collect()
}

/// The battlefield and planet of a recorded call's state.
fn battle_state(st: &Regions, p: i32) -> (Field, GroundPlanet) {
    let w = &st["wf"];
    let mut f = Field {
        attack_fleet: i32_at(w, 0x338),
        self_destruct: i32_at(w, 0x334),
        countdown: i32_at(w, 0x148),
        simulating: w[0x88c] != 0,
        last_status: i32_at(w, 0x8c8),
        status_valid: i32_at(w, 0x8cc) != 0,
        finished_flag: w[0x190] != 0,
        ..Default::default()
    };
    f.present.copy_from_slice(&w[0x8b1..0x8b1 + 19]);
    f.radar.copy_from_slice(&w[0x8d8..0x8d8 + 19]);
    f.not_watching.copy_from_slice(&w[0x88d..0x88d + 18]);
    f.retreat.copy_from_slice(&w[0x89f..0x89f + 18]);
    for e in 0..18 {
        f.timers[e] = (i32_at(w, 0x7fc + 8 * e), i32_at(w, 0x800 + 8 * e));
    }
    f.lists.fighters_on = w[0x32c] != 0;
    for (k, x) in f.lists.fighters.iter_mut().enumerate() {
        let o = 0x1f4 + 0x40 * k;
        *x = Fighter {
            pos: f3(w, o),
            vel: f3(w, o + 0xc),
            step: f3(w, o + 0x18),
            phase: i32_at(w, o + 0x24),
            ground: f32_at(w, o + 0x28),
            cooldown: i32_at(w, o + 0x34),
        };
    }
    f.ai = ai_blocks(w);
    let n = i32_at(w, 0x180).max(0) as usize;
    let alive = st.get("alive").cloned().unwrap_or_default();
    for i in 0..n {
        let a = alive.get(i).copied().unwrap_or(0) != 0;
        f.alive.push(a);
        let g = match (a, st.get(&format!("u{i}"))) {
            (true, Some(r)) => {
                let hp = st.get(&format!("hp{i}")).map(|b| i32s(b)).unwrap_or_default();
                let or = st.get(&format!("or{i}")).map(|b| i32s(b)).unwrap_or_default();
                let wp = st.get(&format!("wp{i}")).map(|b| i32s(b).chunks(2).map(|c| (c[0], c[1])).collect()).unwrap_or_default();
                unit_bytes(r, hp, or, wp)
            }
            (false, _) if st.contains_key(&format!("freed-u{i}")) => {
                let hp = st.get(&format!("freed-hp{i}")).map(|b| i32s(b)).unwrap_or_default();
                let or = st.get(&format!("freed-or{i}")).map(|b| i32s(b)).unwrap_or_default();
                unit_bytes(&st[&format!("freed-u{i}")], hp, or, Vec::new())
            }
            _ => Group::default(),
        };
        f.units.push(g);
    }
    f.lists.shells = objects(st, w, "A", "LA", 0xe0)
        .into_iter()
        .map(|o| o.map(|r| Shell {
            vel: f3(&r, 8),
            pos: f3(&r, 0x14),
            life: i32_at(&r, 0x20),
            target: i32_at(&r, 0x24),
            pull: f32_at(&r, 0x28),
            dmg: i32_at(&r, 0x2c),
            by: i32_at(&r, 0x30),
            jitter: r[0x34],
        }))
        .collect();
    f.lists.missiles = objects(st, w, "B", "LB", 0xe8)
        .into_iter()
        .map(|o| o.map(|r| Missile {
            vel: f3(&r, 8),
            pos: f3(&r, 0x14),
            dest: f3(&r, 0x20),
            life: i32_at(&r, 0x2c),
            dmg: i32_at(&r, 0x30),
            target: i32_at(&r, 0x34),
            kind: i32_at(&r, 0x38),
            fade: f32_at(&r, 0x3c),
            by: i32_at(&r, 0x40),
        }))
        .collect();
    f.lists.bolts = objects(st, w, "C", "LC", 0xe4)
        .into_iter()
        .map(|o| o.map(|r| Bolt { vel: f3(&r, 8), pos: f3(&r, 0x14), dest: f3(&r, 0x20), life: i32_at(&r, 0x2c), target: i32_at(&r, 0x30), power: i32_at(&r, 0x34) }))
        .collect();
    if let Some(z) = st.get("landing") {
        f.landing = Some(z.iter().map(|&b| b as i8).collect());
    }
    f.passability = (0..6)
        .map(|k| st.get(&format!("atj{k}")).map(|b| b.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()).unwrap_or_default())
        .collect();
    let pr = &st["planet"];
    let b = st.get("bld").cloned().unwrap_or_default();
    let planet = GroundPlanet {
        index: p,
        owner: pr[0x20],
        garrison: i32_at(pr, 0x58),
        garrison_strength: i32_at(pr, 0x5c),
        in_war: pr[0x64],
        buildings: b
            .chunks(32)
            .map(|x| Building {
                kind: x[0],
                damage: i32_at(x, 4),
                work_left: u16::from_le_bytes([x[8], x[9]]),
                x: f32_at(x, 0xc),
                y: f32_at(x, 0x10),
                flags: x[0x19],
                ..Default::default()
            })
            .collect(),
        building_count: st.get("bcount").map_or(0, |b| i32_at(b, 0)),
        building_z: b.chunks(32).map(|x| f32_at(x, 0x14)).collect(),
    };
    (f, planet)
}

/// The projectile lists, keyed for comparison; debris (kind 7) only by its kind (its lifetime is
/// uninitialised heap memory in the exe and its motion is cosmetic); then the fighter wave.
fn list_keys(f: &Field) -> Vec<String> {
    let mut out = Vec::new();
    for (k, x) in f.lists.fighters.iter().enumerate() {
        let on = f.lists.fighters_on;
        out.push(format!("F{k} {on} {:?}", (x.pos.map(bits), x.vel.map(bits), x.step.map(bits), x.phase, bits(x.ground), x.cooldown)));
    }
    for (k, s) in f.lists.shells.iter().enumerate() {
        out.push(format!("A{k} {:?}", s.as_ref().map(|s| (s.vel.map(bits), s.pos.map(bits), s.life, s.target, bits(s.pull), s.dmg, s.by, s.jitter))));
    }
    for (k, m) in f.lists.missiles.iter().enumerate() {
        out.push(match m {
            Some(m) if m.kind == 7 => format!("B{k} debris"),
            m => format!("B{k} {:?}", m.as_ref().map(|m| (m.vel.map(bits), m.pos.map(bits), m.dest.map(bits), m.life, m.dmg, m.target, m.kind, m.by))),
        });
    }
    for (k, b) in f.lists.bolts.iter().enumerate() {
        out.push(format!("C{k} {:?}", b.as_ref().map(|b| (b.vel.map(bits), b.pos.map(bits), b.dest.map(bits), b.life, b.target, b.power))));
    }
    out
}

/// `FRISSIT_Grwar`'s own part before the nested `WF::MI`/`WF::Refresh` (or its exit): self-destruct
/// timers and `WF::Collapse`; `entry` is the libc pair the next call saw.
#[allow(clippy::too_many_arguments)]
fn check_grwar(
    tag: &str,
    env: &Env,
    gr: &GroundRules,
    meta: &Value,
    before: &Regions,
    ev: &Value,
    after: &Regions,
    entry: &Value,
    fuzz_masked: bool,
    fails: &mut Vec<String>,
    kinds: &mut std::collections::BTreeMap<String, usize>,
) -> bool {
    let n0 = fails.len();
    let p = meta["p"].as_i64().unwrap() as i32;
    let g = &ev["g"];
    let mut ctx = context(meta);
    ctx.on_screen = g["aktKep"].as_i64() == Some(3) && g["aktPlanet"].as_i64() == Some(p as i64) && g["belep"] == 0 && g["atlepes"] == 0;
    let clock = Clock { speed: g["speed"].as_i64().unwrap() as i32, paused: false };
    let (mut field, mut planet) = battle_state(before, p);
    let s = serde_json::json!({ "tanks": before.get("tanks").map(|b| hexs(b)).unwrap_or_default() });
    let mut world = world(meta, &s);
    let mut rng = Streams::default();
    for (k, c) in before["seeds"].chunks(4).enumerate() {
        rng.set_state(k, u32::from_le_bytes(c.try_into().unwrap()));
    }
    let mut lcg = Lcg(ev["seed"].as_u64().unwrap() as u32);
    let m = before["planet"][0x23] as usize;
    let mut voices = before.get("voices").map(|b| voices_of(b));
    {
        let mut libc = || lcg.next();
        let mut place = |_s: Option<(i32, i32)>| (false, (0, 0));
        let mut war = War::new(&mut field, &mut planet, &mut world, gr, &ctx, &mut rng, &mut libc, &mut place).with_map(&env.maps[m]);
        if let Some(v) = voices.as_mut() {
            war = war.with_voices(v);
        }
        war.grwar_planet(clock, g["gvalue"].as_i64().unwrap() as i32 + 1);
    }
    let (want, wplanet) = battle_state(after, p);
    let mut fail = |kind: &str, msg: String| {
        *kinds.entry(format!("grwar {kind}")).or_default() += 1;
        fails.push(format!("{tag}: {msg}"));
    };
    if field.units.len() != want.units.len() || field.alive != want.alive {
        fail("units", format!("units port {} exe {}", field.units.len(), want.units.len()));
    } else {
        for i in 0..field.units.len() {
            let mut w = want.units[i].clone();
            if fuzz_masked {
                // Recorded before the tracer's `aifuzz` event: the AI fuzzer's writes (weapons, air units,
                // hit markers) made before `WF::MI` show up in this diff.
                let g = &field.units[i];
                (w.weapons, w.air, w.last_hit) = (g.weapons, g.air, g.last_hit);
            }
            if field.alive[i] && tick_key(&field.units[i]) != tick_key(&w) {
                fail("unit", format!("unit {i} differs"));
            }
        }
    }
    let (gl, wl) = (list_keys(&field), list_keys(&want));
    if gl != wl {
        fail("lists", format!("lists port {} exe {}", gl.len(), wl.len()));
    }
    if field.self_destruct != want.self_destruct {
        fail("timer", format!("self-destruct port {} exe {}", field.self_destruct, want.self_destruct));
    }
    let bk = |p: &GroundPlanet| p.buildings.iter().map(|x| (x.kind, x.damage)).collect::<Vec<_>>();
    if bk(&planet) != bk(&wplanet) {
        fail("buildings", "buildings differ".into());
    }
    let mut wr = Streams::default();
    for (k, c) in after["seeds"].chunks(4).enumerate() {
        wr.set_state(k, u32::from_le_bytes(c.try_into().unwrap()));
    }
    if rng.state(2) != wr.state(2) {
        fail("stream2", "stream 2 differs".into());
    }
    if let (Some(r1), Some(r2)) = (entry["r1"].as_i64(), entry["r2"].as_i64()) {
        let at = lcg.0;
        if (lcg.next() as i64, lcg.next() as i64) != (r1, r2) {
            let seed = ev["seed"].as_u64().unwrap() as u32;
            let mut l = Lcg(seed);
            let port = (0..20_000).find(|_| {
                let hit = l.0 == at;
                l.next();
                hit
            });
            let mut w = Lcg(seed);
            let exe = (0..20_000).find(|_| {
                let mut t = Lcg(w.0);
                let hit = (t.next() as i64, t.next() as i64) == (r1, r2);
                w.next();
                hit
            });
            fail("libc", format!("libc draws port {port:?} exe {exe:?}"));
        }
    }
    fails.len() == n0
}

/// A list mismatch only because the exe freed fresh debris in the same tick: debris lifetimes are
/// uninitialised heap bytes in the exe (spec §11.12), so its slots are not reproducible. Shells and bolts
/// match exactly, the other list-B objects match as a multiset and the port holds at least as much debris.
fn debris_only(port: &[String], exe: &[String]) -> bool {
    let split = |v: &[String]| {
        let mut other: Vec<String> = Vec::new();
        let mut debris = 0;
        for k in v {
            let (slot, body) = k.split_once(' ').unwrap();
            if body == "debris" {
                debris += 1;
            } else if slot.starts_with('B') {
                if body != "None" {
                    other.push(format!("B {body}"));
                }
            } else {
                other.push(k.clone());
            }
        }
        other.sort();
        (other, debris)
    };
    let ((po, pd), (eo, ed)) = (split(port), split(exe));
    po == eo && pd >= ed
}

fn tick_key(u: &Group) -> String {
    let rt = &u.rt;
    format!(
        "{} aim{:x} dir{:x}/{:x} want{:x} blk{} pt{} br{:x} wp{:?}/{} lay{} mc{:?} fired{} an{}",
        key(u),
        bits(rt.aim),
        bits(rt.dir.0),
        bits(rt.dir.1),
        bits(rt.want_angle),
        rt.blocked,
        rt.path_timer,
        bits(rt.bld_range),
        rt.waypoints,
        rt.wp_cursor,
        rt.laying,
        rt.mine_cell,
        rt.fired,
        rt.alive_n
    )
}

fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn battle_tick_matches_trace() {
    let Some(env) = env() else { return };
    let gr = ground_rules(&env.rules);
    let dir = std::env::var_os("IG2_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.starts_with("grt-battle") || n.starts_with("grt-small") || n.starts_with("grt-ai") || n.starts_with("grt-transport") || n.starts_with("grt-laser")
        })
        .collect();
    files.sort();
    let limit: usize = std::env::var("IG2_BATTLE_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
    let mut fails: Vec<String> = Vec::new();
    let (mut calls, mut exact, mut heap_debris, mut generated) = (0usize, 0usize, 0usize, 0usize);
    // Builds whose result lies outside the strip the unrecorded bytes past the height grid reach / fully.
    let mut gen_tail = (0usize, 0usize);
    let (mut grwars, mut grwar_exact, mut finished, mut elevation_ulps) = (0usize, 0usize, 0usize, 0usize);
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    let (mut fighter_ticks, mut bomb_ticks, mut mad_ticks) = (0usize, 0usize, 0usize);
    // `GroundEvent::Voice` (the client's lines) against the exe's speaker: on-screen ticks checked, lines picked.
    let (mut voice_ticks, mut voice_plays) = (0usize, 0usize);
    for path in files {
        let mut meta = Value::Null;
        let mut st = Regions::new();
        let mut grwar: Option<(Regions, Value)> = None;
        let fuzz_masked = path.file_name().unwrap().to_string_lossy().starts_with("grt-ai")
            && !std::fs::read_to_string(&path).unwrap().contains("\"k\":\"aifuzz\"");
        for line in std::io::BufReader::new(std::fs::File::open(&path).unwrap()).lines() {
            let ev: Value = serde_json::from_str(&line.unwrap()).unwrap();
            let k = ev["k"].as_str().unwrap();
            if matches!(k, "mi" | "ref" | "grwar_end" | "aifuzz") {
                if let Some((gst, gev)) = grwar.take() {
                    let mut after = st.clone();
                    apply(&mut after, if k == "grwar_end" { &ev["post"] } else { &ev["pre"] });
                    let tag = format!("{} battle {} grwar tick {}", path.file_name().unwrap().to_string_lossy(), gev["battle"], gev["tick"]);
                    grwars += 1;
                    if check_grwar(&tag, &env, &gr, &meta, &gst, &gev, &after, &ev["entry"], fuzz_masked, &mut fails, &mut kinds) {
                        grwar_exact += 1;
                    }
                }
            }
            match k {
                "battle" => meta = ev,
                "grwar" => {
                    apply(&mut st, &ev["pre"]);
                    grwar = Some((st.clone(), ev));
                }
                "grwar_end" => apply(&mut st, &ev["post"]),
                "aifuzz" => apply(&mut st, &ev["pre"]),
                // The dropships and landing orders between ticks (replayed by `transporters_match_trace`).
                "hor" | "order" | "gorder" => {
                    apply(&mut st, &ev["pre"]);
                    apply(&mut st, &ev["post"]);
                }
                "init" => {
                    st.clear();
                    for (n, h) in ev["regions"].as_object().unwrap() {
                        st.insert(n.clone(), unhex(h.as_str().unwrap()));
                    }
                }
                "mi" => {
                    apply(&mut st, &ev["pre"]);
                    apply(&mut st, &ev["post"]);
                }
                "ref" => {
                    apply(&mut st, &ev["pre"]);
                    let mut before = st.clone();
                    apply(&mut st, &ev["post"]);
                    // The planet's passability maps, built by the first path search inside this call
                    // (`PlanetAtjarasMap::GenerateMap`, exe 0x479300): generated by the port from the live
                    // colony-map record and checked against the exe's.
                    if !before.contains_key("atj1") && st.contains_key("atj1") {
                        let tag = format!("{} battle {} tick {}", path.file_name().unwrap().to_string_lossy(), ev["battle"], ev["tick"]);
                        let m = before["planet"][0x23] as usize;
                        if let Some(built) = generated_maps(&env, &meta, m, &st, &tag, &mut fails, &mut gen_tail) {
                            for (k, g) in built.iter().enumerate().skip(1) {
                                before.insert(format!("atj{k}"), g.iter().flat_map(|c| c.to_le_bytes()).collect());
                            }
                            generated += 1;
                        }
                    }
                    if calls >= limit {
                        continue;
                    }
                    calls += 1;
                    let p = meta["p"].as_i64().unwrap() as i32;
                    let tag = format!("{} battle {} tick {}", path.file_name().unwrap().to_string_lossy(), ev["battle"], ev["tick"]);
                    let g = &ev["g"];
                    let mut ctx = context(&meta);
                    ctx.on_screen =
                        g["aktKep"].as_i64() == Some(3) && g["aktPlanet"].as_i64() == Some(p as i64) && g["belep"] == 0 && g["atlepes"] == 0;
                    let clock = Clock { speed: g["speed"].as_i64().unwrap() as i32, paused: false };
                    let watching: [i32; 18] = std::array::from_fn(|k| i32_at(&before["kimelyik"], 4 * k));
                    let (mut field, mut planet) = battle_state(&before, p);
                    let before_countdown = field.countdown;
                    fighter_ticks += field.lists.fighters_on as usize;
                    bomb_ticks += field.lists.missiles.iter().flatten().any(|m| matches!(m.kind, 1 | 2)) as usize;
                    let s = serde_json::json!({ "tanks": before.get("tanks").map(|b| hexs(b)).unwrap_or_default() });
                    let mut world = world(&meta, &s);
                    let mut rng = Streams::default();
                    for (k, c) in before["seeds"].chunks(4).enumerate() {
                        rng.set_state(k, u32::from_le_bytes(c.try_into().unwrap()));
                    }
                    let before_s5 = rng.state(5);
                    let mut lcg = Lcg(ev["rand"]["seed"].as_u64().unwrap() as u32);
                    let m = before["planet"][0x23] as usize;
                    let mut voices = before.get("voices").map(|b| voices_of(b));
                    let voices_before = voices.clone();
                    let events = {
                        let mut libc = || lcg.next();
                        let mut place = |_s: Option<(i32, i32)>| (false, (0, 0));
                        let mut war = War::new(&mut field, &mut planet, &mut world, &gr, &ctx, &mut rng, &mut libc, &mut place).with_map(&env.maps[m]);
                        if let Some(v) = voices.as_mut() {
                            war = war.with_voices(v);
                        }
                        war.refresh(clock, &watching);
                        war.events
                    };
                    // The client's view: the voice events alone, played on the speaker as it was, give the exe's.
                    if let (true, Some(mut from_events), Some(w)) = (ctx.on_screen, voices_before, st.get("voices").map(|b| voices_of(b))) {
                        voice_ticks += 1;
                        for e in &events {
                            if let GroundEvent::Voice(v) = *e {
                                let (line, kind) = v.line(ctx.human);
                                voice_plays += from_events.play(line, kind, ctx.human) as usize;
                            }
                        }
                        if from_events.lines != w.lines {
                            *kinds.entry("voice events".into()).or_default() += 1;
                            fails.push(format!("{tag}: the voice events don't give the exe's speaker"));
                        }
                    }
                    if let (Some(v), Some(w)) = (&voices, st.get("voices").map(|b| voices_of(b))) {
                        if *v != w {
                            *kinds.entry("voices".into()).or_default() += 1;
                            let d: Vec<String> =
                                v.lines.iter().zip(&w.lines).filter(|(a, b)| a != b).map(|(a, b)| format!("port {a:?} exe {b:?}")).collect();
                            fails.push(format!("{tag}: voices muted {}/{} {}", v.muted, w.muted, d.join("; ")));
                        }
                    }
                    let (want, wplanet) = battle_state(&st, p);
                    let before_fails = fails.len();
                    let n = field.units.len().max(want.units.len());
                    for i in 0..n {
                        let ga = field.alive.get(i).copied().unwrap_or(false);
                        let wa = want.alive.get(i).copied().unwrap_or(false);
                        if ga != wa {
                            fails.push(format!("{tag}: unit {i} alive port {ga} exe {wa}"));
                            *kinds.entry("alive".into()).or_default() += 1;
                            continue;
                        }
                        if !ga {
                            continue;
                        }
                        let mut w = want.units[i].clone();
                        let g2 = &field.units[i];
                        if g2.last_hit.is_none() {
                            w.last_hit = None;
                        }
                        if w.flags & flag::TURRET != 0 {
                            w.rt.step = g2.rt.step;
                        }
                        let (gk, wk) = (tick_key(g2), tick_key(&w));
                        // The gun elevation `+0x1c8` is render-only; it differs by up to ~150 ulps in a
                        // handful of solutions (identical inputs, the asm's chain: spec §11.8).
                        let elev_only = gk.split(' ').zip(wk.split(' ')).filter(|(a, b)| a != b).all(|(a, b)| {
                            let n = |s: &str| s.strip_prefix("elevation").and_then(|v| v.parse::<i64>().ok());
                            matches!((n(a), n(b)), (Some(x), Some(y)) if (x - y).abs() <= 256)
                        });
                        if gk != wk && elev_only {
                            elevation_ulps += 1;
                        } else if gk != wk {
                            let d: Vec<String> = gk
                                .split(' ')
                                .zip(wk.split(' '))
                                .filter(|(a, b)| a != b)
                                .map(|(a, b)| {
                                    let name: String = a.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
                                    *kinds.entry(name).or_default() += 1;
                                    format!("port {a} exe {b}")
                                })
                                .collect();
                            fails.push(format!("{tag}: unit {i}: {}", d.join("; ")));
                        }
                    }
                    let (gl, wl) = (list_keys(&field), list_keys(&want));
                    if gl != wl && debris_only(&gl, &wl) {
                        heap_debris += 1;
                    } else if gl != wl {
                        *kinds.entry("lists".into()).or_default() += 1;
                        let d: Vec<String> =
                            gl.iter().zip(&wl).filter(|(a, b)| a != b).map(|(a, b)| format!("\n    port {a}\n    exe  {b}")).take(3).collect();
                        fails.push(format!("{tag}: lists (len port {} exe {}){}", gl.len(), wl.len(), d.join("")));
                    }
                    let gg = (field.countdown, field.last_status, field.finished_flag, field.timers, field.not_watching);
                    let wg = (want.countdown, want.last_status, want.finished_flag, want.timers, want.not_watching);
                    if gg != wg {
                        *kinds.entry("field".into()).or_default() += 1;
                        fails.push(format!("{tag}: field port {gg:?} exe {wg:?}"));
                    }
                    if want.finished_flag && field.countdown == -1 && before_countdown != -1 {
                        // The countdown ran out in this call: `GroundWarFinished(p, s, false)`.
                        finished += 1;
                        if planet.owner != wplanet.owner {
                            *kinds.entry("finish".into()).or_default() += 1;
                            fails.push(format!("{tag}: owner after GroundWarFinished port {} exe {}", planet.owner, wplanet.owner));
                        }
                    }
                    let bk = |p: &GroundPlanet| p.buildings.iter().map(|x| (x.kind, x.damage)).collect::<Vec<_>>();
                    if bk(&planet) != bk(&wplanet) {
                        *kinds.entry("buildings".into()).or_default() += 1;
                        fails.push(format!("{tag}: buildings port {:?} exe {:?}", bk(&planet), bk(&wplanet)));
                    }
                    let mut wr = Streams::default();
                    for (k, c) in st["seeds"].chunks(4).enumerate() {
                        wr.set_state(k, u32::from_le_bytes(c.try_into().unwrap()));
                    }
                    if rng.state(2) != wr.state(2) {
                        *kinds.entry("stream2".into()).or_default() += 1;
                        fails.push(format!("{tag}: stream 2 port {} exe {}", rng.state(2), wr.state(2)));
                    }
                    // Stream 5: the story mad tanks (`--mad`).
                    if rng.state(5) != wr.state(5) {
                        *kinds.entry("stream5".into()).or_default() += 1;
                        fails.push(format!("{tag}: stream 5 port {} exe {}", rng.state(5), wr.state(5)));
                    }
                    if rng.state(5) != before_s5 {
                        mad_ticks += 1;
                    }
                    let (r1, r2) = (ev["rand"]["r1"].as_i64().unwrap() as i32, ev["rand"]["r2"].as_i64().unwrap() as i32);
                    let (a, b) = (lcg.next(), lcg.next());
                    if (a, b) != (r1, r2) {
                        *kinds.entry("libc".into()).or_default() += 1;
                        let seed = ev["rand"]["seed"].as_u64().unwrap() as u32;
                        let count = |stop: u32| {
                            let mut l = Lcg(seed);
                            (0..20_000).find(|_| {
                                let hit = l.0 == stop;
                                l.next();
                                hit
                            })
                        };
                        let mut w = Lcg(seed);
                        let exe = (0..20_000).find(|_| {
                            let mut t = Lcg(w.0);
                            let hit = (t.next(), t.next()) == (r1, r2);
                            w.next();
                            hit
                        });
                        let mut back = Lcg(lcg.0);
                        back.0 = back.0.wrapping_sub(2531011).wrapping_mul(0xb9b33155).wrapping_sub(2531011).wrapping_mul(0xb9b33155);
                        fails.push(format!("{tag}: libc draws port {:?} exe {exe:?}", count(back.0)));
                    }
                    if fails.len() == before_fails {
                        exact += 1;
                    }
                }
                _ => {}
            }
        }
    }
    if calls == 0 {
        return;
    }
    eprintln!("voice events: {voice_ticks} on-screen ticks checked, {voice_plays} lines picked");
    eprintln!("FRISSIT_Grwar passes: {grwars} replayed, {grwar_exact} exact; {finished} end countdowns into GroundWarFinished");
    eprintln!("battle ticks: {calls} replayed, {exact} exact ({heap_debris} with heap-garbage debris slots, {generated} passability map builds generated ({} needing the strip y >= 195 from the trace, {} with the next record's head and exact), {elevation_ulps} gun elevations a few ulps off; {fighter_ticks} with a fighter wave up, {bomb_ticks} with air-strike bombs falling, {mad_ticks} drawing for mad tanks); mismatch kinds {kinds:?}", gen_tail.0, gen_tail.1);
    if !fails.is_empty() {
        for x in fails.iter().take(std::env::var("IG2_SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(30)) {
            eprintln!("{x}");
        }
        panic!("{} mismatches", fails.len());
    }
}

/// Every recorded `WF::MI` call (exe 0x486d40) replayed through `War::mi` (re/rules/ground-ai.md).
#[test]
#[allow(clippy::needless_range_loop)]
fn ground_ai_matches_trace() {
    let Some(env) = env() else { return };
    let gr = ground_rules(&env.rules);
    let dir = std::env::var_os("IG2_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy();
            n.starts_with("grt-battle") || n.starts_with("grt-small") || n.starts_with("grt-ai") || n.starts_with("grt-transport") || n.starts_with("grt-laser")
        })
        .collect();
    files.sort();
    let mut fails: Vec<String> = Vec::new();
    let (mut calls, mut exact, mut inits, mut strikes, mut heap_calls) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut big_strikes = 0usize;
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    let mut plans = std::collections::BTreeMap::<(i32, i32), usize>::new();
    for path in files {
        let mut meta = Value::Null;
        let mut st = Regions::new();
        for line in std::io::BufReader::new(std::fs::File::open(&path).unwrap()).lines() {
            let ev: Value = serde_json::from_str(&line.unwrap()).unwrap();
            match ev["k"].as_str().unwrap() {
                "battle" => meta = ev,
                "init" => {
                    st.clear();
                    for (n, h) in ev["regions"].as_object().unwrap() {
                        st.insert(n.clone(), unhex(h.as_str().unwrap()));
                    }
                }
                "grwar" | "aifuzz" => apply(&mut st, &ev["pre"]),
                "ref" | "hor" | "order" | "gorder" => {
                    apply(&mut st, &ev["pre"]);
                    apply(&mut st, &ev["post"]);
                }
                "grwar_end" => apply(&mut st, &ev["post"]),
                "mi" => {
                    apply(&mut st, &ev["pre"]);
                    let before = st.clone();
                    apply(&mut st, &ev["post"]);
                    calls += 1;
                    let p = meta["p"].as_i64().unwrap() as i32;
                    let tag = format!("{} battle {} tick {}", path.file_name().unwrap().to_string_lossy(), ev["battle"], ev["tick"]);
                    let mut ctx = context(&meta);
                    let g = &ev["g"];
                    ctx.on_screen =
                        g["aktKep"].as_i64() == Some(3) && g["aktPlanet"].as_i64() == Some(p as i64) && g["belep"] == 0 && g["atlepes"] == 0;
                    let speed = ev["g"]["speed"].as_i64().unwrap() as i32;
                    let (mut field, mut planet) = battle_state(&before, p);
                    // `GetMITarget` reads a target's hp at the chooser's size − 1: past the end of a smaller
                    // group's array (heap memory the trace doesn't hold; ground-ai.md §8). A chooser that
                    // is bigger than some live enemy group can pick differently from the exe.
                    let overread: Vec<bool> = (0..field.units.len())
                        .map(|i| {
                            let u = &field.units[i];
                            field.alive[i]
                                && (0..field.units.len()).any(|j| field.alive[j] && field.units[j].owner != u.owner && field.units[j].hp.len() < u.hp.len())
                        })
                        .collect();
                    let s = serde_json::json!({ "tanks": before.get("tanks").map(|b| hexs(b)).unwrap_or_default() });
                    let mut world = world(&meta, &s);
                    let mut rng = Streams::default();
                    for (k, c) in before["seeds"].chunks(4).enumerate() {
                        rng.set_state(k, u32::from_le_bytes(c.try_into().unwrap()));
                    }
                    let m = before["planet"][0x23] as usize;
                    let mut lcg = Lcg(ev["rand"]["seed"].as_u64().unwrap_or(0) as u32);
                    let mut libc = || lcg.next();
                    let mut place = |_s: Option<(i32, i32)>| (false, (0, 0));
                    let mut war = War::new(&mut field, &mut planet, &mut world, &gr, &ctx, &mut rng, &mut libc, &mut place).with_map(&env.maps[m]);
                    war.mi(speed);
                    strikes += war.events.iter().filter(|e| matches!(e, GroundEvent::AirStrike { .. })).count();
                    big_strikes += war.events.iter().filter(|e| matches!(e, GroundEvent::AirStrike { big_ships: true, .. })).count();
                    drop(war);
                    let (want, _) = battle_state(&st, p);
                    for a in want.ai.iter().skip(1).filter(|a| a.strategy != 0) {
                        *plans.entry((a.strategy, a.phase)).or_default() += 1;
                    }
                    let n0 = fails.len();
                    let mut heap = false;
                    let fail = |kinds: &mut std::collections::BTreeMap<String, usize>, fails: &mut Vec<String>, kind: &str, msg: String| {
                        *kinds.entry(kind.to_string()).or_default() += 1;
                        fails.push(format!("{tag}: {msg}"));
                    };
                    for e in 1..19 {
                        if field.ai[e] != want.ai[e] {
                            fail(&mut kinds, &mut fails, "ai", format!("empire {e} ai port {:?} exe {:?}", field.ai[e], want.ai[e]));
                        }
                    }
                    for i in 0..field.units.len().min(want.units.len()) {
                        if !want.alive[i] {
                            continue;
                        }
                        let mut w = want.units[i].clone();
                        if field.units[i].last_hit.is_none() {
                            w.last_hit = None;
                        }
                        if field.units[i].rt.ai_home != w.rt.ai_home {
                            fail(&mut kinds, &mut fails, "home", format!("unit {i} home port {} exe {}", field.units[i].rt.ai_home, w.rt.ai_home));
                        }
                        let (gk, wk) = (tick_key(&field.units[i]), tick_key(&w));
                        if gk != wk && overread[i] && field.units[i].target != w.target {
                            heap = true;
                        } else if gk != wk {
                            let d: Vec<String> = gk
                                .split(' ')
                                .zip(wk.split(' '))
                                .filter(|(a, b)| a != b)
                                .map(|(a, b)| {
                                    let name: String = a.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
                                    *kinds.entry(format!("unit {name}")).or_default() += 1;
                                    format!("port {a} exe {b}")
                                })
                                .collect();
                            fails.push(format!("{tag}: unit {i}: {}", d.join("; ")));
                        }
                    }
                    let (gl, wl) = (list_keys(&field), list_keys(&want));
                    if gl != wl {
                        let d: Vec<String> = gl.iter().zip(&wl).filter(|(a, b)| a != b).map(|(a, b)| format!("
    port {a}
    exe  {b}")).take(3).collect();
                        fail(&mut kinds, &mut fails, "lists", format!("lists (len port {} exe {}){}", gl.len(), wl.len(), d.join("")));
                    }
                    let (r1, r2) = (ev["rand"]["r1"].as_i64().unwrap() as i32, ev["rand"]["r2"].as_i64().unwrap() as i32);
                    if (lcg.next(), lcg.next()) != (r1, r2) {
                        fail(&mut kinds, &mut fails, "libc", "libc draws differ".into());
                    }
                    if field.timers != want.timers {
                        fail(&mut kinds, &mut fails, "timers", format!("timers port {:?} exe {:?}", field.timers, want.timers));
                    }
                    let mut wr = Streams::default();
                    for (k, c) in st["seeds"].chunks(4).enumerate() {
                        wr.set_state(k, u32::from_le_bytes(c.try_into().unwrap()));
                    }
                    if wr.state(13) != u32::from_le_bytes(before["seeds"][52..56].try_into().unwrap()) {
                        inits += 1;
                    }
                    for s in [2, 13] {
                        if rng.state(s) != wr.state(s) {
                            let (mut a, mut k) = (u32::from_le_bytes(before["seeds"][4 * s..4 * s + 4].try_into().unwrap()), 0);
                            let mut exe_n = None;
                            while k < 2000 {
                                if a == wr.state(s) {
                                    exe_n = Some(k);
                                    break;
                                }
                                a = ig2_sim::rng::step(a);
                                k += 1;
                            }
                            fail(&mut kinds, &mut fails, "stream", format!("stream {s} port {} exe {} (exe draws {exe_n:?})", rng.state(s), wr.state(s)));
                        }
                    }
                    if fails.len() == n0 && heap {
                        heap_calls += 1;
                    } else if fails.len() == n0 {
                        exact += 1;
                    }
                }
                _ => {}
            }
        }
    }
    if calls == 0 {
        return;
    }
    eprintln!("WF::MI: {calls} calls replayed, {exact} exact, {heap_calls} with a target picked over heap garbage ({inits} drawing stream 13, {strikes} air strikes, {big_strikes} of them by big ships); (strategy, phase) seen {plans:?}; mismatch kinds {kinds:?}");
    if !fails.is_empty() {
        for x in fails.iter().take(std::env::var("IG2_SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(30)) {
            eprintln!("{x}");
        }
        panic!("{} mismatches", fails.len());
    }
}

/// The transporters (re/rules/ground-combat.md §11.16): every recorded dropship pass (`WF_Hordozo::Refresh2`,
/// exe 0x48e0a0; `hor`) replayed through `War::dropship`, and every landing order (`order`: `PlaceAllTanks`
/// 0x48c150, `PlaceTank` 0x48c2c0, `UnplaceTank` 0x4a4f70) through its port, and every ground-war message
/// (`gorder`: staged through the exe's `MULTIGET_Colony` 0x48a270) through `War::order`, from the exe's state
/// before the call: every unit, the projectile lists (a shot-down pick-up's debris), the retreat flags, call-in
/// timers and landing zone, the status cache, streams 0 and 2 and the libc sequence.
#[test]
fn transporters_match_trace() {
    let Some(env) = env() else { return };
    let gr = ground_rules(&env.rules);
    let dir = std::env::var_os("IG2_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut files: Vec<PathBuf> =
        rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("grt-")).collect();
    files.sort();
    let mut fails: Vec<String> = Vec::new();
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    let (mut passes, mut exact, mut orders, mut gorders, mut heap_debris) = (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    for path in files {
        let mut meta = Value::Null;
        let mut st = Regions::new();
        for line in std::io::BufReader::new(std::fs::File::open(&path).unwrap()).lines() {
            let ev: Value = serde_json::from_str(&line.unwrap()).unwrap();
            let k = ev["k"].as_str().unwrap();
            match k {
                "battle" => meta = ev,
                "init" => {
                    st.clear();
                    for (n, h) in ev["regions"].as_object().unwrap() {
                        st.insert(n.clone(), unhex(h.as_str().unwrap()));
                    }
                }
                "grwar" | "aifuzz" => apply(&mut st, &ev["pre"]),
                "grwar_end" => apply(&mut st, &ev["post"]),
                "mi" | "ref" => {
                    apply(&mut st, &ev["pre"]);
                    apply(&mut st, &ev["post"]);
                }
                "hor" | "order" | "gorder" => {
                    apply(&mut st, &ev["pre"]);
                    let before = st.clone();
                    apply(&mut st, &ev["post"]);
                    let p = meta["p"].as_i64().unwrap() as i32;
                    let tag = format!("{} battle {} tick {} {k} {}", path.file_name().unwrap().to_string_lossy(), ev["battle"], ev["tick"], ev["seq"]);
                    let g = &ev["g"];
                    let mut ctx = context(&meta);
                    ctx.on_screen =
                        g["aktKep"].as_i64() == Some(3) && g["aktPlanet"].as_i64() == Some(p as i64) && g["belep"] == 0 && g["atlepes"] == 0;
                    let (mut field, mut planet) = battle_state(&before, p);
                    let s = serde_json::json!({ "tanks": before.get("tanks").map(|b| hexs(b)).unwrap_or_default() });
                    let mut world = world(&meta, &s);
                    let mut rng = Streams::default();
                    for (k, c) in before["seeds"].chunks(4).enumerate() {
                        rng.set_state(k, u32::from_le_bytes(c.try_into().unwrap()));
                    }
                    let mut lcg = Lcg(ev["rand"]["seed"].as_u64().unwrap() as u32);
                    let m = before["planet"][0x23] as usize;
                    let before_fails = fails.len();
                    let mut voices = before.get("voices").map(|b| voices_of(b));
                    {
                        let mut libc = || lcg.next();
                        let mut place = |_s: Option<(i32, i32)>| (false, (0, 0));
                        let mut war = War::new(&mut field, &mut planet, &mut world, &gr, &ctx, &mut rng, &mut libc, &mut place).with_map(&env.maps[m]);
                        if let Some(v) = voices.as_mut() {
                            war = war.with_voices(v);
                        }
                        if k == "hor" {
                            passes += 1;
                            let i = ev["idx"].as_i64().unwrap() as i32;
                            let s0 = war.field.units[i as usize].transport;
                            if (ev["cc"].as_f64().unwrap() as i32) % 5 != 0 {
                                fails.push(format!("{tag}: ColonyCounter {} not a multiple of 5", ev["cc"]));
                            }
                            let lost = war.dropship(i);
                            let s1 = war.field.units[i as usize].transport;
                            let to = if lost { "lost".to_string() } else { format!("{s1:#x}") };
                            if s0 != s1 || lost {
                                *seen.entry(format!("{s0:#x}->{to}")).or_default() += 1;
                            }
                            if lost != (ev["ret"].as_i64() == Some(1)) {
                                fails.push(format!("{tag}: shot down port {lost} exe {}", ev["ret"]));
                            }
                        } else if k == "gorder" {
                            gorders += 1;
                            let (t, v) = (ev["type"].as_i64().unwrap() as i32, ints(&ev["vals"]));
                            *seen.entry(format!("gorder {t:#x}")).or_default() += 1;
                            let Some(ColonyOrder::Ground { order, .. }) = ColonyOrder::from_message(t, &v[1..]) else {
                                panic!("{tag}: not a ground-war message")
                            };
                            match (order, ev["freed"].as_array()) {
                                // The exe's victim bursts read the freed detonator's planet (`War::detonate`).
                                (GroundOrder::Detonate { unit, big }, Some(reads)) if war.field.alive.get(unit as usize) == Some(&true) => {
                                    let reads: Vec<i32> = reads.iter().map(|r| r.as_i64().unwrap() as i32).collect();
                                    if reads.iter().any(|&r| r != p) {
                                        *seen.entry("detonate, freed record reused".into()).or_default() += 1;
                                    }
                                    let mut it = reads.iter().copied();
                                    let mut used = 0;
                                    war.detonate(unit, big, &mut || {
                                        used += 1;
                                        it.next().unwrap_or(i32::MIN)
                                    });
                                    if used != reads.len() {
                                        fails.push(format!("{tag}: detonate tested the freed record {used} times, exe {}", reads.len()));
                                    }
                                }
                                _ => war.order(v[0], order),
                            }
                        } else {
                            orders += 1;
                            let c = ints(&ev["cmd"]);
                            *seen.entry(format!("order {}", c[0])).or_default() += 1;
                            match c[0] {
                                0 => war.place_all_tanks(c[1], c[2], c[3], c[4] != 0),
                                1 => war.place_tank(c[1], c[2], c[3]),
                                _ => war.unplace_tank(c[1]),
                            }
                        }
                    }
                    if let (Some(v), Some(w)) = (&voices, st.get("voices").map(|b| voices_of(b))) {
                        if *v != w {
                            *kinds.entry("voices".into()).or_default() += 1;
                            fails.push(format!("{tag}: voices differ"));
                        }
                    }
                    let (want, _) = battle_state(&st, p);
                    for i in 0..field.units.len().max(want.units.len()) {
                        let ga = field.alive.get(i).copied().unwrap_or(false);
                        let wa = want.alive.get(i).copied().unwrap_or(false);
                        if ga != wa {
                            fails.push(format!("{tag}: unit {i} alive port {ga} exe {wa}"));
                            *kinds.entry("alive".into()).or_default() += 1;
                            continue;
                        }
                        if !ga {
                            continue;
                        }
                        let mut w = want.units[i].clone();
                        let g2 = &field.units[i];
                        if g2.last_hit.is_none() {
                            w.last_hit = None;
                        }
                        if w.flags & flag::TURRET != 0 {
                            w.rt.step = g2.rt.step;
                        }
                        let (gk, wk) = (tick_key(g2), tick_key(&w));
                        if gk != wk {
                            let d: Vec<String> = gk
                                .split(' ')
                                .zip(wk.split(' '))
                                .filter(|(a, b)| a != b)
                                .map(|(a, b)| {
                                    let name: String = a.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
                                    *kinds.entry(name).or_default() += 1;
                                    format!("port {a} exe {b}")
                                })
                                .collect();
                            fails.push(format!("{tag}: unit {i}: {}", d.join("; ")));
                        }
                    }
                    let (gl, wl) = (list_keys(&field), list_keys(&want));
                    if gl != wl && debris_only(&gl, &wl) {
                        heap_debris += 1;
                    } else if gl != wl {
                        *kinds.entry("lists".into()).or_default() += 1;
                        fails.push(format!("{tag}: lists (len port {} exe {})", gl.len(), wl.len()));
                    }
                    if (field.retreat, field.timers, field.landing.is_some()) != (want.retreat, want.timers, want.landing.is_some()) {
                        *kinds.entry("field".into()).or_default() += 1;
                        fails.push(format!("{tag}: retreat/timers/landing port {:?} {:?} {} exe {:?} {:?} {}", field.retreat, field.timers,
                            field.landing.is_some(), want.retreat, want.timers, want.landing.is_some()));
                    }
                    if field.status_valid != want.status_valid {
                        *kinds.entry("status".into()).or_default() += 1;
                        fails.push(format!("{tag}: status cache port {} exe {}", field.status_valid, want.status_valid));
                    }
                    for s in [0, 2] {
                        let w = u32::from_le_bytes(st["seeds"][4 * s..4 * s + 4].try_into().unwrap());
                        if rng.state(s) != w {
                            *kinds.entry(format!("stream{s}")).or_default() += 1;
                            fails.push(format!("{tag}: stream {s} port {} exe {w}", rng.state(s)));
                        }
                    }
                    check_libc(&tag, &mut lcg, &ev["rand"], &mut fails);
                    if fails.len() == before_fails {
                        exact += 1;
                    }
                }
                _ => {}
            }
        }
    }
    if passes + orders + gorders == 0 {
        return;
    }
    eprintln!("dropships: {passes} passes, {orders} landing orders and {gorders} ground-war messages replayed, {exact} exact ({heap_debris} with heap-garbage debris slots); transitions {seen:?}; mismatch kinds {kinds:?}");
    if !fails.is_empty() {
        for x in fails.iter().take(std::env::var("IG2_SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(30)) {
            eprintln!("{x}");
        }
        panic!("{} mismatches", fails.len());
    }
}
