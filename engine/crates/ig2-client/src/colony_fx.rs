//! The ground war's effects on the colony screen (spec `re/screens/colony.md` §5.2–§5.6, §5.11): the projectiles'
//! trails, the bomb blasts' dust, the hit and blast flashes, the machine gun's tracers and the laser's beams, fed by
//! the sim's render-only [`GroundEvent`]s of the shown war. Owner 1's particles and the scorch marks wait in
//! [`WarFx::owner1`] and [`WarFx::scorches`] for the scene's [`crate::colony_smoke::Smoke`].

use crate::spacewar_fx::Bolt;
use crate::starmap::MsvcRand;
use ig2_sim::ground::{GroundEvent, TrailSource};

type V3 = [f32; 3];

/// A flash lives 25 frames of `robbanas.bmp`'s 5 × 5 sheet (`loadanimtexture("robbanas.bmp", 0x19)`).
pub const FLASH_FRAMES: i32 = 25;

/// A particle of owner 0 (`ParticleOwner::AddParticle` exe 0x45d270): velocity, position (height, x, y), colour,
/// fade `T`, size and type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Puff {
    vel: V3,
    pub pos: V3,
    rgb: [i32; 3],
    t: i32,
    pub size: f32,
    ty: u8,
}

impl Puff {
    /// `ParticleOwner::Draw` (exe 0x526f80): `c·(255 − T) >> 8`, added.
    pub fn colour(&self) -> [f32; 3] {
        self.rgb.map(|c| ((c * (255 - self.t)) >> 8) as f32 / 255.0)
    }

    /// One round of `WF::Refresh`'s owner-0 step (spec §5.2); false when it goes.
    fn step(&mut self) -> bool {
        // (dT, limit, growth, grey k)
        let (dt, limit, grow, grey) = match self.ty {
            0 => (5, 0xfb, 1.0, Some(10.0)),
            2 => (10, 0xf6, 0.2, None),
            _ => (0x14, 0xf6, 0.2, None),
        };
        self.t += dt;
        if self.t >= limit {
            return false;
        }
        for k in 0..3 {
            self.pos[k] += self.vel[k];
            self.vel[k] = (self.vel[k] as f64 * 0.95) as f32;
        }
        self.size += grow;
        if let Some(k) = grey {
            let avg = (self.rgb[0] + self.rgb[1] + self.rgb[2]) as f32 / 3.0;
            self.rgb = self.rgb.map(|c| (c as f32 + (avg - c as f32) / k) as i32);
        }
        true
    }
}

/// A trail's look by its source (spec §5.2): colour, `T`, size, type.
fn trail_look(of: TrailSource) -> ([i32; 3], i32, f32, u8) {
    match of {
        TrailSource::Fighter => ([0x32, 0x3c, 0xff], 0x96, 10.0, 0),
        TrailSource::Rocket => ([0xff, 0x82, 0], 0x40, 10.0, 0),
        TrailSource::Missile(0 | 1) => ([100, 100, 0xff], 200, 10.0, 2),
        TrailSource::Missile(2) => ([100, 100, 0xff], 0xdc, 10.0, 2),
        TrailSource::Missile(3) => ([100, 0xff, 100], 200, 7.0, 2),
        TrailSource::Missile(4) => ([0x38, 0xb2, 0xbb], 200, 6.0, 7),
        TrailSource::Missile(5) => ([0x40, 0x56, 0xc0], 200, 6.0, 7),
        TrailSource::Missile(_) => ([0x55, 0x95, 199], 200, 6.0, 7),
        TrailSource::Paralyser => ([100, 0xff, 100], 100, 10.0, 2),
    }
}

/// A type-4 particle of owner 1 (spec §5.3): where, its size and the `ColonyCounter` it came on; a projectile's hit
/// waits for [`WarFx::resolve`] to move it onto the target's mesh (the ray's far point and the target, §5.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flash {
    pub at: V3,
    pub size: f32,
    born: i64,
    ray: Option<(V3, i32)>,
}

/// `GetIntersectionPoint`'s far point: `target + 20·(muzzle − target)`.
fn far(from: V3, to: V3) -> V3 {
    std::array::from_fn(|k| to[k] + 20.0 * (from[k] - to[k]))
}

/// A machine-gun tracer (`LineParticle` type 2, spec §5.4): its ends (`a` on the target's side) and the sim ticks
/// until it shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tracer {
    pub a: V3,
    pub b: V3,
    life: i32,
}

impl Tracer {
    /// `LineParticle::Draw` (exe 0x471360) draws a type-2 line only while its life is 0.
    pub fn shown(&self) -> bool {
        self.life == 0
    }
}

/// The shown war's effects.
#[derive(Debug, Clone, PartialEq)]
pub struct WarFx {
    pub puffs: Vec<Puff>,
    pub flashes: Vec<Flash>,
    pub tracers: Vec<Tracer>,
    /// The laser beams and their sim ticks left.
    pub beams: Vec<(Bolt, i32)>,
    /// The beams still to be put onto their target's mesh: muzzle, target point, target, sim ticks left.
    aimed: Vec<(V3, V3, i32, i32)>,
    /// Owner 1's particles and the vehicles' scorch marks the sim reported since the scene last took them.
    pub owner1: Vec<crate::colony_smoke::Puff>,
    pub scorches: Vec<V3>,
    /// The tracers' and beams' draws (the exe's shared libc stream).
    rand: MsvcRand,
    /// The "Enhanced" battle effects' layers (ADR 0057; none with Classic) and the 20 Hz tick they last stepped on.
    pub extra: Option<crate::battle_fx::Layers>,
    stepped: Option<i64>,
}

impl Default for WarFx {
    fn default() -> Self {
        Self { puffs: Vec::new(), flashes: Vec::new(), tracers: Vec::new(), beams: Vec::new(), aimed: Vec::new(), owner1: Vec::new(), scorches: Vec::new(), rand: MsvcRand(1), extra: None, stepped: None }
    }
}

impl WarFx {
    /// One of the shown war's events; `counter` = `ColonyCounter` now.
    pub fn event(&mut self, e: &GroundEvent, counter: f32) {
        if let Some(x) = self.layers() {
            match *e {
                GroundEvent::Flash { at, size } => x.blast(at.map(|v| v as f32), size as f32 * 4.0),
                GroundEvent::HitFlash { to, size, .. } => x.hit(to, size as f32 * 1.5),
                _ => {}
            }
        }
        match *e {
            GroundEvent::Flash { at, size } => {
                let now = counter as i64;
                self.flashes.retain(|f| Self::frame(f, now) < FLASH_FRAMES);
                self.flashes.push(Flash { at: at.map(|v| v as f32), size: size as f32, born: now, ray: None });
            }
            GroundEvent::HitFlash { from, to, size, target } => {
                let now = counter as i64;
                self.flashes.retain(|f| Self::frame(f, now) < FLASH_FRAMES);
                self.flashes.push(Flash { at: to, size: size as f32, born: now, ray: Some((far(from, to), target)) });
            }
            GroundEvent::Particle { owner1: true, at, vel, rgb, t, size, ty } => self.owner1.push(crate::colony_smoke::Puff::new(ty, at, vel, rgb, t, size)),
            GroundEvent::Particle { owner1: false, at, vel, rgb, t, size, ty } => self.puffs.push(Puff { vel, pos: at, rgb, t, size, ty }),
            GroundEvent::Scorch { at } => self.scorches.push(at),
            GroundEvent::Trail { at, vel, of } => {
                let (rgb, t, size, ty) = trail_look(of);
                self.puffs.push(Puff { vel: vel.map(f32::from), pos: at, rgb, t, size, ty });
            }
            GroundEvent::TrailStep { rounds } => {
                for _ in 0..rounds {
                    self.puffs.retain_mut(Puff::step);
                }
                self.sim_tick();
            }
            GroundEvent::MachineGun { from, to } => self.machine_gun(from, to),
            GroundEvent::Laser { from, to, target } => self.aimed.push((from, to, target, 20)),
            _ => {}
        }
    }

    /// The enhanced layers, made or dropped as the "Battle effects" option says.
    pub fn layers(&mut self) -> Option<&mut crate::battle_fx::Layers> {
        if crate::battle_fx::enhanced() != self.extra.is_some() {
            // The ground war's up is −height.
            self.extra = crate::battle_fx::enhanced().then(|| crate::battle_fx::Layers::new([1.0, 0.0, 0.0], 20.0));
        }
        self.extra.as_mut()
    }

    /// The enhanced layers stepped on the 20 Hz ticks (`(int)counter / 5`) up to `counter` (at most a second's worth).
    pub fn step_layers(&mut self, counter: f32) {
        let now = (counter as i64).div_euclid(5);
        let last = self.stepped.replace(now).unwrap_or(now);
        if let Some(x) = self.layers() {
            for _ in 0..(now - last).clamp(0, 20) {
                x.step();
            }
        }
    }

    /// `Lines::Refresh` and the beams' `villam::Refresh` (`FRISSIT_Colony` exe 0x474680, once a sim tick).
    fn sim_tick(&mut self) {
        self.tracers.retain_mut(|t| {
            t.life -= 1;
            t.life != -1
        });
        for (b, life) in &mut self.beams {
            b.refresh(&mut self.rand);
            *life -= 1;
        }
        self.beams.retain(|b| b.1 > 0);
        for a in &mut self.aimed {
            a.3 -= 1;
        }
        self.aimed.retain(|a| a.3 > 0);
    }

    /// `GetIntersectionPoint` (spec §5.6) for the flashes and beams that came since: `hit(target, far point, point)` is
    /// the nearest crossing of the target's mesh from the far point, if any.
    pub fn resolve(&mut self, hit: &mut dyn FnMut(i32, V3, V3) -> Option<V3>) {
        for f in &mut self.flashes {
            if let Some((a, t)) = f.ray.take() {
                f.at = hit(t, a, f.at).unwrap_or(f.at);
            }
        }
        for (from, to, target, life) in std::mem::take(&mut self.aimed) {
            let end = hit(target, far(from, to), to).unwrap_or(to);
            // `villamgenesis(12, 10, 1, 0.6, 4, flare, 3, −0.05, 15, 20, 1, A, B)`.
            self.beams.push((Bolt::new(&mut self.rand, 12, 10.0, 1.0, 0.6, 4, true, from, end), life));
        }
    }

    /// `Lines::AddMachineGunLine` (ELF 0x47c340) ×16: a random part of a jittered muzzle–target segment.
    fn machine_gun(&mut self, from: V3, to: V3) {
        for k in 0..16 {
            let r = |s: &mut Self, n: i32| s.rand.random(n) as f32;
            let dy = if k == 4 { 5.0 } else { 3.0 };
            // `AddMachineGunLine(target end, muzzle end, life)`: the line's white end is at the target.
            let p0 = [to[0], to[1] + r(self, 6) - 3.0, to[2] + r(self, 6) - dy];
            let p1 = [from[0] - 15.0 + r(self, 10) - 10.0, from[1] + r(self, 10) - 5.0, from[2] + r(self, 10) - 5.0];
            let life = self.rand.random(13) + 1;
            let m: V3 = std::array::from_fn(|i| (p0[i] + p1[i]) / 2.0);
            let half: V3 = std::array::from_fn(|i| p1[i] - m[i]);
            let l = ((half[0] * half[0] + half[1] * half[1] + half[2] * half[2]) as f64).sqrt() as f32;
            if l <= 0.0 {
                continue;
            }
            let d: V3 = std::array::from_fn(|i| (m[i] - p0[i]) / l);
            let (r0, r1) = (self.rand.random(100) as f64, self.rand.random(100) as f64);
            let a = std::array::from_fn(|i| m[i] - (l as f64 * r0 / 100.0 * d[i] as f64) as f32);
            let b = std::array::from_fn(|i| m[i] + (l as f64 * r1 / 100.0 * d[i] as f64) as f32);
            self.tracers.push(Tracer { a, b, life });
        }
    }

    /// The `robbanas.bmp` frame of `f` at `ColonyCounter` `now`: `WF::Refresh2` counts it up when `(int)counter % 5
    /// == 0` (exe 0x48ce10).
    fn frame(f: &Flash, now: i64) -> i32 {
        (now.div_euclid(5) - f.born.div_euclid(5)) as i32
    }

    /// The flashes still up at `counter`, with their frames.
    pub fn shown_flashes(&self, counter: f32) -> impl Iterator<Item = (&Flash, i32)> {
        let now = counter as i64;
        self.flashes.iter().map(move |f| (f, Self::frame(f, now))).filter(|&(_, k)| (0..FLASH_FRAMES).contains(&k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rocket_trail_greys_and_fades() {
        let mut fx = WarFx::default();
        fx.event(&GroundEvent::Trail { at: [0.0, 10.0, 20.0], vel: [1, 0, -1], of: TrailSource::Rocket }, 0.0);
        fx.event(&GroundEvent::TrailStep { rounds: 1 }, 0.0);
        let p = fx.puffs[0];
        assert_eq!((p.pos, p.size, p.t, p.rgb), ([1.0, 10.0, 19.0], 11.0, 0x45, [242, 129, 12]));
        // Type 0 goes once T reaches 0xfb: (0xfb − 0x40) / 5 = 38 rounds up.
        fx.event(&GroundEvent::TrailStep { rounds: 36 }, 0.0);
        assert_eq!(fx.puffs.len(), 1);
        fx.event(&GroundEvent::TrailStep { rounds: 1 }, 0.0);
        assert!(fx.puffs.is_empty());
    }

    #[test]
    fn a_flash_shows_25_frames_on_the_20_hz_clock() {
        let mut fx = WarFx::default();
        fx.event(&GroundEvent::Flash { at: [0, 1, 2], size: 20 }, 3.0);
        assert_eq!(fx.shown_flashes(4.9).map(|f| f.1).collect::<Vec<_>>(), [0]);
        assert_eq!(fx.shown_flashes(5.0).map(|f| f.1).collect::<Vec<_>>(), [1]);
        assert_eq!(fx.shown_flashes(124.0).map(|f| f.1).collect::<Vec<_>>(), [24]);
        assert_eq!(fx.shown_flashes(125.0).count(), 0);
    }

    #[test]
    fn tracers_show_for_one_tick_and_beams_live_20() {
        let mut fx = WarFx::default();
        fx.event(&GroundEvent::MachineGun { from: [0.0; 3], to: [0.0, 100.0, 0.0] }, 0.0);
        fx.event(&GroundEvent::Laser { from: [0.0; 3], to: [0.0, 100.0, 0.0], target: 0 }, 0.0);
        fx.resolve(&mut |_, _, _| None);
        assert_eq!(fx.tracers.len(), 16);
        let mut shown = 0;
        for _ in 0..20 {
            fx.event(&GroundEvent::TrailStep { rounds: 1 }, 0.0);
            shown += fx.tracers.iter().filter(|t| t.shown()).count();
        }
        assert_eq!((shown, fx.tracers.len(), fx.beams.len()), (16, 0, 0));
    }

    #[test]
    fn a_hit_flash_lands_on_the_mesh_from_the_far_point() {
        let mut fx = WarFx::default();
        fx.event(&GroundEvent::HitFlash { from: [0.0, 0.0, 0.0], to: [0.0, 10.0, 0.0], size: 20, target: 3 }, 0.0);
        let mut seen = None;
        fx.resolve(&mut |t, a, b| {
            seen = Some((t, a, b));
            Some([0.0, 7.0, 0.0])
        });
        assert_eq!(seen, Some((3, [0.0, -190.0, 0.0], [0.0, 10.0, 0.0])));
        assert_eq!(fx.flashes[0].at, [0.0, 7.0, 0.0]);
    }
}
