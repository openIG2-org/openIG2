//! `openIG2 --make-pack`: a texture pack (ADR 0054 §5) made from the player's own install on their machine (ADR 0088
//! §5–6, row 567). Every install texture the client draws, upscaled, as `mods/<name>/hires/<game path>.png`; the pack
//! derives from the game's files, so it is never shared (the mod index carries none). ADR 0089 makes it the HD pack:
//! stages (`--only`), the quality model (Real-ESRGAN x4plus), the fidelity lock, seamless wrapped textures, the
//! videos re-encoded at ×2 and the talking heads at ×4.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ig2_formats::mp4::{self, Codec, SampleOut, Track, TrackOut};
use ig2_formats::pvr::Pvr;
use ig2_formats::vfs::{Mod, MOD_FILE};
use ig2_formats::xv::{Archive, Material, Mesh};
use ig2_formats::Install;

use crate::assets::Image;
use crate::font_sdf;
use crate::learned::{self, rrdb, Gpu, Model, Upscaler};
use crate::upscale::{self, head_file, PACK_DIR};
use crate::video::decode::{Clip, Frames};

/// The pack's folder name under the mods folder.
pub const NAME: &str = "texture-pack";

/// Not a picture of the pack: the font atlas, made into a distance field by its own stage (ADR 0089 §5).
const SKIP: &[&str] = &["dataraw/screen/charset_bmp.pvr"];

const USAGE: &str = "usage: --make-pack [--out <mod folder>] [--only pictures|videos|heads|font|maps] [--factor 2|4] \
                     [--prefix <game path prefix>] [--force] [--fast] [--filters-only]";

/// The pack's parts (ADR 0089 §1), each made alone by `--only`, and whether this version makes it (rows 571–573
/// add the others).
const STAGES: &[(&str, bool)] = &[("pictures", true), ("videos", true), ("heads", true), ("font", true), ("maps", false)];

/// The videos' factor unless `--factor` says (ADR 0089 §4); the pictures' is 4, the talking heads' always 4.
const VIDEO_FACTOR: u8 = 2;
/// A pack video's bit rate over the original's per pixel. OpenH264's Baseline needs more than the original's High
/// profile: at 1 its rate control starves fast motion (`misc/end_x.mp4` at ×2: 5.7 levels a channel off on
/// average in its worst frame), at 2 the worst frame is 2.8 off and the file about 4.5 times the original's.
const BITRATE: f64 = 2.0;
/// A key frame at least this often in a pack's video (10 s at 25 fps).
const KEY_EVERY: u32 = 250;
/// The talking heads (re/formats/ani.md).
const HEADS: &str = "anim/fejek/";

/// Each source texel's colour must survive the upscale within this many levels a channel (ADR 0089 §2).
const LOCK: f32 = 2.0;
/// The lock's rounds at most: two smooth ones, the rest by blocks.
const MAX_ROUNDS: usize = 8;
/// Texels of the opposite side added around a texture the models tile before it is upscaled (ADR 0089 §3).
const WRAP_PAD: usize = 16;

struct Args {
    out: Option<PathBuf>,
    /// `--factor`: the pictures' and videos' (else 4 and [`VIDEO_FACTOR`]).
    factor: Option<u8>,
    /// The one stage to make; every made one when `None`.
    only: Option<&'static str>,
    prefix: String,
    force: bool,
    /// The compact model instead of x4plus.
    fast: bool,
    /// Leave the learned models out: every picture through ADR 0054 §3's filters.
    filters_only: bool,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args { out: None, factor: None, only: None, prefix: String::new(), force: false, fast: false, filters_only: false };
    let mut it = args.iter();
    while let Some(k) = it.next() {
        let mut val = || it.next().cloned().ok_or_else(|| format!("{k}: a value\n{USAGE}"));
        match k.as_str() {
            "--out" => a.out = Some(val()?.into()),
            "--factor" => a.factor = Some(val()?.parse().ok().filter(|f| matches!(f, 2 | 4)).ok_or("--factor: 2 or 4")?),
            "--only" => {
                let v = val()?.to_ascii_lowercase();
                a.only = Some(STAGES.iter().map(|s| s.0).find(|s| *s == v).ok_or(format!("--only {v}: not a stage\n{USAGE}"))?);
            }
            "--prefix" => a.prefix = val()?.to_ascii_lowercase(),
            "--force" => a.force = true,
            "--fast" => a.fast = true,
            "--filters-only" => a.filters_only = true,
            _ => return Err(format!("{k}: unknown\n{USAGE}")),
        }
    }
    Ok(a)
}

/// The pack file for the install texture `path`: `hires/<path without extension>.png` (what `upscale::pack` reads).
pub fn pack_file(path: &str) -> String {
    let stem = path.rsplit_once('.').map_or(path, |(s, _)| s);
    format!("{PACK_DIR}{stem}.png")
}

/// The install's textures the pack covers: every `.pvr` under `prefix` but the font atlas, sorted.
fn textures<'a>(install: &'a Install, prefix: &str) -> Vec<&'a str> {
    let mut v: Vec<&str> =
        install.all_files().into_iter().filter(|p| p.ends_with(".pvr") && p.starts_with(prefix) && !SKIP.contains(p)).collect();
    v.sort_unstable();
    v
}

/// A texture coordinate past the picture: the model tiles its texture there.
fn wraps(t: f32) -> bool {
    !(-1e-3..=1.0 + 1e-3).contains(&t)
}

/// The textures some model wraps (ADR 0089 §3: u or v outside 0..1 on a face that uses it), by the name the install
/// stores them under (`foo.bmp` → `foo_bmp`): whether u wraps, whether v wraps. Every `.xv` archive, every mesh;
/// a material's member is `<archive stem>-<material>.xvm` like its mesh's.
fn wrapped(install: &Install) -> HashMap<String, (bool, bool)> {
    let mut map: HashMap<String, (bool, bool)> = HashMap::new();
    for path in install.all_files().into_iter().filter(|p| p.ends_with(".xv")) {
        let Ok(bytes) = install.read(path) else { continue };
        let Ok(ar) = Archive::parse(&bytes) else { continue };
        for m in ar.members.iter().filter(|m| m.name.to_ascii_lowercase().ends_with(".xvo")) {
            let Ok(mesh) = Mesh::parse(m.data) else { continue };
            let prefix = m.name.find('-').map_or("", |i| &m.name[..=i]);
            let last = mesh.materials.len().saturating_sub(1);
            for (k, mat) in mesh.materials.iter().enumerate() {
                let (mut u, mut v) = (false, false);
                for f in mesh.faces.iter().filter(|f| (f.material as usize).min(last) == k) {
                    for vx in f.idx.iter().filter_map(|&i| mesh.vertices.get(i as usize)) {
                        u |= wraps(vx.uv[0]);
                        v |= wraps(vx.uv[1]);
                    }
                }
                let member = (u || v).then(|| ar.get(&format!("{prefix}{mat}.xvm"))).flatten();
                let Some(mt) = member.and_then(|m| Material::parse(m.data).ok()) else { continue };
                for t in [mt.texture, mt.texture2].iter().filter(|t| !t.is_empty()) {
                    let e = map.entry(t.to_ascii_lowercase().replace('.', "_")).or_default();
                    *e = (e.0 | u, e.1 | v);
                }
            }
        }
    }
    map
}

/// Whether the texture at `path` (`…/foo_bmp.pvr`) wraps in u and in v.
fn wrap_of(map: &HashMap<String, (bool, bool)>, path: &str) -> (bool, bool) {
    let stem = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".pvr");
    map.get(stem).copied().unwrap_or_default()
}

/// The free bytes on the disk holding `dir`, where the system says.
fn free_bytes(dir: &Path) -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
        let mut free = 0u64;
        // SAFETY: a NUL-terminated path and one out-pointer; the others may be null.
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut())
        };
        (ok != 0).then_some(free)
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
        // SAFETY: a NUL-terminated path and a zeroed struct the call fills.
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        (unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0).then(|| s.f_bavail as u64 * s.f_frsize as u64)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = dir;
        None
    }
}

/// The stage's need against the free space of the disk holding `out` (ADR 0089 §8): refused when it lacks it.
fn check_space(out: &Path, need: u64) -> Result<(), String> {
    let mb = |b: u64| b.div_ceil(1 << 20);
    match free_bytes(out) {
        Some(free) if free < need => Err(format!("this stage needs about {} MB; {} has {} MB free", mb(need), out.display(), mb(free))),
        Some(free) => {
            println!("this stage needs about {} MB ({} MB free)", mb(need), mb(free));
            Ok(())
        }
        None => Ok(()),
    }
}

/// The pack's `mod.toml`: a look-only mod (ADR 0086 §2), so it stays out of the multiplayer check.
fn mod_toml(factor: u8) -> String {
    format!(
        "name = \"Texture pack ({factor}x)\"\nkind = \"visual\"\ndescription = \"Made by openIG2 --make-pack from this \
         computer's Imperium Galactica II install. It derives from the game's files: keep it to yourself.\"\n"
    )
}

/// The pictures stage: makes or completes the pack in `out` from `install` (the game's own files, no mods); returns
/// (written, kept, failed).
pub fn make(install: &Install, out: &Path, factor: u8, prefix: &str, force: bool, model: Option<&dyn Upscaler>) -> Result<(usize, usize, usize), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::write(out.join(MOD_FILE), mod_toml(factor)).map_err(|e| format!("{}: {e}", out.display()))?;
    let (list, made): (Vec<&str>, Vec<&str>) = textures(install, prefix).into_iter().partition(|p| force || !out.join(pack_file(p)).is_file());
    // The upscaled pictures' PNGs take about half their RGBA.
    let size = |p: &str| install.read(p).ok().and_then(|b| Pvr::parse(&b).ok().map(|v| v.width as u64 * v.height as u64));
    check_space(out, list.iter().filter_map(|p| size(p)).sum::<u64>() * factor as u64 * factor as u64 * 2)?;
    let wrap = wrapped(install);
    let (mut written, mut failed, mut held) = (0, 0, 0);
    for (i, path) in list.iter().enumerate() {
        let file = out.join(pack_file(path));
        let result = Image::load_original(install, path).and_then(|img| {
            let f = upscale::fit(img.width, img.height, factor).ok_or("too large to upscale")?;
            let (rgba, rounds) = picture(&img, f, model, wrap_of(&wrap, path));
            held += (rounds > 0) as usize;
            let (w, h) = (img.width * f as u32, img.height * f as u32);
            std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
            // Written beside and renamed: a stopped run leaves no half file that a later run would keep.
            let part = file.with_extension("png.part");
            crate::assets::write_png(&part, w, h, &rgba)?;
            std::fs::rename(&part, &file).map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => written += 1,
            Err(e) => {
                failed += 1;
                eprintln!("{path}: {e}");
            }
        }
        if (i + 1) % 50 == 0 || i + 1 == list.len() {
            println!("{}/{} textures", i + 1, list.len());
        }
    }
    if held > 0 {
        println!("{held} pictures had drifted from the original's colours and were held to them (ADR 0089 §2)");
    }
    Ok((written, made.len(), failed))
}

/// `img` at `f` for the pack: a texture the models tile (`wrap`: u, v) padded with its opposite side first so its
/// edges meet, then held to the original's colours; returns the RGBA and the lock's rounds.
fn picture(img: &Image, f: u8, model: Option<&dyn Upscaler>, wrap: (bool, bool)) -> (Vec<u8>, usize) {
    let (w, h) = (img.width as usize, img.height as usize);
    let (px, py) = (if wrap.0 { WRAP_PAD.min(w) } else { 0 }, if wrap.1 { WRAP_PAD.min(h) } else { 0 });
    let mut out = if px + py == 0 {
        scale(img, f, model)
    } else {
        let (pw, ph) = (w + 2 * px, h + 2 * py);
        let at = |x: usize, y: usize| ((y + h - py) % h * w + (x + w - px) % w) * 4;
        let rgba: Vec<u8> = (0..ph).flat_map(|y| (0..pw).flat_map(move |x| img.rgba[at(x, y)..at(x, y) + 4].to_vec())).collect();
        let padded = Image { width: pw as u32, height: ph as u32, rgba, mips: Vec::new(), hires: None };
        let big = scale(&padded, f, model);
        let (f, row) = (f as usize, pw * f as usize * 4);
        (0..h * f).flat_map(|y| big[(y + py * f) * row + px * f * 4..][..w * f * 4].to_vec()).collect()
    };
    let rounds = lock(&img.rgba, w, h, f as usize, &mut out);
    (out, rounds)
}

/// The fidelity lock (ADR 0089 §2): `out` (RGBA, `w·f` × `h·f`) box-downscaled by `f` must match the source `src`
/// (RGBA, `w` × `h`) within ±[`LOCK`] levels per colour channel wherever the source shows (alpha > 0). Where it does
/// not, the difference is added back (iterative back-projection): two rounds spread smoothly (bilinear), then over
/// each texel's block until it settles (more than one only where texels sit at 0 or 255 and the rest take up the
/// difference). Alpha stays as the filter made it. Returns the rounds used.
pub fn lock(src: &[u8], w: usize, h: usize, f: usize, out: &mut [u8]) -> usize {
    let ow = w * f;
    let mut hr: Vec<f32> = out.iter().map(|&v| v as f32).collect();
    let mut rounds = 0;
    loop {
        let mut diff = vec![0.0f32; w * h * 3];
        let mut worst = 0.0f32;
        for y in 0..h {
            for x in 0..w {
                let s = &src[(y * w + x) * 4..][..4];
                if s[3] == 0 {
                    continue;
                }
                for c in 0..3 {
                    let sum: f32 = (0..f * f).map(|k| hr[((y * f + k / f) * ow + x * f + k % f) * 4 + c].round()).sum();
                    let d = s[c] as f32 - sum / (f * f) as f32;
                    diff[(y * w + x) * 3 + c] = d;
                    worst = worst.max(d.abs());
                }
            }
        }
        if worst <= LOCK || rounds == MAX_ROUNDS {
            break;
        }
        rounds += 1;
        // Where a result texel's centre falls between the source texels: (left or top, right or bottom, weight).
        let at = |t: usize, len: usize| {
            let u = ((t as f32 + 0.5) / f as f32 - 0.5).clamp(0.0, (len - 1) as f32);
            let i = (u as usize).min(len.saturating_sub(2));
            (i, (i + 1).min(len - 1), u - i as f32)
        };
        for oy in 0..h * f {
            let (y0, y1, fy) = at(oy, h);
            for ox in 0..ow {
                let (x0, x1, fx) = at(ox, w);
                for c in 0..3 {
                    let g = |x: usize, y: usize| diff[(y * w + x) * 3 + c];
                    let d = if rounds < 3 {
                        (g(x0, y0) * (1.0 - fx) + g(x1, y0) * fx) * (1.0 - fy) + (g(x0, y1) * (1.0 - fx) + g(x1, y1) * fx) * fy
                    } else {
                        g(ox / f, oy / f)
                    };
                    let v = &mut hr[(oy * ow + ox) * 4 + c];
                    *v = (*v + d).clamp(0.0, 255.0);
                }
            }
        }
    }
    for (i, o) in out.iter_mut().enumerate().filter(|(i, _)| i % 4 != 3) {
        *o = hr[i].round() as u8;
    }
    rounds
}

/// The learned model on the graphics card: x4plus unless `fast` asks for the compact one (or x4plus is missing),
/// else `None` (said why: the filters then do every picture).
fn learned_model(fast: bool) -> Option<Box<dyn Upscaler>> {
    if !fast {
        let gpu = rrdb::default_path().ok_or(format!("no models/{}", rrdb::MODEL_FILE)).and_then(|p| rrdb::Model::load(&p)).and_then(|m| rrdb::Gpu::new(&m));
        match gpu {
            Ok(g) => {
                println!("scene textures: Real-ESRGAN x4plus ({}) on the graphics card, in tiles of {} texels", rrdb::MODEL_FILE, g.tile());
                return Some(Box::new(g));
            }
            Err(e) => println!("{e}: the compact model instead"),
        }
    }
    compact_model()
}

/// Real-ESRGAN's compact model on the graphics card (also the videos' model, ADR 0089 §4), else `None` (said why).
fn compact_model() -> Option<Box<dyn Upscaler>> {
    let Some(path) = learned::default_path() else {
        println!("no models/{} beside openIG2: the smooth filter instead", learned::MODEL_FILE);
        return None;
    };
    match Model::load(&path).and_then(|m| Gpu::new(&m)) {
        Ok(g) => {
            println!("Real-ESRGAN compact ({}) on the graphics card", learned::MODEL_FILE);
            Some(Box::new(g))
        }
        Err(e) => {
            println!("{e}: the smooth filter instead");
            None
        }
    }
}

/// The pack's folder and, when it has none yet, its `mod.toml` (a videos or heads run before the pictures').
fn prepare(out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let file = out.join(MOD_FILE);
    if file.is_file() {
        return Ok(());
    }
    std::fs::write(&file, mod_toml(4)).map_err(|e| format!("{}: {e}", file.display()))
}

/// `file` made by `write` as a `.part` beside it, then renamed: a stopped run leaves no half file a later run would keep.
fn write_whole(file: &Path, write: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut part = file.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    write(&part)?;
    std::fs::rename(&part, file).map_err(|e| e.to_string())
}

/// The install's videos under `prefix`: every `anim/mpeg4/*.mp4`, sorted.
fn videos<'a>(install: &'a Install, prefix: &str) -> Vec<&'a str> {
    let mut v: Vec<&str> =
        install.all_files().into_iter().filter(|p| p.starts_with("anim/mpeg4/") && p.ends_with(".mp4") && p.starts_with(prefix)).collect();
    v.sort_unstable();
    v
}

/// The bytes of the file at `p` (0 when there is none).
fn size_of(p: &Path) -> u64 {
    std::fs::metadata(p).map_or(0, |m| m.len())
}

/// The videos stage (ADR 0089 §4): every video's frames at `f` as `hires/<game path>.mp4`; returns (written, kept,
/// failed).
pub fn make_videos(install: &Install, out: &Path, f: u8, prefix: &str, force: bool, model: Option<&dyn Upscaler>) -> Result<(usize, usize, usize), String> {
    prepare(out)?;
    let file = |p: &str| out.join(PACK_DIR).join(p);
    let (list, made): (Vec<&str>, Vec<&str>) = videos(install, prefix).into_iter().partition(|p| force || !file(p).is_file());
    // The bit rate grows with the area (`encode_video`).
    let size = |p: &str| install.read_raw(p).map_or(0, |b| b.len() as u64);
    check_space(out, (list.iter().map(|p| size(p)).sum::<u64>() as f64 * (f as f64 * f as f64) * BITRATE) as u64)?;
    let (mut written, mut failed) = (0, 0);
    for (i, path) in list.iter().enumerate() {
        let started = std::time::Instant::now();
        let result = install
            .read(path)
            .map_err(|e| e.to_string())
            .and_then(Clip::new)
            .and_then(|clip| encode_video(Arc::new(clip), f, model))
            .and_then(|(bytes, frames)| write_whole(&file(path), |p| std::fs::write(p, &bytes).map_err(|e| e.to_string())).map(|()| frames));
        match result {
            Ok(frames) => {
                written += 1;
                println!("{}/{} {path}: {frames} frames, {} kB, in {:.0} s", i + 1, list.len(), size_of(&file(path)) / 1024, started.elapsed().as_secs_f32());
            }
            Err(e) => {
                failed += 1;
                eprintln!("{path}: {e}");
            }
        }
    }
    Ok((written, made.len(), failed))
}

/// One frame of a video at `f` for the pack: the learned model when there is one (its 4× halved for 2×), else
/// Catmull-Rom; held to the original's colours (ADR 0089 §2).
fn video_frame(img: &Image, f: u8, model: Option<&dyn Upscaler>) -> Vec<u8> {
    let (w, h, fu) = (img.width as usize, img.height as usize, f as usize);
    let mut out = match model {
        Some(m) => {
            let x4 = m.upscale(&img.rgba, w, h, &vec![255; w * h * 64]);
            if f == 4 { x4 } else { halve(&x4, w * 4, h * 4) }
        }
        None => upscale::cubic(&img.rgba, w, h, fu),
    };
    lock(&img.rgba, w, h, fu, &mut out);
    out
}

/// RGBA → Y′CbCr 4:2:0 (`w`, `h` even), the exact inverse of the player's `video_fragment.shader` arithmetic
/// (`video::decode::rgba`), so a frame plays back in the colours it was given; each chroma the mean of its 2 × 2.
fn yuv420(rgba: &[u8], w: usize, h: usize) -> Vec<u8> {
    const KB: f32 = 0.391 / 2.018;
    const KR: f32 = 0.813 / 1.596;
    let px = |i: usize| {
        let [r, g, b] = [0, 1, 2].map(|c| rgba[i * 4 + c] as f32 / 255.0);
        let l = (g + KB * b + KR * r) / (1.0 + KB + KR);
        [l / 1.1643 + 0.0625, (b - l) / 2.018 + 0.5, (r - l) / 1.596 + 0.5]
    };
    let q = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    let mut out = vec![0u8; w * h * 3 / 2];
    let (luma, chroma) = out.split_at_mut(w * h);
    let (cb, cr) = chroma.split_at_mut(w * h / 4);
    for (i, y) in luma.iter_mut().enumerate() {
        *y = q(px(i)[0]);
    }
    for cy in 0..h / 2 {
        for cx in 0..w / 2 {
            let sum = (0..4).map(|k| px((2 * cy + k / 2) * w + 2 * cx + k % 2)).fold([0.0; 2], |a, p| [a[0] + p[1], a[1] + p[2]]);
            cb[cy * w / 2 + cx] = q(sum[0] / 4.0);
            cr[cy * w / 2 + cx] = q(sum[1] / 4.0);
        }
    }
    out
}

/// OpenH264's encoder with every frame kept (no skipping), the parameter sets taken out of the stream for `avcC`.
struct H264 {
    enc: openh264::encoder::Encoder,
    sps: Vec<Vec<u8>>,
    pps: Vec<Vec<u8>>,
}

impl H264 {
    fn new(fps: f32, bps: u32) -> Result<Self, String> {
        use openh264::encoder::{BitRate, EncoderConfig, FrameRate, IntraFramePeriod, RateControlMode};
        let cfg = EncoderConfig::new()
            .bitrate(BitRate::from_bps(bps))
            .max_frame_rate(FrameRate::from_hz(fps))
            .rate_control_mode(RateControlMode::Bitrate)
            .skip_frames(false)
            .intra_frame_period(IntraFramePeriod::from_num_frames(KEY_EVERY));
        let enc = openh264::encoder::Encoder::with_api_config(openh264::OpenH264API::from_source(), cfg).map_err(|e| e.to_string())?;
        Ok(Self { enc, sps: Vec::new(), pps: Vec::new() })
    }

    /// One frame (RGBA, `w` × `h`): its sample (4-byte length-prefixed NAL units) and whether it is a key frame.
    fn frame(&mut self, rgba: &[u8], w: usize, h: usize) -> Result<(Vec<u8>, bool), String> {
        let yuv = openh264::formats::YUVBuffer::from_vec(yuv420(rgba, w, h), w, h);
        let bs = self.enc.encode(&yuv).map_err(|e| e.to_string())?;
        let mut sample = Vec::new();
        for layer in (0..bs.num_layers()).filter_map(|i| bs.layer(i)) {
            for nal in (0..layer.nal_count()).filter_map(|j| layer.nal_unit(j)) {
                // Each unit comes with its Annex B start code.
                let nal = nal.iter().position(|&b| b != 0).filter(|&i| nal[i] == 1).map_or(nal, |i| &nal[i + 1..]);
                match nal.first().map(|b| b & 0x1f) {
                    Some(7) if self.sps.is_empty() => self.sps.push(nal.to_vec()),
                    Some(8) if self.pps.is_empty() => self.pps.push(nal.to_vec()),
                    Some(7 | 8) | None => {}
                    Some(_) => {
                        sample.extend_from_slice(&(nal.len() as u32).to_be_bytes());
                        sample.extend_from_slice(nal);
                    }
                }
            }
        }
        if sample.is_empty() {
            return Err("a frame came out empty".into());
        }
        Ok((sample, bs.frame_type() == openh264::encoder::FrameType::IDR))
    }
}

/// Each sample's duration from the decode times, the last's from the track's length (else its predecessor's).
fn durations(t: &Track) -> Vec<u32> {
    let s = &t.samples;
    let mut d: Vec<u32> = s.windows(2).map(|w| (w[1].dts - w[0].dts) as u32).collect();
    let last = s.last().map(|l| t.duration.saturating_sub(l.dts) as u32).filter(|&v| v > 0).or(d.last().copied()).unwrap_or(1);
    d.extend((!s.is_empty()).then_some(last));
    d
}

/// `clip` at `f` (ADR 0089 §4): every frame upscaled and held to its colours, encoded H.264 at the original's bit rate
/// times the area times [`BITRATE`]; the same frame count and timing, the sound's samples copied, both laid out in the original's
/// interleave (the sound's start depends on it). Returns the file and its frame count.
fn encode_video(clip: Arc<Clip>, f: u8, model: Option<&dyn Upscaler>) -> Result<(Vec<u8>, usize), String> {
    let src = clip.video();
    let (w, h) = (clip.size().0 as usize * f as usize, clip.size().1 as usize * f as usize);
    let n = src.samples.len();
    let secs = (n as f64 / clip.fps() as f64).max(0.04);
    let bits = src.samples.iter().map(|s| s.size as f64 * 8.0).sum::<f64>();
    let bps = (bits / secs * (f as f64 * f as f64) * BITRATE).clamp(200e3, 100e6) as u32;
    let mut enc = H264::new(clip.fps(), bps)?;
    let mut frames = Frames::new(clip.clone())?;
    let mut coded: Vec<(Vec<u8>, bool)> = Vec::with_capacity(n);
    while let Some(img) = frames.next_frame() {
        coded.push(enc.frame(&video_frame(&img, f, model), w, h)?);
    }
    if coded.len() != n {
        return Err(format!("{} of {n} frames decoded", coded.len()));
    }
    let codec = Codec::Avc { width: w as u16, height: h as u16, nal_length_size: 4, sps: enc.sps, pps: enc.pps };
    let samples = coded.iter().zip(src.samples.iter().zip(durations(src))).map(|((data, sync), (s, duration))| SampleOut { data, duration, sync: *sync, order: s.offset });
    let video = TrackOut { timescale: src.timescale, codec, samples: samples.collect() };
    let audio = match &clip.mp4.audio {
        Some(a) => {
            let samples = a.samples.iter().zip(durations(a)).map(|(s, duration)| Ok(SampleOut { data: a.data(&clip.data, s)?, duration, sync: true, order: s.offset }));
            Some(TrackOut { timescale: a.timescale, codec: a.codec.clone(), samples: samples.collect::<ig2_formats::Result<_>>().map_err(|e| e.to_string())? })
        }
        None => None,
    };
    Ok((mp4::write(&video, audio.as_ref()).map_err(|e| e.to_string())?, n))
}

/// The install's talking heads under `prefix`, sorted.
fn heads<'a>(install: &'a Install, prefix: &str) -> Vec<&'a str> {
    let mut v: Vec<&str> = install.all_files().into_iter().filter(|p| p.starts_with(HEADS) && p.ends_with(".ani") && p.starts_with(prefix)).collect();
    v.sort_unstable();
    v
}

/// The heads stage (ADR 0089 §4): every frame of every talking head ×4, held to its colours, as [`head_file`]s; a
/// head counts as made once its last frame is there. Returns (written, kept, failed).
pub fn make_heads(install: &Install, out: &Path, prefix: &str, force: bool, model: Option<&dyn Upscaler>) -> Result<(usize, usize, usize), String> {
    prepare(out)?;
    let decoded = |p: &str| {
        let b = install.read(p).map_err(|e| e.to_string())?;
        let ani = ig2_formats::ani::Ani::parse(&b).map_err(|e| e.to_string())?;
        Ok::<_, String>((ani.width as u32, ani.height as u32, ani.frames().map_err(|e| e.to_string())?))
    };
    let count = |p: &str| install.read(p).ok().and_then(|b| ig2_formats::ani::Ani::parse(&b).ok().map(|a| (a.frame_count as usize, a.width as u64 * a.height as u64)));
    let (list, made): (Vec<&str>, Vec<&str>) =
        heads(install, prefix).into_iter().partition(|p| force || count(p).is_none_or(|(n, _)| !out.join(head_file(p, n.saturating_sub(1))).is_file()));
    check_space(out, list.iter().filter_map(|p| count(p)).map(|(n, px)| n as u64 * px * 16 * 2).sum())?;
    let (mut written, mut failed) = (0, 0);
    for path in &list {
        let result = decoded(path).and_then(|(w, h, frames)| {
            for (i, fr) in frames.into_iter().enumerate() {
                let img = Image { width: w, height: h, rgba: fr.rgba, mips: Vec::new(), hires: None };
                let (rgba, _) = picture(&img, 4, model, (false, false));
                write_whole(&out.join(head_file(path, i)), |p| crate::assets::write_png(p, w * 4, h * 4, &rgba))?;
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                written += 1;
                println!("{path}");
            }
            Err(e) => {
                failed += 1;
                eprintln!("{path}: {e}");
            }
        }
    }
    Ok((written, made.len(), failed))
}

/// The font stage (ADR 0089 §5): the atlas's glyphs as a distance field ([`font_sdf::file`]); returns (written, kept,
/// failed).
pub fn make_font(install: &Install, out: &Path, force: bool) -> Result<(usize, usize, usize), String> {
    prepare(out)?;
    let file = out.join(font_sdf::file());
    if !force && file.is_file() {
        return Ok((0, 1, 0));
    }
    let rules = ig2_rules::Rules::from_install(install)?;
    let atlas = Image::load_original(install, font_sdf::ATLAS)?;
    let cs = &rules.charset;
    let sdf = font_sdf::build(&atlas, cs, cs.order.len().max(cs.order_hu.len()));
    write_whole(&file, |p| crate::assets::write_png(p, sdf.width, sdf.height, &sdf.rgba))?;
    Ok((1, 0, 0))
}

/// `img` at `f`: flat-colour art by xBRZ (§3), the rest by the learned model when there is one (its 4× halved for
/// 2×, alpha from the smooth filter), else Catmull-Rom.
fn scale(img: &Image, f: u8, model: Option<&dyn Upscaler>) -> Vec<u8> {
    let Some(gpu) = model.filter(|_| !upscale::is_flat(img)) else { return upscale::upscale(img, f).rgba };
    let (w, h) = (img.width as usize, img.height as usize);
    let x4 = gpu.upscale(&img.rgba, w, h, &upscale::cubic(&img.rgba, w, h, learned::SCALE));
    if f == 4 {
        return x4;
    }
    halve(&x4, w * 4, h * 4)
}

/// A 2 × 2 box filter (`w`, `h` even).
fn halve(rgba: &[u8], w: usize, h: usize) -> Vec<u8> {
    let at = |x: usize, y: usize, c: usize| rgba[(y * w + x) * 4 + c] as u32;
    let px = |x: usize, y: usize, c: usize| ((at(2 * x, 2 * y, c) + at(2 * x + 1, 2 * y, c) + at(2 * x, 2 * y + 1, c) + at(2 * x + 1, 2 * y + 1, c) + 2) / 4) as u8;
    (0..h / 2).flat_map(|y| (0..w / 2).flat_map(move |x| (0..4).map(move |c| px(x, y, c)))).collect()
}

/// `openIG2 --make-pack …`.
pub fn main(args: &[String], install: &Install) -> Result<(), String> {
    let a = parse(args)?;
    if let Some(s) = a.only.filter(|s| !STAGES.contains(&(s, true))) {
        return Err(format!("--only {s}: not made by this version yet (ADR 0089 §6)"));
    }
    let out = a.out.or_else(|| Mod::folder().map(|d| d.join(NAME))).ok_or("no mods folder; pass --out")?;
    println!("making an HD pack in {} (a stopped run continues where it left off)", out.display());
    for (stage, _) in STAGES.iter().filter(|s| s.1 && a.only.is_none_or(|o| o == s.0)) {
        println!("{stage}:");
        let (written, kept, failed) = match *stage {
            "videos" => {
                let gpu = if a.filters_only { None } else { compact_model() };
                make_videos(install, &out, a.factor.unwrap_or(VIDEO_FACTOR), &a.prefix, a.force, gpu.as_deref())?
            }
            "font" => make_font(install, &out, a.force)?,
            "heads" => {
                let gpu = if a.filters_only { None } else { learned_model(a.fast) };
                make_heads(install, &out, &a.prefix, a.force, gpu.as_deref())?
            }
            _ => {
                let gpu = if a.filters_only { None } else { learned_model(a.fast) };
                make(install, &out, a.factor.unwrap_or(4), &a.prefix, a.force, gpu.as_deref())?
            }
        };
        println!("{written} written, {kept} already there, {failed} failed.");
        if failed > 0 {
            return Err(format!("{stage}: {failed} failed"));
        }
    }
    println!("Enable \"{NAME}\" on the Mods page.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_and_names() {
        let a = parse(&["--factor".into(), "2".into(), "--prefix".into(), "DataRaw/Screen/".into(), "--only".into(), "Pictures".into()]).unwrap();
        assert_eq!((a.factor, a.prefix.as_str(), a.only, a.force, a.out, a.fast), (Some(2), "dataraw/screen/", Some("pictures"), false, None, false));
        assert!(parse(&["--factor".into(), "3".into()]).is_err());
        assert!(parse(&["--only".into(), "sounds".into()]).is_err());
        assert!(parse(&["--bogus".into()]).is_err());
        assert!(parse(&["--filters-only".into()]).unwrap().filters_only);
        assert!(parse(&["--fast".into()]).unwrap().fast);
        assert_eq!(halve(&[0, 0, 0, 0, 4, 4, 4, 4, 8, 8, 8, 8, 255, 255, 255, 255], 2, 2), [67; 4]);
        assert_eq!(pack_file("dataraw/textures/colony/foo_bmp.pvr"), "hires/dataraw/textures/colony/foo_bmp.png");
        let info = ig2_formats::vfs::ModInfo::parse(&mod_toml(4)).unwrap();
        assert_eq!(info.kind, ig2_formats::vfs::ModKind::Visual);
        assert!(wraps(1.5) && wraps(-0.25) && !wraps(1.0) && !wraps(0.0));
        let map = HashMap::from([("rock_bmp".to_string(), (true, false))]);
        assert_eq!(wrap_of(&map, "dataraw/textures/colony/rock_bmp.pvr"), (true, false));
        assert_eq!(wrap_of(&map, "dataraw/textures/colony/sand_bmp.pvr"), (false, false));
        assert!(free_bytes(&std::env::temp_dir()).is_some_and(|b| b > 0));
        assert!(check_space(&std::env::temp_dir(), u64::MAX).is_err());
    }

    /// The lock: a result whose colours drifted (a shift over half the picture, noise everywhere) is brought back
    /// within ±2 of the source per texel, its detail kept where it was close; one that was close is left alone.
    #[test]
    fn the_lock_holds_colours() {
        let (w, h, f) = (24, 17, 4);
        let src: Vec<u8> = (0..w * h).flat_map(|i| [(i * 37 % 256) as u8, (i * 11 % 200 + 30) as u8, (i % w * 10) as u8, if i % 7 == 0 { 0 } else { 255 }]).collect();
        let near: Vec<u8> = (0..w * h * f * f).flat_map(|i| {
            let (x, y) = (i % (w * f) / f, i / (w * f) / f);
            let s = &src[(y * w + x) * 4..][..4];
            let wiggle = (i % 3) as i32 - 1;
            [0, 1, 2].map(|c| (s[c] as i32 + wiggle).clamp(0, 255) as u8).into_iter().chain([s[3]])
        }).collect();
        let mut out = near.clone();
        assert_eq!(lock(&src, w, h, f, &mut out), 0);
        assert_eq!(out, near);
        let mut drift: Vec<u8> = near.iter().enumerate().map(|(i, &v)| {
            let x = i / 4 % (w * f);
            if i % 4 == 3 { v } else { (v as i32 + if x < w * f / 2 { 14 } else { -9 } + (i * 7919 % 13) as i32 - 6).clamp(0, 255) as u8 }
        }).collect();
        let alpha: Vec<u8> = drift.iter().skip(3).step_by(4).copied().collect();
        assert!(lock(&src, w, h, f, &mut drift) > 0);
        assert_eq!(drift.iter().skip(3).step_by(4).copied().collect::<Vec<_>>(), alpha, "alpha is left alone");
        for y in 0..h {
            for x in 0..w {
                let s = &src[(y * w + x) * 4..][..4];
                if s[3] == 0 {
                    continue;
                }
                for c in 0..3 {
                    let sum: u32 = (0..f * f).map(|k| drift[((y * f + k / f) * w * f + x * f + k % f) * 4 + c] as u32).sum();
                    let mean = sum as f32 / (f * f) as f32;
                    assert!((mean - s[c] as f32).abs() <= LOCK, "texel {x},{y} channel {c}: {mean} for {}", s[c]);
                }
            }
        }
    }

    /// A wrapped texture is padded with its opposite side: upscaled by a filter that reads its neighbours, its left
    /// edge comes out continuing its right edge; unwrapped, it clamps.
    #[test]
    fn wrapped_textures_meet_at_their_edges() {
        let (w, h) = (8u32, 6u32);
        let rgba: Vec<u8> = (0..w * h).flat_map(|i| [if i % w == 0 { 250 } else if i % w == w - 1 { 10 } else { (i * 37 % 150 + 50) as u8 }, (i * 53 % 200) as u8, 120, 255]).collect();
        let img = Image { width: w, height: h, rgba, mips: Vec::new(), hires: None };
        let (plain, _) = picture(&img, 4, None, (false, false));
        let (wrapped, _) = picture(&img, 4, None, (true, false));
        let left = |p: &[u8]| p[0];
        // The outermost texel's left half: clamped it stays at the edge colour; wrapped it blends towards the right edge.
        assert!(left(&wrapped) < left(&plain), "{} vs {}", left(&wrapped), left(&plain));
        assert_eq!((wrapped.len(), plain.len()), ((w * h * 64) as usize, (w * h * 64) as usize));
    }

    /// The install's models wrap some textures (the planets' and terrains' repeats); the scan finds them.
    #[test]
    fn the_install_wraps_some_textures() {
        use ig2_formats::text::Lang;
        let Some(Ok(install)) = Install::from_env(Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let map = wrapped(&install);
        assert!(!map.is_empty());
        let hits = textures(&install, "").iter().filter(|p| wrap_of(&map, p) != (false, false)).count();
        assert!(hits > 0, "no install texture is wrapped");
        eprintln!("{} wrapped texture names, {hits} install textures", map.len());
    }

    /// `hi` (`w·f` × `h·f`, RGBA) box-downscaled by `f` against `lo` (`w` × `h`): the largest and the mean difference
    /// over the colour channels.
    fn box_diff(lo: &[u8], hi: &[u8], w: usize, h: usize, f: usize) -> (f32, f32) {
        let d: Vec<f32> = (0..w * h * 3)
            .map(|i| {
                let (x, y, c) = (i / 3 % w, i / 3 / w, i % 3);
                let sum: f32 = (0..f * f).map(|k| hi[((y * f + k / f) * w * f + x * f + k % f) * 4 + c] as f32).sum();
                (sum / (f * f) as f32 - lo[(y * w + x) * 4 + c] as f32).abs()
            })
            .collect();
        (d.iter().copied().fold(0.0, f32::max), d.iter().sum::<f32>() / d.len() as f32)
    }

    /// The encoder's conversion is the player's inverse: a colour the player can show comes back within a level.
    #[test]
    fn colours_survive_the_conversion() {
        let mut n = 0;
        for y in (16..=235).step_by(9) {
            for cb in (40..=216).step_by(11) {
                for cr in (40..=216).step_by(11) {
                    let rgb = crate::video::decode::shade(y as f32 / 255.0, cb as f32 / 255.0, cr as f32 / 255.0);
                    if rgb.iter().any(|v| !(2..=253).contains(v)) {
                        continue;
                    }
                    let yuv = yuv420(&[rgb[0], rgb[1], rgb[2], 255].repeat(4), 2, 2);
                    let back = crate::video::decode::shade(yuv[0] as f32 / 255.0, yuv[4] as f32 / 255.0, yuv[5] as f32 / 255.0);
                    assert!(back.iter().zip(rgb).all(|(a, b)| a.abs_diff(b) <= 1), "{rgb:?} came back {back:?}");
                    n += 1;
                }
            }
        }
        assert!(n > 1000, "{n} colours");
    }

    /// A video made for the pack: the same frame count, frame rate and sound (its samples and their place in the file,
    /// so the sound starts as the original's), twice the size, and each frame held to the original's colours (the lock
    /// per frame, then H.264's loss).
    #[test]
    fn a_video_keeps_frames_timing_sound_and_colours() {
        let (w, h, n) = (64usize, 48usize, 8usize);
        let frame = |k: usize| -> Vec<u8> {
            (0..w * h).flat_map(|i| [((i % w * 4 + k * 6) % 256) as u8, (i / w * 5) as u8, ((i % w + i / w) * 2 + 40) as u8, 255]).collect()
        };
        let mut enc = H264::new(25.0, 2_000_000).unwrap();
        let coded: Vec<(Vec<u8>, bool)> = (0..n).map(|k| enc.frame(&frame(k), w, h).unwrap()).collect();
        assert!(coded[0].1 && !coded[1].1);
        let sound: Vec<Vec<u8>> = (0..4u8).map(|i| vec![i; 20]).collect();
        let video = TrackOut {
            timescale: 12800,
            codec: Codec::Avc { width: w as u16, height: h as u16, nal_length_size: 4, sps: enc.sps.clone(), pps: enc.pps.clone() },
            samples: coded.iter().enumerate().map(|(i, (data, sync))| SampleOut { data, duration: 512, sync: *sync, order: i as u64 * 10 }).collect(),
        };
        let audio = TrackOut {
            timescale: 44100,
            codec: Codec::Aac { sample_rate: 44100, channels: 2, config: vec![0x12, 0x10] },
            samples: sound.iter().enumerate().map(|(i, data)| SampleOut { data, duration: 1024, sync: true, order: i as u64 * 20 + 15 }).collect(),
        };
        let src = Arc::new(Clip::new(mp4::write(&video, Some(&audio)).unwrap()).unwrap());
        let (bytes, frames) = encode_video(src.clone(), 2, None).unwrap();
        assert_eq!(frames, n);
        let hi = Arc::new(Clip::new(bytes).unwrap());
        assert_eq!((hi.size(), hi.fps(), hi.video().samples.len(), hi.sound_wait()), ((128, 96), 25.0, n, src.sound_wait()));
        let (a, b) = (src.mp4.audio.as_ref().unwrap(), hi.mp4.audio.as_ref().unwrap());
        for (s, t) in a.samples.iter().zip(&b.samples) {
            assert_eq!((a.data(&src.data, s).unwrap(), s.dts), (b.data(&hi.data, t).unwrap(), t.dts));
        }
        assert_eq!((a.samples.len(), a.duration), (b.samples.len(), b.duration));
        let (mut lo, mut up) = (Frames::new(src).unwrap(), Frames::new(hi).unwrap());
        for k in 0..n {
            let (l, u) = (lo.next_frame().unwrap(), up.next_frame().unwrap());
            assert_eq!((u.width, u.height), (128, 96));
            // The lock alone holds within ±2 (`the_lock_holds_colours`); the encoder adds its loss.
            let locked = video_frame(&l, 2, None);
            assert!(box_diff(&l.rgba, &locked, w, h, 2).0 <= LOCK, "frame {k}: the lock");
            let (worst, mean) = box_diff(&l.rgba, &u.rgba, w, h, 2);
            assert!(mean <= 2.0 && worst <= 24.0, "frame {k}: {worst} worst, {mean} mean");
        }
        assert!(lo.next_frame().is_none() && up.next_frame().is_none());
    }

    /// The install's shortest video and the empty talking head made into a pack play from it in the client: the video
    /// at twice the size in the original's place, every head frame with its 4× picture; a second run keeps them.
    #[test]
    fn a_made_video_and_head_play() {
        use ig2_formats::text::Lang;
        let Some(Ok(install)) = Install::from_env(Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("ig2-make-pack-av-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (video, head) = ("anim/mpeg4/misc/end_x.mp4", "anim/fejek/ures.ani");
        assert_eq!(make_videos(&install, &dir, 2, video, false, None).unwrap(), (1, 0, 0));
        assert_eq!(make_videos(&install, &dir, 2, video, false, None).unwrap(), (0, 1, 0));
        assert_eq!(make_heads(&install, &dir, head, false, None).unwrap(), (1, 0, 0));
        assert_eq!(make_heads(&install, &dir, head, false, None).unwrap(), (0, 1, 0));
        let modded = Install::open_with_mods(install.root(), Lang::En, vec![Mod { name: NAME.into(), dir: dir.clone() }]).unwrap();
        let (mut plain, mut packed) = (crate::video::Player::open(&install, video, false).unwrap(), crate::video::Player::open(&modded, video, false).unwrap());
        assert_eq!(packed.size(), plain.size());
        let (mut n, (w, h)) = (0, plain.size());
        while let (Some(a), Some(b)) = (plain.next_blocking(), packed.next_blocking()) {
            assert_eq!((b.width, b.height), (w * 2, h * 2));
            let (worst, mean) = box_diff(&a.rgba, &b.rgba, w as usize, h as usize, 2);
            assert!(mean <= 3.0, "frame {n}: {worst} worst, {mean} mean");
            n += 1;
        }
        assert!(n > 0 && plain.next_blocking().is_none() && packed.next_blocking().is_none());
        let (a, b) = (upscale::head_frames(&install, head, true).unwrap(), upscale::head_frames(&modded, head, true).unwrap());
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(&b) {
            assert!(a.hires.is_none() && b.rgba == a.rgba);
            let hi = b.hires.as_deref().unwrap();
            assert_eq!((hi.width, hi.height, hi.upscaled), (a.width * 4, a.height * 4, true));
            assert!(hi.rgba.chunks_exact(4).all(|p| p[3] == 255));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A pack made from the install loads in the client: the image keeps its size and pixels, draws the pack's
    /// picture at 4×; a second run keeps what is there.
    #[test]
    fn a_made_pack_loads() {
        use ig2_formats::text::Lang;
        let Some(Ok(install)) = Install::from_env(Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("ig2-make-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let only = "dataraw/screen/mouse";
        let n = textures(&install, only).len();
        assert!(n > 0);
        assert_eq!(make(&install, &dir, 4, only, false, None).unwrap(), (n, 0, 0));
        assert_eq!(make(&install, &dir, 4, only, false, None).unwrap(), (0, n, 0));
        assert!(textures(&install, "dataraw/screen/charset").is_empty());
        let modded = Install::open_with_mods(install.root(), Lang::En, vec![Mod { name: NAME.into(), dir: dir.clone() }]).unwrap();
        let path = "dataraw/screen/mouse_bmp.pvr";
        let (a, b) = (Image::load_original(&install, path).unwrap(), Image::load(&modded, path).unwrap());
        assert_eq!((b.width, b.height, &b.rgba), (a.width, a.height, &a.rgba));
        let hi = b.hires.as_deref().unwrap();
        assert_eq!((hi.width, hi.height), (a.width * 4, a.height * 4));
        let mut want = upscale::upscale(&a, 4).rgba;
        lock(&a.rgba, a.width as usize, a.height as usize, 4, &mut want);
        assert_eq!(hi.rgba, want);
        assert_eq!(crate::mods_fingerprint(&modded), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
