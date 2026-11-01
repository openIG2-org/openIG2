//! Ours (ADR 0053): the main menu's Options screen. Every setting of the in-game screen (`OptionsScreen`'s rows,
//! sliders) and the multiplayer name in seven tabs with switches, choices, sliders and a description line; Apply
//! writes them through `OptionsScreen::store`, Back takes back what was not applied. Drawn with the kit's faders,
//! filled quads and the game's font in the 640×480 virtual pixels; its colours and font sizes are the UI theme's (ADR 0056).

use super::{check_dependencies, Act, Draw, Group, OptionsScreen, Row};
use crate::font::Align;
use crate::render::{kind, Blend, Frame, Instance};

/// A setting's line.
const ROW_H: f32 = 17.0;
const PANEL: [f32; 4] = [20.0, 12.0, 600.0, 456.0];
const TABS_Y: f32 = 44.0;
const TAB: [f32; 3] = [26.0, 84.0, 82.0];
const LIST_Y: f32 = 72.0;
/// The settings a tab may hold (the list ends above the description line).
#[cfg(test)]
const MAX_ITEMS: usize = 19;
const LABEL_X: f32 = 40.0;
const CTRL: [f32; 2] = [392.0, 600.0];
const DESC_Y: f32 = 408.0;
const BACK: [f32; 4] = [36.0, 432.0, 130.0, 22.0];
const APPLY: [f32; 4] = [474.0, 432.0, 130.0, 22.0];
/// `MultiplayerNev`'s length (`IOMaxLen` 9 in the menu's name entry, mainscr.rs).
const NAME_MAX: usize = 9;
const NAME_KEY: &str = "MultiplayerNev";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Display,
    Sound,
    Interface,
    Game,
    Accessibility,
    Multiplayer,
    Language,
}

impl Tab {
    pub const ALL: [Tab; 7] = [Tab::Display, Tab::Sound, Tab::Interface, Tab::Game, Tab::Accessibility, Tab::Multiplayer, Tab::Language];

    /// The label (the remake text table's key).
    pub fn title(self) -> &'static str {
        ["DISPLAY", "SOUND", "INTERFACE", "GAME", "ACCESSIBILITY", "MULTIPLAYER", "LANGUAGE"][self as usize]
    }

    fn group(self) -> Option<Group> {
        match self {
            Tab::Display => Some(Group::Display),
            Tab::Sound => None,
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
}

/// The tab's settings in order (ADR 0053 §2): Display the remake's rows, gamma, the exe's graphics rows, the modern
/// lighting's (ADR 0055); Sound the
/// two volumes; Game the exe's game rows, then ours; Multiplayer the name, then ours; the rest the group's rows.
pub fn items(rows: &[Row], tab: Tab) -> Vec<Item> {
    let ours = |g| rows.iter().enumerate().filter(move |(_, r)| r.ours && r.group == g).map(|(k, _)| Item::Row(k));
    let exe = |page| rows.iter().enumerate().filter(move |(_, r)| !r.ours && r.page == page).map(|(k, _)| Item::Row(k));
    let mut v: Vec<Item> = match tab {
        Tab::Sound => return vec![Item::Slider(0), Item::Slider(1)],
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
        "Sound Volume" => "The loudness of the sound effects and speech.",
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
        "BattleEffects" => "Sparks, shock waves, smoke and glowing shots on top of the battles' own effects.",
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

/// What is under the pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Spot {
    Tab(Tab),
    /// A setting and where on its line: -1 the choice's left arrow, 1 its right arrow or the switch, 0 elsewhere.
    Item(usize, i32),
    Back,
    Apply,
}

/// The screen's own state; the values are the `OptionsScreen`'s.
#[derive(Debug, Clone)]
pub struct Modern {
    pub tab: Tab,
    /// The focused line: a setting of the tab, then Back, then Apply.
    pub focus: usize,
    hover: Option<Spot>,
    /// The slider being dragged.
    drag: Option<usize>,
    pub editing: bool,
    /// `MultiplayerNev` in the font's bytes.
    pub name: Vec<u8>,
    /// The applied values Back puts back: the rows', the sliders', the name.
    kept: (Vec<i32>, [i32; 3], Vec<u8>),
}

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    crate::kit::inside_f(r, at)
}

fn tab_box(k: usize) -> [f32; 4] {
    [TAB[0] + k as f32 * TAB[1], TABS_Y, TAB[2], 20.0]
}

fn line_y(i: usize) -> f32 {
    LIST_Y + i as f32 * ROW_H
}

/// The slider's track: x, width.
const TRACK: [f32; 2] = [CTRL[0] + 4.0, 160.0];

impl Modern {
    pub fn new(o: &OptionsScreen, name: Vec<u8>) -> Self {
        let mut m = Self { tab: Tab::Display, focus: 0, hover: None, drag: None, editing: false, name, kept: Default::default() };
        m.keep(o);
        m
    }

    fn values(&self, o: &OptionsScreen) -> (Vec<i32>, [i32; 3], Vec<u8>) {
        (o.rows.iter().map(|r| r.value).collect(), [0, 1, 2].map(|i| o.sliders[i].value), self.name.clone())
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

    fn spot(&self, o: &OptionsScreen, at: [f32; 2]) -> Option<Spot> {
        if let Some(k) = (0..Tab::ALL.len()).find(|&k| inside(tab_box(k), at)) {
            return Some(Spot::Tab(Tab::ALL[k]));
        }
        if inside(BACK, at) {
            return Some(Spot::Back);
        }
        if inside(APPLY, at) {
            return Some(Spot::Apply);
        }
        let n = self.items(o).len();
        let i = (0..n).find(|&i| inside([32.0, line_y(i), 576.0, ROW_H], at))?;
        let part = if at[0] >= CTRL[1] - 18.0 { 1 } else if (CTRL[0]..CTRL[0] + 18.0).contains(&at[0]) { -1 } else { 0 };
        Some(Spot::Item(i, part))
    }

    fn enabled(o: &OptionsScreen, it: Item) -> bool {
        match it {
            Item::Row(k) => o.rows[k].enabled,
            Item::Slider(i) => o.sliders[i].enabled,
            Item::Name => true,
        }
    }

    /// Step a row's value by `d`, wrapping as the in-game press does.
    fn step(o: &mut OptionsScreen, k: usize, d: i32) {
        let r = &mut o.rows[k];
        let n = r.states + 1;
        r.value = (r.value + d).rem_euclid(n.max(1));
        if !r.ours {
            o.gfx_detail = 3;
        }
        check_dependencies(&mut o.rows, o.multiplayer);
    }

    fn slider_to(o: &mut OptionsScreen, i: usize, x: f32) {
        o.sliders[i].value = (((x - TRACK[0]) / TRACK[1]) * 100.0).round().clamp(0.0, 100.0) as i32;
    }

    pub fn hover(&mut self, o: &mut OptionsScreen, at: [f32; 2]) {
        self.hover = self.spot(o, at);
        if let Some(i) = self.drag {
            Self::slider_to(o, i, at[0]);
        }
    }

    pub fn press(&mut self, o: &mut OptionsScreen, at: [f32; 2], sound: &mut dyn FnMut(crate::audio::SoundId)) -> Option<Act> {
        self.hover = self.spot(o, at);
        self.editing = false;
        let spot = self.hover?;
        match spot {
            Spot::Tab(t) => self.set_tab(t),
            Spot::Back => self.back(o),
            Spot::Apply if !self.pending(o) => return None,
            Spot::Apply => self.apply(o),
            Spot::Item(i, part) => {
                self.focus = i;
                let it = self.items(o)[i];
                // The value's side only: a press on the label just focuses.
                if !Self::enabled(o, it) || at[0] < CTRL[0] {
                    return None;
                }
                self.editing = it == Item::Name;
                match it {
                    Item::Row(k) => Self::step(o, k, if part < 0 { -1 } else { 1 }),
                    Item::Slider(s) if at[0] >= CTRL[0] => {
                        Self::slider_to(o, s, at[0]);
                        self.drag = Some(s);
                    }
                    _ => {}
                }
            }
        }
        sound(crate::audio::CLICK);
        None
    }

    pub fn release(&mut self) {
        self.drag = None;
    }

    fn set_tab(&mut self, t: Tab) {
        self.tab = t;
        self.focus = 0;
        self.editing = false;
        self.drag = None;
        self.hover = None;
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
        check_dependencies(&mut o.rows, o.multiplayer);
        if o.leaving.is_none() {
            o.leave(if o.load_only { Act::MainMenu } else { Act::Back });
        }
    }

    pub fn key(&mut self, o: &mut OptionsScreen, k: Nav, sound: &mut dyn FnMut(crate::audio::SoundId)) {
        self.hover = None;
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
            Nav::Up => self.focus = (self.focus + lines - 1) % lines,
            Nav::Down => self.focus = (self.focus + 1) % lines,
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

// The draw.

fn quad(f: &mut Frame, d: &Draw, r: [f32; 4], c: [f32; 3], a: f32) {
    let v = &d.v;
    f.push(d.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0), [r[2] * v.k / 2.0, r[3] * v.k / 2.0], [c[0], c[1], c[2], a]));
}

#[allow(clippy::too_many_arguments)]
fn text(f: &mut Frame, d: &Draw, s: &[u8], x: f32, y: f32, size: usize, align: Align, c: [f32; 3]) {
    match align {
        Align::Right => d.font.text(f, &d.v, s, x - s.len() as f32 * d.font.advance(size), y, 0, size, Align::Left, c, None),
        _ => d.font.text(f, &d.v, s, x, y, 0, size, align, c, None),
    }
}

fn boxed(f: &mut Frame, d: &Draw, r: [f32; 4], fill: [f32; 3], edge: [f32; 3], clear: [f32; 2]) {
    crate::kit::fader::fader(f, d.white, &d.v, r, fill, clear[0], edge, clear[1]);
}


impl Modern {
    /// A setting's label.
    fn label(o: &OptionsScreen, d: &Draw, it: Item) -> Vec<u8> {
        match it {
            Item::Row(k) => {
                let r = &o.rows[k];
                match exe_name(&r.key) {
                    _ if r.ours => d.texts.ours(&String::from_utf8_lossy(&r.name)),
                    Some(n) => d.texts.get(13, 2, n),
                    None => r.name.clone(),
                }
            }
            Item::Slider(i) => o.sliders[i].title.clone(),
            Item::Name => d.texts.ours("Player name"),
        }
    }

    fn key_of(o: &OptionsScreen, it: Item) -> &str {
        match it {
            Item::Row(k) => &o.rows[k].key,
            Item::Slider(i) => ["Music Volume", "Sound Volume", "Gamma Correction"][i],
            Item::Name => NAME_KEY,
        }
    }

    pub fn draw(&self, o: &OptionsScreen, f: &mut Frame, d: &Draw) {
        let t = &crate::theme::look(d);
        // Opaque: the menu's ring labels would show through.
        boxed(f, d, PANEL, t.panel, t.panel_edge, t.panel_clear);
        text(f, d, &d.texts.ours("OPTIONS"), 320.0, 22.0, t.big, Align::Centre, t.text);
        for (k, &tab) in Tab::ALL.iter().enumerate() {
            let on = tab == self.tab;
            let hot = self.hover == Some(Spot::Tab(tab));
            let r = tab_box(k);
            boxed(f, d, r, if on { t.tab_on } else { t.tab }, if on { t.panel_edge } else { t.tab }, t.button_clear);
            text(f, d, &d.texts.ours(tab.title()), r[0] + r[2] / 2.0, r[1] + 6.0, t.small, Align::Centre, if hot { t.hot } else if on { t.text } else { t.dim });
        }
        quad(f, d, [32.0, LIST_Y - 5.0, 576.0, 1.0], t.panel_edge.map(|c| c / 255.0), 1.0);
        let items = self.items(o);
        for (i, &it) in items.iter().enumerate() {
            self.draw_item(o, f, d, i, it);
        }
        // The Language tab: the restart notice (as on the in-game Remake page).
        let lang = items.iter().find_map(|&it| match it {
            Item::Row(k) if o.rows[k].key == crate::display::LANGUAGE => Some(o.rows[k].value),
            _ => None,
        });
        if lang.is_some() && o.lang_at_start.is_some_and(|v| Some(v) != lang) {
            text(f, d, &d.texts.ours("Restart to change the language"), LABEL_X, line_y(items.len()) + 8.0, t.small, Align::Left, t.hot);
        }
        quad(f, d, [32.0, DESC_Y - 6.0, 576.0, 1.0], t.panel_edge.map(|c| c / 255.0), 1.0);
        let desc = match self.hover.or(Some(Spot::Item(self.focus, 0))) {
            Some(Spot::Item(i, _)) => items.get(i).map(|&it| describe(Self::key_of(o, it))),
            Some(Spot::Back) => Some("Leave; changes not applied are taken back."),
            Some(Spot::Apply) => Some("Save the changes and use them."),
            _ => None,
        };
        if let Some(s) = desc.filter(|s| !s.is_empty()) {
            text(f, d, &d.texts.ours(s), 320.0, DESC_Y, t.small, Align::Centre, t.dim);
        }
        let pending = self.pending(o);
        for (r, label, spot, k, live) in [(BACK, "Back", Spot::Back, 0, true), (APPLY, "Apply", Spot::Apply, 1, pending)] {
            let focused = self.focus == items.len() + k;
            let hot = self.hover == Some(spot);
            boxed(f, d, r, if live { t.button } else { t.tab }, if focused || hot { t.panel_edge } else { t.tab }, t.button_clear);
            text(f, d, &d.texts.ours(label), r[0] + r[2] / 2.0, r[1] + 7.0, t.small, Align::Centre, if hot { t.hot } else if live { t.text } else { t.dim });
        }
    }

    fn draw_item(&self, o: &OptionsScreen, f: &mut Frame, d: &Draw, i: usize, it: Item) {
        let t = &crate::theme::look(d);
        let y = line_y(i);
        let ty = y + (ROW_H - d.font.height(t.small)) / 2.0;
        let live = Self::enabled(o, it);
        let hot = matches!(self.hover, Some(Spot::Item(h, _)) if h == i);
        if self.focus == i || hot {
            quad(f, d, [32.0, y, 576.0, ROW_H], t.on_track, if self.focus == i { 0.28 } else { 0.14 });
        }
        let c = if !live { t.off } else if hot { t.hot } else { t.text };
        text(f, d, &Self::label(o, d, it), LABEL_X, ty, t.small, Align::Left, c);
        let vc = if live { t.accent } else { t.off };
        match it {
            Item::Row(k) => {
                let r = &o.rows[k];
                let v = if r.locked { 0 } else { r.value };
                if r.states == 1 && r.labels.is_empty() {
                    // The switch: its track, the knob at the right when on.
                    let on = v != 0;
                    let tr = [CTRL[1] - 30.0, y + 3.0, 28.0, ROW_H - 6.0];
                    quad(f, d, tr, if !live { t.off_track } else if on { t.on_track } else { t.off_track }, 1.0);
                    let kx = if on { tr[0] + tr[2] - tr[3] } else { tr[0] };
                    quad(f, d, [kx + 1.0, tr[1] + 1.0, tr[3] - 2.0, tr[3] - 2.0], if live { t.knob } else { t.off }, 1.0);
                    let s = d.texts.get(13, 3, if on { 1 } else { 2 });
                    text(f, d, &s, tr[0] - 6.0, ty, t.small, Align::Right, vc);
                } else {
                    let s = match r.labels.get(v.max(0) as usize) {
                        Some(l) => d.texts.ours(&String::from_utf8_lossy(l)),
                        None if r.states == 2 => d.texts.get(13, 3, 3 + v.clamp(0, 2) as u16),
                        None => d.texts.get(13, 3, if v != 0 { 1 } else { 2 }),
                    };
                    text(f, d, b"<", CTRL[0] + 4.0, ty, t.small, Align::Left, vc);
                    text(f, d, b">", CTRL[1] - 4.0, ty, t.small, Align::Right, vc);
                    text(f, d, &s, (CTRL[0] + CTRL[1]) / 2.0, ty, t.small, Align::Centre, vc);
                }
            }
            Item::Slider(s) => {
                let v = o.sliders[s].value;
                let mid = y + ROW_H / 2.0;
                quad(f, d, [TRACK[0], mid - 1.5, TRACK[1], 3.0], t.off_track, 1.0);
                let x = TRACK[0] + TRACK[1] * v as f32 / 100.0;
                quad(f, d, [TRACK[0], mid - 1.5, x - TRACK[0], 3.0], if live { t.on_track } else { t.off_track }, 1.0);
                quad(f, d, [x - 3.0, y + 3.0, 6.0, ROW_H - 6.0], if live { t.knob } else { t.off }, 1.0);
                text(f, d, format!("{v}%").as_bytes(), CTRL[1] - 4.0, ty, t.small, Align::Right, vc);
            }
            Item::Name => {
                let r = [CTRL[0], y + 1.0, CTRL[1] - CTRL[0], ROW_H - 2.0];
                quad(f, d, r, t.off_track, 1.0);
                if self.editing {
                    quad(f, d, [r[0], r[1] + r[3] - 1.0, r[2], 1.0], t.accent, 1.0);
                }
                text(f, d, &self.name, r[0] + 6.0, ty, t.small, Align::Left, t.text);
                if self.editing && crate::kit::fader_design::blink(d.now_ms) {
                    let x = r[0] + 6.0 + self.name.len() as f32 * d.font.advance(t.small);
                    quad(f, d, [x, r[1] + 2.0, 1.0, r[3] - 4.0], t.text, 1.0);
                }
            }
        }
    }
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
        o.modern = Some(Modern::new(&o, b"Kim".to_vec()));
        o
    }

    /// Every setting sits on exactly one tab, and every tab fits above the description line.
    #[test]
    fn every_setting_on_one_tab() {
        let o = screen();
        let all: Vec<Item> = Tab::ALL.iter().flat_map(|&t| items(&o.rows, t)).collect();
        assert_eq!(all.len(), o.rows.len() + 3 + 1);
        for (k, _) in o.rows.iter().enumerate() {
            assert_eq!(all.iter().filter(|&&i| i == Item::Row(k)).count(), 1, "{}", o.rows[k].key);
        }
        for t in Tab::ALL {
            assert!(!items(&o.rows, t).is_empty(), "{t:?}");
            assert!(items(&o.rows, t).len() <= MAX_ITEMS && line_y(MAX_ITEMS) <= DESC_Y - 8.0, "{t:?}");
        }
        assert_eq!(items(&o.rows, Tab::Sound), [Item::Slider(0), Item::Slider(1)]);
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
        for s in ["Leave; changes not applied are taken back.", "Save the changes and use them."] {
            assert!(crate::tr::has(s));
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
