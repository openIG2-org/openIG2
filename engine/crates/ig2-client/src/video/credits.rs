//! The remaster's credits line over `credits.bik` (re/screens/video.md §3, `ig2screen::render` exe 0x7370f0): the
//! exe's own string, rendered once by `PlatformUtils::RenderText` (exe 0x6e9130: SDL_ttf into a 4-bit alpha texture,
//! the install's `fonts/jurabook.ttf`) and drawn 1:1 in window pixels, centred at (W/2, 0.96·H) over a black shadow
//! at +2, +2 (four fifths of the text's alpha), for the clip's first 12288 ms.

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

/// The text's texture for a window `h` pixels high: `RenderText(text, w = (h>>5)<<6, h = h>>5, size = h>>5, centred)`,
/// both sides rounded up to 4 (the packed font texture group), white with the coverage's top 4 bits as alpha.
/// The glyphs' scale is fitted to ig2.exe's capture at 800×600 (em height `1.25·size`, width 0.84 of that; the
/// remaster's own `TTF_RenderUTF8_8Bit_Wrapped` 0x72d2f0 was not read): re/deviations.md 215.
pub fn render(install: &ig2_formats::Install, text: &str, h: u32) -> Option<Image> {
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
}
