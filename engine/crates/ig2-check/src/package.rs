//! `openIG2-check --package FOLDER` (ADR 0086 §1.2): checks a mod folder, writes its zip, the zip's SHA-256 and the
//! mod index entry. The format checks need no install (the index's CI runs them without one); with an install the
//! mod is also loaded over it, as `openIG2-check --mod` does, and a file byte-identical to the install's is refused.

use std::path::{Path, PathBuf};

use ig2_formats::vfs::{ModInfo, Version, MOD_FILE, STORY_FILE};
use ig2_formats::zipfs::{MAX_ENTRIES, MAX_FILE};
use sha2::{Digest, Sha256};

/// Already compressed kinds, stored as they are.
const STORED: [&str; 3] = ["ogg", "mp4", "png"];

/// What `--package` wrote.
#[derive(Debug)]
pub struct Packaged {
    pub zip: PathBuf,
    pub sha256: String,
    pub size: u64,
    /// The index entry, written beside the zip.
    pub entry: PathBuf,
    /// What the checks said, one line each.
    pub notes: Vec<String>,
}

/// Check `folder` (over `install` when given) and write `<id>-<version>.zip`, `.zip.sha256` and `.toml` (the index
/// entry) into `out`. Every problem found is in the error, one per line.
pub fn package(folder: &Path, install: Option<&Path>, out: &Path) -> Result<Packaged, String> {
    let folder_name = folder.file_name().and_then(|n| n.to_str()).ok_or_else(|| format!("{}: not a mod folder", folder.display()))?;
    let info_text = std::fs::read_to_string(folder.join(MOD_FILE)).map_err(|e| format!("{MOD_FILE}: {e} (a published mod needs one)"))?;
    let info = ModInfo::parse(&info_text)?;
    let id = info.id_or(folder_name);
    let mut bad = Vec::new();
    if info.id.is_none() {
        // The folder's name stands in; it must be a valid id too.
        if let Err(e) = ModInfo::parse(&format!("id = {:?}", id)) {
            bad.push(format!("{e} (the folder's name; set id in {MOD_FILE})"));
        }
    }
    let version = info.version.clone().unwrap_or_default();
    if Version::parse(&version).is_none() {
        bad.push(format!("{MOD_FILE}: version = \"1.0.0\" is needed (semantic versioning), not {version:?}"));
    }
    if info.requires.is_none() {
        bad.push(format!("{MOD_FILE}: requires = \">=0.7\" is needed (the openIG2 versions it loads on)"));
    }
    if info.license.as_deref().is_none_or(str::is_empty) {
        bad.push(format!("{MOD_FILE}: license = \"CC-BY-4.0\" is needed (an SPDX licence that allows sharing)"));
    }
    let files = collect(folder, &mut bad)?;
    if let Some(p) = info.picture.as_deref().filter(|p| !files.iter().any(|(r, _)| r.eq_ignore_ascii_case(p))) {
        bad.push(format!("{MOD_FILE}: picture {p:?} isn't in the mod"));
    }
    let mut notes = vec![format!("mod {id} {version}: {} files", files.len())];
    for (rel, bytes) in &files {
        if let Err(e) = check_file(rel, bytes) {
            bad.push(format!("{rel}: {e}"));
        }
    }
    match install {
        Some(root) => notes.extend(over_install(folder, folder_name, root, &files, &mut bad)),
        None => notes.push("no install: the format checks only".to_owned()),
    }
    if !bad.is_empty() {
        return Err(bad.join("\n"));
    }
    let zip = zip(&files)?;
    let sha256: String = Sha256::digest(&zip).iter().map(|b| format!("{b:02x}")).collect();
    let stem = format!("{id}-{version}");
    let write = |name: String, data: &[u8]| -> Result<PathBuf, String> {
        let p = out.join(name);
        std::fs::write(&p, data).map_err(|e| format!("{}: {e}", p.display()))?;
        Ok(p)
    };
    let zip_path = write(format!("{stem}.zip"), &zip)?;
    write(format!("{stem}.zip.sha256"), format!("{sha256}  {stem}.zip\n").as_bytes())?;
    let entry = write(format!("{stem}.toml"), index_entry(&id, &version, &info, zip.len() as u64, &sha256).as_bytes())?;
    notes.push(format!("the index entry goes to mods/{id}/{version}.toml in the index repository, its download URL filled in"));
    Ok(Packaged { zip: zip_path, sha256, size: zip.len() as u64, entry, notes })
}

/// Every file under `folder` (path inside the mod, bytes), sorted; dot files and folders are left out.
fn collect(folder: &Path, bad: &mut Vec<String>) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut files = Vec::new();
    let mut todo = vec![folder.to_path_buf()];
    while let Some(dir) = todo.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let e = e.map_err(|e| format!("{}: {e}", dir.display()))?;
            let path = e.path();
            let rel = path.strip_prefix(folder).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let name = e.file_name().to_string_lossy().into_owned();
            let kind = e.file_type().map_err(|e| format!("{rel}: {e}"))?;
            if name.starts_with('.') {
                continue;
            }
            if kind.is_symlink() {
                bad.push(format!("{rel}: a link (copy the file into the mod instead)"));
            } else if kind.is_dir() {
                todo.push(path);
            } else if e.file_name().to_str().is_none() {
                bad.push(format!("{rel}: the name isn't Unicode"));
            } else {
                let len = e.metadata().map_err(|e| format!("{rel}: {e}"))?.len();
                if len > MAX_FILE {
                    bad.push(format!("{rel}: larger than {} MiB", MAX_FILE >> 20));
                    continue;
                }
                files.push((rel, std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?));
            }
        }
    }
    if files.len() > MAX_ENTRIES {
        bad.push(format!("more than {MAX_ENTRIES} files"));
    }
    files.sort();
    Ok(files)
}

/// One file's kind ([`ig2_formats::zipfs::data_only`]) and, for the mod's own files, its format: all of it needs no install.
fn check_file(rel: &str, bytes: &[u8]) -> Result<(), String> {
    ig2_formats::zipfs::data_only(rel, bytes)?;
    let name = rel.rsplit('/').next().unwrap_or(rel).to_ascii_lowercase();
    let ext = name.rsplit_once('.').map(|(_, e)| e);
    let text = || std::str::from_utf8(bytes).map_err(|_| "not UTF-8 text".to_string());
    match ext {
        Some("png") if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Err("not a PNG".into()),
        Some("toml") => {
            let t = text()?;
            let top = !rel.contains('/');
            if top && rel == STORY_FILE {
                ig2_sim::story_mods::parse(t, "package").map(drop)
            } else if top && rel == ig2_formats::visuals::VISUALS_FILE {
                ig2_formats::visuals::Visuals::parse(t, rel).map(drop)
            } else if top && rel == ig2_formats::effects::EFFECTS_FILE {
                ig2_formats::effects::Effects::parse(t, rel).map(drop)
            } else if let Some(lang) = top.then(|| ig2_formats::textpatch::file_lang(rel)).flatten() {
                ig2_formats::textpatch::TextPatch::default().add(t, rel, lang).map(drop)
            } else {
                // A rules.toml's tables and ids are checked against the install's (`over_install`); its syntax here.
                t.parse::<toml_edit::DocumentMut>().map(drop).map_err(|e| e.to_string())
            }
        }
        _ => Ok(()),
    }
}

/// The checks that need the install: the mod loaded over it (its rules, texts and stories, as `--mod` does) and no
/// file that is the install's own.
fn over_install(folder: &Path, name: &str, root: &Path, files: &[(String, Vec<u8>)], bad: &mut Vec<String>) -> Vec<String> {
    let lang = ig2_formats::locale::exe_language(root).0;
    let m = ig2_formats::vfs::Mod { name: name.to_owned(), dir: folder.to_path_buf() };
    let inst = match ig2_formats::Install::open_with_mods(root, lang, vec![m]) {
        Ok(i) => i,
        Err(e) => {
            bad.push(format!("loading the mod over the install: {e}"));
            return Vec::new();
        }
    };
    let plain = ig2_formats::Install::open(root, lang).ok();
    for (rel, bytes) in files {
        // The localized copy and the base file.
        let mut own = [plain.as_ref().and_then(|p| p.resolve(rel)).map(Path::to_path_buf), ig2_rules::install::find_file(root, rel)].into_iter().flatten();
        if own.any(|p| std::fs::read(p).is_ok_and(|b| b == *bytes)) {
            bad.push(format!("{rel}: the install's own file (a mod may hold only its author's work)"));
        }
    }
    let g = crate::check_mods(&inst, Vec::new(), lang);
    bad.extend(g.failed.into_iter().map(|(what, e)| format!("{what}: {e}")));
    let mut notes = vec![format!("loaded over {}", root.display())];
    notes.extend(g.notes);
    notes
}

/// The zip of `files`: at its top, the compressed kinds stored; read back through the client's zip rules.
fn zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let total: u64 = files.iter().map(|(_, b)| b.len() as u64).sum();
    if total >= u32::MAX as u64 {
        return Err("the mod is 4 GiB or more: too large for one zip".into());
    }
    let entries: Vec<(&str, &[u8], bool)> =
        files.iter().map(|(r, b)| (r.as_str(), b.as_slice(), !STORED.iter().any(|e| r.to_ascii_lowercase().ends_with(&format!(".{e}"))))).collect();
    let zip = ig2_formats::zipfs::build(&entries);
    let mut c = std::io::Cursor::new(&zip);
    let z = ig2_formats::zipfs::Zip::open(&mut c).map_err(|e| format!("the written zip: {e}"))?;
    for (r, b) in files {
        if z.read(&mut c, r).map_err(|e| format!("the written zip: {e}"))? != *b {
            return Err(format!("the written zip: {r} reads back different"));
        }
    }
    Ok(zip)
}

/// The mod index entry (ADR 0086 §1.2) for the zip; the download URL is the author's to fill in.
fn index_entry(id: &str, version: &str, info: &ModInfo, size: u64, sha256: &str) -> String {
    use toml_edit::{value, Array, DocumentMut, InlineTable};
    let mut d = DocumentMut::new();
    d["id"] = value(id);
    d["version"] = value(version);
    for (k, v) in [("name", &info.name), ("author", &info.author), ("description", &info.description)] {
        if let Some(v) = v {
            d[k] = value(v.as_str());
        }
    }
    // The index's check wants these the same as in mod.toml, where a missing key is not an empty one.
    if let Some(r) = &info.requires {
        d["requires"] = value(r.to_string());
    }
    if !info.depends.is_empty() {
        let mut deps = InlineTable::new();
        for (dep, r) in &info.depends {
            deps.insert(dep, r.to_string().into());
        }
        d["depends"] = value(deps);
    }
    if !info.conflicts.is_empty() {
        d["conflicts"] = value(info.conflicts.iter().map(String::as_str).collect::<Array>());
    }
    d["license"] = value(info.license.as_deref().unwrap_or_default());
    if info.kind == ig2_formats::vfs::ModKind::Visual {
        d["kind"] = value("visual");
    }
    d["size"] = value(size as i64);
    d["sha256"] = value(sha256);
    d["download"] = value("");
    format!(
        "# The mod index entry of {id} {version} (mods/{id}/{version}.toml in the index repository).\n\
         # Fill in download (the zip's URL, a release asset of your repository); a picture = \"https://…\" may be added.\n{d}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ig2_formats::text::Lang;

    const INFO: &str = "id = \"zorg-race\"\nname = \"Zorgons\"\nversion = \"1.2.0\"\nrequires = \">=0.7, <0.9\"\nlicense = \"CC-BY-4.0\"\n\
                        conflicts = [\"other\"]\n[depends]\nbase = \">=1.0\"\n";

    fn folder(files: &[(&str, &[u8])]) -> (PathBuf, PathBuf) {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!("openig2-package-{}-{n}", std::process::id()));
        let dir = base.join("zorg");
        for (rel, data) in files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, data).unwrap();
        }
        let out = base.join("out");
        std::fs::create_dir_all(&out).unwrap();
        (dir, out)
    }

    #[test]
    fn a_mod_packages_without_an_install() {
        let png = b"\x89PNG\r\n\x1a\nrest";
        let texts = "[\"dataraw/text/s_races.txt\"]\n900 = \"Zorgons\"\n";
        let (dir, out) = folder(&[
            ("mod.toml", INFO.as_bytes()),
            ("texts.en.toml", texts.as_bytes()),
            ("rules.toml", b"[techs.1]\ncost = 5\n"),
            ("hires/pic.png", png),
            ("dataraw/text/note.txt", b"x"),
            (".git/config", b"MZ skipped"),
        ]);
        let p = package(&dir, None, &out).unwrap();
        assert!(p.notes.iter().any(|n| n.contains("format checks only")), "{:?}", p.notes);
        let zip = std::fs::read(&p.zip).unwrap();
        assert_eq!((p.zip.file_name().unwrap().to_str(), p.size), (Some("zorg-race-1.2.0.zip"), zip.len() as u64));
        // The hash is the zip's, in sha256sum's form.
        let want: String = Sha256::digest(&zip).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(p.sha256, want);
        assert_eq!(std::fs::read_to_string(out.join("zorg-race-1.2.0.zip.sha256")).unwrap(), format!("{want}  zorg-race-1.2.0.zip\n"));
        // The zip holds the mod's files at its top, the dot folder left out, and reads as a mod.
        let mut c = std::io::Cursor::new(&zip);
        let z = ig2_formats::zipfs::Zip::open(&mut c).unwrap();
        let names: Vec<&str> = z.names().collect();
        assert_eq!(names, ["dataraw/text/note.txt", "hires/pic.png", "mod.toml", "rules.toml", "texts.en.toml"]);
        assert_eq!(z.read(&mut c, "hires/pic.png").unwrap(), png);
        // The entry.
        let entry: toml_edit::DocumentMut = std::fs::read_to_string(&p.entry).unwrap().parse().unwrap();
        assert_eq!(entry["id"].as_str(), Some("zorg-race"));
        assert_eq!(entry["version"].as_str(), Some("1.2.0"));
        assert_eq!(entry["requires"].as_str(), Some(">=0.7, <0.9"));
        assert_eq!(entry["depends"]["base"].as_str(), Some(">=1.0"));
        assert_eq!(entry["conflicts"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect::<Vec<_>>(), ["other"]);
        assert_eq!(entry["license"].as_str(), Some("CC-BY-4.0"));
        assert_eq!(entry["size"].as_integer(), Some(zip.len() as i64));
        assert_eq!(entry["sha256"].as_str(), Some(want.as_str()));
        assert_eq!(entry["download"].as_str(), Some(""));
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn the_entry_leaves_out_what_mod_toml_leaves_out() {
        let info = ModInfo::parse("id = \"look\"\nversion = \"0.1.0\"\nlicense = \"CC0-1.0\"\nkind = \"visual\"\n").unwrap();
        let entry: toml_edit::DocumentMut = index_entry("look", "0.1.0", &info, 1, "00").parse().unwrap();
        assert_eq!(entry["kind"].as_str(), Some("visual"));
        assert!(["requires", "depends", "conflicts"].iter().all(|k| !entry.contains_key(k)), "{entry}");
        let full = ModInfo::parse(INFO).unwrap();
        assert!(!index_entry("zorg-race", "1.2.0", &full, 1, "00").contains("kind"));
    }

    #[test]
    fn data_only_and_the_index_fields() {
        let (dir, out) = folder(&[
            ("mod.toml", b"name = \"x\"\n"),
            ("tool.exe", b"MZ\x90\x00"),
            ("data/sneaky.pvr", b"\x7fELF...."),
            ("run.sh", b"echo"),
            ("inner.zip", b"PK\x03\x04"),
            ("art.psd", b"8BPS"),
            ("hires/fake.png", b"GIF89a"),
            ("texts.en.toml", b"[\"dataraw/text/fleet.txt\"]\n1 = \"x\"\n"),
            ("stories.toml", b"not = [toml"),
            ("LICENSE", b"CC-BY-4.0"),
        ]);
        let e = package(&dir, None, &out).unwrap_err();
        for says in [
            "version = \"1.0.0\" is needed",
            "requires = \">=0.7\" is needed",
            "license = \"CC-BY-4.0\" is needed",
            "tool.exe: a Windows program",
            "data/sneaky.pvr: a Linux program",
            "run.sh: not a kind",
            "inner.zip: an archive",
            "art.psd: not a kind",
            "hires/fake.png: not a PNG",
            "texts.en.toml: texts.en.toml: [\"dataraw/text/fleet.txt\"]: not a patchable",
            "stories.toml:",
        ] {
            assert!(e.contains(says), "{says}: {e}");
        }
        assert!(!e.contains("LICENSE"), "{e}");
        assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0, "nothing written on a failure");
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    /// With the install in `IG2_INSTALL_DIR`: the mod's rules are checked against the exe's tables, and a copy of an
    /// install file is refused.
    #[test]
    fn over_the_install() {
        let Some(root) = std::env::var_os("IG2_INSTALL_DIR").map(PathBuf::from).filter(|r| r.join("ig2.exe").is_file()) else {
            eprintln!("IG2_INSTALL_DIR not set: skipped");
            return;
        };
        let inst = ig2_formats::Install::open(&root, Lang::En).unwrap();
        let rel = "dataraw/text/s_races.txt";
        let own = std::fs::read(inst.resolve(rel).unwrap()).unwrap();
        let (dir, out) = folder(&[("mod.toml", INFO.as_bytes()), (rel, &own), ("rules.toml", b"[nosuchtable.1]\nx = 1\n")]);
        let e = package(&dir, Some(&root), &out).unwrap_err();
        assert!(e.contains(&format!("{rel}: the install's own file")), "{e}");
        assert!(e.contains("rules.toml"), "{e}");
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }
}
