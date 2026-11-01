//! Texture upscaling and high-resolution packs (ADR 0054): the pixels the renderer uploads in place of an install
//! texture's, its size and pixels kept for layout.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use ig2_formats::pvr::Pvr;
use ig2_formats::Install;

use crate::assets::Image;

/// A texture's drawn pixels when they are not its own: an upscaled copy or a pack's picture.
#[derive(Debug)]
pub struct Hires {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// An exact multiple of the original: its halvings reach the original's size, whose own levels follow.
    pub upscaled: bool,
}

/// A pack's pictures: `hires/<game path>` in an enabled mod (ADR 0054 §5).
pub const PACK_DIR: &str = "hires/";

/// The largest side a filter makes (§3).
const MAX_SIDE: u32 = 4096;
/// The cache is emptied past this many bytes (§4).
const CACHE_LIMIT: usize = 1 << 30;

static FACTOR: AtomicU8 = AtomicU8::new(1);

/// The "Texture upscaling" row's factor: 1 (off), 2 or 4.
pub fn set_factor(f: u8) {
    FACTOR.store(if matches!(f, 2 | 4) { f } else { 1 }, Ordering::Relaxed);
}

pub fn factor() -> u8 {
    FACTOR.load(Ordering::Relaxed)
}

type Cache = HashMap<(String, u8), Arc<Hires>>;
static CACHE: Mutex<Option<(Cache, usize)>> = Mutex::new(None);

fn cached(key: (String, u8), make: impl FnOnce() -> Option<Hires>) -> Option<Arc<Hires>> {
    if let Some(h) = CACHE.lock().unwrap().as_ref().and_then(|(c, _)| c.get(&key).cloned()) {
        return Some(h);
    }
    let h = Arc::new(make()?);
    let mut guard = CACHE.lock().unwrap();
    let (c, bytes) = guard.get_or_insert_with(Default::default);
    if *bytes + h.rgba.len() > CACHE_LIMIT {
        c.clear();
        *bytes = 0;
    }
    *bytes += h.rgba.len();
    c.insert(key, h.clone());
    Some(h)
}

/// What the renderer draws for the install texture `path` (decoded as `img`): an enabled pack's picture, else the
/// filtered picture at the row's factor, else nothing (the texture's own pixels).
pub fn hires(install: &Install, path: &str, img: &Image) -> Option<Arc<Hires>> {
    if !install.mods().is_empty() {
        if let Some(h) = pack(install, path) {
            return Some(h);
        }
    }
    let f = fit(img.width, img.height, factor())?;
    cached((path.to_ascii_lowercase(), f), || Some(upscale(img, f)))
}

/// A pack's picture for `path`: `hires/<path>` with `.png` for its extension, else as it is (a `.pvr`).
fn pack(install: &Install, path: &str) -> Option<Arc<Hires>> {
    let stem = path.rsplit_once('.').map_or(path, |(s, _)| s);
    let (file, bytes) = [format!("{PACK_DIR}{stem}.png"), format!("{PACK_DIR}{path}")].into_iter().find_map(|p| install.read(&p).ok().map(|b| (p, b)))?;
    // Keyed by the game path and the file's length: another pack's picture of the same name is another entry.
    cached((format!("{file}#{}", bytes.len()), 0), || {
        let decoded = if file.ends_with(".png") { decode_png(&bytes) } else { decode_pvr(&bytes) };
        decoded.map_err(|e| eprintln!("ig2-client: {file}: {e}")).ok()
    })
}

fn decode_png(bytes: &[u8]) -> Result<Hires, String> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA | png::Transformations::STRIP_16);
    let mut r = d.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("too large")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        c => return Err(format!("unsupported colour type {c:?}")),
    };
    Ok(Hires { width: info.width, height: info.height, rgba, upscaled: false })
}

fn decode_pvr(bytes: &[u8]) -> Result<Hires, String> {
    let pvr = Pvr::parse(bytes).map_err(|e| e.to_string())?;
    let rgba = pvr.decode_levels().map_err(|e| e.to_string())?.into_iter().next().ok_or("no pixels")?.pixels;
    Ok(Hires { width: pvr.width, height: pvr.height, rgba, upscaled: false })
}

/// The factor a `w` × `h` texture is scaled by at the row's `f`: 4, else 2, so that no side passes [`MAX_SIDE`].
pub fn fit(w: u32, h: u32, f: u8) -> Option<u8> {
    [4, 2].into_iter().filter(|&k| k <= f).find(|&k| w.max(h).saturating_mul(k as u32) <= MAX_SIDE && w > 0 && h > 0)
}

/// Flat-colour art (§3): at least 40 % of the horizontally adjacent opaque texel pairs exactly equal.
pub fn is_flat(img: &Image) -> bool {
    let (mut pairs, mut equal) = (0usize, 0usize);
    for row in img.rgba.chunks_exact(img.width as usize * 4) {
        for x in (4..row.len()).step_by(4) {
            if row[x - 1] > 0 && row[x + 3] > 0 {
                pairs += 1;
                equal += (row[x - 4..x] == row[x..x + 4]) as usize;
            }
        }
    }
    pairs > 0 && equal * 5 >= pairs * 2
}

/// `img` scaled by `f`: xBRZ for flat-colour art, Catmull-Rom otherwise.
pub fn upscale(img: &Image, f: u8) -> Hires {
    let (w, h) = (img.width as usize, img.height as usize);
    let rgba = if is_flat(img) { xbrz(&img.rgba, w, h, f as usize) } else { cubic(&img.rgba, w, h, f as usize) };
    Hires { width: img.width * f as u32, height: img.height * f as u32, rgba, upscaled: true }
}

/// xBRZ with the border texels repeated (the crate reads past the edge as transparent, which would fade every
/// picture's border).
fn xbrz(rgba: &[u8], w: usize, h: usize, f: usize) -> Vec<u8> {
    const PAD: usize = 2;
    let (pw, ph) = (w + 2 * PAD, h + 2 * PAD);
    let at = |x: usize, y: usize| {
        let i = (y.clamp(PAD, h + PAD - 1) - PAD) * w + x.clamp(PAD, w + PAD - 1) - PAD;
        &rgba[i * 4..i * 4 + 4]
    };
    let padded: Vec<u8> = (0..ph).flat_map(|y| (0..pw).flat_map(move |x| at(x, y).to_vec())).collect();
    let big = xbrz::scale_rgba(&padded, pw, ph, f);
    let (row, skip) = (w * f * 4, PAD * f * 4);
    (0..h * f).flat_map(|y| &big[(y + PAD * f) * pw * f * 4 + skip..][..row]).copied().collect()
}

/// The Catmull-Rom weights of the 4 taps around a sample `t` (0..1) past the second.
fn weights(t: f32) -> [f32; 4] {
    [t * (-0.5 + t * (1.0 - 0.5 * t)), 1.0 + t * t * (-2.5 + 1.5 * t), t * (0.5 + t * (2.0 - 1.5 * t)), t * t * (-0.5 + 0.5 * t)]
}

/// One axis of the cubic scale: `n` lines of `len` premultiplied texels (texel `i` of line `l` at `at(i, l)`), each
/// scaled by `f` into `out` (at `out_at`).
fn cubic_axis(src: &[[f32; 4]], len: usize, n: usize, f: usize, at: impl Fn(usize, usize) -> usize, out_at: impl Fn(usize, usize) -> usize, out: &mut [[f32; 4]]) {
    for i in 0..len * f {
        let x = (i as f32 + 0.5) / f as f32 - 0.5;
        let x0 = x.floor();
        let w = weights(x - x0);
        let taps: [usize; 4] = std::array::from_fn(|k| (x0 as isize + k as isize - 1).clamp(0, len as isize - 1) as usize);
        for line in 0..n {
            let mut c = [0.0f32; 4];
            for k in 0..4 {
                let s = src[at(taps[k], line)];
                for ch in 0..4 {
                    c[ch] += s[ch] * w[k];
                }
            }
            out[out_at(i, line)] = c;
        }
    }
}

/// `rgba` (`w` × `h`, straight alpha) scaled by `f` with Catmull-Rom on premultiplied colour.
pub fn cubic(rgba: &[u8], w: usize, h: usize, f: usize) -> Vec<u8> {
    let pre: Vec<[f32; 4]> = rgba
        .chunks_exact(4)
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, p[3] as f32]
        })
        .collect();
    let (ow, oh) = (w * f, h * f);
    let mut wide = vec![[0.0; 4]; ow * h];
    cubic_axis(&pre, w, h, f, |x, y| y * w + x, |x, y| y * ow + x, &mut wide);
    let mut out = vec![[0.0; 4]; ow * oh];
    cubic_axis(&wide, h, ow, f, |y, x| y * ow + x, |y, x| y * ow + x, &mut out);
    out.iter()
        .flat_map(|c| {
            let a = c[3].clamp(0.0, 255.0);
            let k = if a > 0.0 { 255.0 / a } else { 0.0 };
            [(c[0] * k).clamp(0.0, 255.0).round() as u8, (c[1] * k).clamp(0.0, 255.0).round() as u8, (c[2] * k).clamp(0.0, 255.0).round() as u8, a.round() as u8]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Image {
        let f = &f;
        let rgba = (0..h).flat_map(|y| (0..w).flat_map(move |x| f(x, y))).collect();
        Image { width: w, height: h, rgba, mips: Vec::new(), hires: None }
    }

    #[test]
    fn the_factor_drops_to_stay_under_the_limit() {
        assert_eq!(fit(256, 256, 4), Some(4));
        assert_eq!(fit(1024, 512, 4), Some(4));
        assert_eq!(fit(2048, 64, 4), Some(2));
        assert_eq!(fit(2048, 64, 2), Some(2));
        assert_eq!(fit(4096, 64, 4), None);
        assert_eq!(fit(256, 256, 1), None);
        assert_eq!(fit(0, 256, 4), None);
    }

    #[test]
    fn flat_art_and_rendered_pictures_are_told_apart() {
        let flat = image(16, 16, |x, _| if x < 8 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
        let ramp = image(16, 16, |x, y| [(x * 16) as u8, (y * 16) as u8, (x * y) as u8, 255]);
        assert!(is_flat(&flat));
        assert!(!is_flat(&ramp));
        // Transparent texels don't count either way.
        assert!(!is_flat(&image(16, 16, |_, _| [0, 0, 0, 0])));
    }

    #[test]
    fn upscaling_multiplies_the_size_and_keeps_flat_colour() {
        for img in [image(8, 4, |_, _| [10, 200, 30, 255]), image(8, 4, |x, y| [(x * 30) as u8, (y * 60) as u8, 7, 255])] {
            for f in [2, 4] {
                let h = upscale(&img, f);
                let f32 = f as u32;
                assert_eq!((h.width, h.height, h.rgba.len()), (8 * f32, 4 * f32, (32 * f32 * f32 * 4) as usize));
                assert!(h.upscaled);
            }
        }
        let flat = upscale(&image(8, 4, |_, _| [10, 200, 30, 255]), 4);
        assert!(flat.rgba.chunks_exact(4).all(|p| p == [10, 200, 30, 255]));
        let smooth = cubic(&image(8, 4, |_, _| [10, 200, 30, 255]).rgba, 8, 4, 2);
        assert!(smooth.chunks_exact(4).all(|p| p == [10, 200, 30, 255]));
    }

    /// ADR 0054 §5–6: a synthetic 4× pack (a picture at 4× and 3× the size, a model at another scale) loads in the
    /// originals' places: the image keeps its size and pixels, the model its largest half-extent.
    #[test]
    fn a_pack_draws_in_the_originals_place() {
        use ig2_formats::text::Lang;
        use ig2_formats::vfs::Mod;
        let Some(Ok(install)) = Install::from_env(Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("ig2-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (cursor, frame) = ("dataraw/screen/mouse_bmp.pvr", "dataraw/textures/messages/finalinter_bmp.pvr");
        for (path, k) in [(cursor, 4), (frame, 3)] {
            let img = Image::load_original(&install, path).unwrap();
            let (w, h) = (img.width * k, img.height * k);
            let rgba: Vec<u8> = (0..h).flat_map(|y| (0..w).flat_map(move |x| [(x % 256) as u8, (y % 256) as u8, 99, 255])).collect();
            let file = dir.join(PACK_DIR).join(path.replace(".pvr", ".png"));
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            crate::assets::write_png(&file, w, h, &rgba).unwrap();
        }
        let models = dir.join(PACK_DIR).join("dataraw/vector/colony");
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(models.join("allvany1.xv"), install.read("dataraw/vector/colony/allvany3.xv").unwrap()).unwrap();
        let modded = Install::open_with_mods(install.root(), Lang::En, vec![Mod { name: "pack".into(), dir: dir.clone() }]).unwrap();
        for (path, k) in [(cursor, 4), (frame, 3)] {
            let (a, b) = (Image::load_original(&install, path).unwrap(), Image::load(&modded, path).unwrap());
            assert_eq!((b.width, b.height, &b.rgba), (a.width, a.height, &a.rgba));
            let hi = b.hires.as_deref().unwrap();
            assert_eq!((hi.width, hi.height, hi.upscaled, hi.rgba[20]), (a.width * k, a.height * k, false, 5));
            assert_eq!(crate::assets::Image::uv_mul(&modded, path), crate::assets::Image::uv_mul(&install, path));
        }
        let max = |o: [f32; 3]| o[0].max(o[1]).max(o[2]);
        let original = crate::scene::Model::load(&install, "vector/colony/", "allvany1.xvb", "textures/colony/").unwrap();
        let other = crate::scene::Model::load(&install, "vector/colony/", "allvany3.xvb", "textures/colony/").unwrap();
        let packed = crate::scene::Model::load(&modded, "vector/colony/", "allvany1.xvb", "textures/colony/").unwrap();
        assert_eq!(packed.parts.len(), other.parts.len());
        assert!((max(packed.origin) - max(original.origin)).abs() < 1e-3, "{:?} vs {:?}", packed.origin, original.origin);
        let ratio = |o: [f32; 3]| o.map(|x| x / max(o));
        assert_eq!(ratio(packed.origin), ratio(other.origin));
        // Display only: a pack stays out of the room's fingerprint.
        assert_eq!(crate::mods_fingerprint(&modded), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A transparent texel's colour never bleeds into an opaque neighbour (premultiplied filtering).
    #[test]
    fn the_cubic_filter_keeps_transparent_colour_out() {
        let img = image(4, 1, |x, _| if x < 2 { [255, 255, 255, 255] } else { [255, 0, 0, 0] });
        let out = cubic(&img.rgba, 4, 1, 4);
        for p in out.chunks_exact(4).filter(|p| p[3] > 8) {
            assert!(p[1] > 200 && p[2] > 200, "{p:?}");
        }
    }
}
