//! Ours (ADR 0053): the main menu's Options screen. Every setting of the in-game screen (`OptionsScreen`'s rows,
//! sliders) and the multiplayer name in eight tabs with switches, drop-downs, sliders and descriptions; Apply writes
//! them through `OptionsScreen::store`, Back takes back what was not applied. A designed egui page (ADR 0062) in the
//! UI theme's colours (ADR 0056); the keyboard moves through it with the screen's own keys.

use super::{check_dependencies, Act, Group, OptionsScreen, Row};
use crate::ui::UiTexts;

/// `MultiplayerNev`'s length (`IOMaxLen` 9 in the menu's name entry, mainscr.rs).
const NAME_MAX: usize = 9;
const NAME_KEY: &str = "MultiplayerNev";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Display,
    Sound,
    Interface,
    Game,
    Accessibility,
    Multiplayer,
    Language,
    /// ADR 0058: the key bindings.
    Controls,
}

impl Tab {
    pub const ALL: [Tab; 8] = [Tab::Display, Tab::Sound, Tab::Interface, Tab::Game, Tab::Controls, Tab::Accessibility, Tab::Multiplayer, Tab::Language];

    /// The label (the remake text table's key).
    pub fn title(self) -> &'static str {
        ["DISPLAY", "SOUND", "INTERFACE", "GAME", "ACCESSIBILITY", "MULTIPLAYER", "LANGUAGE", "CONTROLS"][self as usize]
    }

    fn group(self) -> Option<Group> {
        match self {
            Tab::Display => Some(Group::Display),
            Tab::Sound => Some(Group::Sound),
            Tab::Controls => None,
            Tab::Interface => Some(Group::Interface),
            Tab::Game => Some(Group::Game),
            Tab::Accessibility => Some(Group::Accessibility),
            Tab::Multiplayer => Some(Group::Multiplayer),
            Tab::Language => Some(Group::Language),
        }
    }
}

/// A setting on a tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Row(usize),
    Slider(usize),
    Name,
    /// ADR 0058: an action's key (`crate::bindings::ACTIONS`), and the reset of every key.
    Bind(usize),
    Reset,
}

/// The tab's settings in order (ADR 0053 §2): Display the remake's rows, gamma, the exe's graphics rows, the modern
/// lighting's (ADR 0055); Sound the
/// two volumes, then ours; Game the exe's game rows, then ours; Multiplayer the name, then ours; Controls the keys
/// and their reset (ADR 0058); the rest the group's rows.
pub fn items(rows: &[Row], tab: Tab) -> Vec<Item> {
    let ours = |g| rows.iter().enumerate().filter(move |(_, r)| r.ours && r.group == g).map(|(k, _)| Item::Row(k));
    let exe = |page| rows.iter().enumerate().filter(move |(_, r)| !r.ours && r.page == page).map(|(k, _)| Item::Row(k));
    let mut v: Vec<Item> = match tab {
        Tab::Sound => vec![Item::Slider(0), Item::Slider(1)],
        Tab::Controls => return (0..crate::bindings::ACTIONS.len()).map(Item::Bind).chain([Item::Reset]).collect(),
        Tab::Display => ours(Group::Display).chain([Item::Slider(2)]).chain(exe(1)).chain(ours(Group::Graphics)).collect(),
        Tab::Game => exe(2).collect(),
        Tab::Multiplayer => vec![Item::Name],
        _ => Vec::new(),
    };
    if !matches!(tab, Tab::Display) {
        v.extend(tab.group().into_iter().flat_map(ours));
    }
    v
}

/// The string (file 13, section 2) an exe row's key stands for: rows 7–16 show the next-but-one string in the exe
/// (re/screens/options.md §6), here the one they mean (ADR 0053 §4).
fn exe_name(key: &str) -> Option<u16> {
    const KEYS: [&str; 15] = [
        "OptionsKellAutoSave", "DebugData_grBattle", "DebugData_GrNoOverrun", "DebugData_Battle", "DebugData_SpNoOverrun", "KELLKINYITASDEFAULT", "Lowrestextures",
        "KellEpuletnevKiiras", "OptionsKellArnyek", "OptionsKellUt", "OptionsKellSpecFX", "OptionsKellIdojaras", "OptionsBuildingRes", "OptionsHiresVectors", "OptionsFogResolution",
    ];
    KEYS.iter().position(|&k| k == key).map(|n| n as u16 + 1)
}

/// A setting's one-line description (the remake text table's key, ADR 0053 §5).
pub fn describe(key: &str) -> &'static str {
    match key {
        "OptionsKellAutoSave" => "Saves the game into slot 24 at the start of January and July.",
        "DebugData_grBattle" => "Ground battles are fought out by the computer, without the battle screen.",
        "DebugData_GrNoOverrun" => "Overwhelming forces win ground battles without them being fought.",
        "DebugData_Battle" => "Space battles are fought out by the computer, without the battle screen.",
        "DebugData_SpNoOverrun" => "Overwhelming fleets win space battles without them being fought.",
        "Cutscenes" => "Plays the story's video scenes.",
        "VROT" => "A switch of the original without a known effect.",
        "KELLKINYITASDEFAULT" => "Screens open and close with their animation.",
        "Lowrestextures" => "Loads the smaller textures.",
        "KellEpuletnevKiiras" => "Shows the buildings' names on the colony screen.",
        "OptionsKellArnyek" => "Draws shadows under the colony's buildings and units.",
        "OptionsKellUt" => "Draws the roads between the colony's buildings.",
        "OptionsKellSpecFX" => "Smoke, dust and sparks on the colony screen.",
        "OptionsKellIdojaras" => "Rain, snow and other weather on the colony screen.",
        "OptionsBuildingRes" => "The detail of the colony's building models.",
        "OptionsHiresVectors" => "Sharper line drawings on the research screen.",
        "OptionsFogResolution" => "How many nebula clouds the starmap draws.",
        "Music Volume" => "The music's loudness.",
        "Sound Volume" => "The loudness of the sound effects, and of speech at Same as sound.",
        "VoiceVolume" => "The loudness of the advisors' and messages' speech.",
        "ExtraSpeeds" => "The fastest speed's key or button pressed again goes faster: x2, then x4.",
        "Bind" => "Enter or a click, then the new key; Esc keeps the old one.",
        "ResetKeys" => "Every key back to the original game's.",
        "Gamma Correction" => "The picture's brightness, where the display allows it.",
        NAME_KEY => "Your name in multiplayer games, at most 9 letters.",
        "DisplayMode" => "A window, a borderless window over the screen, or fullscreen.",
        "Resolution" => "The size of the window or of the fullscreen mode.",
        "VSync" => "Waits for the display's refresh, so the picture doesn't tear.",
        "Letterbox" => "Keeps the original 4:3 picture with bars at the sides.",
        "LiveBackground" => "The game moves on visibly behind the screens over it.",
        "OneLanguage" => "Empire, planet and other names in the game's language.",
        "CleanNameEntry" => "A save's old name isn't shown under the one being typed.",
        "BoxModelsInside" => "The info boxes' turning models stay inside their boxes.",
        "SliderUnderLabel" => "The space battle's attack slider sits under its label.",
        "HungarianArticles" => "The right a/az article before names in Hungarian texts.",
        "CentredExitLabel" => "The starmap panels' Exit label is centred on its button.",
        "WasdCamera" => "W, A, S and D move the camera on the starmap, or on colonies too.",
        "AutosaveEvery" => "How often the game saves itself (Original: twice a year).",
        "MenuSettings" => "The main menu's Settings label and the O key.",
        "Language" => "The texts' language, from the next start.",
        "FleetGroups" => "Ctrl+1..9 stores the selected fleet, Alt+1..9 selects it.",
        "GameStatistics" => "A statistics screen after each game, and the match history.",
        "RecordReplays" => "Each game is recorded and can be watched from the main menu.",
        "EventLog" => "The messages screen keeps a log of the game's events.",
        "Subtitles" => "The spoken messages' text at the bottom of the screen.",
        "ColourBlindColours" => "Empire colours that stay apart with colour blindness.",
        "SharpUi" => "Pictures and panels stay sharp on big screens.",
        "SharpText" => "Letters with crisp, smooth edges on big screens.",
        "StarmapMouseCamera" => "The wheel zooms and the right button drags the starmap.",
        "HighScoreEntry" => "A won scenario's time goes into its high-score table.",
        "EnhancedAi" => "How the computer empires of the next game manage their economy and research.",
        "TitleMusicWithIntro" => "The title video's music starts with the intro, not under the logos.",
        "StoryFixes" => "Enhanced fixes the original campaign story's slips in the next new game.",
        "UiTheme" => "The look of the remake's own screens; Classic keeps the game's colours.",
        "TextureUpscale" => "Smooths the game's pictures and model textures 2x or 4x as they load.",
        "AntiAliasing" => "Smooth edges on the 3D scenes' buildings, ships and planets.",
        "PixelLighting" => "The space battle's ships lit per pixel, so their hulls shade smoothly.",
        "SoftShadows" => "Soft edges on the colony's building shadows.",
        "Bloom" => "A soft glow around fire, blasts and the colony's weather.",
        "SmoothGlows" => "Soft, round glows and stars on the starmap and the backgrounds at any resolution.",
        "BattleEffects" => "Sparks, shock waves, smoke and glowing shots on top of the battles' own effects.",
        "ModernStarmapHud" => "The starmap's bar, info box, minimap and camera buttons as clear modern panels; off = the original's.",
        "MenuNavigation" => "A grouped main menu with Continue; Back, Esc or right click on every menu screen.",
        _ => "",
    }
}

/// A key on the screen (the window maps the keyboard to these).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    NextTab,
    PrevTab,
    /// Enter or Space.
    Activate,
    /// Esc.
    Back,
    /// In the name field: a letter (the font's byte), Backspace.
    Char(u8),
    Erase,
}

/// The screen's own state; the values are the `OptionsScreen`'s.
#[derive(Debug, Clone)]
pub struct Modern {
    pub tab: Tab,
    /// The focused line: a setting of the tab, then Back, then Apply.
    pub focus: usize,
    /// The last input was a key: the focus is marked and kept in view.
    keyed: bool,
    /// The focused line last scrolled into view.
    shown: Option<(Tab, usize)>,
    pub editing: bool,
    /// `MultiplayerNev` in the font's bytes.
    pub name: Vec<u8>,
    /// ADR 0058: the keys, the action waiting for its new key.
    pub binds: crate::bindings::Bindings,
    pub capture: Option<usize>,
    /// The applied values Back puts back: the rows', the sliders', the name, the keys.
    kept: (Vec<i32>, [i32; 3], Vec<u8>, crate::bindings::Bindings),
}

impl Modern {
    pub fn new(o: &OptionsScreen, name: Vec<u8>, binds: crate::bindings::Bindings) -> Self {
        let mut m = Self { tab: Tab::Display, focus: 0, keyed: false, shown: None, editing: false, name, binds, capture: None, kept: Default::default() };
        m.keep(o);
        m
    }

    fn values(&self, o: &OptionsScreen) -> (Vec<i32>, [i32; 3], Vec<u8>, crate::bindings::Bindings) {
        (o.rows.iter().map(|r| r.value).collect(), [0, 1, 2].map(|i| o.sliders[i].value), self.name.clone(), self.binds.clone())
    }

    fn keep(&mut self, o: &OptionsScreen) {
        self.kept = self.values(o);
    }

    /// Settings changed since the last Apply.
    pub fn pending(&self, o: &OptionsScreen) -> bool {
        self.values(o) != self.kept
    }

    fn items(&self, o: &OptionsScreen) -> Vec<Item> {
        items(&o.rows, self.tab)
    }

    fn enabled(o: &OptionsScreen, it: Item) -> bool {
        match it {
            Item::Row(k) => o.rows[k].enabled,
            Item::Slider(i) => o.sliders[i].enabled,
            Item::Name | Item::Bind(_) | Item::Reset => true,
        }
    }

    /// Set a row's value as the in-game press does: an exe row marks the detail custom, the dependencies follow.
    fn set(o: &mut OptionsScreen, k: usize, v: i32) {
        let r = &mut o.rows[k];
        r.value = v;
        if !r.ours {
            o.gfx_detail = 3;
        }
        check_dependencies(&mut o.rows, o.multiplayer);
    }

    /// Step a row's value by `d`, wrapping as the in-game press does.
    fn step(o: &mut OptionsScreen, k: usize, d: i32) {
        let r = &o.rows[k];
        let v = (r.value + d).rem_euclid((r.states + 1).max(1));
        Self::set(o, k, v);
    }

    fn set_tab(&mut self, t: Tab) {
        self.tab = t;
        self.focus = 0;
        self.capture = None;
        self.editing = false;
    }

    /// Apply: the values into the settings file (the window stores and applies them on `dirty`).
    fn apply(&mut self, o: &mut OptionsScreen) {
        if self.name.is_empty() {
            self.name = self.kept.2.clone();
        }
        self.keep(o);
        o.dirty = true;
    }

    /// Back: what wasn't applied taken back, then the screen leaves (as Esc on the in-game screen).
    fn back(&mut self, o: &mut OptionsScreen) {
        self.editing = false;
        for (r, &v) in o.rows.iter_mut().zip(&self.kept.0) {
            r.value = v;
        }
        for (s, &v) in o.sliders.iter_mut().zip(&self.kept.1) {
            s.value = v;
        }
        self.name = self.kept.2.clone();
        self.binds = self.kept.3.clone();
        self.capture = None;
        check_dependencies(&mut o.rows, o.multiplayer);
        if o.leaving.is_none() {
            o.leave(if o.load_only { Act::MainMenu } else { Act::Back });
        }
    }

    /// ADR 0058: the key pressed while an action waits for one becomes its key (a key no binding takes is ignored).
    pub fn bind(&mut self, k: winit::keyboard::KeyCode, sound: &mut dyn FnMut(crate::audio::SoundId)) {
        let Some(b) = self.capture else { return };
        if crate::bindings::key_name(k).is_some() {
            self.binds.0[b] = k;
            self.capture = None;
            sound(crate::audio::CLICK);
        }
    }

    pub fn key(&mut self, o: &mut OptionsScreen, k: Nav, sound: &mut dyn FnMut(crate::audio::SoundId)) {
        self.keyed = true;
        if self.capture.is_some() {
            // Esc (or another key of the screen's own) gives up waiting for the key.
            self.capture = None;
            return;
        }
        let items = self.items(o);
        let lines = items.len() + 2;
        if self.editing {
            match k {
                Nav::Char(c) if c >= 0x20 && self.name.len() < NAME_MAX => self.name.push(c),
                Nav::Erase => {
                    self.name.pop();
                }
                Nav::Activate | Nav::Back => self.editing = false,
                _ => {}
            }
            return;
        }
        let tab = Tab::ALL.iter().position(|&t| t == self.tab).unwrap();
        match k {
            Nav::NextTab => self.set_tab(Tab::ALL[(tab + 1) % Tab::ALL.len()]),
            Nav::PrevTab => self.set_tab(Tab::ALL[(tab + Tab::ALL.len() - 1) % Tab::ALL.len()]),
            Nav::Up | Nav::Down => self.focus = (self.focus + if k == Nav::Up { lines - 1 } else { 1 }) % lines,
            Nav::Back => self.back(o),
            Nav::Left | Nav::Right | Nav::Activate => {
                let d = if k == Nav::Left { -1 } else { 1 };
                match (self.focus.checked_sub(items.len()), items.get(self.focus)) {
                    (Some(0), _) if k == Nav::Activate => self.back(o),
                    (Some(1), _) if k == Nav::Activate && self.pending(o) => self.apply(o),
                    (Some(1), _) if k == Nav::Activate => return,
                    (Some(_), _) => self.focus = items.len() + usize::from(k == Nav::Right),
                    (None, Some(&it)) if Self::enabled(o, it) => match it {
                        Item::Row(r) => Self::step(o, r, d),
                        Item::Slider(s) if k != Nav::Activate => o.sliders[s].value = (o.sliders[s].value + 5 * d).clamp(0, 100),
                        Item::Name if k == Nav::Activate => self.editing = true,
                        Item::Bind(b) if k == Nav::Activate => self.capture = Some(b),
                        Item::Reset if k == Nav::Activate => self.binds = Default::default(),
                        _ => return,
                    },
                    _ => return,
                }
                sound(crate::audio::CLICK);
            }
            Nav::Char(_) | Nav::Erase => {}
        }
    }
}

// The page (ADR 0062): egui over the main menu's scene.

/// The card a setting sits in: its group's heading (the remake text table's key); `None` = the card before's.
fn section(o: &OptionsScreen, it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Row(k) if o.rows[k].ours => o.rows[k].group.title(),
        Item::Row(k) if o.rows[k].page == 1 => Group::Graphics.title(),
        Item::Row(_) => Group::Game.title(),
        Item::Slider(2) => Group::Display.title(),
        Item::Slider(_) => Group::Sound.title(),
        Item::Name => Group::Multiplayer.title(),
        Item::Bind(b) => crate::bindings::ACTIONS[b].ctx.title(),
        Item::Reset => return None,
    })
}

/// The tab's settings in cards: (heading, the settings' indices into the tab's items).
fn cards(o: &OptionsScreen, items: &[Item]) -> Vec<(&'static str, Vec<usize>)> {
    let mut v: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (i, &it) in items.iter().enumerate() {
        match (section(o, it), v.last_mut()) {
            (Some(s), Some(last)) if last.0 != s => v.push((s, vec![i])),
            (Some(s), None) => v.push((s, vec![i])),
            (_, Some(last)) => last.1.push(i),
            (None, None) => v.push(("", vec![i])),
        }
    }
    v
}

/// A setting's label.
fn label(o: &OptionsScreen, texts: &UiTexts, it: Item) -> Vec<u8> {
    match it {
        Item::Row(k) => {
            let r = &o.rows[k];
            match exe_name(&r.key) {
                _ if r.ours => texts.ours(&String::from_utf8_lossy(&r.name)),
                Some(n) => texts.get(13, 2, n),
                None => r.name.clone(),
            }
        }
        Item::Slider(i) => o.sliders[i].title.clone(),
        Item::Name => texts.ours("Player name"),
        Item::Bind(b) => texts.ours(crate::bindings::ACTIONS[b].name),
        Item::Reset => texts.ours("Reset all keys"),
    }
}

/// A row's value `v` as the choice shows it.
fn value_text(texts: &UiTexts, r: &Row, v: i32) -> Vec<u8> {
    match r.labels.get(v.max(0) as usize) {
        Some(l) => texts.ours(&String::from_utf8_lossy(l)),
        None if r.states == 2 => texts.get(13, 3, 3 + v.clamp(0, 2) as u16),
        None => texts.get(13, 3, if v != 0 { 1 } else { 2 }),
    }
}

fn key_of(o: &OptionsScreen, it: Item) -> &str {
    match it {
        Item::Row(k) => &o.rows[k].key,
        Item::Slider(i) => ["Music Volume", "Sound Volume", "Gamma Correction"][i],
        Item::Name => NAME_KEY,
        Item::Bind(_) => "Bind",
        Item::Reset => "ResetKeys",
    }
}

/// The page's size in points, its sidebar's width and its controls' column.
const PAGE: [f32; 2] = [860.0, 500.0];
const SIDEBAR: f32 = 168.0;
const CONTROL: f32 = 210.0;

impl Modern {
    /// One frame of the page: draws it and takes its pointer input.
    pub fn show(&mut self, o: &mut OptionsScreen, ctx: &egui::Context, texts: &UiTexts, theme: &crate::theme::Theme, sound: &mut dyn FnMut(crate::audio::SoundId)) {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let tr = |en: &str| s(texts.ours(en));
        if ctx.input(|i| i.pointer.any_pressed()) {
            // A press anywhere ends the name entry and the wait for a key, as a click on the old screen did.
            (self.editing, self.capture, self.keyed) = (false, None, false);
        }
        let mut click = false;
        let hot = crate::gui::ink(theme.hot);
        crate::gui::page(ctx, "options", egui::Vec2::from(PAGE), |ui| {
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new(tr("OPTIONS")).strong());
            });
            ui.add_space(10.0);
            let body = ui.available_height() - 52.0;
            ui.horizontal_top(|ui| {
                // The tabs.
                ui.vertical(|ui| {
                    ui.set_width(SIDEBAR);
                    for tab in Tab::ALL {
                        let on = tab == self.tab;
                        let title = title_case(&tr(tab.title()));
                        let r = ui.add_sized([SIDEBAR, 34.0], egui::Button::selectable(on, egui::RichText::new(title).size(16.0)));
                        if on {
                            let bar = egui::Rect::from_min_size(r.rect.left_top() + egui::vec2(0.0, 7.0), egui::vec2(3.0, r.rect.height() - 14.0));
                            ui.painter().rect_filled(bar, 2.0, ui.visuals().hyperlink_color);
                        }
                        if r.clicked() && !on {
                            self.set_tab(tab);
                            click = true;
                        }
                    }
                });
                ui.add_space(12.0);
                // The settings.
                let items = self.items(o);
                ui.vertical(|ui| egui::ScrollArea::vertical().id_salt(self.tab).scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).max_height(body).auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    let all = cards(o, &items);
                    let several = all.len() > 1;
                    for (title, lines) in all {
                        let title = (several && !title.is_empty()).then(|| tr(title));
                        crate::gui::card(ui, title.as_deref(), |ui| {
                            for i in lines {
                                click |= self.line(o, ui, texts, i, items[i], hot, sound);
                            }
                        });
                        ui.add_space(8.0);
                    }
                }));
            });
            ui.add_space(6.0);
            ui.separator();
            ui.horizontal(|ui| {
                let items = self.items(o);
                // The status: the wait for a key, a key two actions share, the language's restart.
                let lang = items.iter().find_map(|&it| match it {
                    Item::Row(k) if o.rows[k].key == crate::display::LANGUAGE => Some(o.rows[k].value),
                    _ => None,
                });
                let clash = match items.get(self.focus) {
                    Some(&Item::Bind(b)) => self.binds.clash(b),
                    _ => None,
                };
                if self.capture.is_some() {
                    ui.label(egui::RichText::new(tr("Press the new key; Esc keeps the old one.")).color(hot));
                } else if let Some(c) = clash {
                    ui.label(egui::RichText::new(format!("{} {}", tr("Also the key of:"), s(label(o, texts, Item::Bind(c))))).color(hot));
                } else if lang.is_some() && o.lang_at_start.is_some_and(|v| Some(v) != lang) {
                    ui.label(egui::RichText::new(tr("Restart to change the language")).color(hot));
                } else if self.tab == Tab::Controls {
                    ui.label(egui::RichText::new(tr(describe("Bind"))).weak());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let pending = self.pending(o);
                    let apply = crate::gui::primary(ui, pending, &tr("Apply")).on_hover_text(tr("Save the changes and use them."));
                    if self.keyed && self.focus == items.len() + 1 {
                        crate::gui::focus_ring(ui, apply.rect);
                    }
                    let back = crate::gui::secondary(ui, &tr("Back")).on_hover_text(tr("Leave; changes not applied are taken back."));
                    if self.keyed && self.focus == items.len() {
                        crate::gui::focus_ring(ui, back.rect);
                    }
                    if apply.clicked() {
                        self.apply(o);
                        click = true;
                    }
                    if back.clicked() {
                        self.back(o);
                        click = true;
                    }
                });
            });
        });
        if click {
            sound(crate::audio::CLICK);
        }
    }

    /// A setting's line: its label and description, its control at the right. True when it was clicked.
    #[allow(clippy::too_many_arguments)]
    fn line(&mut self, o: &mut OptionsScreen, ui: &mut egui::Ui, texts: &UiTexts, i: usize, it: Item, hot: egui::Color32, sound: &mut dyn FnMut(crate::audio::SoundId)) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let live = Self::enabled(o, it);
        let mut click = false;
        let bg = ui.painter().add(egui::Shape::Noop);
        let row = ui.horizontal(|ui| {
            ui.set_min_height(36.0);
            let text_w = ui.available_width() - CONTROL - 12.0;
            ui.vertical(|ui| {
                ui.set_width(text_w);
                ui.spacing_mut().item_spacing.y = 1.0;
                let name = egui::RichText::new(s(label(o, texts, it))).size(15.5);
                ui.add_enabled(live, egui::Label::new(name).wrap());
                // The keys' common description is the status line's (one line, not one per key).
                let d = if matches!(it, Item::Bind(_)) { "" } else { describe(key_of(o, it)) };
                if !d.is_empty() {
                    ui.add_enabled(live, egui::Label::new(egui::RichText::new(s(texts.ours(d))).small().weak()).wrap());
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.set_width(CONTROL);
                ui.add_enabled_ui(live, |ui| click |= self.control(o, ui, texts, it, hot, sound));
            });
        });
        let rect = row.response.rect.expand2(egui::vec2(6.0, 4.0));
        let hovered = ui.rect_contains_pointer(rect);
        if hovered && ui.input(|x| x.pointer.primary_pressed()) {
            self.focus = i;
        }
        let focused = self.keyed && self.focus == i;
        if hovered || focused {
            let fill = ui.visuals().selection.bg_fill.gamma_multiply(if focused { 0.30 } else { 0.16 });
            ui.painter().set(bg, egui::Shape::rect_filled(rect, 8.0, fill));
        }
        if focused {
            crate::gui::focus_ring(ui, rect);
            if self.shown != Some((self.tab, i)) {
                ui.scroll_to_rect(rect, None);
                self.shown = Some((self.tab, i));
            }
        }
        click
    }

    /// A setting's control: a switch, a drop-down, a slider, the name field, a key. True when it was clicked.
    fn control(&mut self, o: &mut OptionsScreen, ui: &mut egui::Ui, texts: &UiTexts, it: Item, hot: egui::Color32, sound: &mut dyn FnMut(crate::audio::SoundId)) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let field = egui::vec2(CONTROL - 10.0, 30.0);
        match it {
            Item::Row(k) => {
                let r = &o.rows[k];
                let v = if r.locked { 0 } else { r.value };
                if r.states == 1 && r.labels.is_empty() {
                    let mut on = v != 0;
                    if crate::gui::switch(ui, &mut on).changed() {
                        Self::set(o, k, on as i32);
                        return true;
                    }
                    ui.label(egui::RichText::new(s(texts.get(13, 3, if on { 1 } else { 2 }))).weak());
                    return false;
                }
                let mut pick = None;
                egui::ComboBox::from_id_salt(("row", k)).width(field.x).height(320.0).selected_text(s(value_text(texts, r, v))).show_ui(ui, |ui| {
                    for n in 0..=r.states {
                        if ui.add(egui::Button::selectable(n == v, s(value_text(texts, r, n)))).clicked() {
                            pick = Some(n);
                        }
                    }
                });
                if let Some(n) = pick.filter(|&n| n != v) {
                    Self::set(o, k, n);
                    return true;
                }
            }
            Item::Slider(i) => {
                let mut v = o.sliders[i].value;
                // The value in a column of its own, so the track doesn't move with it.
                ui.add_sized([44.0, 30.0], egui::Label::new(format!("{v}%")));
                ui.spacing_mut().slider_width = CONTROL - 64.0;
                let r = ui.add(egui::Slider::new(&mut v, 0..=100).show_value(false));
                if r.changed() {
                    o.sliders[i].value = v;
                }
                if r.drag_started() {
                    sound(crate::audio::CLICK);
                }
            }
            Item::Name => {
                let caret = self.editing && (ui.input(|x| x.time) * 2.0) as u64 % 2 == 0;
                let text = s(self.name.clone()) + if caret { "|" } else { " " };
                let stroke = if self.editing { egui::Stroke::new(1.5f32, ui.visuals().hyperlink_color) } else { ui.visuals().widgets.inactive.bg_stroke };
                let b = egui::Button::new(egui::RichText::new(text).monospace()).fill(ui.visuals().extreme_bg_color).stroke(stroke);
                if ui.add_sized(field, b).clicked() {
                    self.editing = true;
                    return true;
                }
            }
            Item::Bind(b) => {
                let (text, c) = match self.capture {
                    // The status line says what to do.
                    Some(c) if c == b => ("…".to_owned(), hot),
                    _ => (crate::bindings::key_name(self.binds.0[b]).unwrap_or("?").to_owned(), if self.binds.clash(b).is_some() { hot } else { ui.visuals().text_color() }),
                };
                if ui.add_sized([120.0, 30.0], egui::Button::new(egui::RichText::new(text).monospace().color(c))).clicked() {
                    self.capture = Some(b);
                    return true;
                }
            }
            Item::Reset => {
                let t = if self.binds.is_default() { "Original keys" } else { "Reset" };
                if ui.add_enabled(!self.binds.is_default(), egui::Button::new(s(texts.ours(t))).min_size(egui::vec2(120.0, 30.0))).clicked() {
                    self.binds = Default::default();
                    return true;
                }
            }
        }
        false
    }
}

/// "DISPLAY" → "Display" (the tab titles are the table's upper-case texts).
fn title_case(s: &str) -> String {
    let lower = s.to_lowercase();
    let mut c = lower.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> OptionsScreen {
        let texts = crate::ui::UiTexts { strings: Default::default(), kinds: Vec::new(), lang: ig2_formats::text::Lang::En };
        let mut rows: Vec<Row> = [("OptionsKellAutoSave", 2), ("Cutscenes", 2), ("Lowrestextures", 1), ("OptionsBuildingRes", 1)]
            .into_iter()
            .enumerate()
            .map(|(k, (key, page))| Row { key: key.into(), value: 1, states: if k == 3 { 2 } else { 1 }, page, id: 101 + k as i32, enabled: true, ..Row::default() })
            .collect();
        rows.extend(crate::display::rows(&Default::default(), &[(800, 600), (640, 480)], (800, 600)));
        let s = super::super::Settings { music: 80, sound: 60, gamma: 50, gamma_enabled: false, page: 0, slot: 1, gfx_detail: 0 };
        let mut o = OptionsScreen::enter(&texts, rows, &s, true, false, Vec::new());
        o.modern = Some(Modern::new(&o, b"Kim".to_vec(), Default::default()));
        o
    }

    /// Every setting sits on exactly one tab, in a card with a heading.
    #[test]
    fn every_setting_on_one_tab() {
        let o = screen();
        let all: Vec<Item> = Tab::ALL.iter().filter(|&&t| t != Tab::Controls).flat_map(|&t| items(&o.rows, t)).collect();
        assert_eq!(all.len(), o.rows.len() + 3 + 1);
        for (k, _) in o.rows.iter().enumerate() {
            assert_eq!(all.iter().filter(|&&i| i == Item::Row(k)).count(), 1, "{}", o.rows[k].key);
        }
        for t in Tab::ALL {
            assert!(!items(&o.rows, t).is_empty(), "{t:?}");
            // Every card has a heading in the remake text table.
            assert!(cards(&o, &items(&o.rows, t)).iter().all(|(h, _)| crate::tr::has(h)), "{t:?}");
        }
        assert_eq!(items(&o.rows, Tab::Sound)[..2], [Item::Slider(0), Item::Slider(1)]);
        assert_eq!(items(&o.rows, Tab::Controls).len(), crate::bindings::ACTIONS.len() + 1);
        assert_eq!(items(&o.rows, Tab::Game)[..2], [Item::Row(0), Item::Row(1)]);
    }

    /// Every setting has a description, and every text the screen writes is in the remake text table.
    #[test]
    fn descriptions_and_labels_are_in_the_table() {
        let o = screen();
        let mut keys: Vec<&str> = o.rows.iter().map(|r| r.key.as_str()).collect();
        keys.extend(["Music Volume", "Sound Volume", "Gamma Correction", NAME_KEY, "DebugData_grBattle", "DebugData_GrNoOverrun", "DebugData_Battle", "DebugData_SpNoOverrun", "VROT"]);
        keys.extend(["KELLKINYITASDEFAULT", "KellEpuletnevKiiras", "OptionsKellArnyek", "OptionsKellUt", "OptionsKellSpecFX", "OptionsKellIdojaras", "OptionsHiresVectors", "OptionsFogResolution"]);
        for k in keys {
            let s = describe(k);
            assert!(!s.is_empty() && crate::tr::has(s), "{k}: {s:?}");
        }
        for t in Tab::ALL {
            assert!(crate::tr::has(t.title()), "{t:?}");
        }
        for s in ["Leave; changes not applied are taken back.", "Save the changes and use them.", describe("Bind"), describe("ResetKeys")] {
            assert!(crate::tr::has(s), "{s}");
        }
        for a in crate::bindings::ACTIONS {
            assert!(crate::tr::has(a.name) && crate::tr::has(a.ctx.title()), "{}", a.name);
        }
    }

    /// Changes stay on the screen until Apply; Back takes back what wasn't applied; the stored file holds the values.
    #[test]
    fn apply_and_back() {
        let mut o = screen();
        let mut m = o.modern.take().unwrap();
        let mut quiet = |_| {};
        let vsync = o.rows.iter().position(|r| r.key == crate::display::VSYNC).unwrap();
        let at = items(&o.rows, Tab::Display).iter().position(|&i| i == Item::Row(vsync)).unwrap();
        m.focus = at;
        m.key(&mut o, Nav::Activate, &mut quiet);
        assert_eq!(o.rows[vsync].value, 0);
        assert!(m.pending(&o) && !o.dirty);
        // Apply: stored, nothing pending.
        m.focus = items(&o.rows, Tab::Display).len() + 1;
        m.key(&mut o, Nav::Activate, &mut quiet);
        assert!(o.dirty && !m.pending(&o));
        o.modern = Some(m.clone());
        let mut f = ig2_formats::options::Options::default();
        o.store(&mut f);
        assert_eq!((f.int(crate::display::VSYNC), f.get(NAME_KEY)), (Some(0), Some("Kim")));
        // A slider and the name changed, then Back: both taken back, the screen leaves.
        m.key(&mut o, Nav::NextTab, &mut quiet);
        m.key(&mut o, Nav::Right, &mut quiet);
        assert_eq!(o.sliders[0].value, 85);
        m.key(&mut o, Nav::PrevTab, &mut quiet);
        m.key(&mut o, Nav::PrevTab, &mut quiet);
        assert_eq!(m.tab, Tab::Language);
        m.key(&mut o, Nav::PrevTab, &mut quiet);
        m.key(&mut o, Nav::Activate, &mut quiet);
        assert!(m.editing);
        m.key(&mut o, Nav::Erase, &mut quiet);
        m.key(&mut o, Nav::Char(b'o'), &mut quiet);
        m.key(&mut o, Nav::Activate, &mut quiet);
        assert_eq!(m.name, b"Kio");
        m.key(&mut o, Nav::Back, &mut quiet);
        assert_eq!((o.sliders[0].value, m.name.as_slice(), o.rows[vsync].value), (80, &b"Kim"[..], 0));
        assert!(o.leaving());
    }

    /// The page draws every tab (egui without a GPU), and the keyboard's focus on Apply is the page's Apply.
    #[test]
    fn page_shows_every_tab() {
        let mut o = screen();
        let texts = crate::ui::UiTexts { strings: Default::default(), kinds: Vec::new(), lang: ig2_formats::text::Lang::En };
        let theme = crate::theme::Theme::modern();
        let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        for t in Tab::ALL {
            o.modern.as_mut().unwrap().set_tab(t);
            for _ in 0..2 {
                g.run((1280, 720), &crate::gui::style(&theme), |ctx| o.modern_page(ctx, &texts, &theme, &mut |_| {}));
            }
            assert_eq!(o.modern.as_ref().unwrap().tab, t);
        }
        assert!(!o.leaving());
    }

    /// A choice steps both ways and wraps; a disabled row ignores the keys.
    #[test]
    fn choices_wrap_and_disabled_rows_hold() {
        let mut o = screen();
        let mut m = o.modern.take().unwrap();
        let mut quiet = |_| {};
        let mode = o.rows.iter().position(|r| r.key == crate::display::MODE).unwrap();
        let res = o.rows.iter().position(|r| r.key == crate::display::RESOLUTION).unwrap();
        m.focus = 0;
        m.key(&mut o, Nav::Left, &mut quiet);
        assert_eq!(o.rows[mode].value, 2);
        m.key(&mut o, Nav::Right, &mut quiet);
        m.key(&mut o, Nav::Right, &mut quiet);
        assert_eq!(o.rows[mode].value, 1);
        assert!(!o.rows[res].enabled);
        m.focus = 1;
        let before = o.rows[res].value;
        m.key(&mut o, Nav::Right, &mut quiet);
        assert_eq!(o.rows[res].value, before);
    }
}
