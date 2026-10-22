//! The design orders' unit walks (`re/rules/design-orders.md` §6): how many units of a design (or upgradable to it)
//! a holder has and what they cost (`HanyHajoVanATervbol`, `TervHajoinakAra`), and the refits
//! (`TervAzOsszesHajotAtalalitja`, `TervAzOsszesHajorolAtalalitja`). Holder `h`: 0 = every holder of the empire, a
//! fleet, or planet p as −p; stations count only for `h == 0`.

use super::World;
use crate::fleet::DELETED;
use crate::spacewar::{Design, NO_DESIGN};

impl World<'_> {
    fn ship_owner(&self, fleet: i16) -> u8 {
        self.rec(fleet as i32).map_or(DELETED, |r| r.owner)
    }

    /// `HanyHajoVanATervbol(e, h, d)` (exe 0x4b4410) and `TervHajoinakAra(e, h, d)` (exe 0x4b70b0): the units of
    /// `e` held by `h` whose design is `d` or upgrades to it (`+0x1c == d`), and the sum of their designs' prices.
    pub(crate) fn design_units(&self, e: usize, h: i32, d: i32) -> (i32, i32) {
        let of = |x: i16| x as i32 == d || self.des_i32(e, x as i32, 0x1c) == d;
        let price = |x: i16| self.des_i32(e, x as i32, 0x24);
        let (mut n, mut sum) = (0i32, 0i32);
        for s in self.spy.combat.ships.iter().skip(1) {
            if s.design != NO_DESIGN && self.ship_owner(s.fleet) as usize == e && of(s.design) && (h == 0 || s.fleet as i32 == h) {
                n = n.wrapping_add(1);
                sum = sum.wrapping_add(price(s.design));
            }
        }
        for v in self.spy.combat.vehicles.iter().skip(1) {
            if v.design != NO_DESIGN && self.holder_owner(v.fleet as i32) as usize == e && of(v.design) && (h == 0 || v.fleet as i32 == h) {
                n = n.wrapping_add(v.count);
                sum = sum.wrapping_add(price(v.design).wrapping_mul(v.count));
            }
        }
        if h == 0 {
            for p in self.spy.colony.planets.iter().skip(1).filter(|p| p.owner as usize == e) {
                for &x in p.stations.iter().take(p.field_114.max(0) as usize) {
                    if of(x) {
                        n = n.wrapping_add(1);
                        sum = sum.wrapping_add(price(x));
                    }
                }
            }
        }
        (n, sum)
    }

    /// `TervAzOsszesHajotAtalalitja(e, h, d)` (exe 0x4b6ee0): the units whose design upgrades to `d` take `d`
    /// (stations only for `h == 0`), then `FlottakUjraSzamol()`.
    pub(crate) fn upgrade_units_to(&mut self, e: usize, h: i32, d: i32) {
        let up = |w: &Self, x: i16| w.des_i32(e, x as i32, 0x1c) == d;
        for i in 1..self.spy.combat.ships.len() {
            let s = self.spy.combat.ships[i];
            if s.design != NO_DESIGN && self.ship_owner(s.fleet) as usize == e && up(self, s.design) && (h == 0 || s.fleet as i32 == h) {
                self.spy.combat.ships[i].design = d as i16;
            }
        }
        for i in 1..self.spy.combat.vehicles.len() {
            let v = self.spy.combat.vehicles[i];
            if v.design != NO_DESIGN && self.holder_owner(v.fleet as i32) as usize == e && up(self, v.design) && (h == 0 || v.fleet as i32 == h) {
                self.spy.combat.vehicles[i].design = d as i16;
            }
        }
        if h == 0 {
            for p in 1..self.spy.colony.planets.len() {
                if self.spy.colony.planets[p].owner as usize != e {
                    continue;
                }
                let n = self.spy.colony.planets[p].field_114.max(0) as usize;
                let mut st = self.spy.colony.planets[p].stations;
                for x in st.iter_mut().take(n) {
                    if up(self, *x) {
                        *x = d as i16;
                    }
                }
                self.spy.colony.planets[p].stations = st;
            }
        }
        self.recount();
    }

    /// `TervAzOsszesHajorolAtalalitja(e, h, d)` (exe 0x4b6d60): with an upgrade target, `e`'s ships and vehicles of
    /// design `d` held by `h` take it, then `FlottakUjraSzamol()`. Returns the target when a planet's (`h < 0`)
    /// vehicles changed: its battlefield's units change too (exe 0x4a78d0, the caller's part).
    pub(crate) fn refit_units(&mut self, e: usize, h: i32, d: i32) -> Option<i32> {
        let to = self.des_i32(e, d, 0x1c);
        if to == 0 {
            return None;
        }
        for i in 1..self.spy.combat.ships.len() {
            let s = self.spy.combat.ships[i];
            if s.design as i32 == d && self.ship_owner(s.fleet) as usize == e && (h == 0 || s.fleet as i32 == h) {
                self.spy.combat.ships[i].design = to as i16;
            }
        }
        let mut field = false;
        for i in 1..self.spy.combat.vehicles.len() {
            let v = self.spy.combat.vehicles[i];
            if v.design as i32 == d && self.holder_owner(v.fleet as i32) as usize == e && (h == 0 || v.fleet as i32 == h) {
                field |= h < 0;
                self.spy.combat.vehicles[i].design = to as i16;
            }
        }
        self.recount();
        field.then_some(to)
    }

    /// `TervCopy2(e, rec, d)` (exe 0x541ca0): design `d` of `e` becomes `rec`.
    pub(crate) fn copy_design(&mut self, e: usize, d: usize, rec: &[u8; 0x84]) {
        if let Some(x) = self.spy.combat.designs.get_mut(e).and_then(|t| t.get_mut(d)) {
            *x = Design::from_bytes(rec);
        }
    }

    /// `MULTIGET_Design` 0x33's queue pass (re/rules/design-orders.md §2): on every planet of `e`, a line whose head
    /// is design `d` or upgrades to `bc` keeps its progress in proportion to the price change (`old` → `new`, SSE
    /// f32); with `copy` every such word of the line becomes `bc`.
    pub(crate) fn rescale_queues(&mut self, e: usize, d: i32, bc: i32, old: f32, new: f32, copy: bool) {
        let c = self.des_i32(e, d, 0x20);
        for p in 1..self.spy.colony.planets.len() {
            if self.spy.colony.planets[p].owner as usize != e {
                continue;
            }
            for line in 0..2 {
                let q = self.spy.colony.planets[p].production;
                let word = |k: usize| i16::from_le_bytes([q[8 * line + 2 * k], q[8 * line + 2 * k + 1]]);
                let hit = |x: i16| x as i32 == d || self.des_i32(e, x as i32, 0x1c) == bc;
                let mut q2 = q;
                if hit(word(0)) {
                    let at = 0x28 + 4 * line;
                    let prog = i32::from_le_bytes(q[at..at + 4].try_into().unwrap());
                    let (cf, x) = (c as f32, c as f32 - (c.wrapping_sub(prog) as f32 * old) / new);
                    // exe 0x4b522a..0x4b5249: NaN passes both compares (and truncates to i32::MIN).
                    let m = if x < 0.0 { 0.0 } else { x };
                    let r = if m > cf { cf } else { m };
                    q2[at..at + 4].copy_from_slice(&super::cvtt(r as f64).to_le_bytes());
                }
                if copy {
                    for k in 0..4 {
                        if hit(word(k)) {
                            q2[8 * line + 2 * k..8 * line + 2 * k + 2].copy_from_slice(&(bc as i16).to_le_bytes());
                        }
                    }
                }
                self.spy.colony.planets[p].production = q2;
            }
        }
    }
}
