//! The transporters of the real-time ground war (spec `re/rules/ground-combat.md` §11.16): every unit's
//! dropship (`WF_Hordozo`, the unit's `+0x264..+0x27c`) and the landing orders.
//!
//! - [`War::dropships`] = the dropship part of `WF::Refresh2` (exe 0x48ce10): `WF_Hordozo::Refresh2`
//!   (exe 0x48e0a0, ELF 0x48bb70) for every unit without flag 2 whose dropship isn't away. The original runs
//!   it from the 100 Hz colony UI tick (0x477dc0) when `(int)ColonyCounter` (f32 at 0x99823c, +1 per UI
//!   tick) is a multiple of 5, for **every** planet, on or off screen; the caller keeps that cadence.
//! - [`War::place_all_tanks`] / [`War::place_tank`] / [`War::unplace_tank`] (exe 0x48c150 / 0x48c2c0 /
//!   0x4a4f70): the human's landing orders from the colony screen's mouse handler (0x47b350).
//!
//! A dropship descends from its height `+0x26c` (−1650..−1151 below the unit) by `max(2, √−h)` per pass,
//! drifting a third of that on both map axes, then acts on its order bits and climbs back by the same rule;
//! below −1400 it is away again with a fresh random height. Its draws: stream 2 for the retreat pick-up's
//! survival (line 0x32b1), stream 0 for the climb's exhaust particle and the next height (cosmetic but
//! seeded), libc for a shot-down pick-up's wreck (debris in list B).
//!
//! The transporter pod (0x9f, flag 0x400000) only gives the unit a partner (§3.1): a one-hit-point
//! escort that stays in the dropship (flags 2 and 0x8000000; `WF::Refresh` and the dropships skip it),
//! takes one point whenever its carrier is hit (§5.6) and re-embarks with it (§6.1).

use super::{flag, War};
use crate::placement::{self, SIDE};

/// Dropship state bits (`WF_Tank +0x278`, the `WF_Hordozo` flags `+0x14`).
pub mod drop {
    /// Off the field: no pass (the constructor's state).
    pub const AWAY: u32 = 1;
    pub const ARRIVING: u32 = 2;
    pub const LEAVING: u32 = 4;
    /// The unit rides with it.
    pub const CARRYING: u32 = 8;
    /// Set the unit down and leave (landings: `AddTanksToPlanet`, the placing orders).
    pub const DELIVER: u32 = 0x10;
    /// Take the unit back on board (`UnplaceTank`).
    pub const COLLECT: u32 = 0x20;
    /// A retreat pick-up (`Retreat`): it may be shot down.
    pub const RETREAT: u32 = 0x40;
}

/// `max(2, (f32)√(−h))` (`libm_sse2_sqrt_precise` on the double; a NaN stays NaN).
fn drop_speed(h: f32) -> f32 {
    let v = ((-h) as f64).sqrt() as f32;
    if 2.0 > v {
        2.0
    } else {
        v
    }
}

impl War<'_> {
    /// The dropships' pass of `WF::Refresh2` (exe 0x48ce10; see the module doc for the cadence).
    pub fn dropships(&mut self) {
        let mut i = 0;
        while (i as usize) < self.n() {
            if self.alive(i) && self.u(i).flags & flag::NOT_PLACED == 0 && self.u(i).transport & drop::AWAY == 0 {
                self.dropship(i);
            }
            i += 1;
        }
    }

    /// `WF_Hordozo::Refresh2` (exe 0x48e0a0) of unit `i`'s dropship; true when it was shot down with the unit
    /// (the group is removed).
    pub fn dropship(&mut self, i: i32) -> bool {
        let s = self.u(i).transport;
        if s & drop::AWAY != 0 {
            return false;
        }
        if !self.ctx.multiplayer && s & drop::ARRIVING != 0 {
            let d = &mut self.um(i).rt.drop;
            let v = drop_speed(d[2]);
            let h = v + d[2];
            d[2] = h;
            d[0] += v / 3.0;
            d[1] += v / 3.0;
            if h > 0.0 {
                *d = [0.0; 3];
                // The order bits are tested in this order: deliver, collect, retreat.
                if s & drop::DELIVER != 0 {
                    let u = self.um(i);
                    u.transport = (s & !(drop::AWAY | drop::ARRIVING | drop::CARRYING)) | drop::LEAVING;
                    u.flags &= !flag::LANDED;
                } else if s & drop::RETREAT != 0 && s & drop::COLLECT == 0 {
                    let u = self.um(i);
                    u.transport = (s & !(drop::AWAY | drop::ARRIVING)) | drop::LEAVING | drop::CARRYING;
                    u.flags &= !(flag::PARALYSED | flag::SEIZED);
                    u.owner = u.side;
                    u.flags = (u.flags | flag::NOT_COUNTED) & !8;
                    self.remove_target(i);
                    if self.rng.range(2, 0, 0x13) < 6 {
                        // Shot down: a flash and 16 pieces of the wreck (libc draws, list-B debris).
                        let u = self.u(i);
                        self.burst([u.rt.z, u.pos.0, u.pos.1], 16, true);
                        self.remove_group(i);
                        return true;
                    }
                } else {
                    if s & drop::COLLECT != 0 {
                        self.um(i).transport = (s & !(drop::AWAY | drop::ARRIVING)) | drop::LEAVING | drop::CARRYING;
                        self.remove_target(i);
                        self.um(i).flags |= flag::LANDED;
                    } else {
                        self.remove_target(i);
                        self.um(i).transport |= drop::CARRYING;
                    }
                    let u = self.um(i);
                    u.flags &= !(flag::PARALYSED | flag::SEIZED);
                    u.owner = u.side;
                }
            } else if s & drop::CARRYING != 0 {
                self.um(i).flags |= flag::PLACED;
            }
        }
        let s = self.u(i).transport;
        if s & drop::LEAVING == 0 {
            return false;
        }
        if s & drop::CARRYING != 0 {
            self.um(i).flags &= !8;
        }
        let d = &mut self.um(i).rt.drop;
        let v = drop_speed(d[2]);
        d[2] -= v;
        d[0] += v / 3.0;
        d[1] += v / 3.0;
        // The exhaust particle (lines 0x3300, 0x32fe, 0x32fd, 0x32fc, 0x32fb, 0x32fa: the exe evaluates the
        // arguments last to first).
        for (lo, hi) in [(0, 9), (6, 8), (0, 2), (0, 2), (0, 4), (0, 4)] {
            self.rng.range(0, lo, hi);
        }
        if -1400.0 > self.u(i).rt.drop[2] {
            let u = self.um(i);
            u.transport |= drop::AWAY;
            if u.transport & drop::CARRYING != 0 {
                u.flags |= flag::NOT_PLACED;
            }
            let h = (self.rng.range(0, 0, 499) - 0x672) as f32;
            self.um(i).rt.drop = [h / 3.0, h / 3.0, h];
            self.field.status_valid = false;
        } else if s & drop::CARRYING != 0 {
            self.um(i).flags |= flag::PLACED;
        }
        false
    }

    /// Unit `i`'s dropship comes down to set it on its cell (the shared tail of the placing orders).
    fn send_down(&mut self, i: i32) {
        let u = self.um(i);
        u.flags &= !(flag::NOT_PLACED | flag::LANDED);
        u.transport = ((u.transport & !(drop::AWAY | drop::LEAVING)) | drop::ARRIVING | drop::CARRYING | drop::DELIVER) & !drop::COLLECT;
    }

    /// `WF::PlaceAllTanks(x, y, owner, all)` (exe 0x48c150): every unit of `owner` (not a turret or mine)
    /// whose dropship is away and that waits in it (flag 0x8000000; `all`: any) is placed near `(x, y)`
    /// (clamped to 11..=189; the landing zone closed), else at the default start, and flown in.
    pub fn place_all_tanks(&mut self, x: i32, y: i32, owner: i32, all: bool) {
        let at = (x.clamp(0xb, 0xbd), y.clamp(0xb, 0xbd));
        let mut i = 0;
        while (i as usize) < self.n() {
            let u = self.u(i);
            if self.alive(i)
                && u.owner == owner
                && u.flags & (flag::TURRET | flag::MINE) == 0
                && u.transport & drop::AWAY != 0
                && (all || u.flags & flag::LANDED != 0)
            {
                let (ok, c) = self.locate_tank(Some(at));
                let c = if ok { c } else { self.locate_tank(None).1 };
                self.set_pos(i, c.0, c.1);
                self.send_down(i);
            }
            i += 1;
        }
    }

    /// `WF::PlaceTank(x, y, owner)` (exe 0x48c2c0): the first unit of `owner` waiting in its dropship
    /// (flag 0x8000000, dropship away) lands on exactly `(x, y)`: only when the placement search (landing
    /// zone closed) and the nearest landing cell (`GetNearestLandingZone`) both return that cell. The unit
    /// is moved there even when the second test fails [original quirk].
    pub fn place_tank(&mut self, x: i32, y: i32, owner: i32) {
        let Some(i) = (0..self.n() as i32).find(|&i| {
            let u = self.u(i);
            self.alive(i) && u.flags & flag::LANDED != 0 && u.owner == owner && u.transport & drop::AWAY != 0
        }) else {
            return;
        };
        // A failed search leaves the exe's outputs at (owner, y).
        let (ok, c) = self.locate_tank(Some((x, y)));
        if (if ok { c } else { (owner, y) }) != (x, y) {
            return;
        }
        self.set_pos(i, x, y);
        if self.nearest_landing_cell(i).unwrap_or((x, y)) != (x, y) {
            return;
        }
        self.send_down(i);
    }

    /// `WF::UnplaceTank(i)` (exe 0x4a4f70): a unit whose dropship is away calls it back to pick it up.
    pub fn unplace_tank(&mut self, i: i32) {
        let u = self.um(i);
        let mut s = u.transport;
        if s & drop::AWAY == 0 {
            return;
        }
        if s & drop::ARRIVING == 0 {
            s = ((s & !(drop::AWAY | drop::LEAVING)) | drop::ARRIVING) & !drop::CARRYING;
        } else {
            s = (s & !(drop::AWAY | drop::ARRIVING)) | drop::LEAVING | drop::CARRYING;
        }
        u.transport = (s & !drop::DELIVER) | drop::COLLECT;
        u.flags &= !flag::NOT_PLACED;
    }

    /// `WF_Tank::GetNearestLandingZone` (exe 0x47b980): the spiral from the unit's cell to the first cell the
    /// landing zone leaves open, generating the zone first. Its test (0x4609a0) reads index `y·200 + x`, a row
    /// below where the zone is written [original quirk]. `None` when nothing is open (the exe asserts).
    fn nearest_landing_cell(&mut self, i: i32) -> Option<(i32, i32)> {
        if self.field.landing.is_none() {
            let map = self.map?;
            let buildings: Vec<(u8, f32, f32)> = self.planet.buildings.iter().skip(1).map(|b| (b.kind, b.x, b.y)).collect();
            self.field.landing = Some(placement::landing_zone(&buildings, &map.footprints));
        }
        let z = self.field.landing.as_deref()?;
        placement::locate_pos(self.u(i).cell, 1, 1, |x, y| z[(y * SIDE + x) as usize] != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ground::{GroundContext, GroundPlanet, GroundRules, Field, Group};
    use crate::rng::Streams;
    use crate::spacewar::CombatWorld;

    struct Fx {
        field: Field,
        planet: GroundPlanet,
        world: CombatWorld,
        rules: GroundRules,
        ctx: GroundContext,
        rng: Streams,
    }

    fn fx(units: Vec<Group>) -> Fx {
        let field = Field { alive: vec![true; units.len()], units, ..Default::default() };
        Fx {
            field,
            planet: GroundPlanet::default(),
            world: CombatWorld::default(),
            rules: GroundRules::default(),
            ctx: GroundContext::default(),
            rng: Streams::new(21),
        }
    }

    fn run<R>(x: &mut Fx, f: impl FnOnce(&mut War) -> R) -> R {
        let mut libc = || 0;
        let mut place = |_: Option<(i32, i32)>| (true, (50, 60));
        let mut w = War::new(&mut x.field, &mut x.planet, &mut x.world, &x.rules, &x.ctx, &mut x.rng, &mut libc, &mut place);
        f(&mut w)
    }

    fn unit(transport: u32, h: f32) -> Group {
        let mut u = Group { transport, owner: 3, side: 3, ..Default::default() };
        u.rt.drop = [h / 3.0, h / 3.0, h];
        u
    }

    #[test]
    fn a_landing_delivers_and_flies_away() {
        // The AI's landing: arriving with the unit, deliver.
        let mut x = fx(vec![unit(0x1a, -1400.0)]);
        x.field.units[0].flags |= flag::LANDED;
        let mut passes = 0;
        while x.field.units[0].transport & drop::ARRIVING != 0 {
            assert!(x.field.units[0].flags & flag::LANDED != 0, "can't move before touchdown");
            run(&mut x, |w| w.dropship(0));
            passes += 1;
        }
        let u = &x.field.units[0];
        assert_eq!(u.transport, 0x14, "leaving, empty");
        assert_eq!(u.flags & flag::LANDED, 0);
        assert!(passes > 20 && passes < 100, "{passes}");
        while x.field.units[0].transport & drop::AWAY == 0 {
            run(&mut x, |w| w.dropship(0));
        }
        let u = &x.field.units[0];
        assert_eq!(u.transport, 0x15);
        assert_eq!(u.flags & flag::NOT_PLACED, 0, "the unit stays");
        assert!((-1650.0..=-1151.0).contains(&u.rt.drop[2]));
        assert_eq!(u.rt.drop[0], u.rt.drop[2] / 3.0);
    }

    #[test]
    fn a_retreat_takes_the_unit_or_loses_it() {
        let (mut taken, mut lost) = (0, 0);
        for seed in 0..40u32 {
            let mut x = fx(vec![unit(0x42, -1.0)]);
            x.rng.set_state(2, seed.wrapping_mul(0x9e3779b9));
            x.field.units[0].owner = 5; // seized
            x.field.units[0].flags |= flag::SEIZED;
            if run(&mut x, |w| w.dropship(0)) {
                assert!(!x.field.alive[0]);
                lost += 1;
                continue;
            }
            let u = &x.field.units[0];
            assert_eq!(u.transport & (drop::LEAVING | drop::CARRYING), drop::LEAVING | drop::CARRYING);
            assert_eq!(u.owner, 3);
            assert_eq!(u.flags & (flag::SEIZED | flag::NOT_COUNTED), flag::NOT_COUNTED);
            while x.field.units[0].transport & drop::AWAY == 0 {
                run(&mut x, |w| w.dropship(0));
            }
            assert!(x.field.units[0].flags & flag::NOT_PLACED != 0, "gone with the dropship");
            taken += 1;
        }
        assert!(lost > 0 && taken > lost, "{taken} {lost}");
    }

    #[test]
    fn orders_send_the_dropship_down() {
        let mut waiting = unit(drop::AWAY, -1500.0);
        waiting.flags |= flag::LANDED | flag::NOT_PLACED;
        let mut x = fx(vec![waiting.clone(), waiting]);
        x.field.units[1].owner = 4;
        run(&mut x, |w| w.place_all_tanks(5, 300, 3, false));
        let u = &x.field.units[0];
        assert_eq!(u.cell, (50, 60), "the placement hook's cell");
        assert_eq!(u.transport, 0x1a);
        assert_eq!(u.flags & (flag::LANDED | flag::NOT_PLACED), 0);
        assert_eq!(x.field.units[1].transport, drop::AWAY, "another owner's");
        // Picked up again once its dropship is away.
        x.field.units[0].transport = 0x15;
        run(&mut x, |w| w.unplace_tank(0));
        assert_eq!(x.field.units[0].transport, 0x22);
    }
}
