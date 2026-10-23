//! The players' ground-war orders (`MULTIGET_Colony` exe 0x48a270, ELF 0x4ec2e0, re/screens/colony.md §9): the
//! messages the colony screen sends for the selected units while a real-time ground war runs, applied on every
//! peer. Unit `i` is the battlefield's slot (`WF+0x330`), `e` the sending empire.

use super::*;

/// One ground-war message's payload after the issuer and the planet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundOrder {
    /// 0x22: drive to cell `(x, y)` (the waypoints go, `MoveG`); a unit with flag 0x80000000 ignores it.
    Move { unit: i32, x: i32, y: i32 },
    /// 0x23: queue cell `(x, y)` (shift-click; `WT_WayPoint::Add`, any cell, no limit).
    Waypoint { unit: i32, x: i32, y: i32 },
    /// 0x24: attack `target` (a unit, or `−(slot)` a building; the waypoints go, `SetTarget`).
    Target { unit: i32, target: i32 },
    /// 0x25: stop (`WF_Tank::Stop`, inlined; the waypoints stay).
    Stop { unit: i32 },
    /// 0x12d: blow up (`WF_Tank::Detonate`; `big` 0: the kamikaze pod, weapon 0x40; 1: the big one, 0x80).
    Detonate { unit: i32, big: i32 },
    /// 0x12f: the issuer's side retreats, or calls the retreat off (key R; the landing zone is freed).
    Retreat,
    /// 0x130: an air strike on cell `(x, y)` (`kind` 0 fighters, 1 big ships; `ShotSpecWeapon`).
    AirStrike { x: i32, y: i32, kind: i32 },
    /// 0x134: mine laying on or off (`+0x250`; the message carries one byte).
    Mines { unit: i32, on: bool },
    /// Ours (no packet in the original: the colony screen's mouse handler, exe 0x47b350, calls `WF::PlaceTank`
    /// directly): the issuer's first unit still on board lands on cell `(x, y)`.
    Place { x: i32, y: i32 },
    /// Ours (`WF::UnplaceTank`, called the same way): the issuer's unit `unit` is picked up again.
    Unplace { unit: i32 },
    /// Ours (`WF::PlaceAllTanks(x, y, e, false)`, the right click): every unit of the issuer still on board lands
    /// near `(x, y)`.
    PlaceAll { x: i32, y: i32 },
}

/// What a ground-war order did that the colony screen reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundOrderEvent {
    /// 0x12f on the shown planet: `ColonySpeaker.Play(1, 3)` for the local player's own retreat, `(1, 10)` for
    /// another's.
    Retreat { empire: i32 },
    /// 0x12d: `Blast07.wav + rand() % 5` (on the shown planet outside `Simulate`), once for the unit and once per
    /// unit or building it took with it.
    Blast { variant: i32 },
}

impl War<'_> {
    /// `MULTIGET_Colony`'s handling of a ground-war message from empire `e`. The exe reads the unit table at any
    /// index (a dead or missing slot's stale pointer): such a unit is refused, as are an issuer outside 1..=18 and
    /// an air-strike kind other than 0/1 (they would write another empire's timer).
    pub fn order(&mut self, e: i32, o: GroundOrder) {
        let live = |w: &Self, i: i32| i >= 0 && (i as usize) < w.n() && w.alive(i);
        match o {
            GroundOrder::Move { unit: i, x, y } => {
                if live(self, i) && self.u(i).flags & 0x8000_0000 == 0 {
                    self.free_waypoints(i);
                    self.move_g(i, x, y);
                }
            }
            GroundOrder::Waypoint { unit: i, x, y } => {
                if live(self, i) {
                    self.um(i).rt.waypoints.push((x, y));
                }
            }
            GroundOrder::Target { unit: i, target: t } => {
                if (t < 0 || live(self, t)) && live(self, i) {
                    self.free_waypoints(i);
                    self.set_target(i, t);
                }
            }
            GroundOrder::Stop { unit: i } => {
                if live(self, i) {
                    self.stop_orders(i);
                }
            }
            GroundOrder::Detonate { unit: i, big } => {
                if live(self, i) {
                    let planet = self.u(i).rt.planet;
                    self.detonate(i, big, &mut || planet);
                }
            }
            GroundOrder::Retreat => {
                if !(1..=18).contains(&e) {
                    return;
                }
                // `LandingZone::Free` (and its outline, `FUN_00477d40`): generated again at the next landing.
                self.field.landing = None;
                // `ColonySpeaker.Play(1, 3 | 10)` direct (no `Simulate` test): a picked line costs a libc draw.
                self.events.push(GroundEvent::Voice(Voice::Retreat { empire: e }));
                if self.ctx.on_screen {
                    let (kind, en) = (if e == self.ctx.human { 3 } else { 10 }, self.ctx.human);
                    if self.voices.as_deref_mut().is_some_and(|v| v.play(1, kind, en)) {
                        (self.libc)();
                    }
                    self.events.push(GroundEvent::Order(GroundOrderEvent::Retreat { empire: e }));
                }
                // The exe flips the flag on the battlefield of `AktPlanet`, the sender's screen [original bug: a peer
                // not looking at `p` flips another planet's]; in a war every machine shows `p` (`StartGrWar`).
                let r = &mut self.field.retreat[(e - 1) as usize];
                *r = (*r == 0) as u8;
            }
            GroundOrder::AirStrike { x, y, kind } => {
                if (1..=18).contains(&e) && matches!(kind, 0 | 1) {
                    // The handler zeroes the charge timer again after `ShotSpecWeapon` (the same cell).
                    self.shot_spec_weapon(x, y, kind, e);
                }
            }
            GroundOrder::Mines { unit: i, on } => {
                if live(self, i) {
                    self.um(i).rt.laying = on as u8;
                }
            }
            // The screen calls these for EN only; a peer's order is refused for another side's unit.
            GroundOrder::Place { x, y } => self.place_tank(x, y, e),
            GroundOrder::Unplace { unit: i } => {
                if live(self, i) && self.u(i).owner == e {
                    self.unplace_tank(i);
                }
            }
            GroundOrder::PlaceAll { x, y } => self.place_all_tanks(x, y, e, false),
        }
    }

    /// `WT_WayPoint::Free` (inlined): the list and its cursor go.
    fn free_waypoints(&mut self, i: i32) {
        let rt = &mut self.um(i).rt;
        rt.waypoints.clear();
        rt.wp_cursor = 0;
    }

    /// The blast sound's draw: one libc `rand()`, played on the shown planet outside `Simulate`.
    fn blast_sound(&mut self) {
        let r = (self.libc)();
        if self.ctx.on_screen && !self.field.simulating {
            self.events.push(GroundEvent::Order(GroundOrderEvent::Blast { variant: r % 5 }));
        }
    }

    /// `WF_Tank::Detonate(big, wf, i)` (exe 0x46dc40, ELF 0x4a0130): `k = big + 1`; `100k` particles (seven libc
    /// draws each), on screen a flash and 24 pieces of debris (velocities `/ 1.2`), the sound, the scorch mark
    /// (`Koszfolt`, render only); the group goes. Unless it was a partner: every placed, counted unit within 6
    /// cells takes `(int)(80000k − 80000k·d / 6)` (a dead group bursts on screen and goes), every building within
    /// `w/2 + 6` cells `(int)(10000k − 10000k·d / (w/2 + 6))` (`WF::Damage(−slot)`); distances between int cells,
    /// `(f32)sqrt`.
    ///
    /// A dead unit's burst tests "on screen" with the detonator's `+0x280` (its planet) after `RemoveTankGroup`
    /// freed the record [original bug]: when the heap has reused the block meanwhile (debris objects), the test
    /// fails and the burst (and its draws) is skipped. The port reads the record as left (`freed_planet` gives the
    /// unit's planet); a replay gives what the exe read at each test (exe 0x46f49d).
    pub fn detonate(&mut self, i: i32, big: i32, freed_planet: &mut dyn FnMut() -> i32) {
        let k = big.wrapping_add(1);
        let u = self.u(i);
        let (flags, (cx, cy), p) = (u.flags, u.cell, [u.rt.z, u.pos.0, u.pos.1]);
        for _ in 0..k.wrapping_mul(100).max(0) {
            for _ in 0..7 {
                (self.libc)();
            }
        }
        if self.ctx.on_screen {
            (self.libc)();
            for _ in 0..24 {
                let a = (self.libc)();
                let b = (self.libc)();
                let c = (self.libc)();
                let vy = ((a % 0x3c) as f32 / 10.0 - 3.0) / 1.2;
                let vx = ((b % 0x3c) as f32 / 10.0 - 3.0) / 1.2;
                let vh = ((c % 0x1e) as f32 / 10.0 - 4.0) / 1.2;
                self.add_debris(p, [vh, vx, vy]);
            }
        }
        self.blast_sound();
        self.remove_group(i);
        self.remove_target(i);
        if flags & flag::PARTNER != 0 {
            return;
        }
        // `(f32)sqrt((f64)(f32)(dx² + dy²))`; the fall-off in f64 from the f32 product, rounded to f32, truncated.
        let dist = |dx: i32, dy: i32| ((dx * dx + dy * dy) as f32 as f64).sqrt() as f32;
        let fall = |full: i32, d: f32, r: f64| (full as f64 - (full as f32 * d) as f64 / r) as f32 as i32;
        let full = k.wrapping_mul(80000);
        let mut j = 0;
        while (j as usize) < self.n() {
            if !self.alive(j) {
                j += 1;
                continue;
            }
            let v = self.u(j);
            if v.flags & (flag::NOT_PLACED | flag::NOT_COUNTED) == 0 && v.flags2 & 4 == 0 {
                let d = dist(cx - v.cell.0, cy - v.cell.1);
                if d <= 6.0 && self.unit_damage(j, fall(full, d, 6.0), NONE) {
                    let v = self.u(j);
                    if v.flags & flag::PARTNER == 0 && freed_planet() == self.planet.index && self.ctx.on_screen {
                        let at = [v.rt.z, v.pos.0, v.pos.1];
                        self.burst(at, 24, true);
                    }
                    self.blast_sound();
                    self.remove_group(j);
                    self.remove_target(j);
                }
            }
            j += 1;
        }
        let full = k.wrapping_mul(10000);
        let mut slot = 1;
        while slot < self.planet.buildings.len() {
            let b = self.planet.buildings[slot];
            if b.kind != 0 {
                let half = self.footprint_w(b.kind) / 2;
                let d = dist(cx - b.x as i32, cy - b.y as i32);
                if d <= (half + 6) as f32 {
                    self.damage(-(slot as i32), fall(full, d, half as f64 + 6.0), NONE);
                    self.blast_sound();
                }
            }
            slot += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn blast_damage_falls_off_in_f32() {
        // k = 1 at 6 cells: nothing left; at sqrt(5) = 2.236068: 80000 − 80000·2.236068/6 = 50185.8 → 50185.
        let f = |k: i32, d: f32| ((k * 80000) as f64 - ((k * 80000) as f32 * d) as f64 / 6.0) as f32 as i32;
        assert_eq!(f(1, 6.0), 0);
        assert_eq!(f(1, (5.0f64).sqrt() as f32), 50185);
        assert_eq!(f(2, 0.0), 160000);
    }
}
