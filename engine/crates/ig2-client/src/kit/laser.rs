//! The lasers and sparks of the screens' opening faders (`DrawLaser`, `AddSpark`, `UpdateSparks`, `DrawSparks`).

use super::Virtual;
use crate::render::{kind, Blend, Frame, Instance, TexId};

/// The lasers `DrawLaser` (ELF 0x53c200) runs round a fader while it opens (`ResearchGfx2D_1`, `OptionsGfx2D_1`) and
/// the sparks they throw (`AddSpark` 0x53bcd0, `UpdateSparks` 0x53b9a0 every UI frame from `Frissit2_Grafika`,
/// `DrawSparks` 0x53b870). Cosmetic: their `random(n)` (`rand() % n`) runs on a local generator here.
#[derive(Debug, Clone, Default)]
pub struct Lasers {
    /// `Sparks[1500]`: place, speed, colour (r, g, b).
    sparks: Vec<([f32; 2], [f32; 2], [i32; 3])>,
    /// The laser frame drawn last: per rectangle its four heads, the beam colour, the heads' glow radii and the
    /// opacity (`255 − fade`).
    shots: Vec<Shot>,
    seed: u32,
}

#[derive(Debug, Clone)]
struct Shot {
    rect: [i32; 4],
    from: [i32; 2],
    t: f32,
    beam: [i32; 3],
    /// `DrawLaser`'s colour (the heads, their glow, the trailing edges).
    color: [i32; 3],
    radii: [[i32; 2]; 4],
    alpha: i32,
}

impl Lasers {
    fn random(&mut self, n: i32) -> i32 {
        // The MSVC `rand()` (cosmetic; the exe's draws aren't replayed).
        self.seed = self.seed.wrapping_mul(214013).wrapping_add(2531011);
        ((self.seed >> 16) & 0x7fff) as i32 % n.max(1)
    }

    /// `AddSpark(x, y, r, g, b)`: ten sparks in the first free slots, flying off at `(random(50)/5 − 2.5)/3`.
    fn add(&mut self, at: [i32; 2], c: [i32; 3]) {
        for _ in 0..10 {
            if self.sparks.len() >= 0x5dc {
                return;
            }
            let vx = (self.random(0x32) as f32 / 5.0 - 2.5) / 3.0;
            let vy = (self.random(0x32) as f32 / 5.0 - 2.5) / 3.0;
            self.sparks.push(([at[0] as f32, at[1] as f32], [vx, vy], c));
        }
    }

    /// `UpdateSparks` (one UI frame): they fly on, slow down by 0.93, fall by 0.04 and darken by 3 a channel; gone
    /// below a colour sum of 50 or at the screen's edge.
    pub fn tick(&mut self) {
        self.sparks.retain_mut(|(p, v, c)| {
            p[0] += v[0];
            p[1] += v[1];
            for x in c.iter_mut() {
                *x = (*x - 3).max(0);
            }
            v[0] *= 0.93;
            v[1] = v[1] * 0.93 + 0.04;
            c[0] + c[1] + c[2] >= 0x32 && p[0] > 1.0 && p[0] <= 638.0 && p[1] > 1.0 && p[1] <= 478.0
        });
    }

    /// No lasers this frame (each frame shoots its own).
    pub fn clear(&mut self) {
        self.shots.clear();
    }

    /// `DrawLaser(x, y, w, h, r, g, b, t)` from `LaserPosX/Y` = `from`: at `t` 0 or 1 nothing; the heads' glow fades
    /// in over the first tenth and out over the last (`(0.1 − t)·2550`), each head throws sparks.
    pub fn shoot(&mut self, rect: [i32; 4], from: [i32; 2], t: f32, color: [i32; 3]) {
        if t == 1.0 || t == 0.0 {
            return;
        }
        let fade = if t < 0.1 { ((0.1 - t) * 2550.0) as i32 } else if t > 0.9 { ((0.1 - (1.0 - t)) * 2550.0) as i32 } else { 0 };
        let beam = [color[0] / 5 + self.random(0xf), color[1] / 5 + self.random(0xf), color[2] / 5 + self.random(0xf)];
        let mut radii = [[0; 2]; 4];
        for (k, head) in heads(rect, t).into_iter().enumerate() {
            self.add(head, color);
            let n = self.random(10);
            radii[k] = [((n + 0xf) as f64 / 1.5) as i32, ((n + 5) as f64 / 1.5) as i32];
        }
        self.shots.push(Shot { rect, from, t, beam, color, radii, alpha: 0xff - fade });
    }

    /// Draw the lasers (additive: the fading edges `DrawFadingLineH`, the beams `line2d`, the heads' glow
    /// `laservago.bmp` × opacity) and the sparks (`PutPixelAdditivNew`).
    pub fn draw(&self, f: &mut Frame, white: TexId, glow: TexId, v: &Virtual, shots: bool, sparks: bool) {
        let rgb = |c: [i32; 3], k: f32| [c[0] as f32 * k / 255.0, c[1] as f32 * k / 255.0, c[2] as f32 * k / 255.0, 1.0];
        let pixel = |f: &mut Frame, x: f32, y: f32, c: [f32; 4]| {
            f.push(white, Blend::GammaAdditive, Instance::new(kind::SPRITE, v.px(x + 0.5, y + 0.5), [v.k / 2.0, v.k / 2.0], c));
        };
        for s in self.shots.iter().filter(|_| shots) {
            let [x, y, w, h] = s.rect;
            let (t, u) = (s.t, 1.0 - s.t);
            // From the head back to the start, the colour less 2 a pixel.
            let fading = |f: &mut Frame, from: i32, to: i32, y: i32| {
                let step = if from > to { 1 } else { -1 };
                let mut p = to;
                let mut c = s.color;
                while p != from {
                    pixel(f, p as f32, y as f32, rgb(c, 1.0));
                    c = c.map(|x| (x - 2).max(0));
                    p += step;
                }
            };
            // `DrawFadingLineV` (ELF 0x53c040) fills its pixel list but never draws it: only the top and bottom
            // edges trail [obs: no side tails on the exe's captures].
            fading(f, x, (x as f32 + w as f32 * t) as i32, y);
            fading(f, x + w, (x as f32 + w as f32 * u) as i32, y + h);
            let hs = heads(s.rect, t);
            let half = s.beam.map(|c| c >> 1);
            for (from, c) in [(s.from, s.beam), ([s.from[0] - 1, s.from[1]], half), ([s.from[0], s.from[1] - 1], half)] {
                for hd in hs {
                    let (a, b) = (v.px(from[0] as f32 + 0.5, from[1] as f32 + 0.5), v.px(hd[0] as f32 + 0.5, hd[1] as f32 + 0.5));
                    f.push(white, Blend::GammaAdditive, Instance::line(a, b, 1.0, rgb(c, 1.0)));
                }
            }
            let a = s.alpha as f32 / 256.0;
            for (hd, r) in hs.into_iter().zip(s.radii) {
                for (rad, c) in [(r[0], rgb(s.color, a)), (r[1], [a * 255.0 / 255.0; 4])] {
                    let c = [c[0], c[1], c[2], 1.0];
                    f.push(glow, Blend::GammaAdditive, Instance::new(kind::SPRITE, v.px(hd[0] as f32, hd[1] as f32), [rad as f32 * v.k, rad as f32 * v.k], c));
                }
            }
        }
        if sparks {
            for (p, _, c) in &self.sparks {
                pixel(f, p[0], p[1], rgb(*c, 1.0));
            }
        }
    }
}

/// The four laser heads on fader `r` at `t`: along the top from the left, the bottom from the right, the left side
/// from the bottom, the right side from the top.
fn heads(r: [i32; 4], t: f32) -> [[i32; 2]; 4] {
    let [x, y, w, h] = r;
    let u = 1.0 - t;
    [
        [(x as f32 + w as f32 * t) as i32, y],
        [(x as f32 + w as f32 * u) as i32, y + h],
        [x, (y as f32 + h as f32 * u) as i32],
        [x + w, (y as f32 + h as f32 * t) as i32],
    ]
}
