//! Radar ranges and visibility on the starmap: `GetENRadarKifejlesztve` (exe 0x53e020, every tick)
//! and the refresh block of `FRISSIT_Starmap` (exe 0x594ae0, ticks with `Ido2 % 10 == 1`).
//! Spec: `re/rules/fleet-movement.md` §3 and §6. Precision follows the exe: everything is f32 except
//! the radar-booster distance, which is summed in f64 (§6.3 step 4).
//!
//! Story hooks, discovery bookkeeping outside the starmap (`FelfedezBirodalom`) and the tutorial are
//! reported as [`StarmapEvent`]s.

use crate::fleet::{flag, order, Starmap, StarmapContext, StarmapEvent};

/// Story ids of the radar-booster fleets (supply anchors, §6.3 step 4).
pub const BOOSTER_IDS: [i16; 3] = [1011, 1021, 1311];
/// Story id exempt from the "enemy approaching" warning.
pub const NO_WARNING_ID: i16 = 0x1f88;
/// `+0x52` bit6: tell the story when the human first sees this fleet.
pub const NOTICE: u16 = 1 << 6;
/// Empires are 0..=18.
pub const EMPIRES: usize = 19;

/// The compiled-in radar tables, read from the user's exe (`ig2-rules`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RadarRules {
    /// `RadarHatotav`: range per radar tech level 0..=5.
    pub ranges: [f32; 6],
    /// `RadarHatotav2Init`: planet radar range per planet radar level, stored right after `ranges`.
    pub planet_ranges: [f32; 8],
    /// Wormhole `w`'s colour (`i32 [6][3]` at exe 0x913850, read by `AddWormhole`; `Game::add_wormhole`).
    pub wormhole_colours: [[i32; 3]; 6],
}

impl RadarRules {
    /// `RadarHatotav[i]`; like the exe, indices past the end read on into `RadarHatotav2Init`.
    pub fn range(&self, i: usize) -> f32 {
        match i {
            0..6 => self.ranges[i],
            _ => self.planet_ranges.get(i - 6).copied().unwrap_or(0.0),
        }
    }
}

/// The live per-empire tables (`RadarHatotav2`, `RadarHatotav2Negyzet`, `RadarHatotavSzorzo`,
/// `RadarHatotavSzorzoNegyzet`); row 0 mirrors the human empire's row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RadarTables {
    /// `[e][level]`: planet radar range; `[e][1]` is the empire's radar tech range.
    pub range: [[f32; 8]; EMPIRES],
    pub range_sq: [[f32; 8]; EMPIRES],
    /// 1.0 for human-controlled empires and their allies, 1.5 for the others.
    pub mult: [f32; EMPIRES],
    pub mult_sq: [f32; EMPIRES],
}

impl RadarTables {
    /// `InitNewGame`: empires 1..=18 get `RadarHatotav2Init` and multiplier 1.0.
    pub fn new_game(rules: &RadarRules) -> Self {
        let mut t = Self::default();
        for e in 1..EMPIRES {
            t.range[e] = rules.planet_ranges;
            t.range_sq[e] = rules.planet_ranges.map(|r| r * r);
            t.mult[e] = 1.0;
        }
        t
    }

    /// `GetENRadarKifejlesztve` (§6.2). Returns the human's radar tech level.
    pub fn refresh(&mut self, rules: &RadarRules, ctx: &StarmapContext) -> i32 {
        let n = ctx.program_players2.min(EMPIRES - 1);
        for e in 1..=n {
            let c = ctx.radar_tech[e];
            // The highest developed radar tech; empire 2 (the literal index) gets one level more.
            self.range[e][1] = if c > 0 { (c + (e == 2) as u8) as f32 } else { 0.0 };
        }
        let en = ctx.human_empire as usize;
        if !ctx.multiplayer && ctx.story_radar_bonus {
            self.range[en][1] += 1.0;
        }
        let level = self.range[en][1];
        for e in 1..=n {
            let r = rules.range(self.range[e][1] as i32 as usize);
            self.range[e][1] = r;
            self.range_sq[e][1] = r * r;
            let human = |j: usize| ctx.human_controlled[j];
            let allied_human = (1..=n).any(|j| human(j) && ctx.relation_level(e, j) == 5);
            self.mult[e] = if human(e) || allied_human { 1.0 } else { 1.5 };
            self.mult_sq[e] = self.mult[e] * self.mult[e];
        }
        self.range[0] = self.range[en];
        self.range_sq[0] = self.range_sq[en];
        self.mult[0] = self.mult[en];
        self.mult_sq[0] = self.mult_sq[en];
        level as i32
    }

    /// Squared radius of empire `o`'s radar circle around a planet of radar level `level`.
    fn planet_circle(&self, o: usize, level: usize) -> f32 {
        self.range_sq[o].get(level).copied().unwrap_or(0.0) * self.mult_sq[o]
    }
}

/// One ship as the refresh sees it: its fleet and its design's speed, type, radar level and stealth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShipDesign {
    pub fleet: usize,
    /// Design `+0x3c`.
    pub speed: f32,
    /// Design `+0x48` (80 = slow class).
    pub design_type: i32,
    /// Design `+0x6a`: radar tech level.
    pub radar: u8,
    /// Design `+0x6b`.
    pub stealth: u8,
}

/// `dy² + dx²` in f32, the exe's order.
fn dist_sq(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (dx, dy) = (ax - bx, ay - by);
    dy * dy + dx * dx
}

impl Starmap {
    /// "Meet": `BirodalomIsmeri[e][o] = 1`, reported once.
    fn meet(&mut self, e: usize, o: u8, ev: &mut Vec<StarmapEvent>) {
        if self.met[e][o as usize] == 0 {
            self.met[e][o as usize] = 1;
            ev.push(StarmapEvent::EmpireMet { by: e as u8, empire: o });
        }
    }

    /// The refresh tick (§3 + §6.3 steps 1–7). `self.radar` must already be refreshed for this tick.
    pub fn refresh(&mut self, ships: &[ShipDesign], ctx: &StarmapContext, ev: &mut Vec<StarmapEvent>) {
        let rules = &ctx.radar_rules;
        let tracked = ctx.story_tracked_fleet;
        let is_tracked = |f: usize| tracked == f as i32;

        // 1. Planet radar level from finished colony hubs.
        for p in self.planets.iter_mut().skip(1) {
            if p.radar_level < 2 {
                let own_vis = p.visible.get(p.owner as usize).copied().unwrap_or(-1);
                p.radar_level = (p.owner != 0 && own_vis >= 0 && p.has_hub) as u8;
            }
        }

        // 2. Fleet reset, idle colony ship/terraformer pools, boosters.
        self.colony_ship_pool[1..].fill(0);
        self.terraformer_pool[1..].fill(0);
        let mut boosters = Vec::new();
        for (f, fl) in self.fleets.iter_mut().enumerate().skip(1).filter(|(_, f)| f.alive()) {
            fl.flags &= !(flag::IN_BATTLE | flag::STEALTH);
            fl.speed = 0.0;
            fl.radar = 0.0;
            if is_tracked(f) && fl.state & 1 == 0 {
                fl.radar = rules.range(2) * 0.25;
            }
            let o = fl.owner as usize;
            if fl.flags & flag::COLONISING == 0 {
                self.colony_ship_pool[o] += fl.colony_ships as i32;
            }
            if fl.flags & flag::TERRAFORMING == 0 {
                self.terraformer_pool[o] += fl.terraformers as i32;
            }
            if BOOSTER_IDS.contains(&fl.story_id) {
                boosters.push(f);
            }
        }

        // 3. Ships: speed (§3), radar range, stealth.
        for s in ships {
            let Some(f) = self.fleets.get_mut(s.fleet).filter(|f| f.alive()) else { continue };
            let mut v = s.speed;
            if !(v > 0.0 || s.design_type == 80) {
                continue;
            }
            if s.design_type == 80 {
                v *= 0.25;
            }
            if f.speed as f64 <= 1e-5 {
                f.speed = v;
            }
            let speed = if f.speed_override > 0.0 { f.speed_override } else { v.min(f.speed) };
            f.speed = if ctx.debug_speed { speed * 100.0 } else { speed };
            f.radar = if f.state & 1 == 0 {
                let r = rules.range(s.radar as usize) * 0.5 * self.radar.mult[f.owner as usize];
                if f.radar <= r { r } else { f.radar }
            } else {
                0.0
            };
            if s.stealth != 0 {
                f.flags |= flag::STEALTH;
            }
        }

        // 4. Supply: radar fades with the distance to the nearest own/allied radar planet or booster.
        for f in 1..self.fleets.len() {
            let fl = &self.fleets[f];
            if !fl.alive() {
                continue;
            }
            let o = fl.owner as usize;
            let (fx, fy) = (fl.x, fl.y);
            let mut best = 1e20f32;
            if !ctx.story_no_supply && !is_tracked(f) {
                for (i, p) in self.planets.iter().enumerate().skip(1) {
                    let anchor = (p.owner as usize == o || ctx.relation_level(o, p.owner as usize) == 5) && p.radar_level != 0;
                    if anchor || i as i32 == ctx.story_supply_planet {
                        best = best.min(dist_sq(fx, fy, p.x, p.y));
                    }
                }
            } else {
                best = 0.0;
            }
            let lead = self.radar.range[o][1] as f64 * 1.5;
            for &b in &boosters {
                let (dx, dy) = (fx - self.fleets[b].x, fy - self.fleets[b].y);
                let d = ((dy * dy * 2.0 + dx * dx) as f64 + lead) as f32;
                if d < best {
                    best = d;
                }
            }
            let r = self.radar.mult_sq[o] * self.radar.range_sq[o][1];
            if r * 4.0 < best {
                let fl = &mut self.fleets[f];
                fl.radar = if best <= r * 9.0 { (3.0 - ((best / r) as f64).sqrt() as f32) * fl.radar } else { 0.0 };
            }
        }

        // 5. Alliances (level-5 relations, self included), in index order.
        let n2 = ctx.program_players2.min(EMPIRES - 1);
        let mut allies: Vec<Vec<usize>> = vec![Vec::new(); EMPIRES];
        for (a, list) in allies.iter_mut().enumerate().take(n2 + 1).skip(1) {
            list.extend((1..=n2).filter(|&j| ctx.relation_level(a, j) == 5 || a == j));
        }
        let en = ctx.human_empire as usize;
        // Who sees inside empire `o`'s circle around a radar planet: its allies, and in SP the human
        // too when the planet's level is above 1 and the point lies within the human's own circle.
        let viewers = |o: usize, level: usize, d: f32, tables: &RadarTables| -> Vec<usize> {
            let mut v = allies[o].clone();
            if !ctx.multiplayer && level > 1 && d < tables.planet_circle(en, level) {
                v.push(en);
            }
            v
        };
        let observer = |owner: u8| if !ctx.multiplayer && owner == 0 { en } else { owner as usize };
        let stealth_cut = |f_flags: u16, o: usize, tables: &RadarTables| {
            f_flags & flag::STEALTH != 0 && tables.range[o][1] < rules.range(4)
        };

        // 6. Fleet visibility.
        for f in 1..self.fleets.len() {
            if !self.fleets[f].alive() {
                continue;
            }
            let fl = &mut self.fleets[f];
            if fl.speed <= 0.0 {
                fl.speed = 0.06;
            }
            let (fx, fy, f_flags) = (fl.x, fl.y, fl.flags);
            let old = fl.visible;
            fl.visible[1..=n2].fill(0);
            fl.visible[fl.owner as usize] = 1;
            for p in self.planets.iter().skip(1).filter(|p| p.radar_level != 0) {
                let (o, level) = (observer(p.owner), p.radar_level as usize);
                let mut r = self.radar.planet_circle(o, level);
                if stealth_cut(f_flags, o, &self.radar) {
                    r /= 25.0;
                }
                let d = dist_sq(fx, fy, p.x, p.y);
                if d < r {
                    for e in viewers(o, level, d, &self.radar) {
                        self.fleets[f].visible[e] = 1;
                    }
                }
            }
            for g in 1..self.fleets.len() {
                let gl = &self.fleets[g];
                if !gl.alive() || gl.flags & flag::RADAR_IGNORED != 0 {
                    continue;
                }
                let go = gl.owner as usize;
                let mut r = gl.radar * gl.radar;
                if stealth_cut(f_flags, go, &self.radar) {
                    r /= 25.0;
                }
                let (dx, dy) = (fx - gl.x, fy - gl.y);
                if dx * dx + dy * dy < r {
                    self.fleets[f].visible[go] = 1;
                    if is_tracked(g) {
                        self.fleets[f].visible[en] = 1;
                    }
                }
            }
            if is_tracked(f) && self.fleets[f].state & 1 == 0 {
                self.fleets[f].visible[en] = 1;
            }
            for w in 1..self.wormholes.len() {
                let h = self.wormholes[w];
                for (revealed, end) in [(h.revealed_a, h.fleet_a), (h.revealed_b, h.fleet_b)] {
                    if revealed {
                        if let Some(e) = self.fleets.get_mut(end as usize) {
                            e.visible[en] = 1;
                        }
                    }
                }
            }
            let fl = &self.fleets[f];
            if fl.visible[en] != 0 && old[en] == 0 && fl.order == order::ATTACK && fl.tx > 0.0 {
                let t = fl.tx as i32 as usize;
                let target_owner = match fl.kind {
                    order::PLANET => self.planets.get(t).map(|p| p.owner as usize),
                    order::FLEET => self.fleets.get(t).map(|g| g.owner as usize),
                    _ => None,
                };
                if target_owner == Some(en) && fl.owner != 0 && fl.story_id != NO_WARNING_ID {
                    ev.push(StarmapEvent::EnemyApproaching { fleet: f });
                }
            }
            let owner = self.fleets[f].owner;
            for (e, &was) in old.iter().enumerate().take(ctx.program_players.min(EMPIRES - 1) + 1).skip(1) {
                let fl = &mut self.fleets[f];
                if fl.visible[e] != 0 && (was == 0 || fl.flags & NOTICE != 0) {
                    if fl.flags & NOTICE != 0 && e == en {
                        fl.flags &= !NOTICE;
                        ev.push(StarmapEvent::FleetSighted { fleet: f });
                    }
                    self.meet(e, owner, ev);
                }
            }
        }

        // 7. Planet visibility (never cleared).
        for p in 1..self.planets.len() {
            let (px, py) = (self.planets[p].x, self.planets[p].y);
            for q in 1..self.planets.len() {
                let qp = self.planets[q];
                if qp.radar_level == 0 {
                    continue;
                }
                let (o, level) = (observer(qp.owner), qp.radar_level as usize);
                let pl = &self.planets[p];
                let wanted = pl.visible[o] == 0 || (o == en && pl.notice != 0) || allies[o].len() >= 2 || level >= 2;
                let d = dist_sq(px, py, qp.x, qp.y);
                if !wanted || d >= self.radar.planet_circle(o, level) {
                    continue;
                }
                for e in viewers(o, level, d, &self.radar) {
                    self.sight_planet(p, e, en, ev);
                }
            }
            for g in 1..self.fleets.len() {
                let gl = &self.fleets[g];
                if !gl.alive() || gl.flags & flag::RADAR_IGNORED != 0 {
                    continue;
                }
                let go = gl.owner as usize;
                let pl = &self.planets[p];
                let considered = pl.visible[go] == 0 || (go == en && pl.notice != 0) || is_tracked(g);
                let (dx, dy) = (px - gl.x, py - gl.y);
                if !considered || dy * dy + dx * dx >= gl.radar * gl.radar {
                    continue;
                }
                self.planets[p].visible[go] = 1;
                if ctx.tutorial_sighting {
                    ev.push(StarmapEvent::TutorialPlanetSighted { planet: p });
                }
                if go == en && self.planets[p].notice != 0 {
                    self.planets[p].notice = 0;
                    ev.push(StarmapEvent::PlanetSighted { planet: p });
                }
                let po = self.planets[p].owner;
                if go != 0 && po != 0 {
                    self.meet(go, po, ev);
                }
                if is_tracked(g) {
                    self.planets[p].visible[en] = 1;
                    if po != 0 {
                        self.meet(en, po, ev);
                    }
                }
            }
        }
    }

    /// Planet `p` comes into empire `e`'s view through a radar planet (§6.3 step 7).
    fn sight_planet(&mut self, p: usize, e: usize, en: usize, ev: &mut Vec<StarmapEvent>) {
        let pl = &mut self.planets[p];
        if !(pl.visible[e] == 0 || (e == en && pl.notice != 0)) {
            return;
        }
        pl.visible[e] = 1;
        if e == en && pl.notice != 0 {
            pl.notice = 0;
            ev.push(StarmapEvent::PlanetSighted { planet: p });
        }
        let po = pl.owner;
        if e > 0 && po != 0 {
            self.meet(e, po, ev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::{Fleet, StarPlanet, DELETED};

    fn rules() -> RadarRules {
        // Synthetic values (the real ones come from the user's exe).
        RadarRules { ranges: [0.0, 10.0, 20.0, 30.0, 40.0, 50.0], planet_ranges: [0.0, 0.0, 0.0, 10.0, 20.0, 30.0, 40.0, 50.0], ..Default::default() }
    }

    fn ctx() -> StarmapContext {
        let mut c = StarmapContext {
            tick: 1,
            speed_mult: 1,
            human_empire: 1,
            program_players: 8,
            program_players2: 16,
            alive: [true; 19],
            radar_rules: rules(),
            story_tracked_fleet: 0,
            ..Default::default()
        };
        c.human_controlled[1] = true;
        c.relations = vec![[(0, 1); 19]; 19];
        c
    }

    fn map() -> Starmap {
        let mut m = Starmap { fleets: vec![Fleet { owner: DELETED, ..Default::default() }], planets: vec![StarPlanet::default()], ..Default::default() };
        m.radar = RadarTables::new_game(&rules());
        for e in 0..EMPIRES {
            m.met[e][e] = 1; // every empire knows itself in a running game
        }
        m
    }

    fn fleet(owner: u8, x: f32) -> Fleet {
        Fleet { owner, x, ship_count: 1, ..Default::default() }
    }

    fn ship(fleet: usize, radar: u8, stealth: u8) -> ShipDesign {
        ShipDesign { fleet, speed: 1.0, design_type: 0, radar, stealth }
    }

    #[test]
    fn tables_follow_radar_tech_humans_and_alliances() {
        let mut c = ctx();
        c.radar_tech[1] = 2;
        c.radar_tech[2] = 1;
        c.radar_tech[3] = 4;
        c.relations[4][1] = (0, 5); // AI empire 4 allied with the human
        let mut t = RadarTables::new_game(&rules());
        assert_eq!(t.refresh(&rules(), &c), 2, "returns the human's tech level");
        assert_eq!(t.range[1][1], 20.0);
        assert_eq!(t.range[2][1], 20.0, "empire 2 gets one level more");
        assert_eq!(t.range[3][1], 40.0);
        assert_eq!(t.range_sq[3][1], 1600.0);
        assert_eq!((t.mult[1], t.mult[3], t.mult[4]), (1.0, 1.5, 1.0));
        assert_eq!(t.mult_sq[3], 2.25);
        assert_eq!(t.range[0], t.range[1], "row 0 mirrors the human");
        assert_eq!(t.range[3][3], 10.0, "other levels keep the new-game ranges");
        c.story_radar_bonus = true;
        t.refresh(&rules(), &c);
        assert_eq!(t.range[1][1], 30.0, "story bonus: one more level for the human");
    }

    #[test]
    fn fleet_radar_is_the_best_ship_scaled_by_supply() {
        let mut c = ctx();
        c.radar_tech[1] = 1; // planet-free supply radius: R = 10², fade from 2·10 to 3·10
        let mut m = map();
        m.radar.refresh(&c.radar_rules, &c);
        m.planets.push(StarPlanet { owner: 1, has_hub: true, ..Default::default() });
        m.fleets.push(fleet(1, 15.0));
        m.fleets.push(fleet(1, 25.0));
        m.fleets.push(fleet(1, 31.0));
        let ships = [ship(1, 2, 0), ship(1, 4, 0), ship(2, 4, 0), ship(3, 4, 0)];
        let mut ev = Vec::new();
        m.refresh(&ships, &c, &mut ev);
        assert_eq!(m.planets[1].radar_level, 1, "a finished hub makes a radar planet");
        assert_eq!(m.fleets[1].radar, 20.0, "max over ships of range/2 × 1.0");
        let fade = (3.0 - ((625.0f32 / 100.0) as f64).sqrt() as f32) * 20.0;
        assert_eq!(m.fleets[2].radar, fade);
        assert_eq!(m.fleets[3].radar, 0.0, "beyond 3R: no radar");
    }

    /// The booster sum is f64 in the exe (§6.3 step 4) but can't differ from an f32 sum on reachable inputs: an
    /// integral radar range times 1.5 is exact in f32, and two f32s whose exponents differ by < 29 add exactly in
    /// f64, so the cast back rounds once like `addss` (`traces/2026-10-16-mp-radar.md`, "Booster sum").
    #[test]
    fn booster_sum_in_f64_equals_f32_on_reachable_inputs() {
        let mut x = 12345u32;
        let mut next = || {
            x = x.wrapping_mul(1103515245).wrapping_add(12345);
            (x >> 8) as f32 / (1 << 24) as f32
        };
        for _ in 0..200_000 {
            let range = (next() * 1000.0) as i32 as f32;
            let (dx, dy) = ((next() - 0.5) * 8000.0, (next() - 0.5) * 8000.0);
            let d = dy * dy * 2.0 + dx * dx;
            let wide = (d as f64 + range as f64 * 1.5) as f32;
            assert_eq!(wide.to_bits(), (d + range * 1.5).to_bits(), "range {range}, d {d}");
        }
    }

    #[test]
    fn stealth_fleets_hide_from_weak_radar_and_discovery_is_reported_once() {
        let mut c = ctx();
        c.radar_tech[1] = 1;
        let mut m = map();
        m.radar.refresh(&c.radar_rules, &c);
        m.planets.push(StarPlanet { owner: 1, has_hub: true, ..Default::default() });
        m.fleets.push(fleet(3, 5.0)); // inside the hub circle (10²) but not the stealth one (10²/25)
        m.fleets.push(fleet(4, 8.0));
        let mut ev = Vec::new();
        m.refresh(&[ship(1, 0, 1), ship(2, 0, 0)], &c, &mut ev);
        assert!(m.fleets[1].flags & flag::STEALTH != 0);
        assert_eq!(m.fleets[1].visible[1], 0, "stealth beats a level-1 radar");
        assert_eq!(m.fleets[2].visible[1], 1);
        assert_eq!(ev, vec![StarmapEvent::EmpireMet { by: 1, empire: 4 }]);
        assert_eq!(m.planets[1].visible[1], 1, "the hub sees its own planet");
        ev.clear();
        m.refresh(&[ship(1, 0, 1), ship(2, 0, 0)], &c, &mut ev);
        assert!(ev.is_empty(), "already met");
    }

    #[test]
    fn wormhole_ends_and_approaching_attackers_are_shown_to_the_human() {
        use crate::fleet::Wormhole;
        let c = StarmapContext { story_no_supply: true, ..ctx() }; // no radar tech: keep radar unfaded
        let mut m = map();
        m.radar.refresh(&c.radar_rules, &c);
        m.fleets.push(fleet(1, 0.0)); // the human's fleet, radar 10 from its ship
        m.fleets.push(Fleet { order: order::ATTACK, kind: order::FLEET, tx: 1.0, ..fleet(4, 3.0) });
        m.fleets.push(Fleet { story_id: NO_WARNING_ID, order: order::ATTACK, kind: order::FLEET, tx: 1.0, ..fleet(5, 4.0) });
        m.fleets.push(fleet(9, 500.0)); // a wormhole end, far away
        m.wormholes = vec![Wormhole::default(), Wormhole { revealed_a: true, fleet_a: 4, ..Default::default() }];
        let mut ev = Vec::new();
        m.refresh(&[ship(1, 2, 0)], &c, &mut ev);
        assert_eq!(m.fleets[4].visible[1], 1, "a revealed wormhole end stays visible");
        assert!(ev.contains(&StarmapEvent::EnemyApproaching { fleet: 2 }));
        assert!(!ev.contains(&StarmapEvent::EnemyApproaching { fleet: 3 }), "story id 0x1F88 is exempt");
        ev.clear();
        m.refresh(&[ship(1, 2, 0)], &c, &mut ev);
        assert!(!ev.iter().any(|e| matches!(e, StarmapEvent::EnemyApproaching { .. })), "only when newly seen");
    }

    #[test]
    fn an_approaching_enemy_slows_the_same_ticks_movement() {
        let run = |locked: bool| -> (i32, f32, Vec<StarmapEvent>) {
            let mut c = StarmapContext { story_no_supply: true, speed_mult: 16, speed_locked: locked, ..ctx() };
            let mut m = map();
            m.fleets.push(fleet(1, 0.0)); // the human's fleet, radar 10
            m.fleets.push(Fleet { order: order::ATTACK, kind: order::FLEET, tx: 1.0, ..fleet(4, 6.0) });
            let ev = m.tick(&[ship(1, 2, 0), ship(2, 0, 0)], &mut c, &mut crate::rng::Streams::default(), &mut ());
            assert!(ev.contains(&StarmapEvent::EnemyApproaching { fleet: 2 }));
            (c.speed_mult, m.fleets[2].x, ev)
        };
        // The attacker can't see its target, so upkeep turns the attack into a move to its position.
        let step = (1.0f64 * 0.3) as f32;
        let (mult, x, _) = run(false);
        assert_eq!(mult, 1, "SetIdoLelassit: SP back to x1");
        assert_eq!(x, (-6.0 * step) / 6.0 + 6.0, "one x1 step, not x16");
        let (mult, x, _) = run(true);
        assert_eq!(mult, 16, "the speed lock keeps the clock");
        assert_eq!(x, (-6.0 * (16.0 * step)) / 6.0 + 6.0);
    }

    #[test]
    fn allies_share_planet_radar_and_radar_stations_show_the_human() {
        let mut c = ctx();
        c.relations[2][3] = (0, 5);
        let mut m = map();
        m.radar.refresh(&c.radar_rules, &c);
        m.planets.push(StarPlanet { owner: 2, radar_level: 4, ..Default::default() }); // station of AI 2
        m.planets.push(StarPlanet { owner: 0, x: 15.0, notice: 1, ..Default::default() });
        m.fleets.push(fleet(5, 12.0));
        let mut ev = Vec::new();
        m.refresh(&[], &c, &mut ev);
        assert_eq!(m.fleets[1].visible[..6], [0, 1, 1, 1, 0, 1], "owner, both allies and the human");
        assert_eq!(m.planets[2].visible[..4], [0, 1, 1, 1]);
        assert!(ev.contains(&StarmapEvent::PlanetSighted { planet: 2 }));
        assert_eq!(m.planets[2].notice, 0);
    }
}
