//! Read-only view of the user's IG2 install.
//!
//! Path resolution mirrors the remaster's `AssetHelper::rule_t` set up in `main` (ELF 0x442389):
//! a rule rewrites `""` → `"localization/<lang>/"`, then falls back to the unprefixed path.
//! The localization tree mirrors the root layout (`localization/hu/dataraw/text/...`).
//! Lookups are case-insensitive because the 1999 code uses names like `HADIGYAR.ani`.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use crate::container::{self, DecryptingReader};
use crate::text::Lang;
use crate::{Error, Result};

/// The environment variable tests and tools use to find the install.
pub const INSTALL_ENV: &str = "IG2_INSTALL_DIR";

pub struct Install {
    root: PathBuf,
    lang: Lang,
    /// Lowercase relative path with `/` separators → absolute path (a mod's file over the install's).
    index: HashMap<String, PathBuf>,
    /// The enabled mods, highest priority first (ADR 0044).
    mods: Vec<Mod>,
    /// Paths a mod provides → (index in `mods`, whether the install has the path too).
    modded: HashMap<String, (usize, bool)>,
}

/// An enabled mod: a folder mirroring the install's layout (ADR 0044).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mod {
    pub name: String,
    pub dir: PathBuf,
}

impl Mod {
    /// The mods folder beside the settings files (`mods/` next to [`Options::path`](crate::options::Options::path)).
    pub fn folder() -> Option<PathBuf> {
        crate::options::Options::path().and_then(|p| p.parent().map(|d| d.join("mods")))
    }

    /// `mods/enabled.txt`'s names in order: one a line, `#` comments and blank lines skipped.
    pub fn parse_enabled(text: &str) -> Vec<String> {
        text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()).map(str::to_owned).collect()
    }

    /// The mods to load (ADR 0044 §2): `given` (`--mod`), else `mods/enabled.txt`'s names, found in [`Mod::folder`];
    /// the names without a folder are the second list.
    pub fn enabled(given: Option<Vec<String>>) -> (Vec<Mod>, Vec<String>) {
        let Some(folder) = Self::folder() else { return (Vec::new(), given.unwrap_or_default()) };
        let names = given.unwrap_or_else(|| std::fs::read_to_string(folder.join("enabled.txt")).map(|t| Self::parse_enabled(&t)).unwrap_or_default());
        Self::find(&folder, &names)
    }

    /// The named mods in `folder`, in the given order; the names without a folder are the second list.
    pub fn find(folder: &Path, names: &[String]) -> (Vec<Mod>, Vec<String>) {
        let (found, missing): (Vec<&String>, Vec<&String>) = names.iter().partition(|n| folder.join(n).is_dir());
        (found.into_iter().map(|n| Mod { name: n.clone(), dir: folder.join(n) }).collect(), missing.into_iter().cloned().collect())
    }
}

/// One file an enabled mod provides ([`Install::overrides`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override<'a> {
    /// The game path (lowercase, `/`-separated).
    pub path: &'a str,
    pub mod_name: &'a str,
    pub file: &'a Path,
    /// The install has this path too (otherwise the mod adds it).
    pub replaces: bool,
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").trim_start_matches('/').to_ascii_lowercase()
}

impl Install {
    pub fn open(root: impl Into<PathBuf>, lang: Lang) -> Result<Self> {
        Self::open_with_mods(root, lang, Vec::new())
    }

    /// The install with `mods` (highest priority first) over it (ADR 0044): the first mod that has a path wins.
    pub fn open_with_mods(root: impl Into<PathBuf>, lang: Lang, mods: Vec<Mod>) -> Result<Self> {
        let root = root.into();
        let mut index = scan(&root)?;
        if !index.contains_key("dataraw/text/scenario.txt") {
            return Err(Error::NotFound(format!(
                "{} doesn't look like an IG2 install (no dataraw/text/scenario.txt)",
                root.display()
            )));
        }
        let mut modded = HashMap::new();
        for (i, m) in mods.iter().enumerate().rev() {
            // The rule tables are read from the exe (ADR 0004); a mod never replaces it (part 2: rule tables).
            for (k, p) in scan(&m.dir)?.into_iter().filter(|(k, _)| !k.ends_with(".exe")) {
                let replaces = modded.get(&k).map_or_else(|| index.contains_key(&k), |&(_, r)| r);
                modded.insert(k.clone(), (i, replaces));
                index.insert(k, p);
            }
        }
        Ok(Self { root, lang, index, mods, modded })
    }

    /// The same install and mods in another texts' language.
    pub fn with_lang(&self, lang: Lang) -> Result<Self> {
        Self::open_with_mods(&self.root, lang, self.mods.clone())
    }

    /// The enabled mods, highest priority first.
    pub fn mods(&self) -> &[Mod] {
        &self.mods
    }

    /// Every file a mod provides, in path order.
    pub fn overrides(&self) -> Vec<Override<'_>> {
        let mut v: Vec<Override> = self
            .modded
            .iter()
            .map(|(k, &(i, replaces))| Override { path: k, mod_name: &self.mods[i].name, file: &self.index[k], replaces })
            .collect();
        v.sort_unstable_by_key(|o| o.path);
        v
    }

    /// Open the install named by `IG2_INSTALL_DIR`, if set.
    pub fn from_env(lang: Lang) -> Option<Result<Self>> {
        std::env::var_os(INSTALL_ENV).map(|p| Self::open(PathBuf::from(p), lang))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Resolve a game path to a file on disk, preferring the localized copy.
    /// A mod's copy of either path comes before the install's (ADR 0044 §3).
    pub fn resolve(&self, path: &str) -> Option<&Path> {
        let p = normalize(path);
        let keys = [self.lang.dir().map(|d| format!("localization/{d}/{p}")), Some(p)];
        let keys = keys.iter().flatten();
        let key = keys.clone().find(|k| self.modded.contains_key(*k)).or_else(|| keys.clone().find(|k| self.index.contains_key(*k)))?;
        self.index.get(key).map(PathBuf::as_path)
    }

    /// Raw on-disk bytes (still wrapped in the container).
    pub fn read_raw(&self, path: &str) -> Result<Vec<u8>> {
        let p = self.resolve(path).ok_or_else(|| Error::NotFound(path.to_owned()))?;
        std::fs::read(p).map_err(|source| Error::Io { path: p.display().to_string(), source })
    }

    /// Fully decoded file contents.
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let raw = self.read_raw(path)?;
        Ok(container::decode(path, &raw)?.into_owned())
    }

    /// Seekable decrypted stream, for `neGs` media (Ogg, MP4).
    pub fn open_stream(&self, path: &str) -> Result<DecryptingReader<BufReader<File>>> {
        let p = self.resolve(path).ok_or_else(|| Error::NotFound(path.to_owned()))?;
        let io = |source| Error::Io { path: p.display().to_string(), source };
        let f = File::open(p).map_err(io)?;
        DecryptingReader::new(path, BufReader::new(f)).map_err(io)
    }

    /// Resolve a 1999 `.bmp` texture name used by a `.xv` material to its remaster `.pvr`.
    ///
    /// The game's rule is [`Install::resolve_texture`] with the `TextureDir` of the screen that loads
    /// the model. This approximates `TextureDir` with the archive's top-level
    /// `dataraw/vector/<screen>/` directory, then falls back to any screen directory. It resolves
    /// 99.8% of the references; the misses are textures the game generates at runtime (planet
    /// maps, loading screens).
    pub fn resolve_model_texture(&self, screen: &str, bmp_name: &str) -> Option<String> {
        if let Some(p) = self.resolve_texture(&format!("textures/{screen}/"), bmp_name) {
            return Some(p);
        }
        texture_candidates(bmp_name).into_iter().find_map(|file| {
            let suffix = format!("/{file}.pvr");
            let mut any: Vec<&String> =
                self.index.keys().filter(|k| k.starts_with("dataraw/textures/") && k.ends_with(&suffix)).collect();
            any.sort();
            any.first().map(|k| (*k).clone())
        })
    }

    /// The game's own texture lookup for a material texture name: `loadtexture` (ELF 0x5ff310)
    /// prefixes the screen's `TextureDir` (`textures/colony/`, `textures/design/screen/`,
    /// `textures/colony/terrainmap/<n>/`, …, set by each screen's init code), `D3DTexture::Load`
    /// (0x4f5e60) retries with the name's last character replaced by `t`, then `f`, and
    /// `command_glLoadTexture` (0x426e30) turns every `.` into `_` and asks for `dataraw/<path>` as a
    /// texture resource (`.pvr`).
    pub fn resolve_texture(&self, texture_dir: &str, name: &str) -> Option<String> {
        let dir = normalize(texture_dir);
        texture_candidates(name).into_iter().map(|file| format!("dataraw/{dir}{file}.pvr")).find(|p| self.index.contains_key(p))
    }

    /// Every file in the install, including localization copies (lowercase, `/`-separated).
    pub fn all_files(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.index.keys().map(String::as_str).collect();
        v.sort_unstable();
        v
    }

    /// All relative paths (lowercase, `/`-separated) under `prefix`, excluding localization copies.
    pub fn list(&self, prefix: &str) -> Vec<&str> {
        let prefix = normalize(prefix);
        let mut v: Vec<&str> = self
            .index
            .keys()
            .filter(|k| k.starts_with(&prefix) && !k.starts_with("localization/"))
            .map(String::as_str)
            .collect();
        v.sort_unstable();
        v
    }
}

/// Every file under `root`: lowercase `/`-separated relative path → absolute path.
fn scan(root: &Path) -> Result<HashMap<String, PathBuf>> {
    let mut index = HashMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|source| Error::Io { path: dir.display().to_string(), source })?;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(rel) = p.strip_prefix(root) {
                index.insert(normalize(&rel.to_string_lossy()), p);
            }
        }
    }
    Ok(index)
}

/// `foo.bmp`, then `foo.bmt`, then `foo.bmf` (`D3DTexture::Load`), with `.` → `_`, lowercased.
fn texture_candidates(name: &str) -> Vec<String> {
    let name = name.to_ascii_lowercase();
    let mut out = vec![name.replace('.', "_")];
    if let Some((i, _)) = name.char_indices().last() {
        out.extend(['t', 'f'].map(|c| format!("{}{c}", &name[..i]).replace('.', "_")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny synthetic install and two mods under the temp folder (no game data).
    fn tree(tag: &str) -> PathBuf {
        let t = std::env::temp_dir().join(format!("ig2-vfs-mods-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        for (rel, body) in [
            ("install/dataraw/text/scenario.txt", "base"),
            ("install/dataraw/textures/colony/a_bmp.pvr", "base a"),
            ("install/dataraw/textures/colony/b_bmp.pvr", "base b"),
            ("install/localization/hu/dataraw/textures/colony/a_bmp.pvr", "base hu a"),
            ("install/ig2.exe", "exe"),
            ("mods/first/DataRaw/Textures/Colony/A_BMP.pvr", "first a"),
            ("mods/first/ig2.exe", "modded exe"),
            ("mods/second/dataraw/textures/colony/a_bmp.pvr", "second a"),
            ("mods/second/dataraw/textures/colony/b_bmp.pvr", "second b"),
            ("mods/second/dataraw/textures/colony/new_bmp.pvr", "second new"),
        ] {
            let p = t.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
        t
    }

    fn text(i: &Install, path: &str) -> String {
        String::from_utf8(i.read(path).unwrap()).unwrap()
    }

    #[test]
    fn mods_win_in_list_order() {
        let t = tree("order");
        let names = Mod::parse_enabled("# load order\nfirst\n\n  second  # comment\nmissing\n");
        assert_eq!(names, ["first", "second", "missing"]);
        let (mods, missing) = Mod::find(&t.join("mods"), &names);
        assert_eq!(missing, ["missing"]);
        let i = Install::open_with_mods(t.join("install"), Lang::En, mods).unwrap();
        assert_eq!(text(&i, "dataraw/textures/colony/a_bmp.pvr"), "first a");
        assert_eq!(text(&i, r"DATARAW\textures\colony\b_bmp.pvr"), "second b");
        assert_eq!(text(&i, "dataraw/textures/colony/new_bmp.pvr"), "second new");
        assert_eq!(text(&i, "dataraw/text/scenario.txt"), "base");
        assert_eq!(i.resolve_texture("textures/colony/", "new.bmp").as_deref(), Some("dataraw/textures/colony/new_bmp.pvr"));
        // The exe stays the install's (rule tables are part 2).
        assert_eq!(text(&i, "ig2.exe"), "exe");
        let o: Vec<(&str, &str, bool)> = i.overrides().iter().map(|o| (o.path, o.mod_name, o.replaces)).collect();
        assert_eq!(
            o,
            [
                ("dataraw/textures/colony/a_bmp.pvr", "first", true),
                ("dataraw/textures/colony/b_bmp.pvr", "second", true),
                ("dataraw/textures/colony/new_bmp.pvr", "second", false),
            ]
        );
        // The other order: the second mod's copy wins.
        let (mods, _) = Mod::find(&t.join("mods"), &["second".into(), "first".into()]);
        let i = Install::open_with_mods(t.join("install"), Lang::En, mods).unwrap();
        assert_eq!(text(&i, "dataraw/textures/colony/a_bmp.pvr"), "second a");
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn a_mod_beats_the_installs_localized_copy() {
        let t = tree("lang");
        let plain = Install::open(t.join("install"), Lang::Hu).unwrap();
        assert_eq!(text(&plain, "dataraw/textures/colony/a_bmp.pvr"), "base hu a");
        assert!(plain.overrides().is_empty());
        let (mods, _) = Mod::find(&t.join("mods"), &["second".into()]);
        let i = Install::open_with_mods(t.join("install"), Lang::Hu, mods).unwrap();
        assert_eq!(text(&i, "dataraw/textures/colony/a_bmp.pvr"), "second a");
        assert_eq!(text(&i.with_lang(Lang::En).unwrap(), "dataraw/textures/colony/a_bmp.pvr"), "second a");
        let _ = std::fs::remove_dir_all(&t);
    }
}
