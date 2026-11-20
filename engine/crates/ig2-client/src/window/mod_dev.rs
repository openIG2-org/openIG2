//! Ours (ADR 0092 §2): `--mod-dev`'s live reload. Once a second the enabled folder mods' files are compared with the
//! last look; a changed texture, sound, text, `visuals.toml` or `effects.toml` is loaded again in the running game,
//! anything else is named for Apply on the Mods page. A file that fails keeps the loaded version and says why. No
//! reload runs while a multiplayer game or room is open.

use ig2_formats::mod_watch::{classify, Reload, Stamps};
use ig2_formats::vfs::Mod;

use super::App;

/// How often the mods' files are looked at.
const EVERY_MS: u32 = 1000;

pub(super) struct ModDev {
    stamps: Stamps,
    next: u32,
}

impl ModDev {
    pub(super) fn new(mods: &[Mod]) -> Self {
        Self { stamps: Stamps::scan(mods), next: 0 }
    }
}

impl App {
    /// The once-a-second look; changes made in multiplayer wait until it is over.
    pub(super) fn mod_dev_frame(&mut self) {
        let now = self.now_ms();
        let Some(w) = self.mod_dev.as_mut().filter(|w| now >= w.next) else { return };
        w.next = now + EVERY_MS;
        if self.world.net.is_some() || self.front.as_ref().is_some_and(|f| f.mp.is_some() || f.session.is_some()) {
            return;
        }
        let stamps = Stamps::scan(self.install.mods());
        let changed = stamps.changed(&w.stamps);
        w.stamps = stamps;
        if !changed.is_empty() {
            self.mod_reload(&changed);
        }
    }

    /// Load `changed` (mod, path inside it) again where it is look-only data.
    fn mod_reload(&mut self, changed: &[(String, String)]) {
        let (mut textures, mut sounds, mut texts, mut looks) = (Vec::new(), false, false, false);
        for (m, rel) in changed {
            match classify(rel) {
                Reload::Texture(p) => textures.push(p),
                Reload::Sound => sounds = true,
                Reload::Texts => texts = true,
                Reload::Looks => looks = true,
                Reload::Nothing => {}
                Reload::Apply => {
                    let what = format!("{m}: {rel}");
                    self.mod_say(&format!("{what} changed: main menu, Mods, Apply reloads it"));
                    crate::mods_page::mark_stale(what);
                }
            }
        }
        if textures.is_empty() && !sounds && !texts && !looks {
            return;
        }
        // The install again: a new file is indexed, the text patches apply as they load.
        let new = match ig2_formats::Install::open_with_mods(self.install.root(), self.install.lang(), self.install.mods().to_vec()) {
            Ok(i) => i,
            Err(e) => return self.mod_problem(e.to_string()),
        };
        let (mut problems, mut replaced) = (Vec::new(), 0);
        if let Some(g) = &mut self.gpu {
            for p in &textures {
                crate::upscale::forget(p);
                match g.renderer.reload_file(&new, p) {
                    Ok(n) => replaced += n,
                    Err(e) => problems.push(e),
                }
            }
        }
        if sounds {
            self.audio.reload(new.clone());
        }
        if texts {
            if let Some(h) = &mut self.hud {
                h.texts = crate::ui::UiTexts::load(&new);
            }
        }
        if looks {
            match look_files(&new) {
                Ok(()) => crate::visuals::load(&new),
                Err(e) => problems.push(e),
            }
        }
        self.install = new;
        eprintln!("ig2-client: mod-dev: reloaded {replaced} textures{}{}{}", if sounds { ", sounds" } else { "" }, if texts { ", texts" } else { "" }, if looks { ", looks" } else { "" });
        match problems.is_empty() {
            true => crate::mods_page::set_live_problem(None),
            false => self.mod_problem(problems.join("; ")),
        }
    }

    fn mod_problem(&mut self, e: String) {
        self.mod_say(&format!("live reload: {e}"));
        crate::mods_page::set_live_problem(Some(e));
    }

    /// A line on the game's message line (only in a game: the menu's Mods page shows its own), and the log.
    fn mod_say(&mut self, text: &str) {
        eprintln!("ig2-client: {text}");
        if self.front.is_none() {
            self.world.local_lines.push(self.install.lang().encoding().encode(text).0.into_owned());
        }
    }
}

/// Every mod's `visuals.toml` and `effects.toml` reads: else the loaded look stays (one file's error would drop it).
fn look_files(install: &ig2_formats::Install) -> Result<(), String> {
    use ig2_formats::{effects, visuals};
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|_| "not UTF-8 text".to_string());
    for (m, b) in install.visual_files().unwrap_or_default() {
        text(b).and_then(|t| visuals::Visuals::parse(&t, &format!("mod {m}: {}", visuals::VISUALS_FILE)).map(drop))?;
    }
    for (m, b) in install.effect_files().unwrap_or_default() {
        text(b).and_then(|t| effects::Effects::parse(&t, &format!("mod {m}: {}", effects::EFFECTS_FILE)).map(drop))?;
    }
    Ok(())
}
