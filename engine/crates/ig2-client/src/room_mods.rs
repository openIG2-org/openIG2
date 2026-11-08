//! Ours (ADR 0086 §3, closing ADR 0068 §6's open item): the multiplayer room's mods. The host sends its enabled mods
//! (id, version, the zip's SHA-256) to a joiner it refuses for other mods; the joiner compares them with its own
//! ([`diff`]: missing, another version, extra), looks the room's up in the signed mod index (id, version and SHA-256
//! must all match) and, when every one is there and nothing extra is enabled, "Download and join" installs them
//! ([`mod_repo::install`]), writes `enabled.txt` in the room's order and starts the game again into the room
//! ([`restart_into`]: the mods load at start, ADR 0068 §5). The fingerprint stays the check: a joiner whose files
//! still differ is refused again. `kind = "visual"` mods are left out on both sides, as they are of the fingerprint.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use ig2_formats::vfs::{Mod, ModKind};
use ig2_net::lobby::RoomMod;

use crate::mod_repo::{self, Index};

/// `m` as the room sees it; `None` for a look-only mod.
fn room_mod(m: &Mod) -> Option<RoomMod> {
    let info = m.info().unwrap_or_default();
    if info.kind == ModKind::Visual {
        return None;
    }
    let sha256 = if m.is_zip() { std::fs::read(&m.dir).ok().map(|b| mod_repo::sha256(&b)) } else { None };
    Some(RoomMod { id: info.id_or(&m.name), version: info.version.unwrap_or_default(), sha256 })
}

/// The enabled mods in priority order, look-only ones left out.
pub fn list(mods: &[Mod]) -> Vec<RoomMod> {
    mods.iter().filter_map(room_mod).collect()
}

/// How a joiner's mods differ from the room's.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Diff {
    /// The room's mods this player hasn't enabled.
    pub missing: Vec<RoomMod>,
    /// The room's mods this player has in another version or with other files (the room's, this player's version).
    pub other: Vec<(RoomMod, String)>,
    /// This player's mods the room doesn't have.
    pub extra: Vec<RoomMod>,
    /// The same mods in another priority order.
    pub order: bool,
}

/// `mine` against `room`, both in priority order.
pub fn diff(room: &[RoomMod], mine: &[RoomMod]) -> Diff {
    let mut d = Diff::default();
    for r in room {
        match mine.iter().find(|m| m.id == r.id) {
            None => d.missing.push(r.clone()),
            Some(m) if m != r => d.other.push((r.clone(), m.version.clone())),
            Some(_) => {}
        }
    }
    d.extra = mine.iter().filter(|m| !room.iter().any(|r| r.id == m.id)).cloned().collect();
    d.order = d.missing.is_empty() && d.other.is_empty() && d.extra.is_empty() && room != mine;
    d
}

impl Diff {
    /// The room's mods to install.
    pub fn wanted(&self) -> impl Iterator<Item = &RoomMod> {
        self.missing.iter().chain(self.other.iter().map(|o| &o.0))
    }

    /// The wanted mods `index` doesn't have (id, version and SHA-256; a folder mod has no SHA-256, so none).
    pub fn unavailable(&self, index: &Index) -> Vec<&RoomMod> {
        self.wanted().filter(|m| m.sha256.as_deref().is_none_or(|s| index.find(&m.id, &m.version, s).is_none())).collect()
    }
}

/// Install `d`'s wanted mods from `index` into `dir` (the mods folder beside the game; `folders`: every mods folder),
/// downloading through `get`. Gives `enabled.txt`'s names: the room's mods in the room's order, then the look-only mods
/// of `enabled` (this player's list) that are still there.
pub fn install(index: &Index, room: &[RoomMod], d: &Diff, dir: &Path, folders: &[PathBuf], enabled: &[String], get: &mod_repo::Download) -> Result<Vec<String>, String> {
    for m in d.wanted() {
        let sha = m.sha256.as_deref().ok_or_else(|| format!("{} {} isn't in the mod index", m.id, m.version))?;
        mod_repo::install(index, &m.id, &m.version, sha, dir, folders, get)?;
    }
    let have = mod_repo::present(folders);
    let mut names = Vec::new();
    for r in room {
        let mut of_id = have.iter().filter(|p| p.id == r.id);
        let p = of_id.clone().find(|p| p.version.as_deref().unwrap_or("") == r.version).or_else(|| of_id.next()).ok_or_else(|| format!("{} isn't in the mods folder", r.id))?;
        names.push(p.m.name.clone());
    }
    let visual = |n: &String| have.iter().any(|p| p.m.name == *n && p.m.info().is_ok_and(|i| i.kind == ModKind::Visual));
    let looks: Vec<String> = enabled.iter().filter(|n| visual(n) && !names.contains(n)).cloned().collect();
    names.extend(looks);
    Ok(names)
}

/// The game again with this run's arguments but `--mod` and `--mp-join` (the new `enabled.txt` decides the mods),
/// joining `target` (an address or a join code) once the main menu is up; the caller then quits.
pub fn restart_into(target: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe).args(restart_args(std::env::args_os().skip(1), target)).spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn restart_args(run: impl Iterator<Item = std::ffi::OsString>, target: &str) -> Vec<std::ffi::OsString> {
    let mut out = Vec::new();
    let mut it = run;
    while let Some(a) = it.next() {
        if a == "--mod" || a == "--mp-join" {
            it.next();
            continue;
        }
        out.push(a);
    }
    out.extend(["--mp-join".into(), target.into()]);
    out
}

/// A download a thread can take ([`mod_repo::Download`]).
pub type Get = dyn Fn(&str, u64) -> Result<Vec<u8>, String> + Send;

/// The room-mods dialog's state ([`crate::mp_lobby::MpScreen`]).
pub struct RoomMods {
    pub room: Vec<RoomMod>,
    pub diff: Diff,
    /// The mod index, or why there is none; `None` while it is fetched (only when anything is to be installed).
    pub index: Option<Result<Index, String>>,
    checking: Option<mpsc::Receiver<Result<Index, String>>>,
    installing: Option<mpsc::Receiver<Result<(), String>>>,
    /// The last install's error.
    pub error: Option<String>,
}

/// What the dialog offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    /// Busy (the index, the download).
    Wait,
    /// "Download and join" (`true`: something to download, else only `enabled.txt`'s order).
    Join(bool),
    /// What blocks it, said in the body; the button opens Mods.
    Mods,
}

impl RoomMods {
    /// A refusal for other mods: `room` against `mine`; the index fetched when anything is to be installed (`fetch`:
    /// [`mod_repo::load`] in the game, a fixture in tests).
    pub fn new(room: Vec<RoomMod>, mine: &[RoomMod], fetch: impl FnOnce() -> Result<Index, String> + Send + 'static) -> Self {
        let diff = diff(&room, mine);
        let mut s = Self { room, diff, index: None, checking: None, installing: None, error: None };
        if s.diff.wanted().next().is_none() {
            s.index = Some(Ok(Index::default()));
        } else {
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || _ = tx.send(fetch()));
            s.checking = Some(rx);
        }
        s
    }

    /// Take a finished fetch or install; `true` once the mods are installed (the restart is due).
    pub fn poll(&mut self) -> bool {
        if let Some(rx) = &self.checking {
            match rx.try_recv() {
                Ok(r) => (self.index, self.checking) = (Some(r), None),
                Err(mpsc::TryRecvError::Disconnected) => (self.index, self.checking) = (Some(Err("the mod index didn't load".into())), None),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let Some(rx) = &self.installing else { return false };
        let done = match rx.try_recv() {
            Ok(r) => r,
            Err(mpsc::TryRecvError::Disconnected) => Err("the download stopped".into()),
            Err(mpsc::TryRecvError::Empty) => return false,
        };
        self.installing = None;
        match done {
            Ok(()) => true,
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }

    /// The download runs.
    pub fn installing(&self) -> bool {
        self.installing.is_some()
    }

    pub fn busy(&self) -> bool {
        self.checking.is_some() || self.installing.is_some()
    }

    pub fn offer(&self) -> Offer {
        let wanted = self.diff.wanted().next().is_some();
        match &self.index {
            _ if self.busy() => Offer::Wait,
            _ if !self.diff.extra.is_empty() => Offer::Mods,
            Some(Ok(index)) if self.diff.unavailable(index).is_empty() && (wanted || self.diff.order) => Offer::Join(wanted),
            _ => Offer::Mods,
        }
    }

    /// Start the install (with [`Offer::Join`]): into `dir`, `enabled_file` written once every zip is in.
    pub fn start(&mut self, dir: PathBuf, folders: Vec<PathBuf>, enabled_file: Option<PathBuf>, get: Box<Get>) {
        let (Offer::Join(_), Some(Ok(index))) = (self.offer(), &self.index) else { return };
        let (index, room, d) = (index.clone(), self.room.clone(), self.diff.clone());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || _ = tx.send(join(&index, &room, &d, &dir, &folders, enabled_file, &*get)));
        self.error = None;
        self.installing = Some(rx);
    }
}

/// [`install`], then `enabled_file` written.
fn join(index: &Index, room: &[RoomMod], d: &Diff, dir: &Path, folders: &[PathBuf], enabled_file: Option<PathBuf>, get: &mod_repo::Download) -> Result<(), String> {
    let file = enabled_file.ok_or("no settings folder")?;
    let enabled = ig2_formats::userfs::read_to_string(&file).map(|t| Mod::parse_enabled(&t)).unwrap_or_default();
    let names = install(index, room, d, dir, folders, &enabled, get)?;
    file.parent().map_or(Ok(()), ig2_formats::userfs::create_dir_all).and_then(|_| ig2_formats::userfs::write(&file, Mod::enabled_text(&names))).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mod_repo::tests::{entry, mod_zip, server, tmp};

    fn rm(id: &str, version: &str, sha: Option<&str>) -> RoomMod {
        RoomMod { id: id.into(), version: version.into(), sha256: sha.map(Into::into) }
    }

    #[test]
    fn mod_lists_differ_by_missing_version_and_extra() {
        let room = [rm("a", "1.0.0", Some("aa")), rm("b", "2.0.0", Some("bb")), rm("c", "1.0.0", None)];
        let mine = [rm("b", "1.5.0", Some("b1")), rm("c", "1.0.0", None), rm("x", "0.1.0", Some("xx"))];
        let d = diff(&room, &mine);
        assert_eq!(d.missing, [room[0].clone()]);
        assert_eq!(d.other, [(room[1].clone(), "1.5.0".to_string())]);
        assert_eq!(d.extra, [mine[2].clone()]);
        assert!(!d.order);
        assert_eq!(d.wanted().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        // The same files with another hash count as another version; the same mods in another order only as order.
        assert_eq!(diff(&[rm("a", "1", Some("1"))], &[rm("a", "1", Some("2"))]).other.len(), 1);
        let swapped = diff(&[rm("a", "1", None), rm("b", "1", None)], &[rm("b", "1", None), rm("a", "1", None)]);
        assert!(swapped.order && swapped.wanted().next().is_none());
        assert_eq!(diff(&room, &room), Diff::default());
    }

    #[test]
    fn visual_mods_stay_out_of_the_list() {
        let dir = tmp("room-visual");
        for (name, toml) in [("looks", "id = \"looks\"
version = \"1.0.0\"
kind = \"visual\"
"), ("rules", "id = \"rules\"
version = \"2.0.0\"
")] {
            std::fs::create_dir_all(dir.join(name)).unwrap();
            std::fs::write(dir.join(name).join(ig2_formats::vfs::MOD_FILE), toml).unwrap();
        }
        std::fs::write(dir.join("zipped.zip"), mod_zip("zipped", "0.3.0", "", &[])).unwrap();
        let mods = Mod::available_in(std::slice::from_ref(&dir));
        let l = list(&mods);
        assert_eq!(l.iter().map(|m| (m.id.as_str(), m.version.as_str(), m.sha256.is_some())).collect::<Vec<_>>(), [("rules", "2.0.0", false), ("zipped", "0.3.0", true)]);
        let zip = l.iter().find(|m| m.id == "zipped").unwrap();
        assert_eq!(zip.sha256.as_deref(), Some(mod_repo::sha256(&std::fs::read(dir.join("zipped.zip")).unwrap()).as_str()));
        // A joiner's look-only mod isn't extra: it isn't in its list either.
        assert!(diff(&l, &l).extra.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn download_and_join_installs_from_the_index() {
        let dir = tmp("room-join");
        let (a, b) = (mod_zip("armada", "1.2.0", "", &[]), mod_zip("fusion", "0.3.0", "", &[]));
        let (ae, be) = (entry("armada", "1.2.0", &a), entry("fusion", "0.3.0", &b));
        let index = Index { format: 1, generated: String::new(), mods: vec![ae.clone(), be.clone()] };
        let room = vec![rm("fusion", "0.3.0", Some(&be.sha256)), rm("armada", "1.2.0", Some(&ae.sha256))];
        // This player has an old fusion and a look-only mod enabled.
        std::fs::write(dir.join("fusion.zip"), mod_zip("fusion", "0.2.0", "", &[])).unwrap();
        std::fs::create_dir_all(dir.join("looks")).unwrap();
        std::fs::write(dir.join("looks").join(ig2_formats::vfs::MOD_FILE), "id = \"looks\"
kind = \"visual\"
").unwrap();
        let enabled_file = dir.join("enabled.txt");
        std::fs::write(&enabled_file, "looks
fusion
").unwrap();
        let folders = vec![dir.clone()];
        let mine = list(&Mod::pick(&Mod::available_in(&folders), &["looks".into(), "fusion".into()]).0);
        let index2 = index.clone();
        let mut r = RoomMods::new(room.clone(), &mine, move || Ok(index2));
        assert_eq!(r.diff.other.len(), 1);
        assert_eq!(r.diff.missing.len(), 1);
        while r.busy() {
            r.poll();
        }
        assert_eq!(r.offer(), Offer::Join(true));
        r.start(dir.clone(), folders.clone(), Some(enabled_file.clone()), Box::new(server(vec![(ae.download.clone(), a.clone()), (be.download.clone(), b.clone())])));
        let mut done = false;
        while r.busy() {
            done |= r.poll();
        }
        assert!(done, "{:?}", r.error);
        assert_eq!(std::fs::read(dir.join("armada.zip")).unwrap(), a);
        assert_eq!(std::fs::read(dir.join("fusion.zip")).unwrap(), b);
        // The room's order, then the look-only mod.
        assert_eq!(Mod::parse_enabled(&std::fs::read_to_string(&enabled_file).unwrap()), ["fusion", "armada", "looks"]);
        // Now the lists agree.
        let now = list(&Mod::pick(&Mod::available_in(&folders), &Mod::parse_enabled(&std::fs::read_to_string(&enabled_file).unwrap())).0);
        assert_eq!(now, room);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mods_the_index_lacks_or_extra_mods_block_the_join() {
        let a = mod_zip("armada", "1.2.0", "", &[]);
        let ae = entry("armada", "1.2.0", &a);
        let index = Index { format: 1, generated: String::new(), mods: vec![ae.clone()] };
        let wait = |mut r: RoomMods| {
            while r.busy() {
                r.poll();
            }
            r
        };
        // Another hash than the index's, a folder mod (no hash), an unknown id.
        for m in [rm("armada", "1.2.0", Some(&"0".repeat(64))), rm("armada", "1.2.0", None), rm("other", "1.0.0", Some(&ae.sha256))] {
            let i = index.clone();
            let r = wait(RoomMods::new(vec![m.clone()], &[], move || Ok(i)));
            assert_eq!(r.diff.unavailable(r.index.as_ref().unwrap().as_ref().unwrap()), [&m]);
            assert_eq!(r.offer(), Offer::Mods);
        }
        // The index didn't load.
        assert_eq!(wait(RoomMods::new(vec![rm("armada", "1.2.0", Some(&ae.sha256))], &[], || Err("offline".into()))).offer(), Offer::Mods);
        // An extra mod blocks even when the rest downloads; nothing to fetch then.
        let i = index.clone();
        let r = wait(RoomMods::new(vec![rm("armada", "1.2.0", Some(&ae.sha256))], &[rm("mine", "1", None)], move || Ok(i)));
        assert_eq!(r.offer(), Offer::Mods);
        let r = RoomMods::new(Vec::new(), &[rm("mine", "1", None)], || panic!("no fetch"));
        assert_eq!(r.offer(), Offer::Mods);
        // Only the order differs: join without a download.
        let r = RoomMods::new(vec![rm("a", "1", None), rm("b", "1", None)], &[rm("b", "1", None), rm("a", "1", None)], || panic!("no fetch"));
        assert_eq!(r.offer(), Offer::Join(false));
    }

    #[test]
    fn the_restart_drops_mod_and_join_arguments() {
        let run = ["--fullscreen", "--mod", "x", "--mp-join", "old", "--lang", "hu"].map(std::ffi::OsString::from);
        assert_eq!(restart_args(run.into_iter(), "IG2-NEW"), ["--fullscreen", "--lang", "hu", "--mp-join", "IG2-NEW"].map(std::ffi::OsString::from));
    }
}
