//! Read-only view of the user's IG2 install.
//!
//! Path resolution mirrors the remaster's `AssetHelper::rule_t` set up in `main` (ELF 0x442389):
//! a rule rewrites `""` → `"localization/<lang>/"`, then falls back to the unprefixed path.
//! The localization tree mirrors the root layout (`localization/hu/dataraw/text/...`).
//! Lookups are case-insensitive because the 1999 code uses names like `HADIGYAR.ani`.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::container::{self, DecryptingReader};
use crate::text::Lang;
use crate::textpatch;
use crate::{Error, Result};

/// The environment variable tests and tools use to find the install.
pub const INSTALL_ENV: &str = "IG2_INSTALL_DIR";

/// Where an install's bytes come from: the disk ([`Disk`]), or a store the host fills, such as a browser's copy of
/// the install folder (ADR 0047). Paths are the ones [`Files::scan`] returned.
pub trait Files: Send + Sync {
    /// Every file under `root` (links followed).
    fn scan(&self, root: &Path) -> io::Result<Vec<PathBuf>>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// A seekable reader, for streamed media.
    fn open(&self, path: &Path) -> io::Result<Box<dyn ReadSeek>>;
}

pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

/// The local file system.
pub struct Disk;

impl Files for Disk {
    fn scan(&self, root: &Path) -> io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)?.flatten() {
                let p = entry.path();
                // The entry's own type costs no extra file-system call (`is_dir` stats each file); a link is followed.
                if entry.file_type().is_ok_and(|t| if t.is_symlink() { p.is_dir() } else { t.is_dir() }) {
                    stack.push(p);
                } else {
                    files.push(p);
                }
            }
        }
        Ok(files)
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn open(&self, path: &Path) -> io::Result<Box<dyn ReadSeek>> {
        Ok(Box::new(BufReader::new(File::open(path)?)))
    }
}

#[derive(Clone)]
pub struct Install {
    files: Arc<dyn Files>,
    /// The files without the zip mods' (`files` wraps them when a zip mod is enabled), for [`Install::with_lang`].
    base: Arc<dyn Files>,
    root: PathBuf,
    lang: Lang,
    /// Lowercase relative path with `/` separators → absolute path (a mod's file over the install's).
    index: HashMap<String, PathBuf>,
    /// The enabled mods, highest priority first (ADR 0044).
    mods: Vec<Mod>,
    /// Paths a mod provides → (index in `mods`, whether the install has the path too).
    modded: HashMap<String, (usize, bool)>,
    /// Each mod's `rules.toml` (ADR 0044 §6), lowest priority first: (index in `mods`, file).
    patches: Vec<(usize, PathBuf)>,
    /// Each mod's `texts.<lang>.toml` (ADR 0044 §10), lowest priority first, English first within a mod.
    texts: Vec<(usize, Lang, PathBuf)>,
    /// Each mod's `stories.toml` (ADR 0061 §1), lowest priority first: (index in `mods`, file).
    stories: Vec<(usize, PathBuf)>,
    /// The text files the texts' language's patches change: game path → the patched decoded bytes.
    patched: HashMap<String, Vec<u8>>,
}

/// A mod's rule patch file at its folder's top (ADR 0044 §6); not a game path.
pub const RULES_PATCH: &str = "rules.toml";
/// A mod's story description file at its folder's top (ADR 0061 §1); not a game path.
pub const STORY_FILE: &str = "stories.toml";

/// A mod's description file at its top (ADR 0068 §3); not a game path.
pub const MOD_FILE: &str = "mod.toml";

/// An enabled mod: a folder mirroring the install's layout (ADR 0044), or a `.zip` of one (ADR 0068).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mod {
    pub name: String,
    /// The mod's folder, or its zip file.
    pub dir: PathBuf,
}

/// A mod's `mod.toml` (ADR 0068 §3, version 2: ADR 0086 §2): every key optional, a version-1 file is a valid one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModInfo {
    pub name: Option<String>,
    pub author: Option<String>,
    /// Semantic versioning by convention; any text is kept (old files), [`Version::parse`] reads its numbers.
    pub version: Option<String>,
    pub description: Option<String>,
    /// A PNG in the mod (its path there).
    pub picture: Option<String>,
    /// The openIG2 version it was made for (MAJOR.MINOR): a hint only, [`ModInfo::requires`] is the rule.
    pub game: Option<String>,
    /// The stable name other mods' `depends` and `conflicts` use: lower-case letters, digits, `-` and `_`.
    pub id: Option<String>,
    /// The openIG2 versions the mod loads on.
    pub requires: Option<VersionReq>,
    /// The mods it needs (id, their versions), in the file's order.
    pub depends: Vec<(String, VersionReq)>,
    /// The mods it can't load beside (ids).
    pub conflicts: Vec<String>,
    /// An SPDX licence expression.
    pub license: Option<String>,
    pub kind: ModKind,
}

/// What a mod changes (`kind` in `mod.toml`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ModKind {
    /// Anything (the default).
    #[default]
    Full,
    /// Looks only (`kind = "visual"`): its replaced and added files stay out of the multiplayer fingerprint, as a
    /// `hires/` pack's do; its `rules.toml`, `stories.toml` and text patches still count.
    Visual,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

impl ModInfo {
    pub fn parse(text: &str) -> std::result::Result<Self, String> {
        let doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| format!("{MOD_FILE}: {}", e.message()))?;
        let quoted = |k: &str| format!("{MOD_FILE}: {k} must be a text in quotes");
        let key = |k: &str| -> std::result::Result<Option<String>, String> {
            match doc.get(k) {
                None => Ok(None),
                Some(v) => v.as_str().map(|s| Some(s.trim().to_owned())).ok_or_else(|| quoted(k)),
            }
        };
        let range = |k: &str, s: &str| VersionReq::parse(s).map_err(|e| format!("{MOD_FILE}: {k}: {e}"));
        let id = key("id")?;
        if let Some(id) = id.as_deref().filter(|id| !valid_id(id)) {
            return Err(format!("{MOD_FILE}: id \"{id}\" may only have lower-case letters, digits, - and _"));
        }
        let requires = key("requires")?.map(|s| range("requires", &s)).transpose()?;
        let mut depends = Vec::new();
        if let Some(v) = doc.get("depends") {
            let t = v.as_table_like().ok_or_else(|| format!("{MOD_FILE}: depends must be a table of mod = \"versions\""))?;
            for (dep, v) in t.iter() {
                let s = v.as_str().ok_or_else(|| quoted(&format!("depends.{dep}")))?;
                depends.push((dep.to_owned(), range(&format!("depends.{dep}"), s)?));
            }
        }
        let conflicts = match doc.get("conflicts") {
            None => Vec::new(),
            Some(v) => {
                let a = v.as_array().ok_or_else(|| format!("{MOD_FILE}: conflicts must be a list of mod ids"))?;
                a.iter().map(|v| v.as_str().map(|s| s.trim().to_owned()).ok_or_else(|| quoted("conflicts"))).collect::<std::result::Result<_, _>>()?
            }
        };
        let kind = match key("kind")?.as_deref() {
            None | Some("full") => ModKind::Full,
            Some("visual") => ModKind::Visual,
            Some(k) => return Err(format!("{MOD_FILE}: kind \"{k}\" isn't \"visual\" or \"full\"")),
        };
        Ok(Self {
            name: key("name")?,
            author: key("author")?,
            version: key("version")?,
            description: key("description")?,
            picture: key("picture")?,
            game: key("game")?,
            id,
            requires,
            depends,
            conflicts,
            license: key("license")?,
            kind,
        })
    }

    /// The mod's id: `id`, else the folder's or zip's name in lower case.
    pub fn id_or(&self, folder: &str) -> String {
        self.id.clone().unwrap_or_else(|| folder.to_lowercase())
    }
}

/// A version's numbers (MAJOR.MINOR.PATCH, missing ones 0); a pre-release or build suffix (`-dev`, `+x`) is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub [u64; 3]);

impl Version {
    /// `1`, `1.2`, `0.7.12`, `0.7-dev`; `None` for anything else.
    pub fn parse(s: &str) -> Option<Self> {
        Self::parts(s).map(|(v, _)| v)
    }

    /// The version and how many numbers it gave.
    fn parts(s: &str) -> Option<(Self, usize)> {
        let s = s.trim().trim_start_matches('v');
        let core = s.split(['-', '+']).next()?;
        let mut v = [0; 3];
        let mut n = 0;
        for p in core.split('.') {
            *v.get_mut(n)? = p.parse().ok()?;
            n += 1;
        }
        Some((Self(v), n))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Eq,
    Gt,
    Ge,
    Lt,
    Le,
    Caret,
    Tilde,
}

/// A version range (ADR 0086 §2), Cargo's rules: comparators joined by commas, all of which must hold, each `>=`, `>`,
/// `<=`, `<`, `=`, `^`, `~` or none (as `^`) before a version of one to three numbers; `*` or nothing is any version.
/// A partial version compares on its numbers only: `<=1.2` takes 1.2.9, `=0.7` every 0.7.N.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionReq {
    text: String,
    parts: Vec<(Op, Version, usize)>,
}

impl VersionReq {
    pub fn parse(s: &str) -> std::result::Result<Self, String> {
        let text = s.trim().to_owned();
        let mut parts = Vec::new();
        if text != "*" && !text.is_empty() {
            for c in text.split(',') {
                let c = c.trim();
                let (op, rest) = [(">=", Op::Ge), ("<=", Op::Le), (">", Op::Gt), ("<", Op::Lt), ("=", Op::Eq), ("^", Op::Caret), ("~", Op::Tilde)]
                    .into_iter()
                    .find_map(|(p, op)| c.strip_prefix(p).map(|r| (op, r)))
                    .unwrap_or((Op::Caret, c));
                let (v, n) = Version::parts(rest).filter(|&(_, n)| n > 0).ok_or_else(|| format!("\"{c}\" isn't a version like 1.2 or >=0.7"))?;
                parts.push((op, v, n));
            }
        }
        Ok(Self { text, parts })
    }

    pub fn matches(&self, v: Version) -> bool {
        self.parts.iter().all(|&(op, want, n)| {
            let cmp = v.0[..n].cmp(&want.0[..n]);
            match op {
                Op::Eq => cmp.is_eq(),
                Op::Gt => cmp.is_gt(),
                Op::Ge => cmp.is_ge(),
                Op::Lt => cmp.is_lt(),
                Op::Le => cmp.is_le(),
                // ^: the numbers up to the first non-zero one stay; ~: MAJOR.MINOR (MAJOR alone if only it's given).
                Op::Caret => {
                    let k = want.0[..n].iter().position(|&x| x != 0).unwrap_or(n - 1);
                    v >= want && v.0[..=k] == want.0[..=k]
                }
                Op::Tilde => v >= want && v.0[..n.min(2)] == want.0[..n.min(2)],
            }
        })
    }

    /// Whether the version text `v` is in the range (a text [`Version::parse`] can't read isn't).
    pub fn matches_text(&self, v: &str) -> bool {
        Version::parse(v).is_some_and(|v| self.matches(v))
    }

    /// Any version.
    pub fn is_any(&self) -> bool {
        self.parts.is_empty()
    }
}

impl std::fmt::Display for VersionReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.is_any() { "*" } else { &self.text })
    }
}

/// Why a mod can't load beside the others (ADR 0086 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Its `requires` excludes the running openIG2.
    Version(VersionReq),
    /// A mod it depends on isn't enabled (the id, the versions it needs).
    Missing(String, VersionReq),
    /// A mod it depends on is enabled in a version outside the range (the id, the range, that mod's version).
    DependencyVersion(String, VersionReq, Option<String>),
    /// It and an enabled mod (that id) exclude each other.
    Conflict(String),
    /// An enabled mod above it has the same id.
    Duplicate(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Version(r) => write!(f, "needs openIG2 {r}"),
            Self::Missing(id, r) if r.is_any() => write!(f, "needs the mod {id}"),
            Self::Missing(id, r) => write!(f, "needs the mod {id} {r}"),
            Self::DependencyVersion(id, r, v) => write!(f, "needs the mod {id} {r}, not {}", v.as_deref().unwrap_or("an unversioned one")),
            Self::Conflict(id) => write!(f, "can't load beside {id}"),
            Self::Duplicate(id) => write!(f, "another enabled mod is {id} too"),
        }
    }
}

/// Why each of `enabled` (id, `mod.toml`; highest priority first) can't load beside the others on openIG2 `version`;
/// `None` loads. Only enabled mods count: a refused one is still another's dependency or conflict.
pub fn refusals(enabled: &[(String, &ModInfo)], version: &str) -> Vec<Option<Refusal>> {
    let running = Version::parse(version);
    enabled
        .iter()
        .enumerate()
        .map(|(i, (id, info))| {
            if let Some(r) = info.requires.as_ref().filter(|r| !running.is_some_and(|v| r.matches(v))) {
                return Some(Refusal::Version(r.clone()));
            }
            if enabled[..i].iter().any(|(o, _)| o == id) {
                return Some(Refusal::Duplicate(id.clone()));
            }
            for (dep, range) in &info.depends {
                match enabled.iter().find(|(o, _)| o == dep) {
                    None => return Some(Refusal::Missing(dep.clone(), range.clone())),
                    Some((_, d)) if !range.is_any() && !d.version.as_deref().is_some_and(|v| range.matches_text(v)) => {
                        return Some(Refusal::DependencyVersion(dep.clone(), range.clone(), d.version.clone()));
                    }
                    Some(_) => {}
                }
            }
            let other = enabled.iter().enumerate().find(|&(j, (o, oi))| j != i && o != id && (info.conflicts.contains(o) || oi.conflicts.contains(id)));
            other.map(|(_, (o, _))| Refusal::Conflict(o.clone()))
        })
        .collect()
}

/// `mods` (id, `mod.toml`; highest priority first) reordered so each mod sits above the mods it depends on, the
/// order kept otherwise (ADR 0086 §2): the indices, highest priority first. A cycle keeps the cycle's given order.
pub fn dependency_order(mods: &[(String, &ModInfo)]) -> Vec<usize> {
    let mut left: Vec<usize> = (0..mods.len()).collect();
    let mut out = Vec::with_capacity(mods.len());
    while !left.is_empty() {
        // The first mod no remaining one needs; the first one in a cycle.
        let needed = |&i: &usize| left.iter().any(|&j| j != i && mods[j].1.depends.iter().any(|(d, _)| *d == mods[i].0));
        let at = left.iter().position(|i| !needed(i)).unwrap_or(0);
        out.push(left.remove(at));
    }
    out
}

impl Mod {
    /// The mods folder beside the settings files (`mods/` next to [`Options::path`](crate::options::Options::path)):
    /// where `enabled.txt` is.
    pub fn folder() -> Option<PathBuf> {
        crate::options::Options::path().and_then(|p| p.parent().map(|d| d.join("mods")))
    }

    /// The mods folder beside the game's executable (ADR 0068 §1).
    pub fn exe_folder() -> Option<PathBuf> {
        if cfg!(target_arch = "wasm32") {
            return None;
        }
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("mods")))
    }

    /// The folders searched for mods, the first one winning a name: beside the executable, then the settings'.
    pub fn folders() -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = Self::exe_folder().into_iter().chain(Self::folder()).collect();
        v.dedup();
        v
    }

    /// Every mod in `folders` (each a folder or a `.zip`), by name; a name the first folder has wins, a folder over a
    /// zip of the same name (ADR 0068 §1).
    pub fn available_in(folders: &[PathBuf]) -> Vec<Mod> {
        let mut found: std::collections::BTreeMap<String, Mod> = std::collections::BTreeMap::new();
        for folder in folders {
            let Ok(entries) = std::fs::read_dir(folder) else { continue };
            let mut here: Vec<Mod> = entries
                .flatten()
                .filter_map(|e| {
                    let (p, name) = (e.path(), e.file_name().into_string().ok()?);
                    if p.is_dir() {
                        return Some(Mod { name, dir: p });
                    }
                    let stem = name.len().checked_sub(4).filter(|&i| name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(".zip"))?;
                    Some(Mod { name: name[..stem].to_owned(), dir: p })
                })
                .collect();
            here.sort_by_key(|m| !m.dir.is_dir());
            for m in here {
                found.entry(m.name.clone()).or_insert(m);
            }
        }
        found.into_values().collect()
    }

    /// [`Mod::available_in`] the [`Mod::folders`].
    pub fn available() -> Vec<Mod> {
        Self::available_in(&Self::folders())
    }

    pub fn is_zip(&self) -> bool {
        self.dir.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) && !self.dir.is_dir()
    }

    /// Whether the mod can be loaded: a zip's directory reads and follows the rules (ADR 0068 §2).
    pub fn check(&self) -> std::result::Result<(), String> {
        self.zip().map(|_| ())
    }

    fn zip(&self) -> std::result::Result<Option<crate::zipfs::Zip>, String> {
        if !self.is_zip() {
            return Ok(None);
        }
        let mut f = BufReader::new(File::open(&self.dir).map_err(|e| e.to_string())?);
        crate::zipfs::Zip::open(&mut f).map(Some)
    }

    /// The file `rel` (a path inside the mod, `/`-separated, any case) from the disk.
    pub fn read(&self, rel: &str) -> std::result::Result<Vec<u8>, String> {
        match self.zip()? {
            Some(z) => {
                let name = z.names().find(|n| n.eq_ignore_ascii_case(rel)).ok_or_else(|| format!("{rel}: not in the mod"))?.to_owned();
                z.read(&mut BufReader::new(File::open(&self.dir).map_err(|e| e.to_string())?), &name)
            }
            None => std::fs::read(self.dir.join(rel)).map_err(|e| format!("{rel}: {e}")),
        }
    }

    /// The mod's `mod.toml`; the default without one.
    pub fn info(&self) -> std::result::Result<ModInfo, String> {
        self.check()?;
        match self.read(MOD_FILE) {
            Ok(b) => ModInfo::parse(&String::from_utf8(b).map_err(|_| format!("{MOD_FILE}: not UTF-8 text"))?),
            Err(_) => Ok(ModInfo::default()),
        }
    }

    /// `mods/enabled.txt`'s names in order: one a line, `#` comments and blank lines skipped.
    pub fn parse_enabled(text: &str) -> Vec<String> {
        text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()).map(str::to_owned).collect()
    }

    /// `mods/enabled.txt` in the settings' mods folder (ADR 0044 §2).
    pub fn enabled_file() -> Option<PathBuf> {
        Self::folder().map(|f| f.join("enabled.txt"))
    }

    /// `enabled.txt`'s names (none without the file).
    pub fn enabled_names() -> Vec<String> {
        Self::enabled_file().and_then(|f| crate::userfs::read_to_string(f).ok()).map(|t| Self::parse_enabled(&t)).unwrap_or_default()
    }

    /// `enabled.txt` for `names` in priority order (ADR 0068 §4).
    pub fn enabled_text(names: &[String]) -> String {
        let mut t = String::from("# The enabled mods, the first one winning a file. Written by the game's Mods page.\n");
        for n in names {
            t.push_str(n);
            t.push('\n');
        }
        t
    }

    /// The mods to load (ADR 0044 §2): `given` (`--mod`), else `mods/enabled.txt`'s names, found in [`Mod::folders`];
    /// the names that can't be loaded are the second list, each with why.
    pub fn enabled(given: Option<Vec<String>>) -> (Vec<Mod>, Vec<(String, String)>) {
        let names = given.unwrap_or_else(Self::enabled_names);
        Self::pick(&Self::available(), &names)
    }

    /// The named mods of `available`, in the given order; the names missing or broken are the second list, with why.
    pub fn pick(available: &[Mod], names: &[String]) -> (Vec<Mod>, Vec<(String, String)>) {
        let (mut found, mut bad) = (Vec::new(), Vec::new());
        for n in names {
            match available.iter().find(|m| &m.name == n) {
                None => bad.push((n.clone(), "no such folder or zip in the mods folders".to_owned())),
                Some(m) => match m.check() {
                    Ok(()) => found.push(m.clone()),
                    Err(e) => bad.push((n.clone(), e)),
                },
            }
        }
        (found, bad)
    }

    /// The named mods in `folder`, in the given order; the names missing or broken are the second list.
    pub fn find(folder: &Path, names: &[String]) -> (Vec<Mod>, Vec<String>) {
        let (found, bad) = Self::pick(&Self::available_in(&[folder.to_path_buf()]), names);
        (found, bad.into_iter().map(|(n, _)| n).collect())
    }
}

/// The install's files with the zip mods' files beside them (ADR 0068 §2): a path under a zip's path is that zip's file.
struct WithZips {
    base: Arc<dyn Files>,
    zips: Vec<(PathBuf, crate::zipfs::Zip)>,
}

impl WithZips {
    fn zip_of<'a>(&'a self, path: &Path) -> Option<(&'a Path, &'a crate::zipfs::Zip, String)> {
        self.zips.iter().find_map(|(z, zip)| {
            let rel = path.strip_prefix(z).ok()?.to_string_lossy().replace('\\', "/");
            Some((z.as_path(), zip, rel))
        })
    }
}

impl Files for WithZips {
    fn scan(&self, root: &Path) -> io::Result<Vec<PathBuf>> {
        match self.zips.iter().find(|(z, _)| z == root) {
            Some((z, zip)) => Ok(zip.names().map(|n| z.join(n)).collect()),
            None => self.base.scan(root),
        }
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        match self.zip_of(path) {
            Some((z, zip, rel)) => zip.read(&mut self.base.open(z)?, &rel).map_err(io::Error::other),
            None => self.base.read(path),
        }
    }

    fn open(&self, path: &Path) -> io::Result<Box<dyn ReadSeek>> {
        match self.zip_of(path) {
            Some(_) => Ok(Box::new(io::Cursor::new(self.read(path)?))),
            None => self.base.open(path),
        }
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
        Self::open_from(Arc::new(Disk), root, lang, mods)
    }

    /// [`Install::open_with_mods`] with the bytes from `files`.
    pub fn open_from(files: Arc<dyn Files>, root: impl Into<PathBuf>, lang: Lang, mods: Vec<Mod>) -> Result<Self> {
        let root = root.into();
        let base = files;
        let mut zips = Vec::new();
        for m in mods.iter().filter(|m| m.is_zip()) {
            let mut r = base.open(&m.dir).map_err(|source| Error::Io { path: m.dir.display().to_string(), source })?;
            let zip = crate::zipfs::Zip::open(&mut r).map_err(|msg| Error::Format { format: "mod zip", msg: format!("mod {}: {msg}", m.name) })?;
            zips.push((m.dir.clone(), zip));
        }
        let files: Arc<dyn Files> = if zips.is_empty() { base.clone() } else { Arc::new(WithZips { base: base.clone(), zips }) };
        let mut index = scan(&*files, &root)?;
        if !index.contains_key("dataraw/text/scenario.txt") {
            return Err(Error::NotFound(format!(
                "{} doesn't look like an IG2 install (no dataraw/text/scenario.txt)",
                root.display()
            )));
        }
        let (mut modded, mut patches, mut texts, mut stories) = (HashMap::new(), Vec::new(), Vec::new(), Vec::new());
        for (i, m) in mods.iter().enumerate().rev() {
            let mut found = scan(&*files, &m.dir)?;
            // The description and its picture aren't game files (ADR 0068 §3).
            if let Some(p) = found.remove(MOD_FILE) {
                let info = files.read(&p).ok().and_then(|b| ModInfo::parse(&String::from_utf8_lossy(&b)).ok()).unwrap_or_default();
                if let Some(pic) = info.picture {
                    found.remove(&normalize(&pic));
                }
            }
            patches.extend(found.remove(RULES_PATCH).map(|p| (i, p)));
            stories.extend(found.remove(STORY_FILE).map(|p| (i, p)));
            texts.extend(Lang::ALL.into_iter().filter_map(|l| found.remove(&textpatch::file_name(l)).map(|p| (i, l, p))));
            // The rule tables are read from the exe (ADR 0004); a mod never replaces it, `rules.toml` patches them.
            for (k, p) in found.into_iter().filter(|(k, _)| !k.ends_with(".exe")) {
                let replaces = modded.get(&k).map_or_else(|| index.contains_key(&k), |&(_, r)| r);
                modded.insert(k.clone(), (i, replaces));
                index.insert(k, p);
            }
        }
        let mut install = Self { files, base, root, lang, index, mods, modded, patches, texts, stories, patched: HashMap::new() };
        install.patched = install.patch_texts()?;
        Ok(install)
    }

    /// Apply the texts' language's patches, falling back to English (ADR 0044 §11), to the files they name.
    fn patch_texts(&self) -> Result<HashMap<String, Vec<u8>>> {
        let err = |msg| Error::Format { format: "mod text patch", msg };
        let mut patch = textpatch::TextPatch::default();
        for (i, l, p) in self.texts.iter().filter(|t| t.1 == Lang::En || t.1 == self.lang) {
            let bytes = self.files.read(p).map_err(|source| Error::Io { path: p.display().to_string(), source })?;
            let from = format!("{} of mod {}", textpatch::file_name(*l), self.mods[*i].name);
            let toml = String::from_utf8(bytes).map_err(|_| err(format!("{from}: not UTF-8 text")))?;
            patch.add(&toml, &from, self.lang).map_err(err)?;
        }
        patch
            .paths()
            .map(|path| {
                let raw = self.read(path).map_err(|_| err(format!("{path}: no such file in the install or the mods")))?;
                let bom = raw.starts_with(b"\xEF\xBB\xBF");
                let text = patch.apply(path, &crate::text::decode(&raw, self.lang)).map_err(err)?;
                let bytes = if bom { [&b"\xEF\xBB\xBF"[..], text.as_bytes()].concat() } else { crate::text::encode(&text, self.lang) };
                Ok((path.to_owned(), bytes))
            })
            .collect()
    }

    /// The same install and mods in another texts' language.
    pub fn with_lang(&self, lang: Lang) -> Result<Self> {
        Self::open_from(self.base.clone(), &self.root, lang, self.mods.clone())
    }

    /// The enabled mods, highest priority first.
    pub fn mods(&self) -> &[Mod] {
        &self.mods
    }

    /// The enabled mods' `rules.toml` files in the order they apply (lowest priority first, ADR 0044 §7): (mod, bytes).
    pub fn rule_patches(&self) -> Result<Vec<(&str, Vec<u8>)>> {
        self.mod_files(&self.patches)
    }

    /// The enabled mods' `stories.toml` files in the order they run (lowest priority first, ADR 0061 §5): (mod, bytes).
    pub fn story_files(&self) -> Result<Vec<(&str, Vec<u8>)>> {
        self.mod_files(&self.stories)
    }

    fn mod_files(&self, files: &[(usize, PathBuf)]) -> Result<Vec<(&str, Vec<u8>)>> {
        files
            .iter()
            .map(|(i, p)| {
                let bytes = self.files.read(p).map_err(|source| Error::Io { path: p.display().to_string(), source })?;
                Ok((self.mods[*i].name.as_str(), bytes))
            })
            .collect()
    }

    /// Every enabled mod's text patch files in the order they apply (lowest priority first, ADR 0044 §11), all
    /// languages: (mod, file name, bytes).
    pub fn text_patches(&self) -> Result<Vec<(&str, String, Vec<u8>)>> {
        self.texts
            .iter()
            .map(|(i, l, p)| {
                let bytes = self.files.read(p).map_err(|source| Error::Io { path: p.display().to_string(), source })?;
                Ok((self.mods[*i].name.as_str(), textpatch::file_name(*l), bytes))
            })
            .collect()
    }

    /// The bytes of a file the index holds ([`Override::file`], a zip mod's too), as stored.
    pub fn read_file(&self, file: &Path) -> Result<Vec<u8>> {
        self.files.read(file).map_err(|source| Error::Io { path: file.display().to_string(), source })
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
        let raw = self.files.read(p).map_err(|source| Error::Io { path: p.display().to_string(), source })?;
        // other stores' builds are read through the GOG layout (crate::exe_builds)
        Ok(if normalize(path) == "ig2.exe" { crate::exe_builds::to_gog_layout(&raw).unwrap_or(raw) } else { raw })
    }

    /// Fully decoded file contents, with the mods' text patches (ADR 0044 §10) applied.
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        if let Some(b) = self.patched.get(&normalize(path)) {
            return Ok(b.clone());
        }
        let raw = self.read_raw(path)?;
        Ok(container::decode(path, &raw)?.into_owned())
    }

    /// Seekable decrypted stream, for `neGs` media (Ogg, MP4).
    pub fn open_stream(&self, path: &str) -> Result<DecryptingReader<Box<dyn ReadSeek>>> {
        let p = self.resolve(path).ok_or_else(|| Error::NotFound(path.to_owned()))?;
        let io = |source| Error::Io { path: p.display().to_string(), source };
        DecryptingReader::new(path, self.files.open(p).map_err(io)?).map_err(io)
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

/// Steam's own bookkeeping files in the game's folders (`re/formats/exe-builds.md`): not game data.
const STORE_FILES: [&str; 3] = [".list.dir", ".filedb.json", ".sourcefiledb.json"];

/// Every file under `root` but the store's own: lowercase `/`-separated relative path → absolute path.
fn scan(files: &dyn Files, root: &Path) -> Result<HashMap<String, PathBuf>> {
    let found = files.scan(root).map_err(|source| Error::Io { path: root.display().to_string(), source })?;
    Ok(found
        .into_iter()
        .filter(|p| !p.file_name().is_some_and(|n| STORE_FILES.iter().any(|s| n.eq_ignore_ascii_case(s))))
        .filter_map(|p| Some((normalize(&p.strip_prefix(root).ok()?.to_string_lossy()), p)))
        .collect())
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
            ("mods/first/Rules.toml", "first patch"),
            ("mods/second/rules.toml", "second patch"),
            ("mods/second/Stories.toml", "second stories"),
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
        // Patches aren't game files; they apply lowest priority first.
        assert!(i.resolve("rules.toml").is_none());
        let p: Vec<(&str, String)> = i.rule_patches().unwrap().into_iter().map(|(m, b)| (m, String::from_utf8(b).unwrap())).collect();
        assert_eq!(p, [("second", "second patch".into()), ("first", "first patch".into())]);
        assert!(i.resolve("stories.toml").is_none());
        assert_eq!(i.story_files().unwrap(), [("second", b"second stories".to_vec())]);
        // The other order: the second mod's copy wins.
        let (mods, _) = Mod::find(&t.join("mods"), &["second".into(), "first".into()]);
        let i = Install::open_with_mods(t.join("install"), Lang::En, mods).unwrap();
        assert_eq!(text(&i, "dataraw/textures/colony/a_bmp.pvr"), "second a");
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn zip_mods_beside_folders_by_priority() {
        let t = tree("zip");
        let (exe, settings) = (t.join("exe-mods"), t.join("mods"));
        std::fs::create_dir_all(&exe).unwrap();
        let toml = "name = \"Zipped\"\nauthor = \"Someone\"\nversion = \"1.2\"\npicture = \"Shot.png\"\ngame = \"0.7\"\n";
        let z = crate::zipfs::build(&[
            ("zipped/mod.toml", toml.as_bytes(), true),
            ("zipped/shot.png", b"png", false),
            ("zipped/DataRaw/Textures/Colony/B_BMP.pvr", b"zip b", true),
            ("zipped/dataraw/textures/colony/zip_bmp.pvr", b"zip new", false),
            ("zipped/rules.toml", b"zip patch", false),
        ]);
        std::fs::write(exe.join("zipped.zip"), &z).unwrap();
        // A broken download, and a zip that names a file outside it.
        std::fs::write(exe.join("broken.zip"), &z[..z.len() / 2]).unwrap();
        std::fs::write(exe.join("evil.zip"), crate::zipfs::build(&[("../../evil.dll", b"x", false)])).unwrap();
        // The same name in both folders: the one beside the executable wins.
        std::fs::write(settings.join("second.zip"), crate::zipfs::build(&[("x.txt", b"x", false)])).unwrap();
        let all = Mod::available_in(&[exe.clone(), settings.clone()]);
        let names: Vec<(&str, bool)> = all.iter().map(|m| (m.name.as_str(), m.is_zip())).collect();
        assert_eq!(names, [("broken", true), ("evil", true), ("first", false), ("second", false), ("zipped", true)]);
        let want: Vec<String> = ["zipped", "broken", "second", "evil", "gone"].map(String::from).into();
        let (mods, bad) = Mod::pick(&all, &want);
        assert_eq!(mods.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["zipped", "second"]);
        let bad: Vec<&str> = bad.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(bad, ["broken", "evil", "gone"]);
        assert!(Mod::pick(&all, &["evil".into()]).1[0].1.contains("unsafe file name"));
        let info = mods[0].info().unwrap();
        assert_eq!((info.name.as_deref(), info.version.as_deref(), info.picture.as_deref()), (Some("Zipped"), Some("1.2"), Some("Shot.png")));
        assert_eq!(mods[0].read("SHOT.PNG").unwrap(), b"png");
        let i = Install::open_with_mods(t.join("install"), Lang::Hu, mods.clone()).unwrap();
        // The zip is first: its copy wins; a file only the folder mod has still comes from it.
        assert_eq!(text(&i, "dataraw/textures/colony/b_bmp.pvr"), "zip b");
        assert_eq!(text(&i, "dataraw/textures/colony/a_bmp.pvr"), "second a");
        assert_eq!(text(&i.with_lang(Lang::En).unwrap(), "dataraw/textures/colony/zip_bmp.pvr"), "zip new");
        // `mod.toml` and its picture aren't game files; the zip's rules patch is one.
        assert!(i.resolve("mod.toml").is_none() && i.resolve("shot.png").is_none());
        assert_eq!(i.rule_patches().unwrap(), [("second", b"second patch".to_vec()), ("zipped", b"zip patch".to_vec())]);
        let o = i.overrides().into_iter().find(|o| o.path.ends_with("zip_bmp.pvr")).unwrap();
        assert_eq!((o.mod_name, i.read_file(o.file).unwrap()), ("zipped", b"zip new".to_vec()));
        // Lower priority: the folder's copy wins.
        let i = Install::open_with_mods(t.join("install"), Lang::En, mods.into_iter().rev().collect()).unwrap();
        assert_eq!(text(&i, "dataraw/textures/colony/b_bmp.pvr"), "second b");
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn mod_toml_and_enabled_text() {
        let i = ModInfo::parse("name = \"A\"\ndescription = \"Two\\nlines\"\n").unwrap();
        assert_eq!((i.name.as_deref(), i.description.as_deref(), i.author), (Some("A"), Some("Two\nlines"), None));
        assert!(ModInfo::parse("version = 2").unwrap_err().contains("version"));
        assert!(ModInfo::parse("name = ").is_err());
        let names: Vec<String> = vec!["b".into(), "a".into()];
        assert_eq!(Mod::parse_enabled(&Mod::enabled_text(&names)), names);
    }

    #[test]
    fn mod_toml_2_fields() {
        // A version-1 file: the new fields at their defaults, the folder's name as the id.
        let old = ModInfo::parse("name = \"Old\"\nversion = \"1.2\"\ngame = \"0.7\"\n").unwrap();
        assert_eq!((old.id_or("Old-Mod"), old.requires.is_none(), old.depends.len(), old.kind), ("old-mod".to_string(), true, 0, ModKind::Full));
        let t = "id = \"zorg_race-2\"\nversion = \"1.4.0\"\nrequires = \">=0.7, <0.9\"\nlicense = \"CC-BY-4.0\"\nkind = \"visual\"\n\
                 conflicts = [\"other\"]\n[depends]\nbase = \">=1.2\"\nany = \"*\"\n";
        let i = ModInfo::parse(t).unwrap();
        assert_eq!((i.id_or("x"), i.license.as_deref(), i.kind, i.conflicts.as_slice()), ("zorg_race-2".to_string(), Some("CC-BY-4.0"), ModKind::Visual, ["other".to_string()].as_slice()));
        assert_eq!(i.depends.iter().map(|(d, r)| format!("{d} {r}")).collect::<Vec<_>>(), ["base >=1.2", "any *"]);
        assert_eq!(i.requires.unwrap().to_string(), ">=0.7, <0.9");
        // The inline table works as well; wrong shapes say which key.
        assert_eq!(ModInfo::parse("depends = { \"a\" = \"1\" }").unwrap().depends[0].0, "a");
        for (bad, key) in [("id = \"Big\"", "id"), ("requires = \"soon\"", "requires"), ("depends = [\"a\"]", "depends"), ("depends = { a = 1 }", "depends.a"), ("conflicts = \"a\"", "conflicts"), ("kind = \"sound\"", "kind")] {
            assert!(ModInfo::parse(bad).unwrap_err().contains(key), "{bad}");
        }
    }

    #[test]
    fn version_ranges() {
        let r = |s: &str| VersionReq::parse(s).unwrap();
        let yes = |s: &str, v: &str| r(s).matches_text(v);
        assert_eq!(Version::parse("0.7-dev"), Some(Version([0, 7, 0])));
        assert_eq!((Version::parse("1.2.3.4"), Version::parse("x"), Version::parse("")), (None, None, None));
        assert!(yes(">=0.7, <0.9", "0.7.12") && yes(">=0.7, <0.9", "0.8") && !yes(">=0.7, <0.9", "0.9.0") && !yes(">=0.7, <0.9", "0.6.99"));
        // Partial versions compare on their numbers.
        assert!(yes("<=1.2", "1.2.9") && !yes(">1.2", "1.2.9") && yes(">1.2", "1.3") && yes("=0.7", "0.7.40") && !yes("=0.7", "0.8"));
        // Caret (also a bare version) and tilde, as Cargo.
        assert!(yes("1.2", "1.9.0") && !yes("1.2", "2.0") && !yes("1.2", "1.1"));
        assert!(yes("^0.7", "0.7.3") && !yes("^0.7", "0.8") && yes("^0.0.3", "0.0.3") && !yes("^0.0.3", "0.0.4"));
        assert!(yes("~1.2.3", "1.2.9") && !yes("~1.2.3", "1.3") && yes("~1", "1.9"));
        assert!(r("*").is_any() && r("").is_any() && yes("*", "0.1") && !yes("*", "beta"));
        assert!(VersionReq::parse(">=").is_err() && VersionReq::parse("1.2, x").is_err());
    }

    #[test]
    fn refusals_and_dependency_order() {
        let info = |t: &str| ModInfo::parse(t).unwrap();
        let (base, needs, newer, old_game, foe, plain) = (
            info("version = \"1.3\""),
            info("[depends]\nbase = \">=1.2\""),
            info("[depends]\nbase = \">=2\""),
            info("requires = \"<0.7\""),
            info("conflicts = [\"plain\"]"),
            info(""),
        );
        fn set<'a>(v: &[(&str, &'a ModInfo)]) -> Vec<(String, &'a ModInfo)> {
            v.iter().map(|&(i, m)| (i.to_string(), m)).collect()
        }
        let why = |v: &[(&str, &ModInfo)]| refusals(&set(v), "0.7.5").into_iter().map(|r| r.map(|r| r.to_string())).collect::<Vec<_>>();
        assert_eq!(why(&[("needs", &needs), ("base", &base), ("plain", &plain)]), [None, None, None]);
        assert_eq!(why(&[("needs", &needs)]), [Some("needs the mod base >=1.2".to_string())]);
        assert_eq!(why(&[("newer", &newer), ("base", &base)])[0].as_deref(), Some("needs the mod base >=2, not 1.3"));
        assert_eq!(why(&[("old", &old_game)])[0].as_deref(), Some("needs openIG2 <0.7"));
        // A conflict refuses both sides; the same id twice refuses the lower one.
        assert_eq!(why(&[("plain", &plain), ("foe", &foe)]), [Some("can't load beside foe".to_string()), Some("can't load beside plain".to_string())]);
        assert_eq!(why(&[("base", &base), ("base", &plain)])[1].as_deref(), Some("another enabled mod is base too"));
        // Dependencies go under the mods that need them, the rest keeps its order.
        let all = set(&[("base", &base), ("plain", &plain), ("needs", &needs), ("foe", &foe)]);
        assert_eq!(dependency_order(&all), [1, 2, 0, 3]);
        let cycle = (info("[depends]\nb = \"*\""), info("[depends]\na = \"*\""));
        assert_eq!(dependency_order(&set(&[("a", &cycle.0), ("b", &cycle.1)])), [0, 1]);
    }

    /// Files a host hands over in memory (how a browser feeds its copy of the install).
    struct Memory(HashMap<PathBuf, Vec<u8>>);

    impl Files for Memory {
        fn scan(&self, root: &Path) -> io::Result<Vec<PathBuf>> {
            Ok(self.0.keys().filter(|p| p.starts_with(root)).cloned().collect())
        }

        fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
            self.0.get(path).cloned().ok_or_else(|| io::ErrorKind::NotFound.into())
        }

        fn open(&self, path: &Path) -> io::Result<Box<dyn ReadSeek>> {
            Ok(Box::new(io::Cursor::new(self.read(path)?)))
        }
    }

    #[test]
    fn an_install_from_memory() {
        let files = [("game/DataRaw/Text/Scenario.txt", "base"), ("game/localization/hu/dataraw/text/scenario.txt", "hu")];
        let files = Memory(files.into_iter().map(|(p, b)| (PathBuf::from(p), b.as_bytes().to_vec())).collect());
        let i = Install::open_from(Arc::new(files), "game", Lang::En, Vec::new()).unwrap();
        assert_eq!(i.all_files(), ["dataraw/text/scenario.txt", "localization/hu/dataraw/text/scenario.txt"]);
        assert_eq!(text(&i, "DATARAW/TEXT/SCENARIO.TXT"), "base");
        assert_eq!(text(&i.with_lang(Lang::Hu).unwrap(), "dataraw/text/scenario.txt"), "hu");
        let mut s = String::new();
        i.open_stream("dataraw/text/scenario.txt").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "base");
    }

    #[test]
    fn text_patches_by_language_with_english_fallback() {
        let files = [
            ("game/dataraw/text/scenario.txt", "base"),
            ("game/dataraw/text/s_races.txt", "1 Alpha\r\n2 Beta\r\n"),
            ("game/localization/hu/dataraw/text/s_races.txt", "1 Alfa\r\n2 Béta\r\n"),
            ("mods/new/texts.en.toml", "[\"dataraw/text/s_races.txt\"]\n2 = \"Blorbs\"\n3 = \"Zorgons\"\n"),
            ("mods/new/texts.hu.toml", "[\"dataraw/text/s_races.txt\"]\n3 = \"Zorgonok ő\"\n"),
            ("mods/top/texts.en.toml", "[\"dataraw/text/s_races.txt\"]\n2 = \"Top\"\n"),
            ("mods/bad/texts.en.toml", "[\"dataraw/text/s_story.txt\"]\n1 = \"x\"\n"),
        ];
        // The game's files in its code page, the patches in UTF-8.
        let bytes = |p: &str, b: &str| if p.ends_with(".toml") { b.as_bytes().to_vec() } else { encoding_rs::WINDOWS_1250.encode(b).0.into_owned() };
        let files: Arc<dyn Files> = Arc::new(Memory(files.into_iter().map(|(p, b)| (PathBuf::from(p), bytes(p, b))).collect()));
        let m = |n: &str| Mod { name: n.into(), dir: PathBuf::from("mods").join(n) };
        let races = |i: &Install| crate::text::decode(&i.read("dataraw/text/s_races.txt").unwrap(), i.lang());
        let i = Install::open_from(files.clone(), "game", Lang::Hu, vec![m("top"), m("new")]).unwrap();
        // Hungarian: the mod's Hungarian line, its English one where it has none, the higher mod's over both.
        assert_eq!(races(&i), "1 Alfa\r\n2|\"Top\"\r\n3|\"Zorgonok ő\"\r\n");
        assert_eq!(races(&i.with_lang(Lang::En).unwrap()), "1 Alpha\r\n2|\"Top\"\r\n3|\"Zorgons\"\r\n");
        // Not game files; every language's file is listed in the order they apply.
        assert!(i.resolve("texts.en.toml").is_none() && i.overrides().is_empty());
        let t: Vec<(&str, String)> = i.text_patches().unwrap().into_iter().map(|(m, f, _)| (m, f)).collect();
        assert_eq!(t, [("new", "texts.en.toml".into()), ("new", "texts.hu.toml".into()), ("top", "texts.en.toml".into())]);
        // A patch for a file that isn't there fails the load, naming the file.
        let e = Install::open_from(files, "game", Lang::En, vec![m("bad")]).err().unwrap().to_string();
        assert!(e.contains("dataraw/text/s_story.txt: no such file"), "{e}");
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
