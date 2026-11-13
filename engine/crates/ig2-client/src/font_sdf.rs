//! The HD pack's sharp font (ADR 0089 §5, row 572): the original's glyphs cut from `screen/charset_bmp`, each
//! upscaled ×8 with ADR 0054's edge-keeping filter (xBRZ), thresholded and turned into a signed distance field, laid
//! out in the original's cells at [`SCALE`] times the atlas's size. The client draws `KiirStr`'s quads from it
//! unchanged (same uv, advances and baselines); only the shader reads the edge from the distance.

use ig2_rules::Charset;

use crate::assets::Image;
use crate::upscale::PACK_DIR;

/// The install's font atlas.
pub const ATLAS: &str = "dataraw/screen/charset_bmp.pvr";
/// The distance field's size over the atlas's.
pub const SCALE: usize = 4;
/// The glyphs' upscale before the threshold (twice [`SCALE`]: the field is measured at ×8 and stored at ×4).
const UP: usize = 8;
/// The field's reach each side of the edge, in stored texels: 0 at `-SPREAD`, 0.5 on the edge, 1 at `+SPREAD`.
pub const SPREAD: f32 = 8.0;

/// The pack's file: `hires/dataraw/screen/charset_bmp.sdf.png`.
pub fn file() -> String {
    format!("{PACK_DIR}{}.sdf.png", ATLAS.trim_end_matches(".pvr"))
}

/// Every glyph cell `KiirStr` reads (re/screens/messages.md §1), in the atlas's 1/256 units: (x, y, w, h) for each
/// size 1..=4 and glyph 0..`n`, where it fits in the atlas.
pub fn cells(cs: &Charset, n: usize) -> Vec<[u32; 4]> {
    let at = |t: &Vec<i32>, s: usize| t.get(s).copied().unwrap_or(0).max(0) as u32;
    let mut v = Vec::new();
    for s in 1..cs.width.len().min(5) {
        let (w, h, sx, sy, y0, per_row) = (at(&cs.width, s), at(&cs.height, s), at(&cs.step_x, s), at(&cs.step_y, s), at(&cs.start_y, s), at(&cs.per_row, s).max(1));
        for g in 0..n as u32 {
            let (x, y) = ((g % per_row) * sx, (g / per_row) * sy + y0);
            if w > 0 && h > 0 && x + w <= 256 && y + h <= 256 {
                v.push([x, y, w, h]);
            }
        }
    }
    v
}

/// The squared distance from each cell of a row to the nearest 0 of `f` (0 = a seed, a large number = none):
/// Felzenszwalb and Huttenlocher's lower envelope of parabolas.
fn dt1(f: &[f32], d: &mut [f32]) {
    let n = f.len();
    let (mut v, mut z) = (vec![0usize; n], vec![0f32; n + 1]);
    let mut k = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let s = loop {
            let p = v[k];
            let s = ((f[q] + (q * q) as f32) - (f[p] + (p * p) as f32)) / (2 * (q - p)) as f32;
            if s > z[k] {
                break s;
            }
            k -= 1;
        };
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, d) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        *d = (q as f32 - v[k] as f32).powi(2) + f[v[k]];
    }
}

/// The distance from each texel of the `w` × `h` grid to the nearest one where `seed` is true (a large number when
/// none is).
fn distance(seed: &[bool], w: usize, h: usize) -> Vec<f32> {
    const FAR: f32 = 1e12;
    let mut g: Vec<f32> = seed.iter().map(|&s| if s { 0.0 } else { FAR }).collect();
    let (mut col, mut out) = (vec![0f32; h], vec![0f32; h.max(w)]);
    for x in 0..w {
        col.iter_mut().enumerate().for_each(|(y, c)| *c = g[y * w + x]);
        dt1(&col, &mut out[..h]);
        (0..h).for_each(|y| g[y * w + x] = out[y]);
    }
    for row in g.chunks_exact_mut(w) {
        dt1(row, &mut out[..w]);
        row.copy_from_slice(&out[..w]);
    }
    g.into_iter().map(f32::sqrt).collect()
}

/// A thresholded `w` × `h` shape's signed distance to its edge in texels: positive inside, the edge half a texel past
/// the outermost ink texel's centre.
fn signed(ink: &[bool], w: usize, h: usize) -> Vec<f32> {
    let out = distance(ink, w, h);
    let inside = distance(&ink.iter().map(|&i| !i).collect::<Vec<_>>(), w, h);
    ink.iter().zip(out.iter().zip(inside)).map(|(&i, (&o, n))| if i { n - 0.5 } else { 0.5 - o }).collect()
}

/// One glyph's cell (`w` × `h` texels of `rgba`, straight from the atlas) as a stored field `w·SCALE` × `h·SCALE`
/// (0..=255, the edge at 128): upscaled ×[`UP`] by xBRZ, ink where the brightest channel passes half, the distance
/// measured there and averaged over each stored texel's 2 × 2.
fn glyph(rgba: &[u8], w: usize, h: usize) -> Vec<u8> {
    let big = crate::upscale::xbrz(&crate::upscale::xbrz(rgba, w, h, 4), w * 4, h * 4, UP / 4);
    let (bw, bh) = (w * UP, h * UP);
    let ink: Vec<bool> = big.chunks_exact(4).map(|p| p[0].max(p[1]).max(p[2]) >= 128).collect();
    let d = signed(&ink, bw, bh);
    let k = UP / SCALE;
    let (sw, sh) = (w * SCALE, h * SCALE);
    let mut out = vec![0u8; sw * sh];
    for y in 0..sh {
        for x in 0..sw {
            let sum: f32 = (0..k * k).map(|i| d[(y * k + i / k) * bw + x * k + i % k]).sum();
            let stored = sum / (k * k) as f32 / k as f32;
            out[y * sw + x] = ((0.5 + stored / (2.0 * SPREAD)).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    out
}

/// The distance-field atlas of `atlas` (the install's font texture) for the cells of `cs` (`n` glyphs a size): grey
/// RGBA, opaque, 0 outside every glyph.
pub fn build(atlas: &Image, cs: &Charset, n: usize) -> Image {
    let (aw, ah) = (atlas.width as usize, atlas.height as usize);
    let t = aw / 256;
    let (ow, oh) = (aw * SCALE, ah * SCALE);
    let mut field = vec![0u8; ow * oh];
    for [x, y, w, h] in cells(cs, n) {
        let (x, y, w, h) = (x as usize * t, y as usize * t, w as usize * t, h as usize * t);
        if x + w > aw || y + h > ah {
            continue;
        }
        let cut: Vec<u8> = (0..h).flat_map(|r| atlas.rgba[((y + r) * aw + x) * 4..][..w * 4].to_vec()).collect();
        let g = glyph(&cut, w, h);
        for r in 0..h * SCALE {
            field[(y * SCALE + r) * ow + x * SCALE..][..w * SCALE].copy_from_slice(&g[r * w * SCALE..][..w * SCALE]);
        }
    }
    let rgba = field.iter().flat_map(|&v| [v, v, v, 255]).collect();
    Image { width: ow as u32, height: oh as u32, rgba, mips: Vec::new(), hires: None }
}

/// The shader's coverage of a stored field value `v` (0..1) whose texel spans `px` screen pixels: the edge at 0.5,
/// a ramp one screen pixel wide (`fwidth` there is `1 / (2·SPREAD·px)` a pixel).
#[cfg(test)]
pub fn coverage(v: f32, px: f32) -> f32 {
    ((v - 0.5) * 2.0 * SPREAD * px + 0.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_and_cells() {
        // A 1-D check: the nearest seed's squared distance.
        let mut d = [0f32; 5];
        dt1(&[1e12, 0.0, 1e12, 1e12, 0.0], &mut d);
        assert_eq!(d, [1.0, 0.0, 1.0, 1.0, 0.0]);
        // A 4 × 4 ink square in an 8 × 8 grid: one texel in, 0.5; one out, -0.5; the corner's diagonal neighbour √2 off.
        let ink: Vec<bool> = (0..64).map(|i| (2..6).contains(&(i % 8)) && (2..6).contains(&(i / 8))).collect();
        let s = signed(&ink, 8, 8);
        assert_eq!((s[2 * 8 + 2], s[3 * 8 + 3], s[2 * 8 + 1]), (0.5, 1.5, -0.5));
        assert!((s[9] - (0.5 - 2f32.sqrt())).abs() < 1e-6);
        // KiirStr's layout (font.rs's test values): size 2's glyph 43 is the second row's second cell.
        let cs = Charset { width: vec![0, 6, 6, 8, 12], height: vec![0, 5, 7, 10, 12], step_x: vec![0, 6, 6, 8, 12], step_y: vec![0, 6, 8, 10, 12], start_y: vec![0, 0, 24, 62, 126], per_row: vec![0, 42, 42, 32, 21], ..Charset::default() };
        let c = cells(&cs, 50);
        assert_eq!(c.len(), 200);
        assert_eq!((c[0], c[50 + 43], c[150 + 21]), ([0, 0, 6, 5], [6, 32, 6, 7], [0, 138, 12, 12]));
        assert_eq!(file(), "hires/dataraw/screen/charset_bmp.sdf.png");
    }

    /// The field's edge at any scale: drawn at 1, 2.25 and 4.5 screen pixels a stored texel, the coverage is the
    /// shape's (inside 1, outside 0) one screen pixel past the edge, half on it.
    #[test]
    fn the_field_draws_sharp_at_any_scale() {
        // A 2-texel-thick bar across a 6 × 6 cell, white on black like the atlas.
        let rgba: Vec<u8> = (0..36).flat_map(|i| if (2..4).contains(&(i / 6)) { [255, 255, 255, 255] } else { [0, 0, 0, 255] }).collect();
        let f = glyph(&rgba, 6, 6);
        let w = 6 * SCALE;
        let at = |y: usize| f[y * w + w / 2] as f32 / 255.0;
        // The bar's edge at texel 2 of the cell = stored 8: the stored texels either side straddle 128.
        assert!(at(7) < 0.5 && at(8) > 0.5 && at(12) > 0.7 && at(0) < 0.5 - 4.0 / (2.0 * SPREAD));
        for px in [1.0, 2.25, 4.5] {
            assert_eq!(coverage(at(4), px), 0.0, "{px}");
            assert_eq!(coverage(at(11), px), 1.0, "{px}");
            let edge = (at(7) + at(8)) / 2.0;
            assert!((coverage(edge, px) - 0.5).abs() < 0.1, "{px}");
        }
    }

    /// The install's glyphs keep their shapes: the field read back at the atlas's texel centres has ink where the
    /// original has more than half, all but a few edge texels.
    #[test]
    fn the_install_font_keeps_its_shapes() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let rules = ig2_rules::Rules::from_install(&install).unwrap();
        let atlas = Image::load_original(&install, ATLAS).unwrap();
        let cs = &rules.charset;
        let sdf = build(&atlas, cs, cs.order.len().max(cs.order_hu.len()));
        assert_eq!((sdf.width, sdf.height), (atlas.width * SCALE as u32, atlas.height * SCALE as u32));
        let (aw, sw) = (atlas.width as usize, sdf.width as usize);
        let (mut ink, mut wrong) = (0, 0);
        for [x, y, w, h] in cells(cs, cs.order.len()) {
            let t = aw / 256;
            for yy in y as usize * t..(y + h) as usize * t {
                for xx in x as usize * t..(x + w) as usize * t {
                    let a = atlas.rgba[(yy * aw + xx) * 4] >= 128;
                    // A texel's centre falls between the stored texels SCALE/2 - 1 and SCALE/2 of its block.
                    let (cx, cy) = (xx * SCALE + SCALE / 2, yy * SCALE + SCALE / 2);
                    let v = [(cx - 1, cy - 1), (cx, cy - 1), (cx - 1, cy), (cx, cy)].iter().map(|&(x, y)| sdf.rgba[(y * sw + x) * 4] as u32).sum::<u32>();
                    ink += a as usize;
                    wrong += (a != (v >= 4 * 128)) as usize;
                }
            }
        }
        assert!(ink > 10_000, "{ink}");
        assert!(wrong * 50 < ink, "{wrong} of {ink}");
    }
}
