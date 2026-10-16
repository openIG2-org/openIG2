//! The real-time ground war (spec `re/rules/ground-combat.md` §11): placement on the colony map
//! (`LocateTankSuitablePosition` exe 0x486b90 with the landing zone), terrain heights (`GetHeight`
//! 0x47ad60), building turrets (`AddBuildingTurrets` 0x45af60, `AddTurret` 0x45e670, `BuildingBuilt`
//! 0x45faf0) and the battle start (`StartGrWar` 0x4a4a70).

use super::{flag, weapon, Group, War, EMPTY, CONTESTED, NONE};
use crate::placement::{self, Terrain};

/// Real-time fields of a unit (`WF_Tank`, exe offsets; constructor values in [`Default`]).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TankRt {
    /// `+0x1dc` height on the map (the third map coordinate, with [`Group::pos`]).
    pub z: f32,
    /// `+0xe0 / +0xe4` turning speeds (body, turret).
    pub turn: (f32, f32),
    /// `+0x108` top speed while path-driving, `+0x10c` current speed, `+0x110` acceleration and `+0x114`
    /// braking per tick (2.5 / 0 / 0.15 / 0.3 once placed).
    pub max_speed: f32,
    pub velocity: f32,
    pub accel: f32,
    pub decel: f32,
    /// `+0x150` driving speed (`(f32)(int)TankAlapSpeed[hull]`).
    pub speed: f32,
    /// `+0x148` fire range (the longest of the weapons', in grid cells).
    pub range: f32,
    /// `+0x1f0 + 4k` reload time of `WF_NonSimulateWeapons[k]` (30.0 until set), `+0xa8 + 4k` its counter.
    pub reload: [f32; 14],
    pub reload_left: [f32; 14],
    /// `+0x1c0` the body angle (non-zero → flag 0x200 on the first `AddTank`), `+0x1c4` the turret's,
    /// `+0x1c8` the gun elevation; `+0x228` [unk]; `+0x248` the AI group (`WF::MI`, 0 = main body) and
    /// `+0x24c` its home (−slot of the fortress it guards, `re/rules/ground-ai.md` §6.2; uninitialised in a new unit).
    pub angle: f32,
    pub turret_angle: f32,
    pub elevation: f32,
    pub f228: u32,
    pub ai_group: i32,
    pub ai_home: i32,
    /// `+0x125` the alive-member count `+0x128` is cached for this tick (`AliveTankNum` 0x45f380).
    pub alive_cached: u8,
    pub alive_n: i32,
    /// The dropship (`WF_Hordozo`, §11.16): `+0x264/+0x268` its offset on the map axes, `+0x26c` its height
    /// above the unit (negative: up).
    pub drop: [f32; 3],
    /// `+0x280` the battlefield's planet.
    pub planet: i32,
    /// `+0x1c` the smoothed terrain basis, `+0x40` the body's and `+0x64` the turret's orientation
    /// (3 × 3, row-major; identity from the constructor). `+0x1c4` the turret's angle (`+0x1c0` the body's).
    pub basis: [f32; 9],
    pub body: [f32; 9],
    pub turret: [f32; 9],
    /// `+0xc / +0x10` the point the unit drives to next (also its render position; `SetPosG` sets it),
    /// `+0x284` "snap the basis" (1 from the constructor, cleared by `SetTankPositionAndMatrix`), `+0x285`
    /// moved this tick (`GetNearestEnemy` only looks at movers).
    pub step: (f32, f32),
    pub snap: u8,
    pub moved: u8,
    /// `+0x18` the direction to the target, `+0x98/+0x9c` the driving direction, `+0x12c` the wanted body
    /// angle, `+0x104` blocked while turning.
    pub aim: f32,
    pub dir: (f32, f32),
    pub want_angle: f32,
    pub blocked: u8,
    /// `+0x118` ticks until the next path search, `+0x14c` the fire range at buildings.
    pub path_timer: i32,
    pub bld_range: f32,
    /// `+0x22c/+0x230/+0x234` queued waypoints (cells) and the next one's index.
    pub waypoints: Vec<(i32, i32)>,
    pub wp_cursor: i32,
    /// `+0x250` mine laying on, `+0x254/+0x258` the cell of the last mine.
    pub laying: u8,
    pub mine_cell: (i32, i32),
    /// `+0x28c` the unit has fired in range (it can be found by `GetNearestEnemy`).
    pub fired: u8,
}

const IDENTITY: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

impl TankRt {
    /// The dropship as the unit constructor (`WF_Hordozo::WF_Hordozo`, inlined at exe 0x4570c0) leaves it from
    /// its libc `rand()` `r`: height `r % 500 − 1650`, offsets a third of it (f32).
    pub fn dropship_start(r: i32) -> [f32; 3] {
        let h = (r % 500 - 0x672) as f32;
        [h / 3.0, h / 3.0, h]
    }
}

impl Default for TankRt {
    fn default() -> Self {
        Self {
            z: 0.0,
            turn: (0.0, 0.0),
            max_speed: 3.0,
            velocity: 0.0,
            accel: 0.25,
            decel: 0.5,
            speed: 0.0,
            range: 0.0,
            reload: [30.0; 14],
            reload_left: [0.0; 14],
            angle: 0.0,
            turret_angle: 0.0,
            elevation: 0.0,
            f228: 0,
            ai_group: 0,
            ai_home: 0,
            alive_cached: 0,
            alive_n: 0,
            drop: [0.0; 3],
            planet: 0,
            basis: IDENTITY,
            body: IDENTITY,
            turret: IDENTITY,
            step: (0.0, 0.0),
            snap: 1,
            moved: 0,
            aim: 0.0,
            dir: (0.0, 0.0),
            want_angle: 0.0,
            blocked: 0,
            path_timer: 0,
            bld_range: 0.0,
            waypoints: Vec::new(),
            wp_cursor: 0,
            laying: 0,
            mine_cell: (0, 0),
            fired: 1,
        }
    }
}

/// One colony map as the ground war sees it: terrain mask and AI build start (`placement`), the height
/// grid `Terepek[m]+0x138c` (50 × 50 i16, the file's) and the base height `+0x84`, which the loader
/// stores as `−(file value − 200)` (`InitHatterekEsTerepek` ELF 0x4be142).
#[derive(Debug, Clone)]
pub struct GroundMap {
    pub terrain: Terrain,
    /// 50 × 50 (`y·50 + x`), optionally followed by what the exe reads past the grid: the first 51 i16 of
    /// the next record (`Terepek[m + 1]`: its background byte, padding, a heap pointer, then strings;
    /// [original nondeterminism], read by samples with `fy ≥ 49`). Missing values read as 0.
    pub heights: Vec<i16>,
    pub base: f32,
    /// Building footprints by kind (`ig2_rules::Rules::building_footprints`).
    pub footprints: Vec<(i32, i32)>,
}

impl GroundMap {
    /// One height sample: `(f32)((f64)h · 0.0625) − 700.0`; past the grid the exe reads the next record
    /// (port: 0).
    fn sample(&self, x: i32, y: i32) -> f32 {
        let h = usize::try_from(y * 50 + x).ok().and_then(|i| self.heights.get(i)).copied().unwrap_or(0);
        ((h as f64 * 0.0625) as f32) - 700.0
    }

    /// `GetHeight(fx, fy, p − 1)` (exe 0x47ad60): the height of the triangle under `(fx, fy)` (grid
    /// cells / 4), intersected in f64 (`0x44ff80`), plus the map's base height (spec §11.2).
    pub fn height(&self, fx: f32, fy: f32) -> f32 {
        let (ix, iy) = (fx as i32, fy as i32);
        let corner = |x: i32, y: i32| [x as f32, y as f32, self.sample(x, y)];
        let a = if (ix + 1) as f32 - fx > fy - iy as f32 { corner(ix, iy) } else { corner(ix + 1, iy + 1) };
        let b = corner(ix + 1, iy);
        let c = corner(ix, iy + 1);
        let z = plane_z(a, b, c, [fx, fy, 1.0], [fx, fy, 0.0]).unwrap_or(100000.0);
        self.base + z
    }
}

/// `0x44ff80`: where the line `q1 → q0` meets the plane through `a, b, c` (only the z coordinate is
/// used). Coordinate differences in f32, everything else in f64 (asm: `subss` then `cvtps2pd`, `mulsd`).
fn plane_z(a: [f32; 3], b: [f32; 3], c: [f32; 3], q1: [f32; 3], q0: [f32; 3]) -> Option<f32> {
    let d = |x: f32, y: f32| (x - y) as f64;
    let (bx, cx, dx) = (d(b[0], a[0]), d(c[0], a[0]), d(q0[0], q1[0]));
    let (by, cy, dy) = (d(b[1], a[1]), d(c[1], a[1]), d(q0[1], q1[1]));
    let (bz, cz, dz) = (d(b[2], a[2]), d(c[2], a[2]), d(q0[2], q1[2]));
    let n4 = cz * by - bz * cy;
    let n1 = by * cx - cy * bx;
    let n3 = cz * bx - bz * cx;
    let den = n4 * dx - n1 * dz - n3 * dy;
    if den == 0.0 {
        return None;
    }
    let (q1x, q1y, q1z) = (q1[0] as f64, q1[1] as f64, q1[2] as f64);
    let (ax, ay, az) = (a[0] as f64, a[1] as f64, a[2] as f64);
    let num = (((q1y * dz - ay * dz) - q1z * dy) * n3 - az * dz * n1) - ((q1x * dz - ax * dz) - q1z * dx) * n4;
    Some((num / den) as f32)
}

// ------------------------------------------------------------------------------------- orientation (§11.2)

type V3 = [f32; 3];
type M3 = [f32; 9];

/// `0x4501f0`: normalise in place (f32, the root in f64); below 1e-5 the vector stays as it is.
fn normalise(v: &mut V3) {
    let len = ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) as f64).sqrt() as f32;
    if len < 1e-5 {
        return;
    }
    *v = [v[0] / len, v[1] / len, v[2] / len];
}

/// `0x44f4c0`: `out = a ⊗ b` in the exe's term order.
fn mul(a: &M3, b: &M3) -> M3 {
    [
        a[0] * b[0] + b[1] * a[3] + b[2] * a[6],
        a[4] * b[1] + a[1] * b[0] + a[7] * b[2],
        a[5] * b[1] + a[2] * b[0] + a[8] * b[2],
        b[4] * a[3] + b[3] * a[0] + b[5] * a[6],
        a[4] * b[4] + a[1] * b[3] + a[7] * b[5],
        a[5] * b[4] + a[2] * b[3] + a[8] * b[5],
        a[0] * b[6] + a[3] * b[7] + a[6] * b[8],
        a[4] * b[7] + a[1] * b[6] + a[7] * b[8],
        a[5] * b[7] + a[2] * b[6] + a[8] * b[8],
    ]
}

/// `0x44f480`: transpose.
fn transpose(a: &M3) -> M3 {
    [a[0], a[3], a[6], a[1], a[4], a[7], a[2], a[5], a[8]]
}

/// `0x44f9f0`: rotation by `angle` about the unit axis `a` (quaternion form; products as in the asm).
fn axis_rotation(a: V3, angle: f32) -> M3 {
    let half = angle * 0.5;
    let s = (half as f64).sin() as f32;
    let (qx, qy, qz) = (a[0] * s, a[1] * s, a[2] * s);
    let c = (half as f64).cos() as f32;
    let (x2, y2, z2) = (qx * 2.0, qy * 2.0, qz * 2.0);
    let (wx, xx, wy, yy) = (x2 * c, x2 * qx, y2 * c, y2 * qy);
    let (yz, zz, wz, xy, xz) = (z2 * qy, z2 * qz, z2 * c, y2 * qx, z2 * qx);
    [1.0 - (zz + yy), xy - wz, xz + wy, xy + wz, 1.0 - (zz + xx), yz - wx, xz - wy, yz + wx, 1.0 - (yy + xx)]
}

/// `0x44f170`: the Euler rotation (here always `(0, 0, −π/2)`).
fn euler(a: f32, b: f32, c: f32) -> M3 {
    let (s1, c1) = ((a as f64).sin() as f32, (a as f64).cos() as f32);
    let (s3, c4) = ((b as f64).sin() as f32, (b as f64).cos() as f32);
    let (s5, c7) = ((c as f64).sin() as f32, (c as f64).cos() as f32);
    [
        c7 * c4,
        -(c7 * s3 * s1) - s5 * c1,
        s5 * s1 - c7 * s3 * c1,
        s5 * c4,
        c7 * c1 - s5 * s3 * s1,
        -(s5 * s3 * c1) - c7 * s1,
        s3,
        c4 * s1,
        c4 * c1,
    ]
}

/// `0x44ef70`: an orthonormal basis from two directions (`p` forward, `q` aside), negated.
fn basis_from(p: V3, q: V3) -> M3 {
    let mut f6 = p[0] * q[1] - q[0] * p[1];
    let mut f8 = q[2] * p[1] - q[1] * p[2];
    let mut f9 = q[0] * p[2] - p[0] * q[2];
    let len = ((f9 * f9 + f8 * f8 + f6 * f6) as f64).sqrt() as f32;
    if len >= 1e-5 {
        f8 /= len;
        f9 /= len;
        f6 /= len;
    }
    let mut f2 = p[1] * f6 - p[2] * f9;
    let mut f5 = p[2] * f8 - p[0] * f6;
    let mut f7 = p[0] * f9 - p[1] * f8;
    let len = ((f5 * f5 + f2 * f2 + f7 * f7) as f64).sqrt() as f32;
    if len >= 1e-5 {
        f2 /= len;
        f5 /= len;
        f7 /= len;
    }
    [-p[0], -f2, -f8, -p[1], -f5, -f9, -p[2], -f7, -f6]
}

/// `0x48b560`: the body orientation from the terrain basis `t`: the smoothed basis moves each row by 0.3
/// (f64) toward the new one when it differs by at least 0.3 (`snap`: take it at once).
fn smooth(body: &mut M3, angle: f32, basis: &mut M3, snap: bool) {
    let r = axis_rotation([1.0, 0.0, 0.0], angle);
    let t = mul(body, &transpose(&r));
    if snap {
        *basis = t;
    } else {
        for k in 0..3 {
            let mut d = [t[3 * k] - basis[3 * k], t[3 * k + 1] - basis[3 * k + 1], t[3 * k + 2] - basis[3 * k + 2]];
            let len = ((d[1] * d[1] + d[0] * d[0] + d[2] * d[2]) as f64).sqrt() as f32;
            if 0.3 > len as f64 {
                basis[3 * k..3 * k + 3].copy_from_slice(&t[3 * k..3 * k + 3]);
                continue;
            }
            normalise(&mut d);
            let mut row = [0f32; 3];
            for j in 0..3 {
                row[j] = (d[j] as f64 * 0.3 + basis[3 * k + j] as f64) as f32;
            }
            normalise(&mut row);
            basis[3 * k..3 * k + 3].copy_from_slice(&row);
        }
    }
    *body = mul(basis, &r);
}

impl GroundMap {
    /// `0x4a8b60`: the terrain basis and height under map position `(x, y)` with body angle `angle`:
    /// heights 1/100 ahead and aside of the unit, on the 50 × 50 height grid.
    fn terrain(&self, x: f32, y: f32, angle: f32, (w, h): (f32, f32)) -> (M3, f32) {
        let cell = |x: f32, y: f32| (49.0 - (w * 0.5 + x) / w * 50.0 * 49.0 / 50.0, (h * 0.5 + y) / h * 50.0 * 49.0 / 50.0);
        let (fx, fy) = cell(x, y);
        let h0 = self.height(fx, fy);
        let s = ((-angle) as f64).sin() as f32;
        let c = ((-angle) as f64).cos() as f32;
        let (bx, by) = (s / 100.0 + x, c / 100.0 + y);
        let (cx, cy) = (c / 100.0 + x, -(s / 100.0) + y);
        let (f1x, f1y) = cell(bx, by);
        let h1 = self.height(f1x, f1y);
        let (f2x, f2y) = cell(cx, cy);
        let h2 = self.height(f2x, f2y);
        let mut a = [h0 - h1, x - bx, y - by];
        let mut b = [h0 - h2, x - cx, y - cy];
        normalise(&mut a);
        normalise(&mut b);
        let m = basis_from(b, a);
        (mul(&euler(0.0, 0.0, -std::f32::consts::FRAC_PI_2), &m), h0)
    }
}

/// Map position → grid cell (`Real2Grid`, inlined in `AddTurret`; f32 with an f64 `floor`).
pub fn real_to_grid(x: f32, y: f32, (w, h): (f32, f32)) -> (i32, i32) {
    let gx = (w * 0.5 - x) * 99.0 / w / 100.0 * 99.0 * 2.0;
    let gy = (h * 0.5 + y) * 99.0 / h / 100.0 * 99.0 * 2.0;
    ((gx as f64).floor() as f32 as i32, (gy as f64).floor() as f32 as i32)
}

/// Grid cell → map position without the half-cell offset (`AddBuildingTurrets`; `SetPosG` adds 0.5):
/// f32 products, the centring subtraction in f64.
pub fn grid_to_real(gx: i32, gy: i32, (w, h): (f32, f32)) -> (f32, f32) {
    let axis = |g: i32, size: f32| (g as f32 * 0.5 / 99.0 * 100.0 * size / 99.0) as f64 - size as f64 * 0.5;
    ((-axis(gx, w)) as f32, axis(gy, h) as f32)
}

/// The building kinds with turrets (`AddBuildingTurrets`: 0x1c..=0x23) and their turret pod tech
/// (`kind + 0x85`, 0xa1..=0xa8; only 0xa5..=0xa8 carry a weapon).
pub const TURRET_KINDS: std::ops::RangeInclusive<u8> = 0x1c..=0x23;

/// What `StartGrWar` decided (spec §11.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// The battle runs: `GetStatus` said contested (or nobody) or the field was already finished.
    Battle,
    /// One side already holds the field: `GroundWarFinished(p, s, false)` ran at once.
    Finished(i32),
}

impl War<'_> {
    /// The cells of the units that block a vehicle (`LocateTankSuitablePosition`'s unit pass).
    fn placed_cells(&self) -> Vec<(i32, i32)> {
        (0..self.field.units.len())
            .filter(|&i| {
                let u = &self.field.units[i];
                self.field.alive[i] && u.flags & (flag::NOT_PLACED | flag::NOT_COUNTED) == 0 && u.flags2 & 4 == 0
            })
            .map(|i| self.field.units[i].cell)
            .collect()
    }

    /// `LocateTankSuitablePosition(p, &x, &y, start)`: with a [`GroundMap`] the real search, else the
    /// [`War::place`] hook. On failure the caller's outputs keep their previous value (`self.place_out`;
    /// uninitialised locals in the exe).
    pub(super) fn locate_tank(&mut self, start: Option<(i32, i32)>) -> (bool, (i32, i32)) {
        let Some(map) = self.map else {
            let r = (self.place)(start);
            if r.0 {
                self.place_out = r.1;
            }
            return r;
        };
        let buildings: Vec<(u8, f32, f32)> = self.planet.buildings.iter().skip(1).map(|b| (b.kind, b.x, b.y)).collect();
        if start.is_some() && self.field.landing.is_none() {
            self.field.landing = Some(placement::landing_zone(&buildings, &map.footprints));
        }
        let units = self.placed_cells();
        match placement::locate_tank(&map.terrain, &map.footprints, &buildings, self.field.landing.as_deref(), &units, start) {
            Some(at) => {
                self.place_out = at;
                (true, at)
            }
            None => (false, self.place_out),
        }
    }

    /// `WF_Tank::SetTankPositionAndMatrix` (exe 0x49c4f0, spec §11.2): orient the unit on the terrain
    /// and take its height; the slope sets the driving speed (`+0x150 = 2·basis[1] + 3`). The heights
    /// come from the map of the planet **on the colony screen** (`AktPlanet − 1`) [original quirk].
    pub fn set_matrix(&mut self, i: i32) {
        let map = self.view_map;
        let size = self.rules.map_size;
        let u = &mut self.field.units[i as usize];
        if u.flags & flag::TURRET == 0 {
            let (terrain, h0) = map.map_or((IDENTITY, 0.0), |m| m.terrain(u.pos.0, u.pos.1, u.rt.angle, size));
            let (x, y) = u.pos;
            u.rt.body = terrain;
            let snap = u.rt.snap != 0;
            smooth(&mut u.rt.body, u.rt.angle, &mut u.rt.basis, snap);
            u.rt.z = h0;
            u.pos = (x, y);
        }
        u.rt.snap = 0;
        if u.flags & flag::TURRET == 0 {
            let b = &u.rt.basis;
            u.rt.speed = (b[0] * 0.0 + b[1] + b[2] * 0.0) * 2.0 + 3.0;
            let axis = [u.rt.body[2], u.rt.body[5], u.rt.body[8]];
            let r = axis_rotation(axis, -(u.rt.turret_angle - u.rt.angle));
            u.rt.turret = mul(&u.rt.body, &r);
        }
    }

    /// The state side of `WF_Tank::LoadMesh` (exe 0x4847a0), which runs when a unit is created while its
    /// battlefield is on the colony screen ([`GroundContext::on_screen`]): flags 0x10/0x20/0x200, and the
    /// orientation is computed again, so a vehicle landed on screen drives at the slope speed instead of
    /// its hull's (`AddTank` set that before) [original quirk]. Mines take their cell's map position.
    pub(super) fn load_mesh(&mut self, i: i32) {
        let u = self.u(i);
        if u.flags & flag::MINE != 0 {
            // The mine branch (position from the cell, `GetHeightFloat`): not reached here (spec §11.5).
            self.um(i).flags |= 0x230;
            return;
        }
        if u.flags & flag::TURRET == 0 {
            self.set_matrix(i);
        }
        self.um(i).flags |= 0x230;
    }

    /// `GetHeight` for the battlefield's map (0 without one).
    pub fn height(&self, fx: f32, fy: f32) -> f32 {
        self.map.map_or(0.0, |m| m.height(fx, fy))
    }

    /// `WF::AddTurret(pod, hp, side, slot, x, y, z)` (exe 0x45e670, spec §11.3): a one-member turret group
    /// of a defence building in the first dead slot (never merged); it stays out of the status count
    /// (flags2 bit 2) until its building is finished ([`War::building_built`]).
    #[allow(clippy::too_many_arguments)]
    pub fn add_turret(&mut self, pod: i32, hp: i32, side: i32, slot: i32, x: f32, y: f32, z: f32) -> i32 {
        self.field.status_valid = false;
        let i = match self.field.alive.iter().position(|&a| !a) {
            Some(i) => i,
            None => {
                self.field.units.push(Group::default());
                self.field.alive.push(false);
                self.field.units.len() - 1
            }
        };
        let r = (self.libc)();
        self.field.tail.created.push(i);
        let mut u = Group::default();
        u.rt.drop = TankRt::dropship_start(r);
        u.design = pod;
        u.hp.push(hp);
        u.origin.push(-1);
        u.max_hp = hp;
        u.owner = side;
        u.side = side;
        u.rt.max_speed = 2.5;
        u.rt.accel = 0.15;
        u.rt.decel = 0.3;
        // `+0x150 := 0` (a turret doesn't drive); the default range candidate, then the weapon's.
        u.rt.speed = 0.0;
        let hull2 = ((u.flags >> 20) & 1) as i32 * 2;
        let cand = |range: i32| ((hull2 as f32) + range as f32) as i32 as f32;
        if u.rt.range < cand(1) {
            u.rt.range = cand(1);
        }
        u.rt.turn = (0.65, 1.0);
        u.move_state = (0, -1);
        u.last_hit = Some((0, NONE));
        u.rt.elevation = 0.0;
        u.rt.f228 = 0;
        u.rt.planet = self.planet.index;
        // Pod → weapon bit and its `WF_FortressWeapons` entry (range raises the fire range, reload).
        let gun = match pod {
            0xa5 => Some((weapon::W1, 0)),
            0xa6 => Some((weapon::W100, 4)),
            0xa7 => Some((weapon::W2, 8)),
            0xa8 => Some((weapon::W400, 6)),
            _ => None,
        };
        if let Some((bit, k)) = gun {
            u.weapons |= bit;
            let w = self.rules.fortress.get(k).copied().unwrap_or([0; 4]);
            if u.rt.range < cand(w[3]) {
                u.rt.range = cand(w[3]);
            }
            u.rt.reload[k] = w[2] as f32;
        }
        u.flags |= flag::TURRET;
        u.building = slot;
        u.index = i as i32;
        u.rt.ai_group = 0;
        u.pos = (x, y);
        u.rt.z = z;
        u.pods = 1;
        u.cell = real_to_grid(x, y, self.rules.map_size);
        u.last_cell = u.cell;
        u.flags2 |= 4;
        u.has_gun = true;
        self.field.units[i] = u;
        self.field.alive[i] = true;
        if self.ctx.on_screen && self.field.units[i].flags & flag::MESH == 0 {
            self.load_mesh(i as i32);
        }
        i as i32
    }

    /// `WF::AddBuildingTurrets(kind, 100, side, slot, p)` (exe 0x45af60, spec §11.3): the turrets of a new
    /// defence building, their hit points split from the building's.
    pub fn add_building_turrets(&mut self, kind: u8, side: i32, slot: i32) {
        if !TURRET_KINDS.contains(&kind) {
            return;
        }
        let Some(b) = self.planet.buildings.get(slot.max(0) as usize).copied() else { return };
        let (gx, gy) = (b.x as i32, b.y as i32);
        let (px, py) = grid_to_real(gx, gy, self.rules.map_size);
        let (fx, fy) = ((gx as f64 * 0.25) as f32, (gy as f64 * 0.25) as f32);
        let mut z = if fx > 200.0 || fy > 200.0 { 0.0 } else { self.height(fx, fy) };
        let pod = kind as i32 + 0x85;
        let hp = self.rules.building_hp.get(kind as usize).copied().unwrap_or(0);
        let add = |w: &mut Self, hp: i32, x: f32, y: f32, z: f32| {
            w.add_turret(pod, hp, side, slot, x, y, z);
        };
        match kind {
            0x1c..=0x20 => {
                let (mut x, mut y) = (px, py);
                if kind == 0x20 {
                    z -= 133.0;
                } else {
                    z -= 20.0;
                    y -= 20.0;
                    x -= 20.0;
                }
                add(self, hp, x, y, z);
            }
            0x21 => {
                z -= 65.0;
                let a = hp / 4;
                add(self, a, px + 50.0, py + 50.0, z);
                let rest = hp - a;
                let b2 = rest / 3;
                add(self, b2, px - 50.0, py + 50.0, z);
                let rest = rest - b2;
                let c = rest / 2;
                add(self, c, px - 50.0, py - 50.0, z);
                add(self, rest - c, px + 50.0, py - 50.0, z);
            }
            0x22 => {
                z -= 60.0;
                let a = hp / 4;
                let y1 = (py + 70.0) - 25.0;
                let x1 = (px + 70.0) - 25.0;
                add(self, a, x1, y1, z);
                let rest = hp - a;
                let b2 = rest / 3;
                let x2 = ((px - 70.0) - 25.0) + 10.0;
                add(self, b2, x2, y1, z);
                let rest = rest - b2;
                let y2 = ((py - 70.0) - 25.0) + 10.0;
                let c = rest / 2;
                add(self, c, x2, y2, z);
                add(self, rest - c, x1, y2, z);
            }
            _ => {
                z -= 68.0;
                let a = hp / 6;
                add(self, a, px + 75.0, py + 75.0, z);
                let rest = hp - a;
                let b2 = rest / 5;
                add(self, b2, px - 75.0, py + 75.0, z);
                let rest = rest - b2;
                let c = rest / 4;
                add(self, c, px - 75.0, py - 75.0, z);
                let rest = rest - c;
                let d = rest / 3;
                add(self, d, px + 75.0, py - 75.0, z);
                let rest = rest - d;
                let e = rest / 2;
                add(self, e, px + 110.0, py, z);
                add(self, rest - e, px - 110.0, py, z);
            }
        }
    }

    /// `WF::BuildingBuilt(slot)` (exe 0x45faf0): the slot's turrets count from now on (flags2 bit 2 off).
    pub fn building_built(&mut self, slot: i32) {
        for i in 0..self.field.units.len() {
            if self.field.alive[i] && self.field.units[i].building == slot {
                self.field.units[i].flags2 &= !4;
                self.field.status_valid = false;
            }
        }
    }

    /// `WF::InitMineSystem` (exe 0x483640): mines go, ammunition and air units are refilled.
    pub(super) fn init_mines(&mut self) {
        for i in 0..self.field.units.len() as i32 {
            if !self.alive(i) {
                continue;
            }
            if self.u(i).flags & flag::MINE != 0 {
                self.kill(i);
                continue;
            }
            let u = self.um(i);
            if u.weapons & weapon::AMMO != 0 {
                u.ammo = u.size() * 3;
            }
            u.air = if u.weapons & (weapon::FIGHTER | weapon::BIG_SHIP) != 0 { u.size() } else { 0 };
        }
    }

    /// `StartGrWar(p)` (exe 0x4a4a70, spec §11.4) after a real-time `GroundWarIndit`: mines cleared,
    /// every empire's call-in timers reset (`+0x7fc/+0x800` := 140, 240), and unless one side already
    /// holds the field the battle runs (the colony screen of `p` opens when the human takes part: an
    /// event of the UI). The retreat flags `+0x89f..` are cleared either way.
    pub fn start(&mut self) -> Start {
        self.init_mines();
        for t in self.field.timers.iter_mut() {
            *t = (0x8c, 0xf0);
        }
        self.field.finished_flag = false;
        let s = self.status();
        let out = if s == CONTESTED || s == EMPTY || self.field.finished_flag {
            self.field.not_watching = [0; 18];
            Start::Battle
        } else {
            self.finish(s, false);
            self.field.finished_flag = true;
            Start::Finished(s)
        };
        self.field.retreat = [0; 18];
        out
    }
}
