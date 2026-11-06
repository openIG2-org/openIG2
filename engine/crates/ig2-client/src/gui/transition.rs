//! Ours (ADR 0084): a modern screen's own opening and closing, in place of the original's animations. Counted in the
//! screens' UI ticks (10 ms): the panels slide in from their sides and fade in ([`super::ds::entering`]), the game's
//! renders appear in their wells a little later ([`Transition::veil`]); closing runs it backwards, faster, and the
//! screen goes once it is out. Off with the reduced motion option: at once.

use crate::render::{kind, Blend, Frame, Instance, TexId};

/// The opening's and the closing's length in UI ticks.
pub const OPEN_TICKS: f32 = 30.0;
pub const CLOSE_TICKS: f32 = 18.0;
/// The renders start this far into the opening (and are gone this far before the end of the closing).
const RENDER_LAG: f32 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    /// How far the screen is in (0..1, linear).
    p: f32,
    closing: bool,
}

impl Transition {
    /// A screen starting to open.
    pub fn opening() -> Self {
        Self { p: 0.0, closing: false }
    }

    /// A screen at rest (snapshots).
    pub fn open() -> Self {
        Self { p: 1.0, closing: false }
    }

    /// One UI tick.
    pub fn tick(&mut self) {
        self.step(super::ds::anim(1.0) > 0.0);
    }

    /// One UI tick, with `motion` or at once.
    fn step(&mut self, motion: bool) {
        self.p = match (self.closing, motion) {
            (true, true) => (self.p - 1.0 / CLOSE_TICKS).max(0.0),
            (false, true) => (self.p + 1.0 / OPEN_TICKS).min(1.0),
            (true, false) => 0.0,
            (false, false) => 1.0,
        };
    }

    /// Start closing (again: nothing).
    pub fn close(&mut self) {
        self.closing = true;
    }

    pub fn closing(&self) -> bool {
        self.closing
    }

    /// The closing has run out: the screen goes.
    pub fn closed(&self) -> bool {
        self.closing && self.p <= 0.0
    }

    /// The panels' part (0..1, eased).
    pub fn panels(&self) -> f32 {
        ease(self.p)
    }

    /// The renders' part (0..1, eased): later than the panels when opening, earlier when closing.
    pub fn renders(&self) -> f32 {
        ease(((self.p - RENDER_LAG) / (1.0 - RENDER_LAG)).clamp(0.0, 1.0))
    }

    /// Fade the 3D drawn into `f` so far in by [`Self::renders`]: the screen's backdrop picture (or its black)
    /// over it at the rest. `size` is the band's.
    pub fn veil(&self, f: &mut Frame, backdrop: Option<(TexId, f32)>, white: TexId, size: [f32; 2]) {
        let a = 1.0 - self.renders();
        if a <= 0.0 {
            return;
        }
        let half = [size[0] / 2.0, size[1] / 2.0];
        let (tex, c) = backdrop.unwrap_or((white, 0.0));
        f.push(tex, Blend::GammaAlpha, Instance::new(kind::SPRITE, half, half, [c, c, c, a]));
    }
}

/// Smoothstep.
fn ease(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opening runs 30 ticks, the renders behind the panels; closing 18 (one more for the rounding), the renders out
    /// first; then closed.
    #[test]
    fn opens_and_closes() {
        let mut t = Transition::opening();
        assert_eq!((t.panels(), t.renders(), t.closed()), (0.0, 0.0, false));
        for _ in 0..8 {
            t.step(true);
        }
        assert!(t.panels() > 0.15 && t.renders() == 0.0);
        for _ in 0..22 {
            t.step(true);
        }
        assert!(t.panels() > 0.999 && t.renders() > 0.999);
        t.close();
        let mut n = 0;
        while !t.closed() {
            t.step(true);
            n += 1;
            assert!(t.renders() <= t.panels());
        }
        assert!((18..=19).contains(&n));
        assert!(!Transition::open().closed() && Transition::open().renders() == 1.0);
        // Reduced motion: at once.
        let mut t = Transition::opening();
        t.step(false);
        assert_eq!(t.panels(), 1.0);
        t.close();
        t.step(false);
        assert!(t.closed());
    }
}
