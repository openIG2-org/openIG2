//! The faders (`FadeScreen` and its rounded boxes, re/screens/hud.md §1.1): every screen's translucent panels,
//! drawn in the 2D pass on the gamma-space frame ([`Blend::GammaAlpha`]).

use ig2_formats::Install;

use super::Virtual;
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};

/// `fadertexture`: every screen's 3D init loads `textures/design/screen/dt.bmp` into it (`Init3dStarmap`,
/// `Init3dColony`, `Research3DInit`, `Diplomacy3DInit`, `Infoing3DInit`, …); `polygonsfader` (ELF 0x5f16a0) draws
/// every fader quad with it, the texel multiplied into the vertex colour.
struct FaderTexture {
    tex: TexId,
    /// The texel at the coordinates (0, 0) (a texture scale of 0): bilinear with repeat there is the mean of the four
    /// corner texels.
    corner: [f32; 4],
}

/// One per process: the client has one renderer (the window's or the snapshot's).
static FADER_TEXTURE: std::sync::OnceLock<FaderTexture> = std::sync::OnceLock::new();

/// Load `dt.bmp` for the faders (without it they are drawn untextured, like `nulltexture`).
pub fn load_fader_texture(r: &mut Renderer, install: &Install) {
    if FADER_TEXTURE.get().is_some() {
        return;
    }
    let img = install.resolve_texture("textures/design/screen/", "dt.bmp").and_then(|p| crate::assets::Image::load(install, &p).ok());
    if let Some(img) = img.filter(|i| i.width > 0 && i.height > 0) {
        let (w, h) = (img.width as usize, img.height as usize);
        let px = |x: usize, y: usize, k: usize| img.rgba[4 * (y * w + x) + k] as f32 / 255.0;
        let corner = std::array::from_fn(|k| (px(0, 0, k) + px(w - 1, 0, k) + px(0, h - 1, k) + px(w - 1, h - 1, k)) / 4.0);
        let _ = FADER_TEXTURE.set(FaderTexture { tex: r.add_texture(&img), corner });
    }
}

/// `FadeKerettelKerekit(x, y, w, h, fill, t, edge, t', …, 0, 0, 0, 0)` (ELF 0x538330): `FadeKerekit` 0x537bd0's
/// rounded fill (five strips) and `FadeKeretKerekit` 0x537e70's one-pixel rounded frame (twelve pieces), each a
/// `FadeScreen` 0x537890 that queues the colour as `0xTTBBGGRR`; `polygonsfader` inverts the fourth value (a
/// transparency: opacity `(255 − t)/255`) and blends with src alpha in the 2D pass ([`Blend::GammaAlpha`]). With a
/// texture scale of 0 `PutDarkeningFace` (ELF 0x538de0) gives every vertex the coordinates (0, 0): the fill and the
/// frame are tinted by `fadertexture`'s corner texel.
#[allow(clippy::too_many_arguments)]
pub fn fader(f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4], fill: [f32; 3], ft: f32, edge: [f32; 3], et: f32) {
    rounded(f, white, v, r, fill, ft, Some((edge, et)), None);
}

/// `FadeKerekit(x, y, w, h, fill, t, false, 0.005, 0.005, u, v)` (ELF 0x537bd0): the rounded fill alone, textured
/// by `fadertexture` at `((x + u)·0.005, (y + v)·0.005)` per virtual pixel (`PutDarkeningFace`).
pub fn fade_kerekit(f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4], fill: [f32; 3], t: f32, off: [f32; 2]) {
    rounded(f, white, v, r, fill, t, None, Some(off));
}

/// `KirakFader(i, faders, x, y, w, h, fill, t, edge, t', …, 0.005, 0.005)` (ELF 0x547920): `GLOBALKELLDESIGN` is
/// set (1 in the image, never written), so it passes the texture scale 0.005 and the fader's own offsets
/// (`+0x1e0`/`+0x258 + 4i`, drifting by `rand()` in `FaderDesignFrissito`; 0 here, as `InitFaderDesignStruktura`
/// leaves them): the fill samples `fadertexture` at `((x + u)·0.005, (y + v)·0.005)` per virtual pixel; the frame
/// keeps the scale 0. The callers' colours are × `s` (`+0x168 + 4i` = 0.9 + the occasional shine, [`S`]).
#[allow(clippy::too_many_arguments)]
pub fn kirak_fader(f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4], fill: [f32; 3], ft: f32, edge: [f32; 3], et: f32) {
    rounded(f, white, v, r, fill, ft, Some((edge, et)), Some([0.0; 2]));
}

#[allow(clippy::too_many_arguments)]
/// The fill (textured at 0.005 a pixel from `design`'s offsets, else at the corner texel) and the frame, if any.
fn rounded(f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4], fill: [f32; 3], ft: f32, frame: Option<([f32; 3], f32)>, design: Option<[f32; 2]>) {
    let [x, y, w, h] = r.map(|c| c as i32 as f32);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let dt = FADER_TEXTURE.get();
    let corner = dt.map_or([1.0; 4], |t| t.corner);
    let quad = |f: &mut Frame, x: f32, y: f32, w: f32, h: f32, c: [f32; 4], scale: f32, off: [f32; 2]| {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let mut i = Instance::new(kind::SPRITE, v.px(x + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], c);
        match dt {
            Some(t) if scale > 0.0 => {
                i.uv = [(x + off[0]) * scale, (y + off[1]) * scale, w * scale, h * scale];
                f.push(t.tex, Blend::GammaAlpha, i);
            }
            _ => {
                i.color = std::array::from_fn(|k| c[k] * corner[k]);
                f.push(white, Blend::GammaAlpha, i);
            }
        }
    };
    let tint = |c: [f32; 3], tr: f32| {
        let a = (255.0 - tr.clamp(0.0, 255.0)) / 255.0;
        [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0, a]
    };
    let c = tint(fill, ft);
    let (scale, off) = match design {
        Some(o) => (0.005, o),
        None => (0.0, [0.0; 2]),
    };
    for [qx, qy, qw, qh] in [
        [x + 4.0, y + 1.0, w - 8.0, 1.0],
        [x + 2.0, y + 2.0, w - 4.0, 2.0],
        [x + 1.0, y + 4.0, w - 2.0, h - 8.0],
        [x + 2.0, y + h - 4.0, w - 4.0, 2.0],
        [x + 4.0, y + h - 2.0, w - 8.0, 1.0],
    ] {
        quad(f, qx, qy, qw, qh, c, scale, off);
    }
    let Some((edge, et)) = frame else { return };
    let e = tint(edge, et);
    let (x1, y1) = (x + w, y + h);
    for [qx, qy, qw, qh] in [
        [x + 4.0, y, w - 8.0, 1.0],
        [x + 4.0, y1 - 1.0, w - 8.0, 1.0],
        [x, y + 4.0, 1.0, h - 8.0],
        [x1 - 1.0, y + 4.0, 1.0, h - 8.0],
        [x + 2.0, y + 1.0, 2.0, 1.0],
        [x1 - 4.0, y + 1.0, 2.0, 1.0],
        [x + 2.0, y1 - 2.0, 2.0, 1.0],
        [x1 - 4.0, y1 - 2.0, 2.0, 1.0],
        [x + 1.0, y + 2.0, 1.0, 2.0],
        [x + 1.0, y1 - 4.0, 1.0, 2.0],
        [x1 - 2.0, y + 2.0, 1.0, 2.0],
        [x1 - 2.0, y1 - 4.0, 1.0, 2.0],
    ] {
        quad(f, qx, qy, qw, qh, e, 0.0, [0.0; 2]);
    }
}

/// `FaderDesignFrissito` (ELF 0x547e10) keeps a fader's `s` at 0.9 (the remaster's shine adds to it only with
/// `GLOBALKELLDESIGN`).
pub const S: f32 = 0.9;

/// The exe's colour `c` (0..255) at opacity `a` as a [`Blend::GammaAlpha`] tint: `FadeScreen`'s quads are the 2D
/// pass's (re/screens/hud.md §1.1), blended on the frame's stored values.
pub fn fade_color(c: [f32; 3], a: f32) -> [f32; 4] {
    [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0, a]
}
