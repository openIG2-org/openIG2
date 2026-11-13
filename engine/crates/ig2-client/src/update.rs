//! Ours (ADR 0087): the update notice and the updater. At the start the game asks GitHub for the project's latest
//! release ([`LATEST`]); a newer one than this build shows on the main menu ([`Updater::notice`]). On Windows the
//! player's press downloads the release's zip, checks its size and SHA-256 against the release's own, puts its files
//! in the place of the ones beside the running exe (the old ones kept until the new ones are in place) and starts the
//! game again; elsewhere, and when that fails, the press opens the download page. A build without a release version
//! (`-dev`) and the browser build never ask; `CheckUpdates` 0 in the settings turns it off.

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};

use egui::Vec2;

use crate::gui::ds::{self, Font, Text, Weight};
use crate::mod_repo::{self, Download};
use crate::ui::UiTexts;

/// The latest release, as GitHub's API tells it.
const LATEST: &str = "https://api.github.com/repos/openIG2-org/openIG2/releases/latest";
/// Where a release's archives are; nothing is downloaded from anywhere else.
const DOWNLOADS: &str = "https://github.com/openIG2-org/openIG2/releases/download/";
/// The page a player downloads a release from by hand.
pub const PAGE: &str = "https://github.com/openIG2-org/openIG2/releases/latest";
/// The option's key in our settings file: 0 never asks (default: asks at each start).
pub const KEY: &str = "CheckUpdates";
/// The largest reply and archive read.
const MAX_REPLY: u64 = 1 << 20;
const MAX_ARCHIVE: u64 = 256 << 20;
/// A replaced file until the next start, and a new one until it is in place.
const OLD: &str = ".update-old";
const NEW: &str = ".update-new";

/// The archive the updater installs on this platform (the release workflow's names).
// shortcut: Windows only (the tar.gz bundles aren't unpacked), the others get the notice and the page; add a tar
// reader and the executable bit when Linux or macOS players ask for it.
const ARCHIVE: Option<&str> = if cfg!(all(windows, target_arch = "x86_64")) { Some("windows-x64.zip") } else { None };

/// A release newer than this build.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// MAJOR.MINOR.N.
    pub version: String,
    /// This platform's archive, when the updater can install it.
    pub archive: Option<Archive>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Archive {
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(serde::Deserialize)]
struct Reply {
    tag_name: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(serde::Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    /// `sha256:<hex>`.
    #[serde(default)]
    digest: Option<String>,
}

/// MAJOR.MINOR.N as numbers; a build without a release version (`0.7-dev`) has none.
fn numbers(v: &str) -> Option<[u32; 3]> {
    if !v.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    let mut p = v.split('.').map(|n| n.parse().ok());
    let n = [p.next()??, p.next()??, p.next()??];
    p.next().is_none().then_some(n)
}

/// The release in GitHub's reply `json` when it is newer than the build `current`; its archive named `suffix`
/// (`openig2-<version>-<suffix>`) when the reply has it with a SHA-256 and at the project's own download address.
pub fn parse(json: &[u8], current: &str, suffix: Option<&str>) -> Result<Option<Release>, String> {
    let r: Reply = serde_json::from_slice(json).map_err(|e| format!("the release list doesn't read: {e}"))?;
    let version = r.tag_name.strip_prefix('v').unwrap_or(&r.tag_name);
    let (Some(new), Some(have)) = (numbers(version), numbers(current)) else { return Ok(None) };
    if new <= have {
        return Ok(None);
    }
    let archive = suffix.and_then(|s| {
        let name = format!("openig2-{version}-{s}");
        let a = r.assets.iter().find(|a| a.name == name)?;
        let sha256 = a.digest.as_deref()?.strip_prefix("sha256:")?;
        (a.browser_download_url == format!("{DOWNLOADS}v{version}/{name}") && a.size <= MAX_ARCHIVE).then(|| Archive { url: a.browser_download_url.clone(), size: a.size, sha256: sha256.into() })
    });
    Ok(Some(Release { version: version.into(), archive }))
}

/// A file path the updater writes beside the exe: `/`-separated plain names (the bundle's `models/`), no drives,
/// parent folders or hidden files.
fn plain(name: &str) -> bool {
    name.split('/').all(|p| !p.starts_with('.') && !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')))
}

/// Install the release's zip `a` into `dir`, the folder of the running `exe` (its file name), downloading through
/// `get`. The zip is checked in memory first (size, SHA-256, the zip's own checksums, plain file names, `exe` among
/// them); then every file is written beside its place and swapped in, the old ones kept as `.update-old` (a running
/// exe can be renamed, not overwritten) and put back when a swap fails.
pub fn install(a: &Archive, dir: &Path, exe: &str, get: &Download) -> Result<(), String> {
    let zip = get(&a.url, a.size + 1)?;
    if zip.len() as u64 != a.size || !mod_repo::sha256(&zip).eq_ignore_ascii_case(&a.sha256) {
        return Err("the download isn't the release's file (its size or SHA-256 differs); nothing was changed".into());
    }
    let mut c = std::io::Cursor::new(&zip);
    let z = ig2_formats::zipfs::Zip::open(&mut c)?;
    let mut files = Vec::new();
    // Folder entries ("models/") are made with their files.
    for name in z.names().filter(|n| !n.ends_with('/')) {
        if !plain(name) {
            return Err(format!("the release's zip has {name:?}, which the updater doesn't unpack: update by hand"));
        }
        files.push((name, z.read(&mut c, name)?));
    }
    if !files.iter().any(|(n, _)| n.eq_ignore_ascii_case(exe)) {
        return Err(format!("this game ({exe}) isn't a release's: the release's zip has no such file; update by hand"));
    }
    let at = |name: &str, ext: &str| dir.join(format!("{name}{ext}"));
    let io = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    let drop_new = || files.iter().for_each(|(n, _)| drop(std::fs::remove_file(at(n, NEW))));
    for (name, bytes) in &files {
        let new = at(name, NEW);
        if let Err(e) = new.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&new, bytes)) {
            drop_new();
            return Err(io(&new, e));
        }
    }
    // The files swapped in so far: (name, whether one was there before).
    let mut done: Vec<(&str, bool)> = Vec::new();
    for (name, _) in &files {
        let (path, old) = (at(name, ""), at(name, OLD));
        let had = path.exists();
        let swap = || {
            if had {
                let _ = std::fs::remove_file(&old);
                std::fs::rename(&path, &old)?;
            }
            std::fs::rename(at(name, NEW), &path)
        };
        if let Err(e) = swap() {
            if had && !path.exists() {
                let _ = std::fs::rename(&old, &path);
            }
            for (n, had) in done.into_iter().rev() {
                let _ = std::fs::remove_file(at(n, ""));
                if had {
                    let _ = std::fs::rename(at(n, OLD), at(n, ""));
                }
            }
            drop_new();
            return Err(io(&path, e));
        }
        done.push((name, had));
    }
    Ok(())
}

/// The last update's leftovers in `dir` and its folders removed (the replaced files; the old exe was still running).
pub fn clean(dir: &Path) {
    for f in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if f.file_type().is_ok_and(|t| t.is_dir()) {
            clean(&f.path());
        } else if f.file_name().to_str().is_some_and(|n| n.ends_with(OLD) || n.ends_with(NEW)) {
            let _ = std::fs::remove_file(f.path());
        }
    }
}

/// The running exe's folder and file name.
fn exe() -> Result<(std::path::PathBuf, String), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = exe.file_name().and_then(|n| n.to_str()).ok_or("the game's file has no readable name")?.to_owned();
    Ok((exe.parent().ok_or("the game's file has no folder")?.to_owned(), name))
}

/// [`clean`] beside the running exe, at the start.
pub fn clean_start() {
    if let Ok((dir, _)) = exe() {
        clean(&dir);
    }
}

fn spawn<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx
}

#[derive(Default)]
enum State {
    /// Not asked, nothing newer, or the ask failed.
    #[default]
    Idle,
    Checking(Receiver<Result<Option<Release>, String>>),
    Available(Release),
    Installing(Release, Receiver<Result<(), String>>),
    Failed(Release, String),
}

/// The window's updater: the ask at the start, the notice, the install.
#[derive(Default)]
pub struct Updater {
    state: State,
}

impl Updater {
    /// Asks for the latest release in the background when `on` and this is a release's build.
    pub fn start(on: bool) -> Self {
        let current = crate::version();
        if !on || cfg!(target_arch = "wasm32") || numbers(current).is_none() {
            return Self::default();
        }
        Self { state: State::Checking(spawn(move || parse(&mod_repo::get(LATEST, MAX_REPLY)?, current, ARCHIVE))) }
    }

    /// The background work's results taken; whether an update is in place and the game starts again.
    fn poll(&mut self) -> bool {
        match std::mem::take(&mut self.state) {
            State::Checking(rx) => match rx.try_recv() {
                Ok(Ok(Some(r))) => {
                    eprintln!("ig2-client: openIG2 {} is out: {PAGE}", r.version);
                    self.state = State::Available(r);
                }
                Ok(Ok(None)) | Err(TryRecvError::Disconnected) => {}
                Ok(Err(e)) => eprintln!("ig2-client: looking for a new version: {e}"),
                Err(TryRecvError::Empty) => self.state = State::Checking(rx),
            },
            State::Installing(r, rx) => match rx.try_recv() {
                Ok(Ok(())) => return true,
                Ok(Err(e)) => {
                    eprintln!("ig2-client: updating to {}: {e}", r.version);
                    self.state = State::Failed(r, e);
                }
                Err(TryRecvError::Disconnected) => self.state = State::Failed(r, "the update stopped".into()),
                Err(TryRecvError::Empty) => self.state = State::Installing(r, rx),
            },
            s => self.state = s,
        }
        false
    }

    /// One frame of the main menu's notice, at the bottom right over the version: the new version and the button that
    /// installs it (the download page's where the updater can't, or after it failed). Whether the update is in place:
    /// the caller starts the game again.
    pub fn notice(&mut self, ctx: &egui::Context, texts: &UiTexts, opacity: f32) -> bool {
        if self.poll() {
            return true;
        }
        let (release, busy, error) = match &self.state {
            State::Available(r) => (r.clone(), false, None),
            State::Installing(r, _) => (r.clone(), true, None),
            State::Failed(r, e) => (r.clone(), false, Some(e.clone())),
            _ => return false,
        };
        let k = ds::tokens();
        let tr = |en: &str| ig2_formats::text::decode(&texts.ours(en), texts.lang);
        let small = Font { size: 13.0, line: 18.0, weight: Weight::Regular, tracking: 0.04, upper: false, lcd: false };
        let w = 300.0f32.min(ctx.screen_rect().width() - 32.0);
        egui::Area::new(egui::Id::new("mm-update")).anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-44.0, -60.0)).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(opacity);
            ds::panel(ui, ds::PanelKind::Deep, 0.85, egui::Margin::same(16), |ui| {
                let iw = w - 32.0;
                ui.set_width(iw);
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 10.0);
                Text::new(&format!("{}: {}", tr("New version"), release.version), Font::LABEL).color(k.ink_silver).show(ui, Vec2::new(iw, 18.0));
                if busy {
                    Text::new(&tr("Downloading the update..."), small).color(k.ink_muted).show(ui, Vec2::new(iw, 18.0));
                    return;
                }
                if let Some(e) = &error {
                    Text::new(&tr("The update failed; the log has the reason."), small).color(k.ink_muted).show(ui, Vec2::new(iw, 18.0)).on_hover_text(e);
                }
                match release.archive.clone().filter(|_| error.is_none()) {
                    Some(a) => {
                        if ui.add(ds::Plate::new(&tr("Update and restart")).primary(true).width(iw)).clicked() {
                            self.state = State::Installing(release.clone(), spawn(move || {
                                let (dir, name) = exe()?;
                                install(&a, &dir, &name, &mod_repo::get)
                            }));
                        }
                    }
                    None => {
                        if ui.add(ds::Plate::new(&tr("Download page")).width(iw)).clicked() {
                            if let Err(e) = crate::mods_page::open(std::ffi::OsStr::new(PAGE)) {
                                eprintln!("ig2-client: {e}");
                            }
                        }
                    }
                }
            });
        });
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mod_repo::tests::{server, tmp};

    fn reply(tag: &str, name: &str, url: &str, digest: &str) -> Vec<u8> {
        format!(r#"{{"tag_name": "{tag}", "assets": [{{"name": "{name}", "browser_download_url": "{url}", "size": 5, "digest": {digest}}}]}}"#).into_bytes()
    }

    #[test]
    fn newer_releases_only() {
        let (name, url) = ("openig2-0.7.12-windows-x64.zip", format!("{DOWNLOADS}v0.7.12/openig2-0.7.12-windows-x64.zip"));
        let json = reply("v0.7.12", name, &url, "\"sha256:ab\"");
        let zip = Some("windows-x64.zip");
        let r = parse(&json, "0.7.9", zip).unwrap().unwrap();
        assert_eq!((r.version.as_str(), r.archive), ("0.7.12", Some(Archive { url: url.clone(), size: 5, sha256: "ab".into() })));
        // The same, an older one, a build without a release version, a tag that is no version: no notice.
        for have in ["0.7.12", "0.7.13", "0.8.0", "1.0.0", "0.7-dev"] {
            assert_eq!(parse(&json, have, zip).unwrap(), None, "{have}");
        }
        assert_eq!(parse(&reply("nightly", name, &url, "null"), "0.7.9", zip).unwrap(), None);
        assert!(parse(&reply("v0.7.12", name, &url, "null"), "0.6.200", zip).unwrap().is_some());
        // Without the platform's archive, its SHA-256 or the project's own address: the notice, nothing to install.
        assert_eq!(parse(&json, "0.7.9", None).unwrap().unwrap().archive, None);
        assert_eq!(parse(&json, "0.7.9", Some("linux-x64.tar.gz")).unwrap().unwrap().archive, None);
        assert_eq!(parse(&reply("v0.7.12", name, &url, "null"), "0.7.9", zip).unwrap().unwrap().archive, None);
        assert_eq!(parse(&reply("v0.7.12", name, "https://example.com/openig2-0.7.12-windows-x64.zip", "\"sha256:ab\""), "0.7.9", zip).unwrap().unwrap().archive, None);
        assert!(parse(b"<html>rate limit</html>", "0.7.9", zip).is_err());
    }

    #[test]
    fn installs_and_keeps_the_old_files_on_a_failure() {
        let dir = tmp("update");
        std::fs::write(dir.join("openIG2.exe"), b"old game").unwrap();
        std::fs::write(dir.join("ig2.settings"), b"mine").unwrap();
        let zip = ig2_formats::zipfs::build(&[("openig2-0.7.12-windows-x64/openIG2.exe", b"new game", true), ("openig2-0.7.12-windows-x64/README.txt", b"read", false), ("openig2-0.7.12-windows-x64/models/m.bin", b"model", false)]);
        let a = Archive { url: "https://x/a.zip".into(), size: zip.len() as u64, sha256: mod_repo::sha256(&zip) };
        let read = |n: &str| std::fs::read(dir.join(n)).unwrap();
        // A damaged download, another game's file name, a zip with a folder: nothing changes.
        let mut damaged = zip.clone();
        *damaged.last_mut().unwrap() ^= 1;
        assert!(install(&a, &dir, "openIG2.exe", &server(vec![(a.url.clone(), damaged)])).unwrap_err().contains("SHA-256"));
        let get = server(vec![(a.url.clone(), zip)]);
        assert!(install(&a, &dir, "ig2-client.exe", &get).unwrap_err().contains("by hand"));
        let nested = ig2_formats::zipfs::build(&[("a/openIG2.exe", b"x", false), ("a/../c.txt", b"x", false)]);
        let n = Archive { url: "https://x/n.zip".into(), size: nested.len() as u64, sha256: mod_repo::sha256(&nested) };
        assert!(install(&n, &dir, "openIG2.exe", &server(vec![(n.url.clone(), nested)])).is_err());
        assert_eq!((read("openIG2.exe"), std::fs::read_dir(&dir).unwrap().count()), (b"old game".to_vec(), 2));
        // The update: the new files in place, the old exe beside it until the next start, the player's files untouched.
        install(&a, &dir, "openIG2.exe", &get).unwrap();
        assert_eq!((read("openIG2.exe"), read("README.txt"), read("openIG2.exe.update-old"), read("ig2.settings")), (b"new game".to_vec(), b"read".to_vec(), b"old game".to_vec(), b"mine".to_vec()));
        // A second update replaces the folder's file too; clean takes the leftovers in folders as well.
        install(&a, &dir, "openIG2.exe", &get).unwrap();
        assert_eq!((read("models/m.bin"), read("models/m.bin.update-old")), (b"model".to_vec(), b"model".to_vec()));
        clean(&dir);
        assert_eq!((std::fs::read_dir(&dir).unwrap().count(), std::fs::read_dir(dir.join("models")).unwrap().count()), (4, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plain_names_only() {
        assert!(plain("openIG2-check.exe") && plain("VERSION.txt") && plain("models/x.bin"));
        assert!(!plain("../x") && !plain("a/../b") && !plain("/a") && !plain("a//b") && !plain("c:x") && !plain(".hidden") && !plain("") && !plain("a\\b"));
    }
}
