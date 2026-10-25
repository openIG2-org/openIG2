//! The remaster's screen layout (re/screens/widescreen.md, ADR 0010): `ig2_resize` (exe 0x4d84d0, ELF 0x550050) puts
//! the 640 × 480 virtual screen into the window, `ig2_render_first` (exe 0x4d7fe0) maps it over the GL viewport and
//! `ModifyViewPort` (exe 0x4ac4f0, ELF 0x4f6df0) widens the 3D range to the window's sides.

use std::sync::atomic::{AtomicBool, Ordering};

/// The window's layout, as `ig2_resize` computes it without the toolbar (all C integer arithmetic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// `IG2_WIDTH/HEIGHT`: the whole window.
    pub size: [i32; 2],
    /// `IG2_WINDOW_POSX/POSY/WIDTH/HEIGHT`: the GL viewport (y from the bottom, `glViewport`'s).
    pub window: [i32; 4],
    /// `IG2_VIRTUAL_POSX/POSY/WIDTH/HEIGHT`: the 4:3 frame in window pixels (y from the top, the mouse's).
    pub frame: [i32; 4],
}

/// `ModifyViewPort`'s 3D range in virtual pixels (`HatarX1`, `HatarX2`, `HatarY1`, `HatarY2`, `HatarXm`, `HatarYm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hatar {
    pub x1: i32,
    pub x2: i32,
    pub y1: i32,
    pub y2: i32,
    pub xm: i32,
    pub ym: i32,
}

impl Layout {
    /// `ig2_resize` for a `w × h` window (`sdl_width/height`).
    pub fn new(w: i32, h: i32) -> Self {
        let (w, h) = (w.max(1), h.max(1));
        // The test is in f32: `1.3333334 <= (float)width / (float)height`.
        let (window, frame) = if 4.0f32 / 3.0 <= w as f32 / h as f32 {
            let vw = h * 640 / 480;
            ([0, 0, w, h], [(w - vw) / 2, 0, vw, h])
        } else {
            let vh = w * 480 / 640;
            let y = (h - vh) / 2;
            ([0, y, w, vh], [0, y, w, vh])
        };
        Self { size: [w, h], window, frame }
    }

    /// The viewport in window pixels from the top (`x, y, w, h`): the band everything is drawn in.
    pub fn band(&self) -> [i32; 4] {
        let [x, y, w, h] = self.window;
        [x, self.size[1] - y - h, w, h]
    }

    /// `ModifyViewPort`: `m = ((W·480)/H − 640)/2` virtual pixels each side of the frame.
    pub fn hatar(&self) -> Hatar {
        let [_, _, w, h] = self.window;
        let m = ((w * 480) / h.max(1) - 640) / 2;
        let (x1, x2) = (m * -639 / 639, (m + 639) * 639 / 639);
        Hatar { x1, x2, y1: 0, y2: 479, xm: x2 - x1 + 1, ym: 480 }
    }

    /// `MainCallBackEventHandler`: a window pixel → virtual pixels through the frame, unclamped.
    pub fn mouse(&self, p: [i32; 2]) -> [i32; 2] {
        // A minimised or display-off window has a 0-wide frame: no division by zero.
        let [x, y, w, h] = self.frame;
        [(p[0] - x) * 640 / w.max(1), (p[1] - y) * 480 / h.max(1)]
    }
}

/// The "letterbox" option (ADR 0010 §2, off by default): the 3D scenes and background pictures stop at the 4:3 frame.
static LETTERBOX: AtomicBool = AtomicBool::new(false);

pub fn set_letterbox(on: bool) {
    LETTERBOX.store(on, Ordering::Relaxed);
}

pub fn letterbox() -> bool {
    LETTERBOX.load(Ordering::Relaxed)
}

/// Under [`letterbox`], black over the side strips beside the frame: the 3D scenes and pictures stop at its edges
/// while the 2D layer, which stays inside it, is untouched.
pub fn letterbox_bars(f: &mut crate::render::Frame, l: &Layout, white: crate::render::TexId) {
    use crate::render::{kind, Blend, Instance};
    let [fx, _, fw, _] = l.frame;
    let [_, _, w, h] = l.band();
    if !letterbox() || fw >= w {
        return;
    }
    for (x0, x1) in [(0, fx), (fx + fw, w)] {
        let (cx, hw) = ((x0 + x1) as f32 / 2.0, (x1 - x0) as f32 / 2.0);
        f.push(white, Blend::Alpha, Instance::new(kind::SPRITE, [cx, h as f32 / 2.0], [hw, h as f32 / 2.0], [0.0, 0.0, 0.0, 1.0]));
    }
}

/// A snapshot of the band (RGBA8, `band()`'s size) put on the window as [`Layout`] and [`letterbox`] say: black bars
/// above and below a narrow window's band, black side strips under the option.
pub fn compose(l: &Layout, band: &[u8]) -> Vec<u8> {
    let [w, h] = l.size.map(|c| c as usize);
    let [bx, by, bw, bh] = l.band().map(|c| c as usize);
    let mut out = vec![0u8; w * h * 4];
    for (i, px) in out.chunks_exact_mut(4).enumerate() {
        px[3] = 255;
        let (x, y) = (i % w, i / w);
        if (bx..bx + bw).contains(&x) && (by..by + bh).contains(&y) {
            let j = 4 * ((y - by) * bw + (x - bx));
            px.copy_from_slice(&band[j..j + 4]);
        }
    }
    let [fx, _, fw, _] = l.frame.map(|c| c as usize);
    if letterbox() && fw < bw {
        for (i, px) in out.chunks_exact_mut(4).enumerate() {
            if !(fx..fx + fw).contains(&(i % w)) {
                px.copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    out
}

/// The virtual screen → pixels of the viewport it is drawn in (`ig2_render_first`'s ortho matrix: 480 lines high,
/// centred, so 1 virtual pixel = `H/480` pixels).
#[derive(Debug, Clone, Copy)]
pub struct Virtual {
    pub k: f32,
    pub x0: f32,
    /// The band's top in the given pixels (0 unless the size is narrower than 4:3).
    pub y0: f32,
    layout: Layout,
}

impl Virtual {
    /// For a viewport of `size` pixels (the window's [`Layout::band`]).
    pub fn new(size: [f32; 2]) -> Self {
        let layout = Layout::new(size[0].round() as i32, size[1].round() as i32);
        let [_, y0, _, wh] = layout.band();
        let k = if layout.window[3] == layout.size[1] { size[1] / 480.0 } else { wh as f32 / 480.0 };
        Self { k, x0: size[0] / 2.0 - 320.0 * k, y0: y0 as f32, layout }
    }

    pub fn px(&self, x: f32, y: f32) -> [f32; 2] {
        [self.x0 + x * self.k, self.y0 + y * self.k]
    }

    /// Pixels → virtual pixels, as the exe maps the mouse ([`Layout::mouse`]: whole pixels, truncated, unclamped).
    pub fn virtual_of(&self, p: [f32; 2]) -> [f32; 2] {
        self.layout.mouse(p.map(|c| c.floor() as i32)).map(|c| c as f32)
    }

    /// Pixels → virtual pixels in floats (the 3D picking rays).
    pub fn virtual_f(&self, p: [f32; 2]) -> [f32; 2] {
        [(p[0] - self.x0) / self.k, (p[1] - self.y0) / self.k]
    }

    /// `HatarX1..X2`: the x range text clipped "to the screen" may use (`KiirStr`'s flag 8).
    pub fn x_range(&self) -> (f32, f32) {
        let h = self.layout.hatar();
        (h.x1 as f32, h.x2 as f32)
    }
}

/// An offscreen 3D target over the virtual screen: as wide as the viewport (the exe clips its 3D at `HatarX1..X2`,
/// which GL widens to the viewport), or the 4:3 frame under [`letterbox`].
#[derive(Debug, Clone, Copy)]
pub struct Wide {
    /// The target's pixels.
    pub px: (u32, u32),
    /// Its width over the frame's (`px.0 / 640k`).
    pub sx: f32,
}

impl Wide {
    pub fn new(v: &Virtual) -> Self {
        let [_, _, bw, bh] = v.layout.band();
        let frame = (640.0 * v.k).round().max(1.0);
        let w = if letterbox() { frame } else { (bw as f32).max(frame) };
        Self { px: (w as u32, bh.max(1) as u32), sx: w / (640.0 * v.k) }
    }

    /// A target over the 4:3 frame alone: the info and messages screens' frame models stop at its edges in ig2.exe
    /// at 16:9 [obs: `k10-1920x1080`, re/screens/widescreen.md §3].
    pub fn frame(v: &Virtual) -> Self {
        let [_, _, _, bh] = v.layout.band();
        Self { px: ((640.0 * v.k).round().max(1.0) as u32, bh.max(1) as u32), sx: 1.0 }
    }

    /// A clip matrix's rows over the virtual 640 × 480 → over the target (x divided by `sx`).
    pub fn rows(&self, mut rows: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
        rows[0] = rows[0].map(|c| c / self.sx);
        rows
    }

    /// The target drawn back: its centre and half size in pixels.
    pub fn half(&self, v: &Virtual) -> [f32; 2] {
        [320.0 * v.k * self.sx, 240.0 * v.k]
    }

    /// The u coordinate of virtual x.
    pub fn u(&self, x: f32) -> f32 {
        (x - 320.0) / (640.0 * self.sx) + 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// re/screens/widescreen.md §2: the layout globals read out of ig2.exe (`layout_probe.py`) at six window sizes.
    #[test]
    fn layout_matches_the_exe() {
        for (w, h, win, frame, hatar) in [
            (800, 600, [0, 0, 800, 600], [0, 0, 800, 600], (0, 639, 640)),
            (1920, 1080, [0, 0, 1920, 1080], [240, 0, 1440, 1080], (-106, 745, 852)),
            (2520, 1080, [0, 0, 2520, 1080], [540, 0, 1440, 1080], (-240, 879, 1120)),
            (1366, 768, [0, 0, 1366, 768], [171, 0, 1024, 768], (-106, 745, 852)),
            (1280, 1024, [0, 32, 1280, 960], [0, 32, 1280, 960], (0, 639, 640)),
            (1000, 1000, [0, 125, 1000, 750], [0, 125, 1000, 750], (0, 639, 640)),
        ] {
            let l = Layout::new(w, h);
            assert_eq!((l.window, l.frame), (win, frame), "{w}×{h}");
            let t = l.hatar();
            assert_eq!((t.x1, t.x2, t.xm, t.y1, t.y2, t.ym), (hatar.0, hatar.1, hatar.2, 0, 479, 480), "{w}×{h}");
        }
    }

    #[test]
    fn the_band_is_the_viewport_from_the_top_and_the_mouse_maps_through_the_frame() {
        let l = Layout::new(1000, 1001);
        // glViewport's y counts from the bottom: (1001 − 750)/2 = 125 below, 126 above.
        assert_eq!(l.band(), [0, 126, 1000, 750]);
        let l = Layout::new(1920, 1080);
        assert_eq!(l.mouse([240, 0]), [0, 0]);
        assert_eq!(l.mouse([1679, 1079]), [639, 479]);
        // Unclamped, truncated towards zero in the side strips.
        assert_eq!(l.mouse([0, 0]), [-106, 0]);
        assert_eq!(l.mouse([1919, 0]), [746, 0]);
        // A minimised window: a 1 × 1 size gives a 0-high frame, no panic.
        let l = Layout::new(0, 0);
        assert_eq!(l.frame[3], 0);
        let _ = (l.mouse([5, 5]), l.hatar());
    }

    #[test]
    fn virtual_keeps_the_old_mapping_for_wide_windows() {
        let v = Virtual::new([1920.0, 1080.0]);
        assert_eq!((v.k, v.x0, v.y0), (2.25, 240.0, 0.0));
        let v = Virtual::new([800.0, 600.0]);
        assert_eq!((v.k, v.x0, v.y0), (1.25, 0.0, 0.0));
        assert_eq!(v.virtual_of([1.0, 799.0 / 1.25]), [0.0, 511.0]);
        // A narrow size: the 480 lines are the band's.
        let v = Virtual::new([1280.0, 1024.0]);
        assert_eq!((v.k, v.x0, v.y0), (2.0, 0.0, 32.0));
    }

    #[test]
    fn a_narrow_snapshot_gets_bars_above_and_below() {
        let l = Layout::new(4, 5);
        // 4 × 3 band at y = 1 (from the top: 5 − 1 − 3 = 1).
        assert_eq!(l.band(), [0, 1, 4, 3]);
        let band = vec![200u8; 4 * 3 * 4];
        let out = compose(&l, &band);
        let row = |y: usize| out[y * 16..y * 16 + 16].to_vec();
        assert_eq!(row(0), [0, 0, 0, 255].repeat(4));
        assert_eq!(row(1), vec![200u8; 16]);
        assert_eq!(row(4), [0, 0, 0, 255].repeat(4));
    }

    #[test]
    fn wide_targets_span_the_viewport_unless_letterboxed() {
        let v = Virtual::new([1920.0, 1080.0]);
        let t = Wide::new(&v);
        assert_eq!(t.px, (1920, 1080));
        assert_eq!(t.half(&v), [960.0, 540.0]);
        assert!(t.u(-106.0 - 2.0 / 3.0).abs() < 1e-6);
        let v = Virtual::new([800.0, 600.0]);
        let t = Wide::new(&v);
        assert_eq!((t.px, t.sx), ((800, 600), 1.0));
    }
}
