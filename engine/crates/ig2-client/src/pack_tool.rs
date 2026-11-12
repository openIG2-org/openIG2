//! `openIG2 --make-pack`: a texture pack (ADR 0054 §5) made from the player's own install on their machine (ADR 0088
//! §5–6, row 567). Every install texture the client draws, upscaled, as `mods/<name>/hires/<game path>.png`; the pack
//! derives from the game's files, so it is never shared (the mod index carries none).

use std::path::{Path, PathBuf};

use ig2_formats::vfs::{Mod, MOD_FILE};
use ig2_formats::Install;

use crate::assets::Image;
use crate::learned::{self, Gpu, Model};
use crate::upscale::{self, PACK_DIR};

/// The pack's folder name under the mods folder.
pub const NAME: &str = "texture-pack";

/// Drawn as they are (`Image::load_original`): the font atlas (ADR 0054 §3).
const SKIP: &[&str] = &["dataraw/screen/charset_bmp.pvr"];

const USAGE: &str = "usage: --make-pack [--out <mod folder>] [--factor 2|4] [--only <game path prefix>] [--force] [--filters-only]";

struct Args {
    out: Option<PathBuf>,
    factor: u8,
    only: String,
    force: bool,
    /// Leave the learned model out: every picture through ADR 0054 §3's filters.
    filters_only: bool,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args { out: None, factor: 4, only: String::new(), force: false, filters_only: false };
    let mut it = args.iter();
    while let Some(k) = it.next() {
        let mut val = || it.next().cloned().ok_or_else(|| format!("{k}: a value\n{USAGE}"));
        match k.as_str() {
            "--out" => a.out = Some(val()?.into()),
            "--factor" => a.factor = val()?.parse().ok().filter(|f| matches!(f, 2 | 4)).ok_or("--factor: 2 or 4")?,
            "--only" => a.only = val()?.to_ascii_lowercase(),
            "--force" => a.force = true,
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

/// The install's textures the pack covers: every `.pvr` under `only` but the font atlas, sorted.
fn textures<'a>(install: &'a Install, only: &str) -> Vec<&'a str> {
    let mut v: Vec<&str> =
        install.all_files().into_iter().filter(|p| p.ends_with(".pvr") && p.starts_with(only) && !SKIP.contains(p)).collect();
    v.sort_unstable();
    v
}

/// The pack's `mod.toml`: a look-only mod (ADR 0086 §2), so it stays out of the multiplayer check.
fn mod_toml(factor: u8) -> String {
    format!(
        "name = \"Texture pack ({factor}x)\"\nkind = \"visual\"\ndescription = \"Made by openIG2 --make-pack from this \
         computer's Imperium Galactica II install. It derives from the game's files: keep it to yourself.\"\n"
    )
}

/// Makes or completes the pack in `out` from `install` (the game's own files, no mods); returns (written, kept, failed).
pub fn make(install: &Install, out: &Path, factor: u8, only: &str, force: bool, model: Option<&Gpu>) ->Result<(usize, usize, usize), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::write(out.join(MOD_FILE), mod_toml(factor)).map_err(|e| format!("{}: {e}", out.display()))?;
    let list = textures(install, only);
    let (mut written, mut kept, mut failed) = (0, 0, 0);
    for (i, path) in list.iter().enumerate() {
        let file = out.join(pack_file(path));
        if !force && file.is_file() {
            kept += 1;
            continue;
        }
        let made = Image::load_original(install, path).and_then(|img| {
            let f = upscale::fit(img.width, img.height, factor).ok_or("too large to upscale")?;
            let rgba = scale(&img, f, model);
            let (w, h) = (img.width * f as u32, img.height * f as u32);
            std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
            // Written beside and renamed: a stopped run leaves no half file that a later run would keep.
            let part = file.with_extension("png.part");
            crate::assets::write_png(&part, w, h, &rgba)?;
            std::fs::rename(&part, &file).map_err(|e| e.to_string())
        });
        match made {
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
    Ok((written, kept, failed))
}

/// The learned model on the graphics card, else `None` (said why: the filters then do every picture).
fn learned_model() -> Option<Gpu> {
    let Some(path) = learned::default_path() else {
        println!("no models/{} beside openIG2: the scene textures get the smooth filter", learned::MODEL_FILE);
        return None;
    };
    match Model::load(&path).and_then(|m| Gpu::new(&m)) {
        Ok(g) => {
            println!("scene textures: Real-ESRGAN ({}) on the graphics card", learned::MODEL_FILE);
            Some(g)
        }
        Err(e) => {
            println!("{e}: the scene textures get the smooth filter");
            None
        }
    }
}

/// `img` at `f`: flat-colour art by xBRZ (§3), the rest by the learned model when there is one (its 4× halved for
/// 2×, alpha from the smooth filter), else Catmull-Rom.
fn scale(img: &Image, f: u8, model: Option<&Gpu>) -> Vec<u8> {
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
    let out = a.out.or_else(|| Mod::folder().map(|d| d.join(NAME))).ok_or("no mods folder; pass --out")?;
    println!("making a {}x texture pack in {} (a stopped run continues where it left off)", a.factor, out.display());
    let gpu = if a.filters_only { None } else { learned_model() };
    let (written, kept, failed) = make(install, &out, a.factor, &a.only, a.force, gpu.as_ref())?;
    println!("{written} written, {kept} already there, {failed} failed. Enable \"{NAME}\" on the Mods page.");
    if failed > 0 {
        return Err(format!("{failed} textures failed"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_and_names() {
        let a = parse(&["--factor".into(), "2".into(), "--only".into(), "DataRaw/Screen/".into()]).unwrap();
        assert_eq!((a.factor, a.only.as_str(), a.force, a.out), (2, "dataraw/screen/", false, None));
        assert!(parse(&["--factor".into(), "3".into()]).is_err());
        assert!(parse(&["--bogus".into()]).is_err());
        assert!(parse(&["--filters-only".into()]).unwrap().filters_only);
        assert_eq!(halve(&[0, 0, 0, 0, 4, 4, 4, 4, 8, 8, 8, 8, 255, 255, 255, 255], 2, 2), [67; 4]);
        assert_eq!(pack_file("dataraw/textures/colony/foo_bmp.pvr"), "hires/dataraw/textures/colony/foo_bmp.png");
        let info = ig2_formats::vfs::ModInfo::parse(&mod_toml(4)).unwrap();
        assert_eq!(info.kind, ig2_formats::vfs::ModKind::Visual);
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
        assert_eq!(hi.rgba, upscale::upscale(&a, 4).rgba);
        assert_eq!(crate::mods_fingerprint(&modded), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
