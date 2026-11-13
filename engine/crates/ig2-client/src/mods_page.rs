//! Ours (ADR 0068 §5): the mod manager, an egui page (ADR 0062) over the main menu in the Options page's style: every
//! mod in the mods folders with its `mod.toml`, a switch and a priority, the checks `ig2-check` runs on a mod, the
//! files two enabled mods both have; Apply writes `enabled.txt` and loads it in place (no restart). `mod.toml` 2 (ADR 0086
//! §2): a mod's dependencies sit under it and switch on and off with it, a mod that can't load says why. The Browse
//! tab (ADR 0086 §1.2, [`crate::mods_browse`]) lists, installs, updates and removes the mod repository's mods.

use std::collections::HashMap;

use ig2_formats::vfs::{Mod, ModInfo};
use ig2_formats::Install;

/// The page's size in points (the Options page's).
const PAGE: [f32; 2] = [860.0, 500.0];
const PICTURE: [f32; 2] = [112.0, 72.0];
/// The largest picture shown (either side, pixels).
const MAX_PICTURE: u32 = 2048;
/// The first modding tutorial (ADR 0086 §4) on the repository's page.
const HOW_TO: &str = "https://github.com/openIG2-org/openIG2/blob/main/docs/modding/first-mod.md";

/// `--mod` chose this run's mods (set when the install opens).
pub static GIVEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// This run's mods for the multiplayer room (ADR 0068 §6): "name version", highest priority first.
static LOADED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Remember `install`'s mods for [`loaded`].
pub fn set_loaded(install: &Install) {
    let names = install.mods().iter().map(|m| {
        let info = m.info().unwrap_or_default();
        let name = info.name.unwrap_or_else(|| m.name.clone());
        info.version.map_or(name.clone(), |v| format!("{name} {v}"))
    });
    *LOADED.lock().unwrap_or_else(|e| e.into_inner()) = names.collect();
}

/// The mods this run loaded, as [`set_loaded`] named them.
pub fn loaded() -> Vec<String> {
    LOADED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Why the enabled mods didn't load this run (the game started without them).
static FAILED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Why the enabled mods didn't load (`None`: they did).
pub fn set_failed(e: Option<&str>) {
    *FAILED.lock().unwrap_or_else(|e| e.into_inner()) = e.map(str::to_owned);
}

/// What a frame of the page asks of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Back,
    /// Start the game again (the update's new version).
    Restart,
    /// Load the written list in place (the game's start again on the same window, no restart).
    Swap,
    Click,
    /// The mods folder changed (an install or removal): open the page again over it, the tab kept.
    Reload,
}

struct Row {
    m: Mod,
    info: Result<ModInfo, String>,
    /// [`ModInfo::id_or`].
    id: String,
    /// Why it can't load beside the enabled mods (an enabled row), or why it can't be switched on (a disabled one).
    refused: Option<String>,
    on: bool,
    /// What the mod's checks found wrong.
    problems: Vec<String>,
    /// The game paths the mod provides.
    files: Vec<String>,
    /// The mod can load at all (a zip's directory reads).
    loads: bool,
    /// Its files another enabled mod above it provides too (that mod, how many).
    lost: Vec<(String, usize)>,
    /// What a broken `mod.toml` counts as.
    fallback: ModInfo,
}

impl Row {
    fn new(m: Mod, info: Result<ModInfo, String>, on: bool, problems: Vec<String>, files: Vec<String>, loads: bool) -> Self {
        let id = info.as_ref().map_or_else(|_| m.name.to_lowercase(), |i| i.id_or(&m.name));
        Self { m, info, id, refused: None, fallback: ModInfo::default(), on, problems, files, loads, lost: Vec::new() }
    }
}

pub struct ModsPage {
    rows: Vec<Row>,
    /// `enabled.txt`'s names as last written.
    saved: Vec<String>,
    /// The mods this run loaded.
    loaded: Vec<String>,
    /// `--mod` chose this run's mods.
    given: bool,
    /// Why the enabled mods didn't load this run.
    failed: Option<String>,
    pictures: HashMap<String, Option<egui::TextureHandle>>,
    error: Option<String>,
    /// The keyboard's row (ADR 0053's focus), and whether it moved this frame (the list scrolls to it).
    focus: Option<usize>,
    scroll: bool,
    /// The Browse tab shows (else the installed mods).
    browse_tab: bool,
    pub browse: crate::mods_browse::Browse,
}

fn tr(en: &'static str) -> &'static str {
    crate::tr::text(crate::tr::game(), en)
}

/// What a mod's checks find (ADR 0044 §8, §12; ADR 0061 §7), the mod alone over the install, and the paths it provides.
fn check(install: &Install, rules: &Result<ig2_rules::Rules, String>, m: &Mod) -> (Vec<String>, Vec<String>) {
    let solo = match Install::open_with_mods(install.root(), install.lang(), vec![m.clone()]) {
        Ok(i) => i,
        Err(e) => return (vec![e.to_string()], Vec::new()),
    };
    let files = solo.overrides().iter().map(|o| o.path.to_owned()).collect();
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|_| "not UTF-8 text".to_string());
    let mut problems = Vec::new();
    for (_, b) in solo.rule_patches().unwrap_or_default() {
        let applied = rules.clone().and_then(|mut r| r.apply_patch(&text(b)?));
        problems.extend(applied.err().map(|e| format!("{}: {e}", ig2_formats::vfs::RULES_PATCH)));
    }
    for (_, file, b) in solo.text_patches().unwrap_or_default() {
        let lang = ig2_formats::textpatch::file_lang(&file).unwrap_or(install.lang());
        let checked = text(b).and_then(|t| ig2_formats::textpatch::TextPatch::default().add(&t, &file, lang));
        problems.extend(checked.err().map(|e| format!("{file}: {e}")));
    }
    for (name, b) in solo.story_files().unwrap_or_default() {
        let checked = text(b).and_then(|t| ig2_sim::story_mods::parse(&t, name));
        problems.extend(checked.err().map(|e| format!("{}: {e}", ig2_formats::vfs::STORY_FILE)));
    }
    for (_, b) in solo.visual_files().unwrap_or_default() {
        let checked = text(b).and_then(|t| ig2_formats::visuals::Visuals::parse(&t, ig2_formats::visuals::VISUALS_FILE));
        problems.extend(checked.err());
    }
    for (_, b) in solo.effect_files().unwrap_or_default() {
        let checked = text(b).and_then(|t| ig2_formats::effects::Effects::parse(&t, ig2_formats::effects::EFFECTS_FILE));
        problems.extend(checked.err());
    }
    (problems, files)
}

impl ModsPage {
    /// Every mod in the mods folders, the enabled ones first in their order, each checked over `install`.
    pub fn open(install: &Install) -> Self {
        let failed = FAILED.lock().unwrap_or_else(|e| e.into_inner()).clone();
        // The game started without the mods: `--mod` isn't what chose that.
        let given = GIVEN.load(std::sync::atomic::Ordering::Relaxed) && failed.is_none();
        let saved = Mod::prune_enabled();
        let mut all = Mod::available();
        let mut mods: Vec<Mod> = saved.iter().filter_map(|n| all.iter().position(|m| &m.name == n).map(|i| all.remove(i))).collect();
        let first_off = mods.len();
        mods.extend(all);
        let rules = install.read_raw("ig2.exe").map_err(|e| e.to_string()).and_then(|exe| ig2_rules::Rules::from_exe(&exe).map_err(|e| e.to_string()));
        // shortcut: each mod is checked on the frame the page opens (the install's index once per mod); a thread if
        // players keep dozens of mods.
        let rows = mods
            .into_iter()
            .enumerate()
            .map(|(i, m)| {
                let on = i < first_off;
                match m.check() {
                    Ok(()) => {
                        let (problems, files) = check(install, &rules, &m);
                        let info = m.info();
                        Row::new(m, info, on, problems, files, true)
                    }
                    Err(e) => Row::new(m, Ok(ModInfo::default()), on, vec![e], Vec::new(), false),
                }
            })
            .collect();
        let mut page = Self { rows, saved, loaded: install.mods().iter().map(|m| m.name.clone()).collect(), given, failed, pictures: HashMap::new(), error: None, focus: None, scroll: false, browse_tab: false, browse: Default::default() };
        page.refresh();
        page
    }

    /// The page opened again over `install` after the mods folder changed, `old`'s tab and Browse state kept.
    pub fn reopen(install: &Install, old: Self) -> Self {
        let mut p = Self::open(install);
        (p.browse_tab, p.browse) = (old.browse_tab, old.browse);
        p
    }

    /// The search field takes the keys (key repeats reach it).
    pub fn typing(&self) -> bool {
        self.browse_tab && self.browse.typing
    }

    /// The rows `set` as [`ig2_formats::vfs::refusals`] takes them (a broken `mod.toml` as an empty one).
    fn ids(&self, set: &[usize]) -> Vec<(String, &ModInfo)> {
        set.iter().map(|&i| (self.rows[i].id.clone(), self.rows[i].info.as_ref().unwrap_or(&self.rows[i].fallback))).collect()
    }

    fn on(&self) -> Vec<usize> {
        (0..self.rows.len()).filter(|&i| self.rows[i].on).collect()
    }

    fn depends(&self, i: usize) -> &[(String, ig2_formats::vfs::VersionReq)] {
        self.rows[i].info.as_ref().map_or(&[], |inf| inf.depends.as_slice())
    }

    /// Row `i` and the disabled rows it needs, each once, `i` first: what switching it on switches on; why, when a
    /// dependency is in no row.
    fn needs(&self, i: usize) -> Result<Vec<usize>, String> {
        let mut out = vec![i];
        let mut k = 0;
        while k < out.len() {
            for (dep, range) in self.depends(out[k]) {
                if self.rows.iter().any(|r| r.on && &r.id == dep) {
                    continue;
                }
                let j = self.rows.iter().position(|r| &r.id == dep && r.loads).ok_or_else(|| ig2_formats::vfs::Refusal::Missing(dep.clone(), range.clone()).to_string())?;
                if !out.contains(&j) {
                    out.push(j);
                }
            }
            k += 1;
        }
        Ok(out)
    }

    /// Why row `i` (disabled) can't be switched on: a dependency in no row, or a refusal of it or of a dependency it
    /// brings, among the enabled ones.
    fn blocked(&self, i: usize) -> Option<String> {
        let add = match self.needs(i) {
            Ok(a) => a,
            Err(e) => return Some(e),
        };
        let set: Vec<usize> = self.on().into_iter().chain(add.iter().copied()).collect();
        let why = ig2_formats::vfs::refusals(&self.ids(&set), crate::version());
        set.iter().zip(why).find_map(|(j, r)| r.filter(|_| add.contains(j)).map(|r| if *j == i { r.to_string() } else { format!("{}: {r}", self.rows[*j].id) }))
    }

    /// After a change: the enabled rows in dependency order (ADR 0086 §2), each row's refusal and lost files.
    fn refresh(&mut self) {
        let on = self.on();
        let order = ig2_formats::vfs::dependency_order(&self.ids(&on));
        let mut rows: Vec<Option<Row>> = std::mem::take(&mut self.rows).into_iter().map(Some).collect();
        let mut sorted: Vec<Row> = order.iter().filter_map(|&k| rows[on[k]].take()).collect();
        sorted.extend(rows.into_iter().flatten());
        self.rows = sorted;
        let on = self.on();
        let why = ig2_formats::vfs::refusals(&self.ids(&on), crate::version());
        for (&i, r) in on.iter().zip(why) {
            self.rows[i].refused = r.map(|r| r.to_string());
        }
        for i in 0..self.rows.len() {
            if !self.rows[i].on {
                self.rows[i].refused = self.blocked(i);
            }
        }
        self.conflicts();
    }

    /// An enabled mod can't load: Apply waits.
    fn any_refused(&self) -> bool {
        self.rows.iter().any(|r| r.on && r.refused.is_some())
    }

    fn enabled(&self) -> Vec<String> {
        self.rows.iter().filter(|r| r.on).map(|r| r.m.name.clone()).collect()
    }

    /// The changes aren't written yet.
    pub fn pending(&self) -> bool {
        self.enabled() != self.saved
    }

    /// The written list isn't the one this run loaded.
    fn restart_needed(&self) -> bool {
        let loads = |n: &&String| self.rows.iter().any(|r| &r.m.name == *n && r.loads);
        !self.given && !self.saved.iter().filter(loads).eq(self.loaded.iter())
    }

    /// Each enabled mod's files that an enabled mod above it provides too.
    fn conflicts(&mut self) {
        let mut owner: HashMap<&str, &str> = HashMap::new();
        let mut lost = Vec::new();
        for r in &self.rows {
            let mut by: Vec<(String, usize)> = Vec::new();
            if r.on {
                for f in &r.files {
                    match owner.get(f.as_str()) {
                        Some(&w) => match by.iter_mut().find(|(n, _)| n == w) {
                            Some(e) => e.1 += 1,
                            None => by.push((w.to_owned(), 1)),
                        },
                        None => {
                            owner.insert(f, &r.m.name);
                        }
                    }
                }
            }
            lost.push(by);
        }
        for (r, l) in self.rows.iter_mut().zip(lost) {
            r.lost = l;
        }
    }

    /// Move the enabled row `i` up (`-1`) or down (`1`) among the enabled ones.
    fn shift(&mut self, i: usize, by: isize) {
        let j = i as isize + by;
        if j >= 0 && (j as usize) < self.rows.len() && self.rows[j as usize].on {
            self.rows.swap(i, j as usize);
            self.refresh();
        }
    }

    /// Switch row `i`: a disabled mod goes last among the enabled with the mods it needs under it, an enabled one
    /// first among the disabled with the enabled mods that need it; a mod that can't be switched on stays off.
    fn toggle(&mut self, i: usize) {
        let rows: Vec<usize> = if self.rows[i].on {
            // It and every enabled mod needing it, transitively.
            let mut off = vec![i];
            while let Some(j) = (0..self.rows.len()).find(|&j| self.rows[j].on && !off.contains(&j) && self.depends(j).iter().any(|(d, _)| off.iter().any(|&k| &self.rows[k].id == d))) {
                off.push(j);
            }
            off
        } else if self.rows[i].refused.is_some() {
            return;
        } else {
            self.needs(i).unwrap_or_default()
        };
        let on = !self.rows[i].on;
        let mut moved: Vec<Option<Row>> = Vec::new();
        let mut rest: Vec<Option<Row>> = std::mem::take(&mut self.rows).into_iter().map(Some).collect();
        for &k in &rows {
            moved.push(rest[k].take());
        }
        let mut moved: Vec<Row> = moved.into_iter().flatten().collect();
        for r in &mut moved {
            r.on = on;
        }
        self.rows = rest.into_iter().flatten().collect();
        let at = self.rows.iter().position(|r| !r.on).unwrap_or(self.rows.len());
        self.rows.splice(at..at, moved);
        self.refresh();
    }

    /// Write `enabled.txt` (ADR 0068 §4).
    fn apply(&mut self) {
        let names = self.enabled();
        let written = Mod::enabled_file().ok_or_else(|| "no settings folder".to_string()).and_then(|f| {
            f.parent().map_or(Ok(()), ig2_formats::userfs::create_dir_all).and_then(|_| ig2_formats::userfs::write(&f, Mod::enabled_text(&names))).map_err(|e| format!("{}: {e}", f.display()))
        });
        match written {
            Ok(()) => (self.saved, self.error) = (names, None),
            Err(e) => self.error = Some(e),
        }
    }

    fn picture(&mut self, ctx: &egui::Context, i: usize) -> Option<egui::TextureHandle> {
        let r = &self.rows[i];
        let key = r.m.dir.display().to_string();
        self.pictures
            .entry(key)
            .or_insert_with(|| {
                let file = r.info.as_ref().ok()?.picture.clone()?;
                let img = r.m.read(&file).and_then(|b| crate::upscale::decode_png(&b)).map_err(|e| eprintln!("ig2-client: mod {}: {e}", r.m.name)).ok()?;
                (img.width <= MAX_PICTURE && img.height <= MAX_PICTURE).then(|| {
                    let c = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.rgba);
                    ctx.load_texture(format!("mod-{}", r.m.name), c, egui::TextureOptions::LINEAR)
                })
            })
            .clone()
    }

    /// The keys: Esc Back; Enter Apply, or Back with nothing to apply; Up/Down (Tab, Shift+Tab) the focused mod, Space
    /// its switch, Shift+Up/Down its priority, Home/End the first and last; Ctrl+Tab the other tab (`chord`, Ctrl or
    /// Alt held). On the Browse tab, [`crate::mods_browse::Browse::key`].
    pub fn key(&mut self, key: &winit::keyboard::Key, shift: bool, chord: bool) -> Option<Act> {
        use winit::keyboard::{Key, NamedKey};
        if chord && key == &Key::Named(NamedKey::Tab) {
            self.browse_tab = !self.browse_tab;
            self.browse.unfocus(false);
            return Some(Act::Click);
        }
        if self.browse_tab {
            return match self.browse.key(key, shift, chord) {
                None => Some(Act::Back),
                Some(true) => Some(Act::Click),
                Some(false) => None,
            };
        }
        let n = self.rows.len();
        let at = self.focus.filter(|&i| i < n);
        let step = |by: isize| at.map_or(if by > 0 { 0 } else { n.saturating_sub(1) }, |i| (i as isize + by).clamp(0, n.saturating_sub(1) as isize) as usize);
        let moved = |p: &mut Self, i: usize| {
            (p.focus, p.scroll) = (Some(i), true);
            None
        };
        match key.as_ref() {
            Key::Named(NamedKey::Escape) => Some(Act::Back),
            Key::Named(NamedKey::Enter) if self.pending() && !self.any_refused() => {
                self.apply();
                Some(Act::Click)
            }
            Key::Named(NamedKey::Enter) => Some(Act::Back),
            _ if n == 0 => None,
            Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) if shift => {
                let (i, by) = (at?, if key == &Key::Named(NamedKey::ArrowUp) { -1 } else { 1 });
                let before = self.enabled();
                self.shift(i, by);
                if self.enabled() == before {
                    return None;
                }
                moved(self, (i as isize + by) as usize);
                Some(Act::Click)
            }
            Key::Named(NamedKey::ArrowUp) => moved(self, step(-1)),
            Key::Named(NamedKey::Tab) if shift => moved(self, step(-1)),
            Key::Named(NamedKey::ArrowDown | NamedKey::Tab) => moved(self, step(1)),
            Key::Named(NamedKey::Home) => moved(self, 0),
            Key::Named(NamedKey::End) => moved(self, n - 1),
            Key::Named(NamedKey::Space) => {
                let i = at?;
                let name = self.rows[i].m.name.clone();
                self.toggle(i);
                let j = self.rows.iter().position(|r| r.m.name == name).unwrap_or(i);
                moved(self, j);
                Some(Act::Click)
            }
            _ => None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &crate::theme::Theme) -> Vec<Act> {
        let mut acts = Vec::new();
        let hot = crate::gui::ink(theme.hot);
        let bad = egui::Color32::from_rgb(235, 110, 100);
        crate::gui::page(ctx, "mods", egui::Vec2::from(PAGE), |ui| {
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new(tr("MODS")).strong());
                ui.add_space(24.0);
                for (browse, label, tip) in [(false, tr("Installed"), tr("The mods in the mods folders (Ctrl+Tab).")), (true, tr("Browse"), tr("The openIG2 mod repository: fetched from the internet when opened (Ctrl+Tab)."))] {
                    if ui.selectable_label(self.browse_tab == browse, egui::RichText::new(label).size(16.0)).on_hover_text(tip).clicked() && self.browse_tab != browse {
                        self.browse_tab = browse;
                        self.browse.unfocus(false);
                        acts.push(Act::Click);
                    }
                }
            });
            ui.add_space(10.0);
            let body = ui.available_height() - 52.0 - 26.0;
            let (mut toggle, mut shift) = (None, None);
            if self.browse_tab {
                self.browse.start(false);
                self.browse.locked = self.pending();
                ui.allocate_ui(egui::vec2(ui.available_width(), body), |ui| {
                    ui.set_min_height(body);
                    if self.browse.show(ui, body) {
                        acts.push(Act::Click);
                    }
                });
                if std::mem::take(&mut self.browse.changed) {
                    acts.push(Act::Reload);
                }
            } else {
                egui::ScrollArea::vertical().id_salt("mods").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).max_height(body).auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    if self.rows.is_empty() {
                        crate::gui::card(ui, None, |ui| {
                            ui.label(tr("No mods found. Put a mod's folder or .zip file into the mods folder, then open this page again."));
                        });
                    }
                    let enabled = self.rows.iter().filter(|r| r.on).count();
                    for i in 0..self.rows.len() {
                        let pic = self.picture(ui.ctx(), i);
                        let (focused, scroll) = (self.focus == Some(i), self.scroll);
                        let r = &mut self.rows[i];
                        let card = ui.scope(|ui| crate::gui::card(ui, None, |ui| {
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(egui::Vec2::from(PICTURE), egui::Sense::hover());
                                match &pic {
                                    Some(t) => {
                                        egui::Image::new(t).corner_radius(6.0).paint_at(ui, rect);
                                    }
                                    None => {
                                        ui.painter().rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
                                        let letter = r.m.name.chars().next().unwrap_or('?').to_uppercase().to_string();
                                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, letter, egui::FontId::proportional(30.0), ui.visuals().weak_text_color());
                                    }
                                }
                                ui.add_space(8.0);
                                let text_w = ui.available_width() - 150.0;
                                ui.vertical(|ui| {
                                    ui.set_width(text_w);
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    let info = r.info.clone().unwrap_or_default();
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(info.name.as_deref().unwrap_or(&r.m.name)).size(17.0).strong());
                                        if let Some(v) = &info.version {
                                            ui.label(egui::RichText::new(v).weak());
                                        }
                                        if info.kind == ig2_formats::vfs::ModKind::Visual {
                                            ui.label(egui::RichText::new(tr("look only")).small().weak()).on_hover_text(tr("Changes only how the game looks: multiplayer rooms don't need it."));
                                        }
                                        if r.on {
                                            let rank = self_rank(enabled, i);
                                            ui.label(egui::RichText::new(rank).small().color(ui.visuals().hyperlink_color));
                                        }
                                    });
                                    if let Some(a) = &info.author {
                                        ui.label(egui::RichText::new(format!("{} {a}", tr("by"))).small().weak());
                                    }
                                    if let Some(d) = &info.description {
                                        ui.add(egui::Label::new(d).wrap());
                                    }
                                    if !info.depends.is_empty() {
                                        let deps: Vec<String> = info.depends.iter().map(|(d, v)| if v.is_any() { d.clone() } else { format!("{d} {v}") }).collect();
                                        ui.label(egui::RichText::new(format!("{} {}", tr("Needs:"), deps.join(", "))).small());
                                    }
                                    let kind = if r.m.is_zip() { tr("zip") } else { tr("folder") };
                                    let licence = info.license.as_deref().map_or(String::new(), |l| format!(", {l}"));
                                    ui.label(egui::RichText::new(format!("{} ({kind}{licence})", r.m.dir.display())).small().weak());
                                    if let Some(why) = &r.refused {
                                        ui.add(egui::Label::new(egui::RichText::new(format!("{} {why}", tr("Can't load: it"))).small().color(bad)).wrap());
                                    }
                                    if let Some(g) = info.game.as_deref().filter(|g| !crate::version().starts_with(&format!("{g}.")) && !crate::version().starts_with(&format!("{g}-"))) {
                                        ui.label(egui::RichText::new(format!("{} {g}", tr("Made for openIG2"))).small().color(hot));
                                    }
                                    if let Err(e) = &r.info {
                                        ui.add(egui::Label::new(egui::RichText::new(e).small().color(bad)).wrap());
                                    }
                                    for p in &r.problems {
                                        ui.add(egui::Label::new(egui::RichText::new(p).small().color(bad)).wrap());
                                    }
                                    for (w, n) in &r.lost {
                                        ui.label(egui::RichText::new(format!("{n} {} {w}", tr("of its files come from"))).small().color(hot));
                                    }
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let mut on = r.on;
                                    let can = r.on || r.refused.is_none();
                                    if ui.add_enabled_ui(can, |ui| crate::gui::switch(ui, &mut on)).inner.on_hover_text(tr("Load this mod at the next start.")).clicked() {
                                        toggle = Some(i);
                                    }
                                    if r.on {
                                        if arrow(ui, i + 1 < enabled, false).on_hover_text(tr("Lower priority")).clicked() {
                                            shift = Some((i, 1));
                                        }
                                        if arrow(ui, i > 0, true).on_hover_text(tr("Higher priority: its files win")).clicked() {
                                            shift = Some((i, -1));
                                        }
                                    }
                                });
                            });
                        })).response;
                        if focused {
                            crate::gui::focus_ring(ui, card.rect);
                            if scroll {
                                card.scroll_to_me(None);
                            }
                        }
                    }
                });
            }
            self.scroll = false;
            if ui.ctx().input(|i| i.pointer.any_pressed()) {
                self.focus = None;
                self.browse.unfocus(true);
            }
            if let Some(i) = toggle {
                self.toggle(i);
                acts.push(Act::Click);
            }
            if let Some((i, by)) = shift {
                self.shift(i, by);
                acts.push(Act::Click);
            }
            ui.add_space(4.0);
            // The status, then the buttons.
            ui.horizontal(|ui| {
                ui.set_min_height(22.0);
                let browse = self.browse_tab.then(|| self.browse.status_line()).flatten();
                if let Some(e) = &self.error {
                    ui.label(egui::RichText::new(e).color(bad));
                } else if let Some(s) = browse {
                    let (text, colour) = match &s {
                        Ok(t) => (t, hot),
                        Err(e) => (e, bad),
                    };
                    ui.add(egui::Label::new(egui::RichText::new(text).color(colour)).truncate()).on_hover_text(text);
                } else if let Some(e) = self.failed.as_ref().filter(|_| !self.pending()) {
                    let line = format!("{} {e}", tr("The mods failed to load; the game started without them:"));
                    ui.add(egui::Label::new(egui::RichText::new(line).color(bad)).truncate()).on_hover_text(e);
                } else if self.any_refused() {
                    ui.label(egui::RichText::new(tr("A mod can't load: switch it off or add what it needs.")).color(bad));
                } else if self.given {
                    ui.label(egui::RichText::new(tr("This run's mods come from --mod; the list applies without it.")).color(hot));
                } else if self.pending() {
                    ui.label(egui::RichText::new(tr("Apply saves the list and loads the mods.")).weak());
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::gui::primary(ui, self.pending() && !self.any_refused(), tr("Apply")).on_hover_text(tr("Save the list of enabled mods.")).clicked() {
                        self.apply();
                        acts.push(if self.restart_needed() { Act::Swap } else { Act::Click });
                    }
                    if crate::gui::secondary(ui, tr("Back")).on_hover_text(tr("Leave; changes not applied are taken back.")).clicked() {
                        acts.push(Act::Back);
                    }
                    if crate::gui::secondary(ui, tr("Open mods folder")).on_hover_text(tr("The mods folder beside the game: drop a mod's .zip file here.")).clicked() {
                        if let Err(e) = open_folder() {
                            self.error = Some(e);
                        }
                        acts.push(Act::Click);
                    }
                    if crate::gui::secondary(ui, tr("How to make a mod")).on_hover_text(tr("The modding tutorial on the openIG2 website, in your browser.")).clicked() {
                        if let Err(e) = open(std::ffi::OsStr::new(HOW_TO)) {
                            self.error = Some(e);
                        }
                        acts.push(Act::Click);
                    }
                });
            });
        });
        acts
    }
}

/// A priority arrow: a button with a triangle (the fonts have no arrow glyphs).
fn arrow(ui: &mut egui::Ui, enabled: bool, up: bool) -> egui::Response {
    let r = ui.add_enabled(enabled, egui::Button::new("").min_size(egui::vec2(30.0, 28.0)));
    let c = r.rect.center();
    let (tip, base) = if up { (-5.0, 4.0) } else { (5.0, -4.0) };
    let colour = if enabled { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };
    let points = vec![egui::pos2(c.x, c.y + tip), egui::pos2(c.x - 6.0, c.y + base), egui::pos2(c.x + 6.0, c.y + base)];
    ui.painter().add(egui::Shape::convex_polygon(points, colour, egui::Stroke::NONE));
    r
}

/// "priority 1/3": an enabled row's place.
fn self_rank(enabled: usize, i: usize) -> String {
    format!("{} {}/{enabled}", tr("priority"), i + 1)
}

/// The mods folder beside the executable, made when missing, in the system's file manager.
fn open_folder() -> Result<(), String> {
    let dir = Mod::exe_folder().ok_or_else(|| "no mods folder here".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    open(dir.as_os_str())
}

/// A folder or a web address in the system's file manager or browser.
pub(crate) fn open(what: &std::ffi::OsStr) -> Result<(), String> {
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener).arg(what).spawn().map(|_| ()).map_err(|e| format!("{opener}: {e}"))
}

/// The game again with this run's arguments (the updater's new version, ADR 0087); the caller then quits.
pub fn restart() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe).args(std::env::args_os().skip(1)).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// `--snapshot out.png --screen mods` (or `mods-browse`, the Browse tab once its index has loaded, at most 30 s).
pub fn snapshot(o: &crate::Options, out: &std::path::Path, browse: bool) -> Result<(), String> {
    let l = crate::load(o)?;
    let mut r = crate::render::Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb).install_fonts(&l.install);
    let mut page = ModsPage::open(&l.install);
    page.browse_tab = browse;
    let theme = crate::theme::Theme::modern();
    let style = crate::gui::style(&theme);
    let until = web_time::Instant::now() + std::time::Duration::from_secs(30);
    for i in 0.. {
        gui.run(o.size, &style, |ctx| _ = page.show(ctx, &theme));
        if i >= 40 && (!page.browse.busy() || web_time::Instant::now() > until) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let rgba = r.draw_to_image_over(&crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]), o.size, 0.0, |d, q, t| gui.paint(d, q, t));
    crate::assets::write_snapshot(out, o, &rgba)?;
    println!("{}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, on: bool, files: &[&str]) -> Row {
        let m = Mod { name: name.into(), dir: name.into() };
        Row::new(m, Ok(ModInfo::default()), on, Vec::new(), files.iter().map(|f| f.to_string()).collect(), name != "broken")
    }

    fn page(rows: Vec<Row>) -> ModsPage {
        let mut p = ModsPage { rows, saved: Vec::new(), loaded: Vec::new(), given: false, failed: None, pictures: HashMap::new(), error: None, focus: None, scroll: false, browse_tab: false, browse: Default::default() };
        p.refresh();
        p
    }

    #[test]
    fn priority_toggles_and_conflicts() {
        let mut p = page(vec![row("a", true, &["x", "y"]), row("b", true, &["x", "y", "z"]), row("c", false, &["z"])]);
        assert_eq!(p.rows[1].lost, [("a".to_string(), 2)]);
        // Up: b wins now; a disabled one has no conflicts.
        p.shift(1, -1);
        assert_eq!(p.enabled(), ["b", "a"]);
        assert_eq!((p.rows[0].lost.len(), p.rows[1].lost.clone()), (0, vec![("b".to_string(), 2)]));
        // Not past the enabled ones, nor above the top.
        p.shift(1, 1);
        p.shift(0, -1);
        assert_eq!(p.enabled(), ["b", "a"]);
        // c on: last among the enabled; b off: first among the disabled.
        p.toggle(2);
        assert_eq!(p.enabled(), ["b", "a", "c"]);
        assert_eq!(p.rows[2].lost, [("b".to_string(), 1)]);
        p.toggle(0);
        assert_eq!(p.rows.iter().map(|r| (r.m.name.as_str(), r.on)).collect::<Vec<_>>(), [("a", true), ("c", true), ("b", false)]);
        assert!(p.pending() && p.rows[1].lost.is_empty());
        p.saved = p.enabled();
        assert!(!p.pending() && p.restart_needed());
        p.loaded = p.saved.clone();
        assert!(!p.restart_needed());
        // A mod that can't load is no change to restart for.
        p.rows.push(row("broken", true, &[]));
        p.saved.push("broken".into());
        assert!(!p.restart_needed());
    }

    fn with(name: &str, on: bool, toml: &str) -> Row {
        Row::new(Mod { name: name.into(), dir: name.into() }, ModInfo::parse(toml), on, Vec::new(), Vec::new(), true)
    }

    fn names(p: &ModsPage) -> Vec<(&str, bool)> {
        p.rows.iter().map(|r| (r.m.name.as_str(), r.on)).collect()
    }

    #[test]
    fn dependencies_switch_with_their_mods() {
        let needs = "[depends]
base = \">=1.2\"";
        let mut p = page(vec![with("plain", true, ""), with("Needs", false, needs), with("base", false, "version = \"1.4\""), with("lonely", false, "depends = { gone = \"*\" }")]);
        // Off rows say why they can't be switched on; the switch then does nothing.
        assert_eq!(p.rows[3].refused.as_deref(), Some("needs the mod gone"));
        p.toggle(3);
        assert_eq!(p.enabled(), ["plain"]);
        // On: its dependency comes along, under it.
        p.toggle(1);
        assert_eq!(names(&p), [("plain", true), ("Needs", true), ("base", true), ("lonely", false)]);
        assert!(!p.any_refused());
        // The dependency moved above its mod goes back under it.
        p.shift(2, -1);
        assert_eq!(p.enabled(), ["plain", "Needs", "base"]);
        // Off: the mods needing it go off too.
        p.toggle(2);
        assert_eq!(names(&p), [("plain", true), ("base", false), ("Needs", false), ("lonely", false)]);
        // A list written by hand: the mod without its dependency is refused, Apply waits.
        let mut p = page(vec![with("needs", true, needs), with("old", false, "requires = \"<0.1\""), with("base", false, "version = \"1.1\"")]);
        assert_eq!(p.rows[0].refused.as_deref(), Some("needs the mod base >=1.2"));
        assert!(p.any_refused() && p.key(&winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter), false, false) == Some(Act::Back));
        assert_eq!(p.rows[1].refused.as_deref(), Some("needs openIG2 <0.1"));
        // Switching the too old dependency on: refused with its reason.
        assert_eq!(p.rows[2].refused.as_deref(), None);
        p.toggle(2);
        assert_eq!(p.rows[0].refused.as_deref(), Some("needs the mod base >=1.2, not 1.1"));
    }

    #[test]
    fn keys_focus_toggle_and_reorder() {
        use winit::keyboard::{Key, NamedKey};
        let k = |n| Key::Named(n);
        let mut p = page(vec![row("a", true, &[]), row("b", true, &[]), row("c", false, &[])]);
        p.saved = p.enabled();
        assert_eq!(p.key(&k(NamedKey::ArrowDown), false, false), None);
        assert_eq!(p.focus, Some(0));
        p.key(&k(NamedKey::ArrowDown), false, false);
        // Shift+Up: b first, the focus with it; not above the top.
        assert_eq!(p.key(&k(NamedKey::ArrowUp), true, false), Some(Act::Click));
        assert_eq!((p.enabled(), p.focus), (vec!["b".to_string(), "a".to_string()], Some(0)));
        assert_eq!(p.key(&k(NamedKey::ArrowUp), true, false), None);
        // End, Space: c on, last among the enabled.
        p.key(&k(NamedKey::End), false, false);
        assert_eq!(p.key(&k(NamedKey::Space), false, false), Some(Act::Click));
        assert_eq!((p.enabled().len(), p.focus), (3, Some(2)));
        // Changes pending; Esc leaves (taking them back).
        assert!(p.pending());
        p.error = None;
        assert_eq!(p.key(&k(NamedKey::Escape), false, false), Some(Act::Back));
    }
}
