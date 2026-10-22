//! Replays real-time battle ticks recorded from the running GOG `ig2.exe` (tools/trace/run_spwc.py):
//! every `SpwC::SpaceWarFrissit` call (exe 0x56dff0) with the battle's memory before and after.
//!
//! Each tick is decoded from the exe's memory (units, fighters, planet guns, the projectile pools, the AI
//! squads, the strategic records the battle touches, stream 6), stepped by the port, and compared with the
//! exe's state after the tick. Per-tick resync: a mismatch stays local. Rules and switches come from the
//! trace (the live weapon table and the battle's globals). Traces: `_local/traces/spwc-*.jsonl` (game
//! state: never committed; `IG2_TRACE_DIR` points elsewhere). Skips without them.
//!
//! `spwo-*.jsonl` (`run_spwc.py --orders N`) add the human's orders between ticks: each staged through the exe's own
//! `MULTIGET_SpaceWar` (spec §7.12) with the bytes it changed; the replay applies `Battle::order` / `Battle::arm_all`
//! to the decoded state and compares the same way.

use std::collections::{BTreeMap, HashMap};
use std::io::BufRead;
use std::path::PathBuf;

use ig2_sim::rng::Streams;
use ig2_sim::spacebattle::*;
use ig2_sim::spacebattle_raw::*;
use serde_json::Value;
use ig2_sim::spacewar::{CombatFleet, CombatWorld, Design, FleetTotals, Ship, Vehicle};

fn traces(prefix: &str) -> Vec<PathBuf> {
    // `IG2_TRACE_DIR` overrides the checkout's `_local/traces` (a worktree without its own traces).
    let dir = std::env::var_os("IG2_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
    let mut v: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix))
        .collect();
    v.sort();
    v
}

/// A dump's region diff or full regions onto the memory.
trait Apply {
    fn apply(&mut self, d: &Value);
}

impl Apply for Mem {
    fn apply(&mut self, d: &Value) {
        for (k, v) in d.as_object().unwrap() {
            if v.get("gone").is_some() {
                self.r.remove(k);
            } else if let Some(full) = v.get("full") {
                self.r.insert(k.clone(), (ptr(v["p"].as_str().unwrap()), unhex(full.as_str().unwrap())));
            } else {
                let e = self.r.get_mut(k).unwrap();
                for run in v["r"].as_array().unwrap() {
                    let off = run[0].as_u64().unwrap() as usize;
                    let b = unhex(run[1].as_str().unwrap());
                    e.1[off..off + b.len()].copy_from_slice(&b);
                }
            }
        }
    }
}

fn statics(v: &Value) -> Statics {
    let mut types = HashMap::new();
    for (k, t) in v["types"].as_object().unwrap() {
        let b = unhex(t.as_str().unwrap());
        types.insert(ptr(k), ShipType {
            scale: f32_at(&b, 0x10),
            top_speed: f32_at(&b, 0x14),
            accel: f32_at(&b, 0x1c),
            turn: f32_at(&b, 0x24),
            size_xy: i32_at(&b, 0x28),
            size_z: i32_at(&b, 0x2c),
        });
    }
    let mut designs = vec![Vec::new(); 19];
    for (k, d) in v["designs"].as_object().unwrap() {
        designs[k.parse::<usize>().unwrap()] = unhex(d.as_str().unwrap()).chunks(0x84).map(Design::from_bytes).collect();
    }
    let building_hp = v.get("buildingHp").map_or(Vec::new(), |b| unhex(b.as_str().unwrap()).chunks(0x24).map(|c| i32_at(c, 0)).collect());
    Statics { weapons: unhex(v["weapons"].as_str().unwrap()), building_hp, types, designs }
}


/// The strategic records a battle touches, as the trace holds them.
struct RawWorld {
    w: CombatWorld,
    planet: Option<Vec<u8>>,
    /// The battle planet's buildings (0x20 each, slot 0 unused) and the types' hit points.
    buildings: Option<Vec<u8>>,
    building_hp: Vec<i32>,
    centres: Vec<(i32, i32)>,
    buildings_hit: usize,
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

fn world(mem: &Mem, st: &Statics) -> RawWorld {
    let ships = mem.find("hajok@").unwrap().1.chunks(6).map(|c| Ship { fleet: i16_at(c, 0), design: i16_at(c, 2) }).collect();
    let fleets = mem.find("flottak@").unwrap().1.chunks(0x80).map(fleet_of).collect();
    let nt = i32_at(mem.get("tankcount"), 0).max(0) as usize;
    let vehicles = mem.get("tankok").chunks(8).take(nt + 1).map(|c| Vehicle { fleet: i16_at(c, 0), design: i16_at(c, 2), count: i32_at(c, 4) }).collect();
    let mut w = CombatWorld { fleets, ships, vehicles, designs: st.designs.clone(), ..Default::default() };
    for (h, &k) in w.human_controlled.iter_mut().zip(mem.global(0xbae560)) {
        *h = k == 1;
    }
    let cx = mem.global(0xdb2848);
    let cy = mem.global(0xdb2898);
    let planet = mem.r.iter().find(|(k, _)| k.starts_with("planet")).map(|(_, v)| v.1.clone());
    let buildings = mem.find("buildings").map(|(_, b)| b.to_vec());
    RawWorld {
        w,
        planet,
        buildings,
        building_hp: st.building_hp.clone(),
        centres: (0..19).map(|i| (i32_at(cx, 4 * i), i32_at(cy, 4 * i))).collect(),
        buildings_hit: 0,
    }
}

impl BattleWorld for RawWorld {
    fn ship_fleet(&self, ship: i32) -> i32 {
        self.w.ships[ship as usize].fleet as i32
    }
    fn ship_attack(&self, owner: i32, ship: i32) -> i32 {
        let d = self.w.ships[ship as usize].design;
        self.w.designs[owner as usize].get(d as usize).map_or(0, |d| d.attack)
    }
    fn delete_ship(&mut self, ship: i32) {
        self.w.delete_ship(ship as usize);
    }
    /// A taken-over asteroid that dies names the fleet in the unused `Hajok[0]` (garbage, past the table):
    /// the exe read some memory there; the replay treats it as not empty.
    fn fleet_empty(&self, fleet: i32) -> bool {
        self.w.fleets.get(fleet as usize).is_some_and(|f| f.totals.ships == 0 && f.totals.fighters == 0)
    }
    fn delete_fleet(&mut self, fleet: i32) {
        self.w.delete_fleet(fleet as usize);
    }
    fn fleet_owner(&self, fleet: i32) -> i32 {
        self.w.fleets.get(fleet as usize).map_or(0xff, |f| f.owner as i32)
    }
    fn delete_fighters(&mut self, fleet: i32, design: i32, n: i32) {
        self.w.delete_vehicles(fleet as i16, design as i16, n);
    }
    fn remove_planet_ship(&mut self, _planet: i32, k: i32) {
        let p = self.planet.as_mut().expect("battle planet record");
        let n = i32_at(p, 0x114);
        if k < n {
            for j in k..=n - 1 {
                let v = i16_at(p, 0xfe + 2 * (j as usize + 1));
                p[0xfe + 2 * j as usize..0x100 + 2 * j as usize].copy_from_slice(&v.to_le_bytes());
            }
        }
        p[0x114..0x118].copy_from_slice(&(n - 1).to_le_bytes());
    }
    /// 0x474160, then `FoldTalalat`'s test (which reads the slot's type again, 0 once `EpuletDelete` ran).
    fn damage_building(&mut self, _planet: i32, slot: i32, damage: i32) -> bool {
        self.buildings_hit += 1;
        let b = self.buildings.as_mut().expect("battle planet buildings");
        let o = 0x20 * slot as usize;
        let hp = |t: u8| self.building_hp.get(t as usize).copied().unwrap_or(0);
        let cur = i32_at(b, o + 4);
        if cur < hp(b[o]) {
            let d = damage.wrapping_add(cur);
            b[o + 4..o + 8].copy_from_slice(&d.to_le_bytes());
            if hp(b[o]) <= d {
                b[o] = 0;
            }
        }
        hp(b[o]) <= i32_at(b, o + 4)
    }
    fn retreat_fleet(&mut self, fleet: i32) {
        let o = self.w.fleets[fleet as usize].owner as usize;
        if o != 0xff {
            self.w.retreat(fleet as usize, self.centres[o]);
        }
    }
}

/// Fields that hold uninitialised bytes in the exe (the fourth float of copied vectors) are cleared.
fn tidy(b: &mut Battle) {
    for u in &mut b.units {
        // The exe keeps the array's stale tail; the trace holds the live part (at least one entry).
        let live = (u.ship_count.max(1) as usize).min(u.ships.len());
        u.ships.truncate(live);
        for v in [&mut u.pos, &mut u.dest, &mut u.step, &mut u.vel, &mut u.heading, &mut u.mid_heading, &mut u.plan_pos] {
            v.w = 0.0;
        }
        u.cell[3] = 0;
        u.step_cell[3] = 0;
        for w in &mut u.waypoints {
            w[3] = 0;
        }
    }
    for f in &mut b.fighters {
        for v in [&mut f.pos, &mut f.dest, &mut f.vel, &mut f.dir] {
            v.w = 0.0;
        }
    }
    for g in &mut b.guns {
        g.pos.w = 0.0;
    }
    for p in &mut b.pools {
        for s in &mut p.shots {
            for v in [&mut s.pos, &mut s.vel, &mut s.dir, &mut s.sprite_pos] {
                v.w = 0.0;
            }
            // Only weapon 11 reads its sprite back.
            if s.weapon != 0xb {
                s.sprite_pos = V4::default();
                s.sprite_size = 0.0;
            }
        }
    }
    for s in &mut b.sides {
        for q in &mut s.squads {
            q.centre.w = 0.0;
        }
    }
}

/// Field-level differences of two debug renderings (first few).
fn diff_debug(label: &str, a: &impl std::fmt::Debug, b: &impl std::fmt::Debug, out: &mut Vec<String>) {
    let (sa, sb) = (format!("{a:#?}"), format!("{b:#?}"));
    if sa == sb {
        return;
    }
    let la: Vec<&str> = sa.lines().collect();
    let lb: Vec<&str> = sb.lines().collect();
    let mut n = 0;
    for (i, (x, y)) in la.iter().zip(&lb).enumerate() {
        if x != y {
            // The enclosing field: the nearest line above that opens a struct.
            let parent = la[..i].iter().rev().find(|l| l.trim_end().ends_with('{')).map_or("", |l| l.trim());
            out.push(format!("{label} {parent} {}: port | exe {}", x.trim(), y.trim()));
            n += 1;
            if n >= 4 {
                break;
            }
        }
    }
    if la.len() != lb.len() {
        out.push(format!("{label}: {} vs {} lines", la.len(), lb.len()));
    }
}

fn compare(port: &Battle, exe: &Battle, out: &mut Vec<String>) {
    if port.tick != exe.tick {
        out.push(format!("tick {} vs {}", port.tick, exe.tick));
    }
    for (i, (a, b)) in port.units.iter().zip(&exe.units).enumerate() {
        if a != b {
            diff_debug(&format!("unit {i}"), a, b, out);
        }
    }
    for (i, (a, b)) in port.fighters.iter().zip(&exe.fighters).enumerate() {
        if a != b {
            diff_debug(&format!("fighter {i}"), a, b, out);
        }
    }
    for (i, (a, b)) in port.guns.iter().zip(&exe.guns).enumerate() {
        if a != b {
            diff_debug(&format!("gun {i}"), a, b, out);
        }
    }
    for (f, (a, b)) in port.pools.iter().zip(&exe.pools).enumerate() {
        for (s, (x, y)) in a.shots.iter().zip(&b.shots).enumerate() {
            let live = x.shooter.is_some() || y.shooter.is_some();
            if (live && x != y) || x.shooter != y.shooter || x.owner != y.owner {
                diff_debug(&format!("shot {f}/{s}"), x, y, out);
            }
        }
    }
    for (e, (a, b)) in port.sides.iter().zip(&exe.sides).enumerate() {
        if a != b {
            diff_debug(&format!("side {e}"), a, b, out);
        }
    }
    let scalars = |b: &Battle| {
        (b.ai_timer, b.aim_timer, b.end_timer, b.pod_timer, b.winner, b.victory, b.human_retreated, b.ai_retreated,
         (b.f2d84, b.f2d86, b.finished), b.counts)
    };
    if scalars(port) != scalars(exe) {
        diff_debug("scalars", &scalars(port), &scalars(exe), out);
    }
    if port.present != exe.present {
        diff_debug("present", &port.present, &exe.present, out);
    }
    if port.participants != exe.participants {
        diff_debug("participants", &port.participants, &exe.participants, out);
    }
}

struct Run {
    weapons: [usize; 64],
    kinds: [usize; 8],
    /// Feature coverage: name → ticks (or events) seen.
    cover: BTreeMap<&'static str, usize>,
    buildings: usize,
    steps: usize,
    bad: usize,
    hits: usize,
    shots: usize,
    lost: usize,
    over: usize,
    /// Staged orders by message type.
    orders: BTreeMap<i32, usize>,
}

trait Shots<'a> {
    fn flatten_shots(self) -> Box<dyn Iterator<Item = &'a Shot> + 'a>;
}
impl<'a, I: Iterator<Item = &'a Pool> + 'a> Shots<'a> for I {
    fn flatten_shots(self) -> Box<dyn Iterator<Item = &'a Shot> + 'a> {
        Box::new(self.flat_map(|p| p.shots.iter()))
    }
}

fn replay(path: &PathBuf, run: &mut Run) {
    let mut mem = Mem::default();
    let mut st: Option<Statics> = None;
    for line in std::io::BufReader::new(std::fs::File::open(path).unwrap()).lines() {
        let e: Value = serde_json::from_str(&line.unwrap()).unwrap();
        match e["k"].as_str().unwrap() {
            "init" => {
                mem = Mem::default();
                mem.apply(&e["regions"]);
                st = Some(statics(&e["statics"]));
            }
            "between" => mem.apply(&e["d"]),
            "step" => {
                let st = st.as_ref().unwrap();
                let mut before = decode(&mem, st);
                let dbg = std::env::var("SPWC_DEBUG").ok().and_then(|v| v.parse::<i64>().ok());
                if dbg.is_some_and(|t| e["n"].as_i64() == Some(t)) {
                    for (i, u) in before.units.iter().enumerate() {
                        if i == 27 {
                            eprintln!("pre unit {i}: owner {} kind {} hp {} dmg {} sh {}/{} n {} ships {:?} lightning {:?} retreat {} speed {}", u.owner, u.kind, u.hp, u.damage, u.shield_used, u.shield, u.ship_count, u.ships, u.lightning, u.retreating, u.speed);
                        }
                    }
                }
                let (rules, words) = rules(&mem, st);
                let mut rng = Streams::default();
                let seeds = mem.get("seeds").to_vec();
                for i in 0..21 {
                    rng.set_state(i, u32_at(&seeds, 4 * i));
                }
                let mut wd = world(&mem, st);
                let mut events = Vec::new();
                let over = {
                    let mut t = Tick { rules: &rules, weapon_words: &words, rng: &mut rng, world: &mut wd, events: &mut events };
                    before.step(&mut t)
                };
                mem.apply(&e["d"]);
                let mut after = decode(&mem, st);
                let mut errs = Vec::new();
                tidy(&mut before);
                tidy(&mut after);
                compare(&before, &after, &mut errs);
                strategic(&mem, st, &rng, &wd, &mut errs);
                run.buildings += wd.buildings_hit;
                run.steps += 1;
                let exe = &after;
                let mut see = |k: &'static str, c: bool| {
                    if c {
                        *run.cover.entry(k).or_default() += 1;
                    }
                };
                see("multi-ship units", exe.units.iter().any(|u| u.alive && u.ship_count > 1));
                see("fighters", exe.fighters.iter().any(|f| f.alive));
                see("planet guns", exe.guns.iter().any(|g| g.building != 0));
                see("retreating", exe.units.iter().any(|u| u.retreating) || exe.fighters.iter().any(|f| f.retreating));
                see("cloaked", exe.units.iter().any(|u| u.cloaked));
                see("jammed", exe.units.iter().any(|u| u.jammed));
                see("disabled", exe.units.iter().any(|u| u.disabled > 0));
                see("taken over", exe.units.iter().any(|u| u.taken > 0));
                see("lightning", exe.units.iter().any(|u| u.lightning.is_some()));
                see("beam attached", exe.pools.iter().flatten_shots().any(|x| x.shooter.is_some() && x.attached));
                see("astray", exe.pools.iter().flatten_shots().any(|x| x.shooter.is_some() && x.astray));
                see("shields", exe.units.iter().any(|u| u.shield_used > 0));
                see("scenery", exe.units.iter().any(|u| u.alive && u.owner == 0));
                see("attack orders", exe.units.iter().any(|u| u.order == order::ATTACK));
                see("squads", exe.sides.iter().any(|x| !x.squads.is_empty()));
                for x in exe.pools.iter().flatten_shots().filter(|x| x.shooter.is_some()) {
                    run.weapons[x.weapon as usize & 63] += 1;
                    run.kinds[x.kind as usize & 7] += 1;
                }
                run.hits += events.iter().filter(|x| matches!(x, BattleEvent::Hit { .. })).count();
                run.shots += events.iter().filter(|x| matches!(x, BattleEvent::Fired { .. })).count();
                run.lost += events.iter().filter(|x| matches!(x, BattleEvent::ShipLost { .. })).count();
                run.over += over as usize;
                if !errs.is_empty() {
                    run.bad += 1;
                    if run.bad <= 6 {
                        eprintln!("{} battle {} tick {}: {:#?}", path.file_name().unwrap().to_string_lossy(), e["battle"], e["n"], errs);
                        if std::env::var_os("SPWC_DEBUG").is_some() {
                            eprintln!("events: {events:?}");
                            let mut prev = Mem::default();
                            std::mem::swap(&mut prev, &mut mem);
                            mem = prev;
                        }
                    }
                }
            }
            "order" => {
                let st = st.as_ref().unwrap();
                let mut before = decode(&mem, st);
                let (rules, words) = rules(&mem, st);
                let mut rng = Streams::default();
                let seeds = mem.get("seeds").to_vec();
                for i in 0..21 {
                    rng.set_state(i, u32_at(&seeds, 4 * i));
                }
                let mut wd = world(&mem, st);
                let mut events = Vec::new();
                let ty = e["type"].as_i64().unwrap() as i32;
                let v: Vec<i32> = e["vals"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap() as i32).collect();
                let order = staged_order(ty, &v);
                match order {
                    Some(Order::ArmAll) => before.arm_all(&rules.weapons, &mut rng, v[0]),
                    Some(o) => {
                        let mut t = Tick { rules: &rules, weapon_words: &words, rng: &mut rng, world: &mut wd, events: &mut events };
                        before.order(&mut t, v[0], o);
                    }
                    None => {}
                }
                mem.apply(&e["d"]);
                let mut after = decode(&mem, st);
                let mut errs = Vec::new();
                tidy(&mut before);
                tidy(&mut after);
                compare(&before, &after, &mut errs);
                strategic(&mem, st, &rng, &wd, &mut errs);
                *run.orders.entry(ty).or_default() += 1;
                if order.is_none() {
                    *run.orders.entry(-1).or_default() += 1;
                }
                if !errs.is_empty() {
                    run.bad += 1;
                    if run.bad <= 6 {
                        eprintln!("{} battle {} tick {} order {ty:#x} {v:?}: {:#?}", path.file_name().unwrap().to_string_lossy(), e["battle"], e["n"], errs);
                    }
                }
            }
            _ => {}
        }
    }
}

/// A staged `MULTIGET_SpaceWar` message as the port's order (spec §7.12); `None` = one the handler ignores (a target
/// of −1). `v` = the `Multi_Get` out-parameters in order (the issuer first).
fn staged_order(ty: i32, v: &[i32]) -> Option<Order> {
    let u = |i: usize| v[i] as usize;
    Some(match ty {
        0x70 => Order::Move { unit: u(4), x: f32::from_bits(v[1] as u32), y: f32::from_bits(v[2] as u32), z: f32::from_bits(v[3] as u32) },
        0x71 if v[2] != -1 => Order::Attack { unit: u(1), target: u(2) },
        0x72 => Order::Retreat,
        0x73 if v[2] != -1 => Order::AttackGun { unit: u(1), gun: u(2) },
        0x74 if v[2] != -1 => Order::GunTarget { gun: u(1), unit: u(2) },
        0x75 => Order::ArmAll,
        0x76 => Order::Explode { unit: u(2) },
        0x77 => Order::FighterStance { side: u(2), value: v[1] },
        0x44e => Order::DirectFire { unit: u(1), target: u(2), mount: u(3) },
        0x44f => Order::Weapons { unit: u(1), keep: v[2] as u32, set: v[3] as u32 },
        _ => return None,
    })
}

/// The streams and the strategic records after a step or an order, against the exe's.
fn strategic(mem: &Mem, st: &Statics, rng: &Streams, wd: &RawWorld, errs: &mut Vec<String>) {
    let seeds = mem.get("seeds");
    for i in 0..21 {
        if rng.state(i) != u32_at(seeds, 4 * i) {
            errs.push(format!("stream {i}"));
        }
    }
    let want = world(mem, st);
    if wd.w.ships != want.w.ships {
        errs.push("Hajok".into());
    }
    if wd.w.vehicles[..] != want.w.vehicles[..wd.w.vehicles.len().min(want.w.vehicles.len())] {
        errs.push("Tankok".into());
    }
    for (f, (a, b)) in wd.w.fleets.iter().zip(&want.w.fleets).enumerate() {
        if a.owner != b.owner || (b.owner != 0xff && (a.totals != b.totals || a.x != b.x || a.y != b.y)) {
            errs.push(format!("fleet {f}"));
        }
    }
    if wd.planet != want.planet {
        errs.push("planet".into());
    }
    let types = |b: &Option<Vec<u8>>| b.as_ref().map(|b| b.chunks(0x20).map(|c| (c[0], i32_at(c, 4))).collect::<Vec<_>>());
    if types(&wd.buildings) != types(&want.buildings) {
        errs.push("buildings".into());
    }
}

#[test]
fn battle_ticks_match_trace() {
    let mut files = traces("spwc-");
    files.extend(traces("spwo-"));
    if files.is_empty() {
        return;
    }
    let mut run = Run { weapons: [0; 64], kinds: [0; 8], cover: BTreeMap::new(), buildings: 0, steps: 0, bad: 0, hits: 0, shots: 0, lost: 0, over: 0, orders: BTreeMap::new() };
    for p in &files {
        replay(p, &mut run);
    }
    eprintln!("coverage (ticks): {:?}", run.cover);
    eprintln!("projectile-ticks by weapon: {:?}", run.weapons.iter().enumerate().filter(|x| *x.1 > 0).collect::<Vec<_>>());
    eprintln!("projectile-ticks by kind: {:?}", run.kinds);
    eprintln!("orders by message (−1: ignored by the handler): {:x?}", run.orders);
    eprintln!(
        "spwc: {} ticks, {} shots fired, {} hits, {} ships lost, {} building hits, {} end checks true, {} mismatches",
        run.steps, run.shots, run.hits, run.lost, run.buildings, run.over, run.bad
    );
    assert_eq!(run.bad, 0);
}

// ------------------------------------------------------------------------------------------------
// Setup and end (spec §7.10, §7.11): `presetup`/`setup` snapshots around `SpaceWarIndit`, `left` after the
// battle screen.

/// The formations and the compiled ship-type table from the user's install (`IG2_INSTALL_DIR`), or None.
fn install_data() -> Option<([Vec<FormationSlot>; 2], Vec<ShipType>)> {
    let inst = ig2_formats::Install::from_env(ig2_formats::text::Lang::En)?.ok()?;
    let load = |n: usize| -> Option<Vec<FormationSlot>> {
        let b = inst.read(&format!("dataraw/spacewar/felallas{n}.bin")).ok()?;
        Some(ig2_formats::formation::parse(&b).ok()?.iter().map(|s| FormationSlot::from_record(s.cell, s.kind, &s.extra)).collect())
    };
    let rules = ig2_rules::Rules::from_install(&inst).ok()?;
    let types = rules
        .ship_types
        .iter()
        .map(|t| ShipType { scale: t.scale, top_speed: t.top_speed, accel: t.accel, turn: t.turn, size_xy: t.size_xy, size_z: t.size_z })
        .collect();
    Some(([load(0)?, load(1)?], types))
}

fn ship_types(b: &[u8]) -> Vec<ShipType> {
    b.chunks_exact(0x30)
        .map(|r| ShipType {
            scale: f32_at(r, 0x10),
            top_speed: f32_at(r, 0x14),
            accel: f32_at(r, 0x1c),
            turn: f32_at(r, 0x24),
            size_xy: i32_at(r, 0x28),
            size_z: i32_at(r, 0x2c),
        })
        .collect()
}

/// The slot's fields `InitSpaceWar` keeps, from its memory before the setup.
fn slot_before(o: &[u8]) -> Battle {
    let mut b = Battle::slot();
    b.ai_timer = f32_at(o, 0xc40);
    b.aim_timer = i32_at(o, 0xc44);
    b.winner = i32_at(o, 0xc48);
    b.end_period = i32_at(o, 0xc58);
    b.planet_pos = [i32_at(o, 0x84c), i32_at(o, 0x850), i32_at(o, 0x854)];
    b.planet_size = i32_at(o, 0x858);
    for k in 0..19 {
        b.story_retreat[k] = i32_at(o, 0xe34 + 4 * k);
    }
    b
}

/// Bytes the exe leaves uninitialised at setup are cleared on both sides (on top of `tidy`).
fn tidy_setup(b: &mut Battle) {
    tidy(b);
    for u in &mut b.units {
        // +0x194 is not written; waypoints beyond the count come from fresh memory; scenery has no model.
        u.attacker_dist2 = 0.0;
        for w in u.waypoints.iter_mut().skip(u.waypoint_count.max(0) as usize) {
            *w = [0; 4];
        }
        if u.owner == 0 {
            u.model = 0;
        }
    }
    for f in &mut b.fighters {
        f.turn = 0.0;
    }
    for g in &mut b.guns {
        if g.weapon < 1 {
            (g.timer, g.timer_set, g.reload) = (0, 0, 0);
        }
    }
    // A fresh pool's slots are free (`Lovedekinit` clears only the shooter): checked separately.
    b.pools.clear();
}

struct SetupRun {
    setups: usize,
    joins: usize,
    units: usize,
    merged: usize,
    fighters: usize,
    guns: usize,
    asteroids: usize,
    planet_ships: usize,
    bad: usize,
    ends: usize,
    undefined: usize,
}

fn replay_setups(path: &PathBuf, forms: &[Vec<FormationSlot>; 2], compiled: &[ShipType], run: &mut SetupRun) {
    let mut first = true;
    let mut pre: Option<Value> = None;
    let mut last: Option<Battle> = None;
    let mut mem = Mem::default();
    let mut st: Option<Statics> = None;
    for line in std::io::BufReader::new(std::fs::File::open(path).unwrap()).lines() {
        let e: Value = serde_json::from_str(&line.unwrap()).unwrap();
        match e["k"].as_str().unwrap() {
            "presetup" => {
                // Before a session's first battle the live ship-type table is the compiled one (ig2-rules).
                if first {
                    first = false;
                    let mut a = Mem::default();
                    a.apply(&e["regions"]);
                    if ship_types(a.global(0x91f488)) != compiled {
                        run.bad += 1;
                        eprintln!("{}: the ship-type table differs from ig2-rules", path.display());
                    }
                }
                pre = Some(e);
            }
            "setup" => {
                let p = pre.take().expect("presetup before setup");
                let mut a = Mem::default();
                a.apply(&p["regions"]);
                let mut b = Mem::default();
                b.apply(&e["regions"]);
                let st = statics(&e["statics"]);
                let errs = check_setup(&p, &a, &b, &st, forms, run);
                run.setups += 1;
                if !errs.is_empty() {
                    run.bad += 1;
                    if run.bad <= 6 {
                        eprintln!("{} setup {}: {:#?}", path.file_name().unwrap().to_string_lossy(), e["setup"], errs);
                    }
                }
            }
            // The last tick's battle, for the end check.
            "init" => {
                mem = Mem::default();
                mem.apply(&e["regions"]);
                st = Some(statics(&e["statics"]));
            }
            "between" | "step" => {
                mem.apply(&e["d"]);
                if let Some(st) = &st {
                    last = Some(decode(&mem, st));
                }
            }
            "left" => {
                // Natural ends only (the tracer abandons long battles by closing the slot itself).
                if let Some(mut b) = last.take() {
                    if b.finished && e["slots"].as_i64() == Some(0) {
                        let r = b.finish();
                        run.ends += 1;
                        let ok = r.winner as i64 == e["kiNyert"].as_i64().unwrap()
                            && !r.loser_retreated == (e["vesztes"].as_i64() == Some(0))
                            && r.human_took_part == (e["you"].as_i64() != Some(0))
                            && r.participants.len() as i64 == e["num"].as_i64().unwrap()
                            && !b.active;
                        if !ok {
                            run.bad += 1;
                            eprintln!("end of battle {}: port {r:?} vs exe {e}", e["battle"]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn check_setup(p: &Value, a: &Mem, b: &Mem, st: &Statics, forms: &[Vec<FormationSlot>; 2], run: &mut SetupRun) -> Vec<String> {
    let mut errs = Vec::new();
    let slot = p["slot"].as_u64().unwrap() as usize;
    let join = p["join"].as_bool().unwrap();
    // The battle as it was (a join) or the slot's kept fields (a new battle); the slots before a new one are busy.
    let mut slots: Vec<Battle> = (0..10).map(|_| Battle::slot()).collect();
    for s in slots.iter_mut().take(slot) {
        s.active = true;
    }
    slots[slot] = if join { decode(a, &statics(&p["statics"])) } else { slot_before(a.get("obj")) };
    // Strategic state: from after the call (SpaceWarIndit only clears orders and rolls the story and the
    // diplomatic incident before SpaceWar_NewBattle, which changes none of it); stream 6 from before.
    let g = |addr: u32| b.global(addr);
    let sp = g(0xbaa1c0);
    let planet = i32_at(sp, 0x24);
    let story = g(0xf26150);
    let mut human_controlled = [false; 19];
    for (h, &k) in human_controlled.iter_mut().zip(g(0xbae560)) {
        *h = k == 1;
    }
    let fleet_records: Vec<SetupFleet> = b
        .find("flottak@")
        .unwrap()
        .1
        .chunks(0x80)
        .map(|r| SetupFleet { owner: r[0x15], story_id: i16_at(r, 0x16), hp: i32_at(r, 0x6c).wrapping_add(i32_at(r, 0x70)), x: f32_at(r, 0x30), y: f32_at(r, 0x34) })
        .collect();
    let ships: Vec<SetupShip> = b.find("hajok@").unwrap().1.chunks(6).map(|c| SetupShip { fleet: i16_at(c, 0), design: i16_at(c, 2), experience: c[4] }).collect();
    let nt = i32_at(b.get("tankcount"), 0).max(0) as usize;
    let vehicles: Vec<Vehicle> = b.get("tankok").chunks(8).take(nt + 1).map(|c| Vehicle { fleet: i16_at(c, 0), design: i16_at(c, 2), count: i32_at(c, 4) }).collect();
    let planet_record = (planet > 0).then(|| {
        let r = b.get(&format!("planet{planet}"));
        let n = i32_at(r, 0x114).max(0) as usize;
        let buildings = b.find(&format!("buildings{planet}@")).map_or(Vec::new(), |(_, x)| {
            x.chunks(0x20).skip(1).map(|c| SetupBuilding { kind: c[0], finished: i16_at(c, 8) == 0, on: c[0x19] & 1 != 0 }).collect()
        });
        SetupPlanet { owner: r[0x20], ships: (1..=n).map(|k| i16_at(r, 0xfe + 2 * k)).collect(), buildings }
    });
    let fleets: Vec<i32> = p["fleets"].as_array().unwrap().iter().map(|x| x.as_i64().unwrap() as i32).collect();
    let mut inp = SetupInput {
        fleets,
        planet,
        ground_war: g(0xb9c027)[0] != 0,
        human: i32_at(g(0x912a3c), 0),
        human_controlled,
        multiplayer: g(0xbae2c0)[0] != 0,
        asteroids: g(0xe72dd2)[0] != 0,
        story_pact: std::array::from_fn(|k| i32_at(story, 4 * k)),
        story_zachkin: i32_at(story, 0x4c),
        story_retreat: std::array::from_fn(|k| i32_at(story, 0x50 + 4 * k)),
        fleet_records: &fleet_records,
        ships: &ships,
        vehicles: &vehicles,
        designs: &st.designs,
        planet_record: planet_record.as_ref(),
        defence_fleets: b.r.get("hajokpre").map(|(_, x)| std::array::from_fn(|k| i16_at(x, 0x30 - 6 * (k + 1)) as i32)),
    };
    let (br, _) = rules(b, st);
    let rules = SetupRules {
        weapons: br.weapons,
        forms: br.forms,
        formations: forms.clone(),
        building_tech: g(0x8eba38).chunks(0x24).map(|c| c[0]).collect(),
    };
    let mut types = ship_types(a.global(0x91f488));
    let mut rng = Streams::default();
    let seeds = a.get("seeds");
    for i in 0..21 {
        rng.set_state(i, u32_at(seeds, 4 * i));
    }
    let got = new_battle(&mut slots, &mut inp, &rules, &mut types, &mut rng);
    if got != Some(slot) {
        errs.push(format!("slot {got:?} vs {slot}"));
        return errs;
    }
    let seeds = b.get("seeds");
    for i in 0..21 {
        if rng.state(i) != u32_at(seeds, 4 * i) {
            errs.push(format!("stream {i}"));
        }
    }
    let want_types = ship_types(g(0x91f488));
    for (k, (x, y)) in types.iter().zip(&want_types).enumerate() {
        if x != y {
            errs.push(format!("ship type {k}: {x:?} vs {y:?}"));
        }
    }
    let mut port = slots[slot].clone();
    let mut exe = decode(b, st);
    // Free projectile slots after a new battle's `Lovedekinit`.
    if !join {
        for (f, pool) in exe.pools.iter().enumerate() {
            if pool.shots.iter().any(|s| s.shooter.is_some()) {
                errs.push(format!("pool {f} not empty"));
            }
        }
    }
    run.joins += join as usize;
    run.units += exe.units.iter().filter(|u| u.alive && u.owner != 0).count();
    run.merged += exe.units.iter().filter(|u| u.alive && u.ship_count > 1).count();
    run.fighters += exe.fighters.len();
    run.guns += exe.guns.len();
    run.asteroids += exe.units.iter().filter(|u| u.alive && u.owner == 0).count();
    run.planet_ships += exe.units.iter().filter(|u| u.alive && u.ships.first().is_some_and(|&s| s < 0)).count();
    tidy_setup(&mut port);
    tidy_setup(&mut exe);
    if port.units.len() != exe.units.len() || port.fighters.len() != exe.fighters.len() || port.guns.len() != exe.guns.len() {
        errs.push(format!(
            "counts: units {}/{}, fighters {}/{}, guns {}/{}",
            port.units.len(),
            exe.units.len(),
            port.fighters.len(),
            exe.fighters.len(),
            port.guns.len(),
            exe.guns.len()
        ));
    }
    compare(&port, &exe, &mut errs);
    let more = |x: &Battle| {
        (x.hostile, x.human, x.planet, x.planet_owner, (x.cell, x.grid, x.bounds, x.width), (x.retreat_point, x.scale, x.shown),
         (x.end_period, x.invulnerable, x.ended, x.ground_war, x.planet_pos, x.planet_size), (x.active, x.map_pos, x.story_retreat),
         (x.asteroids, x.formation_choice, x.ground_war_started))
    };
    if more(&port) != more(&exe) {
        diff_debug("battle", &more(&port), &more(&exe), &mut errs);
    }
    // A defence ship without a formation slot joins a unit by the fleet in the heap bytes before `Hajok`
    // (undefined); traces without those bytes can't decide it.
    if !errs.is_empty() && inp.defence_fleets.is_none() && exe.units.iter().any(|u| u.alive && 1 < u.ship_count && u.ships[0] < 0) {
        run.undefined += 1;
        errs.clear();
    }
    errs
}

#[test]
fn battle_setup_matches_trace() {
    let files = traces("spwc");
    let Some((forms, compiled)) = install_data() else { return };
    if files.is_empty() {
        return;
    }
    let mut run = SetupRun { setups: 0, joins: 0, units: 0, merged: 0, fighters: 0, guns: 0, asteroids: 0, planet_ships: 0, bad: 0, ends: 0, undefined: 0 };
    for p in &files {
        replay_setups(p, &forms, &compiled, &mut run);
    }
    eprintln!(
        "setups: {} ({} joins), {} units ({} with several ships, {} defence ships), {} fighter groups, {} planet guns, {} asteroids; {} battle ends; {} undecidable (defence-ship merge without the heap bytes); {} mismatches",
        run.setups, run.joins, run.units, run.merged, run.planet_ships, run.fighters, run.guns, run.asteroids, run.ends, run.undefined, run.bad
    );
    assert_eq!(run.bad, 0);
}
