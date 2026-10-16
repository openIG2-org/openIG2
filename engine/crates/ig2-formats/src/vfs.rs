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
    /// Lowercase relative path with `/` separators → absolute path.
    index: HashMap<String, PathBuf>,
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").trim_start_matches('/').to_ascii_lowercase()
}

impl Install {
    pub fn open(root: impl Into<PathBuf>, lang: Lang) -> Result<Self> {
        let root = root.into();
        let mut index = HashMap::new();
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let entries = std::fs::read_dir(&dir)
                .map_err(|source| Error::Io { path: dir.display().to_string(), source })?;
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if let Ok(rel) = p.strip_prefix(&root) {
                    index.insert(normalize(&rel.to_string_lossy()), p);
                }
            }
        }
        if !index.contains_key("dataraw/text/scenario.txt") {
            return Err(Error::NotFound(format!(
                "{} doesn't look like an IG2 install (no dataraw/text/scenario.txt)",
                root.display()
            )));
        }
        Ok(Self { root, lang, index })
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
    pub fn resolve(&self, path: &str) -> Option<&Path> {
        let p = normalize(path);
        self.lang
            .dir()
            .and_then(|d| self.index.get(&format!("localization/{d}/{p}")))
            .or_else(|| self.index.get(&p))
            .map(PathBuf::as_path)
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

/// `foo.bmp`, then `foo.bmt`, then `foo.bmf` (`D3DTexture::Load`), with `.` → `_`, lowercased.
fn texture_candidates(name: &str) -> Vec<String> {
    let name = name.to_ascii_lowercase();
    let mut out = vec![name.replace('.', "_")];
    if let Some((i, _)) = name.char_indices().last() {
        out.extend(['t', 'f'].map(|c| format!("{}{c}", &name[..i]).replace('.', "_")));
    }
    out
}
