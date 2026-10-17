//! `KiirStr`'s bitmap font (re/screens/messages.md §1): the `screen/charset_bmp` atlas, monospaced glyphs in four
//! sizes, drawn additively in a `DDColor` entry on the 640 × 480 virtual screen.

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_rules::Charset;

use crate::assets::Image;
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};

/// The virtual screen (480 lines high, widened for the window; `ModifyViewPort`) → window pixels.
#[derive(Debug, Clone, Copy)]
pub struct Virtual {
    pub k: f32,
    pub x0: f32,
}

impl Virtual {
    pub fn new(size: [f32; 2]) -> Self {
        let k = size[1] / 480.0;
        Self { k, x0: size[0] / 2.0 - 320.0 * k }
    }

    pub fn px(&self, x: f32, y: f32) -> [f32; 2] {
        [self.x0 + x * self.k, y * self.k]
    }

    /// Window pixels → virtual pixels.
    pub fn virtual_of(&self, p: [f32; 2]) -> [f32; 2] {
        [(p[0] - self.x0) / self.k, p[1] / self.k]
    }
}

/// Text alignment (`KiirStr`'s flags & 3): 0 left, 1 centred in the `len` cells, 2 right-aligned in them, 3 centred
/// on x (ELF 0x4f8570: integer halves).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Field,
    Right,
    Centre,
}

pub struct Font {
    tex: TexId,
    cs: Charset,
    /// Byte → glyph index (`kereso_tabla`: 0x4c for bytes not in the order string).
    glyph: [u8; 256],
}

impl Font {
    pub fn load(r: &mut Renderer, install: &Install, rules: &ig2_rules::Rules) -> Option<Self> {
        let img = Image::load(install, "dataraw/screen/charset_bmp.pvr").map_err(|e| eprintln!("ig2-client: {e}")).ok()?;
        let tex = r.add_texture(&img);
        let order = if install.lang() == Lang::Hu { &rules.charset.order_hu } else { &rules.charset.order };
        let mut glyph = [0x4c_u8; 256];
        for (i, &b) in order.iter().enumerate() {
            glyph[b as usize] = i as u8;
        }
        Some(Self { tex, cs: rules.charset.clone(), glyph })
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
        let get = |t: &Vec<i32>| t.get(size).copied().unwrap_or(1).max(1) as f32;
        let (sx, sy, y0, per_row) = (get(&cs.step_x), get(&cs.step_y), cs.start_y.get(size).copied().unwrap_or(0) as f32, get(&cs.per_row));
        for &b in &s[..n] {
            let visible = clip.is_none_or(|(a, z)| x >= a && x + w - 1.0 <= z);
            if b != b' ' && visible {
                let g = self.glyph[b as usize] as f32;
                let (u, vv) = ((g % per_row).floor() * sx, (g / per_row).floor() * sy + y0);
                let p = v.px(x + w / 2.0, y + h / 2.0);
                let mut i = Instance::new(kind::SPRITE, p, [w * v.k / 2.0, h * v.k / 2.0], [rgb[0], rgb[1], rgb[2], 1.0]);
                i.uv = [u / 256.0, vv / 256.0, w / 256.0, h / 256.0];
                f.push(self.tex, Blend::Additive, i);
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
