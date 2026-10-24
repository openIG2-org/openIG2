//! The hot-spot tooltip every screen shows (re/screens/spy.md §6.1).

use super::Virtual;
use crate::font::{Align, Font};
use crate::render::{Frame, TexId};

/// A hot-spot's tooltip, as every screen shows it: `UpdateFrame` (ELF 0x578e40) picks the hot-spot under the pointer
/// (`AktIkonKiir`, the lowest id of several) once per drawn frame, `Mouse_Lekerdez` restarts the wait on a move of
/// more than a pixel, and `GFX_DateMoneyScrollFade` (ELF 0x66d160, fader pass 3) draws the box with the text over
/// everything (re/screens/spy.md §6.1).
#[derive(Debug, Clone, Copy, Default)]
pub struct Tooltip {
    /// The hot-spot shown (`OldIkonKiir`, 0 none) and its place (`Mouse_Kiiras_x/y`).
    pub shown: u32,
    pub at: [i32; 2],
    /// The enabled hot-spot under the pointer at the last update (`AktIkon`, 0 none): `UpdateFrame`'s pointer takes its
    /// cursor type (`Mouse_Szoveg_Set`'s last argument, 1 the hand on every screen's spots; re/screens/input.md §8).
    pub under: u32,
    /// The pointer (virtual pixels), its last move and the clock (`LastMouseMove`, `AktIdo`, ms).
    pub mouse: [f32; 2],
    last_move: f64,
    now: f64,
}

impl Tooltip {
    /// The pointer is at `at` (`Mouse_Lekerdez`).
    pub fn moved(&mut self, at: [f32; 2]) {
        if (at[0] - self.mouse[0]).abs() as i32 > 1 || (at[1] - self.mouse[1]).abs() as i32 > 1 {
            self.last_move = self.now;
        }
        self.mouse = at;
    }

    /// Once per drawn frame at clock `now` (ms): `under` = the enabled hot-spot under the pointer (0 none) and the
    /// length of its text. It shows once the pointer rested 400 ms (or stays on the one shown); a held button hides
    /// it and restarts the wait. The box follows the pointer, kept on the 640-pixel screen (`len·6 + 0x18` wide).
    pub fn update(&mut self, now: f64, under: u32, len: usize, held: bool) {
        self.now = now;
        self.under = under;
        if under < 1 || (now <= self.last_move + 400.0 && under != self.shown) || held {
            self.shown = 0;
            if held {
                self.last_move = now;
            }
        } else {
            let w = len as i32 * 6 + 0x18;
            let [x, y] = [self.mouse[0] as i32, self.mouse[1] as i32];
            self.at = [if x + w > 0x280 { 0x280 - w } else { x }, y];
            self.shown = under;
        }
    }

    /// [`Self::update`] without the rest and the held button: the starmap's radial icons show their names at once
    /// (`UpdateFrame`'s `AktRopIkon2` case).
    pub fn update_now(&mut self, now: f64, under: u32, len: usize) {
        self.last_move = f64::MIN;
        self.update(now, under, len, false);
    }

    /// The box: `FadeKerettelKerekit(x + 15, y − 1, len·6 + 10, 13)` in (0, 10, 30) at 30 with a (0, 13, 31) edge at
    /// 15, the text at `(x + 21, y + 2)`, 99 cells, size `0x1a % 8` = 2, left, clipped to the screen, colour 2.
    pub fn draw(&self, f: &mut Frame, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], text: &[u8]) {
        let s = text.split(|&b| b == 0).next().unwrap_or_default();
        if self.shown == 0 || s.is_empty() {
            return;
        }
        let [x, y] = self.at.map(|c| c as f32);
        crate::kit::fader::fader(f, white, v, [x + 15.0, y - 1.0, s.len() as f32 * 6.0 + 10.0, 13.0], [0.0, 10.0, 30.0], 30.0, [0.0, 13.0, 31.0], 15.0);
        let c = palette.get(2).copied().unwrap_or([1.0; 3]);
        font.text(f, v, s, x + 21.0, y + 2.0, 99, 2, Align::Left, c, Some(v.x_range()));
    }
}
