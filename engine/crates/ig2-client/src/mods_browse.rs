//! Ours (ADR 0086 §1.2): the Mods page's Browse tab. The mod repository's index ([`crate::mod_repo`]) is fetched on a
//! thread the first time the tab opens (never at start; the page says so), then listed: the mods compatible with this
//! openIG2, a search over name, id, author and description, each with its picture, author, licence, size and
//! dependencies, and Install, Update or Remove. Installs run on a thread; the page reloads its Installed list after.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

use ig2_formats::vfs::{Mod, Version};

use crate::mod_repo::{self, Entry, Index, Present, Source};

const PICTURE: [f32; 2] = [112.0, 72.0];
/// The largest picture shown (either side, pixels).
const MAX_PICTURE: u32 = 2048;
/// The longest search.
const SEARCH_LEN: usize = 60;

fn tr(en: &'static str) -> &'static str {
    crate::tr::text(crate::tr::game(), en)
}

/// The index's build time, `2026-11-08 15:29 UTC`.
fn when(generated: &str) -> String {
    let t = generated.trim_end_matches('Z').replacen('T', " ", 1);
    format!("{} UTC", t.get(..16).unwrap_or(&t))
}

/// `f` on a thread; its result on the receiver. The browser build has no threads (a spawn panics): `f` runs at once
/// there (its downloads fail at once, [`mod_repo::get`]).
fn spawn<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (tx, rx) = channel();
    #[cfg(target_arch = "wasm32")]
    let _ = tx.send(f());
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx
}

enum Picture {
    Loading(Receiver<Result<Vec<u8>, String>>),
    Ready(Option<egui::TextureHandle>),
}

/// What a card's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Do {
    Install,
    Update,
    Remove,
}

#[derive(Default)]
pub struct Browse {
    loading: Option<Receiver<Result<(Index, Source), String>>>,
    index: Option<(Index, Source)>,
    /// Why the index didn't load (none cached either).
    error: Option<String>,
    pub search: String,
    /// The search field takes the keys.
    pub typing: bool,
    have: Vec<Present>,
    /// The install running (what, its result).
    job: Option<(String, Receiver<Result<String, String>>)>,
    /// The last install's or removal's outcome.
    pub status: Option<Result<String, String>>,
    pictures: HashMap<String, Picture>,
    /// The keyboard's card, and whether it moved this frame.
    focus: Option<usize>,
    scroll: bool,
    /// An install or removal changed the mods folder: the Installed list reloads.
    pub changed: bool,
    /// The Installed tab has changes not applied (a reload would drop them): no install or removal.
    pub locked: bool,
}

/// The mods folder installs go to: beside the game, else beside the settings.
fn install_dir() -> Option<PathBuf> {
    Mod::exe_folder().or_else(Mod::folder)
}

impl Browse {
    /// Fetch the index the first time the tab shows (or again, `again`).
    pub fn start(&mut self, again: bool) {
        if self.loading.is_some() || (!again && (self.index.is_some() || self.error.is_some())) {
            return;
        }
        self.error = None;
        self.have = mod_repo::present(&Mod::folders());
        self.loading = Some(spawn(mod_repo::load));
    }

    pub fn busy(&self) -> bool {
        self.loading.is_some() || self.job.is_some()
    }

    /// The mods listed: compatible with this version, matching the search.
    fn listed(&self) -> Vec<&Entry> {
        let Some((index, _)) = &self.index else { return Vec::new() };
        let q = self.search.trim().to_lowercase();
        let hit = |e: &&Entry| q.is_empty() || [&e.name, &e.id, &e.author, &e.description].iter().any(|s| s.to_lowercase().contains(&q));
        index.compatible(crate::version()).into_iter().filter(hit).collect()
    }

    /// The buttons a release has: what is installed decides.
    fn actions(&self, e: &Entry) -> (Option<Do>, bool, Option<&'static str>) {
        match self.have.iter().find(|p| p.id == e.id) {
            None => (Some(Do::Install), false, None),
            Some(p) if !p.m.is_zip() => (None, false, Some(tr("Installed by hand (a folder)"))),
            Some(p) if p.version.as_deref().and_then(Version::parse) < Version::parse(&e.version) => (Some(Do::Update), true, None),
            Some(_) => (None, true, Some(tr("Installed"))),
        }
    }

    /// No card focused (a click elsewhere), no typing (the tab left).
    pub fn unfocus(&mut self, typing: bool) {
        self.focus = None;
        self.typing &= typing;
    }

    fn run(&mut self, e: &Entry, what: Do) {
        if self.locked {
            self.status = Some(Err(tr("Apply or take back the changed list on Installed first.").into()));
            return;
        }
        match what {
            Do::Remove => {
                self.status = Some(remove(&e.id).map(|n| format!("{} {n}.", tr("Removed"))));
                self.have = mod_repo::present(&Mod::folders());
                self.changed = true;
            }
            Do::Install | Do::Update => {
                let (Some((index, _)), Some(dir)) = (&self.index, install_dir()) else {
                    self.status = Some(Err(tr("No mods folder here.").into()));
                    return;
                };
                let (index, e) = (index.clone(), e.clone());
                let label = format!("{} {} {}...", if what == Do::Update { tr("Updating") } else { tr("Installing") }, e.name, e.version);
                let rx = spawn(move || {
                    let done = mod_repo::install(&index, &e.id, &e.version, &e.sha256, &dir, &Mod::folders(), &mod_repo::get)?;
                    let names: Vec<String> = done.iter().map(|i| format!("{} {}", i.id, i.version)).collect();
                    Ok(format!("{} {}.", tr("Installed"), names.join(", ")))
                });
                self.job = Some((label, rx));
                self.status = None;
            }
        }
    }

    /// Pick up the threads' results.
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.loading {
            if let Ok(r) = rx.try_recv() {
                self.loading = None;
                match r {
                    Ok(i) => self.index = Some(i),
                    Err(e) => self.error = Some(e),
                }
            }
        }
        if let Some((_, rx)) = &self.job {
            if let Ok(r) = rx.try_recv() {
                self.job = None;
                self.status = Some(r);
                self.have = mod_repo::present(&Mod::folders());
                self.changed = true;
            }
        }
        if self.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn picture(&mut self, ctx: &egui::Context, url: &str) -> Option<egui::TextureHandle> {
        let p = self.pictures.entry(url.to_owned()).or_insert_with(|| {
            let u = url.to_owned();
            Picture::Loading(spawn(move || mod_repo::get_picture(&u)))
        });
        if let Picture::Loading(rx) = p {
            match rx.try_recv() {
                Ok(r) => {
                    let img = r.and_then(|b| decode_picture(&b)).map_err(|e| eprintln!("ig2-client: mod picture {url}: {e}")).ok();
                    let tex = img.filter(|(w, h, _)| *w <= MAX_PICTURE && *h <= MAX_PICTURE).map(|(w, h, rgba)| {
                        ctx.load_texture(format!("mod-pic-{url}"), egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba), egui::TextureOptions::LINEAR)
                    });
                    *p = Picture::Ready(tex);
                }
                Err(_) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    return None;
                }
            }
        }
        match p {
            Picture::Ready(t) => t.clone(),
            Picture::Loading(_) => None,
        }
    }

    /// The keys while the tab shows: typing into the search while it takes them (Enter, Esc or Tab end it);
    /// Up/Down (Tab, Shift+Tab) the focused card, Enter or Space its Install or Update, Delete its Remove; `/` the
    /// search; F5 the list again. `None` when the key is the page's (Esc).
    pub fn key(&mut self, key: &winit::keyboard::Key, shift: bool, chord: bool) -> Option<bool> {
        use winit::keyboard::{Key, NamedKey};
        if self.typing {
            match key.as_ref() {
                Key::Named(NamedKey::Escape | NamedKey::Enter | NamedKey::Tab) => self.typing = false,
                Key::Named(NamedKey::Backspace) => _ = self.search.pop(),
                _ if chord => {}
                Key::Named(NamedKey::Space) => self.type_text(" "),
                Key::Character(c) => self.type_text(c),
                _ => return Some(false),
            }
            self.focus = None;
            return Some(false);
        }
        let n = self.listed().len();
        let at = self.focus.filter(|&i| i < n);
        let step = |by: isize| at.map_or(if by > 0 { 0 } else { n.saturating_sub(1) }, |i| (i as isize + by).clamp(0, n.saturating_sub(1) as isize) as usize);
        let mut moved = |i: usize| {
            (self.focus, self.scroll) = (Some(i), true);
            Some(false)
        };
        match key.as_ref() {
            Key::Named(NamedKey::Escape) => None,
            Key::Character("/") => {
                self.typing = true;
                Some(true)
            }
            Key::Named(NamedKey::F5) => {
                let idle = self.loading.is_none();
                self.start(true);
                Some(idle)
            }
            _ if n == 0 => Some(false),
            Key::Named(NamedKey::ArrowUp) => moved(step(-1)),
            Key::Named(NamedKey::Tab) if shift => moved(step(-1)),
            Key::Named(NamedKey::ArrowDown | NamedKey::Tab) => moved(step(1)),
            Key::Named(NamedKey::Home) => moved(0),
            Key::Named(NamedKey::End) => moved(n - 1),
            Key::Named(NamedKey::Enter | NamedKey::Space | NamedKey::Delete) if self.job.is_none() => {
                // Nothing to do on the card: the key stays here (a None would be the page's Back).
                let Some(i) = at else { return Some(false) };
                let e = self.listed()[i].clone();
                let (main, removable, _) = self.actions(&e);
                let what = if key == &Key::Named(NamedKey::Delete) { removable.then_some(Do::Remove) } else { main };
                let Some(what) = what else { return Some(false) };
                self.run(&e, what);
                Some(true)
            }
            _ => Some(false),
        }
    }

    fn type_text(&mut self, text: &str) {
        for c in text.chars().filter(|c| !c.is_control()) {
            if self.search.chars().count() < SEARCH_LEN {
                self.search.push(c);
            }
        }
    }

    /// The tab's body in `height`; whether a button was clicked.
    pub fn show(&mut self, ui: &mut egui::Ui, height: f32) -> bool {
        self.poll(ui.ctx());
        let mut clicked = false;
        let bad = egui::Color32::from_rgb(235, 110, 100);
        let note = self.note();
        ui.horizontal(|ui| {
            let caret = self.typing.then(|| self.search.chars().count());
            let resp = crate::gui::ds::text_field(ui, &self.search, caret, tr("Search mods"), egui::vec2(300.0, 34.0)).on_hover_cursor(egui::CursorIcon::Text);
            if resp.clicked() {
                (self.typing, self.focus) = (true, None);
            } else if ui.input(|i| i.pointer.any_pressed()) && !resp.hovered() {
                self.typing = false;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(self.loading.is_none(), egui::Button::new(tr("Refresh"))).on_hover_text(tr("Fetch the mod list again (F5).")).clicked() {
                    self.start(true);
                    clicked = true;
                }
            });
        });
        ui.add(egui::Label::new(egui::RichText::new(&note).small().weak()).truncate()).on_hover_text(note);
        ui.add_space(4.0);
        let list = self.listed().into_iter().cloned().collect::<Vec<_>>();
        let mut act = None;
        egui::ScrollArea::vertical().id_salt("mods-browse").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).max_height(height - 40.0).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            let empty = match (&self.index, &self.error) {
                _ if self.loading.is_some() => Some((tr("Fetching the mod list...").to_owned(), false)),
                (None, Some(e)) => Some((format!("{} {e}", tr("The mod list didn't load:")), true)),
                (Some(_), _) if list.is_empty() && !self.search.trim().is_empty() => Some((tr("No mod matches the search.").to_owned(), false)),
                (Some(_), _) if list.is_empty() => Some((format!("{} {}.", tr("No mods in the list yet for openIG2"), crate::version()), false)),
                _ => None,
            };
            if let Some((text, is_bad)) = empty {
                crate::gui::card(ui, None, |ui| {
                    let t = egui::RichText::new(text);
                    ui.add(egui::Label::new(if is_bad { t.color(bad) } else { t }).wrap());
                });
            }
            for (i, e) in list.iter().enumerate() {
                let pic = e.picture.as_deref().and_then(|u| self.picture(ui.ctx(), u));
                let (main, removable, note) = self.actions(e);
                let installed = self.have.iter().find(|p| p.id == e.id).and_then(|p| p.version.clone());
                let busy = self.job.is_some();
                let card = ui.scope(|ui| crate::gui::card(ui, None, |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::Vec2::from(PICTURE), egui::Sense::hover());
                        match &pic {
                            Some(t) => {
                                egui::Image::new(t).corner_radius(6.0).paint_at(ui, rect);
                            }
                            None => {
                                ui.painter().rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
                                let letter = e.name.chars().next().unwrap_or('?').to_uppercase().to_string();
                                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(30.0), ui.visuals().weak_text_color());
                            }
                        }
                        ui.add_space(8.0);
                        let text_w = ui.available_width() - 170.0;
                        ui.vertical(|ui| {
                            ui.set_width(text_w);
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&e.name).size(17.0).strong());
                                ui.label(egui::RichText::new(&e.version).weak());
                                if e.kind.as_deref() == Some("visual") {
                                    ui.label(egui::RichText::new(tr("look only")).small().weak()).on_hover_text(tr("Changes only how the game looks: multiplayer rooms don't need it."));
                                }
                                if let Some(v) = installed.as_ref().filter(|v| **v != e.version) {
                                    ui.label(egui::RichText::new(format!("{} {v}", tr("installed:"))).small().color(ui.visuals().hyperlink_color));
                                }
                            });
                            ui.label(egui::RichText::new(format!("{} {}", tr("by"), e.author)).small().weak());
                            ui.add(egui::Label::new(&e.description).wrap());
                            if !e.depends.is_empty() {
                                let deps: Vec<String> = e.depends.iter().map(|(d, v)| if v.trim() == "*" { d.clone() } else { format!("{d} {v}") }).collect();
                                ui.label(egui::RichText::new(format!("{} {} ({})", tr("Needs:"), deps.join(", "), tr("installed with it"))).small());
                            }
                            if !e.conflicts.is_empty() {
                                ui.label(egui::RichText::new(format!("{} {}", tr("Can't load with:"), e.conflicts.join(", "))).small());
                            }
                            ui.label(egui::RichText::new(format!("{} · {} · {}", e.license, size_text(e.size), e.id)).small().weak());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if removable && ui.add_enabled(!busy, egui::Button::new(tr("Remove"))).on_hover_text(tr("Delete its zip from the mods folder.")).clicked() {
                                act = Some((i, Do::Remove));
                            }
                            match main {
                                Some(Do::Update) => {
                                    if crate::gui::primary(ui, !busy, tr("Update")).on_hover_text(tr("Download this release; the installed zip stays until the new one is checked.")).clicked() {
                                        act = Some((i, Do::Update));
                                    }
                                }
                                Some(_) => {
                                    if crate::gui::primary(ui, !busy, tr("Install")).on_hover_text(tr("Download, check and put it into the mods folder, with the mods it needs.")).clicked() {
                                        act = Some((i, Do::Install));
                                    }
                                }
                                None => {
                                    ui.label(egui::RichText::new(note.unwrap_or_default()).small().weak());
                                }
                            }
                        });
                    });
                })).response;
                if self.focus == Some(i) {
                    crate::gui::focus_ring(ui, card.rect);
                    if self.scroll {
                        card.scroll_to_me(None);
                    }
                }
            }
        });
        self.scroll = false;
        if let Some((i, what)) = act {
            self.focus = None;
            self.run(&list[i], what);
            clicked = true;
        }
        clicked
    }

    /// Where the list comes from.
    fn note(&self) -> String {
        match &self.index {
            _ if self.loading.is_some() => tr("Fetching the mod list from github.com/openIG2-org/mods...").to_owned(),
            Some((i, Source::Fresh)) => format!("{} {}", tr("Signed mod list from github.com/openIG2-org/mods:"), when(&i.generated)),
            Some((i, Source::Cached(e))) => format!("{} {} ({e})", tr("Offline, the saved mod list:"), when(&i.generated)),
            None => tr("Browse fetches the mod list from github.com/openIG2-org/mods (only when this tab opens).").to_owned(),
        }
    }

    /// The status line: the install running, else the last outcome.
    pub fn status_line(&self) -> Option<Result<String, String>> {
        self.job.as_ref().map(|(what, _)| Ok(what.clone())).or_else(|| self.status.clone())
    }
}

/// Remove mod `id`'s zip and its name from `enabled.txt`.
fn remove(id: &str) -> Result<String, String> {
    let name = mod_repo::remove(&Mod::folders(), id)?;
    let names = Mod::enabled_names();
    if names.contains(&name) {
        let left: Vec<String> = names.into_iter().filter(|n| *n != name).collect();
        if let Some(f) = Mod::enabled_file() {
            ig2_formats::userfs::write(&f, Mod::enabled_text(&left)).map_err(|e| format!("{}: {e}", f.display()))?;
        }
    }
    Ok(name)
}

/// "1.2 MB", "340 KB".
fn size_text(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1u64 << 20) as f64),
        b => format!("{} KB", b.div_ceil(1024)),
    }
}

/// A PNG or JPEG: width, height, RGBA.
fn decode_picture(b: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if b.starts_with(b"\x89PNG") {
        let img = crate::upscale::decode_png(b)?;
        return Ok((img.width, img.height, img.rgba));
    }
    // The browser build has no mod repository (and no JPEG decoder).
    #[cfg(target_arch = "wasm32")]
    {
        Err("not a PNG".into())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut d = zune_jpeg::JpegDecoder::new(b);
        let px = d.decode().map_err(|e| format!("not a PNG or JPEG: {e:?}"))?;
        let (w, h) = d.dimensions().ok_or("no picture size")?;
        let rgba = match px.len() / (w * h).max(1) {
            4 => px,
            3 => px.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            1 => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            _ => return Err("an unusual JPEG".into()),
        };
        Ok((w as u32, h as u32, rgba))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_search_keys() {
        assert_eq!(size_text(340 * 1024), "340 KB");
        assert_eq!(size_text(3 << 20), "3.0 MB");
        assert_eq!(when("2026-11-08T15:29:07Z"), "2026-11-08 15:29 UTC");
        let mut b = Browse::default();
        use winit::keyboard::{Key, NamedKey, SmolStr};
        assert_eq!(b.key(&Key::Character(SmolStr::new("/")), false, false), Some(true));
        assert!(b.typing);
        b.key(&Key::Character(SmolStr::new("ab")), false, false);
        b.key(&Key::Named(NamedKey::Space), false, false);
        b.key(&Key::Named(NamedKey::Backspace), false, false);
        assert_eq!(b.search, "ab");
        // Esc ends the typing first, then is the page's.
        b.key(&Key::Named(NamedKey::Escape), false, false);
        assert!(!b.typing);
        assert_eq!(b.key(&Key::Named(NamedKey::Escape), false, false), None);
        // Space, Delete or Enter with no card to act on stay on the page.
        for k in [NamedKey::Space, NamedKey::Delete, NamedKey::Enter] {
            assert_eq!(b.key(&Key::Named(k), false, false), Some(false));
        }
        b.typing = true;
        b.unfocus(false);
        assert!(!b.typing);
    }
}
