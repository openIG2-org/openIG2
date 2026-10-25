//! The buildings' smoke (re/screens/colony.md §3.5): `SetBuildingSpecialEffects` (exe 0x49a760) and the chimneys'
//! `KemenyFust::Refresh` (exe 0x490240) add puffs to the planet's particle owner 1 (`rcloud.bmp`) every fifth UI tick
//! (`(int)ColonyCounter % 5 == 0`), and `WF::Refresh2` (exe 0x48ce10) ages them on the same ticks; `ParticleOwner::Draw`
//! adds them as screen-aligned quads in `c·(255 − a) >> 8`. The random draws come from tables filled from a
//! wall-clock seed (`GenerateSpecFXRandoms` exe 0x47a240), so any source does; [`crate::colony_weather::CRand`] here.

use crate::colony::{grid_to_real, BuildingView};
use crate::colony_weather::CRand;
use crate::scene::V3;

/// One puff: `kind` 3 (the furnaces' and the damage smoke) or 10 (a chimney's).
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

pub struct Smoke {
    puffs: Vec<Puff>,
    rand: CRand,
    /// The fifth UI tick last stepped (`ColonyCounter / 5`).
    last: i32,
}

impl Default for Smoke {
    fn default() -> Self {
        Self { puffs: Vec::new(), rand: CRand(1), last: 0 }
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
}
