//! Finding the user's IG2 install when `IG2_INSTALL_DIR` is unset (docs/USER_GUIDE.md → Installing).
//!
//! The places, in order: `IG2_INSTALL_DIR`, the folder the remake remembered in its settings file, the remake's own
//! folder and its parent (the exe put into the install, or the zip unpacked into it: that install wins over any
//! other on the machine), the GOG installers' registry keys (`HKLM\SOFTWARE\[WOW6432Node\]GOG.com\Games\<id>`,
//! written by the offline installer and by GOG Galaxy alike: the entry whose `gameName` names the game or whose
//! `exeFile` is `ig2.exe`, its `path`), and the default folders (`<drive>:\GOG Games\*`,
//! `<drive>:\Program Files (x86)\GOG Galaxy\Games\*`, any subfolder named "Imperium Galactica…"). A place
//! counts when it holds `dataraw/text/scenario.txt` and an `ig2.exe` of the catalogued build ([`crate::GOG_2017_SHA256`]).

use std::path::{Path, PathBuf};

/// The remake's settings file key (`ig2_formats::options::Options::own_path`) for the install folder.
pub const SETTINGS_KEY: &str = "InstallDir";

/// Where a candidate folder came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    Remembered,
    Registry,
    DefaultFolder,
    BesideRemake,
    /// The player pointed at it (a folder dropped on the window).
    Chosen,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Env => "IG2_INSTALL_DIR",
            Source::Remembered => "remembered",
            Source::Registry => "GOG registry",
            Source::DefaultFolder => "default folder",
            Source::BesideRemake => "beside the remake",
            Source::Chosen => "chosen",
        }
    }
}

/// Why a folder is not a usable install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Ok,
    Missing,
    /// No `dataraw/text/scenario.txt`.
    NotInstall,
    NoExe,
    /// An `ig2.exe` of another build (its sha256).
    WrongBuild(String),
}

impl Verdict {
    pub fn describe(&self) -> String {
        match self {
            Verdict::Ok => "ok".into(),
            Verdict::Missing => "no such folder".into(),
            Verdict::NotInstall => "not an IG2 install (no dataraw/text/scenario.txt)".into(),
            Verdict::NoExe => "no ig2.exe".into(),
            Verdict::WrongBuild(h) => format!("ig2.exe is another build (sha256 {}…); the GOG 2017 build is needed", &h[..h.len().min(12)]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub source: Source,
}

/// The file `rel` (`/`-separated) under `root`, matched case-insensitively per component (the 1999 names vary).
pub fn find_file(root: &Path, rel: &str) -> Option<PathBuf> {
    let mut at = root.to_path_buf();
    for part in rel.split('/') {
        let direct = at.join(part);
        at = if direct.exists() {
            direct
        } else {
            std::fs::read_dir(&at).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(part)))?
        };
    }
    Some(at)
}

/// Whether `root` is a usable install.
pub fn check(root: &Path) -> Verdict {
    if !root.is_dir() {
        return Verdict::Missing;
    }
    if find_file(root, "dataraw/text/scenario.txt").is_none() {
        return Verdict::NotInstall;
    }
    let Some(exe) = find_file(root, "ig2.exe").and_then(|p| std::fs::read(p).ok()) else { return Verdict::NoExe };
    match crate::check_build(&exe) {
        Ok(()) => Verdict::Ok,
        Err(crate::Error::UnknownBuild(h)) => Verdict::WrongBuild(h),
        Err(_) => Verdict::NoExe,
    }
}

/// A dropped or typed path as an install folder: a file in it (`ig2.exe`) stands for its folder.
pub fn folder_of(p: &Path) -> PathBuf {
    if p.is_file() { p.parent().map_or_else(|| p.to_path_buf(), Path::to_path_buf) } else { p.to_path_buf() }
}

/// The places to look, in order, without duplicates (`remembered`: the settings file's folder).
pub fn candidates(remembered: Option<&Path>) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut add = |path: PathBuf, source| {
        let key = |p: &Path| p.to_string_lossy().trim_end_matches(['/', '\\']).to_ascii_lowercase();
        if !path.as_os_str().is_empty() && !out.iter().any(|c| key(&c.path) == key(&path)) {
            out.push(Candidate { path, source });
        }
    };
    if let Some(p) = std::env::var_os(ig2_formats::vfs::INSTALL_ENV) {
        add(PathBuf::from(p), Source::Env);
    }
    if let Some(p) = remembered {
        add(p.to_path_buf(), Source::Remembered);
    }
    // `IG2_INSTALL_SEARCH=0`: only the two above (tests of the not-found window).
    if std::env::var("IG2_INSTALL_SEARCH").is_ok_and(|v| v == "0") {
        return out;
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        let up = dir.parent().map(Path::to_path_buf);
        add(dir, Source::BesideRemake);
        if let Some(up) = up {
            add(up, Source::BesideRemake);
        }
    }
    for p in registry::gog_paths() {
        add(p, Source::Registry);
    }
    for p in default_folders() {
        add(p, Source::DefaultFolder);
    }
    out
}

/// The first usable candidate, else every candidate with its verdict.
pub fn find(remembered: Option<&Path>) -> Result<Candidate, Vec<(Candidate, Verdict)>> {
    let mut tried = Vec::new();
    for c in candidates(remembered) {
        match check(&c.path) {
            Verdict::Ok => return Ok(c),
            v => tried.push((c, v)),
        }
    }
    Err(tried)
}

/// Whether a folder name looks like the game's (`Imperium Galactica II`, `Imperium Galactica 2 Alliances`, …).
fn named_like_game(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("imperium galactica") && !n.ends_with("galactica") && !n.contains("galactica 1") || n == "ig2"
}

/// `<drive>:\GOG Games\*` and GOG Galaxy's default library on every drive (Windows), the subfolders named like the game.
fn default_folders() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(windows) {
        for d in b'C'..=b'Z' {
            let drive = PathBuf::from(format!("{}:\\", d as char));
            for sub in ["GOG Games", "Games", "Program Files (x86)\\GOG Galaxy\\Games", "Program Files\\GOG Galaxy\\Games"] {
                roots.push(drive.join(sub));
            }
        }
    } else if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join("GOG Games"));
        roots.push(home.join("Games"));
    }
    let mut out = Vec::new();
    for r in roots {
        let Ok(rd) = std::fs::read_dir(&r) else { continue };
        let mut found: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir() && p.file_name().is_some_and(|n| named_like_game(&n.to_string_lossy()))).collect();
        found.sort();
        out.extend(found);
    }
    out
}

#[cfg(windows)]
mod registry {
    //! The GOG installers' uninstall data (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<product id>`), read through
    //! advapi32 without a crate.
    use std::ffi::c_void;
    use std::path::PathBuf;

    type Hkey = *mut c_void;
    const HKEY_LOCAL_MACHINE: usize = 0x8000_0002;
    const KEY_READ: u32 = 0x2_0019;
    const RRF_RT_REG_SZ: u32 = 0x2;
    /// The GOG product id of *Imperium Galactica II: Alliances* (the install's `goggame-1254614904.info`).
    const GOG_ID: &str = "1254614904";

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(key: Hkey, sub: *const u16, options: u32, sam: u32, out: *mut Hkey) -> i32;
        fn RegEnumKeyExW(key: Hkey, index: u32, name: *mut u16, len: *mut u32, reserved: *mut u32, class: *mut u16, class_len: *mut u32, time: *mut c_void) -> i32;
        fn RegGetValueW(key: Hkey, sub: *const u16, value: *const u16, flags: u32, kind: *mut u32, data: *mut c_void, len: *mut u32) -> i32;
        fn RegCloseKey(key: Hkey) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    /// A string value of `key\sub`.
    fn value(key: Hkey, sub: &[u16], name: &str) -> Option<String> {
        let name = wide(name);
        let mut buf = vec![0u16; 1024];
        let mut len = (buf.len() * 2) as u32;
        // SAFETY: `key` is open; the buffers outlive the call and `len` is their size in bytes.
        let r = unsafe { RegGetValueW(key, sub.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len) };
        (r == 0).then(|| String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]))
    }

    /// The `path` of every GOG entry for the game.
    pub fn gog_paths() -> Vec<PathBuf> {
        let mut out = Vec::new();
        for base in ["SOFTWARE\\WOW6432Node\\GOG.com\\Games", "SOFTWARE\\GOG.com\\Games"] {
            let mut key: Hkey = std::ptr::null_mut();
            // SAFETY: opening a key read-only into `key`; closed below.
            if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE as Hkey, wide(base).as_ptr(), 0, KEY_READ, &mut key) } != 0 {
                continue;
            }
            for i in 0.. {
                let mut name = [0u16; 256];
                let mut len = name.len() as u32;
                // SAFETY: `key` is open; `name` holds `len` characters.
                let r = unsafe { RegEnumKeyExW(key, i, name.as_mut_ptr(), &mut len, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
                if r != 0 {
                    break;
                }
                let sub: Vec<u16> = name[..len as usize].iter().copied().chain([0]).collect();
                let game = value(key, &sub, "gameName").unwrap_or_default().to_ascii_lowercase();
                let exe = value(key, &sub, "exeFile").unwrap_or_default();
                let id = String::from_utf16_lossy(&name[..len as usize]);
                if id == GOG_ID || game.contains("imperium galactica ii") || game.contains("imperium galactica 2") || exe.eq_ignore_ascii_case("ig2.exe") {
                    if let Some(p) = value(key, &sub, "path") {
                        out.push(PathBuf::from(p));
                    }
                }
            }
            // SAFETY: opened above.
            unsafe { RegCloseKey(key) };
        }
        out
    }
}

#[cfg(not(windows))]
mod registry {
    pub fn gog_paths() -> Vec<std::path::PathBuf> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names() {
        assert!(named_like_game("Imperium Galactica II"));
        assert!(named_like_game("Imperium Galactica 2 Alliances"));
        assert!(!named_like_game("Imperium Galactica"));
        assert!(!named_like_game("The Witcher 3 Wild Hunt GOTY"));
    }

    #[test]
    fn verdicts() {
        let tmp = std::env::temp_dir().join(format!("ig2-install-test-{}", std::process::id()));
        assert_eq!(check(&tmp), Verdict::Missing);
        std::fs::create_dir_all(tmp.join("DataRaw/Text")).unwrap();
        assert_eq!(check(&tmp), Verdict::NotInstall);
        std::fs::write(tmp.join("DataRaw/Text/Scenario.txt"), b"").unwrap();
        assert_eq!(check(&tmp), Verdict::NoExe, "found case-insensitively");
        std::fs::write(tmp.join("ig2.exe"), b"MZ").unwrap();
        assert!(matches!(check(&tmp), Verdict::WrongBuild(_)));
        assert_eq!(folder_of(&tmp.join("ig2.exe")), tmp);
        std::fs::remove_dir_all(&tmp).unwrap();
    }

    /// The real install (IG2_INSTALL_DIR; skipped without it) passes, and `find` returns it first.
    #[test]
    fn the_install_passes() {
        let Some(root) = std::env::var_os(ig2_formats::vfs::INSTALL_ENV) else { return };
        assert_eq!(check(Path::new(&root)), Verdict::Ok);
        let c = find(None).unwrap();
        assert_eq!((c.path, c.source), (PathBuf::from(root), Source::Env));
    }

    #[test]
    fn own_folder_before_registry_and_default_folders() {
        if std::env::var("IG2_INSTALL_SEARCH").is_ok_and(|v| v == "0") {
            return;
        }
        let c = candidates(None);
        let exe_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        let first = |s: Source| c.iter().position(|c| c.source == s);
        let own = first(Source::BesideRemake).unwrap();
        assert_eq!(c[own].path, exe_dir);
        for later in [Source::Registry, Source::DefaultFolder] {
            assert!(first(later).is_none_or(|i| i > own), "{later:?} before the exe's folder");
        }
    }

    #[test]
    fn candidates_have_no_duplicates() {
        let c = candidates(Some(Path::new("X:\\nowhere\\")));
        assert_eq!(c.iter().filter(|c| c.source == Source::Remembered).count(), 1);
    }
}
