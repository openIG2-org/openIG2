//! The colony's weather (re/screens/colony.md §2.6): snow (`InitSnow` exe 0x4837f0, `UpdateSnow` 0x4a6f80) and rain
//! (`InitRain` 0x4836e0, `UpdateRain` 0x4a6d30, `ShowRain` 0x4a34a0), 1000 particles each in a box ±`X/10` around the
//! camera's pivot (`ColonyXStart`, `−ColonyYStart`) from 819 above the plane at height 0 down to it, moved once a drawn
//! frame (`ShowColony` calls the updates per call, not per UI tick). `InitWeather` (exe 0x483b00) picks one from the
//! battlefield's weather byte (`WF+0x8d0`: 1 snow, 2 rain) with `OptionsKellIdojaras` [taken as on, as the thunder].

use crate::colony::MAP_SIZE;
use crate::render3d::MeshVertex;
use crate::scene::V3;

/// libc `rand()` of the exe's runtime (MSVC: `x = x·214013 + 2531011`, bits 16..30), seeded like a fresh process; the
/// draws are not the exe's (its stream is shared with everything else), only their kind and order.
pub(crate) struct CRand(pub(crate) u32);

impl CRand {
    pub(crate) fn below(&mut self, n: i32) -> f32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        if n < 1 { 0.0 } else { (((self.0 >> 16) & 0x7fff) as i32 % n) as f32 }
    }
}

#[derive(Debug, Clone, Copy)]
struct Particle {
    p: V3,
    v: V3,
    size: f32,
}

pub struct Weather {
    snow: bool,
    parts: Vec<Particle>,
    /// The snow's count this frame (`UpdateSnow`'s `n`).
    shown: usize,
    rand: CRand,
}

/// `TEREPXSIZE / 10` and `/ 5` (the box's half size and size) as the draws use them (`(int)` for the modulus).
const TENTH: f32 = MAP_SIZE / 10.0;
const FIFTH: i32 = (MAP_SIZE / 5.0) as i32;

impl Weather {
    /// `InitWeather` + `InitSnow`/`InitRain` for the battlefield's weather byte; `None` without weather.
    pub fn new(weather: i32) -> Option<Self> {
        let mut rand = CRand(1);
        let snow = match weather {
            1 => true,
            2 => false,
            _ => return None,
        };
        let parts = (0..1000)
            .map(|_| {
                if snow {
                    // Nine draws a flake: x, three burnt, y, z, size, the drift.
                    let x = -rand.below(TENTH as i32);
                    rand.below(MAP_SIZE as i32);
                    rand.below(MAP_SIZE as i32);
                    rand.below(5);
                    let y = rand.below(FIFTH) - TENTH;
                    let z = rand.below(FIFTH) - TENTH;
                    let size = (rand.below(50) / 30.0 + 1.0) * 2.0;
                    let vy = rand.below(6) - 3.0;
                    let vz = rand.below(6) - 3.0;
                    Particle { p: [x, y, z], v: [10.0, vy, vz], size }
                } else {
                    let x = -rand.below(TENTH as i32);
                    let y = rand.below(FIFTH) - TENTH;
                    let z = rand.below(FIFTH) - TENTH;
                    Particle { p: [x, y, z], v: [51.0, 17.0, 0.0], size: 0.0 }
                }
            })
            .collect();
        Some(Self { snow, shown: if snow { 500 } else { 1000 }, parts, rand })
    }

    /// One drawn frame of `UpdateSnow`/`UpdateRain`. Snow: `n = 500 + sgn(s)·(int)(500·⁴√|s|)` flakes move, `s =
    /// sin(ColonyCounter/600)` (the rest stay where they are, unseen); a flake or drop below the plane (`x > 0`) starts
    /// again at the top: snow with seven draws (two burnt, y, z, size, drift), rain with four (two burnt, y, z) and the
    /// fall (50, 17, 0) (the first fall is 51).
    pub fn step(&mut self, counter: f32) {
        if self.snow {
            let s = (counter as f64 / 600.0).sin() as f32;
            let r = (500.0 * (s.abs() as f64).sqrt().sqrt() as f32) as i32;
            self.shown = (500 + r * s.signum() as i32).clamp(0, 1000) as usize;
        }
        let snow = self.snow;
        for i in 0..self.shown {
            let q = &mut self.parts[i];
            q.p = std::array::from_fn(|k| q.p[k] + q.v[k]);
            if q.p[0] <= 0.0 {
                continue;
            }
            let rand = &mut self.rand;
            q.p[0] = -TENTH;
            rand.below(FIFTH);
            rand.below(FIFTH);
            q.p[1] = rand.below(FIFTH) - TENTH;
            q.p[2] = rand.below(FIFTH) - TENTH;
            if snow {
                q.size = (rand.below(50) / 30.0 + 1.0) * 2.0;
                q.v = [10.0, rand.below(6) - 3.0, rand.below(6) - 3.0];
            } else {
                q.v = [50.0, 17.0, 0.0];
            }
        }
    }

    pub fn snow(&self) -> bool {
        self.snow
    }

    /// The particles to draw, moved to the pivot `(0, ColonyXStart, −ColonyYStart)`: centre and half size (snow).
    pub fn flakes(&self, pivot: [f32; 2]) -> impl Iterator<Item = (V3, f32)> + '_ {
        self.parts[..self.shown].iter().map(move |q| ([q.p[0], q.p[1] + pivot[0], q.p[2] + pivot[1]], q.size))
    }

    /// `ShowRain`: each drop a 1-pixel `line3d2` from `P` to `P + (25, 8.5, 0)` (half a frame's fall), `P`'s end at
    /// alpha 0x10 and the lower end at 0x80 (the exe's streaks are brightest at the bottom), blended (`SRC_ALPHA`), depth-tested without writing it, unfogged.
    pub fn streaks(&self, pivot: [f32; 2]) -> impl Iterator<Item = (V3, V3)> + '_ {
        self.parts.iter().map(move |q| {
            let a = [q.p[0], q.p[1] + pivot[0], q.p[2] + pivot[1]];
            (a, [a[0] + 25.0, a[1] + 8.5, a[2]])
        })
    }
}

/// A screen-wide line between `a` and `b` as a thin quad facing the eye, `px` world units a pixel at distance 1, with
/// per-end colours (the engine's `Lines`).
pub fn line_quad(eye: V3, px: f32, a: V3, b: V3, ca: [f32; 4], cb: [f32; 4]) -> [MeshVertex; 6] {
    let mid: V3 = std::array::from_fn(|k| (a[k] + b[k]) / 2.0);
    let to_eye: V3 = std::array::from_fn(|k| eye[k] - mid[k]);
    let dist = (to_eye[0] * to_eye[0] + to_eye[1] * to_eye[1] + to_eye[2] * to_eye[2]).sqrt();
    let d: V3 = std::array::from_fn(|k| b[k] - a[k]);
    let side = crate::scene::normalise([d[1] * to_eye[2] - d[2] * to_eye[1], d[2] * to_eye[0] - d[0] * to_eye[2], d[0] * to_eye[1] - d[1] * to_eye[0]]);
    let w = dist * px / 2.0;
    let q = |p: V3, s: f32, c: [f32; 4]| MeshVertex { pos: std::array::from_fn(|k| p[k] + side[k] * w * s), normal: [0.0; 3], uv: [0.0; 2], color: c };
    [q(a, -1.0, ca), q(b, -1.0, cb), q(b, 1.0, cb), q(a, -1.0, ca), q(b, 1.0, cb), q(a, 1.0, ca)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snow_count_follows_the_counter() {
        let mut w = Weather::new(1).unwrap();
        w.step(0.0);
        assert_eq!(w.shown, 500);
        // sin(600π/2 / 600) = 1: all of them.
        w.step(600.0 * std::f32::consts::FRAC_PI_2);
        assert_eq!(w.shown, 1000);
        w.step(600.0 * 3.0 * std::f32::consts::FRAC_PI_2);
        assert_eq!(w.shown, 0);
    }

    #[test]
    fn drops_fall_and_start_again_at_the_top() {
        let mut w = Weather::new(2).unwrap();
        for _ in 0..40 {
            w.step(0.0);
        }
        assert!(w.parts.iter().all(|q| (-TENTH..=0.0).contains(&q.p[0]) && q.v == [50.0, 17.0, 0.0]));
    }
}
