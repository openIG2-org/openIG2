//! The animated row frame `DesignKeret1` shared by the research lists and the info screen.

use super::Virtual;
use crate::render::{kind, Blend, Frame, Instance, TexId};

/// The device-pixel nudge that gives a device row centred on a virtual row edge to the row above.
const ROW_EDGE: f32 = 0.01;

/// `DesignKeret1` (ELF 0x53f780): the animated frame around a chosen row (the research lists, the info screen's
/// race names). The frame morphs to a new place: `dkmcel = π/2` starts it, `Frissit2_Grafika` (ELF 0x5481c0, every
/// UI frame) runs `dkm` up by 0.05 to π/2 (the old frame fades out and widens), jumps to the new place and runs back
/// down to 0 (it fades in); `GfxNovelo` (+0.02 a frame) runs a light around its border.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignFrame {
    /// `dkm`, `dkmcel`, the frame shown (`dkoldx/y/sz/ma`) and asked for (`dkujx/y/sz/ma`), `GfxNovelo`.
    m: f32,
    target: f32,
    old: [i32; 4],
    new: [i32; 4],
    novelo: f32,
    /// How much of the widening goes to the left: a third (`DesignKeret1`) or a half (`DesignKeret2`).
    left: f32,
    /// `DesignKeretAzonnalMorph` (exe 0xad0ef5): the next new place is taken at once, no morph.
    now: bool,
}

impl Default for DesignFrame {
    /// As `Frissit2_Grafika` leaves it on a new screen (`DesignKeret1OldKep != AktKep`): no frame yet.
    fn default() -> Self {
        Self { m: 0.0, target: 0.0, old: [0, 0, 1, 1], new: [0; 4], novelo: 0.0, left: 3.0, now: false }
    }
}

impl DesignFrame {
    /// `DesignKeret2` (ELF 0x541940 / exe 0x4cd4c0, the design screen's pod and tech rows): the same frame on its
    /// own state (`dk2m`, `dk2old*`, `dk2uj*`), widened half to the left.
    pub fn second() -> Self {
        Self { left: 2.0, ..Self::default() }
    }

    /// The frame on a new screen with `dkm`, `dkmcel` and `GfxNovelo` carried over (globals the exe never resets).
    pub fn with_phase(m: f32, target: f32, novelo: f32) -> Self {
        Self { m, target, novelo, ..Self::default() }
    }

    /// `dkm`, `dkmcel`, `GfxNovelo`: the globals every screen's frame shares.
    pub fn phase(&self) -> [f32; 3] {
        [self.m, self.target, self.novelo]
    }

    /// The same frame on another screen (`Frissit2_Grafika`'s `DesignKeret1OldKep != AktKep`: the places reset).
    pub fn new_screen(&self) -> Self {
        Self::with_phase(self.m, self.target, self.novelo)
    }

    /// `DesignKeret1(x, y, w, h)`'s bookkeeping (exe 0x4cbac0): a new place starts the morph, or with
    /// [`Self::at_once`] set is shown at once; the flag clears either way.
    pub fn ask(&mut self, r: [i32; 4]) {
        if r != self.new {
            self.new = r;
            if self.now {
                self.old = r;
            } else {
                self.target = std::f32::consts::FRAC_PI_2;
            }
        }
        self.now = false;
    }

    /// `DesignKeretAzonnalMorph = 1` (the main menu's scroll-bar drag, `MainScr_Main` box 0x1c): the next call's new
    /// place jumps in without the morph.
    pub fn at_once(&mut self) {
        self.now = true;
    }

    /// `Frissit2_Grafika`'s step (one UI frame).
    pub fn tick(&mut self) {
        if self.target <= self.m {
            if self.m <= self.target {
                self.m = std::f32::consts::FRAC_PI_2;
                if self.old[0] != 0 {
                    self.m = self.target;
                    (self.old, self.target) = (self.new, 0.0);
                }
            } else {
                self.m -= 0.05;
                if self.m <= self.target {
                    self.m = self.target;
                }
            }
        } else {
            self.m += 0.05;
            if self.target <= self.m {
                self.m = self.target;
                (self.old, self.target) = (self.new, 0.0);
            }
        }
        self.novelo += 0.02;
    }

    /// Draw it in colour `c` (`DDColor[1]` × `KiirStrSzinSzorzo`, gamma 0..1): per row a glow (half the colour times
    /// a triangle peaking mid-height, fading out over the first and last tenth), and the border as 18 one-pixel
    /// segments, each `frac((sin(GfxNovelo + k·20°) + 1)/2)` bright; all additive (`line3(…, true)`), scaled by
    /// `1 − sin(dkm)` and widened by `w·sin(dkm)` (a third of it to the left).
    pub fn draw(&self, f: &mut Frame, white: TexId, v: &Virtual, c: [f32; 3]) {
        let fade = 1.0 - (self.m as f64).sin() as f32;
        let [x0, y0, w0, h0] = self.old;
        let w = (w0 as f32 + w0 as f32 * (1.0 - fade)) as i32;
        let x = (x0 as f32 - (w as f32 * (1.0 - fade)) / self.left) as i32;
        let (y, h) = (y0 - 1, h0 + 1);
        let byte = |k: f32| c.map(|v| ((v * 255.0).round() * k).trunc().clamp(0.0, 255.0) / 255.0);
        let mut seg = |a: [i32; 2], b: [i32; 2], ca: [f32; 3], cb: [f32; 3]| {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let (n, xmajor) = if dx.abs() >= dy.abs() { (dx.abs(), true) } else { (dy.abs(), false) };
            for k in 0..n {
                let t = (k as f32 + 0.5) / n as f32;
                let (px, py) = if xmajor {
                    (if dx > 0 { a[0] + k } else { a[0] - 1 - k }, (a[1] as f32 + t * dy as f32).floor() as i32)
                } else {
                    ((a[0] as f32 + t * dx as f32).floor() as i32, if dy > 0 { a[1] + k } else { a[1] - 1 - k })
                };
                let col: [f32; 3] = std::array::from_fn(|i| ca[i] + (cb[i] - ca[i]) * t);
                if col.iter().any(|&v| v > 0.0) {
                    // Each virtual pixel fills its own device rectangle; a device row whose centre lies exactly on a
                    // virtual row's top edge (every 5th at 800x600) goes to the row above in the exe [obs:
                    // `mainscr-desc.json` t 1000, rows 122/127/132/137], hence the nudge down.
                    let [cx, cy] = v.px(px as f32 + 0.5, py as f32 + 0.5);
                    f.push(white, Blend::GammaAdditive, Instance::new(kind::SPRITE, [cx, cy + ROW_EDGE], [v.k / 2.0, v.k / 2.0], [col[0], col[1], col[2], 1.0]));
                }
            }
        };
        let zero = [0.0; 3];
        let tenth = (w as f64 / 10.0) as i32;
        for i in 0..h {
            let k = (1.0 - (h as f32 / 2.0 - i as f32).abs() / (h as f32 / 2.0)) * fade;
            let row = c.map(|v| (((v * 255.0).round() * k) / 2.0).trunc() / 255.0);
            // `TscVonal(x, y, row, x + w/10, y + 1, 0)`, …: `line3` gives each segment's first colour to its second
            // end [obs: dark at both ends, bright inside].
            seg([x, y + i], [x + tenth, y + i + 1], zero, row);
            seg([x + tenth, y + i], [x + w - tenth, y + i + 1], row, row);
            seg([x + w - tenth, y + i], [x + w, y + i + 1], row, zero);
        }
        let (xe, ye) = (x - 1 + w, y - 1 + h);
        let q = |n: i32| x + 4 + n * (w - 8) / 4;
        let border: [([i32; 2], [i32; 2]); 18] = [
            ([q(3), y], [x - 4 + w, y + 1]),
            ([q(2), y], [q(3), y + 1]),
            ([q(1), y], [q(2), y + 1]),
            ([x + 4, y], [q(1), y + 1]),
            ([x + 4, y + 1], [x + 2, y + 2]),
            ([x + 1, y + 2], [x + 2, y + 3]),
            ([x, y + 3], [x + 1, h - 3 + y]),
            ([x + 1, ye - 2], [x + 2, ye - 1]),
            ([x + 4, ye - 1], [x + 2, ye]),
            ([x + 4, ye], [q(1), ye + 1]),
            ([q(1), ye], [q(2), ye + 1]),
            ([q(2), ye], [q(3), ye + 1]),
            ([q(3), ye], [x - 4 + w, ye + 1]),
            ([xe - 3, ye - 1], [xe - 1, ye]),
            ([xe - 1, ye - 2], [xe, ye - 1]),
            ([xe, y + 3], [xe + 1, h - 3 + y]),
            ([xe - 1, y + 2], [xe, y + 3]),
            ([xe - 3, y + 1], [xe - 1, y + 2]),
        ];
        for (k, (a, b)) in border.into_iter().enumerate() {
            let s = ((self.novelo as f64 + k as f64 * std::f64::consts::PI / 9.0).sin() + 1.0) / 2.0;
            let col = byte(fade * (s % 1.0) as f32);
            seg(a, b, col, col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Row 362: from the exe's boot values (`dkm` 0, `dkmcel` π/2) the phase saw-tooths π/2 → 0 while no place was
    /// asked, so a first ask almost always morphs in; only one landing on `dkm == π/2` stays hidden. `at_once` (the
    /// scroll drag) takes the place without a morph.
    #[test]
    fn first_place_and_at_once() {
        let mut d = DesignFrame::with_phase(0.0, std::f32::consts::FRAC_PI_2, 0.0);
        for _ in 0..50 {
            d.tick();
        }
        assert!(d.m < std::f32::consts::FRAC_PI_2 && d.target == 0.0 && d.old[0] == 0);
        for _ in 0..80 {
            d.ask([437, 67, 165, 15]);
            d.tick();
        }
        assert_eq!((d.old, d.m, d.target), ([437, 67, 165, 15], 0.0, 0.0));
        d.at_once();
        d.ask([437, 97, 165, 15]);
        assert_eq!((d.old, d.target), ([437, 97, 165, 15], 0.0));
        d.ask([437, 112, 165, 15]);
        assert_eq!((d.old[1], d.target), (97, std::f32::consts::FRAC_PI_2), "the flag clears after one call");
    }
}
