//! The real-time ground battle tick, the battlefield's part: `WF::Refresh` (exe 0x490890; spec
//! `re/rules/ground-combat.md` §11.7) with its three projectile lists, the shots (`ShotOne` 0x49c810,
//! `ShotWithSpecificWeapon` 0x49d050), the projectile makers (0x45d5d0, 0x45d910, 0x45bba0, 0x45c300,
//! 0x45cca0), debris (`AddRepesz` 0x45d400), `SetRealDamage` (0x49c3c0), `DamageBuildings` (0x46d5f0),
//! `GetNearestEnemy` (0x47b860), `Retreat` (0x499260) / `StopRetreat` (0x4a4d00) and the end countdown.
//!
//! libc `rand()` here never decides anything, but debris shares projectile list B (its slots order the
//! later shots, and so the stream-2 draws), so every draw is replayed in order through [`War::libc`].

use super::tick::{sqrtf, Clock};
use super::{flag, realtime::grid_to_real, War, CONTESTED, NONE};

/// A homing shell of list A (`WF+0xd4`, count `+0xe0`; 0x38-byte objects from 0x45d5d0).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shell {
    /// `+8` velocity and `+0x14` position, both `(height, x, y)`.
    pub vel: [f32; 3],
    pub pos: [f32; 3],
    /// `+0x20` ticks left, `+0x24` target (10000 none, < 0 a building), `+0x28` homing pull.
    pub life: i32,
    pub target: i32,
    pub pull: f32,
    /// `+0x2c` damage, `+0x30` shooter.
    pub dmg: i32,
    pub by: i32,
    /// `+0x34`: deflected by stealth (`byte`, 0 or 1).
    pub jitter: u8,
}

/// A list-B object (`WF+0xd8`, count `+0xe8`; 0x44 bytes): rockets (kinds 4–6), shells (0), manipulator
/// bolts (3), bombs (1, 2), debris (7).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Missile {
    pub vel: [f32; 3],
    pub pos: [f32; 3],
    /// `+0x20` aim point.
    pub dest: [f32; 3],
    /// `+0x2c` ticks left (debris: never initialised by the exe).
    pub life: i32,
    /// `+0x30` damage (manipulator: `100·members + 2·owner + 1`), `+0x34` target (bombs: the ground
    /// height as an int), `+0x38` kind, `+0x3c` debris fade, `+0x40` shooter.
    pub dmg: i32,
    pub target: i32,
    pub kind: i32,
    pub fade: f32,
    pub by: i32,
}

/// A paralyser bolt of list C (`WF+0xdc`, count `+0xe4`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bolt {
    pub vel: [f32; 3],
    pub pos: [f32; 3],
    pub dest: [f32; 3],
    /// `+0x2c` ticks left, `+0x30` target, `+0x34` power (`100·members + owner`).
    pub life: i32,
    pub target: i32,
    pub power: i32,
}

/// One fighter of an air strike's wave (`WF+0x1f4 + 0x40·k`, k = 0..5; `WF::AddFighters` 0x45b8d0).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fighter {
    /// `+0` position and `+0xc` velocity, both `(height, x, y)`; the height velocity is the descent.
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    /// `+0x18` the last step taken (render only; `AddFighters` leaves it from the previous wave).
    pub step: [f32; 3],
    /// `+0x24` the flight phase (ticks of the wave), `+0x28` the ground height under the strike point,
    /// `+0x34` the bomb cooldown.
    pub phase: i32,
    pub ground: f32,
    pub cooldown: i32,
}

/// The projectile lists of a battlefield (slots are reused: a new object takes the first empty one).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Lists {
    pub shells: Vec<Option<Shell>>,
    pub missiles: Vec<Option<Missile>>,
    pub bolts: Vec<Option<Bolt>>,
    /// `+0x32c`: an air strike's fighters are flying (the byte overlaps fighter 4's unused `+0x38`).
    pub fighters_on: bool,
    pub fighters: [Fighter; 5],
}

pub(super) fn slot<T>(v: &mut Vec<Option<T>>) -> usize {
    match v.iter().position(|x| x.is_none()) {
        Some(i) => i,
        None => {
            v.push(None);
            v.len() - 1
        }
    }
}

/// `|b − a|` of two `(h, x, y)` points: f32 differences and squares, the root through f64.
fn dist3(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let d0 = b[0] - a[0];
    let d1 = b[1] - a[1];
    let d2 = b[2] - a[2];
    sqrtf(d0 * d0 + d1 * d1 + d2 * d2)
}

pub(super) fn normalise(v: &mut [f32; 3]) {
    let len = sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    if len < 1e-5 {
        return;
    }
    *v = [v[0] / len, v[1] / len, v[2] / len];
}

impl War<'_> {
    /// [`Self::building_point`] for the damage code.
    pub(super) fn building_point_pub(&self, slot: i32) -> [f32; 3] {
        self.building_point(slot)
    }

    /// The map position of a building's cell as the projectile makers compute it (no +0.5).
    fn building_point(&self, slot: i32) -> [f32; 3] {
        let b = self.planet.buildings.get(slot.max(0) as usize).copied().unwrap_or_default();
        let (x, y) = grid_to_real(b.x as i32, b.y as i32, self.rules.map_size);
        [self.planet.building_z.get(slot.max(0) as usize).copied().unwrap_or(0.0), x, y]
    }

    /// `0x45d5d0`: a homing shell from `p` at `t` (units are aimed 15 below their height).
    #[allow(clippy::too_many_arguments)]
    fn add_shell(&mut self, p: [f32; 3], life: i32, t: i32, pull: f32, dmg: i32, by: i32, steady: bool) {
        let k = slot(&mut self.field.lists.shells);
        let tp = if t < 0 {
            self.building_point(-t)
        } else {
            let u = self.u(t);
            [u.rt.z - 15.0, u.pos.0, u.pos.1]
        };
        let mut d = [tp[0] - p[0], tp[1] - p[1], tp[2] - p[2]];
        normalise(&mut d);
        let vel = [d[0] * 2.0 - 4.0, d[1] * 2.0, d[2] * 2.0];
        self.field.lists.shells[k] = Some(Shell { vel, pos: p, life, target: t, pull, dmg, by, jitter: (!steady) as u8 });
    }

    /// `0x45d910`: a rocket (kind 4 fast, 5 normal, 6 slow).
    fn add_rocket(&mut self, p: [f32; 3], life: i32, t: i32, dmg: i32, kind: i32, by: i32) {
        let k = slot(&mut self.field.lists.missiles);
        let tp = if t < 0 { self.building_point(-t) } else { let u = self.u(t); [u.rt.z, u.pos.0, u.pos.1] };
        let (dy, dx, dh) = (tp[2] - p[2], tp[1] - p[1], tp[0] - p[0]);
        let dist = sqrtf(dy * dy + dx * dx + dh * dh);
        let mult = match kind {
            4 => (self.draw(10, 0x14) as f64 / 100.0 + 1.1) as f32,
            6 => (0.87 - self.draw(10, 0x14) as f64 / 100.0) as f32,
            _ => 1.0,
        };
        let vel = [dh / dist * 5.0 * mult, dx / dist * 5.0 * mult, dy / dist * 5.0 * mult];
        self.field.lists.missiles[k] = Some(Missile {
            vel,
            pos: [p[0] - 10.0, p[1], p[2]],
            dest: [tp[0] - 10.0, tp[1], tp[2]],
            life,
            dmg,
            target: t,
            kind,
            fade: 0.0,
            by,
        });
    }

    /// `0x45bba0` (kind 0) and `0x45c300` (kind 3): a straight bolt at `t`; `speed` 20 / 10, `lift` the
    /// start/aim height offsets (0 / −15 for kind 0, −10 / −10 for kind 3).
    #[allow(clippy::too_many_arguments)]
    fn add_bolt_b(&mut self, p: [f32; 3], life: i32, t: i32, dmg: i32, by: i32, kind: i32) {
        let k = slot(&mut self.field.lists.missiles);
        let (aim_dz, start_dz, dest_dz, speed) = if kind == 0 { (-15.0, 0.0, -15.0, 20.0) } else { (0.0, -10.0, -10.0, 10.0) };
        let tp = if t < 0 {
            self.building_point(-t)
        } else {
            let u = self.u(t);
            [u.rt.z + aim_dz, u.pos.0, u.pos.1]
        };
        let mut d = [tp[0] - p[0], tp[1] - p[1], tp[2] - p[2]];
        normalise(&mut d);
        let vel = [d[0] * speed, d[1] * speed, d[2] * speed];
        let dest = if t < 0 {
            let b = self.building_point(-t);
            [b[0] + if kind == 0 { 0.0 } else { -10.0 }, b[1], b[2]]
        } else {
            let u = self.u(t);
            [u.rt.z + dest_dz, u.pos.0, u.pos.1]
        };
        self.field.lists.missiles[k] = Some(Missile { vel, pos: [p[0] + start_dz, p[1], p[2]], dest, life, dmg, target: t, kind, fade: 0.0, by });
    }

    /// `0x45cca0`: a paralyser bolt (list C).
    fn add_paralyser(&mut self, p: [f32; 3], life: i32, t: i32, power: i32) {
        let k = slot(&mut self.field.lists.bolts);
        let tp = if t < 0 { self.building_point(-t) } else { let u = self.u(t); [u.rt.z, u.pos.0, u.pos.1] };
        let mut d = [tp[0] - p[0], tp[1] - p[1], tp[2] - p[2]];
        normalise(&mut d);
        let vel = [d[0] * 10.0, d[1] * 10.0, d[2] * 10.0];
        let dest = [tp[0] - 10.0, tp[1], tp[2]];
        self.field.lists.bolts[k] = Some(Bolt { vel, pos: [p[0] - 10.0, p[1], p[2]], dest, life, target: t, power });
    }

    /// A bomb of an air strike in list B, falling from `p` to the int point `d`: kind 1 (exe 0x457ad0, a
    /// big ship's: speed `((rndlog(2, 0, 14)/10 + 0.5)·3` in f64, then `·20` in f32) or kind 2 (0x45b600,
    /// a fighter's: speed 20). Shooter 10000; on screen one libc `rand()` picks the texture. The fade
    /// `+0x3c` is left uninitialised by the exe (here 0).
    fn add_bomb(&mut self, p: [f32; 3], d: [i32; 3], dmg: i32, ground: i32, kind: i32) {
        let k = slot(&mut self.field.lists.missiles);
        let dest = d.map(|v| v as f32);
        let mut v = [dest[0] - p[0], dest[1] - p[1], dest[2] - p[2]];
        normalise(&mut v);
        let speed = if kind == 1 { ((self.draw(0, 14) as f64 / 10.0 + 0.5) * 3.0) as f32 * 20.0 } else { 20.0 };
        let vel = v.map(|c| speed * c);
        self.field.lists.missiles[k] = Some(Missile { vel, pos: p, dest, life: 300, dmg, target: ground, kind, fade: 0.0, by: NONE });
        if self.ctx.on_screen {
            (self.libc)();
        }
    }

    /// The impact point of an air strike at cell `(cx, cy)`: the cell's map position (no +0.5) and the
    /// ground height there (`GetHeight` at a quarter of the cell, through f64; 0 beyond 200).
    fn strike_point(&self, cx: i32, cy: i32) -> [f32; 3] {
        let (x, y) = grid_to_real(cx, cy, self.rules.map_size);
        let (hx, hy) = ((cx as f64 * 0.25) as f32, (cy as f64 * 0.25) as f32);
        let h = if 200.0 < hx || 200.0 < hy { 0.0 } else { self.height(hx, hy) };
        [h, x, y]
    }

    /// The objects `WF::ShotSpecWeapon` (exe 0x49cb70) makes at cell `(cx, cy)` (the spread already
    /// added): big ships drop 20 bombs (kind 1, damage 10) from 3000 above a 2000-wide square onto a
    /// 400-wide one; fighters bring a wave of five unless one is still up.
    pub(super) fn air_strike_objects(&mut self, cx: i32, cy: i32, big_ships: bool) {
        let [h, x, y] = self.strike_point(cx, cy);
        if !big_ships {
            self.add_fighters(h, x, y);
            return;
        }
        let (ground, top, start) = (h as i32, (h + 300.0) as i32, h - 3000.0);
        for _ in 0..20 {
            let dy = ((self.draw(0, 399) as f32 + y) - 200.0) as i32;
            let dx = ((self.draw(0, 399) as f32 + x) - 200.0) as i32;
            let sy = (self.draw(0, 1999) as f32 + y) - 1000.0;
            let sx = (self.draw(0, 1999) as f32 + x) - 1000.0;
            self.add_bomb([start, sx, sy], [top, dx, dy], 10, ground, 1);
            self.gr_sound(18);
        }
    }

    /// `WF::AddFighters(h, x, y, e, 40, h)` (exe 0x45b8d0): unless a wave is up, five fighters on a circle
    /// of radius 1500·1.35 around the strike point, 1500 (+60 each) above it, flying across it (f64
    /// trigonometry, asm-checked); each gets a bomb cooldown `rndlog(2, 0, 4)` (line 0x2991).
    fn add_fighters(&mut self, h: f32, x: f32, y: f32) {
        if self.field.lists.fighters_on {
            return;
        }
        self.field.lists.fighters_on = true;
        self.gr_sound(17);
        let top = h - 1500.0;
        for k in 0..5 {
            let a = (k as f32 / 5.0) as f64 * std::f64::consts::PI * 2.0;
            let s = (a.sin() * 500.0 * 3.0) as f32;
            let c = (a.cos() * 500.0 * 3.0) as f32;
            let pos = [top - (k * 60) as f32, (s as f64 * 1.35 + x as f64) as f32, (c as f64 * 1.35 + y as f64) as f32];
            let cooldown = self.draw(0, 4);
            let f = &mut self.field.lists.fighters[k as usize];
            f.pos = pos;
            f.cooldown = cooldown;
            f.phase = 0;
            f.vel = [12.0, (-(s / 1500.0) as f64 * 3.333 * 3.0) as f32, (-(c / 1500.0) as f64 * 3.333 * 3.0) as f32];
            f.ground = h;
        }
    }

    /// The fighters' flight (`WF::Refresh` step 5, `2·S` rounds while the wave is up): every round each
    /// fighter makes a trail (3 libc draws) and flies its phase: 0..95 descending (dropping a kind-2 bomb
    /// every 10th round ahead of itself, `AddFighterLaser` 0x45b600, damage 4000), 95..105 levelling out,
    /// 105..145 level, 145..155 pulling up, 155..200 climbing; phase 200 ends the wave.
    fn fighters_step(&mut self) {
        for k in 0..5 {
            let mut f = self.field.lists.fighters[k];
            f.cooldown += 1;
            for _ in 0..3 {
                (self.libc)();
            }
            // (new height, height step); the descent `vel[0]` itself never changes.
            let up = match f.phase {
                ..95 => {
                    // f32 product, the division by 3.333 in f64 (asm 0x490aef).
                    let t = (((f.ground - f.pos[0]) * 0.5) as f64 / 3.333) as f32;
                    if f.cooldown >= 10 {
                        let g = f.ground as i32;
                        let d = [g, (f.vel[1] * t + f.pos[1]) as i32, (f.vel[2] * t + f.pos[2]) as i32];
                        self.add_bomb(f.pos, d, 4000, g, 2);
                        self.gr_sound(18);
                        f.cooldown = 0;
                    }
                    Some((f.vel[0] + f.pos[0], f.vel[0]))
                }
                95..105 => {
                    let v = f.vel[0] * (1.0 - (f.phase as f32 - 95.0) / 10.0);
                    Some((v + f.pos[0], v))
                }
                105..145 => Some((f.pos[0], 0.0)),
                145..155 => {
                    let v = f.vel[0] * 2.0 * ((f.phase as f32 - 145.0) / 10.0);
                    Some((f.pos[0] - v, -v))
                }
                155..200 => Some((f.pos[0] - f.vel[0] * 2.0, f.vel[0] * -2.0)),
                _ => None,
            };
            if let Some((h, dh)) = up {
                f.pos[0] = h;
                f.phase += 1;
                f.pos[1] += f.vel[1];
                f.pos[2] += f.vel[2];
                f.step = [dh, f.vel[1], f.vel[2]];
            }
            if f.phase >= 200 {
                self.field.lists.fighters_on = false;
            }
            self.field.lists.fighters[k] = f;
        }
    }

    /// `AddRepesz` (exe 0x45d400): a piece of debris in list B (only while `WF+0x334 ≠ 0`; two more libc
    /// draws on screen). Its lifetime is left uninitialised by the exe (here −1: never reaches 0).
    pub(super) fn add_debris(&mut self, p: [f32; 3], v: [f32; 3]) {
        if self.field.self_destruct == 0 {
            return;
        }
        let k = slot(&mut self.field.lists.missiles);
        self.field.lists.missiles[k] = Some(Missile { vel: v, pos: p, dest: [0.0; 3], life: -1, dmg: 0, target: NONE, kind: 7, fade: 0.0, by: NONE });
        if self.ctx.on_screen {
            (self.libc)();
            (self.libc)();
        }
    }

    /// The burst of a dying group, a blown mine or a falling building on screen: one particle draw, then
    /// `n` pieces of debris (three draws each), velocities `/ 1.5` for units and mines.
    pub(super) fn burst(&mut self, p: [f32; 3], n: usize, slow: bool) {
        let size = (self.libc)() % 70 + 20;
        self.flash(p, size);
        let k = if slow { 1.5 } else { 1.0 };
        for _ in 0..n {
            let a = (self.libc)();
            let b = (self.libc)();
            let c = (self.libc)();
            let vy = ((a % 0x3c) as f32 / 10.0 - 3.0) / k;
            let vx = ((b % 0x3c) as f32 / 10.0 - 3.0) / k;
            let vh = ((c % 0x1e) as f32 / 10.0 - 4.0) / k;
            self.add_debris(p, [vh, vx, vy]);
        }
    }

    /// `SetRealDamage(t, &dmg, v)` (exe 0x49c3c0): a ground vehicle takes the hit divided by its armour on
    /// the side `v` comes from (front within 45°, flanks to 135°, rear).
    fn real_damage(&self, t: i32, dmg: i32, vx: f32, vy: f32) -> i32 {
        if t < 0 || t as usize >= self.n() {
            return dmg;
        }
        let u = self.u(t);
        if u.flags & 0x4020000 != 0 || !(0..5).contains(&u.hull) {
            return dmg;
        }
        let a = u.rt.angle as f64;
        let mut f = [a.cos() as f32, a.sin() as f32, 0.0];
        let mut g = [vx, vy, 0.0];
        normalise(&mut f);
        normalise(&mut g);
        let d = sqrtf((g[1] - f[1]) * (g[1] - f[1]) + (g[0] - f[0]) * (g[0] - f[0]) + (g[2] - f[2]) * (g[2] - f[2]));
        let ang = ((d.min(2.0) * 0.5) as f64).asin() as f32 * 2.0;
        let side = if std::f64::consts::FRAC_PI_4 > ang as f64 {
            0
        } else if 2.356194490192345 > ang as f64 {
            1
        } else {
            2
        };
        let armour = self.rules.armour.get(u.hull as usize * 3 + side).copied().unwrap_or(1.0);
        ((1.0 / armour) * dmg as f32) as i32
    }

    /// `DamageBuildings(p, h, x, y, dmg)` (exe 0x46d5f0): every building within √5000 of the blast takes
    /// `dmg · 900`.
    fn damage_buildings(&mut self, h: f32, x: f32, y: f32, dmg: i32) {
        let n = self.planet.buildings.len();
        for slot in 1..n {
            let b = self.planet.buildings[slot];
            if b.kind == 0 {
                continue;
            }
            let (w, hh) = self.rules.map_size;
            let bx = -((b.x * 0.5 / 99.0 * 100.0 * w / 99.0) as f64 - w as f64 * 0.5) as f32;
            let by = ((b.y * 0.5 / 99.0 * 100.0 * hh / 99.0) as f64 - hh as f64 * 0.5) as f32;
            let bz = self.planet.building_z.get(slot).copied().unwrap_or(0.0);
            let dx = x - bx;
            let dy = y - by;
            if dx * dx + (h - bz) * (h - bz) + dy * dy < 5000.0 && self.damage(-(slot as i32), dmg.wrapping_mul(900), NONE) {
                self.remove_target(-(slot as i32));
            }
        }
    }

    /// `WF::GetNearestEnemy` via `WF_Tank` (exe 0x47b860): the nearest counted enemy that has fired and
    /// that moved this tick (or any, for a turret), by squared map distance.
    pub(super) fn nearest_enemy(&self, i: i32) -> i32 {
        let me = self.u(i);
        let mut best = 10_000_000i32;
        let mut pick = NONE;
        for j in 0..self.n() {
            let v = &self.field.units[j];
            if !self.field.alive[j] || v.flags & (flag::NOT_PLACED | flag::NOT_COUNTED) != 0 || v.flags2 & 4 != 0 {
                continue;
            }
            if me.rt.moved == 0 && v.rt.moved == 0 && me.flags & flag::TURRET == 0 {
                continue;
            }
            let dy = me.pos.1 - v.pos.1;
            let dx = me.pos.0 - v.pos.0;
            let d = dx * dx + dy * dy;
            if best as f32 <= d || me.owner == v.owner {
                continue;
            }
            if v.rt.fired != 0 {
                best = d as i32;
                pick = j as i32;
            }
        }
        pick
    }

    /// `ShotOne(t)` (exe 0x49c810): turn the turret to `t` and fire every ready weapon when aligned.
    fn shot_one(&mut self, i: i32, t: i32, clock: Clock) {
        let (tx, ty, _) = self.target_pos(t);
        let u = self.um(i);
        let dy = ty - u.pos.1;
        let dx = tx - u.pos.0;
        let len = sqrtf(dy * dy + dx * dx);
        let mut a = ((dy / len) as f64).asin() as f32;
        if 0.0 > dx / len {
            a = (std::f64::consts::PI - a as f64) as f32;
        }
        let ta = u.rt.turret_angle;
        u.rt.aim = a;
        // The tolerances, the wrap and the step are f64 in the exe (asm 0x49c9a1..0x49cae1).
        if (((a - ta).abs() as f64) < 0.1) || u.flags & flag::MINE != 0 {
            for k in 0..14 {
                if self.u(i).rt.reload_left[k] >= self.u(i).rt.reload[k] {
                    self.shot(i, t, k);
                    let r = self.draw(0, 4);
                    let u = self.um(i);
                    u.rt.reload_left[k] = r as f32 + (u.rt.reload_left[k] - u.rt.reload[k]);
                }
            }
            return;
        }
        if a == ta {
            return;
        }
        let tau = std::f64::consts::TAU;
        let w = ta - a;
        if (w.abs() as f64) > std::f64::consts::PI {
            u.rt.turret_angle = if w > 0.0 { (ta as f64 - tau) as f32 } else { (ta as f64 + tau) as f32 };
        }
        let d = u.rt.turret_angle - a;
        let f = u.flags;
        u.flags = f | 0x2000;
        let ad = d.abs();
        let s = clock.speed as f64;
        if u.rt.turn.1 as f64 / 5.0 * s > ad as f64 {
            u.rt.turret_angle = a;
            u.flags = (f & 0xffffdfff) | 0x200;
            return;
        }
        u.flags |= 0x200;
        let step = (d / ad * u.rt.turn.0) as f64 / 5.0 * clock.run() as f64 * s;
        u.rt.turret_angle = (u.rt.turret_angle as f64 - step) as f32;
    }

    /// `ShotWithSpecificWeapon(t, k)` (exe 0x49d050): weapon `k` at target `t`, if in range.
    pub(super) fn shot(&mut self, i: i32, t: i32, k: usize) {
        let u = self.u(i);
        if u.flags & flag::PARALYSED != 0 {
            return;
        }
        let turret = u.flags & flag::TURRET != 0;
        if turret && !self.building_up(u) {
            return;
        }
        let tc = self.target_cell(t);
        let u = self.u(i);
        let dy = (tc.1 - u.cell.1) as f32;
        let dx = (tc.0 - u.cell.0) as f32;
        let dcell = sqrtf(dy * dy + dx * dx);
        let table = if turret { &self.rules.fortress } else { &self.rules.rt_weapons };
        let mut range = table.get(k).map_or(0, |w| w[3]) + (((u.flags >> 20) & 1) * 2) as i32;
        if t < 0 {
            let kind = self.planet.buildings.get((-t) as usize).map_or(0, |b| b.kind);
            range += self.footprint_w(kind) / 2;
        }
        if dcell > range as f32 {
            return;
        }
        let partner = u.flags & flag::PARTNER != 0;
        let (z, x, y) = (u.rt.z, u.pos.0, u.pos.1);
        let (w, index, owner) = (u.weapons, u.index, u.owner);
        self.um(i).rt.fired = 1;
        let p = [z, x, y];
        if w & 0x200 != 0 && k == 5 {
            let base = if partner { 0 } else { self.calc_damage(i, 5) };
            let n = self.alive_members(i);
            self.add_rocket(p, 300, t, n.wrapping_mul(base), 4, index);
            let r = (self.libc)();
            self.gr_sound(29 + r % 5);
        }
        if w & 0x400 != 0 && k == 6 {
            let base = if partner { 0 } else { self.calc_damage(i, 6) };
            let n = self.alive_members(i);
            self.add_rocket(p, 300, t, n.wrapping_mul(base), 5, index);
            let r = (self.libc)();
            self.gr_sound(29 + r % 5);
        }
        for (bit, kk, below) in [(1u32, 0usize, 2), (0x2000, 1, 5)] {
            if w & bit != 0 && k == kk {
                let base = if partner { 0 } else { self.calc_damage(i, kk) };
                let steady = self.draw(0, 9) < below;
                let n = self.alive_members(i);
                self.add_shell(p, 0x50, t, 2.0, n.wrapping_mul(base), index, steady);
                let r = (self.libc)();
                self.gr_sound(23 + r % 3);
            }
        }
        if w & 0x1000 != 0 && k == 9 {
            let skip = t >= 0 && self.u(t).flags & flag::SEIZED != 0 && w == 0x1000;
            if skip {
                self.um(i).flags &= 0xfffff6ff;
            } else {
                let tag = if partner { 0 } else { owner * 2 + 1 };
                let n = self.alive_members(i);
                self.add_bolt_b(p, 300, t, n.wrapping_mul(100).wrapping_add(tag), index, 3);
            }
        }
        if w & 2 != 0 && k == 8 {
            let base = if partner { 0 } else { self.calc_damage(i, 8) };
            let n = self.alive_members(i);
            self.add_bolt_b([z - 15.0, x, y], 300, t, n.wrapping_mul(base), index, 0);
            self.gr_sound(18);
        }
        if w & 4 != 0 && k == 3 {
            let tag = if partner { 0 } else { owner };
            let skip = t >= 0 && self.u(t).flags & flag::PARALYSED != 0 && w == 4;
            if skip {
                self.um(i).flags &= 0xfffff6ff;
            } else {
                let n = self.alive_members(i);
                self.add_paralyser(p, 300, t, n.wrapping_mul(100).wrapping_add(tag));
                let r = (self.libc)();
                self.gr_sound(29 + r % 5);
            }
        }
        if w & 0x100 != 0 && k == 4 {
            // The machine gun hits at once: 16 tracer lines (9 libc draws each), a flash and a sound.
            let mut dmg = if partner { 0 } else {
                let base = self.calc_damage(i, 4);
                base.wrapping_mul(self.alive_members(i))
            };
            let r = (self.libc)();
            self.gr_sound(19 + (r & 3));
            let tp = if t < 0 { self.building_point(-t) } else { let v = self.u(t); [v.rt.z - 15.0, v.pos.0, v.pos.1] };
            for _ in 0..16 * 9 {
                (self.libc)();
            }
            self.hit_fx(tp);
            let n = self.alive_members(i);
            dmg = n.wrapping_mul(dmg);
            dmg = self.real_damage(t, dmg, tp[1] - x, tp[2] - y);
            if self.damage(t, dmg, index) {
                self.kill(t);
            }
        }
        if w & 0x800 != 0 && k == 7 {
            // The laser hits at once. On screen the exe moves the hit point onto the target's mesh
            // (0x47af70), but along the shooter-target line, so the armour side is the target position's.
            let base = if partner { 0 } else { self.calc_damage(i, 7) };
            let tp = if t < 0 { self.building_point(-t) } else { let v = self.u(t); [v.rt.z, v.pos.0, v.pos.1] };
            // The beam (`villamgenesis` 0x616190, 12 points): 10 inner points of 2 jitters of 3 draws.
            for _ in 0..60 {
                (self.libc)();
            }
            self.gr_sound(43);
            let n = self.alive_members(i);
            let dmg = n.wrapping_mul(base);
            let dmg = self.real_damage(t, dmg, tp[1] - x, tp[2] - y);
            if self.damage(t, dmg, index) {
                self.kill(t);
            }
        }
    }

    /// A building kind's width (kind 0, an emptied slot: what the exe reads before the table).
    pub(super) fn footprint_w(&self, kind: u8) -> i32 {
        if kind == 0 {
            return self.rules.kind0_width;
        }
        self.map.map_or(0, |m| m.footprints.get(kind as usize).map_or(0, |f| f.0))
    }

    /// `Retreat` (exe 0x499260): leave the landing zone's edge or lift off.
    fn retreat(&mut self, i: i32) {
        let u = self.u(i);
        if u.flags & flag::NOT_COUNTED != 0 {
            return;
        }
        let in_zone = {
            let cell = u.cell;
            if self.field.landing.is_none() {
                let buildings: Vec<(u8, f32, f32)> = self.planet.buildings.iter().skip(1).map(|b| (b.kind, b.x, b.y)).collect();
                let fp = self.map.map_or(Vec::new(), |m| m.footprints.clone());
                self.field.landing = Some(crate::placement::landing_zone(&buildings, &fp));
            }
            let z = self.field.landing.as_deref().unwrap();
            z.get(((cell.1 - 1) * 200 + cell.0).max(0) as usize).copied().unwrap_or(0) == 0
        };
        let u = self.u(i);
        if !in_zone {
            if (u.flags as i32) >= 0 || u.flags & 0x400 == 0 {
                let (ok, at) = self.locate_tank(Some(u.cell));
                let at = if ok { at } else { (0, 0) };
                self.move_g(i, at.0, at.1);
                self.um(i).flags |= 0x80000000;
            }
        } else if u.flags & 0x60000000 == 0 {
            if !self.ctx.multiplayer {
                self.stop_orders(i);
                let u = self.um(i);
                u.flags &= !flag::NOT_PLACED;
                let t = u.transport;
                u.transport = (t & 0xffffffe2) | 0x42;
                return;
            }
            let u = self.um(i);
            u.owner = u.side;
            u.flags = (u.flags & 0xfefefff7) | 0x40000002;
            self.field.status_valid = false;
        }
    }

    /// `StopRetreat` (exe 0x4a4d00, ported as `WF_Tank::Stop`).
    fn stop_retreat(&mut self, i: i32) {
        let u = self.um(i);
        let f = u.flags;
        if f & flag::KEEP_ORDERS == 0 && (f as i32) >= 0 {
            return;
        }
        if f & flag::KEEP_ORDERS == 0 {
            u.move_state = (0, -1);
            u.flags = f & 0xfffff27f;
            u.dest = u.cell;
            u.target = NONE;
        }
        u.transport = (u.transport & 0xfffffffc) | 4;
        u.flags &= 0x5fffffff;
    }

    /// `WF::Refresh` (exe 0x490890, spec §11.7): one 20 Hz tick of the battlefield on the colony screen.
    /// `watching[k]` is the planet empire k + 1 looks at (`KiMelyikBolygonVan` 0x9ABAE8).
    pub fn refresh(&mut self, clock: Clock, watching: &[i32; 18]) {
        let s = clock.speed;
        for t in self.field.timers.iter_mut() {
            t.0 = (t.0 + s).min(200);
            t.1 = (t.1 + s).min(300);
        }
        if self.planet.in_war != 0 {
            for i in 0..self.n() as i32 {
                if !self.alive(i) {
                    continue;
                }
                let side = self.u(i).side;
                let r = self.field.retreat.get((side - 1).max(0) as usize).copied().unwrap_or(0);
                if r == 0 {
                    self.stop_retreat(i);
                } else {
                    self.retreat(i);
                }
            }
        }
        for (k, w) in watching.iter().enumerate() {
            self.field.not_watching[k] = (self.planet.index != *w) as u8;
        }
        let s0 = self.status();
        if s0 == CONTESTED {
            self.field.finished_flag = false;
        }
        // The fighters of an air strike: `2·S` rounds while the wave is up (`+0x32c`).
        for _ in 0..s * 2 {
            if self.field.lists.fighters_on {
                self.fighters_step();
            }
        }
        for _ in 0..s {
            self.shells_step();
        }
        for _ in 0..s * 4 {
            self.missiles_step();
        }
        for _ in 0..s {
            self.bolts_step();
        }
        if s0 == CONTESTED {
            self.idle_fire(clock);
        }
        for i in 0..self.n() as i32 {
            if self.alive(i) && self.u(i).flags & flag::NOT_PLACED == 0 {
                self.tank_refresh(i, clock);
            }
        }
        self.status();
        if !self.ctx.multiplayer {
            if self.field.countdown != -1 {
                self.field.countdown -= 1;
                if self.field.countdown == -1 {
                    let st = self.status();
                    self.finish(st, false);
                    self.field.finished_flag = true;
                }
            }
        } else if self.ctx.mp_war {
            // MP (exe 0x494b70..): no countdown; a holder ends the war at once.
            let st = self.status();
            if st != CONTESTED {
                self.finish(st, false);
                self.field.finished_flag = true;
                self.events.push(super::GroundEvent::Ended { holder: st });
            }
        }
    }

    /// `FRISSIT_Grwar`'s part for one planet (exe 0x4747c0, ELF 0x49a7f0, spec §11.7), before `WF::MI` and
    /// `WF::Refresh` of the shown planet: the self-destruct countdown once per battle step, and every 300th
    /// pass (`gvalue`, exe 0x9B2900, already counted up) the slot compaction.
    pub fn grwar_planet(&mut self, clock: Clock, gvalue: i32) {
        for _ in 0..clock.speed {
            if self.field.self_destruct > -1 {
                self.field.self_destruct -= 1;
            }
            if self.field.self_destruct == 0 {
                self.self_destruct_now();
                self.field.self_destruct = -1;
            }
        }
        if gvalue % 300 == 0 {
            self.collapse();
        }
    }

    /// `SelfDestruct(p)` (exe 0x499500): every building blows up (a burst on screen whose height is the
    /// cell x [original bug]), then every group takes 10 000 000 (`WF_Tank::Damage`, no voice draw).
    fn self_destruct_now(&mut self) {
        for slot in 1..self.planet.buildings.len() as i32 {
            if self.planet.buildings[slot as usize].kind != 0 {
                if self.ctx.on_screen {
                    let b = self.planet.buildings[slot as usize];
                    let (x, y) = grid_to_real(b.x as i32, b.y as i32, self.rules.map_size);
                    self.burst([b.x, x, y], 16, true);
                }
                self.building_delete(slot);
            }
            self.remove_target(-slot);
        }
        for i in 0..self.n() as i32 {
            if self.alive(i) && self.unit_damage(i, 10_000_000, NONE) {
                self.remove_group(i);
                self.remove_target(i);
            }
        }
        self.events.push(super::GroundEvent::SelfDestructed);
    }

    /// `WF::Collapse` (exe 0x460d70): trailing empty slots of list A, list B and the units go (list C keeps
    /// its length).
    pub fn collapse(&mut self) {
        let l = &mut self.field.lists;
        while matches!(l.missiles.last(), Some(None)) {
            l.missiles.pop();
        }
        while matches!(l.shells.last(), Some(None)) {
            l.shells.pop();
        }
        let n = self.field.alive.iter().rposition(|&a| a).map_or(0, |k| k + 1);
        self.field.alive.truncate(n);
        self.field.units.truncate(n);
    }

    /// One round of list A: homing shells.
    fn shells_step(&mut self) {
        let mut k = 0;
        while k < self.field.lists.shells.len() {
            let Some(mut sh) = self.field.lists.shells[k].clone() else {
                k += 1;
                continue;
            };
            sh.life -= 1;
            if sh.life == 0 {
                self.field.lists.shells[k] = None;
                k += 1;
                continue;
            }
            for _ in 0..3 {
                (self.libc)();
            }
            sh.pos = [sh.vel[0] + sh.pos[0], sh.vel[1] + sh.pos[1], sh.vel[2] + sh.pos[2]];
            if sh.target == NONE {
                self.field.lists.shells[k] = Some(sh);
                k += 1;
                continue;
            }
            let tp = if sh.target < 0 {
                let b = self.planet.buildings.get((-sh.target) as usize).copied().unwrap_or_default();
                let (x, y) = grid_to_real(b.x as i32, b.y as i32, self.rules.map_size);
                [self.planet.building_z.get((-sh.target) as usize).copied().unwrap_or(0.0), x, y]
            } else {
                let u = self.u(sh.target);
                [u.rt.z, u.pos.0, u.pos.1]
            };
            let d = [tp[0] - sh.pos[0], tp[1] - sh.pos[1], tp[2] - sh.pos[2]];
            let mut n = d;
            normalise(&mut n);
            let pull = [sh.pull * n[0], sh.pull * n[1], sh.pull * n[2]];
            let dist = sqrtf(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
            let mut jit = 0.0f32;
            if sh.target >= 0 && self.u(sh.target).flags & flag::STEALTH != 0 && dist < 500.0 {
                let r = self.draw(0, 0x45);
                // f64 product, then ×5 and the stealth byte in f32 (asm 0x49111e..0x491169).
                let fall = -((500.0 - dist) / 500.0);
                let f = ((r as f64 / 100.0 + 0.3) * fall as f64) as f32 * 5.0;
                jit = sh.jitter as f32 * f;
            }
            sh.vel[0] += jit + pull[0];
            sh.vel[1] += pull[1];
            sh.vel[2] += pull[2];
            let sp = sqrtf(sh.vel[0] * sh.vel[0] + sh.vel[1] * sh.vel[1] + sh.vel[2] * sh.vel[2]);
            if 20.0 < sp {
                sh.vel = [sh.vel[0] / sp * 20.0, sh.vel[1] / sp * 20.0, sh.vel[2] / sp * 20.0];
            }
            if dist < 60.0 {
                self.hit_fx(sh.pos);
                let dmg = self.real_damage(sh.target, sh.dmg, sh.vel[1], sh.vel[2]);
                if self.damage(sh.target, dmg, sh.by) {
                    self.kill(sh.target);
                }
                self.field.lists.shells[k] = None;
            } else {
                self.field.lists.shells[k] = Some(sh);
            }
            k += 1;
        }
    }

    /// The hit of a list-B shell/rocket/bolt within 40 of its aim point.
    fn missile_close(m: &Missile) -> bool {
        dist3(&m.pos, &m.dest) < 40.0
    }

    /// One round of list B.
    fn missiles_step(&mut self) {
        let mut k = 0;
        while k < self.field.lists.missiles.len() {
            let Some(mut m) = self.field.lists.missiles[k].clone() else {
                k += 1;
                continue;
            };
            m.life = m.life.wrapping_sub(1);
            if m.life == 0 {
                self.field.lists.missiles[k] = None;
                k += 1;
                continue;
            }
            match m.kind {
                0 | 4 | 5 | 6 => {
                    for _ in 0..3 {
                        (self.libc)();
                    }
                    m.pos = [m.vel[0] + m.pos[0], m.vel[1] + m.pos[1], m.vel[2] + m.pos[2]];
                    if m.target != NONE && Self::missile_close(&m) {
                        self.hit_fx(m.pos);
                        let dmg = self.real_damage(m.target, m.dmg, m.vel[1], m.vel[2]);
                        if self.damage(m.target, dmg, m.by) {
                            self.kill(m.target);
                        }
                        self.field.lists.missiles[k] = None;
                        k += 1;
                        continue;
                    }
                }
                3 => {
                    for _ in 0..3 {
                        (self.libc)();
                    }
                    m.pos = [m.vel[0] + m.pos[0], m.vel[1] + m.pos[1], m.vel[2] + m.pos[2]];
                    if m.target != NONE && Self::missile_close(&m) {
                        self.hit_fx(m.pos);
                        let t = m.target;
                        if m.dmg & 1 != 0 && t >= 0 && t != NONE && self.u(t).flags & flag::TURRET == 0 {
                            let n = self.alive_members(t);
                            let chance = ((m.dmg / 100) as f64 * 40.0 / n as f64) as i32;
                            if self.damage(t, 2000, m.by) {
                                self.remove_group(t);
                            } else if self.draw(0, 100) <= chance {
                                let owner = (m.dmg % 100 - 1) / 2;
                                let u = self.um(t);
                                u.flags |= flag::SEIZED;
                                u.owner = owner;
                                u.seized = 200;
                                u.flags &= 0xfffff6ff;
                                if u.flags & flag::KEEP_ORDERS == 0 {
                                    u.move_state = (0, -1);
                                    u.flags &= 0xfffff27f;
                                    u.dest = u.cell;
                                    u.target = NONE;
                                }
                            } else {
                                self.field.lists.missiles[k] = None;
                                k += 1;
                                continue;
                            }
                            self.remove_target(t);
                        }
                        self.field.lists.missiles[k] = None;
                        k += 1;
                        continue;
                    }
                }
                1 | 2 => {
                    for _ in 0..3 {
                        (self.libc)();
                    }
                    m.pos = [m.vel[0] + m.pos[0], m.vel[1] + m.pos[1], m.vel[2] + m.pos[2]];
                    let dist = dist3(&m.pos, &m.dest);
                    let (near, radius) = if m.kind == 1 { (100.0, 3600.0) } else { (40.0, 2500.0) };
                    if (m.pos[0] - m.target as f32).abs() < near {
                        let bursts = if m.kind == 1 { 3 } else { 1 };
                        for _ in 0..bursts * 5 {
                            (self.libc)();
                        }
                        for j in 0..self.n() as i32 {
                            if !self.alive(j) {
                                continue;
                            }
                            let v = self.u(j);
                            if v.flags & flag::NOT_PLACED != 0 || v.flags & flag::TURRET != 0 {
                                continue;
                            }
                            let dy = v.pos.1 - m.pos[2];
                            let dx = v.pos.0 - m.pos[1];
                            if dx * dx + dy * dy < radius {
                                self.hit_fx(m.pos);
                                let dmg = if m.kind == 1 { m.dmg.wrapping_mul(20) } else { (m.dmg as f64 * 1.5) as i32 };
                                if self.damage(j, dmg, m.by) {
                                    self.kill(j);
                                }
                            }
                        }
                        let bd = if m.kind == 1 { m.dmg } else { m.dmg / 0x5dc };
                        self.damage_buildings(m.pos[0], m.pos[1], m.pos[2], bd);
                    }
                    if dist < 40.0 {
                        self.field.lists.missiles[k] = None;
                        k += 1;
                        continue;
                    }
                }
                _ => {}
            }
            self.field.lists.missiles[k] = Some(m);
            k += 1;
        }
    }

    /// One round of list C: paralyser bolts.
    fn bolts_step(&mut self) {
        let mut k = 0;
        while k < self.field.lists.bolts.len() {
            let Some(mut b) = self.field.lists.bolts[k].clone() else {
                k += 1;
                continue;
            };
            b.life -= 1;
            if b.life == 0 {
                self.field.lists.bolts[k] = None;
                k += 1;
                continue;
            }
            for _ in 0..3 {
                (self.libc)();
            }
            b.pos = [b.vel[0] + b.pos[0], b.vel[1] + b.pos[1], b.vel[2] + b.pos[2]];
            if b.target != NONE && dist3(&b.pos, &b.dest) < 40.0 {
                (self.libc)();
                let t = b.target;
                if b.power != 0 && t >= 0 && self.u(t).flags & flag::TURRET == 0 {
                    let n = self.alive_members(t);
                    let chance = (b.power as f64 * 40.0 / 100.0 / n as f64) as i32;
                    if self.damage(t, 2000, NONE) {
                        self.kill(t);
                    } else if self.draw(0, 100) <= chance {
                        let u = self.um(t);
                        u.flags |= flag::PARALYSED;
                        u.paralysed = 200;
                    }
                }
                self.field.lists.bolts[k] = None;
            } else {
                self.field.lists.bolts[k] = Some(b);
            }
            k += 1;
        }
    }

    /// Idle units shoot at what comes into range (only while the field is contested).
    fn idle_fire(&mut self, clock: Clock) {
        for i in 0..self.n() as i32 {
            if !self.alive(i) {
                continue;
            }
            let u = self.u(i);
            let f = u.flags;
            if f & (flag::NOT_PLACED | flag::NOT_COUNTED) != 0 || u.flags2 & 4 != 0 {
                continue;
            }
            let idle = f & flag::HAS_TARGET == 0 || u.target == NONE;
            if !(idle && ((u.move_state.0 != 0 && f & 0x400 != 0) || f & 0x400 == 0)) {
                continue;
            }
            if u.aim != NONE {
                let t = u.aim;
                let tv = self.u(t);
                let dx = (u.cell.0 - tv.cell.0) as f32;
                let dy = (u.cell.1 - tv.cell.1) as f32;
                let d = sqrtf(dy * dy + dx * dx);
                if u.rt.range <= d || tv.owner == u.owner {
                    self.um(i).aim = NONE;
                } else {
                    self.shot_one(i, t, clock);
                }
            }
            if self.u(i).aim == NONE {
                let t = self.nearest_enemy(i);
                if t != NONE {
                    let (u, tv) = (self.u(i), self.u(t));
                    let dx = (u.cell.0 - tv.cell.0) as f32;
                    let dy = (u.cell.1 - tv.cell.1) as f32;
                    if sqrtf(dy * dy + dx * dx) < u.rt.range {
                        self.shot_one(i, t, clock);
                        self.um(i).aim = t;
                    }
                }
            }
        }
    }
}
