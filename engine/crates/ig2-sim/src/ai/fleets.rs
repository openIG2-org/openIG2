//! `FRISSIT_MI_FLOTTAK(e)` (exe 0x50b1c0): day counters, vehicles for the main fleet, war, the attack target and
//! every fleet's orders; with `CheckVeszelyesHely`, `MenekulesHaKell`, `BezartSzog`, `MenekulesSuly`,
//! `SearchFlottahozKozeliBolygo` and `MI_KitAkarTamadni` (`re/rules/ai-empire.md` §11, §12). **The exe is
//! normative**: the precision of every float expression follows its asm (noted where it differs from the obvious).

use super::colonise::capturable;
use super::{cvtt, AiHooks, AiState, World, EMPIRES, GRID};
use crate::diplomacy::{field, level};
use crate::fleet::{flag, order};
use crate::spacewar::NO_DESIGN;

/// `CheckVeszelyesHely`'s reach per speed class (f32).
const DANGER_REACH: [f32; 11] = [1.8, 1.7, 1.5, 1.3, 1.2, 1.0, 0.7, 0.6, 0.5, 0.4, 0.3];
/// The attack range per speed class against fleets without (`PLAIN`) and with ground forces (`GROUND`).
const PLAIN_RANGE: [f32; 11] = [10.0, 10.0, 10.0, 10.0, 10.0, 20.0, 30.0, 35.0, 40.0, 45.0, 50.0];
const GROUND_RANGE: [f32; 11] = [30.0, 30.0, 30.0, 30.0, 30.0, 100.0, 130.0, 160.0, 200.0, 1000.0, 1000.0];
/// The flee directions' step (a double: π/8) and end (2π).
const FLEE_STEP: f64 = std::f64::consts::FRAC_PI_8;
const FULL_TURN: f64 = std::f64::consts::TAU;
/// Ground hulls are design `+0x44` 0x13.. (`kind = hull − 0x12 > 0`).
const GROUND_HULL_BASE: i32 = 0x12;

/// The speed class `clamp((int)((f64)((a − b)·50 + 5) + 0.5), 0, 10)` (f32 product and sum).
fn speed_class(own: f32, other: f32) -> usize {
    cvtt(((own - other) * 50.0 + 5.0) as f64 + 0.5).clamp(0, 10) as usize
}

/// `(f32)sqrt((f64)x)`.
fn root(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}

/// The scouting grid cell of a coordinate: `(int)((c + 300) / 30 + 1)` (f32).
fn cell(c: f32) -> i32 {
    cvtt(((c + 300.0) / 30.0 + 1.0) as f64)
}

impl World<'_> {
    fn level(&self, a: usize, b: usize) -> u8 {
        self.spy.relations.get(a).and_then(|r| r.get(b)).map_or(0, |r| r.level)
    }

    /// `MI_HannyalHarcol(e)`: wars of `e` with empires 1..=`ProgramPlayers2`.
    fn wars(&self, e: usize) -> usize {
        (1..=self.players2()).filter(|&c| c != e && self.level(e, c) == level::WAR).count()
    }

    /// `SearchFlottahozKozeliBolygo(f, limit, own, unprotected)` (§12.4).
    pub(crate) fn nearest_planet(&self, f: usize, limit: f32, own: bool, unprotected: bool) -> usize {
        let Some(r) = self.rec(f as i32) else { return 0 };
        let pl = &self.spy.colony.planets;
        let (mut best, mut pick) = (limit, 0);
        if r.owner != 0 {
            for (p, x) in pl.iter().enumerate().skip(1) {
                if own && x.owner != r.owner {
                    continue;
                }
                let d = (r.y - x.y).abs() + (r.x - x.x).abs();
                if d < best {
                    best = d;
                    pick = p;
                }
            }
        }
        if unprotected && pl.get(pick).is_some_and(|x| x.protected) {
            pick = 0;
        }
        pick
    }

    /// `CheckVeszelyesHely(f, x, y, reach)` (§12.1): (0 safe, 2 danger, 3 close danger; the distance to and the
    /// index of the last foe that raised the threat, 0.0 / 0 if none).
    pub(crate) fn danger_at(&self, f: usize, x: f32, y: f32, reach: f32) -> (i32, f32, usize) {
        let Some(me) = self.rec(f as i32) else { return (0, 0.0, 0) };
        let o = me.owner as usize;
        let t = self.totals(f as i32);
        let own = t.hp as f32 * t.attack as f32;
        let m = if self.spy.ctx.difficulty > 2 { 0.9f32 } else { 0.7 };
        let (mut best, mut near, mut dist, mut foe) = (0.0f32, false, 0.0f32, 0);
        for g in 1..self.starmap.fleets.len() {
            let r = &self.starmap.fleets[g];
            if r.visible.get(o).is_none_or(|&v| v == 0) || r.owner == crate::fleet::DELETED || r.owner as usize == o || r.story_id != 0 || r.flags & flag::IN_BATTLE != 0 {
                continue;
            }
            let delta = me.speed - r.speed;
            let a = DANGER_REACH[speed_class(me.speed, r.speed)] * reach;
            let (dx, dy) = (r.x - x, r.y - y);
            let d2 = dy * dy + dx * dx;
            let lv = self.level(o, r.owner as usize);
            let tg = self.totals(g as i32);
            let s = tg.hp as f32 * tg.attack as f32;
            if d2 < a * a && lv < 4 && (lv == level::WAR || d2 < (a * 0.5) * (a * 0.5)) && best < s {
                best = s * m;
                dist = root(d2);
                foe = g;
                let (a, d) = (a as f64, d2 as f64);
                let sq = |k: f64| (a * k) * (a * k);
                let delta64 = delta as f64;
                near = sq(0.25) > d
                    || (sq(0.33) > d && 0.02 >= delta64)
                    || (sq(0.5) > d && 0.0 >= delta)
                    || (sq(0.6) > d && -0.02 >= delta64)
                    || (sq(0.7) > d && -0.04 >= delta64);
            }
        }
        let planets = self.planet_count.get(o).copied().unwrap_or(0);
        if planets == 0 {
            best = 0.0;
        }
        if planets < 2 && self.nearest_planet(f, 20.0, true, false) < 1 {
            best = 0.0;
        }
        let code = if own < best { if near { 3 } else { 2 } } else { 0 };
        (code, dist, foe)
    }

    /// `BezartSzog(f, a, b)` (§12.3): the angle at fleet `f` between points `a` and `b`, degrees (0 when either is
    /// within 0.001).
    pub(crate) fn angle(&self, f: usize, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
        let Some(r) = self.rec(f as i32) else { return 0.0 };
        let (ux, uy, vx, vy) = (ax - r.x, ay - r.y, bx - r.x, by - r.y);
        let lu = root(uy * uy + ux * ux);
        let lv = root(vy * vy + vx * vx);
        if 0.001 >= lu as f64 || 0.001 >= lv as f64 {
            return 0.0;
        }
        let mut c = (vy * uy + vx * ux) / (lv * lu);
        // Compared as doubles against ±0.99, set to the f32 ±0.99: not a `clamp`.
        #[allow(clippy::manual_clamp)]
        if -0.99 > c as f64 {
            c = -0.99;
        }
        if c as f64 > 0.99 {
            c = 0.99;
        }
        ((((c as f64).acos() as f32) * 180.0) as f64 / std::f64::consts::PI) as f32
    }

    /// `MenekulesSuly(f, own, foe, x, y)` (§12.3): how safe point (x, y) is, by the distance to the nearest planet
    /// of each side against its radar reach.
    pub(crate) fn safety(&self, own: usize, foe: usize, x: f32, y: f32) -> f32 {
        let radar = &self.starmap.radar;
        let reach = |e: usize| radar.range.get(e).map_or(0.0, |r| r[1]) * radar.mult.get(e).copied().unwrap_or(0.0);
        let nearest = |e: usize| {
            let mut best = 1e10f32;
            for p in self.spy.colony.planets.iter().skip(1).filter(|p| p.owner as usize == e) {
                let (dx, dy) = (p.x - x, p.y - y);
                let d = dy * dy + dx * dx;
                if d <= best {
                    best = d;
                }
            }
            root(best)
        };
        let (r1, r2) = (reach(own), reach(foe));
        let s1 = nearest(own);
        let third = r1 / 3.0;
        let w = if third > s1 {
            s1 * 0.0 + 0.0
        } else if r1 > s1 {
            (s1 - third) * 6.0 + r1 * 0.0
        } else {
            (s1 - r1) * 10.0 + r1 * 4.0
        };
        let w = 10000.0 - w;
        let s2 = nearest(foe);
        let c1 = r2 * 5.0 * 0.25;
        let c2 = r2 * 13.0 * 0.25;
        if c1 > s2 {
            s2 * 16.0 + 0.0 + w
        } else if c2 > s2 {
            (s2 - c1) * 4.0 + r2 * 20.0 + w
        } else {
            ((s2 - c2) as f64 * 0.5 + (r2 * 28.0) as f64 + w as f64) as f32
        }
    }

    /// `MI_KitAkarTamadni(e)` (`diplomacy.md` §6.2): whom `e` wants to attack (0 = nobody).
    #[allow(clippy::needless_range_loop)]
    pub(crate) fn war_target(&self, e: usize, main_fleet: &[i32; EMPIRES]) -> usize {
        let ctx = self.ctx;
        let diff = self.spy.ctx.difficulty;
        if !ctx.may_attack[e] || self.wars(e) != 0 {
            return 0;
        }
        let g = if diff > 2 { 1.2f32 } else { 1.0 };
        let fleet = |slot: i32| self.rec(slot.max(0)).map_or((0, 0), |r| (r.strength, r.ground_strength));
        let mine = fleet(main_fleet[e]);
        let (mut best, mut pick) = (1e8f32, 0);
        let mp = self.spy.ctx.multiplayer;
        for c in 1..=(self.spy.ctx.program_players.max(0) as usize).min(EMPIRES - 1) {
            let human = self.human(c);
            if c == e
                || !self.alive(c)
                || !ctx.may_attack[c]
                || self.spy.colony.knows[e][c] == 0
                || !(mp || c != self.spy.ctx.human_empire || !ctx.spare_human[e])
                || !(diff > 1 || !human || self.wars(c) == 0)
            {
                continue;
            }
            let r = &self.spy.relations[e][c];
            let mut score = r.v[field::OPINION] as i32 - r.v[field::UNRELIABLE] as i32 / 3;
            score += match r.level {
                level::CEASEFIRE => -500,
                level::NAP => 3000,
                level::ALLIANCE => 6000,
                _ => 0,
            };
            if human && self.wars(c) > 0 {
                score += (3 - diff) * 500;
            }
            let fort = &self.spy.colony.strength.fortress;
            let reach = self.spy.colony.planets.iter().enumerate().skip(1).any(|(p, x)| {
                x.owner as usize == c
                    && x.visibility[e] > 0
                    && (((fort.get(p).copied().unwrap_or(0).wrapping_add(x.field_5c)) as f32 * g) as i32 <= mine.1
                        || self.planet_count[e] == 0)
            });
            if (score as f32) < best && reach && fleet(main_fleet[c]).0 / 4 < mine.0 {
                best = score as f32;
                pick = c;
            }
        }
        pick
    }
}

/// What the vehicle scan of `FRISSIT_MI_FLOTTAK` found (§11.2).
struct Vehicles {
    /// Per planet of e: the best ground kind held, its design and count.
    best: Vec<i32>,
    design: Vec<i16>,
    count: Vec<i32>,
    /// The best kind on any planet; vehicles on e's planets.
    top: i32,
    spare: i32,
    /// The weakest ground kind aboard the main fleet (1000 = none), its design and count.
    main_kind: i32,
    main_design: i16,
    main_count: i32,
}

impl AiState {
    /// `MenekulesHaKell(f, kind, target)` (§12.2): 0 nothing, 1 regrouped with a nearby fleet, 2 / 3 fled from a
    /// threat.
    pub(crate) fn flee(&mut self, w: &mut World, f: usize, kind: i32, target: i32) -> i32 {
        let Some(me) = w.rec(f as i32).cloned() else { return 0 };
        if me.story_id != 0 {
            return 0;
        }
        let o = me.owner as usize;
        let b5 = me.flags & flag::RADAR_IGNORED;
        let (tx, ty) = match kind {
            1 => w.spy.colony.planets.get(target.max(0) as usize).map_or((0.0, 0.0), |p| (p.x, p.y)),
            2 => w.rec(target).map_or((0.0, 0.0), |r| (r.x, r.y)),
            _ => (0.0, 0.0),
        };
        let (mut h, _, foe) = w.danger_at(f, me.x, me.y, 20.0);
        let m = self.main_fleet.get(o).copied().unwrap_or(0);
        if f as i32 != m {
            let mr = w.rec(m.max(0)).cloned().unwrap_or_default();
            let (dx, dy) = (me.x - mr.x, me.y - mr.y);
            if 400.0 > dy * dy + dx * dx {
                h = 0;
            }
        }
        let fr = w.rec(foe as i32).cloned().unwrap_or_default();
        let (mut chased, mut idle) = (false, false);
        if h != 0 {
            chased = fr.order == order::ATTACK && fr.kind == order::FLEET && fr.tx == f as f32;
            idle = fr.order == order::NONE;
            if kind == 1 {
                let df = root((me.y - ty) * (me.y - ty) + (me.x - tx) * (me.x - tx));
                let dg = root((fr.y - ty) * (fr.y - ty) + (fr.x - tx) * (fr.x - tx));
                if dg >= df + 5.0 || (dg >= df + 1.0 && idle) {
                    return 0;
                }
            }
        }
        if h == 3 && idle {
            h = 2;
        }
        if h == 0 {
            if kind != 0 || me.flags & flag::COLONISING != 0 {
                return 0;
            }
            let (mut best, mut pick) = (400.0f32, 0);
            for g in 1..w.starmap.fleets.len() {
                let r = &w.starmap.fleets[g];
                if r.owner as usize != o || g == f || r.strength <= 0 || r.story_id != 0 || r.flags & flag::COLONISING != 0 || g as i32 == m {
                    continue;
                }
                let (dx, dy) = (r.x - me.x, r.y - me.y);
                let d = cvtt((dx * dx + dy * dy) as f64) as f32;
                if best > d {
                    best = d;
                    pick = g;
                }
            }
            if pick < 1 {
                return 0;
            }
            w.order_unscripted(f, order::MOVE, order::FLEET, pick as f32, 0.0);
            w.order_unscripted(pick, order::MOVE, order::FLEET, f as f32, 0.0);
            let r = &mut w.starmap.fleets[f];
            r.flags = (r.flags & !flag::RADAR_IGNORED) | b5;
            w.mirror(f);
            w.starmap.fleets[pick].flags |= flag::RADAR_IGNORED;
            w.settle_radar_flag(pick);
            return 1;
        }

        let q = fr.owner as usize;
        let ok = |a: f32| a >= 80.0 && (a >= 90.0 || (idle && target > 0)) && (a >= 100.0 || !chased) && (a >= 140.0 || h == 2);
        let weight = |a: f32| if h == 2 { a * 0.125 } else { a * 0.25 };
        let base = |w: &World, x: f32, y: f32| {
            if h == 2 && kind >= 1 {
                10000.0 - w.angle(f, tx, ty, x, y)
            } else {
                w.safety(o, q, x, y)
            }
        };
        let (mut best, mut px, mut py, mut via) = (-1e6f32, 0.0f32, 0.0f32, 0usize);
        let mut phi = 0.0f32;
        loop {
            let cx = ((phi as f64).cos() as f32) * 10.0 + me.x;
            let cy = ((phi as f64).sin() as f32) * 10.0 + me.y;
            let a = w.angle(f, fr.x, fr.y, cx, cy);
            if ok(a) {
                let gain = weight(a) + base(w, cx, cy);
                if gain > best {
                    (best, px, py) = (gain, cx, cy);
                }
            }
            phi = (phi as f64 + FLEE_STEP) as f32;
            if (phi as f64) >= FULL_TURN {
                break;
            }
        }
        let t = w.totals(f as i32);
        let own = t.hp as f32 * t.attack as f32 * 0.25;
        let mp = w.spy.ctx.multiplayer;
        for g in 1..w.starmap.fleets.len() {
            let r = &w.starmap.fleets[g];
            if r.owner as usize != o || !(r.flags & flag::IN_BATTLE == 0 || mp) || g as i32 == m || r.flags & flag::COLONISING != 0 || r.story_id != 0 {
                continue;
            }
            let (gx, gy) = (r.x, r.y);
            let (dx, dy) = (me.x - gx, me.y - gy);
            let d2 = dx * dx + dy * dy;
            if !((f as i32 == m || d2 < 25.0) && d2 < 2500.0 && (h != 2 || kind == 0 || d2 < 9.0)) {
                continue;
            }
            let a = w.angle(f, fr.x, fr.y, gx, gy);
            if !ok(a) {
                continue;
            }
            let mut v = base(w, gx, gy) + 5.0;
            if (d2 as f64) < 225.0 {
                v += 5.0;
            }
            if d2 < 64.0 {
                v += 5.0;
            }
            let tg = w.totals(g as i32);
            if own <= tg.hp as f32 * tg.attack as f32 {
                v += 10.0;
            }
            let gain = weight(a) + v;
            if gain > best {
                (best, px, py, via) = (gain, gx, gy, g);
            }
        }
        let mut go = false;
        if kind > 0 {
            let a = w.angle(f, fr.x, fr.y, tx, ty);
            if ok(a) {
                let b = if h == 2 { 10000.0 - w.angle(f, tx, ty, tx, ty) } else { w.safety(o, q, tx, ty) };
                go = weight(a) + b + 1000.0 > best;
            }
        }
        if me.flags & flag::COLONISING != 0 && me.order == order::MOVE && me.kind == order::PLANET {
            if let Some(x) = self.last_bad_colony.get_mut(o) {
                *x = cvtt(me.tx as f64);
            }
        }
        let diff = w.spy.ctx.difficulty;
        if diff < 4 && (9 * diff) != 0 && w.ctx.tick.wrapping_rem(9 * diff) == 0 {
            w.order_unscripted(f, order::NONE, order::POINT, 0.0, 0.0);
            return 0;
        }
        if go {
            return 0;
        }
        if via < 1 {
            w.order_unscripted(f, order::MOVE, order::POINT, px, py);
        } else {
            w.order_unscripted(f, order::MOVE, order::FLEET, via as f32, 0.0);
            w.order_unscripted(via, order::MOVE, order::FLEET, f as f32, 0.0);
            w.starmap.fleets[via].flags |= flag::RADAR_IGNORED;
            w.mirror(via);
        }
        let r = &mut w.starmap.fleets[f];
        r.flags = (r.flags & !flag::RADAR_IGNORED) | b5;
        w.settle_radar_flag(f);
        w.settle_radar_flag(via);
        h
    }

    /// The vehicle scan (§11.2).
    fn scan_vehicles(&self, w: &World, e: usize) -> Vehicles {
        let n = w.spy.colony.planets.len().max(1);
        let mut v = Vehicles {
            best: vec![0; n],
            design: vec![0; n],
            count: vec![0; n],
            top: 0,
            spare: 0,
            main_kind: 1000,
            main_design: 0,
            main_count: 0,
        };
        if self.no_load[e] >= 1 {
            return v;
        }
        let m = self.main_fleet[e];
        for x in w.spy.combat.vehicles.iter().skip(1) {
            if x.design == NO_DESIGN || x.count <= 0 {
                continue;
            }
            let kind = w.des_i32(e, x.design as i32, 0x44).wrapping_sub(GROUND_HULL_BASE);
            if x.fleet < 0 {
                let p = x.fleet.unsigned_abs() as usize;
                if w.spy.colony.planets.get(p).is_some_and(|pl| pl.owner as usize == e) {
                    if v.best[p] < kind {
                        v.best[p] = kind;
                        v.design[p] = x.design;
                        v.count[p] = x.count;
                        v.top = v.top.max(kind);
                    }
                    v.spare = v.spare.wrapping_add(x.count);
                }
            }
            if x.fleet as i32 == m && 0 < kind && kind < v.main_kind {
                v.main_kind = kind;
                v.main_design = x.design;
                v.main_count = x.count;
            }
        }
        v
    }

    /// `FRISSIT_MI_FLOTTAK(e)` (§11).
    pub fn fleets(&mut self, w: &mut World, e: usize, hooks: &mut dyn AiHooks) {
        let mp = w.spy.ctx.multiplayer;
        let pp = (w.spy.ctx.program_players.max(0) as usize).min(EMPIRES - 1);
        let strongest = |w: &World, skip: usize| {
            let mut s = 0;
            for c in 1..=pp {
                if w.alive(c) && c != skip && (s == 0 || w.power[s] < w.power[c]) {
                    s = c;
                }
            }
            s
        };
        let s1 = strongest(w, usize::MAX);
        let s2 = strongest(w, s1);
        let (mut guard1, mut guard5) = (false, false);
        if !mp {
            match w.ctx.story_guard {
                [true, true] => {
                    guard1 = s1 != 1 && s2 != 1 && !w.human(1);
                    guard5 = s1 != 5 && s2 != 5 && !w.human(5);
                }
                [true, false] => guard1 = s1 != 1 && !w.human(1),
                [false, true] => guard5 = s1 != 5 && !w.human(5),
                [false, false] => {}
            }
        }
        if self.no_load[e] > 0 {
            self.no_load[e] -= 1;
        }
        if self.load_phase[e] > 0 {
            self.load_phase[e] -= 1;
        }
        if self.no_upgrade[e] > 0 {
            self.no_upgrade[e] -= 1;
            if self.no_upgrade[e] == 0 {
                self.design(w, e, true, hooks);
            }
        }

        // §11.2 vehicles for the main fleet.
        let mut v = self.scan_vehicles(w, e);
        let m = self.main_fleet[e];
        if self.no_load[e] < 1 {
            let t = w.totals(m);
            if t.ground < t.capacity || v.main_kind < v.top {
                let p = w.nearest_planet(m.max(0) as usize, 20.0, true, true);
                let t = w.totals(m);
                let garrison = w.spy.colony.planets.get(p).map_or(0, |x| x.field_58);
                if (t.ground < t.capacity || v.main_kind < v.best[p]) && p > 0 && v.best[p] > 0 && garrison > 0 {
                    let mut room = t.capacity.wrapping_sub(t.ground);
                    if v.main_kind < 1000 && v.main_kind < v.best[p] && room < v.count[p] {
                        let x = v.count[p].wrapping_sub(room).min(v.main_count);
                        if x > 0 {
                            w.transfer_vehicles(m, -(p as i32), v.main_design, x);
                        }
                        let t = w.totals(m);
                        room = t.capacity.wrapping_sub(t.ground);
                        v.spare = v.spare.wrapping_sub(x);
                    }
                    let x = v.count[p].min(room);
                    if x > 0 {
                        w.transfer_vehicles(-(p as i32), m, v.design[p], x);
                    }
                }
            }
        }

        // §11.3 wars with the dead end; a new war.
        for u in 1..=w.players2() {
            if u != e && w.level(e, u) == level::WAR && !w.alive(u) {
                w.spy.relations[e][u].level = 0;
                w.spy.relations[u][e].level = 0;
            }
        }
        if (!w.ctx.story_no_war || mp) && w.wars(e) == 0 {
            let t = w.war_target(e, &self.main_fleet);
            if t > 0 && w.known_since[e][t] < 0 {
                let diff = w.spy.ctx.difficulty;
                if w.r(1, 720) == 1 || w.planet_count[e] == 0 || (diff == 4 && w.human(t) && w.r(1, 720) == 1) {
                    hooks.declare_war(w, e, t);
                }
            }
        }

        // §11.4 the target.
        let (mut kind, mut target) = (0i32, 0usize);
        let (mut free, mut cand) = (false, false);
        if w.ctx.may_attack[e] && m > 0 && !(w.ctx.year == 0x917 && w.ctx.month == 3) {
            let mr = w.rec(m).cloned().unwrap_or_default();
            let mt = w.totals(m);
            let mut best = 1e10f32;
            let mut bold = false;
            if w.planet_count[e] > 0 {
                for f in 1..w.starmap.fleets.len() {
                    let r = &w.starmap.fleets[f];
                    if r.owner == crate::fleet::DELETED || r.story_id != 0 {
                        continue;
                    }
                    let u = r.owner as usize;
                    let their_main = self.main_fleet.get(u).and_then(|&x| w.rec(x.max(0))).map_or(0, |x| x.strength);
                    if w.level(e, u) != level::WAR || r.visible[e] == 0 || r.strength >= mr.strength {
                        continue;
                    }
                    if !(mr.strength / 8 < their_main || r.ground_strength > 0) {
                        continue;
                    }
                    let (dx, dy) = (mr.x - r.x, mr.y - r.y);
                    let mut d = root(dy * dy + dx * dx);
                    if self.load_phase[e] < 1 {
                        d /= 3.0;
                    }
                    let k = speed_class(mr.speed, r.speed);
                    let mut reach = if r.ground_strength > 0 { GROUND_RANGE[k] } else { PLAIN_RANGE[k] };
                    let p = cvtt(r.tx as f64);
                    let weak = r.order == order::ATTACK && r.kind == order::PLANET && {
                        let pl = w.spy.colony.planets.get(p.max(0) as usize);
                        let fort = w.spy.colony.strength.fortress.get(p.max(0) as usize).copied().unwrap_or(0);
                        pl.is_some_and(|pl| pl.owner as usize == e && r.ground_strength as f64 * 1.3 > fort.wrapping_add(pl.field_5c) as f64)
                    };
                    if weak {
                        reach *= 2.0;
                    }
                    if reach >= d {
                        let ratio = mr.strength.checked_div(r.strength.wrapping_add(1)).unwrap_or(0);
                        let mut sc = (ratio.wrapping_add(3) as f32 * d) / (k as i32 + 2) as f32;
                        if weak {
                            sc /= 3.0;
                        }
                        if sc < best {
                            best = sc;
                            kind = 2;
                            target = f;
                            bold = k >= 6
                                && (reach * 0.5 >= d || mr.strength < r.strength.wrapping_mul(2))
                                && mr.strength as f64 > r.strength as f64 * 1.3;
                        }
                    }
                }
            }
            let fleet_kind = kind;
            let g = if w.spy.ctx.difficulty > 2 { 1.2f32 } else { 1.0 };
            let (cx, cy) = (self.centre_x[e] as f32, self.centre_y[e] as f32);
            let no_planets = w.planet_count[e] == 0;
            let mut best = 1e10f32;
            for p in 1..w.spy.colony.planets.len() {
                let pl = &w.spy.colony.planets[p];
                let o = pl.owner as usize;
                if (guard1 && o == 1) || (guard5 && o == 5) {
                    continue;
                }
                if o == 0 || pl.visibility[e] <= 0 || w.level(e, o) != level::WAR || !capturable(pl) || o == e {
                    continue;
                }
                cand = true;
                let fort = w.spy.colony.strength.fortress.get(p).copied().unwrap_or(0);
                let def = cvtt((fort.wrapping_add(pl.field_5c) as f32 * g) as f64);
                if !(self.load_phase[e] < 1 && mt.ground > 0 && mt.ground_strength > 0) {
                    continue;
                }
                let (px, py) = (pl.x, pl.y);
                if !((w.danger_at(m as usize, px, py, 10.0).0 == 0 || no_planets) && (def <= mt.ground_strength || no_planets)) {
                    continue;
                }
                let pl = &w.spy.colony.planets[p];
                let (dx, dy) = (px - cx, py - cy);
                let a = root(dx * dx + dy * dy) + 100.0;
                let (fx, fy) = (px - mr.x, py - mr.y);
                let b = root(fy * fy + fx * fx) + 50.0;
                let caps = pl.ship_capacity.wrapping_add(pl.vehicle_capacity).wrapping_add(5000);
                let mut sc = (((a * b) / pl.population.wrapping_add(30000) as f32) / caps as f32)
                    * mt.ground_strength.wrapping_add(def.wrapping_mul(5)) as f32;
                let (gd, mine) = (def as f64, mt.ground_strength as f64);
                if mine < gd * 1.5 {
                    sc *= 10.0;
                }
                if gd * 2.0 <= mine {
                    free = true;
                } else {
                    sc *= 10.0;
                }
                if sc < best && (fleet_kind == 0 || (!bold && gd * 1.5 <= mine) || no_planets) {
                    best = sc;
                    kind = 1;
                    target = p;
                }
            }
            let t = w.totals(m);
            if !free && self.load_phase[e] < 1 && kind == 1 && cand {
                let (c78, c5c) = (t.capacity, t.ground);
                if c5c < c78 && c5c < v.spare.wrapping_mul(4) && c5c < c78.wrapping_sub(c5c).wrapping_mul(4) {
                    self.load_phase[e] = 300;
                    kind = 0;
                }
            }
            if self.load_phase[e] > 0 && (t.capacity <= t.ground || v.spare < 1 || !cand) {
                self.load_phase[e] = 0;
            }
        }

        // §11.5 orders.
        let m = self.main_fleet[e];
        let mut fled = 0;
        if m > 0 && w.rec(m).is_some_and(|r| r.owner as usize == e) {
            fled = self.flee(w, m as usize, kind, target as i32);
            let mut f = 1;
            while f < w.starmap.fleets.len() {
                let r = &w.starmap.fleets[f];
                if r.owner as usize == e && r.story_id == 0 && f as i32 != m {
                    let (k, t) = (r.kind as i32, cvtt(r.tx as f64));
                    let res = self.flee(w, f, k, t);
                    let r = &w.starmap.fleets[f];
                    if !matches!(res, 1..=3) && r.flags & (flag::COLONISING | flag::TERRAFORMING) == 0 && (r.flags & flag::RADAR_IGNORED == 0 || r.order == order::NONE) {
                        w.order_unscripted(f, order::MOVE, order::FLEET, m as f32, 0.0);
                        w.starmap.fleets[f].flags |= flag::RADAR_IGNORED;
                        w.settle_radar_flag(f);
                    }
                }
                f += 1;
            }
        }
        if kind > 0 {
            let hi = if w.spy.ctx.difficulty == 1 { 50 } else { 1 };
            if w.r(1, hi) != 1 {
                kind = 0;
            }
        }
        let m = self.main_fleet[e];
        if m > 0 && !matches!(fled, 1..=3) {
            if kind > 0 {
                w.order_unscripted(m as usize, order::ATTACK, kind as i16, target as f32, 0.0);
            } else if w.rec(m).is_some_and(|r| r.order == order::NONE) {
                let t = w.totals(m);
                if t.ships.wrapping_sub(t.fighters) <= 0 {
                    w.order_unscripted(m as usize, order::MOVE, order::PLANET, self.main_planet[e] as f32, 0.0);
                } else {
                    self.scout(w, e, &v, cand);
                }
            }
        }

        // §11.6 unloading after a flight.
        if fled == 2 || fled == 3 {
            let m = self.main_fleet[e];
            let p = w.nearest_planet(m.max(0) as usize, 30.0, true, true);
            if p > 0 {
                while w.totals(m).ground > 0 {
                    let (mut d, mut c) = (0i16, 0i32);
                    for x in w.spy.combat.vehicles.iter().skip(1) {
                        if x.design != NO_DESIGN && x.fleet as i32 == m && x.count >= 1 && w.des_i32(e, x.design as i32, 0x44) - GROUND_HULL_BASE > 0 {
                            (d, c) = (x.design, x.count);
                        }
                    }
                    if d <= 0 {
                        // The original loops forever here (ground units of a non-ground hull aboard): never seen.
                        break;
                    }
                    w.transfer_vehicles(m, -(p as i32), d, c);
                }
                self.no_load[e] = 60;
            }
        }
    }

    /// The idle main fleet scouts, fetches vehicles or goes home (§11.5).
    fn scout(&mut self, w: &mut World, e: usize, v: &Vehicles, cand: bool) {
        let m = self.main_fleet[e];
        for p in w.spy.colony.planets.iter().skip(1).filter(|p| p.owner as usize == e) {
            let (cx, cy) = (cell(p.x), cell(p.y));
            let grid = &mut self.scout[e];
            for i in -1..=1 {
                for j in -1..=1 {
                    if (1..GRID as i32).contains(&(cx + i)) && (1..GRID as i32).contains(&(cy + j)) {
                        grid[(cx + i) as usize][(cy + j) as usize] = 2;
                    }
                }
            }
            for i in -2..=2 {
                for j in -2..=2 {
                    if (1..GRID as i32).contains(&(cx + i)) && (1..GRID as i32).contains(&(cy + j)) {
                        let c = &mut grid[(cx + i) as usize][(cy + j) as usize];
                        if *c == 0 {
                            *c = 1;
                        }
                    }
                }
            }
        }
        let me = w.rec(m).cloned().unwrap_or_default();
        let (mx, my) = (cell(me.x), cell(me.y));
        // Unchecked in the original: a cell outside the grid lands elsewhere in the table (or past it: dropped).
        let flat = ((e * GRID) as i64 + mx as i64) * GRID as i64 + my as i64;
        if let Some(c) = usize::try_from(flat).ok().and_then(|i| self.scout.as_flattened_mut().as_flattened_mut().get_mut(i)) {
            *c = 2;
        }
        let cxc = (self.centre_x[e].wrapping_add(300)) / 30 + 1;
        let cyc = (self.centre_y[e].wrapping_add(300)) / 30 + 1;
        let (mut best, mut bi, mut bj) = (9_999_999.0f32, 0i32, 0i32);
        for i in 1..GRID as i32 {
            for j in 1..GRID as i32 {
                if self.scout[e][i as usize][j as usize] != 1 {
                    continue;
                }
                let (a, b) = ((mx - i) as f32, (my - j) as f32);
                let to_fleet = root(b * b + a * a) as f64 / 3.0;
                let (c, d) = ((cxc - i) as f32, (cyc - j) as f32);
                let sc = (to_fleet + root(d * d + c * c) as f64) as f32;
                if sc < best {
                    best = sc;
                    (bi, bj) = (i, j);
                }
            }
        }
        let t = w.totals(m);
        let fetch = cand && self.no_load[e] < 1 && (t.ground < t.capacity || v.main_kind < v.top) && (bi == 0 || w.r(1, 1) == 1);
        if fetch {
            let (fx, fy) = (cvtt(me.x as f64) as f32, cvtt(me.y as f64) as f32);
            let (mut top, mut pick) = (0.0f32, 0usize);
            for (p, pl) in w.spy.colony.planets.iter().enumerate().skip(1) {
                if pl.owner as usize != e || !((t.ground < t.capacity || v.main_kind < v.best[p]) && v.best[p] == v.top) {
                    continue;
                }
                let g = pl.field_5c as f32;
                let (dx, dy) = (pl.x - fx, pl.y - fy);
                let sc = (g * g) / (dy * dy + dx * dx + 1.0);
                if sc > top {
                    top = sc;
                    pick = p;
                }
            }
            if pick >= 1 {
                w.order_unscripted(m as usize, order::MOVE, order::PLANET, pick as f32, 0.0);
                return;
            }
        }
        if bi > 0 {
            w.order_unscripted(m as usize, order::MOVE, order::POINT, ((bi - 11) * 30) as f32, ((bj - 11) * 30) as f32);
        } else {
            w.order_unscripted(m as usize, order::MOVE, order::PLANET, self.main_planet[e] as f32, 0.0);
        }
    }
}
