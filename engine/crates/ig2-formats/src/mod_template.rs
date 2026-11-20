//! Ours (ADR 0092 §1): a new mod's files: `mod.toml` 2 with every field commented, one example file for the kind and
//! a README. Every line is ours; nothing comes from the game.

use std::path::{Path, PathBuf};

/// What the example file shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Kind {
    Rules,
    Texts,
    /// The first tutorial's kind.
    #[default]
    Story,
    Visual,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Rules, Kind::Texts, Kind::Story, Kind::Visual];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Rules => "rules",
            Kind::Texts => "texts",
            Kind::Story => "story",
            Kind::Visual => "visual",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.name() == s)
    }
}

/// A valid `id` from a folder's name: lower case, every other character `-`.
pub fn id_of(folder: &str) -> String {
    let id: String = folder.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c.to_ascii_lowercase() } else { '-' }).collect();
    if id.trim_matches('-').is_empty() { "my-mod".into() } else { id }
}

const TUTORIAL: &str = "https://github.com/openIG2-org/openIG2/blob/main/docs/modding/first-mod.md";
const GUIDE: &str = "https://github.com/openIG2-org/openIG2/blob/main/docs/USER_GUIDE.md#mods";

/// The template's files (name, text) for a mod in a folder named `folder`; `version` = this openIG2's (MAJOR.MINOR…).
pub fn files(folder: &str, kind: Kind, version: &str) -> Vec<(&'static str, String)> {
    let id = id_of(folder);
    let mm: Vec<&str> = version.split(['.', '-', '+']).take(2).collect();
    let requires = format!(">={}", if mm.len() == 2 { mm.join(".") } else { "0.7".into() });
    let visual = if kind == Kind::Visual { "kind = \"visual\"" } else { "# kind = \"visual\"" };
    let mod_toml = format!(
        "# {id}: what the Mods page shows and what openIG2 needs to load the mod (ADR 0086 §2). Every key is optional\n\
         # for a mod you keep to yourself; openIG2-check --package needs version, requires and license.\n\
         \n\
         # The mod's stable name (lower-case letters, digits, - and _): other mods' depends and conflicts use it.\n\
         id = \"{id}\"\n\
         # The name shown on the Mods page.\n\
         name = \"{id}\"\n\
         # MAJOR.MINOR.PATCH: raise it with every release you share.\n\
         version = \"0.1.0\"\n\
         # You.\n\
         author = \"\"\n\
         # One or two sentences: what the mod changes.\n\
         description = \"A new {kind} mod.\"\n\
         # The openIG2 versions the mod loads on (\">=0.7, <0.9\" for a range).\n\
         requires = \"{requires}\"\n\
         # The licence players may share the mod under (an SPDX name).\n\
         license = \"CC-BY-4.0\"\n\
         # A PNG in the mod that the Mods page shows.\n\
         # picture = \"picture.png\"\n\
         # The mods this one needs, with their versions.\n\
         # depends = {{ \"other-mod\" = \">=1.0\" }}\n\
         # The mods this one can't load beside.\n\
         # conflicts = [\"another-mod\"]\n\
         # \"visual\": the mod changes looks only, so it stays out of the multiplayer check.\n\
         {visual}\n",
        kind = kind.name(),
    );
    let mut out = vec![("mod.toml", mod_toml)];
    match kind {
        Kind::Rules => out.push((
            "rules.toml",
            "# Rule values by name (ADR 0044 §6): [<table>.<id>] then field = value. The ids and the current values are in\n\
             # the file `openIG2-check --dump-rules rules.ron` writes from your install (keep that file to yourself).\n\
             \n\
             # Building kind 4 needs less energy.\n\
             [buildings.4]\n\
             energy = 3\n"
                .into(),
        )),
        Kind::Texts => out.push((
            "texts.en.toml",
            "# Text lines by key (ADR 0044 §10), English; texts.de.toml, .fr, .es, .it and .hu for other languages. The keys\n\
             # are in the file `openIG2-check --dump-texts texts.toml` writes (keep it to yourself: copy only the keys you\n\
             # change and write your own lines).\n\
             \n\
             # An interface text: \"file section index\", as written in the file.\n\
             [\"dataraw/text/txt03.txt\"]\n\
             \"3 1 1\" = \"Mend\"\n"
                .into(),
        )),
        Kind::Story => {
            out.push((
                "stories.toml",
                "# Stories (ADR 0061): when every trigger holds, the actions run, once unless repeat = true. The user guide\n\
                 # lists every trigger and action.\n\
                 \n\
                 # Day 2: a line on the ticker and 5000 credits for the human player.\n\
                 [[story]]\n\
                 name = \"welcome-gift\"\n\
                 when = [{ day = 2 }]\n\
                 do = [{ ticker = 9100 }, { money = { empire = \"human\", amount = 5000 } }]\n"
                    .into(),
            ));
            out.push((
                "texts.en.toml",
                "# The ticker line the story shows: a new line of the story texts.\n\
                 [\"dataraw/text/s_story.txt\"]\n\
                 9100 = \"Welcome, commander: the council sends 5000 credits.\"\n"
                    .into(),
            ));
        }
        Kind::Visual => out.push((
            "visuals.toml",
            "# Looks the mod switches on while it is enabled (ADR 0088); the user guide lists every key.\n\
             \n\
             [lighting]\n\
             anti_aliasing = true\n\
             # per_pixel = true\n\
             # bloom = true\n\
             \n\
             [starmap]\n\
             # A halo round the planets: strength 0..1.\n\
             atmosphere = 0.6\n"
                .into(),
        )),
    }
    let readme = format!(
        "# {id}\n\n\
         A {kind} mod made with `openIG2-check --new-mod`. Its files:\n\n\
         - `mod.toml`: what the Mods page shows (name, version, licence); each line is explained above it.\n\
         {example}\n\
         Check it from the game's folder with `openIG2-check --quick --mod {folder}`, switch it on in the main menu →\n\
         Mods → Apply. Start the game with `--mod-dev` and a changed texture, sound, text or look file shows at once.\n\
         `openIG2-check --package {folder}` makes the zip to share.\n\n\
         - Your first mod, step by step: {TUTORIAL}\n\
         - Everything a mod can do: {GUIDE}\n\n\
         Share only your own work: no file from the game, decoded, converted or upscaled from it.\n",
        kind = kind.name(),
        example = match kind {
            Kind::Rules => "- `rules.toml`: rule values changed by name.\n",
            Kind::Texts => "- `texts.en.toml`: text lines changed or added by key.\n",
            Kind::Story => "- `stories.toml`: a story (triggers and actions); `texts.en.toml`: the line it shows.\n",
            Kind::Visual => "- `visuals.toml`: looks switched on (look-only: `kind = \"visual\"`).\n",
        },
    );
    out.push(("README.md", readme));
    out
}

/// Write the template into `folder` (made when missing); a folder that exists and isn't empty is refused. The paths
/// written.
pub fn write(folder: &Path, kind: Kind, version: &str) -> Result<Vec<PathBuf>, String> {
    let name = folder.file_name().and_then(|n| n.to_str()).ok_or_else(|| format!("{}: not a folder name", folder.display()))?;
    if std::fs::read_dir(folder).is_ok_and(|mut d| d.next().is_some()) {
        return Err(format!("{}: the folder isn't empty (nothing was written)", folder.display()));
    }
    std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    files(name, kind, version)
        .into_iter()
        .map(|(f, text)| {
            let p = folder.join(f);
            std::fs::write(&p, text).map(|_| p.clone()).map_err(|e| format!("{}: {e}", p.display()))
        })
        .collect()
}

/// The first free `my-mod`, `my-mod-2`, … in `mods`.
pub fn free_name(mods: &Path) -> PathBuf {
    (1..).map(|n| mods.join(if n == 1 { "my-mod".into() } else { format!("my-mod-{n}") })).find(|p| !p.exists() && !p.with_extension("zip").exists()).unwrap_or_else(|| mods.join("my-mod"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_parses_and_refuses_a_full_folder() {
        assert_eq!(id_of("My Mod!"), "my-mod-");
        assert_eq!(id_of("!!"), "my-mod");
        let dir = std::env::temp_dir().join(format!("openig2-template-{}", std::process::id()));
        for k in Kind::ALL {
            let f = dir.join(format!("Zorg {}", k.name()));
            let written = write(&f, k, "0.7.123").unwrap();
            assert!(written.len() >= 3);
            let info = crate::vfs::ModInfo::parse(&std::fs::read_to_string(f.join("mod.toml")).unwrap()).unwrap();
            assert_eq!(info.id.as_deref(), Some(&*format!("zorg-{}", k.name())));
            assert_eq!(info.requires.as_ref().map(|r| r.matches_text("0.7.5")), Some(true));
            assert_eq!(info.kind == crate::vfs::ModKind::Visual, k == Kind::Visual);
            for p in &written {
                let rel = p.file_name().unwrap().to_str().unwrap();
                crate::zipfs::data_only(rel, &std::fs::read(p).unwrap()).unwrap();
                if let Some(lang) = crate::textpatch::file_lang(rel) {
                    crate::textpatch::TextPatch::default().add(&std::fs::read_to_string(p).unwrap(), rel, lang).unwrap();
                }
                if rel == crate::visuals::VISUALS_FILE {
                    crate::visuals::Visuals::parse(&std::fs::read_to_string(p).unwrap(), rel).unwrap();
                }
            }
            assert!(write(&f, k, "0.7").unwrap_err().contains("isn't empty"));
        }
        assert_eq!(free_name(&dir), dir.join("my-mod"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
