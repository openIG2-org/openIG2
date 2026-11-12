//! `openIG2 --make-pack`: a texture pack (ADR 0054 §5) made from the player's own install on their machine (ADR 0088
//! §5–6, row 567). Every install texture the client draws, upscaled, as `mods/<name>/hires/<game path>.png`; the pack
//! derives from the game's files, so it is never shared (the mod index carries none). ADR 0089 makes it the HD pack:
//! stages (`--only`), the quality model (Real-ESRGAN x4plus), the fidelity lock, seamless wrapped textures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ig2_formats::pvr::Pvr;
use ig2_formats::vfs::{Mod, MOD_FILE};
use ig2_formats::xv::{Archive, Material, Mesh};
use ig2_formats::Install;

use crate::assets::Image;
use crate::learned::{self, rrdb, Gpu, Model, Upscaler};
use crate::upscale::{self, PACK_DIR};

/// The pack's folder name under the mods folder.
pub const NAME: &str = "texture-pack";

/// Drawn as they are (`Image::load_original`): the font atlas (ADR 0054 §3).
const SKIP: &[&str] = &["dataraw/screen/charset_bmp.pvr"];

const USAGE: &str = "usage: --make-pack [--out <mod folder>] [--only pictures|videos|heads|font|maps] [--factor 2|4] \
                     [--prefix <game path prefix>] [--force] [--fast] [--filters-only]";

/// The pack's parts (ADR 0089 §1), each made alone by `--only`, and whether this version makes it (rows 571–573
/// add the others).
const STAGES: &[(&str, bool)] = &[("pictures", true), ("videos", false), ("heads", false), ("font", false), ("maps", false)];

/// Each source texel's colour must survive the upscale within this many levels a channel (ADR 0089 §2).
const LOCK: f32 = 2.0;
/// Texels of the opposite side added around a texture the models tile before it is upscaled (ADR 0089 §3).
const WRAP_PAD: usize = 16;

struct Args {
    out: Option<PathBuf>,
    factor: u8,
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
    let mut a = Args { out: None, factor: 4, only: None, prefix: String::new(), force: false, fast: false, filters_only: false };
    let mut it = args.iter();
    while let Some(k) = it.next() {
        let mut val = || it.next().cloned().ok_or_else(|| format!("{k}: a value\n{USAGE}"));
        match k.as_str() {
            "--out" => a.out = Some(val()?.into()),
            "--factor" => a.factor = val()?.parse().ok().filter(|f| matches!(f, 2 | 4)).ok_or("--factor: 2 or 4")?,
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
/// not, the difference is added back (iterative back-projection): two rounds spread smoothly (bilinear), a third
/// over each texel's block, which settles it. Alpha stays as the filter made it. Returns the rounds used.
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
        if worst <= LOCK || rounds == 3 {
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
    let Some(path) = learned::default_path() else {
        println!("no models/{} beside openIG2: the scene textures get the smooth filter", learned::MODEL_FILE);
        return None;
    };
    match Model::load(&path).and_then(|m| Gpu::new(&m)) {
        Ok(g) => {
            println!("scene textures: Real-ESRGAN compact ({}) on the graphics card", learned::MODEL_FILE);
            Some(Box::new(g))
        }
        Err(e) => {
            println!("{e}: the scene textures get the smooth filter");
            None
        }
    }
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
        return Err(format!("--only {s}: not made by this version yet (ADR 0089 §4–6)"));
    }
    let out = a.out.or_else(|| Mod::folder().map(|d| d.join(NAME))).ok_or("no mods folder; pass --out")?;
    println!("making a {}x HD pack in {} (a stopped run continues where it left off)", a.factor, out.display());
    for (stage, _) in STAGES.iter().filter(|s| s.1 && a.only.is_none_or(|o| o == s.0)) {
        println!("{stage}:");
        let gpu = if a.filters_only { None } else { learned_model(a.fast) };
        let (written, kept, failed) = make(install, &out, a.factor, &a.prefix, a.force, gpu.as_deref())?;
        println!("{written} written, {kept} already there, {failed} failed.");
        if failed > 0 {
            return Err(format!("{failed} textures failed"));
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
        assert_eq!((a.factor, a.prefix.as_str(), a.only, a.force, a.out, a.fast), (2, "dataraw/screen/", Some("pictures"), false, None, false));
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
