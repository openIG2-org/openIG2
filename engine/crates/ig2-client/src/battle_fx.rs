//! The "Enhanced" battle effects (ADR 0057): particle layers drawn on top of the classic effects of the space battle
//! and the ground war, started where and when the classic ones start. Flashes, sparks, embers, shock rings, smoke
//! and smoke columns; the glows use the install's `flare.bmp`, the smoke its smoke pictures, the sparks and rings
//! two pictures drawn here. Classic (the default) never builds a [`Layers`], so its effects and captures stay the exe's.

use crate::render3d::MeshVertex;

type V3 = [f32; 3];

/// The "Battle effects" option (ADR 0057, Classic by default).
static ENHANCED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_enhanced(on: bool) {
    ENHANCED.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn enhanced() -> bool {
    ENHANCED.load(std::sync::atomic::Ordering::Relaxed)
}

/// At most this many particles per battle; the oldest go first.
const CAP: usize = 4096;

/// What a particle looks like: the glow (`flare.bmp`), the soft dot, the ring (added) or smoke (darkening).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sprite {
    Glow,
    Spark,
    Ring,
    Smoke,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bit {
    pub sprite: Sprite,
    pub pos: V3,
    vel: V3,
    drag: f32,
    fall: f32,
    age: u16,
    life: u16,
    size: [f32; 2],
    rgb: [f32; 3],
}

impl Bit {
    fn t(&self) -> f32 {
        self.age as f32 / self.life.max(1) as f32
    }

    /// The half size now: rings and smoke grow fast then slow (ease-out), the rest go linearly.
    pub fn size(&self) -> f32 {
        let t = self.t();
        let k = match self.sprite {
            Sprite::Ring | Sprite::Smoke => 1.0 - (1.0 - t) * (1.0 - t),
            _ => t,
        };
        self.size[0] + (self.size[1] - self.size[0]) * k
    }

    /// The colour added (or, for smoke, taken away) now: smoke fades in over its first fifth, everything fades out.
    pub fn colour(&self) -> [f32; 3] {
        let t = self.t();
        let a = match self.sprite {
            Sprite::Smoke => (t * 5.0).min(1.0) * (1.0 - t),
            Sprite::Ring => (1.0 - t) * (1.0 - t),
            _ => 1.0 - t,
        };
        self.rgb.map(|c| c * a)
    }
}

/// A smoke column: where, how big, the ticks it still puffs.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Column {
    at: V3,
    r: f32,
    left: u16,
}

/// One battle's extra particles.
#[derive(Debug, Clone, PartialEq)]
pub struct Layers {
    pub bits: Vec<Bit>,
    columns: Vec<Column>,
    /// The world's down (the ground war's +height; none in space).
    down: V3,
    /// Lives are given at 20 ticks a second and scaled by this.
    pace: f32,
    seed: u32,
}

impl Layers {
    /// `down`: the unit vector gravity pulls along (zero in space); `hz`: the ticks a second [`Layers::step`] runs at.
    pub fn new(down: V3, hz: f32) -> Self {
        Self { bits: Vec::new(), columns: Vec::new(), down, pace: hz / 20.0, seed: 0x9e37_79b9 }
    }

    /// xorshift32 in 0..1 (our own stream: the classic effects' draws are untouched).
    fn rand(&mut self) -> f32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn between(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.rand()
    }

    /// A random unit vector.
    fn dir(&mut self) -> V3 {
        loop {
            let v: V3 = std::array::from_fn(|_| self.rand() * 2.0 - 1.0);
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if l > 0.05 && l <= 1.0 {
                return v.map(|x| x / l);
            }
        }
    }

    fn ticks(&self, at20: f32) -> u16 {
        (at20 * self.pace).round().clamp(1.0, u16::MAX as f32) as u16
    }

    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, sprite: Sprite, pos: V3, vel: V3, drag: f32, fall: f32, life20: f32, size: [f32; 2], rgb: [f32; 3]) {
        let life = self.ticks(life20);
        // Per-tick motion given at 20 Hz: a faster clock moves less a tick.
        let (vel, fall) = (vel.map(|v| v / self.pace), fall / (self.pace * self.pace));
        let drag = drag.powf(1.0 / self.pace);
        self.bits.push(Bit { sprite, pos, vel, drag, fall, age: 0, life, size, rgb });
    }

    fn sparks(&mut self, at: V3, r: f32, n: usize, speed: (f32, f32), life: (f32, f32), rgb: [f32; 3]) {
        for _ in 0..n {
            let d = self.dir();
            let s = self.between(speed.0, speed.1) * r;
            let l = self.between(life.0, life.1);
            self.add(Sprite::Spark, at, d.map(|x| x * s), 0.86, 0.0, l, [r * 0.07, r * 0.02], rgb);
        }
    }

    /// A blast of radius `r` at `at`: the flash, the fireball, the shock ring, sparks, embers and smoke.
    pub fn blast(&mut self, at: V3, r: f32) {
        self.add(Sprite::Glow, at, [0.0; 3], 1.0, 0.0, 7.0, [r * 1.4, r * 2.2], [1.0, 0.9, 0.7]);
        self.add(Sprite::Glow, at, [0.0; 3], 1.0, 0.0, 16.0, [r * 0.7, r * 1.5], [0.9, 0.45, 0.12]);
        self.add(Sprite::Ring, at, [0.0; 3], 1.0, 0.0, 14.0, [r * 0.3, r * 3.2], [0.8, 0.65, 0.45]);
        self.sparks(at, r, 24, (0.15, 0.45), (8.0, 20.0), [1.0, 0.8, 0.45]);
        let up = self.down.map(|x| -x);
        for _ in 0..8 {
            let d = self.dir();
            let s = self.between(0.05, 0.15) * r;
            let vel = std::array::from_fn(|k| d[k] * s + up[k] * r * 0.15);
            let l = self.between(25.0, 40.0);
            self.add(Sprite::Spark, at, vel, 0.96, r * 0.012, l, [r * 0.06, r * 0.03], [1.0, 0.4, 0.08]);
        }
        for _ in 0..5 {
            let d = self.dir();
            let pos = std::array::from_fn(|k| at[k] + d[k] * r * 0.3);
            let vel = std::array::from_fn(|k| d[k] * r * 0.01 + up[k] * r * 0.015);
            let l = self.between(35.0, 55.0);
            self.add(Sprite::Smoke, pos, vel, 0.97, 0.0, l, [r * 0.4, r * 1.2], [0.3; 3]);
        }
    }

    /// A hit of size `r`: a short flash and a spray of sparks.
    pub fn hit(&mut self, at: V3, r: f32) {
        self.add(Sprite::Glow, at, [0.0; 3], 1.0, 0.0, 5.0, [r, r * 0.5], [1.0, 0.85, 0.6]);
        self.sparks(at, r, 8, (0.2, 0.5), (5.0, 11.0), [1.0, 0.85, 0.55]);
    }

    /// A hit on a shield of size `r`: a blue-white flash and a quick ring.
    pub fn shield(&mut self, at: V3, r: f32) {
        self.add(Sprite::Glow, at, [0.0; 3], 1.0, 0.0, 8.0, [r * 1.6, r * 2.4], [0.35, 0.6, 1.0]);
        self.add(Sprite::Ring, at, [0.0; 3], 1.0, 0.0, 10.0, [r * 0.4, r * 2.6], [0.3, 0.55, 1.0]);
        self.sparks(at, r, 6, (0.15, 0.35), (4.0, 9.0), [0.55, 0.8, 1.0]);
    }

    /// A smoke column of size `r` at `at`, puffing for `secs` seconds.
    pub fn column(&mut self, at: V3, r: f32, secs: f32) {
        let left = self.ticks(secs * 20.0);
        self.columns.push(Column { at, r, left });
    }

    /// One tick: the columns puff (every third 20 Hz tick), the particles move and age.
    pub fn step(&mut self) {
        let every = self.ticks(3.0);
        let up = self.down.map(|x| -x);
        let rising = up != [0.0; 3];
        for i in 0..self.columns.len() {
            let c = self.columns[i];
            self.columns[i].left -= 1;
            if c.left % every != 0 {
                continue;
            }
            let d = self.dir();
            let pos = std::array::from_fn(|k| c.at[k] + d[k] * c.r * 0.2 + up[k] * c.r * 0.2);
            let drift = self.dir();
            let vel = if rising { std::array::from_fn(|k| up[k] * c.r * 0.05 + drift[k] * c.r * 0.01) } else { drift.map(|x| x * c.r * 0.02) };
            let l = self.between(50.0, 75.0);
            self.add(Sprite::Smoke, pos, vel, 0.99, 0.0, l, [c.r * 0.3, c.r * 1.3], [0.25; 3]);
        }
        self.columns.retain(|c| c.left > 0);
        let down = self.down;
        for b in &mut self.bits {
            for (k, d) in down.iter().enumerate() {
                b.pos[k] += b.vel[k];
                b.vel[k] = b.vel[k] * b.drag + d * b.fall;
            }
            b.age += 1;
        }
        self.bits.retain(|b| b.age < b.life);
        if self.bits.len() > CAP {
            self.bits.drain(..self.bits.len() - CAP);
        }
    }

    /// The camera-facing quads of the particles drawn with `sprite`: `axes` are the eye's right and up in the world,
    /// `world` maps a battle point to the world and `scale` a battle size; `uv` is the sprite's cell.
    pub fn quads(&self, sprite: Sprite, axes: [V3; 2], world: impl Fn(V3) -> V3, scale: f32, uv: [f32; 4], out: &mut Vec<MeshVertex>) {
        let [a, b] = axes;
        for bit in self.bits.iter().filter(|x| x.sprite == sprite) {
            let (w, h, c) = (world(bit.pos), bit.size() * scale, bit.colour());
            let q = |sx: f32, sy: f32| MeshVertex {
                pos: std::array::from_fn(|k| w[k] + (a[k] * sx + b[k] * sy) * h),
                normal: [0.0; 3],
                uv: [uv[0] + (sx + 1.0) / 2.0 * uv[2], uv[1] + (1.0 - sy) / 2.0 * uv[3]],
                color: [c[0], c[1], c[2], 1.0],
            };
            out.extend([q(-1.0, 1.0), q(1.0, 1.0), q(1.0, -1.0), q(-1.0, 1.0), q(1.0, -1.0), q(-1.0, -1.0)]);
        }
    }
}

/// The two pictures drawn here (64 × 64, white on black, for the additive blend): a soft dot (a Gaussian, also the
/// beams' glow across their width) and a thin ring at 0.75 of the radius.
pub fn pictures() -> [crate::assets::Image; 2] {
    const N: u32 = 64;
    let make = |f: &dyn Fn(f32) -> f32| {
        let mut rgba = Vec::with_capacity((N * N * 4) as usize);
        for y in 0..N {
            for x in 0..N {
                let (dx, dy) = ((x as f32 + 0.5) / N as f32 * 2.0 - 1.0, (y as f32 + 0.5) / N as f32 * 2.0 - 1.0);
                let v = (f((dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0) * 255.0).round() as u8;
                rgba.extend([v, v, v, 255]);
            }
        }
        crate::assets::Image { width: N, height: N, rgba, mips: Vec::new(), hires: None }
    };
    let dot = make(&|d| if d >= 1.0 { 0.0 } else { (-d * d * 5.0).exp() * (1.0 - d) });
    let ring = make(&|d| if d >= 1.0 { 0.0 } else { (-((d - 0.75) / 0.09).powi(2)).exp() });
    [dot, ring]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blast_fades_away_and_stays_capped() {
        let mut l = Layers::new([0.0; 3], 20.0);
        l.blast([0.0; 3], 10.0);
        assert_eq!(l.bits.len(), 3 + 24 + 8 + 5);
        for _ in 0..55 {
            l.step();
        }
        assert!(l.bits.is_empty(), "{}", l.bits.len());
        for _ in 0..200 {
            l.blast([0.0; 3], 1.0);
        }
        l.step();
        assert_eq!(l.bits.len(), CAP);
    }

    #[test]
    fn a_column_rises_for_its_seconds_on_the_ground() {
        let mut l = Layers::new([1.0, 0.0, 0.0], 20.0);
        l.column([0.0; 3], 10.0, 1.0);
        for _ in 0..20 {
            l.step();
        }
        // 20 ticks, a puff every third (18, 15, …, 3 ticks left): 6, all above (−height) the column's foot.
        assert_eq!(l.bits.len(), 6);
        assert!(l.bits.iter().all(|b| b.pos[0] < 0.0));
        l.step();
        assert_eq!(l.bits.len(), 6);
    }

    #[test]
    fn lives_follow_the_clock() {
        let (mut slow, mut fast) = (Layers::new([0.0; 3], 20.0), Layers::new([0.0; 3], 100.0));
        slow.hit([0.0; 3], 1.0);
        fast.hit([0.0; 3], 1.0);
        let longest = |l: &Layers| l.bits.iter().map(|b| b.life).max().unwrap();
        // The same draws, five times the ticks (each rounded on its own clock).
        assert!((longest(&fast) as i32 - 5 * longest(&slow) as i32).abs() <= 3);
        assert!(slow.bits[0].colour()[0] > 0.99 && slow.bits[0].size() == 1.0);
    }

    #[test]
    fn pictures_are_bright_where_they_should_be() {
        let [dot, ring] = pictures();
        let px = |i: &crate::assets::Image, x: u32, y: u32| i.rgba[((y * 64 + x) * 4) as usize];
        assert!(px(&dot, 32, 32) > 240 && px(&dot, 0, 32) == 0);
        assert!(px(&ring, 32 + 24, 32) > 200 && px(&ring, 32, 32) == 0);
    }
}
