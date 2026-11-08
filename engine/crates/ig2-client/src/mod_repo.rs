//! Ours (ADR 0086 §1.2): the mod repository. The signed index (`index.json` of `openIG2-org/mods`) fetched over HTTPS
//! when the Mods page's Browse tab opens, its minisign signature checked against the keys built in here and the index
//! cached for offline use; the installer that downloads a release, checks its size, SHA-256 and zip rules (the zip
//! directory's, every file's CRC, [`ig2_formats::zipfs::data_only`], the `mod.toml`'s id and version) in memory and
//! only then moves it into `mods/` as `<id>.zip`, an update replacing the old zip only after the new one is verified.
//! A mod's dependencies are installed with it. [`install`] is the API the Publish button and the multiplayer room's
//! "Download and join" use (ADR 0086 §1.2, §3).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ig2_formats::vfs::{Mod, ModInfo, Version, VersionReq, MOD_FILE};
use sha2::{Digest, Sha256};

/// The signed index and its signature: the index repository's rolling `index` release.
pub const INDEX_URL: &str = "https://github.com/openIG2-org/mods/releases/download/index/index.json";
const SIG_URL: &str = "https://github.com/openIG2-org/mods/releases/download/index/index.json.minisig";
/// The maintainers' public keys (minisign, base64): the index verifies with any of them, so a second key can ship in a
/// release before the index switches to it (ADR 0086 §1.3).
pub const KEYS: [&str; 1] = ["RWTv/5gwRHG3qROOd3TsomYV1s5pfFdaRxzgxXX9+t9yhtO3SniTmmz/"];
/// The index formats this client reads.
const FORMAT: u32 = 1;
/// The largest index and signature read.
const MAX_INDEX: u64 = 16 << 20;
const MAX_SIG: u64 = 4096;
/// The largest picture read.
const MAX_PICTURE: u64 = 8 << 20;
/// A zip's files unpacked: at most this many times its size, and at most 4 GiB (the index's CI rules).
const MAX_RATIO: u64 = 200;
const MAX_UNPACKED: u64 = 4 << 30;

/// The index (`index.json`, format 1).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct Index {
    pub format: u32,
    /// When it was built (UTC, `YYYY-MM-DDTHH:MM:SSZ`).
    #[serde(default)]
    pub generated: String,
    /// Sorted by id, each id's releases newest first.
    pub mods: Vec<Entry>,
}

/// One mod release (`mods/<id>/<version>.toml` of the index repository).
#[derive(Debug, Clone, Default, PartialEq, serde::Deserialize)]
pub struct Entry {
    pub id: String,
    pub version: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub license: String,
    pub download: String,
    pub size: u64,
    pub sha256: String,
    /// The openIG2 versions it loads on (`mod.toml`'s range syntax); none is any.
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    /// Mod id and the version range it needs.
    #[serde(default)]
    pub depends: BTreeMap<String, String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    /// A PNG or JPEG screenshot's address.
    #[serde(default)]
    pub picture: Option<String>,
}

impl Entry {
    /// Whether it loads on the openIG2 `version`.
    pub fn fits(&self, version: &str) -> bool {
        self.requires.as_deref().is_none_or(|r| VersionReq::parse(r).is_ok_and(|r| r.matches_text(version)))
    }
}

/// An id the index's schema allows: it names the zip in `mods/`, so nothing else gets near a path.
fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    (1..=64).contains(&b.len()) && b.iter().enumerate().all(|(i, &c)| c.is_ascii_lowercase() || c.is_ascii_digit() || (i > 0 && (c == b'-' || c == b'_')))
}

impl Index {
    /// The index in `json`, its signature `sig` (a `.minisig` file) made with one of `keys`.
    pub fn verify(json: &[u8], sig: &str, keys: &[&str]) -> Result<Self, String> {
        let bad = "the mod index's signature doesn't match: it isn't the one the openIG2 maintainers signed";
        let sig = minisign_verify::Signature::decode(sig).map_err(|_| bad.to_string())?;
        let signed = keys.iter().filter_map(|k| minisign_verify::PublicKey::from_base64(k).ok()).any(|k| k.verify(json, &sig, false).is_ok());
        if !signed {
            return Err(bad.into());
        }
        let index: Self = serde_json::from_slice(json).map_err(|e| format!("the mod index doesn't read: {e}"))?;
        if index.format != FORMAT {
            return Err(format!("the mod index has format {}; this openIG2 reads format {FORMAT}: update openIG2", index.format));
        }
        Ok(index)
    }

    /// Each mod's newest release that loads on `version`, by id; releases of an id that needs another openIG2 left out.
    pub fn compatible(&self, version: &str) -> Vec<&Entry> {
        let mut out: Vec<&Entry> = Vec::new();
        for e in self.mods.iter().filter(|e| e.fits(version)) {
            match out.iter_mut().find(|o| o.id == e.id) {
                Some(o) if newer(e, o) => *o = e,
                Some(_) => {}
                None => out.push(e),
            }
        }
        out
    }

    /// `id`'s newest release in `range` that loads on `version`.
    pub fn newest(&self, id: &str, range: &VersionReq, version: &str) -> Option<&Entry> {
        let mut best: Option<&Entry> = None;
        for e in self.mods.iter().filter(|e| e.id == id && e.fits(version) && range.matches_text(&e.version)) {
            if best.is_none_or(|b| newer(e, b)) {
                best = Some(e);
            }
        }
        best
    }

    /// The release `id` `version` with the zip `sha256`.
    pub fn find(&self, id: &str, version: &str, sha256: &str) -> Option<&Entry> {
        self.mods.iter().find(|e| e.id == id && e.version == version && e.sha256.eq_ignore_ascii_case(sha256))
    }
}

/// `a` is a later release than `b` (the numbers; equal numbers keep the index's order, newest first).
fn newer(a: &Entry, b: &Entry) -> bool {
    Version::parse(&a.version) > Version::parse(&b.version)
}

/// The cached index's files, beside the settings: `mod-index/index.json` and its `.minisig`.
fn cache_dir() -> Option<PathBuf> {
    ig2_formats::options::Options::path().and_then(|p| p.parent().map(|d| d.join("mod-index")))
}

/// Where the index came from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Fetched just now.
    Fresh,
    /// The cached copy (when it was built), because the fetch failed for the reason given.
    Cached(String),
}

/// The index: fetched and verified, then cached; when the fetch or its signature fails, the cached copy (verified
/// again) with the reason.
pub fn load() -> Result<(Index, Source), String> {
    let fetched = get(INDEX_URL, MAX_INDEX).and_then(|json| {
        let sig = get(SIG_URL, MAX_SIG)?;
        let sig = String::from_utf8(sig).map_err(|_| "the mod index's signature isn't text".to_string())?;
        Ok((Index::verify(&json, &sig, &KEYS)?, json, sig))
    });
    let dir = cache_dir();
    match fetched {
        Ok((index, json, sig)) => {
            if let Some(d) = &dir {
                let written = std::fs::create_dir_all(d).and_then(|_| std::fs::write(d.join("index.json.minisig"), &sig)).and_then(|_| std::fs::write(d.join("index.json"), &json));
                if let Err(e) = written {
                    eprintln!("ig2-client: caching the mod index in {}: {e}", d.display());
                }
            }
            Ok((index, Source::Fresh))
        }
        Err(e) => {
            let cached = dir.and_then(|d| Some((std::fs::read(d.join("index.json")).ok()?, std::fs::read_to_string(d.join("index.json.minisig")).ok()?)));
            match cached {
                Some((json, sig)) => Index::verify(&json, &sig, &KEYS).map(|i| (i, Source::Cached(e.clone()))).map_err(|c| format!("{e}; the saved copy: {c}")),
                None => Err(e),
            }
        }
    }
}

/// The file at the `https` address `url`, at most `limit` bytes; redirects are followed (GitHub's downloads are), plain
/// HTTP is refused.
pub fn get(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (url, limit);
        Err("the browser build can't download mods".into())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !url.starts_with("https://") {
            return Err(format!("{url}: only https addresses are fetched"));
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .https_only(true)
            .timeout_global(Some(std::time::Duration::from_secs(300)))
            .user_agent(format!("openIG2/{}", crate::version()))
            .build()
            .into();
        let short = url.rsplit('/').next().unwrap_or(url);
        let mut r = agent.get(url).call().map_err(|e| format!("downloading {short}: {e}"))?;
        r.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| format!("downloading {short}: {e}"))
    }
}

/// An index entry's picture, from the web (at most [`MAX_PICTURE`]).
pub fn get_picture(url: &str) -> Result<Vec<u8>, String> {
    get(url, MAX_PICTURE)
}

/// The SHA-256 of `bytes`, lower-case hex.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether the downloaded `zip` is `e`'s: its size and SHA-256, the zip rules, data files only, and a top-level
/// `mod.toml` with `e`'s id and version. Checked in memory, before anything is written.
pub fn check_zip(zip: &[u8], e: &Entry) -> Result<(), String> {
    let what = format!("{} {}", e.id, e.version);
    if zip.len() as u64 != e.size {
        return Err(format!("{what}: the download is {} bytes, the index says {}", zip.len(), e.size));
    }
    if !sha256(zip).eq_ignore_ascii_case(&e.sha256) {
        return Err(format!("{what}: the download's SHA-256 isn't the index's (damaged or changed); nothing was installed"));
    }
    let mut c = std::io::Cursor::new(zip);
    let z = ig2_formats::zipfs::Zip::open(&mut c).map_err(|err| format!("{what}: {err}"))?;
    let (mut seen, mut unpacked) = (std::collections::HashSet::new(), 0u64);
    let mut info = None;
    for name in z.names() {
        if !seen.insert(name.to_lowercase()) {
            return Err(format!("{what}: {name} is in the zip twice"));
        }
        let bytes = z.read(&mut c, name).map_err(|err| format!("{what}: {err}"))?;
        unpacked += bytes.len() as u64;
        if unpacked > MAX_UNPACKED || unpacked > MAX_RATIO * zip.len() as u64 {
            return Err(format!("{what}: unpacks to far more than its size"));
        }
        ig2_formats::zipfs::data_only(name, &bytes).map_err(|err| format!("{what}: {name}: {err}"))?;
        if name == MOD_FILE {
            let text = String::from_utf8(bytes).map_err(|_| format!("{what}: {MOD_FILE} isn't UTF-8 text"))?;
            info = Some(ModInfo::parse(&text).map_err(|err| format!("{what}: {err}"))?);
        }
    }
    let info = info.ok_or_else(|| format!("{what}: the zip has no {MOD_FILE}"))?;
    if info.id.as_deref() != Some(e.id.as_str()) || info.version.as_deref() != Some(e.version.as_str()) {
        return Err(format!("{what}: the zip's {MOD_FILE} is another mod or version"));
    }
    Ok(())
}

/// A mod in the mods folders, as the repository sees it.
#[derive(Debug, Clone)]
pub struct Present {
    pub id: String,
    pub version: Option<String>,
    pub m: Mod,
}

/// Every mod in `folders`, by its id (a `mod.toml` that doesn't read counts as the folder's name).
pub fn present(folders: &[PathBuf]) -> Vec<Present> {
    Mod::available_in(folders)
        .into_iter()
        .map(|m| {
            let info = m.info().unwrap_or_default();
            Present { id: info.id_or(&m.name), version: info.version, m }
        })
        .collect()
}

/// Where the zip of mod `id` goes: the zip it already has (an update), else `<id>.zip` in `dir`.
fn target(dir: &Path, have: &[Present], id: &str) -> PathBuf {
    have.iter().find(|p| p.id == id && p.m.is_zip()).map_or_else(|| dir.join(format!("{id}.zip")), |p| p.m.dir.clone())
}

/// A download: the address and the most bytes ([`get`], or a test's files).
pub type Download = dyn Fn(&str, u64) -> Result<Vec<u8>, String>;

/// What [`install`] put into the mods folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    pub id: String,
    pub version: String,
    pub path: PathBuf,
}

/// Install the release `id` `version` with the zip `sha256` from `index` into `dir` (the mods folder beside the game),
/// with the dependencies it needs that `folders` (the mods folders) don't have in range, downloading through `get`
/// (address, the most bytes). Every zip is downloaded and checked first; only then do they replace the old ones. A mod
/// a player put in by hand as a folder is left alone. The mods installed, dependencies first.
pub fn install(index: &Index, id: &str, version: &str, sha256: &str, dir: &Path, folders: &[PathBuf], get: &Download) -> Result<Vec<Installed>, String> {
    let game = crate::version();
    let top = index.find(id, version, sha256).ok_or_else(|| format!("{id} {version} isn't in the mod index"))?;
    let have = present(folders);
    // The releases to fetch, dependencies first.
    let mut plan: Vec<&Entry> = Vec::new();
    fn need<'a>(index: &'a Index, e: &'a Entry, have: &[Present], game: &str, plan: &mut Vec<&'a Entry>, path: &mut Vec<String>) -> Result<(), String> {
        if plan.iter().any(|p| p.id == e.id) || path.contains(&e.id) {
            return Ok(());
        }
        path.push(e.id.clone());
        for (dep, range) in &e.depends {
            let range = VersionReq::parse(range).map_err(|err| format!("{}: depends {dep}: {err}", e.id))?;
            let fits = |v: &Option<String>| v.as_deref().is_some_and(|v| range.matches_text(v)) || range.is_any();
            if have.iter().any(|p| p.id == *dep && fits(&p.version)) || plan.iter().any(|p| p.id == *dep) {
                continue;
            }
            if have.iter().any(|p| p.id == *dep && !p.m.is_zip()) {
                return Err(format!("{} needs {dep} {range}; the {dep} folder in the mods folder is another version: update it by hand", e.id));
            }
            let d = index.newest(dep, &range, game).ok_or_else(|| format!("{} needs {dep} {range}, which the mod index doesn't have for this openIG2", e.id))?;
            need(index, d, have, game, plan, path)?;
        }
        path.pop();
        plan.push(e);
        Ok(())
    }
    need(index, top, &have, game, &mut plan, &mut Vec::new())?;
    for e in &plan {
        if !valid_id(&e.id) {
            return Err(format!("{:?} isn't a mod id", e.id));
        }
        if !e.fits(game) {
            return Err(format!("{} {} needs openIG2 {}", e.id, e.version, e.requires.as_deref().unwrap_or("*")));
        }
        if have.iter().any(|p| p.id == e.id && !p.m.is_zip()) {
            return Err(format!("{} is in the mods folder as a folder (put there by hand): remove it there first", e.id));
        }
    }
    let mut zips = Vec::new();
    for e in &plan {
        let zip = get(&e.download, e.size + 1)?;
        check_zip(&zip, e)?;
        zips.push((*e, zip));
    }
    std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let mut done = Vec::new();
    for (e, zip) in zips {
        let path = target(dir, &have, &e.id);
        replace(&path, &zip)?;
        done.push(Installed { id: e.id.clone(), version: e.version.clone(), path });
    }
    Ok(done)
}

/// Write `bytes` to `path`, a zip there kept as `.old` until the new one is in place (and put back when that fails).
fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let with = |ext: &str| {
        let mut p = path.as_os_str().to_owned();
        p.push(ext);
        PathBuf::from(p)
    };
    let (part, old) = (with(".part"), with(".old"));
    let io = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    std::fs::write(&part, bytes).map_err(|e| io(&part, e))?;
    let had = path.exists();
    if had {
        let _ = std::fs::remove_file(&old);
        std::fs::rename(path, &old).map_err(|e| io(path, e))?;
    }
    if let Err(e) = std::fs::rename(&part, path) {
        if had {
            let _ = std::fs::rename(&old, path);
        }
        let _ = std::fs::remove_file(&part);
        return Err(io(path, e));
    }
    if had {
        let _ = std::fs::remove_file(&old);
    }
    Ok(())
}

/// Remove the zip of mod `id` from `folders`: its name in the mods folder. A mod put there by hand as a folder isn't
/// removed.
pub fn remove(folders: &[PathBuf], id: &str) -> Result<String, String> {
    let have = present(folders);
    let p = have.iter().find(|p| p.id == id).ok_or_else(|| format!("{id} isn't installed"))?;
    if !p.m.is_zip() {
        return Err(format!("{id} is a folder put there by hand: remove it in the mods folder"));
    }
    std::fs::remove_file(&p.m.dir).map_err(|e| format!("{}: {e}", p.m.dir.display()))?;
    Ok(p.m.name.clone())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A key pair and the minisign signature of `data` with it.
    pub(crate) fn signed(data: &[u8]) -> (String, String) {
        let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let sig = minisign::sign(Some(&kp.pk), &kp.sk, std::io::Cursor::new(data), Some("timestamp:0\tfile:index.json\tformat:1"), None).unwrap();
        (kp.pk.to_base64(), sig.into_string())
    }

    fn mod_zip(id: &str, version: &str, extra: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
        let toml = format!("id = \"{id}\"\nversion = \"{version}\"\nrequires = \">=0.1\"\nlicense = \"MIT\"\n{extra}");
        let mut all: Vec<(&str, &[u8], bool)> = vec![(MOD_FILE, toml.as_bytes(), true)];
        all.extend(files.iter().map(|&(n, b)| (n, b, true)));
        ig2_formats::zipfs::build(&all)
    }

    fn entry(id: &str, version: &str, zip: &[u8]) -> Entry {
        Entry {
            id: id.into(),
            version: version.into(),
            name: id.into(),
            download: format!("https://github.com/a/b/releases/download/{version}/{id}.zip"),
            size: zip.len() as u64,
            sha256: sha256(zip),
            ..Default::default()
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ig2-mod-repo-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Downloads served from `files` (address → bytes).
    fn server(files: Vec<(String, Vec<u8>)>) -> impl Fn(&str, u64) -> Result<Vec<u8>, String> {
        move |url, limit| {
            let b = files.iter().find(|(u, _)| u == url).map(|(_, b)| b.clone()).ok_or_else(|| format!("{url}: 404"))?;
            if b.len() as u64 > limit {
                return Err("too large".into());
            }
            Ok(b)
        }
    }

    #[test]
    fn signature_good_bad_and_rotated() {
        let json = br#"{"format": 1, "generated": "2026-11-08T15:29:00Z", "mods": []}"#;
        let (key, sig) = signed(json);
        let (other, _) = signed(b"x");
        assert!(Index::verify(json, &sig, &[&key]).is_ok());
        // A second key beside the first (a rotation): either verifies.
        assert!(Index::verify(json, &sig, &[&other, &key]).is_ok());
        assert!(Index::verify(json, &sig, &[&other]).unwrap_err().contains("signature"));
        let mut changed = json.to_vec();
        changed[12] = b'2';
        assert!(Index::verify(&changed, &sig, &[&key]).is_err());
        assert!(Index::verify(json, "not a signature", &[&key]).is_err());
        let newer = br#"{"format": 2, "mods": []}"#;
        let (k2, s2) = signed(newer);
        assert!(Index::verify(newer, &s2, &[&k2]).unwrap_err().contains("update openIG2"));
    }

    #[test]
    fn the_built_in_key_reads() {
        assert!(KEYS.iter().all(|k| minisign_verify::PublicKey::from_base64(k).is_ok()));
    }

    #[test]
    fn compatible_newest_per_id() {
        let e = |id: &str, v: &str, req: Option<&str>| Entry { id: id.into(), version: v.into(), requires: req.map(Into::into), ..Default::default() };
        let index = Index { format: 1, generated: String::new(), mods: vec![e("a", "2.0.0", Some(">=9")), e("a", "1.5.0", None), e("a", "1.0.0", None), e("b", "0.1.0", Some("^0.7"))] };
        let c: Vec<(&str, &str)> = index.compatible("0.7.3").iter().map(|e| (e.id.as_str(), e.version.as_str())).collect();
        assert_eq!(c, [("a", "1.5.0"), ("b", "0.1.0")]);
        assert!(index.compatible("0.8.0").iter().all(|e| e.id == "a"));
        assert_eq!(index.newest("a", &VersionReq::parse("<1.5").unwrap(), "0.7.3").map(|e| e.version.as_str()), Some("1.0.0"));
    }

    #[test]
    fn zip_rules() {
        let good = mod_zip("m", "1.0.0", "", &[("textures/x.png", b"\x89PNG\r\n\x1a\nrest")]);
        assert!(check_zip(&good, &entry("m", "1.0.0", &good)).is_ok());
        let mut wrong_hash = entry("m", "1.0.0", &good);
        wrong_hash.sha256 = "0".repeat(64);
        assert!(check_zip(&good, &wrong_hash).unwrap_err().contains("SHA-256"));
        let mut wrong_size = entry("m", "1.0.0", &good);
        wrong_size.size += 1;
        assert!(check_zip(&good, &wrong_size).is_err());
        for (name, data) in [("run.exe", &b"MZ\x90\0"[..]), ("x.png", b"MZ\x90\0"), ("inner.zip", b"PK\x03\x04"), ("tool.py", b"print()")] {
            let z = mod_zip("m", "1.0.0", "", &[(name, data)]);
            assert!(check_zip(&z, &entry("m", "1.0.0", &z)).is_err(), "{name}");
        }
        let unsafe_name = ig2_formats::zipfs::build(&[("../evil.toml", b"x", false)]);
        assert!(check_zip(&unsafe_name, &entry("m", "1.0.0", &unsafe_name)).is_err());
        let other = mod_zip("n", "1.0.0", "", &[]);
        assert!(check_zip(&other, &entry("m", "1.0.0", &other)).unwrap_err().contains("another mod"));
        let twice = ig2_formats::zipfs::build(&[(MOD_FILE, b"id = \"m\"\nversion = \"1.0.0\"", false), ("A.txt", b"x", false), ("a.txt", b"y", false)]);
        assert!(check_zip(&twice, &entry("m", "1.0.0", &twice)).unwrap_err().contains("twice"));
    }

    #[test]
    fn installs_with_dependencies_and_updates() {
        let dir = tmp("deps");
        let lib = mod_zip("lib", "1.2.0", "", &[]);
        let app = mod_zip("app", "1.0.0", "[depends]\nlib = \">=1.1\"\n", &[]);
        let (le, ae) = (entry("lib", "1.2.0", &lib), Entry { depends: [("lib".into(), ">=1.1".into())].into(), ..entry("app", "1.0.0", &app) });
        let index = Index { format: 1, generated: String::new(), mods: vec![ae.clone(), le.clone()] };
        let get = server(vec![(ae.download.clone(), app.clone()), (le.download.clone(), lib.clone())]);
        let folders = [dir.clone()];
        let done = install(&index, "app", "1.0.0", &ae.sha256, &dir, &folders, &get).unwrap();
        assert_eq!(done.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["lib", "app"]);
        assert_eq!(std::fs::read(dir.join("app.zip")).unwrap(), app);
        let have = present(&folders);
        assert!(have.iter().any(|p| p.id == "lib" && p.version.as_deref() == Some("1.2.0")));
        // Installed already: the dependency isn't fetched again.
        let only = install(&index, "app", "1.0.0", &ae.sha256, &dir, &folders, &get).unwrap();
        assert_eq!(only.len(), 1);
        // An update whose download is damaged keeps the old zip.
        let app2 = mod_zip("app", "1.1.0", "[depends]\nlib = \">=1.1\"\n", &[]);
        let a2 = entry("app", "1.1.0", &app2);
        let index2 = Index { format: 1, generated: String::new(), mods: vec![a2.clone(), ae, le] };
        let mut damaged = app2.clone();
        *damaged.last_mut().unwrap() ^= 1;
        let bad = server(vec![(a2.download.clone(), damaged)]);
        assert!(install(&index2, "app", "1.1.0", &a2.sha256, &dir, &folders, &bad).unwrap_err().contains("SHA-256"));
        assert_eq!(std::fs::read(dir.join("app.zip")).unwrap(), app);
        let good = server(vec![(a2.download.clone(), app2.clone())]);
        install(&index2, "app", "1.1.0", &a2.sha256, &dir, &folders, &good).unwrap();
        assert_eq!(std::fs::read(dir.join("app.zip")).unwrap(), app2);
        assert!(!dir.join("app.zip.old").exists() && !dir.join("app.zip.part").exists());
        // Remove.
        assert_eq!(remove(&folders, "app").unwrap(), "app");
        assert!(!dir.join("app.zip").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_dependency_and_hand_folders() {
        let dir = tmp("hand");
        let app = mod_zip("app", "1.0.0", "[depends]\nlib = \"2\"\n", &[]);
        let ae = Entry { depends: [("lib".into(), "2".into())].into(), ..entry("app", "1.0.0", &app) };
        let index = Index { format: 1, generated: String::new(), mods: vec![ae.clone()] };
        let get = server(vec![(ae.download.clone(), app)]);
        let folders = [dir.clone()];
        assert!(install(&index, "app", "1.0.0", &ae.sha256, &dir, &folders, &get).unwrap_err().contains("doesn't have"));
        assert!(!dir.join("app.zip").exists());
        // A folder put in by hand isn't replaced or removed.
        std::fs::create_dir_all(dir.join("app")).unwrap();
        std::fs::write(dir.join("app").join(MOD_FILE), "id = \"app\"\nversion = \"0.9.0\"").unwrap();
        assert!(install(&index, "app", "1.0.0", &ae.sha256, &dir, &folders, &get).is_err());
        assert!(remove(&folders, "app").unwrap_err().contains("by hand"));
        assert!(install(&index, "app", "9.0.0", &ae.sha256, &dir, &folders, &get).unwrap_err().contains("isn't in"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ids_name_files_safely() {
        assert!(valid_id("my-mod_2") && !valid_id("-x") && !valid_id("../x") && !valid_id("A") && !valid_id(""));
    }
}
