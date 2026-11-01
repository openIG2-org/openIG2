//! The buildings' smoke (re/screens/colony.md §3.5): `SetBuildingSpecialEffects` (exe 0x49a760) and the chimneys'
//! `KemenyFust::Refresh` (exe 0x490240) add puffs to the planet's particle owner 1 (`rcloud.bmp`) every fifth UI tick
//! (`(int)ColonyCounter % 5 == 0`), and `WF::Refresh2` (exe 0x48ce10) ages them on the same ticks; `ParticleOwner::Draw`
//! adds them as screen-aligned quads in `c·(255 − a) >> 8`. The random draws come from tables filled from a
//! wall-clock seed (`GenerateSpecFXRandoms` exe 0x47a240), so any source does; [`crate::colony_weather::CRand`] here.
//!
//! The ground war's share (§5.7–§5.10) rides on the same clocks: the damaged units' smoke and the particles the sim
//! reports (owner 1), the debris' flight on the second UI ticks, the ruins (`Rom`, `RomTarolo`) and the scorch marks
//! (`Koszfolt`).

use crate::colony::{grid_to_real, BuildingView};
use crate::colony_weather::CRand;
use crate::scene::{Transform, V3};

/// One puff of owner 1: `kind` 3 (the furnaces', the damage and the units' smoke), 10 (a chimney's), or the war's 0
/// (exhaust, debris trails), 6 (the vehicles' dust) and 8 (the ruins' dust), colony.md §5.7.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Puff {
    kind: u8,
    pub p: V3,
    v: V3,
    rgb: [i32; 3],
    a: i32,
    pub size: f32,
}

impl Puff {
    pub fn new(kind: u8, p: V3, v: V3, rgb: [i32; 3], a: i32, size: f32) -> Self {
        Self { kind, p, v, rgb, a, size }
    }

    /// `ParticleOwner::Draw`'s colour: `c·(255 − a) >> 8` a channel.
    pub fn colour(&self) -> [f32; 3] {
        self.rgb.map(|c| ((c * (255 - self.a)) >> 8) as f32 / 255.0)
    }
}

/// Where a building smokes: its chimneys (`SetLangPlaces` exe 0x49bd50: the children `fust1`, `fust2`, … in world
/// space) and its three damage points (the building's top under (gx + 30, gz − 25), (gx − 30, gz + 30), (gx − 30,
/// gz − 30), `vonalmetszes`), by slot.
#[derive(Debug, Clone, Default)]
pub struct Emitters {
    pub chimneys: Vec<(usize, V3)>,
    pub damage: Vec<(usize, [Option<V3>; 3])>,
}

/// A piece of debris in flight (list B kind 7, colony.md §5.8): its sim slot and where the sim made it, then where it
/// is, its velocity and its transparency.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    slot: usize,
    origin: V3,
    pub pos: V3,
    pub vel: V3,
    pub fade: f32,
}

/// A ruin (`Rom`, colony.md §5.9): the building's slot, kind and placement, its footprint, the rubble's place
/// `(ground, x, y)`, the rubble's transparency and the building's fade, and which phases run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ruin {
    pub slot: usize,
    pub kind: u8,
    pub building: Transform,
    pub foot: (i32, i32),
    pub at: V3,
    pub rubble: f32,
    pub fade: f32,
    fading_in: bool,
    pub collapsing: bool,
    fading_out: bool,
}

impl Ruin {
    /// `Rom::Rom` (exe 0x455570): the rubble unseen (1.0), the building whole (0.01), collapsing and fading in.
    pub fn new(slot: usize, kind: u8, building: Transform, foot: (i32, i32), at: V3) -> Self {
        Self { slot, kind, building, foot, at, rubble: 1.0, fade: 0.01, fading_in: true, collapsing: true, fading_out: false }
    }
}

/// A scorch mark (`Koszfolt`, colony.md §5.10): where (ground height), its extents, `n` (the fade, gone at 255) and
/// whether it is a building's (`epuletkosz.bmp`) or a vehicle's (`tankkosz.bmp`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scorch {
    pub at: V3,
    pub size: (f32, f32),
    pub n: i32,
    pub building: bool,
}

pub struct Smoke {
    puffs: Vec<Puff>,
    rand: CRand,
    /// The fifth UI tick last stepped (`ColonyCounter / 5`), for the buildings' and for the war's share (from its
    /// first call: nothing to catch up before).
    last: i32,
    last_war: Option<i32>,
    /// The second UI tick the debris last flew (`ColonyCounter / 2`).
    last_debris: Option<i32>,
    debris: Vec<Flight>,
    pub ruins: Vec<Ruin>,
    /// Newest first (`ElementList::Add` pushes at the head).
    pub scorches: Vec<Scorch>,
}

impl Default for Smoke {
    fn default() -> Self {
        Self { puffs: Vec::new(), rand: CRand(1), last: 0, last_war: None, last_debris: None, debris: Vec::new(), ruins: Vec::new(), scorches: Vec::new() }
    }
}

impl Smoke {
    /// Every fifth UI tick from the last step up to `counter` (`ColonyCounter`): age (`Refresh2`), the chimneys'
    /// puffs, then the buildings' ones. `bottom` gives a slot's `+0x14`.
    pub fn advance(&mut self, counter: f32, buildings: &[BuildingView], hp: &dyn Fn(u8) -> i32, bottom: &dyn Fn(usize) -> Option<f32>, at: &Emitters) {
        let now = counter as i32 / 5;
        if now < self.last {
            self.last = now;
        }
        while self.last < now {
            self.last += 1;
            self.age();
            self.chimneys(buildings, at);
            self.buildings(buildings, hp, bottom, at);
        }
    }

    pub fn puffs(&self) -> &[Puff] {
        &self.puffs
    }

    /// A particle the sim drew (owner 1).
    pub fn push(&mut self, q: Puff) {
        self.puffs.push(q);
    }

    pub fn scorch(&mut self, at: V3, size: (f32, f32), building: bool) {
        self.scorches.insert(0, Scorch { at, size, n: 0, building });
    }

    /// A ruin and its scorch mark (`Rom::Rom`: `40w × 40h`, `epuletkosz.bmp`).
    pub fn ruin(&mut self, r: Ruin) {
        self.scorch(r.at, (r.foot.0 as f32 * 40.0, r.foot.1 as f32 * 40.0), true);
        self.ruins.insert(0, r);
    }

    /// The debris `(slot, place made, velocity)` as the sim holds it, flown up to `counter` ([`Self::flight`]).
    pub fn fly(&mut self, counter: f32, pieces: &[(usize, V3, V3)]) {
        self.debris.retain(|f| pieces.iter().any(|&(slot, at, _)| slot == f.slot && at == f.origin));
        for &(slot, at, vel) in pieces {
            if !self.debris.iter().any(|f| f.slot == slot && f.origin == at) {
                self.debris.push(Flight { slot, origin: at, pos: at, vel, fade: 0.0 });
            }
        }
        let now = counter as i32 / 2;
        let mut last = self.last_debris.unwrap_or(now).min(now);
        while last < now {
            last += 1;
            self.debris_step();
        }
        self.last_debris = Some(now);
    }

    /// Where the piece of debris in `slot` made at `at` is and its transparency; `None` once it has faded.
    pub fn flight(&self, slot: usize, at: V3) -> Option<&Flight> {
        self.debris.iter().find(|f| f.slot == slot && f.origin == at).filter(|f| f.fade <= 1.0)
    }

    /// `WF::Refresh2`'s debris branch (exe 0x48ce10): every twelfth slot a trail puff (the exe draws vy, vx, vh), then
    /// `pos += vel`, `vh += 0.05`, and 0.01 more transparent a step once falling.
    fn debris_step(&mut self) {
        for k in 0..self.debris.len() {
            let f = self.debris[k];
            if f.fade > 1.0 {
                continue;
            }
            if f.slot % 12 == 0 {
                let r = &mut self.rand;
                let vy = r.below(3) - 1.0;
                let vx = r.below(3) - 1.0;
                let vh = r.below(3) - 1.0;
                self.puffs.push(Puff { kind: 0, p: f.pos, v: [vh, vx, vy], rgb: [0xff, 0x82, 0], a: 0xa0, size: 5.0 });
            }
            let f = &mut self.debris[k];
            f.pos = std::array::from_fn(|i| f.pos[i] + f.vel[i]);
            f.vel[0] = (f.vel[0] as f64 + 0.05) as f32;
            if f.vel[0] > 0.0 {
                f.fade = (f.fade as f64 + 0.01) as f32;
            }
        }
    }

    /// The war's share on the fifth UI ticks up to `counter`: the smoke of the damaged units at `smoking` (`(h, x, y)`),
    /// the ruins and the scorch marks.
    pub fn war(&mut self, counter: f32, smoking: &[V3]) {
        let now = counter as i32 / 5;
        let mut last = self.last_war.unwrap_or(now).min(now);
        self.last_war = Some(now);
        while last < now {
            last += 1;
            for &at in smoking {
                self.unit_smoke(at);
                self.unit_smoke(at);
            }
            for k in 0..self.ruins.len() {
                self.ruin_step(k);
            }
            self.ruins.retain(|r| !(r.fading_out && r.rubble >= 1.0));
            // `Koszfolt::Refresh`: `rndlog(0, 0, 99) < 10` → `n + 1`, gone at 255 (once a sim tick in the exe: the same
            // rate at normal speed).
            for k in 0..self.scorches.len() {
                if self.rand.below(100) < 10.0 {
                    self.scorches[k].n += 1;
                }
            }
            self.scorches.retain(|m| m.n != 255);
        }
    }

    /// `WF_Tank::Refresh2` (exe 0x48f300): one orange type-3 puff about a damaged unit (the exe draws the size first,
    /// the x offset last).
    fn unit_smoke(&mut self, at: V3) {
        let r = &mut self.rand;
        let size = (r.below(10) as i32 / 2 + 5) as f32;
        let b = r.below(50) as i32;
        let g = 0xa3 - r.below(130) as i32;
        let red = 0xff - r.below(100) as i32;
        let vy = r.below(20) / 10.0 - 1.0;
        let vx = r.below(20) / 10.0 - 1.0;
        let vh = -(r.below(10) / 10.0 + 0.5) * 5.0;
        let y = at[2] + r.below(20) - 10.0;
        let x = at[1] + r.below(20) - 10.0;
        self.puffs.push(Puff { kind: 3, p: [at[0] - 5.0, x, y], v: [vh, vx, vy], rgb: [red, g, b], a: 0, size });
    }

    /// `Rom::Refresh` (exe 0x4904f0): fade the rubble in, collapse the building with two dust puffs a call, then fade
    /// the rubble out.
    fn ruin_step(&mut self, k: usize) {
        let r = &mut self.ruins[k];
        if r.fading_in {
            r.rubble = (r.rubble as f64 - 0.03) as f32;
            if r.rubble < 0.01 {
                r.rubble = 0.01;
                r.fading_in = false;
            }
        }
        if r.collapsing {
            r.fade = (r.fade as f64 + 0.015) as f32;
            if r.fade > 1.0 {
                r.fade = 1.0;
                r.collapsing = false;
            }
            let at = r.at;
            for _ in 0..2 {
                let vy = self.rand.below(100) / 10.0 - 5.0;
                let vx = self.rand.below(100) / 10.0 - 5.0;
                self.puffs.push(Puff { kind: 8, p: at, v: [0.0, vx, vy], rgb: [200, 200, 0xaf], a: 0x80, size: 20.0 });
            }
        } else if !r.fading_out {
            (r.fading_out, r.fading_in, r.rubble) = (true, false, 0.01);
        }
        let r = &mut self.ruins[k];
        if r.fading_out {
            r.rubble = (r.rubble as f64 + 0.03) as f32;
        }
    }

    /// `WF::Refresh2`'s types 3 and 10. Type 3: `a += a < 190 ? 15 : 4`; alive below 241: moves, `v ·= 0.97`, grows
    /// 0.5, and past 190 each channel goes a fifth of the way to their mean. Type 10: `a += 3`; alive below 251:
    /// moves, drifts 0.08 more in y and z, grows 0.3.
    fn age(&mut self) {
        self.puffs.retain_mut(|q| {
            if q.kind == 3 {
                q.a += if q.a < 0xbe { 15 } else { 4 };
                if q.a >= 241 {
                    return false;
                }
                q.p = std::array::from_fn(|k| q.p[k] + q.v[k]);
                q.v = q.v.map(|x| (x as f64 * 0.97) as f32);
                q.size = (q.size as f64 + 0.5) as f32;
                if q.a > 190 {
                    let m = ((q.rgb[0] + q.rgb[1] + q.rgb[2]) as f64 / 3.0) as f32;
                    q.rgb = q.rgb.map(|c| ((m - c as f32) as f64 / 5.0 + c as f64) as i32);
                }
            } else if q.kind == 0 {
                q.a += 5;
                if q.a >= 0xfb {
                    return false;
                }
                q.p = std::array::from_fn(|k| q.p[k] + q.v[k]);
                q.v = q.v.map(|x| (x as f64 * 0.95) as f32);
                q.size += 1.0;
                let m = (q.rgb[0] + q.rgb[1] + q.rgb[2]) as f32 / 3.0;
                q.rgb = q.rgb.map(|c| (c as f32 + (m - c as f32) / 10.0) as i32);
            } else if q.kind == 6 {
                q.a += 1;
                if q.a >= 0x100 {
                    return false;
                }
                q.p = std::array::from_fn(|k| q.p[k] + q.v[k]);
                q.size = (q.size as f64 + 0.5) as f32;
            } else if q.kind == 8 {
                q.a += 3;
                if q.a >= 0xf6 {
                    return false;
                }
                q.p = std::array::from_fn(|k| q.p[k] + q.v[k]);
                q.v[0] = (q.v[0] as f64 - 0.15) as f32;
                q.v[1] = (q.v[1] as f64 * 0.965) as f32;
                q.v[2] = (q.v[2] as f64 * 0.965) as f32;
                q.size = (q.size as f64 + 0.3) as f32;
            } else {
                q.a += 3;
                if q.a >= 251 {
                    return false;
                }
                q.p = std::array::from_fn(|k| q.p[k] + q.v[k]);
                q.v[1] += 0.08;
                q.v[2] += 0.08;
                q.size = (q.size as f64 + 0.3) as f32;
            }
            true
        });
    }

    /// `KemenyFust::Refresh`: a working building's chimney puffs once a step (exe draw order: size, x speed, z, y).
    fn chimneys(&mut self, buildings: &[BuildingView], at: &Emitters) {
        for &(slot, pos) in &at.chimneys {
            if !buildings.iter().any(|b| b.slot == slot && b.flags & 1 != 0) {
                continue;
            }
            let r = &mut self.rand;
            let size = (r.below(10) as i32 / 2 + 5) as f32 / 2.0;
            let vx = -((r.below(10) / 10.0 + 0.5) * 5.0 / 3.5);
            let z = pos[2] + r.below(5) - 2.5;
            let y = pos[1] + r.below(5) - 2.5;
            self.puffs.push(Puff { kind: 10, p: [pos[0], y, z], v: [vx, 0.0, 0.0], rgb: [100, 100, 90], a: 0x80, size });
        }
    }

    /// `SetBuildingSpecialEffects`: three puffs a step over a working kind-10 building (at `H − 15`), and two at each
    /// damage point the damage has passed (`> hp/4`, `> 2·(hp/4)`, `> 3·(hp/4)`) for any building. The exe reads one
    /// table entry for both horizontal offsets and one for both side speeds.
    fn buildings(&mut self, buildings: &[BuildingView], hp: &dyn Fn(u8) -> i32, bottom: &dyn Fn(usize) -> Option<f32>, at: &Emitters) {
        for b in buildings {
            let quarter = hp(b.kind) / 4;
            if b.kind == 10 && b.flags & 1 != 0 && let Some(h) = bottom(b.slot) {
                let (gx, gz) = grid_to_real(b.x, b.y);
                for _ in 0..3 {
                    self.puff([h - 15.0, gx, gz]);
                }
            }
            let Some((_, points)) = at.damage.iter().find(|(s, _)| *s == b.slot) else { continue };
            for (k, p) in points.iter().enumerate() {
                if let Some(p) = p
                    && b.damage > quarter * (k as i32 + 1)
                {
                    self.puff(*p);
                    self.puff(*p);
                }
            }
        }
    }

    fn puff(&mut self, at: V3) {
        let r = &mut self.rand;
        let off = r.below(20) - 10.0;
        let side = r.below(20) / 10.0 - 1.0;
        let vx = -(r.below(10) / 10.0 + 0.5) * 5.0;
        let rgb = [255 - r.below(100) as i32, 163 - r.below(130) as i32, r.below(50) as i32];
        let size = r.below(10) * 0.5 + 10.0 - 5.0;
        self.puffs.push(Puff { kind: 3, p: [at[0], at[1] + off, at[2] + off], v: [vx, side, side], rgb, a: 0, size });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn furnace_smoke_lives_24_steps_and_greys() {
        let b = BuildingView { slot: 1, kind: 10, x: 50.0, y: 50.0, work_left: 0, damage: 0, flags: 1, yield_pct: 100, efficiency: 256 };
        let mut s = Smoke::default();
        let at = Emitters::default();
        // One step adds three; they die on their 25th ageing (a: 15..195 by 15, then 199..239 by 4, 243 gone).
        s.advance(5.0, std::slice::from_ref(&b), &|_| 100, &|_| Some(0.0), &at);
        let first: Vec<Puff> = s.puffs().to_vec();
        s.advance(5.0 * 26.0, &[], &|_| 100, &|_| Some(0.0), &at);
        assert!(s.puffs().is_empty());
        let mut t = Smoke { puffs: first.clone(), ..Smoke::default() };
        for _ in 0..13 {
            t.age();
        }
        let q = t.puffs()[0];
        assert_eq!(q.a, 195);
        assert!(q.rgb[0] < first[0].rgb[0] && q.rgb[2] > first[0].rgb[2]);
    }

    #[test]
    fn chimney_smoke_lives_40_steps() {
        let b = BuildingView { slot: 2, kind: 3, x: 0.0, y: 0.0, work_left: 0, damage: 0, flags: 1, yield_pct: 100, efficiency: 256 };
        let at = Emitters { chimneys: vec![(2, [0.0, 0.0, 0.0])], damage: Vec::new() };
        let mut s = Smoke::default();
        s.advance(5.0, std::slice::from_ref(&b), &|_| 100, &|_| None, &at);
        assert_eq!(s.puffs().len(), 1);
        for _ in 0..40 {
            s.age();
        }
        assert_eq!(s.puffs().len(), 1);
        s.age();
        assert!(s.puffs().is_empty());
    }

    #[test]
    fn a_ruin_collapses_67_steps_then_fades_out() {
        let xf = Transform { matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0], translation: [0.0; 3] };
        let mut s = Smoke::default();
        s.ruin(Ruin::new(3, 7, xf, (2, 2), [0.0; 3]));
        assert_eq!((s.scorches.len(), s.scorches[0].size), (1, (80.0, 80.0)));
        s.war(0.0, &[]);
        let mut steps = 0;
        while s.ruins.first().is_some_and(|r| r.collapsing) {
            s.war(5.0 * (steps + 1) as f32, &[]);
            steps += 1;
        }
        // 0.01 + 0.015·k passes 1.0 on the 67th call; two dust puffs each call.
        assert_eq!(steps, 67);
        assert_eq!(s.ruins[0].rubble, 0.01);
        while !s.ruins.is_empty() {
            steps += 1;
            s.war(5.0 * steps as f32, &[]);
        }
        // 0.04 + 0.03·k in f32 reaches 1.0 on the 33rd call after the switch only with rounding: the 34th.
        assert_eq!(steps, 67 + 34);
    }

    #[test]
    fn debris_rises_falls_and_fades() {
        let mut s = Smoke::default();
        let piece = [(1, [0.0, 0.0, 0.0], [-1.0, 0.0, 0.0])];
        let mut t = 0;
        s.fly(0.0, &piece);
        while s.flight(1, [0.0; 3]).is_some() {
            t += 1;
            s.fly(2.0 * t as f32, &piece);
        }
        // 20 steps to stop climbing (−1 + 0.05k > 0 from k = 21), then 100 of 0.01 (f32 sums: one more).
        assert!((120..=122).contains(&t), "{t}");
        // The piece the sim drops goes with it.
        s.fly(2.0 * t as f32, &[]);
        assert!(s.debris.is_empty());
    }

    #[test]
    fn dust_types_live_as_the_exe_steps_them() {
        let lives = |kind: u8, a: i32| {
            let mut s = Smoke::default();
            s.push(Puff::new(kind, [0.0; 3], [0.0; 3], [100, 100, 100], a, 4.0));
            let mut n = 0;
            while !s.puffs.is_empty() {
                s.age();
                n += 1;
            }
            n
        };
        // Type 6 from 0xa0 by 1 below 0x100, type 8 from 0x80 by 3 below 0xf6, type 0 from 0x80 by 5 below 0xfb.
        assert_eq!((lives(6, 0xa0), lives(8, 0x80), lives(0, 0x80)), (96, 40, 25));
    }
}
