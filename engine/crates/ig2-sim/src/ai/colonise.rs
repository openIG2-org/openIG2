//! `FRISSIT_MI_KOLONIZALAS(e)` (exe 0x50d9a0): colony ships to the nearest free planets, and the colony-ship
//! designs once there are none left (`re/rules/ai-empire.md` §9, §12.4, §12.5).

use super::{category, AiHooks, AiState, World, BEST};
use crate::colony::Planet;
use crate::fleet::{flag, order, DELETED};
use crate::spacewar::NO_DESIGN;

/// The terraformer category (`MiGyarthatoCuccok[e][2]`).
const TERRAFORMER: usize = 2;
/// Planet event: the AI must not colonise this planet (`BolygoInfoIs(p, −0x2094)`).
const NO_AI_COLONY: i16 = -0x2094;
/// `Ev` / `Honap` of the scenario start (1 March 2327): everyone looks for planets.
const START_YEAR: i32 = 0x917;
const START_MONTH: i32 = 3;
/// The name a colony fleet gets.
const COLONY_FLEET: &[u8] = b"Colonization";

/// `getMiAltalElfoglalhato(p)`: owner below 9 and not AI-proof.
pub(crate) fn capturable(p: &Planet) -> bool {
    p.owner < 9 && !p.ai_proof
}

impl World<'_> {
    /// `FindNearestKolonizalo(x, y, e)` (§12.4): `e`'s fleet with colony ships, not colonising, nearest by
    /// `(int)(|dx|·2 + |dy|)`.
    pub(crate) fn nearest_colony_fleet(&self, x: i32, y: i32, e: usize) -> usize {
        let (mut best, mut pick) = (99_999_999, 0);
        for (f, r) in self.starmap.fleets.iter().enumerate().skip(1) {
            if r.owner == DELETED || r.owner as usize != e || r.colony_ships < 1 || r.flags & flag::COLONISING != 0 {
                continue;
            }
            let dx = ((r.x - x as f32) as f64).abs() as f32;
            let dy = ((r.y - y as f32) as f64).abs() as f32;
            let d = (dx * 2.0 + dy) as i32;
            if d < best {
                best = d;
                pick = f;
            }
        }
        pick
    }

    /// `IndultEKolonizalo(e, p)` (§12.4): some fleet of `e` is on its way to colonise or terraform planet `p`.
    pub(crate) fn colony_ship_heading(&self, e: usize, p: usize) -> bool {
        self.starmap.fleets.iter().skip(1).any(|r| {
            r.owner as usize == e
                && r.tx == p as f32
                && r.order == order::MOVE
                && r.kind == order::PLANET
                && r.flags & (flag::COLONISING | flag::TERRAFORMING) != 0
        })
    }

    /// `DeleteKolonizalo(f, p)` (§12.5): send one colony ship of fleet `f` to planet `p` (the whole fleet when it
    /// has one ship, else a new fleet). Returns the fleet sent (0 when `f` has no colony ship).
    pub(crate) fn send_colony_ship(&mut self, f: usize, p: usize, hooks: &mut dyn AiHooks) -> usize {
        let owner = self.starmap.fleets[f].owner;
        let Some(i) = (1..self.spy.combat.ships.len()).find(|&i| {
            let s = self.spy.combat.ships[i];
            s.design != NO_DESIGN && s.fleet as usize == f && s.fleet >= 0 && self.des(owner as usize, s.design as i32).is_some_and(|d| d.raw[0x34] != 0)
        }) else {
            return 0;
        };
        let g = if self.totals(f as i32).ships < 2 {
            let r = &mut self.starmap.fleets[f];
            r.name[..COLONY_FLEET.len()].copy_from_slice(COLONY_FLEET);
            r.name[COLONY_FLEET.len()] = 0;
            f
        } else {
            let (x, y) = (self.starmap.fleets[f].x, self.starmap.fleets[f].y);
            let g = self.new_fleet(COLONY_FLEET, owner, 0, x, y, 0.0, hooks);
            self.move_ship(i, g);
            g
        };
        self.order(g, order::MOVE, order::PLANET, p as f32, 0.0);
        self.starmap.fleets[g].flags |= flag::COLONISING;
        self.mirror(g);
        let o = self.starmap.fleets[g].owner as usize % super::EMPIRES;
        self.spy.combat.colony_ship_pool[o] -= 1;
        if g != f && self.totals(f as i32).ships < 1 {
            self.delete_fleet(f);
        }
        g
    }

    /// `FindNearestTerraformalo(x, y, e)` (exe 0x53ceb0): as [`World::nearest_colony_fleet`] for terraformers
    /// (`+0x4e > 0`, not terraforming).
    pub(crate) fn nearest_terraformer_fleet(&self, x: i32, y: i32, e: usize) -> usize {
        let (mut best, mut pick) = (99_999_999, 0);
        for (f, r) in self.starmap.fleets.iter().enumerate().skip(1) {
            if r.owner == DELETED || r.owner as usize != e || r.terraformers < 1 || r.flags & flag::TERRAFORMING != 0 {
                continue;
            }
            let dx = ((r.x - x as f32) as f64).abs() as f32;
            let dy = ((r.y - y as f32) as f64).abs() as f32;
            let d = (dx * 2.0 + dy) as i32;
            if d < best {
                best = d;
                pick = f;
            }
        }
        pick
    }

    /// `DeleteTerraformalo(f, p)` (exe 0x53c560): the first terraformer of fleet `f` leaves in a new fleet
    /// "Terraformer" for planet `p` (always a new fleet, and `f` stays even when emptied, unlike
    /// [`World::send_colony_ship`]). Returns the new fleet (0 when `f` has no terraformer).
    pub(crate) fn send_terraformer(&mut self, f: usize, p: usize, hooks: &mut dyn AiHooks) -> usize {
        let owner = self.starmap.fleets[f].owner;
        let Some(i) = (1..self.spy.combat.ships.len()).find(|&i| {
            let s = self.spy.combat.ships[i];
            s.design != NO_DESIGN && s.fleet as usize == f && s.fleet >= 0 && self.des(owner as usize, s.design as i32).is_some_and(|d| d.raw[0x35] != 0)
        }) else {
            return 0;
        };
        let (x, y) = (self.starmap.fleets[f].x, self.starmap.fleets[f].y);
        let g = self.new_fleet(b"Terraformer", owner, 0, x, y, 0.0, hooks);
        self.move_ship(i, g);
        self.order(g, order::MOVE, order::PLANET, p as f32, 0.0);
        self.starmap.fleets[g].flags |= flag::TERRAFORMING;
        self.mirror(g);
        let o = self.starmap.fleets[g].owner as usize % super::EMPIRES;
        self.spy.combat.terraformer_pool[o] -= 1;
        g
    }
}

impl AiState {
    /// `FRISSIT_MI_KOLONIZALAS(e)` (§9).
    pub fn colonise(&mut self, w: &mut World, e: usize, hooks: &mut dyn AiHooks) {
        let mp = w.spy.ctx.multiplayer;
        let colony_ships = |a: &AiState| a.categories[e][category::COLONY_SHIP].units[BEST];
        let start = w.ctx.year == START_YEAR && w.ctx.month == START_MONTH;
        if colony_ships(self) > 0 || start || (self.empty_planets[e] == 0 && w.spy.ctx.day == 10) {
            let (cx, cy) = (self.centre_x[e] as f32, self.centre_y[e] as f32);
            let (mut pick, mut best) = (0usize, 1e9f32);
            self.empty_planets[e] = 0;
            for p in 1..w.spy.colony.planets.len() {
                let pl = &w.spy.colony.planets[p];
                if !((mp || capturable(pl)) && pl.owner == 0 && pl.visibility[e] >= 0) {
                    continue;
                }
                if pl.visibility[e] > 0 {
                    if p as i32 != self.last_bad_colony[e] {
                        let (dy, dx) = (cy - pl.y, cx - pl.x);
                        let mut d = dy * dy + dx * dx;
                        if !mp {
                            let r = w.r(0, 10);
                            d = ((r as f64 * 0.1 + 1.0) * d as f64) as f32;
                        }
                        let pl = &w.spy.colony.planets[p];
                        if (d < best || pick == 0) && !w.colony_ship_heading(e, p) && (!pl.events.contains(&NO_AI_COLONY) || mp) {
                            pick = p;
                            best = d;
                        }
                    }
                    self.empty_planets[e] = 1;
                }
                if w.planet_count[e] < 6 {
                    self.empty_planets[e] = 1;
                }
            }
            if pick > 0 && colony_ships(self) > 0 {
                let (px, py) = (w.spy.colony.planets[pick].x, w.spy.colony.planets[pick].y);
                let f = w.nearest_colony_fleet(px as i32, py as i32, e);
                if f > 0 && self.flee(w, f, 0, 0) == 0 {
                    let seen = w.starmap.fleets[f].visible;
                    let g = w.send_colony_ship(f, pick, hooks);
                    w.starmap.fleets[g].visible = seen;
                    w.mirror(g);
                    if g != f && w.starmap.fleets[f].owner != DELETED && w.danger_at(g, px, py, 10.0).0 != 0 {
                        w.merge(f, g);
                    }
                }
            }
        }
        let (c2, c3) = (self.categories[e][TERRAFORMER], self.categories[e][category::COLONY_SHIP]);
        if self.empty_planets[e] == 0 && (c2.units[BEST] > 0 || c2.queued[BEST] > 0 || c3.units[BEST] > 0 || c3.queued[BEST] > 0) {
            w.delete_design(e, c2.design[BEST] as i32, 0, false);
            w.delete_design(e, c3.design[BEST] as i32, 0, false);
            self.design(w, e, false, hooks);
        }
    }
}
