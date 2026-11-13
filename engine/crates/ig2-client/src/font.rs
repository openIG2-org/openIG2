//! `KiirStr`'s bitmap font (re/screens/messages.md §1): the `screen/charset_bmp` atlas, monospaced glyphs in four
//! sizes, drawn additively in a `DDColor` entry on the 640 × 480 virtual screen.

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_rules::Charset;

use crate::assets::Image;
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};

pub use crate::kit::Virtual;

/// Text alignment (`KiirStr`'s flags & 3): 0 left, 1 centred in the `len` cells, 2 right-aligned in them, 3 centred
/// on x (ELF 0x4f8570: integer halves).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Field,
    Right,
    Centre,
}

/// A pixel's fraction past the rasteriser's sub-pixel grid (1/256).
const TIE: f32 = 1.0 / 64.0;

pub struct Font {
    tex: TexId,
    /// The HD pack's distance-field atlas when a mod's `visuals.toml` asks for it (ADR 0089 §5): drawn in `tex`'s place.
    sdf: Option<TexId>,
    cs: Charset,
    /// Byte → glyph index (`kereso_tabla`: 0x4c for bytes not in the order string).
    glyph: [u8; 256],
}

impl Font {
    pub fn load(r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules) -> Option<Self> {
        let img = Image::load_original(install, "dataraw/screen/charset_bmp.pvr").map_err(|e| eprintln!("ig2-client: {e}")).ok()?;
        let tex = r.add_texture(&img);
        let order = if install.lang() == Lang::Hu { &rules.charset.order_hu } else { &rules.charset.order };
        let mut glyph = [0x4c_u8; 256];
        for (i, &b) in order.iter().enumerate() {
            glyph[b as usize] = i as u8;
        }
        Some(Self { tex, sdf: load_sdf(r, install), cs: rules.charset.clone(), glyph })
    }

    /// `CharXMeret[size]`: the advance (and glyph width) in virtual pixels.
    pub fn advance(&self, size: usize) -> f32 {
        self.cs.width.get(size).copied().unwrap_or(6) as f32
    }

    pub fn height(&self, size: usize) -> f32 {
        self.cs.height.get(size).copied().unwrap_or(7) as f32
    }

    /// `KiirStr(text, x, y, len, size, align, colour)`: at most `len` bytes (0 = all), glyph cells from the atlas
    /// (`(i % per_row)·step_x`, `(i / per_row)·step_y + start_y`, in 1/256 of the atlas), additive in `rgb`. Spaces
    /// draw nothing; `clip` = the virtual x range the text may use (`HatarX1..X2`, flag 8).
    #[allow(clippy::too_many_arguments)]
    pub fn text(&self, f: &mut Frame, v: &Virtual, s: &[u8], x: f32, y: f32, len: usize, size: usize, align: Align, rgb: [f32; 3], clip: Option<(f32, f32)>) {
        let s: &[u8] = &s[..s.iter().position(|&b| b == 0).unwrap_or(s.len())];
        let n = if len == 0 { s.len() } else { s.len().min(len) };
        let w = self.advance(size);
        let cells = |n: usize| (n as i32 * w as i32) as f32;
        let mut x = match align {
            Align::Left => x,
            Align::Field if n < len => x + ((cells(len) - cells(n)) / 2.0).trunc(),
            Align::Right if n < len => x + cells(len) - cells(n),
            Align::Field | Align::Right => x,
            Align::Centre => x - (cells(n) / 2.0).trunc(),
        };
        let (cs, h) = (&self.cs, self.height(size));
        let (tex, kind) = self.sdf.map_or((self.tex, kind::GLYPH), |t| (t, kind::GLYPH_SDF));
        let get = |t: &Vec<i32>| t.get(size).copied().unwrap_or(1).max(1) as f32;
        let (sx, sy, y0, per_row) = (get(&cs.step_x), get(&cs.step_y), cs.start_y.get(size).copied().unwrap_or(0) as f32, get(&cs.per_row));
        for &b in &s[..n] {
            let visible = clip.is_none_or(|(a, z)| x >= a && x + w - 1.0 <= z);
            if b != b' ' && visible {
                let g = self.glyph[b as usize] as f32;
                let (u, vv) = ((g % per_row).floor() * sx, (g / per_row).floor() * sy + y0);
                // The exe's GL fills a pixel whose centre lies exactly on the glyph's bottom edge, not on its top
                // edge (wgpu does the opposite): the quad moves `TIE` pixels down, its uv with it.
                let p = v.px(x + w / 2.0, y + h / 2.0);
                let mut i = Instance::new(kind, [p[0], p[1] + TIE], [w * v.k / 2.0, h * v.k / 2.0], [rgb[0], rgb[1], rgb[2], 1.0]);
                i.uv = [u / 256.0, (vv + TIE / v.k) / 256.0, w / 256.0, h / 256.0];
                f.push(tex, Blend::GammaAdditive, i);
            }
            x += w;
        }
    }

    /// `KiirInt`: a number as text.
    #[allow(clippy::too_many_arguments)]
    pub fn int(&self, f: &mut Frame, v: &Virtual, n: i64, x: f32, y: f32, len: usize, size: usize, align: Align, rgb: [f32; 3]) {
        self.text(f, v, n.to_string().as_bytes(), x, y, len, size, align, rgb, None);
    }
}

/// The pack's distance-field font, when an enabled mod's `visuals.toml` sets `[text] sdf_font` and an enabled pack
/// has the atlas; `None` (the classic font) otherwise.
fn load_sdf(r: &mut Renderer, install: &Install) -> Option<TexId> {
    if !crate::visuals::mods().sdf_font {
        return None;
    }
    let file = crate::font_sdf::file();
    let Ok(bytes) = install.read(&file) else {
        eprintln!("ig2-client: [text] sdf_font: no {file} in an enabled mod (openIG2 --make-pack --only font makes it)");
        return None;
    };
    let h = crate::upscale::decode_png(&bytes).map_err(|e| eprintln!("ig2-client: {file}: {e}")).ok()?;
    Some(r.add_texture_linear(&Image { width: h.width, height: h.height, rgba: h.rgba, mips: Vec::new(), hires: None }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Sharp text" (ADR 0046) is the glyph shader's only: the text's quads, sizes and atlas cells stay the ones
    /// `KiirStr` lays out, at every window size.
    #[test]
    fn sharp_text_keeps_the_metrics() {
        let cs = Charset { width: vec![0, 6, 6, 8, 12], height: vec![0, 5, 7, 10, 12], step_x: vec![0, 6, 6, 8, 12], step_y: vec![0, 6, 8, 10, 12], start_y: vec![0, 0, 24, 62, 126], per_row: vec![0, 42, 42, 32, 21], ..Charset::default() };
        let mut glyph = [0x4c_u8; 256];
        for (i, b) in (b'A'..=b'Z').enumerate() {
            glyph[b as usize] = i as u8;
        }
        let mut font = Font { tex: TexId(0), sdf: None, cs, glyph };
        let quads_of = |font: &Font, size: [f32; 2]| {
            let mut f = Frame::new([0.0; 4]);
            font.text(&mut f, &Virtual::new(size), b"SHARP TEXT", 100.0, 50.0, 12, 2, Align::Field, [1.0; 3], None);
            f.instances.iter().map(|i| (i.center, i.half, i.uv)).collect::<Vec<_>>()
        };
        let quads = |size| quads_of(&font, size);
        let classic = [[800.0, 600.0], [2560.0, 1440.0], [3840.0, 2160.0]].map(quads);
        crate::render::set_sharp_text(true);
        let sharp = [[800.0, 600.0], [2560.0, 1440.0], [3840.0, 2160.0]].map(quads);
        crate::render::set_sharp_text(false);
        assert_eq!(classic, sharp);
        // 9 glyphs (the space draws nothing), centred in 12 cells of 6: from x 106; at 3840 × 2160 the 4:3 frame is
        // 2880 wide, 4.5 pixels a unit.
        assert_eq!(classic[2].len(), 9);
        let v = Virtual::new([3840.0, 2160.0]);
        assert_eq!(classic[2][0].0[0], v.px(106.0 + 3.0, 0.0)[0]);
        assert_eq!(classic[2][0].1, [6.0 * v.k / 2.0, 7.0 * v.k / 2.0]);
        // The distance-field font (ADR 0089 §5): the same quads and uv from its own atlas.
        font.sdf = Some(TexId(1));
        let sdf = [[800.0, 600.0], [2560.0, 1440.0], [3840.0, 2160.0]].map(|size| quads_of(&font, size));
        assert_eq!(classic, sdf);
        let mut f = Frame::new([0.0; 4]);
        font.text(&mut f, &v, b"AB", 0.0, 0.0, 0, 2, Align::Left, [1.0; 3], None);
        assert!(f.instances.iter().all(|i| i.kind == kind::GLYPH_SDF));
    }
}
