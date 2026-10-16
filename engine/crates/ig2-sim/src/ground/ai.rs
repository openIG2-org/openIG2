//! The ground battle AI (`WF::MI`, exe 0x486d40, `grwarmi.cpp`). Spec: `re/rules/ground-ai.md`
//! (exe-normative; section numbers below refer to it).
//!
//! Once per 20 Hz tick, before [`War::refresh`], every computer empire with units and enemies on the
//! field picks a plan (stream 13), splits its vehicles into up to five groups (`CsoportbaRendez`), walks two
//! rally points towards the enemy's centre and lines the groups up around them until it attacks; the
//! planet's owner may instead guard its fortresses. Air strikes go to the cell with the most enemies and
//! the fewest own units nearby.

use super::{flag, weapon, War, NONE};
use crate::colony::flag as bflag;
use crate::rng::stream;
use std::f64::consts::PI;

/// The AI's state for one empire (`WF + 0x33c + 0x40·e`, §2.1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GroundAi {
    /// `+0x33c` 0 none yet, 1..=5 attacker plans, 6 guard the fortresses.
    pub strategy: i32,
    /// `+0x340` 1, or 3 when the planet's owner had no mobile unit at the start.
    pub mode: i32,
    /// `+0x344` 1 approach, 2 form up, 3 attack.
    pub phase: i32,
    /// `+0x348/+0x34c` rally point A, `+0x350/+0x354` rally point B (strategy 2's reserve).
    pub a: (f32, f32),
    pub b: (f32, f32),
    /// `+0x358/+0x35c` the enemy centre at the start (never read).
    pub start: (f32, f32),
    /// `+0x360` timer (−1 idle), `+0x364` the pincer's wait counter.
    pub timer: i32,
    pub counter: i32,
    /// `+0x368 + 4g` target per group (strategy 6).
    pub targets: [i32; 5],
}

/// A census bucket (§3): units, mobile (non-turret) units, summed then averaged cells.
#[derive(Debug, Clone, Copy, Default)]
struct Count {
    n: i32,
    mobile: i32,
    x: f32,
    y: f32,
}

impl Count {
    fn add(&mut self, turret: bool, cx: f32, cy: f32) {
        self.n += 1;
        if !turret {
            self.mobile += 1;
        }
        self.x += cx;
        self.y += cy;
    }
}

/// `((f64)deg · π) / 180` as the exe folds it.
fn rad(deg: i32) -> f64 {
    (deg as f64 * PI) / 180.0
}

/// `(f32)sqrt((f64)x)`.
fn sqrt32(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}

/// `dx² + dy²` in wrapping 32-bit ints (a never-set destination is uninitialised memory in the exe).
fn dist2(a: (i32, i32), b: (i32, i32)) -> i32 {
    let (dx, dy) = (a.0.wrapping_sub(b.0), a.1.wrapping_sub(b.1));
    dy.wrapping_mul(dy).wrapping_add(dx.wrapping_mul(dx))
}

/// `cvttss2si`: truncation, NaN and out of range → `i32::MIN`.
fn cvtt(x: f32) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x < -2147483648.0 { i32::MIN } else { x as i32 }
}

/// `VektorNormalizal` inlined (§5.4): `(dx, dy)` scaled so its larger component is `s` (product first).
fn advance(mut dx: f32, dy: f32, s: i32) -> (f32, f32) {
    if dx.abs() + dy.abs() < 0.1 {
        dx = 1.0;
    }
    let m = if dy.abs() > dx.abs() { dy.abs() } else { dx.abs() };
    let s = s as f32;
    ((dx * s) / m, (dy * s) / m)
}

/// The formation point (`CsoportFelallit2` + `PontMeghatarozas` inlined, §6.4): `dist` from `p` towards
/// `e`, rotated by `theta`, truncated to whole cells.
fn formation_point(p: (f32, f32), e: (f32, f32), theta: f64, dist: f32) -> (f32, f32) {
    let (mut dx, mut dy) = (e.0 - p.0, e.1 - p.1);
    let mut len = sqrt32(dy * dy + dx * dx);
    if (len as f64) < 0.05 {
        (dx, dy, len) = (1.0, 0.0, 1.0);
    }
    let k = dist / len;
    let (dx, dy) = (dx * k, dy * k);
    let (c, s) = (theta.cos() as f32, theta.sin() as f32);
    let x = (c * dx + p.0) - s * dy;
    let y = (s * dx + p.1) + c * dy;
    (x as i32 as f32, y as i32 as f32)
}

impl War<'_> {
    /// `WF::TankUsed(i)`: alive, placed, counted.
    fn used(&self, i: i32) -> bool {
        if !self.alive(i) {
            return false;
        }
        let u = self.u(i);
        u.flags & (flag::NOT_PLACED | flag::NOT_COUNTED) == 0 && u.flags2 & 4 == 0
    }
    /// A used unit that isn't a mine.
    fn on_field(&self, i: i32) -> bool {
        self.used(i) && self.u(i).flags & flag::MINE == 0
    }
    /// A used unit that is neither a mine nor a turret.
    fn mobile(&self, i: i32) -> bool {
        self.on_field(i) && self.u(i).flags & flag::TURRET == 0
    }
    fn in_group(&self, i: i32, e: i32, g: i32) -> bool {
        let u = self.u(i);
        u.owner == e && u.rt.ai_group == g
    }
    fn human(&self, e: i32) -> bool {
        self.ctx.player_ki.get(e.max(0) as usize).copied() == Some(1)
    }

    /// `FRISSIT_Grwar` when the shown planet's ground war is over: every AI forgets its plan (§1).
    pub fn ai_forget(&mut self) {
        for a in self.field.ai.iter_mut().skip(1) {
            a.strategy = 0;
        }
    }

    /// `WF::MI` (exe 0x486d40): one AI pass over the battlefield, before [`War::refresh`] (§3–§5).
    pub fn mi(&mut self, speed: i32) {
        let mut own = [[Count::default(); 5]; 19];
        let mut enemy = [[Count::default(); 5]; 19];
        for i in 0..self.n() as i32 {
            if !self.on_field(i) {
                continue;
            }
            let u = self.u(i);
            let (o, g, turret) = (u.owner as usize, u.rt.ai_group, u.flags & flag::TURRET != 0);
            let (cx, cy) = (u.cell.0 as f32, u.cell.1 as f32);
            let gs: &[usize] = if g > 0 { &[0, g as usize] } else { &[0] };
            for &k in gs {
                own[o][k].add(turret, cx, cy);
                for (e, row) in enemy.iter_mut().enumerate().skip(1) {
                    if e != o {
                        row[k].add(turret, cx, cy);
                    }
                }
            }
        }
        for c in own.iter_mut().chain(enemy.iter_mut()).flatten() {
            if c.n > 0 {
                c.x /= c.n as f32;
                c.y /= c.n as f32;
            }
        }
        for e in 1..19 {
            if self.human(e as i32) || own[e][0].n < 1 || enemy[e][0].n < 1 {
                self.field.ai[e].strategy = 0;
                continue;
            }
            self.mi_empire(e as i32, &own, &enemy, speed);
        }
    }

    fn mi_empire(&mut self, e: i32, own: &[[Count; 5]; 19], enemy: &[[Count; 5]; 19], speed: i32) {
        let ei = e as usize;
        let (mine, foe) = (own[ei][0], enemy[ei][0]);
        let ep = (foe.x, foe.y);
        let init = self.field.ai[ei].strategy == 0;
        if init {
            self.ai_init(e, own, enemy);
        }
        // §4.2: which groups are still driving.
        let mut moving = [false; 5];
        for i in 0..self.n() as i32 {
            if !self.used(i) {
                continue;
            }
            let u = self.u(i);
            if u.flags & (flag::TURRET | flag::MINE) != 0 || u.flags & 0x500 == 0 || u.owner != e {
                continue;
            }
            if cvtt(sqrt32(dist2(u.dest, u.cell) as f32)) > 0 && u.move_state.0 < 2 {
                moving[0] = true;
                if u.rt.ai_group > 0 {
                    moving[u.rt.ai_group as usize] = true;
                }
            }
        }
        let a = &mut self.field.ai[ei];
        if a.timer > 0 {
            a.timer = (a.timer - speed).max(0);
        }
        if !init {
            self.ai_decide(e, mine, foe, own[ei][3].n, &moving);
        }
        self.ai_act(e, ep);
        self.ai_air_strike(e);
    }

    /// INIT (§4.1): the plan, the rally points, the groups.
    fn ai_init(&mut self, e: i32, own: &[[Count; 5]; 19], enemy: &[[Count; 5]; 19]) {
        let ei = e as usize;
        let owner = self.planet.owner as usize;
        let mut mode = 1;
        let strategy = if owner == ei {
            if enemy[ei][0].mobile < own[ei][0].mobile || self.rng.range(stream::GROUND_AI, 1, 3) == 1 {
                self.rng.range(stream::GROUND_AI, 1, 5)
            } else {
                6
            }
        } else {
            let mut s = self.rng.range(stream::GROUND_AI, 1, 5);
            if s == 2 {
                s = 3;
            }
            if own[owner.min(18)][0].mobile < 1 {
                s = 3;
                mode = 3;
            }
            s
        };
        let c = own[ei][0];
        let f = enemy[ei][0];
        self.field.ai[ei] = GroundAi {
            strategy,
            mode,
            phase: 1,
            a: (c.x, c.y),
            b: (c.x, c.y),
            start: (f.x, f.y),
            timer: -1,
            counter: 0,
            targets: [NONE; 5],
        };
        self.groups_clear();
        self.groups_assign();
    }

    /// DECIDE (§5.1–§5.4).
    fn ai_decide(&mut self, e: i32, mine: Count, foe: Count, reserve: i32, moving: &[bool; 5]) {
        let ei = e as usize;
        let a = self.field.ai[ei];
        let (ex, ey) = (foe.x, foe.y);
        let d = {
            let (dy, dx) = (ey - a.a.1, ex - a.a.0);
            dy * dy + dx * dx
        };
        let (mut phase, mut timer, mut counter) = (a.phase, a.timer, a.counter);
        let (mut step, mut step_b) = (0, 0);
        let st = a.strategy;
        // The phase-2 rule of the attack plans 1 and 3.
        let phase2 = |phase: &mut i32, timer: i32| {
            if d > 256.0 {
                *phase = 1;
            }
            if a.mode == 1 && timer < 1 {
                *phase = 3;
            }
        };
        if st == 1 || st == 3 {
            if phase == 1 {
                if !moving[0] {
                    if d <= 225.0 {
                        phase = 2;
                        timer = 140;
                        if a.mode == 3 {
                            phase = 3;
                        } else {
                            phase2(&mut phase, timer);
                        }
                    } else {
                        step = if d > 1225.0 { 10 } else if d > 625.0 { 6 } else { 3 };
                    }
                }
            } else if phase == 2 {
                phase2(&mut phase, timer);
            }
            if d <= 121.0 {
                phase = 3;
            }
            if self.serult(e, 1, NONE) != 0 {
                phase = 3;
            }
            if self.serult(e, 2, NONE) != 0 {
                phase = 3;
            }
        }
        if st == 2 {
            let mut b_block = phase == 2 || phase == 3;
            if phase == 1 {
                if !moving[0] {
                    if d <= 484.0 {
                        phase = 2;
                    } else {
                        step = if d > 1600.0 { 10 } else if d > 900.0 { 6 } else { 3 };
                        step_b = step;
                        timer = 0;
                    }
                }
                if self.serult(e, 1, NONE) != 0 || self.serult(e, 2, NONE) != 0 {
                    phase = 3;
                }
                b_block = phase == 2 || phase == 3;
            }
            if b_block {
                if !moving[3] {
                    let b = self.field.ai[ei].b;
                    let db = {
                        let (dy, dx) = (ey - b.1, ex - b.0);
                        dy * dy + dx * dx
                    };
                    let h = self.serult(e, 3, 100);
                    if db > 144.0 && h == 0 {
                        step_b = 1;
                    }
                    if db < 81.0 || h == NONE {
                        step_b = -3;
                    }
                }
                if phase == 2 {
                    phase = if d <= 81.0 { 3 } else { 2 };
                    if reserve == 0 {
                        phase = 3;
                    }
                    if self.serult(e, 1, NONE) != 0 || self.serult(e, 2, NONE) != 0 {
                        phase = 3;
                    }
                }
            }
        }
        if st == 3 {
            if (phase == 1 || phase == 2) && foe.mobile * 2 < mine.mobile {
                phase = 3;
            } else if phase == 1 {
                if !moving[0] {
                    if d <= 225.0 {
                        phase = 2;
                        timer = 140;
                        phase2(&mut phase, timer);
                    } else {
                        step = if d > 1225.0 { 10 } else if d > 625.0 { 7 } else { 5 };
                    }
                }
            } else if phase == 2 {
                phase2(&mut phase, timer);
            }
            if d <= 121.0 {
                phase = 3;
            }
            if self.serult(e, 1, NONE) != 0 {
                phase = 3;
            }
            if self.serult(e, 2, NONE) != 0 {
                phase = 3;
            }
        }
        if st == 4 || st == 5 {
            let mut wait = phase == 2 && !moving[0];
            if phase == 1 && !moving[0] {
                if d <= 225.0 {
                    phase = 2;
                    counter = 0;
                    wait = true;
                } else {
                    step = if d > 1225.0 { 10 } else if d > 625.0 { 6 } else { 3 };
                }
            }
            if wait {
                counter += 1;
                if counter > 10 {
                    phase = 3;
                }
            }
            if d <= 64.0 {
                phase = 3;
            }
            for g in 1..=4 {
                if self.serult(e, g, NONE) != 0 {
                    phase = 3;
                }
            }
        }
        if st == 6 {
            for g in 1..=4 {
                let t = self.kilotte(e, g, NONE);
                let slot = &mut self.field.ai[ei].targets[g as usize];
                if t != NONE && *slot == NONE {
                    *slot = t;
                }
            }
        }
        let a = &mut self.field.ai[ei];
        if step != 0 {
            if timer == -1 {
                timer = 0;
            } else if timer == 0 {
                let (dx, dy) = advance(ex - a.a.0, ey - a.a.1, step);
                a.a = (dx + a.a.0, dy + a.a.1);
                timer = -1;
            }
        }
        if step_b != 0 {
            let (dx, dy) = advance(ex - a.b.0, ey - a.b.1, step_b);
            a.b = (dx + a.b.0, dy + a.b.1);
        }
        a.phase = phase;
        a.timer = timer;
        a.counter = counter;
    }

    /// ACT (§5.6): line up, attack, or guard the fortresses.
    fn ai_act(&mut self, e: i32, ep: (f32, f32)) {
        let a = self.field.ai[e as usize];
        let fa = |w: &mut Self, g: i32, deg: f64, dist: f32| {
            let (x, y) = formation_point(a.a, ep, deg, dist);
            w.line_up(e, g, x, y, ep);
        };
        let attack = |w: &mut Self, groups: i32| {
            for g in 0..=groups {
                w.tamad(e, g, NONE);
            }
        };
        let (q, h) = (rad(90), rad(-90));
        let (p80, m80, pi) = (rad(80), rad(-80), rad(180));
        let c = a.counter;
        let pincer = |w: &mut Self| {
            fa(w, 3, rad((16 - c) * 5), (c + 18) as f32);
            fa(w, 4, rad((c - 16) * 5), (c + 18) as f32);
        };
        match (a.strategy, a.phase) {
            (1, 1 | 2) => {
                fa(self, 1, q, 10.0);
                fa(self, 2, h, 10.0);
            }
            (2, 1 | 2) => {
                fa(self, 1, q, 10.0);
                fa(self, 2, h, 10.0);
                let (x, y) = formation_point(a.b, ep, pi, 0.0);
                self.line_up(e, 3, x, y, ep);
            }
            (3, 1 | 2) => {
                fa(self, 1, pi, 3.0);
                fa(self, 2, pi, 0.0);
            }
            (4, 1) => {
                fa(self, 1, q, 10.0);
                fa(self, 2, h, 10.0);
                fa(self, 3, p80, 18.0);
                fa(self, 4, m80, 18.0);
            }
            (4, 2) => {
                fa(self, 1, q, 10.0);
                fa(self, 2, h, 10.0);
                pincer(self);
            }
            (5, 1) => {
                fa(self, 1, pi, 3.0);
                fa(self, 2, pi, 0.0);
                fa(self, 3, p80, 18.0);
                fa(self, 4, m80, 18.0);
            }
            (5, 2) => {
                fa(self, 1, pi, 3.0);
                fa(self, 2, pi, 0.0);
                pincer(self);
            }
            (1 | 3, 3) => attack(self, 2),
            (2, 3) => attack(self, 3),
            (4 | 5, 3) => attack(self, 4),
            (6, _) => {
                for g in 1..=4 {
                    if !self.has_turret(e, g) {
                        self.tamad(e, g, NONE);
                        continue;
                    }
                    let mut t = self.field.ai[e as usize].targets[g as usize];
                    if t >= 1 && t != NONE && !self.used(t) {
                        t = NONE;
                    }
                    if t >= 1 && t != NONE && !self.can_reach(e, g, t, 2, true) {
                        t = NONE;
                    }
                    if t == NONE {
                        self.go_home(e, g);
                    } else {
                        self.tamad(e, g, t);
                    }
                    self.field.ai[e as usize].targets[g as usize] = t;
                }
                self.tamad(e, 0, NONE);
            }
            _ => {}
        }
    }

    /// The air strike (§5.7): the best cell by a 200 × 200 scan, then `ShotSpecWeapon`.
    fn ai_air_strike(&mut self, e: i32) {
        let (fighters, ships) = self.field.timers[(e - 1) as usize];
        let mut kind = -1;
        if fighters == 200 && self.air_ready(e, weapon::FIGHTER) {
            kind = 0;
        }
        if ships == 300 && self.air_ready(e, weapon::BIG_SHIP) {
            kind = 1;
        }
        if kind < 0 {
            return;
        }
        let r = [8.0f32, 5.0][kind as usize];
        let cells: Vec<(i32, i32, bool)> = (0..self.n() as i32)
            .filter(|&i| self.on_field(i))
            .map(|i| {
                let u = self.u(i);
                (u.cell.0, u.cell.1, u.owner == e)
            })
            .collect();
        let (mut best, mut bx, mut by) = (-1, 0, 0);
        for x in 0..200 {
            for y in 0..200 {
                let mut s = 0i32;
                for &(cx, cy, mine) in &cells {
                    let (dx, dy) = (cx - x, cy - y);
                    let d = sqrt32((dx * dx + dy * dy) as f32);
                    if d <= r {
                        s = if mine { (s as f32 - (r - d) * 3.0) as i32 } else { ((r - d) + s as f32) as i32 };
                    }
                }
                if s > best {
                    (best, bx, by) = (s, x, y);
                }
            }
        }
        if best > 0 {
            self.shot_spec_weapon(bx, by, kind, e);
        }
    }

    /// `WF::ShotSpecWeapon(x, y, kind, e)` (exe 0x49cb70): the charge goes, one air unit of the first
    /// carrier is spent; big ships drop 20 bombs (list B kind 1, flown by `missiles_step`), fighters start a
    /// wave (only its draws and flag: the fighter objects are not ported, `ground-combat.md` §11.11).
    fn shot_spec_weapon(&mut self, x: i32, y: i32, kind: i32, e: i32) {
        let t = &mut self.field.timers[(e - 1) as usize];
        if kind == 0 {
            t.0 = 0;
        } else {
            t.1 = 0;
        }
        let w = if kind == 0 { weapon::FIGHTER } else { weapon::BIG_SHIP };
        let Some(i) = (0..self.n()).find(|&i| {
            let u = &self.field.units[i];
            self.field.alive[i] && u.weapons & w != 0 && u.owner == e && u.air > 0
        }) else {
            return;
        };
        self.field.units[i].air -= 1;
        let gx = x + self.draw(-4, 4);
        let gy = y + self.draw(-4, 4);
        if kind == 1 {
            // The cell on the battlefield's map (f32 with an f64 tail; x mirrored) and its height.
            let (w, h) = self.rules.map_size;
            let fx = (gx as f32 * 0.5 / 99.0 * 100.0 * w) / 99.0;
            let fy = (gy as f32 * 0.5 / 99.0 * 100.0 * h) / 99.0;
            let ax = (-(fx as f64 - w as f64 * 0.5)) as f32;
            let ay = (fy as f64 - h as f64 * 0.5) as f32;
            let (hx, hy) = ((gx as f64 * 0.25) as f32, (gy as f64 * 0.25) as f32);
            let z = if hx > 200.0 || hy > 200.0 { 0.0 } else { self.height(hx, hy) };
            let (zi, top) = (cvtt(z), cvtt(z + 300.0));
            // 20 bombs from high above, spread around the cell, falling towards a spread point.
            for _ in 0..20 {
                let dy = cvtt((self.draw(0, 399) as f32 + ay) - 200.0);
                let dx = cvtt((self.draw(0, 399) as f32 + ax) - 200.0);
                let sy = (self.draw(0, 1999) as f32 + ay) - 1000.0;
                let sx = (self.draw(0, 1999) as f32 + ax) - 1000.0;
                self.add_bomb([z - 3000.0, sx, sy], [top, dx, dy], 300, 10, zi);
            }
        } else if !self.field.lists.fighters_on {
            // `WF::AddFighters` (exe 0x45b8d0): five fighters, one draw each, unless a wave is still up.
            self.field.lists.fighters_on = true;
            for _ in 0..5 {
                self.draw(0, 4);
            }
        }
        self.events.push(super::GroundEvent::AirStrike { empire: e, cell: (x, y), big_ships: kind == 1 });
    }

    /// The bomb of a big-ship strike (exe 0x457ad0): a list-B object of kind 1 from `p` towards `dest`
    /// (height first), speed `((f64)rndlog(2, 0, 14)/10 + 0.5)·3` × 20; on screen a libc `rand()` picks its
    /// texture. Its `+0x3c` is uninitialised heap in the exe (the port: 0).
    fn add_bomb(&mut self, p: [f32; 3], dest: [i32; 3], life: i32, dmg: i32, target: i32) {
        let k = super::battle::slot(&mut self.field.lists.missiles);
        let dest = dest.map(|v| v as f32);
        let mut v = [dest[0] - p[0], dest[1] - p[1], dest[2] - p[2]];
        super::battle::normalise(&mut v);
        let s = ((self.draw(0, 14) as f64 / 10.0 + 0.5) * 3.0) as f32 * 20.0;
        let vel = v.map(|c| s * c);
        self.field.lists.missiles[k] =
            Some(super::Missile { vel, pos: p, dest, life, dmg, target, kind: 1, fade: 0.0, by: NONE });
        if self.ctx.on_screen {
            (self.libc)();
        }
    }

    // -------------------------------------------------------------------------------------------- §6

    /// `CsoportbaRendezestKitorol` (exe 0x4697a0, §6.1).
    fn groups_clear(&mut self) {
        for i in 0..self.n() as i32 {
            if self.used(i) {
                let u = self.um(i);
                u.rt.ai_group = 0;
                u.last_hit = Some((0, NONE));
            }
        }
    }

    /// `CsoportbaRendez` (exe 0x469280, §6.2): every AI empire's units into groups by its plan.
    fn groups_assign(&mut self) {
        let mut forts = [0i32; 5];
        let (mut nf, mut best) = (0usize, 0usize);
        for slot in 1..self.planet.buildings.len() {
            let b = self.planet.buildings[slot];
            if b.kind != 0 && b.work_left == 0 && (0x20..0x24).contains(&b.kind) && nf < 4 {
                nf += 1;
                forts[nf] = slot as i32;
                if best == 0 || self.planet.buildings[forts[best] as usize].kind < b.kind {
                    best = nf;
                }
            }
        }
        let coin = self.rng.range(stream::GROUND_AI, 0, 1) != 0 && best >= 1;
        let owner = self.planet.owner as usize;
        if nf == 0 && owner < 19 && self.field.ai[owner].strategy == 6 {
            self.field.ai[owner].strategy = 1;
        }
        let mut kinds = [[0i32; 8]; 19];
        let mut size = [[0i32; 5]; 19];
        let smallest = |s: &[i32; 5], hi: usize| (2..=hi).fold(1, |g, c| if s[c] < s[g] { c } else { g });
        for i in 0..self.n() as i32 {
            if !self.used(i) {
                continue;
            }
            let u = self.u(i);
            let o = u.owner as usize;
            if u.flags & (flag::TURRET | flag::MINE | flag::PARTNER) != 0 || u.rt.ai_group != 0 || self.human(u.owner) {
                continue;
            }
            let w = u.weapons;
            let k = if w & 0x10 != 0 {
                6
            } else if w & 0x2001 != 0 {
                1
            } else if w & 0xf02 != 0 {
                2
            } else if w & 4 != 0 {
                3
            } else if w & 0x1000 != 0 {
                4
            } else if w & 0xc0 != 0 {
                5
            } else {
                7
            };
            kinds[o][k] += 1;
            kinds[o][0] += 1;
            let mut home = None;
            let g = match self.field.ai[o].strategy {
                1 | 3 if k == 1 || k == 2 => k,
                2 if kinds[o][0] == 1 => 3,
                2 => if size[o][2] <= size[o][1] { 2 } else { 1 },
                4 | 5 => smallest(&size[o], 4),
                6 => {
                    let g = if coin { best } else { smallest(&size[o], nf.max(1)) };
                    home = Some(-forts[g]);
                    g
                }
                _ => 0,
            };
            let u = self.um(i);
            u.rt.ai_group = g as i32;
            if let Some(h) = home {
                u.rt.ai_home = h;
            }
            size[o][g] += 1;
        }
        for i in 0..self.n() as i32 {
            if !self.used(i) {
                continue;
            }
            let u = self.u(i);
            let o = u.owner as usize;
            if u.flags & flag::TURRET == 0 || u.rt.ai_group != 0 || self.human(u.owner) || self.field.ai[o].strategy != 6 {
                continue;
            }
            let g = (1..=4).rev().find(|&k| forts[k] == u.building).unwrap_or(0);
            let u = self.um(i);
            u.rt.ai_group = g as i32;
            u.rt.ai_home = -forts[g];
            size[o][g] += 1;
        }
    }

    /// `CsoportFelallit(e, g, x, y, fx, fy)` (exe 0x468b30, §6.3): line group `g` up across the direction
    /// from `(x, y)` to `f`, two cells apart, alternating sides.
    fn line_up(&mut self, e: i32, g: i32, x: f32, y: f32, f: (f32, f32)) {
        let mut dy = f.1 - y;
        let nx = -(f.0 - x);
        if dy.abs() + nx.abs() < 0.1 {
            dy = 1.0;
        }
        let m = if nx.abs() > dy.abs() { nx.abs() } else { dy.abs() };
        let (sx, sy) = ((dy * 2.0) / m, (nx * 2.0) / m);
        let mut n = 0;
        for i in 0..self.n() as i32 {
            if !self.mobile(i) || !self.in_group(i, e, g) {
                continue;
            }
            n += 1;
            let k = (n / 2) as f32;
            let (px, py) = if n % 2 == 0 { (k * sx + x, k * sy + y) } else { (x - k * sx, y - k * sy) };
            let (px, py) = (px as i32, py as i32);
            let u = self.u(i);
            if (u.cell.1 - py).abs() + (u.cell.0 - px).abs() < 5 && u.move_state.0 > 9 {
                self.stop_orders(i);
            } else {
                self.move_g(i, px, py);
            }
        }
    }

    /// `CsoportTamad(e, g, t)` (exe 0x468ff0, §6.5).
    fn tamad(&mut self, e: i32, g: i32, t: i32) {
        for i in 0..self.n() as i32 {
            if !self.mobile(i) || !self.in_group(i, e, g) {
                continue;
            }
            if t == NONE {
                let pick = self.mi_target(i);
                self.set_target(i, pick);
            } else if self.used(t) && self.u(t).owner != e {
                self.set_target(i, t);
            }
        }
    }

    /// `WF_Tank::GetMITarget` (exe 0x47b110, §6.6).
    fn mi_target(&self, i: i32) -> i32 {
        let u = self.u(i);
        let n = u.size();
        let range = u.rt.range;
        let (mut best, mut pick) = (1e10f32, NONE);
        for j in 0..self.n() as i32 {
            if !self.used(j) {
                continue;
            }
            let t = self.u(j);
            if t.owner == u.owner {
                continue;
            }
            let seen = self.field.radar.get(u.owner.max(0) as usize).copied().unwrap_or(0) != 0;
            if t.flags & flag::MINE != 0 && t.rt.fired == 0 && !seen {
                continue;
            }
            if n < 1 {
                continue;
            }
            // Quirk: the target's hp indexed with the chooser's size (an over-read past a smaller target).
            let h = t.hp.get((n - 1) as usize).copied().unwrap_or(0);
            let dx = (u.cell.0 - t.cell.0) as f32;
            let dy = (u.cell.1 - t.cell.1) as f32;
            let r = sqrt32(dx * dx + dy * dy);
            let r = if r <= range { range } else { r };
            let mut v = ((h / n) / 500) as f32 + r;
            if u.target == j {
                if u.flags & !flag::HAS_TARGET != 0 {
                    v *= 0.5;
                }
                if u.move_state.0 == 499 {
                    v /= 1000.0;
                }
            }
            if t.flags & flag::TURRET != 0 {
                let f = self.building_flags(t.building);
                if f & bflag::WORKING == 0 || f & bflag::SWITCHED_OFF != 0 {
                    v *= 1000.0;
                }
            }
            if r <= range {
                v /= 3.0;
            }
            if v < best {
                best = v;
                pick = j;
            }
        }
        pick
    }

    /// The group's hit markers (`+0x25c`, attacker `+0x260`) walked and decayed by `k` (§6.7); `who`
    /// picks the attacker result (`CsoportKilotte`) instead of the marker (`CsoportSerultE`).
    fn markers(&mut self, e: i32, g: i32, k: i32, who: bool) -> i32 {
        let mut res = if who { NONE } else { 0 };
        for i in 0..self.n() as i32 {
            if !self.on_field(i) || !self.in_group(i, e, g) {
                continue;
            }
            let (m, by) = self.u(i).last_hit.unwrap_or((0, NONE));
            if who {
                if m == NONE && by != NONE && self.used(by) && self.u(by).owner != e {
                    res = by;
                }
            } else if m > 0 && res <= m {
                res = m;
            }
            self.um(i).last_hit = Some(((m - k).max(0), NONE));
        }
        res
    }
    fn serult(&mut self, e: i32, g: i32, k: i32) -> i32 {
        self.markers(e, g, k, false)
    }
    fn kilotte(&mut self, e: i32, g: i32, k: i32) -> i32 {
        self.markers(e, g, k, true)
    }

    /// `CsoportVanBenneTurret(e, g)` (exe 0x4691e0, §6.8).
    fn has_turret(&self, e: i32, g: i32) -> bool {
        (0..self.n() as i32).any(|i| self.used(i) && self.in_group(i, e, g) && self.u(i).flags & flag::TURRET != 0)
    }

    /// `CsoportTudjaELoniAzEllenseg(e, g, t, extra, turrets)` (exe 0x4690b0, §6.8): enemy `t` can reach
    /// a unit of the group.
    fn can_reach(&self, e: i32, g: i32, t: i32, extra: i32, turrets: bool) -> bool {
        if t == NONE {
            return false;
        }
        let tu = self.u(t);
        let reach = (tu.rt.range as i32 + extra) as f32;
        (0..self.n() as i32).any(|i| {
            if !self.on_field(i) || !self.in_group(i, e, g) || (turrets && self.u(i).flags & flag::TURRET == 0) {
                return false;
            }
            let u = self.u(i);
            let dx = (tu.cell.0 - u.cell.0) as f32;
            let dy = (tu.cell.1 - u.cell.1) as f32;
            sqrt32(dx * dx + dy * dy) <= reach
        })
    }

    /// `CsoportMozgat2(e, g)` (exe 0x468e60, §6.8): back to the fortress (or unit) it guards.
    fn go_home(&mut self, e: i32, g: i32) {
        for i in 0..self.n() as i32 {
            if !self.mobile(i) || !self.in_group(i, e, g) {
                continue;
            }
            let home = self.u(i).rt.ai_home;
            let (x, y) = if home < 1 {
                let b = self.planet.buildings.get((-home) as usize).copied().unwrap_or_default();
                (b.x as i32, b.y as i32)
            } else {
                self.u(home).cell
            };
            self.move_g(i, x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folded_angles_match_the_exe_constants() {
        // The f64 constants the exe folded (π/2, −π/2, 80°, π), as stored in its code.
        assert_eq!(rad(90).to_bits(), 0x3ff921fb54442d18);
        assert_eq!(rad(-90).to_bits(), 0xbff921fb54442d18);
        assert_eq!(rad(80).to_bits(), 0x3ff657184ae74487);
        assert_eq!(rad(180).to_bits(), 0x400921fb54442d18);
    }

    #[test]
    fn garbage_destinations_wrap_like_the_exe() {
        // An uninitialised destination: the squares wrap to a negative sum, sqrt → NaN → INT_MIN: not moving.
        let d = dist2((1781186419, 930819277), (124, 120));
        assert!(d < 0);
        assert_eq!(cvtt(sqrt32(d as f32)), i32::MIN);
        assert_eq!(cvtt(sqrt32(dist2((13, 10), (10, 10)) as f32)), 3);
    }

    #[test]
    fn advance_scales_the_larger_component() {
        assert_eq!(advance(30.0, -10.0, 6), (6.0, -2.0));
        assert_eq!(advance(0.0, 0.0, 3), (3.0, 0.0));
        assert_eq!(advance(-5.0, 20.0, -3), (0.75, -3.0));
    }

    #[test]
    fn formation_points_rotate_towards_the_enemy() {
        // Enemy straight east of A: +90° puts the point 10 north (y + 10), −90° south, 180° behind.
        assert_eq!(formation_point((50.0, 50.0), (100.0, 50.0), rad(90), 10.0), (50.0, 60.0));
        assert_eq!(formation_point((50.0, 50.0), (100.0, 50.0), rad(-90), 10.0), (50.0, 40.0));
        assert_eq!(formation_point((50.0, 50.0), (100.0, 50.0), rad(180), 3.0), (47.0, 50.0));
        // On top of the enemy: the fallback direction (1, 0).
        assert_eq!(formation_point((50.0, 50.0), (50.0, 50.0), 0.0, 4.0), (54.0, 50.0));
    }
}
