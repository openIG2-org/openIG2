//! The modern pointer set (ADR 0073 §5): the original's pointer types (`MOUSE.BMP`'s columns, re/screens/input.md
//! §8) and the pages' egui cursors, drawn from geometry (signed distances, no picture files) in the design's colours,
//! the animated ones (move, select, busy) in 20 frames over the original's 2-second cycle. They replace the game's
//! pointer quad only with a modern UI theme and the "Modern cursors" option on; Classic keeps `MOUSE.BMP`.

use super::Tokens;
use std::sync::atomic::{AtomicBool, Ordering};

/// One pointer of the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cursor {
    /// Type 0: the arrow (the original's blue-tailed dart).
    Arrow,
    /// Type 1: over something clickable (the original's red-tailed dart).
    Click,
    /// Type 2: a fleet's move order (the green diamond of arrows).
    Move,
    /// Type 3: placing a building (the colony).
    Build,
    /// Type 4: the green cross (a destination).
    Goto,
    /// Type 5: something to select (the white corners).
    Select,
    /// Type 6: attack (the red sight).
    Attack,
    /// Type 7: a target ring (redirect over a planet).
    Target,
    /// The pages' extras: waiting, a text field, not allowed, dragging.
    Busy,
    Text,
    Forbidden,
    Drag,
}

/// Animated pointers' frames over their cycle; one a tenth of a second (`MouseFazis` / 10).
pub const FRAMES: usize = 20;
/// The design box in points: the pointer's size at 100 % UI scale on a 1080-line window.
pub const BOX: f32 = 32.0;

impl Cursor {
    pub const ALL: [Cursor; 12] = [Cursor::Arrow, Cursor::Click, Cursor::Move, Cursor::Build, Cursor::Goto, Cursor::Select, Cursor::Attack, Cursor::Target, Cursor::Busy, Cursor::Text, Cursor::Forbidden, Cursor::Drag];

    /// The game's pointer type (`Mouse_Tipus`, 0..7) or one of [`Cursor::kind`]'s extras.
    pub fn from_kind(kind: i32) -> Cursor {
        usize::try_from(kind).ok().and_then(|k| if k < 8 { Self::ALL.get(k) } else { k.checked_sub(100).and_then(|e| Self::ALL.get(8 + e)) }).copied().unwrap_or(Cursor::Arrow)
    }

    /// The pointer type the overlay carries: the game's 0..7, 100.. for the extras (which Classic shows as the arrow or
    /// the hand, [`Cursor::classic_kind`]).
    pub fn kind(self) -> i32 {
        let i = Self::ALL.iter().position(|&c| c == self).unwrap_or(0) as i32;
        if i < 8 { i } else { 100 + i - 8 }
    }

    /// The `MOUSE.BMP` column Classic draws for it.
    pub fn classic_kind(self) -> i32 {
        match self {
            Cursor::Drag => 1,
            Cursor::Busy | Cursor::Text | Cursor::Forbidden => 0,
            c => c.kind(),
        }
    }

    /// An egui page's cursor (`PlatformOutput::cursor_icon`).
    pub fn from_icon(icon: egui::CursorIcon) -> Cursor {
        use egui::CursorIcon as I;
        match icon {
            I::PointingHand | I::ContextMenu | I::Alias | I::Copy => Cursor::Click,
            I::Text | I::VerticalText => Cursor::Text,
            I::Wait | I::Progress => Cursor::Busy,
            I::NotAllowed | I::NoDrop => Cursor::Forbidden,
            I::Grab | I::Grabbing | I::Move | I::AllScroll => Cursor::Drag,
            I::Crosshair | I::Cell => Cursor::Target,
            I::ResizeHorizontal | I::ResizeColumn | I::ResizeEast | I::ResizeWest => Cursor::Drag,
            _ => Cursor::Arrow,
        }
    }

    /// Its frames (1: still).
    pub fn frames(self) -> usize {
        if matches!(self, Cursor::Move | Cursor::Select | Cursor::Busy) { FRAMES } else { 1 }
    }

    /// The frame shown at `MouseFazis` `phase` (10 ms ticks).
    pub fn frame(self, phase: u64) -> usize {
        (phase / 10) as usize % self.frames()
    }

    /// The hot point in the 32-point box.
    pub fn hot(self) -> [f32; 2] {
        match self {
            Cursor::Arrow | Cursor::Click | Cursor::Busy => [3.0, 2.0],
            _ => [16.0, 16.0],
        }
    }

    /// Frame `frame` as `px` × `px` RGBA pixels (straight alpha), the 32-point box at `px / 32` pixels a point.
    pub fn rgba(self, frame: usize, px: usize, k: &Tokens) -> Vec<u8> {
        let t = frame as f32 / self.frames() as f32;
        let layers = self.layers(t, k);
        let ppu = px as f32 / BOX;
        let mut out = vec![0u8; px * px * 4];
        for (i, o) in out.chunks_exact_mut(4).enumerate() {
            let p = [((i % px) as f32 + 0.5) / ppu, ((i / px) as f32 + 0.5) / ppu];
            let mut acc = [0.0f32; 4];
            for l in &layers {
                let d = l.shape.dist(p);
                let a = match l.glow {
                    Some(r) => (-d.max(0.0) / r).exp() * (d < 4.0 * r) as u8 as f32,
                    None => (0.5 - d * ppu).clamp(0.0, 1.0),
                } * l.colour[3];
                for (v, c) in acc.iter_mut().zip(l.colour).take(3) {
                    *v = c * a + *v * (1.0 - a);
                }
                acc[3] = a + acc[3] * (1.0 - a);
            }
            for c in 0..3 {
                o[c] = if acc[3] > 0.0 { (acc[c] / acc[3] * 255.0).round() as u8 } else { 0 };
            }
            o[3] = (acc[3] * 255.0).round() as u8;
        }
        out
    }

    /// The pointer's drawing at cycle point `t` (0..1), back to front.
    fn layers(self, t: f32, k: &Tokens) -> Vec<Layer> {
        let col = |c: egui::Color32, a: f32| [c.r() as f32 / 255.0 * a, c.g() as f32 / 255.0 * a, c.b() as f32 / 255.0 * a, a];
        let dark = col(k.metal_shadow, 1.0);
        let shade = col(egui::Color32::BLACK, 0.35);
        let pulse = 0.5 - 0.5 * (t * std::f32::consts::TAU).cos();
        let mut v = Vec::new();
        // A shape with its dark outline, `w` thick, and a soft drop shade.
        let outlined = |v: &mut Vec<Layer>, s: Shape, fill: [f32; 4], w: f32| {
            v.push(Layer { shape: Shape::Offset(Box::new(s.clone()), -w - 0.6, [0.8, 1.2]), colour: shade, glow: None });
            v.push(Layer { shape: Shape::Offset(Box::new(s.clone()), -w, [0.0, 0.0]), colour: dark, glow: None });
            v.push(Layer { shape: s, colour: fill, glow: None });
        };
        let silver = col(k.ink_silver, 1.0);
        let ink = col(k.ink, 1.0);
        match self {
            Cursor::Arrow | Cursor::Click | Cursor::Busy => {
                let dart = Shape::Poly(vec![[3.0, 2.0], [8.5, 25.0], [11.8, 16.6], [20.5, 20.0]]);
                outlined(&mut v, dart.clone(), silver, 1.4);
                // The lit leading edge and the coloured tail, inside the dart.
                v.push(Layer { shape: Shape::Inside(Box::new(dart.clone()), Box::new(Shape::Poly(vec![[3.0, 2.0], [8.5, 25.0], [6.4, 25.0], [1.0, 2.0]]))), colour: ink, glow: None });
                let tail = if self == Cursor::Click { k.signal_red } else { k.info };
                v.push(Layer { shape: Shape::Inside(Box::new(dart), Box::new(Shape::Poly(vec![[0.0, 30.0], [8.0, 15.0], [24.0, 15.0], [24.0, 30.0]]))), colour: col(tail, 1.0), glow: None });
                if self == Cursor::Click {
                    v.insert(0, Layer { shape: Shape::Disc([12.0, 20.0], 4.0), colour: col(k.signal_red, 0.45), glow: Some(3.0) });
                }
                if self == Cursor::Busy {
                    let a = t * std::f32::consts::TAU;
                    outlined(&mut v, Shape::Ring([23.0, 24.0], 4.6, 1.3), col(k.metal_dark, 1.0), 1.0);
                    v.push(Layer { shape: Shape::Arc([23.0, 24.0], 4.6, 1.3, a, 2.2), colour: col(k.focus, 1.0), glow: None });
                }
            }
            Cursor::Move => {
                let r = 6.0 + 2.0 * pulse;
                let green = col(k.own, 1.0);
                for q in 0..4 {
                    let tri = |a: f32| {
                        let (s, c) = a.sin_cos();
                        let at = |x: f32, y: f32| [16.0 + c * x - s * y, 16.0 + s * x + c * y];
                        Shape::Poly(vec![at(r + 7.0, 0.0), at(r, -5.0), at(r, 5.0)])
                    };
                    outlined(&mut v, tri(q as f32 * std::f32::consts::FRAC_PI_2), green, 1.2);
                }
            }
            Cursor::Build => {
                let tower = Shape::Poly(vec![[9.0, 27.0], [11.0, 13.0], [16.0, 5.0], [21.0, 13.0], [23.0, 27.0]]);
                outlined(&mut v, tower.clone(), silver, 1.3);
                v.push(Layer { shape: Shape::Inside(Box::new(tower), Box::new(Shape::Poly(vec![[16.0, 4.0], [24.0, 4.0], [24.0, 28.0], [16.0, 28.0]]))), colour: col(k.metal_light, 1.0), glow: None });
                outlined(&mut v, Shape::Poly(vec![[6.0, 27.0], [26.0, 27.0], [26.0, 29.5], [6.0, 29.5]]), col(k.info, 1.0), 1.0);
            }
            Cursor::Goto => {
                let green = col(k.own, 1.0);
                outlined(&mut v, Shape::Segment([16.0, 5.0], [16.0, 27.0], 1.2), green, 1.0);
                outlined(&mut v, Shape::Segment([5.0, 16.0], [27.0, 16.0], 1.2), green, 1.0);
                for (a, b, c) in [([16.0, 3.0], [13.0, 8.0], [19.0, 8.0]), ([16.0, 29.0], [13.0, 24.0], [19.0, 24.0]), ([3.0, 16.0], [8.0, 13.0], [8.0, 19.0]), ([29.0, 16.0], [24.0, 13.0], [24.0, 19.0])] {
                    outlined(&mut v, Shape::Poly(vec![a, b, c]), green, 1.0);
                }
                outlined(&mut v, Shape::Disc([16.0, 16.0], 2.4), green, 1.0);
            }
            Cursor::Select => {
                let r = 9.0 + 2.0 * pulse;
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    let c = [16.0 + sx * r, 16.0 + sy * r];
                    outlined(&mut v, Shape::Segment(c, [c[0] - sx * 5.0, c[1]], 1.0), ink, 1.0);
                    outlined(&mut v, Shape::Segment(c, [c[0], c[1] - sy * 5.0], 1.0), ink, 1.0);
                }
            }
            Cursor::Attack => {
                let red = col(k.enemy, 1.0);
                v.push(Layer { shape: Shape::Ring([16.0, 16.0], 8.0, 1.0), colour: col(k.signal_red, 0.6), glow: Some(2.0) });
                outlined(&mut v, Shape::Ring([16.0, 16.0], 8.0, 1.0), red, 1.0);
                for (a, b) in [([16.0, 2.0], [16.0, 11.0]), ([16.0, 21.0], [16.0, 30.0]), ([2.0, 16.0], [11.0, 16.0]), ([21.0, 16.0], [30.0, 16.0])] {
                    outlined(&mut v, Shape::Segment(a, b, 0.9), red, 1.0);
                }
                outlined(&mut v, Shape::Disc([16.0, 16.0], 1.4), red, 0.8);
            }
            Cursor::Target => {
                let red = col(k.enemy, 1.0);
                outlined(&mut v, Shape::Ellipse([16.0, 16.0], [11.0, 7.5], 1.1), red, 1.0);
                outlined(&mut v, Shape::Segment([16.0, 4.0], [16.0, 9.0], 0.9), red, 1.0);
                outlined(&mut v, Shape::Segment([16.0, 23.0], [16.0, 28.0], 0.9), red, 1.0);
                outlined(&mut v, Shape::Disc([16.0, 16.0], 2.6), red, 1.0);
            }
            Cursor::Text => {
                outlined(&mut v, Shape::Segment([16.0, 7.0], [16.0, 25.0], 0.9), ink, 1.1);
                for y in [6.0, 26.0] {
                    outlined(&mut v, Shape::Segment([12.5, y], [19.5, y], 0.9), ink, 1.1);
                }
            }
            Cursor::Forbidden => {
                let red = col(k.signal_red, 1.0);
                outlined(&mut v, Shape::Ring([16.0, 16.0], 9.0, 1.6), red, 1.0);
                outlined(&mut v, Shape::Segment([10.0, 22.0], [22.0, 10.0], 1.6), red, 1.0);
            }
            Cursor::Drag => {
                outlined(&mut v, Shape::Segment([16.0, 6.0], [16.0, 26.0], 1.1), silver, 1.0);
                outlined(&mut v, Shape::Segment([6.0, 16.0], [26.0, 16.0], 1.1), silver, 1.0);
                for (a, b, c) in [([16.0, 2.5], [12.0, 8.0], [20.0, 8.0]), ([16.0, 29.5], [12.0, 24.0], [20.0, 24.0]), ([2.5, 16.0], [8.0, 12.0], [8.0, 20.0]), ([29.5, 16.0], [24.0, 12.0], [24.0, 20.0])] {
                    outlined(&mut v, Shape::Poly(vec![a, b, c]), silver, 1.0);
                }
            }
        }
        v
    }
}

/// One coloured layer: a shape filled (anti-aliased) or glowing (`glow`: its falloff in points).
struct Layer {
    shape: Shape,
    colour: [f32; 4],
    glow: Option<f32>,
}

/// The pointers' geometry as signed distances in points (negative inside).
#[derive(Clone)]
enum Shape {
    Poly(Vec<[f32; 2]>),
    Disc([f32; 2], f32),
    /// Centre, radius, half width.
    Ring([f32; 2], f32, f32),
    /// A ring's arc from angle `a` over `len` radians.
    Arc([f32; 2], f32, f32, f32, f32),
    Ellipse([f32; 2], [f32; 2], f32),
    /// A line with round ends, half width.
    Segment([f32; 2], [f32; 2], f32),
    /// Grown by `-d` (d < 0 grows), moved by the offset.
    Offset(Box<Shape>, f32, [f32; 2]),
    /// The second shape's part inside the first.
    Inside(Box<Shape>, Box<Shape>),
}

fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (pa, ba) = ([p[0] - a[0], p[1] - a[1]], [b[0] - a[0], b[1] - a[1]]);
    let h = ((pa[0] * ba[0] + pa[1] * ba[1]) / (ba[0] * ba[0] + ba[1] * ba[1]).max(1e-6)).clamp(0.0, 1.0);
    ((pa[0] - ba[0] * h).powi(2) + (pa[1] - ba[1] * h).powi(2)).sqrt()
}

impl Shape {
    fn dist(&self, p: [f32; 2]) -> f32 {
        let len = |x: f32, y: f32| (x * x + y * y).sqrt();
        match self {
            Shape::Poly(v) => {
                let mut d = f32::MAX;
                let mut inside = false;
                for i in 0..v.len() {
                    let (a, b) = (v[i], v[(i + 1) % v.len()]);
                    d = d.min(seg_dist(p, a, b));
                    if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                        inside = !inside;
                    }
                }
                if inside { -d } else { d }
            }
            Shape::Disc(c, r) => len(p[0] - c[0], p[1] - c[1]) - r,
            Shape::Ring(c, r, w) => (len(p[0] - c[0], p[1] - c[1]) - r).abs() - w,
            Shape::Arc(c, r, w, a, l) => {
                let ang = (p[1] - c[1]).atan2(p[0] - c[0]);
                let rel = (ang - a).rem_euclid(std::f32::consts::TAU);
                if rel <= *l {
                    (len(p[0] - c[0], p[1] - c[1]) - r).abs() - w
                } else {
                    let end = |t: f32| [c[0] + r * t.cos(), c[1] + r * t.sin()];
                    let (e0, e1) = (end(*a), end(a + l));
                    len(p[0] - e0[0], p[1] - e0[1]).min(len(p[0] - e1[0], p[1] - e1[1])) - w
                }
            }
            Shape::Ellipse(c, r, w) => {
                // Distance to the ring, scaled by the radii (close enough for thin rings).
                let (x, y) = ((p[0] - c[0]) / r[0], (p[1] - c[1]) / r[1]);
                (len(x, y) - 1.0).abs() * r[0].min(r[1]) - w
            }
            Shape::Segment(a, b, w) => seg_dist(p, *a, *b) - w,
            Shape::Offset(s, d, o) => s.dist([p[0] - o[0], p[1] - o[1]]) + d,
            Shape::Inside(a, b) => a.dist(p).max(b.dist(p)),
        }
    }
}

/// The set on the GPU: each pointer's frames (64 pixels across: sharp up to a 2160-line window at 100 %).
pub struct Set(Vec<Vec<crate::render::TexId>>);

impl Set {
    pub const PIXELS: usize = 64;

    pub fn load(r: &mut crate::render::Renderer) -> Self {
        let k = Tokens::DEEP_SPACE;
        let n = Self::PIXELS;
        Self(Cursor::ALL.iter().map(|c| (0..c.frames()).map(|i| r.add_texture_linear(&crate::assets::Image { width: n as u32, height: n as u32, rgba: c.rgba(i, n, &k), mips: Vec::new(), hires: None })).collect()).collect())
    }

    /// Pointer type `kind` ([`Cursor::from_kind`]) at phase `phase`, its hot point at `at` (pixels), `scale` pixels a
    /// point (the pages' [`crate::gui::Gui::scale`]).
    pub fn draw(&self, f: &mut crate::render::Frame, kind: i32, phase: u64, at: [f32; 2], scale: f32) {
        let c = Cursor::from_kind(kind);
        let Some(&tex) = self.0.get(Cursor::ALL.iter().position(|&x| x == c).unwrap_or(0)).and_then(|v| v.get(c.frame(phase))) else { return };
        let k = scale.max(0.5);
        let [hx, hy] = c.hot();
        let half = BOX * k / 2.0;
        let centre = [at[0] + (BOX / 2.0 - hx) * k, at[1] + (BOX / 2.0 - hy) * k];
        f.push(tex, crate::render::Blend::GammaAlpha, crate::render::Instance::new(crate::render::kind::SPRITE, centre, [half, half], [1.0; 4]));
    }
}

/// The "Modern cursors" option (`ModernCursors`, on by default); shown only with a modern UI theme.
static MODERN: AtomicBool = AtomicBool::new(true);

pub fn set_modern(on: bool) {
    MODERN.store(on, Ordering::Relaxed);
}

/// The modern set replaces the game's pointer: the option is on and a theme is chosen (not Classic).
pub fn modern() -> bool {
    MODERN.load(Ordering::Relaxed) && crate::theme::current().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's types and the extras round-trip through the overlay's number; Classic shows the extras as the
    /// arrow or the hand; the pages' cursors map.
    #[test]
    fn kinds_round_trip() {
        for c in Cursor::ALL {
            assert_eq!(Cursor::from_kind(c.kind()), c);
            assert!((0..8).contains(&c.classic_kind()));
        }
        assert_eq!((Cursor::from_kind(1), Cursor::from_kind(6), Cursor::from_kind(9), Cursor::from_kind(-1)), (Cursor::Click, Cursor::Attack, Cursor::Arrow, Cursor::Arrow));
        assert_eq!(Cursor::from_icon(egui::CursorIcon::PointingHand), Cursor::Click);
        assert_eq!(Cursor::from_icon(egui::CursorIcon::Text).kind(), 101);
    }

    /// Every frame draws something with soft edges, its hot point covered (or next to it) and the corners clear; the
    /// animated ones move.
    #[test]
    fn frames_draw() {
        let k = Tokens::DEEP_SPACE;
        for c in Cursor::ALL {
            let px = 64;
            let a = c.rgba(0, px, &k);
            let alpha = |img: &[u8], x: usize, y: usize| img[(y * px + x) * 4 + 3];
            let opaque = a.chunks_exact(4).filter(|p| p[3] == 255).count();
            let soft = a.chunks_exact(4).filter(|p| p[3] > 0 && p[3] < 255).count();
            assert!(opaque > 40 && soft > 40, "{c:?}: {opaque} {soft}");
            assert_eq!(alpha(&a, px - 1, 0), 0, "{c:?}");
            let [hx, hy] = c.hot().map(|v| (v * px as f32 / BOX) as usize);
            let near = (hx.saturating_sub(3)..hx + 4).any(|x| (hy.saturating_sub(3)..hy + 4).any(|y| alpha(&a, x, y) > 0));
            assert!(near || matches!(c, Cursor::Move | Cursor::Select), "{c:?}");
            if c.frames() > 1 {
                assert_ne!(a, c.rgba(c.frames() / 2, px, &k), "{c:?}");
            }
        }
    }
}
