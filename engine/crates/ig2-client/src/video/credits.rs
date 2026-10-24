//! The remaster's credits line over `credits.bik` (re/screens/video.md §3, `ig2screen::render` exe 0x7370f0): the
//! exe's own string, rendered once by `PlatformUtils::RenderText` (exe 0x6e9130) into a 4-bit alpha texture and drawn
//! 1:1 in window pixels, centred at (W/2, 0.96·H) over a black shadow at +2, +2 (four fifths of the text's alpha), for
//! the clip's first 12288 ms. Its font name is empty, so `GetFont` (exe 0x6e84b0) finds none and
//! `RenderTextToSurface` (0x6e9670) takes the Windows GDI fallback (exe 0x6f0e90): Arial (video.md §5.4).

use crate::assets::Image;

/// The text's start in ig2.exe (`"REMASTERED BY "`, read up to its NUL).
const MARK: &[u8] = b"REMASTERED BY ";
const FONT: &str = "fonts/jurabook.ttf";

/// The credits line from the user's ig2.exe.
pub fn text(install: &ig2_formats::Install) -> Option<String> {
    let exe = std::fs::read(install.root().join("ig2.exe")).ok()?;
    let at = exe.windows(MARK.len()).position(|w| w == MARK)?;
    let end = exe[at..].iter().position(|&b| b == 0)?;
    String::from_utf8(exe[at..at + end].to_vec()).ok()
}

/// The text's alpha at `t` ms of the clip (`None` from 0x3000 on): `t >> 4` up to 0x1000, 255 to 0x2000, then
/// `255 − ((t − 0x2000) >> 4)`; the byte arithmetic of 0x7371d0.
pub fn alpha(t: u64) -> Option<u8> {
    if t >= 0x3000 {
        return None;
    }
    Some(if t < 0x1000 { (t >> 4) as u8 } else if t > 0x2000 { 255 - ((t - 0x2000) >> 4) as u8 } else { 255 })
}

/// The shadow's alpha: `(a · 0x333333334) >> 0x22`, four fifths rounded down.
pub fn shadow_alpha(a: u8) -> u8 {
    ((a as u64 * 0x3_3333_3334) >> 0x22) as u8
}

/// The text's texture for a window `h` pixels high: `RenderText(text, w = (h>>5)<<6, h = h>>5, size = h>>5, flags
/// 0xc0)` [obs: 1152, 18, 18, 0xc0 at 600 lines], both sides rounded up to 4 (the packed font texture group), white
/// with the coverage's top 4 bits as alpha. On Windows the exe's own GDI call ([`gdi`]); elsewhere an outline stand-in.
pub fn render(install: &ig2_formats::Install, text: &str, h: u32) -> Option<Image> {
    #[cfg(windows)]
    if let Some(img) = gdi::render(text, h >> 5) {
        return Some(img);
    }
    outline(install, text, h)
}

/// Without GDI: the install's `fonts/jurabook.ttf` fitted to the exe's line (em `1.25·size`, width 0.84 of that).
fn outline(install: &ig2_formats::Install, text: &str, h: u32) -> Option<Image> {
    use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
    let bytes = install.read(FONT).ok()?;
    let font = FontRef::try_from_slice(&bytes).ok()?;
    let size = h >> 5;
    let up4 = |v: u32| if v == 0 { 0 } else { (v - 1) / 4 * 4 + 4 };
    let (tw, th) = (up4(size << 6), up4(size));
    if tw == 0 {
        return None;
    }
    let em_y = size as f32 * 1.25;
    let units = font.height_unscaled() / font.units_per_em()?;
    let scale = PxScale { x: em_y * 0.84 * units, y: em_y * units };
    let sf = font.as_scaled(scale);
    let mut glyphs = Vec::new();
    let mut x = 0.0f32;
    let mut prev = None;
    for c in text.chars() {
        let id = sf.glyph_id(c);
        if let Some(p) = prev {
            x += sf.kern(p, id);
        }
        glyphs.push(id.with_scale_and_position(scale, ab_glyph::point(x, 0.0)));
        x += sf.h_advance(id);
        prev = Some(id);
    }
    // Centred across the box; the baseline at the ascent, which puts the capitals on rows 3..16 of the 20 at 800×600 as in the exe.
    let left = ((tw as f32 - x) / 2.0).floor();
    let base = sf.ascent().round() - 1.0;
    let mut rgba = vec![0u8; (tw * th * 4) as usize];
    for g in glyphs {
        let mut g = g;
        g.position.x += left;
        g.position.y += base;
        let Some(o) = font.outline_glyph(g) else { continue };
        let b = o.px_bounds();
        o.draw(|gx, gy, cov| {
            let (px, py) = (b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32);
            if px < 0 || py < 0 || px >= tw as i32 || py >= th as i32 {
                return;
            }
            let i = ((py as u32 * tw + px as u32) * 4) as usize;
            let a = ((cov.clamp(0.0, 1.0) * 255.0) as u32 >> 4) * 17;
            rgba[i..i + 3].copy_from_slice(&[255, 255, 255]);
            rgba[i + 3] = rgba[i + 3].max(a as u8);
        });
    }
    Some(Image { width: tw, height: th, rgba })
}

/// The GDI fallback `FUN_006f0e90` (exe 0x6f0e90) with flags 0xc0: a screen-compatible bitmap `(w + 3) & !3` × `h`,
/// black with white text; `CreateFontA(size, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, ANTIALIASED_QUALITY, 0, "Arial")` (a
/// positive height: the cell); `DrawTextW(…, (0, 0, w, h), DT_CENTER)`, top-aligned; `GetDIBits` at 24 bits, the blue
/// byte of the first `h` rows as the coverage. [obs: at 600 lines 457 px wide, capitals on rows 3..=14; the exe's
/// capture 456 wide.]
#[cfg(windows)]
mod gdi {
    use crate::assets::Image;
    use std::ffi::c_void;

    type Handle = *mut c_void;

    #[repr(C)]
    struct Rect {
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    }

    #[repr(C)]
    struct BitmapInfo {
        size: u32,
        w: i32,
        h: i32,
        planes: u16,
        bits: u16,
        rest: [u32; 6],
        colours: [u32; 1],
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDC(w: Handle) -> Handle;
        fn ReleaseDC(w: Handle, dc: Handle) -> i32;
        fn FillRect(dc: Handle, r: *const Rect, brush: Handle) -> i32;
        fn DrawTextW(dc: Handle, text: *const u16, n: i32, r: *mut Rect, format: u32) -> i32;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(dc: Handle) -> Handle;
        fn CreateCompatibleBitmap(dc: Handle, w: i32, h: i32) -> Handle;
        fn SelectObject(dc: Handle, o: Handle) -> Handle;
        fn GetStockObject(i: i32) -> Handle;
        fn SetTextColor(dc: Handle, c: u32) -> u32;
        fn SetBkColor(dc: Handle, c: u32) -> u32;
        #[allow(clippy::too_many_arguments)]
        fn CreateFontA(h: i32, w: i32, esc: i32, orient: i32, weight: i32, italic: u32, underline: u32, strike: u32, charset: u32, out: u32, clip: u32, quality: u32, pitch: u32, face: *const u8) -> Handle;
        fn GetDIBits(dc: Handle, bm: Handle, start: u32, lines: u32, bits: *mut u8, info: *mut BitmapInfo, usage: u32) -> i32;
        fn DeleteObject(o: Handle) -> i32;
        fn DeleteDC(dc: Handle) -> i32;
    }

    pub fn render(text: &str, size: u32) -> Option<Image> {
        let (w, h) = ((size << 6) as i32, size as i32);
        if h == 0 {
            return None;
        }
        let cx = (w + 3) & !3;
        let wide: Vec<u16> = text.encode_utf16().chain([0]).collect();
        let row = ((cx * 3 + 3) & !3) as usize;
        let mut bits = vec![0u8; row * h as usize];
        // SAFETY: plain GDI calls on objects made here and released before returning; the buffers outlive the calls.
        let drawn = unsafe {
            let screen = GetDC(std::ptr::null_mut());
            let dc = CreateCompatibleDC(screen);
            let bm = CreateCompatibleBitmap(screen, cx, h);
            let old_bm = SelectObject(dc, bm);
            FillRect(dc, &Rect { l: 0, t: 0, r: cx, b: h }, GetStockObject(4));
            SetTextColor(dc, 0xffffff);
            SetBkColor(dc, 0);
            let font = CreateFontA(h, 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 4, 0, c"Arial".as_ptr().cast());
            let old_font = SelectObject(dc, font);
            let mut r = Rect { l: 0, t: 0, r: w, b: h };
            DrawTextW(dc, wide.as_ptr(), -1, &mut r, 1);
            let mut info = BitmapInfo { size: 40, w: cx, h: -h, planes: 1, bits: 24, rest: [0; 6], colours: [0] };
            let n = GetDIBits(dc, bm, 0, h as u32, bits.as_mut_ptr(), &mut info, 0);
            SelectObject(dc, old_font);
            SelectObject(dc, old_bm);
            DeleteObject(font);
            DeleteObject(bm);
            DeleteDC(dc);
            ReleaseDC(std::ptr::null_mut(), screen);
            n == h
        };
        if !drawn {
            return None;
        }
        let up4 = |v: u32| v.div_ceil(4) * 4;
        let (tw, th) = (up4(w as u32), up4(h as u32));
        let mut rgba = vec![0u8; (tw * th * 4) as usize];
        for y in 0..h as usize {
            for x in 0..w as usize {
                let a = (bits[y * row + x * 3] >> 4) * 17;
                let i = (y * tw as usize + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[255, 255, 255, a]);
            }
        }
        Some(Image { width: tw, height: th, rgba })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_ramp() {
        assert_eq!(alpha(0), Some(0));
        assert_eq!(alpha(0x0fff), Some(255));
        assert_eq!(alpha(0x1800), Some(255));
        assert_eq!(alpha(0x2010), Some(254));
        assert_eq!(alpha(0x2fff), Some(0));
        assert_eq!(alpha(0x3000), None);
        assert_eq!(shadow_alpha(255), 204);
        assert_eq!(shadow_alpha(4), 3);
        assert_eq!(shadow_alpha(5), 4);
    }

    /// The GDI line at 600 lines against ig2.exe's: 457 px wide (the exe's capture 456), the capitals on rows 3..=14.
    #[cfg(windows)]
    #[test]
    fn the_credits_line_is_gdi_arial() {
        let img = gdi::render("REMASTERED BY GYROSCOPE. PUBLISHED BY THQNORDIC", 18).expect("GDI");
        let a = |x: u32, y: u32| img.rgba[((y * img.width + x) * 4 + 3) as usize];
        let cols: Vec<u32> = (0..img.width).filter(|&x| (0..img.height).any(|y| a(x, y) > 40)).collect();
        let rows: Vec<u32> = (0..img.height).filter(|&y| (0..img.width).any(|x| a(x, y) > 128)).collect();
        assert_eq!((img.width, img.height), (1152, 20));
        assert_eq!(cols.last().unwrap() - cols[0] + 1, 457);
        assert_eq!((rows[0], *rows.last().unwrap()), (3, 14));
    }
}
