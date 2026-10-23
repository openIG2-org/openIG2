//! A screen's `faderstruktura` (re/screens/hud.md §1.2.1): its faders' shine, their textures' drift and the hover
//! glow `FadeKerettelKerekitDesign` draws over them.

use super::Virtual;
use crate::render::{kind, Blend, Frame, Instance, TexId};
use crate::starmap::MsvcRand;

/// The faders a structure holds (`KirakFader` ignores `i >= 0x1e`).
pub const N: usize = 0x1e;

/// `faderstruktura` (0x5cc bytes; `optf`, `resf`, `desf`, …).
#[derive(Debug, Clone)]
pub struct FaderDesign {
    /// `+0x000`: ticks to the next shine.
    wait: [f32; N],
    /// `+0x078`: the shine's ticks left.
    shine_left: [f32; N],
    /// `+0x0f0`: the shine, 0..1.
    shine: [f32; N],
    /// `+0x168`: the callers' colour scale `s`.
    s: [f32; N],
    /// `+0x1e0`, `+0x258`: the fill texture's offsets.
    u: [f32; N],
    v: [f32; N],
    /// `+0x2d0`: `v`'s drift a tick.
    dv: [f32; N],
    /// `+0x348`: the shine jitters `v` (0) or `u` (1).
    axis: [i32; N],
    /// `+0x3e0`: the glow's countdown, 1 → 0.
    glow: [f32; N],
    /// `+0x458`: the glow's style 1..5.
    style: [i32; N],
    /// `+0x4d0`: the last fader whose glow started.
    last: i32,
    /// `+0x4d8`: +0.003 a tick (the scan bands, the info heads' pulse).
    clock: f32,
    /// The exe's one `rand()` stream (cosmetic: its draws aren't replayed).
    rng: MsvcRand,
}

impl FaderDesign {
    /// `InitFaderDesignStruktura` (ELF 0x547cf0, exe 0x4d3590), drawing from a `rand()` stream seeded with `seed`.
    pub fn new(seed: u32) -> Self {
        Self::from_rng(MsvcRand(seed))
    }

    /// `InitFaderDesignStruktura` right after `self`'s (`Starmap_Belep`: `fifo`, then `starf`): the one `rand()`
    /// stream goes on where `self` left it.
    pub fn next(&self) -> Self {
        Self::from_rng(self.rng.clone())
    }

    fn from_rng(mut rng: MsvcRand) -> Self {
        let (mut wait, mut dv) = ([0.0; N], [0.0; N]);
        for i in 0..N {
            wait[i] = rng.random(1000) as f32;
            dv[i] = (rng.random(10) + 1) as f32 / 30.0;
        }
        Self {
            wait,
            shine_left: [0.0; N],
            shine: [0.0; N],
            s: [0.0; N],
            u: [0.0; N],
            v: [0.0; N],
            dv,
            axis: [0; N],
            glow: [0.0; N],
            style: [0; N],
            last: -1,
            clock: 0.0,
            rng,
        }
    }

    /// `FaderDesignFrissito` (ELF 0x547e10): one UI tick.
    pub fn tick(&mut self) {
        self.clock = (self.clock as f64 + 0.003) as f32;
        for i in 0..N {
            if self.glow[i] > 0.0 {
                self.glow[i] = (self.glow[i] as f64 - 0.01) as f32;
                if self.glow[i] < 0.0 {
                    self.glow[i] = 0.0;
                }
            }
            if self.shine_left[i] > 0.0 {
                if self.shine[i] < 1.0 {
                    self.shine[i] = (self.shine[i] as f64 + 0.1) as f32;
                }
                let d = (self.rng.rand() % 100 - 50) as f32;
                match self.axis[i] {
                    0 => self.v[i] += d,
                    1 => self.u[i] += d,
                    _ => {}
                }
            }
            if self.shine[i] > 0.0 {
                self.shine[i] = ((self.shine[i] as f64 - 0.02) as f32).max(0.0);
            }
            if self.wait[i] <= 0.0 {
                if self.shine_left[i] <= 0.0 {
                    self.wait[i] = (self.rng.random(1000) + 1000) as f32;
                } else {
                    self.shine_left[i] -= 1.0;
                }
            } else {
                self.wait[i] -= 1.0;
                if self.wait[i] == 0.0 {
                    self.shine_left[i] = (self.rng.random(50) + 50) as f32;
                    self.axis[i] = self.rng.random(2);
                }
            }
            self.v[i] += self.dv[i];
            // `GLOBALKELLDESIGN & 1` is 1.
            self.s[i] = (self.shine[i] as f64 + 0.9) as f32;
        }
    }

    /// `KirakFader`'s glow start: the pointer `at` strictly inside `r`, `free` = `(!Mouse_Bal && !Mouse_Jobb &&
    /// !Mouse_Felenged) || Mouse_Lenyom`, not the last box entered, no glow running.
    pub fn enter(&mut self, i: usize, r: [f32; 4], at: [f32; 2], free: bool) {
        if i >= N {
            return;
        }
        let [x, y, w, h] = r;
        let inside = x < at[0] && at[0] < x + w && y < at[1] && at[1] < y + h;
        if inside && free && self.last != i as i32 && self.glow[i] == 0.0 {
            self.last = i as i32;
            self.glow[i] = 1.0;
            self.style[i] = self.rng.random(5) + 1;
        }
    }

    /// `+0x168 + 4i`: the colour scale, 0.9 and up to 1.9 while it shines.
    pub fn s(&self, i: usize) -> f32 {
        self.s.get(i).copied().unwrap_or(0.0)
    }

    /// `(+0x1e0, +0x258) + 4i`: the fill texture's offsets ([`super::fader::kirak_fader`]).
    pub fn offset(&self, i: usize) -> [f32; 2] {
        if i < N {
            [self.u[i], self.v[i]]
        } else {
            [0.0; 2]
        }
    }

    /// The glow running on fader `i`: `(t, style)` with `t = 1 − g`.
    pub fn glow(&self, i: usize) -> Option<(f32, i32)> {
        (i < N && self.glow[i] != 0.0).then(|| (1.0 - self.glow[i], self.style[i]))
    }

    /// `+0x4d8`.
    pub fn clock(&self) -> f32 {
        self.clock
    }
}

/// `fade_oldx/oldy/oldsz/oldma`: the last box whose glow got past 0.8 (one for the process, as in the exe).
static FADE_OLD: std::sync::Mutex<[i32; 4]> = std::sync::Mutex::new([0; 4]);

/// `AktIdo200`: on for 200 ms, off for 200 ms.
pub fn blink(now_ms: f64) -> bool {
    (now_ms / 200.0) as i64 % 2 == 0
}

/// `FadeKerettelKerekitDesign(x, y, w, h, r, g, b, …, t, 1.0, style)` (ELF 0x545b60): the glow's one-pixel lines
/// (`TscVonal`, `line2d` additive) in colour `c` (0..255); style 0 is the scan band at `t`.
#[allow(clippy::too_many_arguments, clippy::approx_constant)]
pub fn glow(f: &mut Frame, white: TexId, v: &Virtual, r: [i32; 4], c: [i32; 3], t: f32, style: i32, blink: bool) {
    if !c.iter().any(|&k| k > 1) {
        return;
    }
    let [x, y, w, h] = r;
    let cf = c.map(|k| k as f32);
    let mut l = |a: [f32; 2], b: [f32; 2], ca: [f32; 3], cb: [f32; 3]| line(f, white, v, a, b, ca, cb);
    let black = [0.0; 3];
    match style {
        0 => {
            let (xf, wf, hf) = (x as f32, w as f32, h as f32);
            let mut yo = 0;
            while yo < h {
                let yf = yo as f32;
                let d = (t - yf / hf).abs();
                let mut a = 0.0f32;
                if d < 0.1 {
                    a = (0.1 - d) * 1.0 / 0.1;
                    if yf < 0.1 * hf {
                        a /= (hf * 0.1) / yf;
                    }
                    if hf - 0.1 * hf < yf {
                        a *= (hf - yf) / (hf * 0.1);
                    }
                }
                if a as f64 >= 0.1 {
                    let k = cf.map(|p| (p * a / 2.0) as i32 as f32);
                    let yy = (y + yo) as f32;
                    let six = (w / 6) as f32;
                    l([xf, yy], [xf + six, yy], black, k);
                    l([xf + six, yy], [xf + wf - six, yy], k, k);
                    l([xf + wf - six, yy], [xf + wf, yy], k, black);
                }
                yo += 4;
            }
        }
        1 => {
            let (xf, yf, wf, hf) = (x as f32, y as f32, w as f32, h as f32);
            let td = t as f64;
            let p = if t < 0.25 {
                [xf + wf * t * 4.0, yf]
            } else if t < 0.5 {
                [xf + wf, (yf as f64 + hf as f64 * (td - 0.25) * 4.0) as f32]
            } else if t < 0.75 {
                [((xf + wf) as f64 - wf as f64 * (td - 0.5) * 4.0) as f32, yf + hf]
            } else if t < 1.0 {
                [xf, ((yf + hf) as f64 - hf as f64 * (td - 0.75) * 4.0) as f32]
            } else {
                [0.0; 2]
            };
            let [px, py] = p.map(|q| q as i32 as f32);
            let k = c.map(|q| (q as f64 / 1.2) as i32 as f32);
            let s = (10 + ((td * 3.1415 * 10.0).sin() * 5.0) as i32) as f32;
            l([px, 0.0], [px, 479.0], k, k);
            l([0.0, py], [639.0, py], k, k);
            l([px - s, py - s], [px + s, py - s], k, k);
            l([px - s, py + s], [px + s, py + s], k, k);
            l([px - s, py - s], [px - s, py + s], k, k);
            l([px + s, py - s], [px + s, py + s], k, k);
        }
        2 => {
            let (k, o) = if (t as f64) < 0.5 {
                (cf.map(|p| (p * t * 2.0) as i32 as f32), ((0.5 - t as f64) * 200.0) as i32)
            } else {
                (if blink { cf } else { black }, 0)
            };
            let (x0, x1, y0, y1) = ((x - o) as f32, (x + w + o) as f32, (y - o) as f32, (y + h + o) as f32);
            l([x0, 0.0], [x0, 479.0], k, k);
            l([x1, 0.0], [x1, 479.0], k, k);
            l([0.0, y0], [639.0, y0], k, k);
            l([0.0, y1], [639.0, y1], k, k);
        }
        3 => {
            if blink || t < 0.5 {
                let d = if t < 0.5 { ((0.5 - t as f64) * 200.0) as f32 } else { 0.0 };
                let (xf, yf, wf, hf) = (x as f32, y as f32, w as f32, h as f32);
                let corners = [
                    ([xf - d, yf - d], -1.0, -1.0),
                    ([xf + wf + d, yf - d], 1.0, -1.0),
                    ([xf + wf + d, yf + hf + d], 1.0, 1.0),
                    ([xf - d, yf + hf + d], -1.0, 1.0),
                ];
                for ([vx, vy], sx, sy) in corners {
                    let (vx, vy) = (vx as i32 as f32, vy as i32 as f32);
                    l([vx + 20.0 * sx, vy], [vx, vy], cf, cf);
                    l([vx, vy + 20.0 * sy], [vx, vy], cf, cf);
                }
            }
        }
        4 => {
            if blink || t < 0.5 {
                let (xf, yf, wf, hf) = (x as f32, y as f32, w as f32, h as f32);
                let (x0, y0, x1, y1) = if t < 0.5 {
                    let q = (0.5 - t as f64) as f32;
                    (
                        t * xf * 2.0 + q * (xf - 300.0) * 2.0,
                        t * yf * 2.0 + q * (yf - 300.0) * 2.0,
                        t * (xf + wf) * 2.0 + q * (xf + wf + 300.0) * 2.0,
                        // `y + w`: the exe's slip (asm 0x5471d1).
                        t * (yf + hf) * 2.0 + q * (yf + wf + 300.0) * 2.0,
                    )
                } else {
                    (xf, yf, xf + wf, yf + hf)
                };
                l([x0, yf], [x0 + wf, yf], cf, cf);
                l([xf + wf, y0], [xf + wf, y0 + hf], cf, cf);
                l([x1, yf + hf], [x1 - wf, yf + hf], cf, cf);
                l([xf, y1], [xf, y1 - hf], cf, cf);
            }
        }
        5 => {
            let [ox, oy, osz, oma] = *FADE_OLD.lock().unwrap();
            let u = 1.0 - t;
            let (xf, yf, wf, hf) = (x as f32, y as f32, w as f32, h as f32);
            let x0 = xf * t + u * ox as f32;
            let y0 = yf * t + u * oy as f32;
            let x1 = (xf + wf) * t + u * (ox + osz) as f32;
            let y1 = (yf + hf) * t + u * (oy + oma) as f32;
            l([x0, y0], [x1, y0], cf, cf);
            l([x1, y0], [x1, y1], cf, cf);
            l([x1, y1], [x0, y1], cf, cf);
            l([x0, y1], [x0, y0], cf, cf);
        }
        _ => {}
    }
    if style != 0 && t as f64 > 0.8 {
        *FADE_OLD.lock().unwrap() = r;
    }
}

/// `TscVonal` + `line2d` for the glow's level and upright lines: the ends truncated to whole virtual pixels, one
/// screen pixel thick, additive, the colour (0..255) running from `ca` to `cb`.
fn line(f: &mut Frame, white: TexId, v: &Virtual, a: [f32; 2], b: [f32; 2], ca: [f32; 3], cb: [f32; 3]) {
    let (a, b) = (a.map(|q| q as i32 as f32), b.map(|q| q as i32 as f32));
    let level = a[1] == b[1];
    let len = if level { b[0] - a[0] } else { b[1] - a[1] };
    let n = if ca == cb { 1 } else { (len.abs() as usize).max(1) };
    for i in 0..n {
        let (t0, t1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        let tm = (t0 + t1) / 2.0;
        let c: [f32; 3] = std::array::from_fn(|k| (ca[k] + (cb[k] - ca[k]) * tm) / 255.0);
        if c.iter().all(|&q| q <= 0.0) {
            continue;
        }
        let (s0, s1) = (len * t0, len * t1);
        let (center, half) = if level {
            let p = v.px(a[0] + (s0 + s1) / 2.0, a[1]);
            ([p[0], p[1] + 0.5], [(s1 - s0).abs() * v.k / 2.0, 0.5])
        } else {
            let p = v.px(a[0], a[1] + (s0 + s1) / 2.0);
            ([p[0] + 0.5, p[1]], [0.5, (s1 - s0).abs() * v.k / 2.0])
        };
        f.push(white, Blend::GammaAdditive, Instance::new(kind::SPRITE, center, half, [c[0], c[1], c[2], 1.0]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glow_runs_about_a_second() {
        let mut d = FaderDesign::new(1);
        d.enter(3, [10.0, 10.0, 20.0, 20.0], [15.0, 15.0], true);
        assert_eq!(d.glow(3).map(|g| g.0), Some(0.0));
        assert!((1..=5).contains(&d.glow(3).unwrap().1));
        let mut n = 0;
        while d.glow(3).is_some() {
            d.tick();
            n += 1;
        }
        assert!((100..=101).contains(&n), "{n}");
        // The same box again: not until another one has glowed.
        d.enter(3, [10.0, 10.0, 20.0, 20.0], [15.0, 15.0], true);
        assert!(d.glow(3).is_none());
        d.enter(4, [40.0, 10.0, 20.0, 20.0], [45.0, 15.0], true);
        d.enter(3, [10.0, 10.0, 20.0, 20.0], [15.0, 15.0], true);
        assert!(d.glow(3).is_some());
    }

    #[test]
    fn enter_is_strict_and_waits_for_the_buttons() {
        let mut d = FaderDesign::new(1);
        d.enter(0, [10.0, 10.0, 20.0, 20.0], [10.0, 15.0], true);
        d.enter(0, [10.0, 10.0, 20.0, 20.0], [15.0, 15.0], false);
        assert!(d.glow(0).is_none());
    }

    /// ig2.exe, Options (`_local/traces-faderglow`, `tools/trace/probes/fader.js`, 2026-10-23): fader 1's glow from
    /// the tick the pointer entered the save button, and fader 2's shine from the tick its wait ran out (70 ticks).
    #[test]
    #[allow(clippy::excessive_precision)]
    fn glow_and_shine_as_the_exe() {
        let mut d = FaderDesign::new(1);
        d.wait = [5000.0; N];
        d.enter(1, [452.0, 62.0, 78.0, 33.0], [480.0, 80.0], true);
        let mut g = vec![];
        for n in 1..=100 {
            d.tick();
            if [1, 2, 3, 50, 99, 100].contains(&n) {
                g.push(d.glow[1]);
            }
        }
        assert_eq!(g, [0.99, 0.98, 0.97, 0.5000004768371582, 0.01000065915286541, 6.591528745047981e-07]);
        d.tick();
        assert_eq!(d.glow(1), None);

        let (mut d, i) = (FaderDesign::new(1), 2);
        d.wait = [5000.0; N];
        (d.wait[i], d.shine_left[i]) = (0.0, 70.0);
        let mut s = vec![];
        for _ in 0..13 {
            d.tick();
            s.push(d.s(i));
        }
        assert_eq!(s, [0.9800000190734863, 1.059999942779541, 1.1399999856948853, 1.2200000286102295, 1.2999999523162842,
            1.3799999952316284, 1.4600000381469727, 1.540000081062317, 1.6200001239776611, 1.7000001668930054,
            1.7800002098083496, 1.8600002527236938, 1.9400001764297485]);
        for _ in 13..70 {
            d.tick();
        }
        assert_eq!((d.s(i), d.shine_left[i]), (1.9000015258789062, 0.0));
        d.tick();
        assert_eq!(d.s(i), 1.8800015449523926);
        assert!((1000.0..2000.0).contains(&d.wait[i]));
        for _ in 0..49 {
            d.tick();
        }
        assert_eq!(d.s(i), 0.9000019431114197);
        d.tick();
        assert_eq!(d.s(i), 0.9);
    }

    #[test]
    fn s_is_0_9_without_a_shine() {
        let mut d = FaderDesign::new(1);
        assert_eq!(d.s(0), 0.0);
        d.tick();
        assert_eq!(d.s(0), 0.9);
    }
}
