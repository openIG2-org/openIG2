//! The real-time ground battle tick, one unit's part: `WF_Tank::Refresh` (exe 0x494bd0; spec
//! `re/rules/ground-combat.md` §11.8) with its aim, turret and body turning, straight and path driving,
//! attack-move, regeneration, reloads, mine laying and mine contact. Float order and precision follow the
//! exe's asm (several turn rates are f64, the brakes are f64, sums and products f32).

use super::{flag, realtime::real_to_grid, War, NONE};

/// `IdoSpeedSzorzoBattle` and `IdoPause` as the tick sees them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    /// `IdoSpeedSzorzoBattle` (exe 0xBAE260): battle steps per tick (1..=4).
    pub speed: i32,
    /// `IdoPause` (exe 0xBAE264).
    pub paused: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self { speed: 1, paused: false }
    }
}

impl Clock {
    /// `(IdoPause == 0) · S` as an int (the loop counts and the integer increments).
    pub(super) fn steps(self) -> i32 {
        (!self.paused) as i32 * self.speed
    }
    pub(super) fn run(self) -> f32 {
        (!self.paused) as i32 as f32
    }
}

/// `sqrt` as the exe calls it on an f32 value: through f64, rounded back.
pub(super) fn sqrtf(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}
fn absf(x: f32) -> f32 {
    x.abs()
}
const TWO_PI_F32_AS_F64: f64 = 6.2831854820251465;
const TWO_PI: f64 = std::f64::consts::TAU;
const PI: f64 = std::f64::consts::PI;

/// The exe's inlined `KetSzogAltalBezartSzog(from, to)`: the signed shortest turn from `from` to `to`,
/// with both angles reduced by x87 `fmod` against the f32 2π (as a double).
fn shortest_turn(from: f32, to: f32) -> f32 {
    let t = ((to as f64) % TWO_PI_F32_AS_F64) as f32;
    let a = ((from as f64) % TWO_PI_F32_AS_F64) as f32;
    let mut d = t - a;
    let ad = absf(d);
    let mut alt = (TWO_PI - ad as f64) as f32;
    if d > 0.0 {
        alt = -alt;
    }
    if absf(alt) <= ad {
        d = alt;
    }
    d
}

/// The exe's direction-to-angle formula (`2·asin(|(1, 0) − v| / 2)`, mirrored below y = 0) for a unit
/// vector `(dx, dy)`.
fn angle_of(dx: f32, dy: f32) -> f32 {
    let mut n = sqrtf((-dy) * (-dy) + (1.0 - dx) * (1.0 - dx));
    if n > 2.0 {
        n = 2.0;
    }
    let a = ((n * 0.5) as f64).asin() as f32 * 2.0;
    if 0.0 > dy {
        (TWO_PI - a as f64) as f32
    } else {
        a
    }
}

impl War<'_> {
    /// `AliveTankNum` (exe 0x45f380): members with hit points, cached for the tick (`+0x125/+0x128`).
    pub(super) fn alive_members(&mut self, i: i32) -> i32 {
        let u = self.um(i);
        if u.rt.alive_cached != 0 {
            return u.rt.alive_n;
        }
        let n = u.hp.iter().filter(|&&h| h > 0).count() as i32;
        u.rt.alive_cached = 1;
        u.rt.alive_n = n;
        n
    }

    /// `CalcDamage(k)` (exe 0x45fb40): one stream-2 draw in the weapon's damage range (turrets: the
    /// fortress table).
    pub(super) fn calc_damage(&mut self, i: i32, k: usize) -> i32 {
        let table = if self.u(i).flags & flag::TURRET != 0 { &self.rules.fortress } else { &self.rules.rt_weapons };
        let w = table.get(k).copied().unwrap_or([0; 4]);
        self.draw(w[0], w[1])
    }

    /// `WF_Tank::Stop` inlined (0x4a4cb0): orders cleared unless kept.
    pub(super) fn stop_orders(&mut self, i: i32) {
        let u = self.um(i);
        if u.flags & flag::KEEP_ORDERS == 0 {
            u.move_state = (0, -1);
            u.flags &= 0xfffff27f;
            u.dest = u.cell;
            u.target = NONE;
        }
    }

    /// `WF_Tank::MoveG(x, y)` (exe 0x48bdc0): drive to a cell (clamped to 10..=190).
    pub(super) fn move_g(&mut self, i: i32, x: i32, y: i32) {
        let u = self.um(i);
        let f = u.flags;
        if f & 0x4020000 != 0 || !(0..200).contains(&x) || !(0..200).contains(&y) || f & 0x28000000 != 0 || u.transport & 8 != 0 {
            return;
        }
        let (x, y) = (x.clamp(10, 190), y.clamp(10, 190));
        if (x, y) != u.dest {
            u.move_state = (0, -1);
        }
        u.dest = (x, y);
        u.target = NONE;
        u.flags = (f & 0xfffff6ff) | 0x400;
    }

    /// The map position of target `t` (a unit's, or a building's cell without the half-cell offset)
    /// and its height.
    pub(super) fn target_pos(&self, t: i32) -> (f32, f32, f32) {
        if t < 0 {
            let b = self.planet.buildings.get((-t) as usize).copied().unwrap_or_default();
            let (x, y) = super::grid_to_real(b.x as i32, b.y as i32, self.rules.map_size);
            (x, y, self.planet.building_z.get((-t) as usize).copied().unwrap_or(0.0))
        } else {
            let u = self.u(t);
            (u.pos.0, u.pos.1, u.rt.z)
        }
    }

    /// The cell of target `t` (a building's int position).
    pub(super) fn target_cell(&self, t: i32) -> (i32, i32) {
        if t < 0 {
            let b = self.planet.buildings.get((-t) as usize).copied().unwrap_or_default();
            (b.x as i32, b.y as i32)
        } else {
            self.u(t).cell
        }
    }

    /// One dust particle: a libc draw (`rand() % 5`), no state.
    fn dust(&mut self) {
        (self.libc)();
    }

    /// Snap the position to the step point, re-orient, take the cell (the end of a drive).
    fn arrive(&mut self, i: i32) {
        let u = self.um(i);
        u.rt.step = u.pos;
        self.set_matrix(i);
        let size = self.rules.map_size;
        let u = self.um(i);
        u.cell = real_to_grid(u.pos.0, u.pos.1, size);
        u.rt.moved = 1;
        u.flags |= 0x40;
    }

    /// Re-aim at the step point: `dir` = the normalised offset and the wanted body angle from it.
    fn want_step(&mut self, i: i32) {
        let u = self.um(i);
        let (sx, sy) = u.rt.step;
        let (px, py) = u.pos;
        let mut len = sqrtf((sx - px) * (sx - px) + (sy - py) * (sy - py));
        if len == 0.0 {
            len = 1.0;
        }
        let dx = (sx - px) / len;
        let dy = (sy - py) / len;
        u.rt.dir = (dx, dy);
        u.rt.want_angle = angle_of(dx, dy);
    }

    /// [`Self::want_step`] after a path search; `set_dir` → `dir := (cos, sin)` of the wanted angle (path
    /// driving does, attack-move doesn't). Flag 0x1000 when the body must turn.
    fn aim_drive(&mut self, i: i32, set_dir: bool) {
        let u = self.um(i);
        let (sx, sy) = u.rt.step;
        let (px, py) = u.pos;
        let mut len = sqrtf((sx - px) * (sx - px) + (sy - py) * (sy - py));
        if len == 0.0 {
            len = 1.0;
        }
        let dx = (sx - px) / len;
        let dy = (sy - py) / len;
        u.rt.dir = (dx, dy);
        u.rt.want_angle = angle_of(dx, dy);
        if set_dir {
            let w = u.rt.want_angle as f64;
            u.rt.dir = (w.cos() as f32, w.sin() as f32);
        }
        if u.rt.want_angle != u.rt.angle {
            u.flags |= 0x1000;
        }
    }

    /// `StoryMadTankFrissit(u)` (exe 0x5c5370, stream 5): a story "mad tank" (flag 4) of the human picks its
    /// own targets: 3 % the nearest enemy, else 1 % the nearest finished building (one within 7 cells
    /// may win by a running reservoir draw), else 5 % it drops its target.
    fn mad_tank(&mut self, i: i32) {
        use crate::rng::stream::STORY;
        let u = self.u(i);
        if u.owner != self.ctx.human {
            return;
        }
        if u.flags & 0x500 == 0 {
            if self.rng.range(STORY, 0, 99) < 3 {
                let t = self.nearest_enemy(i);
                if t != NONE {
                    self.set_target(i, t);
                }
                return;
            }
            if self.rng.range(STORY, 0, 99) == 0 {
                // `RndValasztas` with weight 1 (0x4e3630 on {count, sum, stream 5}).
                let (mut best, mut best_d, mut sum) = (0usize, 0i32, 0i32);
                let cell = self.u(i).cell;
                for slot in 1..self.planet.buildings.len() {
                    let b = self.planet.buildings[slot];
                    if b.kind == 0 || b.work_left != 0 {
                        continue;
                    }
                    let dx = cell.0.wrapping_sub(b.x as i32);
                    let dy = cell.1.wrapping_sub(b.y as i32);
                    let d = sqrtf(dy.wrapping_mul(dy).wrapping_add(dx.wrapping_mul(dx)) as f32) as i32;
                    let take = best == 0 || (d < 7 && {
                        sum += 1;
                        self.rng.range(STORY, 1, sum) <= 1
                    }) || d < best_d;
                    if take {
                        best = slot;
                        best_d = d;
                    }
                }
                if best != 0 {
                    self.set_target(i, -(best as i32));
                }
                return;
            }
        }
        if self.rng.range(STORY, 0, 99) < 5 {
            self.um(i).flags &= 0xfffff6ff;
        }
    }

    /// The path step of a move (0x400) or an attack-move (0x800): every 15 ticks (or once the step is
    /// reached) search the next cell with `grUtkeres` and set the step point there.
    fn path_step(&mut self, i: i32, attack: bool) {
        let u = self.u(i);
        let at_step = u.rt.step == u.pos;
        if !(at_step || u.rt.path_timer == 0) {
            return;
        }
        let size = self.rules.map_size;
        let u = self.um(i);
        u.rt.path_timer = 0xf;
        u.cell = real_to_grid(u.pos.0, u.pos.1, size);
        let radius = if attack {
            let r = if u.target < 0 { u.rt.bld_range } else { u.rt.range };
            if !(0..200).contains(&u.dest.0) || !(0..200).contains(&u.dest.1) {
                u.dest = u.cell;
            }
            r as i32
        } else {
            1
        };
        let (cell, dest, index, planet) = (u.cell, u.dest, u.index, u.rt.planet);
        let out = self.find_path(cell, dest, radius, index, planet);
        let u = self.um(i);
        u.last_cell = out.next;
        if out.arrived {
            u.flags &= if attack { !0x800 } else { !0x400 };
            return;
        }
        if u.cell == u.last_cell {
            u.rt.step = u.pos;
            return;
        }
        let (w, h) = size;
        let gx = (out.next.0 as f64 + 0.5) as f32;
        let gy = (out.next.1 as f64 + 0.5) as f32;
        let x = (-((gx * 0.5 / 99.0 * 100.0 * w / 99.0) as f64 - w as f64 * 0.5)) as f32;
        let y = ((gy * 0.5 / 99.0 * 100.0 * h / 99.0) as f64 - h as f64 * 0.5) as f32;
        u.rt.step = (x, y);
        self.aim_drive(i, !attack);
    }

    /// The drive towards the step point of a move or attack-move (both branches of `WF_Tank::Refresh`;
    /// `attack` adds the acceleration twice and never clamps the speed at 0 while accelerating).
    /// Returns true when the unit reached its step point (the function returns there).
    fn drive(&mut self, i: i32, attack: bool, drove_straight: bool, clock: Clock) -> bool {
        let steps = clock.steps();
        let u = self.u(i);
        if u.pos == u.rt.step {
            // At the step point: coast to a stop.
            for _ in 0..steps {
                let u = self.um(i);
                if u.rt.velocity == 0.0 {
                    continue;
                }
                let v = (u.rt.velocity as f64 - u.rt.decel as f64 * 1.5) as f32;
                u.rt.velocity = if 0.0 > v { 0.0 } else { v };
                let v = u.rt.velocity;
                u.pos = (u.rt.dir.0 * v + u.pos.0, u.rt.dir.1 * v + u.pos.1);
                u.rt.step = u.pos;
                self.set_matrix(i);
                self.dust();
                let u = self.um(i);
                u.flags |= 0x40;
                u.rt.moved = 1;
            }
        } else {
            let d = shortest_turn(u.rt.angle, u.rt.want_angle);
            const LIMIT: f64 = 1.4959965017094252;
            let straight = |u: &super::Group| u.rt.blocked == 0 && LIMIT > absf(d) as f64;
            for _ in 0..steps {
                let u = self.um(i);
                if straight(u) {
                    if attack {
                        let a = u.rt.accel;
                        let t = if drove_straight { u.rt.velocity - a } else { a + u.rt.velocity };
                        u.rt.velocity = a + t;
                        if u.rt.velocity > u.rt.max_speed {
                            u.rt.velocity = u.rt.max_speed;
                        }
                    } else {
                        let v = if drove_straight { u.rt.velocity - u.rt.accel } else { u.rt.velocity + u.rt.accel };
                        u.rt.velocity = if v > u.rt.max_speed { u.rt.max_speed } else { v };
                        if 0.0 > u.rt.velocity {
                            u.rt.velocity = 0.0;
                        }
                    }
                } else {
                    let v = u.rt.velocity - u.rt.decel;
                    u.rt.velocity = if 0.0 > v { 0.0 } else { v };
                }
            }
            let u = self.um(i);
            if straight(u) || u.rt.velocity != 0.0 {
                self.want_step(i);
                let run = clock.run();
                let s = clock.speed as f32;
                let u = self.um(i);
                let a = u.rt.angle as f64;
                u.rt.dir = (a.cos() as f32, a.sin() as f32);
                let v = u.rt.velocity;
                u.pos.0 += run * (v * u.rt.dir.0) * s;
                u.pos.1 += run * (v * u.rt.dir.1) * s;
                self.set_matrix(i);
                self.dust();
                let u = self.um(i);
                u.flags |= 0x40;
                u.rt.moved = 1;
            } else {
                u.rt.blocked = 1;
            }
        }
        let u = self.u(i);
        let dx = u.rt.step.0 - u.pos.0;
        let dy = u.rt.step.1 - u.pos.1;
        let dist = sqrtf(dx * dx + dy * dy);
        if dist < clock.speed as f32 * u.rt.velocity {
            self.arrive(i);
            return true;
        }
        false
    }

    /// `WF_Tank::Refresh(wf)` (exe 0x494bd0, spec §11.8): one unit's tick.
    pub fn tank_refresh(&mut self, i: i32, clock: Clock) {
        let steps = clock.steps();
        let s = clock.speed;
        {
            let u = self.um(i);
            u.rt.moved = 0;
            u.rt.alive_cached = 0;
            if u.rt.path_timer != 0 {
                u.rt.path_timer = (u.rt.path_timer - s).max(0);
            }
            if u.flags & flag::PARALYSED != 0 {
                u.paralysed -= s;
                if u.paralysed < 1 {
                    u.flags &= !flag::PARALYSED;
                }
            }
            if u.flags & flag::PARALYSED != 0 {
                return;
            }
            for k in 0..14 {
                if u.rt.reload_left[k] < u.rt.reload[k] {
                    u.rt.reload_left[k] += steps as f32;
                }
            }
        }
        self.lay_mines(i);
        {
            let u = self.um(i);
            for k in 0..14 {
                if u.rt.reload_left[k] > u.rt.reload[k] {
                    u.rt.reload_left[k] = u.rt.reload[k];
                }
            }
            if u.regen != 0 && u.flags & flag::SEIZED == 0 {
                let half = u.max_hp / 2;
                let add = u.regen.wrapping_mul(steps);
                for h in u.hp.iter_mut() {
                    if *h < half {
                        *h = h.wrapping_add(add);
                        if *h > half {
                            *h = half;
                        }
                    }
                }
            }
        }
        // Coasting without orders.
        for _ in 0..steps {
            let u = self.um(i);
            if u.flags & 0xc00 != 0 || u.rt.velocity == 0.0 {
                continue;
            }
            let v = (u.rt.velocity as f64 - u.rt.decel as f64 * 1.5) as f32;
            u.rt.velocity = if 0.0 > v { 0.0 } else { v };
            let v = u.rt.velocity;
            u.pos = (u.rt.dir.0 * v + u.pos.0, u.rt.dir.1 * v + u.pos.1);
            u.rt.step = u.pos;
            self.set_matrix(i);
            self.dust();
            let u = self.um(i);
            u.flags |= 0x40;
            u.rt.moved = 1;
        }
        let f = self.u(i).flags;
        if f == 0x4000001 || f == 1 {
            return;
        }
        let u = self.um(i);
        if f & 0x1000 == 0 {
            u.rt.blocked = 0;
        }
        if f & flag::SEIZED != 0 {
            u.seized -= s;
            if u.seized < 1 {
                u.flags = f & !flag::SEIZED;
                if f & flag::KEEP_ORDERS == 0 {
                    u.dest = u.cell;
                    u.move_state = (0, -1);
                    u.target = NONE;
                    u.flags = f & 0xfefff27f;
                }
                u.owner = u.side;
                u.rt.moved = 1;
            }
        }
        if self.u(i).flags & 0x400 == 0 {
            let u = self.um(i);
            let n = u.rt.waypoints.len() as i32;
            if u.rt.wp_cursor == n {
                u.rt.waypoints.clear();
                u.rt.wp_cursor = 0;
            } else {
                let (x, y) = u.rt.waypoints[u.rt.wp_cursor.max(0) as usize];
                u.rt.wp_cursor += 1;
                if u.flags & 0x80000000 == 0 {
                    self.move_g(i, x, y);
                }
            }
        }
        if self.u(i).flags & flag::MAD != 0 {
            self.mad_tank(i);
        }
        if self.u(i).flags & flag::HAS_TARGET != 0 {
            self.aim_and_fire(i, clock);
        }
        if self.u(i).flags & 0x1000 != 0 && self.turn_body(i, clock) {
            return;
        }
        let mut drove_straight = false;
        let u = self.u(i);
        if u.flags & 0x80 != 0 && u.flags & 0x1000 == 0 {
            let dx = u.rt.step.0 - u.pos.0;
            let dy = u.rt.step.1 - u.pos.1;
            let dist = sqrtf(dx * dx + dy * dy);
            let v = u.rt.speed;
            if dist < v * clock.speed as f32 {
                let u = self.um(i);
                u.pos = u.rt.step;
                self.set_matrix(i);
                let u = self.um(i);
                u.rt.moved = 1;
                u.flags = (u.flags & 0xffffff7f) | 0x40;
                return;
            }
            let run = clock.run();
            let sf = clock.speed as f32;
            let u = self.um(i);
            u.pos.0 += u.rt.dir.0 * v * run * sf;
            u.pos.1 += u.rt.dir.1 * v * run * sf;
            self.set_matrix(i);
            let size = self.rules.map_size;
            let u = self.um(i);
            u.cell = real_to_grid(u.pos.0, u.pos.1, size);
            self.dust();
            let u = self.um(i);
            u.flags |= 0x40;
            u.rt.moved = 1;
            drove_straight = true;
        }
        if self.u(i).flags & 0x400 != 0 {
            self.path_step(i, false);
            if self.u(i).flags & 0x400 != 0 && self.drive(i, false, drove_straight, clock) {
                return;
            }
        }
        let f = self.u(i).flags;
        if f & 0x800 != 0 && f & 0x1000 == 0 {
            self.path_step(i, true);
            if self.u(i).flags & 0x800 != 0 && self.drive(i, true, drove_straight, clock) {
                return;
            }
        }
        self.mine_contact(i);
    }

    /// The target part of `WF_Tank::Refresh`: aim, gun elevation, turret turn, fire or close in.
    // The negated compares keep the exe's NaN behaviour (`comiss` + `ja`).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn aim_and_fire(&mut self, i: i32, clock: Clock) {
        let t = self.u(i).target;
        let tc = self.target_cell(t);
        let (tx, ty, tz) = self.target_pos(t);
        let u = self.um(i);
        let cdx = (tc.0 - u.cell.0) as f32;
        let cdy = (tc.1 - u.cell.1) as f32;
        let dcell = sqrtf(cdx * cdx + cdy * cdy);
        let dy = ty - u.pos.1;
        let dx = tx - u.pos.0;
        let len = sqrtf(dy * dy + dx * dx);
        let mut a = ((dy / len) as f64).asin() as f32;
        if 0.0 > dx / len {
            a = (PI - a as f64) as f32;
        }
        u.rt.aim = a;
        // Gun elevation: a ballistic solution (all f32, roots through f64). Render-only: nothing in the
        // battle reads `+0x1c8` (spec §11.8).
        let h = absf(tz - u.rt.z);
        let hh = h * 0.01f32;
        let q = ((625.0f32 - hh * 5.0 * 5.0) - len * 0.0004f32 * len) - h * 0.0003f32 * h;
        if !(0.0 > q) {
            let w = ((25.0 - hh) - sqrtf(q)) / 0.0002f32;
            if !(0.0 > w) {
                let t2 = len / sqrtf(w);
                let r3 = sqrtf(25.0 - t2 * t2);
                u.rt.elevation = if t2 == 0.0 {
                    0.0
                } else {
                    let e = ((r3 / t2) as f64).tan() as f32;
                    (e as f64 / std::f64::consts::FRAC_PI_4 * 101.0) as f32
                };
            }
        }
        if u.flags & flag::MINE != 0 {
            u.rt.turret_angle = a;
        }
        let ta = u.rt.turret_angle;
        if a != ta {
            let d = ta - a;
            if absf(d) as f64 > PI {
                u.rt.turret_angle = if d > 0.0 { (ta as f64 - TWO_PI) as f32 } else { (ta as f64 + TWO_PI) as f32 };
            }
            u.flags |= 0x2000;
        }
        if u.flags & 0x2000 != 0 {
            let ta = u.rt.turret_angle;
            if ta != a {
                let d = ta - a;
                let ad = absf(d);
                let lim = u.rt.turn.1 as f64 / 5.0 * clock.speed as f64;
                if lim > ad as f64 {
                    u.flags &= !0x2000;
                    u.rt.turret_angle = a;
                } else {
                    let step = (((d / ad) * u.rt.turn.0) as f64 / 5.0) * clock.run() as f64 * clock.speed as f64;
                    u.rt.turret_angle = (ta as f64 - step) as f32;
                }
            }
            u.flags |= 0x4000;
        }
        if !(0.1 > (a - u.rt.turret_angle) as f64) {
            return;
        }
        let range = if t < 0 { u.rt.bld_range } else { u.rt.range } as i32 as f32;
        if dcell <= range {
            for k in 0..14 {
                if self.u(i).rt.reload_left[k] >= self.u(i).rt.reload[k] {
                    self.shot(i, t, k);
                    let r = self.draw(0, 4);
                    let u = self.um(i);
                    u.rt.reload_left[k] = r as f32 + (u.rt.reload_left[k] - u.rt.reload[k]);
                }
            }
        } else if u.flags & 0x2c020000 == 0 {
            u.rt.waypoints.clear();
            u.rt.wp_cursor = 0;
            if tc != u.dest {
                u.move_state = (0, -1);
            }
            u.dest = tc;
            u.flags = (u.flags & 0xfffffbff) | 0x800;
        }
    }

    /// The body turn (flag 0x1000). Returns true when the function returns early (a partial turn).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn turn_body(&mut self, i: i32, clock: Clock) -> bool {
        let u = self.um(i);
        let f = u.flags;
        let d = shortest_turn(u.rt.angle, u.rt.want_angle);
        let mut a = ((u.rt.angle as f64) % TWO_PI_F32_AS_F64) as f32;
        if 0.0 > a {
            a = (TWO_PI - a as f64) as f32;
        }
        u.rt.angle = a;
        let want = a + d;
        u.rt.want_angle = want;
        if 0.01 > absf(d) as f64 {
            u.flags = f & !0x1000;
            u.rt.angle = want;
        } else {
            let ad = absf(a - want);
            let turn = u.rt.turn.0;
            let lim = turn as f64 / 5.0 * clock.speed as f64;
            if !(lim > ad as f64) {
                // A partial turn; the turret keeps its world direction.
                let na = (a as f64 - (((a - want) / ad * turn) as f64 / 5.0) * clock.run() as f64 * clock.speed as f64) as f32;
                u.rt.angle = na;
                if f & 0x2000 == 0 {
                    let r = na - u.rt.want_angle;
                    if r != 0.0 {
                        let step = (((r / absf(r)) * turn) as f64 / 5.0) * clock.run() as f64 * clock.speed as f64;
                        u.rt.turret_angle = (u.rt.turret_angle as f64 - step) as f32;
                    }
                    wrap(&mut u.rt.turret_angle);
                }
                self.set_matrix(i);
                self.um(i).flags |= 0x200;
                return false;
            }
            if f & 0x2000 == 0 {
                u.rt.turret_angle += want - a;
                wrap(&mut u.rt.turret_angle);
            }
            u.rt.angle = want;
            self.set_matrix(i);
            self.um(i).flags &= !0x1000;
        }
        let u = self.um(i);
        u.flags |= 0x200;
        u.rt.blocked = 0;
        false
    }

    /// Mine laying (`+0x1d0` mine bits with `+0x250` on): a mine in every new cell while ammunition lasts.
    fn lay_mines(&mut self, i: i32) {
        let u = self.um(i);
        if u.weapons == 0 {
            return;
        }
        if u.rt.laying != 0 && u.rt.mine_cell == u.cell {
            u.rt.laying = 0;
        }
        if u.rt.laying == 0 || u.rt.mine_cell == u.cell {
            return;
        }
        u.rt.laying = 0;
        if u.ammo > 0 {
            // The damage handed to `AddMine` is never stored (the draws still happen).
            if u.flags & flag::PARTNER == 0 {
                self.calc_damage(i, 2);
                self.alive_members(i);
            }
            let u = self.u(i);
            let (cell, owner, w) = (u.cell, u.owner, u.weapons);
            for (bit, hp, kind) in [(0x4000u32, 3000, 0x4000u32), (0x8000, 5000, 0x8000), (0x10000, 10000, 0x100), (0x20000, 20000, 2)] {
                if w & bit != 0 {
                    self.add_mine(cell, owner, hp, kind);
                }
            }
            self.um(i).ammo -= 1;
        }
        let u = self.um(i);
        u.rt.mine_cell = u.cell;
    }

    /// `WF::AddMine(x, y, owner, dmg, hp, weapon)` (exe 0x45c770): a mine unit on a free cell (none if a
    /// mine is already there). Returns its slot.
    pub fn add_mine(&mut self, cell: (i32, i32), owner: i32, hp: i32, kind: u32) -> i32 {
        for j in 0..self.field.units.len() {
            let v = &self.field.units[j];
            if self.field.alive[j] && v.flags & flag::MINE != 0 && v.cell == cell {
                return -1;
            }
        }
        let j = match self.field.alive.iter().position(|&a| !a) {
            Some(j) => j,
            None => {
                self.field.units.push(super::Group::default());
                self.field.alive.push(false);
                self.field.units.len() - 1
            }
        };
        let r = (self.libc)();
        let mut u = super::Group::default();
        u.rt.drop = super::TankRt::dropship_start(r);
        u.hp.push(hp);
        u.origin.push(-1);
        u.max_hp = hp;
        u.owner = owner;
        u.side = owner;
        u.rt.speed = 0.0;
        u.rt.max_speed = 2.5;
        u.rt.accel = 0.15;
        u.rt.decel = 0.3;
        u.rt.turn = (0.0, 0.0);
        u.move_state = (0, -1);
        u.flags |= flag::MINE;
        u.rt.planet = self.planet.index;
        u.index = j as i32;
        u.pods = 1;
        u.weapons |= kind;
        let hull2 = (((u.flags >> 20) & 1) * 2) as f32;
        for (bit, k) in [(2u32, 8usize), (0x100, 4)] {
            if u.weapons & bit != 0 {
                let w = self.rules.rt_weapons.get(k).copied().unwrap_or([0; 4]);
                let cand = (hull2 + w[3] as f32) as i32 as f32;
                if u.rt.range < cand {
                    u.rt.range = cand;
                }
                u.rt.reload[k] = w[2] as f32;
            }
        }
        self.field.units[j] = u;
        self.field.alive[j] = true;
        self.field.status_valid = false;
        self.set_pos(j as i32, cell.0, cell.1);
        let u = self.um(j as i32);
        u.has_gun = false;
        u.rt.fired = 0;
        if self.ctx.on_screen {
            self.load_mesh(j as i32);
        }
        j as i32
    }

    /// A mine (flag 0x20000 with weapon 0x4000/0x8000) blows when a counted enemy unit (not a mine) comes
    /// within √3500 map units.
    fn mine_contact(&mut self, i: i32) {
        let u = self.u(i);
        if u.flags & flag::MINE == 0 || u.weapons & 0xc000 == 0 {
            return;
        }
        let (owner, (x, y)) = (u.owner, u.pos);
        for j in 0..self.n() {
            let v = &self.field.units[j];
            if !self.field.alive[j] || v.flags & (flag::NOT_PLACED | flag::NOT_COUNTED) != 0 || v.flags2 & 4 != 0 {
                continue;
            }
            let dy = v.pos.1 - y;
            let dx = v.pos.0 - x;
            if v.flags & flag::MINE == 0 && v.owner != owner && dy * dy + dx * dx < 3500.0 {
                self.remove_group(i);
                self.remove_target(i);
                return;
            }
        }
    }
}

/// The turret angle kept in `[0, 2π)` (f64 compares against 2π).
fn wrap(a: &mut f32) {
    if *a as f64 >= TWO_PI {
        *a = (*a as f64 - TWO_PI) as f32;
    }
    if 0.0 > *a {
        *a = (*a as f64 + TWO_PI) as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_take_the_short_way_round() {
        let d = shortest_turn(0.1, 6.2);
        assert!(d < 0.0 && (d + 0.183_185_3).abs() < 1e-5, "{d}");
        assert!((shortest_turn(1.0, 2.0) - 1.0).abs() < 1e-6);
        // Angles beyond 2π fold back first (fmod by the f32 2π).
        assert!((shortest_turn(7.0, 1.0) - (1.0 - (7.0 - 6.283_185_5))).abs() < 1e-5);
    }

    #[test]
    fn direction_angles_cover_the_full_turn() {
        assert_eq!(angle_of(1.0, 0.0), 0.0);
        assert!((angle_of(0.0, 1.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((angle_of(-1.0, 0.0) - std::f32::consts::PI).abs() < 1e-6);
        assert!((angle_of(0.0, -1.0) - 3.0 * std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }
}
