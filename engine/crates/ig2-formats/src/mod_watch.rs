//! Ours (ADR 0092 §2): which files of the enabled folder mods changed since the last look (modification time and
//! size, polled), and what a change needs: a live reload of look-only data, or Apply on the Mods page.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::SystemTime;

use crate::vfs::{Mod, MOD_FILE, RULES_PATCH, STORY_FILE};

/// The most files looked at per mod (a mod folder is small; this keeps a wrong folder from costing much).
const MAX_FILES: usize = 20_000;

/// What a changed file needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reload {
    /// A texture at this game path (lower case; a `hires/` picture names its original).
    Texture(String),
    Sound,
    /// A `texts.<lang>.toml` or a text file: the install opens again.
    Texts,
    /// `visuals.toml` or `effects.toml`.
    Looks,
    /// Read when it plays: nothing to do.
    Nothing,
    /// Rule tables, stories, `mod.toml`, the game's data, models: only Apply loads it.
    Apply,
}

/// What `rel` (a path inside a mod, `/`-separated) needs when it changes.
pub fn classify(rel: &str) -> Reload {
    let rel = rel.replace('\\', "/").to_ascii_lowercase();
    if [MOD_FILE, RULES_PATCH, STORY_FILE].contains(&rel.as_str()) {
        return Reload::Apply;
    }
    if rel == crate::visuals::VISUALS_FILE || rel == crate::effects::EFFECTS_FILE {
        return Reload::Looks;
    }
    if !rel.contains('/') && crate::textpatch::file_lang(&rel).is_some() {
        return Reload::Texts;
    }
    if let Some(game) = rel.strip_prefix("hires/") {
        // A pack's picture is `hires/<game path>` with `.png` for its extension, or the `.pvr` as it is (ADR 0054 §5);
        // a normal map or a talking head's frame is read when its screen opens.
        return match game.rsplit_once('.') {
            Some((_, "mp4")) => Reload::Nothing,
            Some((_, "pvr")) => Reload::Texture(game.to_owned()),
            Some((stem, "png")) if !stem.ends_with("_n") && !stem.starts_with("anim/") => Reload::Texture(format!("{stem}.pvr")),
            _ => Reload::Apply,
        };
    }
    match rel.rsplit_once('.').map(|(_, e)| e) {
        Some("pvr") => Reload::Texture(rel),
        Some("ogg") => Reload::Sound,
        Some("mp4") => Reload::Nothing,
        Some("txt" | "eng") if crate::textpatch::kind(&rel).is_some() => Reload::Texts,
        _ => Reload::Apply,
    }
}

/// Each enabled folder mod's files: (mod, path inside it) → (modified, size).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stamps(BTreeMap<(String, String), (Option<SystemTime>, u64)>);

impl Stamps {
    /// The files of `mods` that are folders (a zip is a finished mod); dot files and folders left out.
    pub fn scan(mods: &[Mod]) -> Self {
        let mut out = BTreeMap::new();
        for m in mods.iter().filter(|m| m.dir.is_dir()) {
            let mut todo = vec![m.dir.clone()];
            let mut n = 0;
            while let Some(dir) = todo.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else { continue };
                for e in entries.flatten() {
                    if e.file_name().to_string_lossy().starts_with('.') || n >= MAX_FILES {
                        continue;
                    }
                    let Ok(meta) = e.metadata() else { continue };
                    if meta.is_dir() {
                        todo.push(e.path());
                    } else {
                        n += 1;
                        let rel = rel_of(&m.dir, &e.path());
                        out.insert((m.name.clone(), rel), (meta.modified().ok(), meta.len()));
                    }
                }
            }
        }
        Self(out)
    }

    /// The files changed, added or removed since `old`: (mod, path inside it).
    pub fn changed(&self, old: &Stamps) -> Vec<(String, String)> {
        let mut v: Vec<_> = self.0.iter().filter(|(k, s)| old.0.get(*k) != Some(*s)).map(|(k, _)| k.clone()).collect();
        v.extend(old.0.keys().filter(|k| !self.0.contains_key(*k)).cloned());
        v.sort();
        v
    }
}

fn rel_of(dir: &Path, p: &Path) -> String {
    p.strip_prefix(dir).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_by_kind() {
        assert_eq!(classify("dataraw/textures/main/a_bmp.pvr"), Reload::Texture("dataraw/textures/main/a_bmp.pvr".into()));
        assert_eq!(classify("hires/dataraw/textures/main/a_bmp.png"), Reload::Texture("dataraw/textures/main/a_bmp.pvr".into()));
        assert_eq!(classify("dataraw/sounds/colony/click.ogg"), Reload::Sound);
        assert_eq!(classify("texts.de.toml"), Reload::Texts);
        assert_eq!(classify("dataraw/text/txt03.txt"), Reload::Texts);
        assert_eq!(classify("visuals.toml"), Reload::Looks);
        assert_eq!(classify("effects.toml"), Reload::Looks);
        assert_eq!(classify("hires/video/intro.mp4"), Reload::Nothing);
        for apply in ["rules.toml", "stories.toml", "mod.toml", "dataraw/building.txt", "dataraw/vectors/a.xvb", "x/texts.en.toml", "hires/dataraw/textures/main/a_bmp_n.png"] {
            assert_eq!(classify(apply), Reload::Apply, "{apply}");
        }
    }

    #[test]
    fn a_change_an_addition_and_a_removal_are_seen() {
        let dir = std::env::temp_dir().join(format!("openig2-watch-{}", std::process::id()));
        let m = Mod { name: "m".into(), dir: dir.clone() };
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.toml"), "x").unwrap();
        std::fs::write(dir.join("sub/b.pvr"), "y").unwrap();
        std::fs::write(dir.join(".hidden"), "z").unwrap();
        let s0 = Stamps::scan(std::slice::from_ref(&m));
        assert_eq!(s0.0.len(), 2);
        assert!(Stamps::scan(std::slice::from_ref(&m)).changed(&s0).is_empty());
        std::fs::write(dir.join("sub/b.pvr"), "yy").unwrap(); // a new size, whatever the clock's resolution
        std::fs::write(dir.join("c.ogg"), "c").unwrap();
        std::fs::remove_file(dir.join("a.toml")).unwrap();
        let s1 = Stamps::scan(std::slice::from_ref(&m));
        let k = |r: &str| ("m".to_string(), r.to_string());
        assert_eq!(s1.changed(&s0), [k("a.toml"), k("c.ogg"), k("sub/b.pvr")]);
        // A zip mod isn't watched.
        assert!(Stamps::scan(&[Mod { name: "z".into(), dir: dir.join("c.ogg") }]).0.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
