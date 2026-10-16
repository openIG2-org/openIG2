//! The ground units' path search `grUtkeres` (exe 0x614940, with the node relaxation 0x6142a0, the heap
//! pop 0x614540 and the line-of-sight test 0x614830; spec `re/rules/ground-combat.md` §11.9).
//!
//! The cost grid (exe 0xFAC940, 200 × 200 u16, index `x·200 + y`) starts as a copy of the planet's
//! passability map for the unit's hull (`PlanetAtjarasMap`, free 65000, impassable 65001); other units
//! (65002, 3 × 3) and buildings (65003) are stamped in, free cells around the goal become 0 (goals), and
//! a best-first search with a 1000-entry heap (priority `g + h`, `h = 7·max + 2·min` of the offset to the
//! goal) runs from the unit. The result is the furthest visible cell of the walk back from the best cell.

use super::{flag, War};

const FREE: u16 = 65000;
const WALL: u16 = 65001;
const UNIT: u16 = 65002;
const BUILDING: u16 = 65003;

/// What `grUtkeres` hands back (exe 0xFAC920.. 0xFAC930).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathOut {
    /// `0xFAC920/0xFAC924`: the cell to drive to.
    pub next: (i32, i32),
    /// `0xFAC930`: the goal is reached (within the radius).
    pub arrived: bool,
}

struct Search {
    grid: Vec<u16>,
    goal: (i32, i32),
    /// Heap entries `(x, y, priority)`, index 1-based (0 unused).
    heap: Vec<(u16, u16, u16)>,
    count: usize,
    best: i32,
    result: (i32, i32),
    found: bool,
    widened: bool,
    limit_f: i32,
    limit_h: i32,
    bounded: bool,
}

fn h7(a: (i32, i32), b: (i32, i32)) -> i32 {
    let dx = (a.0 - b.0).abs();
    let dy = (a.1 - b.1).abs();
    dx.max(dy) * 7 + dx.min(dy) * 2
}

impl Search {
    fn at(&self, x: i32, y: i32) -> u16 {
        self.grid.get((x * 200 + y).max(0) as usize).copied().unwrap_or(WALL)
    }
    fn set(&mut self, x: i32, y: i32, v: u16) {
        if let Some(c) = self.grid.get_mut((x * 200 + y).max(0) as usize) {
            *c = v;
        }
    }

    /// `0x6142a0`: relax cell `(x, y)` reached at cost `g + step`.
    fn relax(&mut self, x: i32, y: i32, g: i32, step: i32) {
        if x == 0 || y == 0 || x > 0xc6 || y > 0xc6 {
            return;
        }
        let cell = self.at(x, y);
        let new = g + step;
        if cell == 0 {
            let score = h7((x, y), self.goal) * 5 + (new & 0xffff) * 2;
            if score < self.best || (!self.found && !self.widened) {
                self.result = (x, y);
                self.best = score;
            }
            self.found = true;
            return;
        }
        if self.found || cell <= new as u16 || cell > FREE {
            return;
        }
        let h = h7((x, y), self.goal);
        if self.bounded && ((h + new) & 0xffff) > self.limit_f {
            return;
        }
        if self.bounded && h > self.limit_h {
            return;
        }
        self.limit_h = (h * 2 + 0x23).min(self.limit_h).max(0x8c);
        let score = h * 5 + (new & 0xffff) * 2;
        if score < self.best {
            self.result = (x, y);
            self.best = score;
        }
        let prio = (h + new) as u16;
        let mut at = None;
        if cell < FREE {
            at = (1..=self.count).find(|&i| self.heap[i].0 == x as u16 && self.heap[i].1 == y as u16);
        }
        let i = match at {
            // Already queued: the priority is overwritten in place, without re-sifting [original quirk].
            Some(i) => i,
            None => {
                if self.count > 999 {
                    self.count -= 1;
                }
                self.count += 1;
                let mut i = self.count;
                if self.heap.len() <= i {
                    self.heap.resize(i + 1, (0, 0, 0));
                }
                while i > 1 && prio < self.heap[i / 2].2 {
                    self.heap[i] = self.heap[i / 2];
                    i /= 2;
                }
                i
            }
        };
        self.heap[i] = (x as u16, y as u16, prio);
        self.set(x, y, new as u16);
    }

    /// `0x614540`: take the best entry and relax its eight neighbours.
    fn pop(&mut self) {
        if self.count < 1 {
            self.found = true;
            return;
        }
        if self.found {
            return;
        }
        let (x, y, _) = self.heap[1];
        let (x, y) = (x as i32, y as i32);
        let g = self.at(x, y) as i32;
        self.heap[1] = self.heap[self.count];
        self.count -= 1;
        let mut i = 1;
        loop {
            let (l, r) = (2 * i, 2 * i + 1);
            let mut m = i;
            if l <= self.count && self.heap[l].2 < self.heap[i].2 {
                m = l;
            }
            if r <= self.count && self.heap[r].2 < self.heap[m].2 {
                m = r;
            }
            if m == i {
                break;
            }
            self.heap.swap(i, m);
            i = m;
        }
        for (dx, dy, c) in [(-1, -1, 6), (0, -1, 5), (1, -1, 6), (-1, 0, 5), (1, 0, 5), (-1, 1, 6), (0, 1, 5), (1, 1, 6)] {
            self.relax(x + dx, y + dy, g, c);
        }
    }

    /// `0x614830`: no obstacle (65001..=65003) on the sampled line from `a` to `b`.
    fn visible(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let n = (a.0 - b.0).abs().max((a.1 - b.1).abs());
        let f = 1.0f32 / (n as f32 + 0.01);
        let mut t = f;
        while t <= 1.001 {
            let x = ((b.0 - a.0) as f32 * t + a.0 as f32 + 0.5) as i32;
            let y = ((b.1 - a.1) as f32 * t + a.1 as f32 + 0.5) as i32;
            let v = self.at(x, y);
            if FREE < v && v < 0xfdec {
                return false;
            }
            t += f;
        }
        true
    }

    fn block3(&mut self, (x, y): (i32, i32)) {
        for dx in -1..=1 {
            for dy in -1..=1 {
                self.set(x + dx, y + dy, UNIT);
            }
        }
    }
}

impl War<'_> {
    /// The cell a unit makes for, as `MoveG`/attack orders set it (for the "same goal" test).
    fn order_goal(&self, j: usize) -> (i32, i32) {
        let u = &self.field.units[j];
        let mut g = (-1, -1);
        if u.flags & 0x400 != 0 {
            g = u.dest;
        }
        if u.flags & 0x800 != 0 && u.flags & flag::HAS_TARGET != 0 {
            g = self.target_cell(u.target);
        }
        g
    }

    /// `grUtkeres(x, y, gx, gy, radius, unit, p)` (exe 0x614940, spec §11.9).
    pub(super) fn find_path(&mut self, start: (i32, i32), goal: (i32, i32), radius: i32, i: i32, _planet: i32) -> PathOut {
        let radius = radius.max(1);
        let me = self.u(i).clone();
        let kind = if me.flags & flag::HULL2 != 0 { 3 } else { me.hull + 1 };
        {
            let u = self.um(i);
            u.move_state.0 += 1;
            if u.move_state.0 == 10 {
                u.move_state.0 = 0x1d6;
            }
        }
        let counter = self.u(i).move_state.0;
        if counter > 10 && counter % 20 != 0 && counter != 500 && counter != 501 {
            return PathOut { next: start, arrived: false };
        }
        let d = (goal.0 - start.0, goal.1 - start.1);
        if d.1 * d.1 + d.0 * d.0 < radius * radius {
            return PathOut { next: start, arrived: true };
        }
        if start == goal {
            return PathOut { next: start, arrived: true };
        }
        let h = h7(start, goal);
        let base = self.field.passability.get(kind.max(0) as usize).cloned().unwrap_or_default();
        let mut s = Search {
            grid: if base.len() == 40000 { base } else { vec![FREE; 40000] },
            goal,
            heap: vec![(0, 0, 0), (start.0 as u16, start.1 as u16, 50000)],
            count: 1,
            best: h * 5 + 2,
            result: start,
            found: false,
            widened: false,
            limit_f: (h * 3 + 0x46).max(0x118),
            limit_h: (h * 2 + 0x23).max(0x8c),
            bounded: counter != 500,
        };
        let my_goal = {
            let mut g = (-1, -1);
            if me.flags & 0x400 != 0 {
                g = me.dest;
            }
            if me.flags & flag::HAS_TARGET != 0 {
                g = self.target_cell(me.target);
            }
            g
        };
        // Other units block 3 × 3 around their cell and their next cell.
        for j in 0..self.n() {
            if j as i32 == i || !self.field.alive[j] {
                continue;
            }
            let v = &self.field.units[j];
            if v.flags & (flag::NOT_PLACED | flag::NOT_COUNTED) != 0 || v.flags2 & 4 != 0 || v.flags & 0x4020000 != 0 {
                continue;
            }
            if self.order_goal(j) == my_goal && v.move_state.0 <= 5 {
                continue;
            }
            for c in [v.cell, v.last_cell] {
                if c != start && (1..=0xc6).contains(&c.0) && (1..=0xc6).contains(&c.1) {
                    s.block3(c);
                }
            }
        }
        for slot in 1..self.planet.buildings.len() {
            let b = self.planet.buildings[slot];
            if b.kind == 0 {
                continue;
            }
            let (w, hh) = self.map.map_or((0, 0), |m| m.footprints.get(b.kind as usize).copied().unwrap_or((0, 0)));
            let (bx, by) = (b.x as i32, b.y as i32);
            for x in bx - w / 2..bx + w / 2 + w % 2 {
                for y in by - hh / 2 - hh % 2..by + hh / 2 {
                    s.set(x, y, BUILDING);
                }
            }
        }
        if s.at(goal.0, goal.1) == FREE {
            s.set(goal.0, goal.1, 0);
        }
        s.set(start.0, start.1, 1);
        let mut r = if me.move_state.1 >= 0 { me.move_state.1 } else { radius };
        s.widened = me.move_state.1 >= 0 && radius < me.move_state.1;
        let mut first_crowded = -1;
        let mut crowded = false;
        loop {
            let mut goals = (s.at(goal.0, goal.1) == 0) as i32;
            let mut near = false;
            let x0 = (goal.0 - r).max(1);
            let x1 = (goal.0 + r).min(0xc6);
            let y0 = (goal.1 - r).max(1);
            let y1 = (goal.1 + r).min(0xc6);
            for x in x0..x1 {
                for y in y0..y1 {
                    let v = s.at(x, y);
                    let (ex, ey) = (goal.0 - x, goal.1 - y);
                    if v == WALL || ex * ex + ey * ey >= r * r {
                        continue;
                    }
                    if v == 1 {
                        let arrived = r == radius || !crowded || counter > 500;
                        if first_crowded < 0 {
                            first_crowded = r;
                        }
                        self.um(i).move_state.1 = first_crowded;
                        return PathOut { next: start, arrived };
                    }
                    if v == FREE {
                        goals += 1;
                        s.set(x, y, 0);
                    } else if v == UNIT {
                        near = true;
                        if first_crowded < 0 {
                            first_crowded = r;
                        }
                    }
                }
            }
            if goals == 0 {
                r += 1;
                s.widened = true;
            }
            crowded |= near;
            if goals != 0 {
                break;
            }
        }
        if first_crowded < 0 {
            first_crowded = r;
        }
        self.um(i).move_state.1 = first_crowded;
        s.found = false;
        while !s.found {
            s.pop();
        }
        let mut arrived = false;
        let mut stuck = false;
        let mut res = s.result;
        let mut backtrack = true;
        if res == start {
            if crowded {
                if counter > 500 {
                    self.um(i).move_state.0 = 0xb;
                }
                if counter < 0x2bd {
                    backtrack = true;
                } else {
                    arrived = counter > 500;
                }
            } else {
                arrived = counter > 500;
            }
        } else if self.u(i).move_state.0 == 500 {
            self.um(i).move_state.0 = 499;
        }
        // The walk back down the cost gradient, then the furthest visible point of it.
        let mut walk = Vec::new();
        let mut moves = 0;
        if backtrack {
            let (mut x, mut y) = res;
            loop {
                walk.push((x, y));
                let mut best = s.at(x, y) as u32;
                if best == 0 {
                    best = 64000;
                }
                let (mut nx, mut ny) = (x, y);
                for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                    let v = s.at(x + dx, y + dy) as u32;
                    if v < best && v != 0 {
                        best = v;
                        nx = x + dx;
                        ny = y + dy;
                    }
                }
                if (nx, ny) != (x, y) {
                    moves += 1;
                }
                if best == 1 || (nx, ny) == (x, y) {
                    break;
                }
                x = nx;
                y = ny;
            }
            stuck = moves == 0;
            let mut seen = None;
            for &p in walk.iter().rev() {
                if s.visible(start, p) {
                    seen = Some(p);
                }
            }
            if let Some(p) = seen.filter(|p| p.0 != 0) {
                res = p;
            }
        }
        // One cell along the way; other units' next steps block it.
        let n = (start.1 - res.1).abs().max((start.0 - res.0).abs());
        let step = (
            ((res.0 - start.0) as f32 / (n as f32 + 0.01) + start.0 as f32 + 0.5) as i32,
            ((res.1 - start.1) as f32 / (n as f32 + 0.01) + start.1 as f32 + 0.5) as i32,
        );
        for j in 0..self.n() {
            if j as i32 == i || !self.field.alive[j] {
                continue;
            }
            let v = &self.field.units[j];
            if v.flags & (flag::NOT_PLACED | flag::NOT_COUNTED) != 0 || v.flags2 & 4 != 0 || v.flags & 0x4020000 != 0 {
                continue;
            }
            let m = (v.cell.1 - v.last_cell.1).abs().max((v.cell.0 - v.last_cell.0).abs());
            let p = (
                ((v.last_cell.0 - v.cell.0) as f32 / (m as f32 + 0.01) + v.cell.0 as f32 + 0.5) as i32,
                ((v.last_cell.1 - v.cell.1) as f32 / (m as f32 + 0.01) + v.cell.1 as f32 + 0.5) as i32,
            );
            if (p != start || (j as i32) < i) && (1..=0xc6).contains(&p.0) && (1..=0xc6).contains(&p.1) {
                if (p.0 - start.0).abs() < 2 && (p.1 - start.1).abs() < 2 && i <= j as i32 {
                    s.set(p.0, p.1, UNIT);
                } else {
                    s.block3(p);
                }
            }
        }
        let blocked = |v: u16| FREE < v && v < 0xfdec;
        let mut done = false;
        if step != start && blocked(s.at(step.0, step.1)) {
            let mut best = 1e10f32;
            let mut pick = start;
            for k in 0..9 {
                if k == 4 {
                    continue;
                }
                let c = (start.0 - 1 + k / 3, start.1 - 1 + k % 3);
                let d2 = (res.1 - c.1) * (res.1 - c.1) + (res.0 - c.0) * (res.0 - c.0);
                let d = (d2 as f64).sqrt() as f32;
                if c != step && d < best {
                    best = d;
                    pick = c;
                }
            }
            res = pick;
            if blocked(s.at(pick.0, pick.1)) {
                res = start;
                stuck = true;
                done = true;
            }
        }
        if !done && res == start {
            stuck = true;
        }
        if counter != 500 && !stuck {
            self.um(i).move_state.0 = 0;
        }
        PathOut { next: res, arrived }
    }
}
