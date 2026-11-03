//! Ours (ADR 0053): the main menu's Options screen. Every setting of the in-game screen (`OptionsScreen`'s rows,
//! sliders) and the multiplayer name in five tabs of cards with toggles, segments, steppers, sliders and keys; Apply
//! writes them through `OptionsScreen::store`, Back takes back what was not applied. A designed egui page (ADR 0062,
//! the board's layout: ADR 0078) in the UI theme's colours (ADR 0056); the keyboard moves through it with the screen's
//! own keys. In a game the pause menu first: Continue, Save, Load, Options, Help, Quit and Exit, the save and load page
//! behind Save and Load (from the main menu's Load game that page alone) (ADR 0067).

use super::{check_dependencies, id, Act, Group, OptionsScreen, Row, SLOTS};
use crate::gui::ds::{self, Font, Text};
use crate::ui::UiTexts;

/// `MultiplayerNev`'s length (`IOMaxLen` 9 in the menu's name entry, mainscr.rs).
const NAME_MAX: usize = 9;
const NAME_KEY: &str = "MultiplayerNev";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Display,
    Accessibility,
    /// ADR 0058: the key bindings.
    Controls,
    Sound,
    Game,
    /// ADR 0067: the save and load page (no tab of the rail).
    Saves,
}

impl Tab {
    pub const ALL: [Tab; 5] = [Tab::Display, Tab::Accessibility, Tab::Controls, Tab::Sound, Tab::Game];

    /// The label (the remake text table's key).
    pub fn title(self) -> &'static str {
        ["DISPLAY", "ACCESSIBILITY", "CONTROLS", "SOUND", "GAME", "SAVE & LOAD"][self as usize]
    }

    /// The tab's cards in order (the remake text table's keys); Controls' are the actions' places.
    fn cards(self) -> &'static [&'static str] {
        match self {
            Tab::Display => &["Screen", "Graphics", "Interface", "Detail"],
            Tab::Accessibility => &["Readability", "Subtitles and colours", "Language"],
            Tab::Sound => &["Volume", "Music"],
            Tab::Game => &["Battles and saving", "Enhancements", "Handling"],
            Tab::Controls | Tab::Saves => &[],
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
    /// ADR 0067: a save slot (1..=24), an action of the save and load page.
    Slot(usize),
    Game(Game),
}

/// ADR 0067: the pause menu's entries and the save and load page's actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Game {
    Load,
    Save,
    /// To the main menu (asks first, but under the tutorial).
    Quit,
    /// The game quits (asks first).
    Exit,
    /// Back to the game (under the tutorial: to it).
    Continue,
    /// The settings' tabs.
    Options,
    /// The help screen over the game.
    Help,
}

impl Game {
    /// The entry's label (the remake text table's key).
    fn label(self, tutorial: bool) -> &'static str {
        match self {
            Game::Load => "Load",
            Game::Save => "Save",
            Game::Quit if tutorial => "Quit the tutorial",
            Game::Quit => "Quit to main menu",
            Game::Exit => "Exit game",
            Game::Continue if tutorial => "Continue tutorial",
            Game::Continue => "Continue",
            Game::Options => "Options",
            Game::Help => "Help",
        }
    }
}

/// ADR 0067: the pause menu's entries (the original's buttons, Save and Load on their page; Help and Exit ours).
pub fn menu_items(o: &OptionsScreen) -> Vec<Game> {
    if o.tutorial {
        vec![Game::Continue, Game::Options, Game::Quit]
    } else {
        // No help screen over a battle or a ground war (`open_help` refuses it there).
        let help = (!o.battle).then_some(Game::Help);
        [Game::Continue, Game::Save, Game::Load, Game::Options].into_iter().chain(help).chain([Game::Quit, Game::Exit]).collect()
    }
}

/// ADR 0067: the save and load page's lines: the autosaves when filled (24, 23), the slots the original lets you
/// pick (1..=22), then Save and Load (Load alone from the main menu).
pub fn game_items(o: &OptionsScreen) -> Vec<Item> {
    let slots = [24, 23].into_iter().filter(|&n| o.filled(n)).chain(1..=22).filter(|&n| n <= SLOTS).map(Item::Slot);
    let acts: &[Game] = if o.load_only { &[Game::Load] } else { &[Game::Save, Game::Load] };
    slots.chain(acts.iter().map(|&g| Item::Game(g))).collect()
}

/// Where a setting sits: its tab and card (the boards' grouping; a row the table doesn't name goes by its group).
fn place(rows: &[Row], it: Item) -> (Tab, &'static str) {
    use crate::display as d;
    match it {
        Item::Slider(2) => (Tab::Display, "Screen"),
        Item::Slider(_) => (Tab::Sound, "Volume"),
        Item::Name => (Tab::Game, "Handling"),
        Item::Bind(b) => (Tab::Controls, crate::bindings::ACTIONS[b].ctx.title()),
        Item::Reset => (Tab::Controls, ""),
        Item::Slot(_) | Item::Game(_) => (Tab::Saves, ""),
        Item::Row(k) => {
            let r = &rows[k];
            if !r.ours {
                return if r.page == 1 { (Tab::Display, "Detail") } else { (Tab::Game, "Battles and saving") };
            }
            match r.key.as_str() {
                d::UI_SCALE | d::SHARP_TEXT | d::SLIDER_UNDER | d::CLEAN_ENTRY => (Tab::Accessibility, "Readability"),
                d::WASD | d::STARMAP_MOUSE | d::FLEET_GROUPS | d::EVENT_LOG | crate::menu_flow::KEY => (Tab::Game, "Handling"),
                d::AUTOSAVE => (Tab::Game, "Battles and saving"),
                d::TITLE_MUSIC => (Tab::Sound, "Music"),
                d::TEXTURE_UPSCALE => (Tab::Display, "Graphics"),
                d::SHARP_UI => (Tab::Display, "Interface"),
                _ => match r.group {
                    Group::Display => (Tab::Display, "Screen"),
                    Group::Graphics => (Tab::Display, "Graphics"),
                    Group::Interface => (Tab::Display, "Interface"),
                    Group::Game => (Tab::Game, "Enhancements"),
                    Group::Accessibility => (Tab::Accessibility, "Subtitles and colours"),
                    Group::Language => (Tab::Accessibility, "Language"),
                    Group::Multiplayer => (Tab::Game, "Handling"),
                    Group::Sound => (Tab::Sound, "Volume"),
                },
            }
        }
    }
}

/// The tab's settings in order: card by card (the tab's order; Controls' in the actions' order), within a card the
/// volumes, the rows, the gamma, the name; the keys' reset last.
pub fn items(rows: &[Row], tab: Tab) -> Vec<Item> {
    if tab == Tab::Saves {
        return Vec::new();
    }
    let all = [Item::Slider(0), Item::Slider(1)].into_iter().chain((0..rows.len()).map(Item::Row)).chain([Item::Slider(2), Item::Name]).chain((0..crate::bindings::ACTIONS.len()).map(Item::Bind)).chain([Item::Reset]);
    let mut v: Vec<(usize, Item)> = all.filter_map(|it| {
        let (t, card) = place(rows, it);
        let order = match t {
            Tab::Controls if card.is_empty() => usize::MAX,
            Tab::Controls => crate::bindings::ACTIONS.iter().position(|a| a.ctx.title() == card).unwrap_or(0),
            _ => tab.cards().iter().position(|&c| c == card).unwrap_or(usize::MAX),
        };
        (t == tab).then_some((order, it))
    }).collect();
    v.sort_by_key(|&(o, _)| o);
    v.into_iter().map(|(_, it)| it).collect()
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
        "StarmapMouseCamera" => "The wheel zooms the starmap in and out.",
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
        "UiScale" => "The size of the remake's own pages and panels; they grow with the window.",
        "ModernStarmapHud" => "The starmap's bar, info box, minimap and camera buttons as clear modern panels; off = the original's.",
        "ModernGameMenu" => "Esc's in-game menu with saving, loading and every setting as a modern page; off = the original's.",
        "ModernResearch" => "The research screen and the starmap's production view as modern panels; off = the original's.",
        "ModernColony" => "The colony's build menu, info card, buttons, radar and ground war panels as modern panels; off = the original's.",
        "ModernInfo" => "The info screen's empires, sortable planet and fleet list, details, money, charts and star log as modern panels; off = the original's.",
        "ModernCursors" => "Sharp pointers in the design's colours, animated, with a modern UI theme; off = the original's.",
        "ModernMainMenu" => "The main menu as a modern grouped menu with Continue; off = the original's rings.",
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

/// A script's key name (`--script`) as the screen's key (Space a letter while a text is typed, as the window's).
pub fn nav(name: &str, typing: bool) -> Option<Nav> {
    Some(match name.to_ascii_lowercase().as_str() {
        "escape" | "esc" => Nav::Back,
        "up" => Nav::Up,
        "down" => Nav::Down,
        "left" => Nav::Left,
        "right" => Nav::Right,
        "tab" | "pagedown" => Nav::NextTab,
        "pageup" => Nav::PrevTab,
        "space" if typing => Nav::Char(b' '),
        "enter" | "return" | "space" => Nav::Activate,
        "backspace" => Nav::Erase,
        c if c.len() == 1 => Nav::Char(name.as_bytes()[0]),
        _ => return None,
    })
}

/// The screen's own state; the values are the `OptionsScreen`'s.
#[derive(Debug, Clone)]
pub struct Modern {
    pub tab: Tab,
    /// The focused line: a setting of the tab, then Back, then Apply; on the pause menu its entry.
    pub focus: usize,
    /// The last input was a key: the focus is marked and kept in view.
    keyed: bool,
    /// The focused line last scrolled into view.
    shown: Option<(Tab, usize)>,
    /// The line under the pointer (its description is the footer's).
    hovered: Option<usize>,
    pub editing: bool,
    /// `MultiplayerNev` in the font's bytes.
    pub name: Vec<u8>,
    /// ADR 0058: the keys, the action waiting for its new key.
    pub binds: crate::bindings::Bindings,
    pub capture: Option<usize>,
    /// The applied values Back puts back: the rows', the sliders', the name, the keys.
    kept: (Vec<i32>, [i32; 3], Vec<u8>, crate::bindings::Bindings),
    /// ADR 0067: the in-game page, its pause menu up, the question up (Quit, Exit), the save asked for.
    pub game: bool,
    pub menu: bool,
    pub ask: Option<Game>,
    pub out: Option<Act>,
    /// The UI scale row's value the egui layer last showed (ADR 0062).
    previewed: Option<i32>,
}

impl Modern {
    pub fn new(o: &OptionsScreen, name: Vec<u8>, binds: crate::bindings::Bindings) -> Self {
        let mut m = Self { tab: Tab::Display, focus: 0, keyed: false, shown: None, hovered: None, editing: false, name, binds, capture: None, kept: Default::default(), game: false, menu: false, ask: None, out: None, previewed: None };
        m.keep(o);
        m.previewed = scale_row(o);
        m
    }

    /// ADR 0067: the page as the in-game menu (the pause menu first); from the main menu's Load game the save and load
    /// page with the selected slot focused.
    pub fn in_game(o: &OptionsScreen, name: Vec<u8>, binds: crate::bindings::Bindings) -> Self {
        let mut m = Self { game: true, menu: !o.load_only, ..Self::new(o, name, binds) };
        if o.load_only {
            m.open_saves(o);
        }
        m
    }

    /// The rail's tabs.
    pub fn tabs(&self) -> Vec<Tab> {
        Tab::ALL.to_vec()
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
        if self.tab == Tab::Saves {
            return game_items(o);
        }
        items(&o.rows, self.tab)
    }

    fn enabled(o: &OptionsScreen, it: Item) -> bool {
        match it {
            Item::Row(k) => o.rows[k].enabled,
            Item::Slider(i) => o.sliders[i].enabled,
            // The original's gates (`Options_MouseInit2`): Load a filled slot, not in MP; Save not loading only, in a
            // battle, in MP but for the host, on 23 and 24.
            Item::Game(Game::Load) => o.spot_on(id::LOAD),
            Item::Game(Game::Save) => o.spot_on(id::SAVE) && o.entry.is_none(),
            Item::Name | Item::Bind(_) | Item::Reset | Item::Slot(_) | Item::Game(_) => true,
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

    /// The save and load page, the selected slot focused.
    fn open_saves(&mut self, o: &OptionsScreen) {
        self.set_tab(Tab::Saves);
        self.menu = false;
        self.focus = game_items(o).iter().position(|&it| it == Item::Slot(o.slot)).unwrap_or(0);
    }

    /// Back to the pause menu, on `g`.
    fn back_to_menu(&mut self, o: &OptionsScreen, g: Game) {
        self.menu = true;
        self.focus = menu_items(o).iter().position(|&x| x == g).unwrap_or(0);
    }

    /// Apply: the values into the settings file (the window stores and applies them on `dirty`).
    fn apply(&mut self, o: &mut OptionsScreen) {
        if self.name.is_empty() {
            self.name = self.kept.2.clone();
        }
        self.keep(o);
        o.dirty = true;
    }

    /// Back: what wasn't applied taken back, then the screen leaves (as Esc on the in-game screen); in a game back to
    /// the pause menu first.
    fn back(&mut self, o: &mut OptionsScreen) {
        self.revert(o);
        if self.game && !self.menu && !o.load_only {
            let at = if self.tab == Tab::Saves { Game::Save } else { Game::Options };
            self.back_to_menu(o, at);
            return;
        }
        if o.leaving.is_none() {
            o.leave(if o.load_only { Act::MainMenu } else { Act::Back });
        }
    }

    /// What wasn't applied taken back.
    fn revert(&mut self, o: &mut OptionsScreen) {
        self.editing = false;
        for (r, &v) in o.rows.iter_mut().zip(&self.kept.0) {
            r.value = v;
        }
        for (s, &v) in o.sliders.iter_mut().zip(&self.kept.1) {
            s.value = v;
        }
        self.name = self.kept.2.clone();
        self.binds = self.kept.3.clone();
        (self.capture, self.ask, o.entry) = (None, None, None);
        check_dependencies(&mut o.rows, o.multiplayer);
        self.preview_scale(o);
    }

    /// ADR 0067: a pause menu entry or a save and load action (a button, Enter on its line). False when it is off.
    fn act(&mut self, o: &mut OptionsScreen, g: Game) -> bool {
        // From the menu Save and Load open their page; the page's Save while the name is typed saves it.
        let open = (self.menu && matches!(g, Game::Load | Game::Save)) || (g == Game::Save && o.entry.is_some());
        if !Self::enabled(o, Item::Game(g)) && !open {
            return false;
        }
        match g {
            // From the pause menu Save and Load open their page (Save with the name entry when the slot allows it).
            Game::Load | Game::Save if self.menu => {
                self.open_saves(o);
                if g == Game::Save && Self::enabled(o, Item::Game(Game::Save)) {
                    o.start_entry();
                }
            }
            Game::Load => self.leave(o, Act::Load(o.slot)),
            Game::Save if o.entry.is_some() => self.out = o.entry_key(super::EntryKey::Enter),
            Game::Save => o.start_entry(),
            Game::Options => {
                self.set_tab(Tab::Display);
                self.menu = false;
            }
            Game::Help => self.leave(o, Act::Help),
            // Under the tutorial no question (`Options_Kilep`: `TutorialStop(1)`, `ExitToMainScreen`).
            Game::Quit if o.tutorial => self.leave(o, Act::MainMenu),
            Game::Quit | Game::Exit => self.ask = Some(g),
            Game::Continue => self.back(o),
        }
        true
    }

    /// The question's yes.
    fn confirm(&mut self, o: &mut OptionsScreen) {
        match self.ask.take() {
            Some(Game::Quit) => self.leave(o, Act::MainMenu),
            Some(Game::Exit) => self.leave(o, Act::Exit),
            _ => {}
        }
    }

    /// Load, Quit, Exit, Help: what wasn't applied taken back, then the screen goes.
    fn leave(&mut self, o: &mut OptionsScreen, a: Act) {
        self.revert(o);
        o.leave(a);
    }

    /// ADR 0067: the save name's keys (`Options_EnterPressed` / `Options_EscPressed`): Enter saves, Esc cancels.
    fn entry_key(&mut self, o: &mut OptionsScreen, k: Nav) {
        let key = match k {
            Nav::Char(c) if c >= 0x20 => super::EntryKey::Char(c),
            Nav::Erase => super::EntryKey::Back,
            Nav::Left => super::EntryKey::Left,
            Nav::Right => super::EntryKey::Right,
            Nav::Activate => super::EntryKey::Enter,
            Nav::Back => {
                o.entry = None;
                return;
            }
            _ => return,
        };
        self.out = o.entry_key(key);
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
        if o.entry.is_some() {
            self.entry_key(o, k);
            return;
        }
        if self.ask.is_some() {
            match k {
                Nav::Activate => {
                    self.confirm(o);
                    sound(crate::audio::CLICK);
                }
                Nav::Back => self.ask = None,
                _ => {}
            }
            return;
        }
        if self.menu {
            let entries = menu_items(o);
            match k {
                Nav::Up => self.focus = (self.focus + entries.len() - 1) % entries.len(),
                Nav::Down => self.focus = (self.focus + 1) % entries.len(),
                Nav::Back => {
                    self.back(o);
                    sound(crate::audio::CLICK);
                }
                Nav::Activate if self.act(o, entries[self.focus.min(entries.len() - 1)]) => sound(crate::audio::CLICK),
                _ => {}
            }
            return;
        }
        let items = self.items(o);
        // The save and load page has Back alone after its lines.
        let tail = if self.tab == Tab::Saves { 1 } else { 2 };
        let lines = items.len() + tail;
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
        let tabs = self.tabs();
        let tab = tabs.iter().position(|&t| t == self.tab);
        match k {
            Nav::NextTab | Nav::PrevTab if tab.is_none() => {}
            Nav::NextTab => self.set_tab(tabs[(tab.unwrap() + 1) % tabs.len()]),
            Nav::PrevTab => self.set_tab(tabs[(tab.unwrap() + tabs.len() - 1) % tabs.len()]),
            Nav::Up | Nav::Down => {
                self.focus = (self.focus + if k == Nav::Up { lines - 1 } else { 1 }) % lines;
                // A slot focused is the slot picked.
                if let Some(&Item::Slot(n)) = items.get(self.focus) {
                    o.slot = n;
                }
            }
            Nav::Back => self.back(o),
            Nav::Left | Nav::Right | Nav::Activate => {
                let d = if k == Nav::Left { -1 } else { 1 };
                match (self.focus.checked_sub(items.len()), items.get(self.focus)) {
                    (Some(0), _) if k == Nav::Activate => self.back(o),
                    (Some(1), _) if k == Nav::Activate && self.pending(o) => self.apply(o),
                    (Some(1), _) if k == Nav::Activate => return,
                    (Some(_), _) => self.focus = items.len() + usize::from(k == Nav::Right && tail == 2),
                    (None, Some(&it)) if Self::enabled(o, it) => match it {
                        Item::Row(r) => Self::step(o, r, d),
                        Item::Slider(s) if k != Nav::Activate => o.sliders[s].value = (o.sliders[s].value + 5 * d).clamp(0, 100),
                        Item::Name if k == Nav::Activate => self.editing = true,
                        Item::Bind(b) if k == Nav::Activate => self.capture = Some(b),
                        Item::Reset if k == Nav::Activate => self.binds = Default::default(),
                        // Enter on a slot: Load a filled one, else Save into it.
                        Item::Slot(_) if k == Nav::Activate => {
                            if !self.act(o, Game::Load) && !self.act(o, Game::Save) {
                                return;
                            }
                        }
                        Item::Game(g) if k == Nav::Activate => {
                            if !self.act(o, g) {
                                return;
                            }
                        }
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

// The page (ADR 0062, the boards' layout ADR 0078): egui over the main menu's scene or the game.

/// The tab's settings in cards: (heading, the settings' indices into the tab's items); the keys' reset in none.
fn cards(rows: &[Row], items: &[Item]) -> Vec<(&'static str, Vec<usize>)> {
    let mut v: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (i, &it) in items.iter().enumerate() {
        let (_, card) = place(rows, it);
        match v.last_mut() {
            _ if card.is_empty() => {}
            Some(last) if last.0 == card => last.1.push(i),
            _ => v.push((card, vec![i])),
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
                // The exe's own English-only names (Cutscenes, View Rotation) through the remake's table.
                None => texts.ours(&String::from_utf8_lossy(&r.name)),
            }
        }
        Item::Slider(i) => o.sliders[i].title.clone(),
        Item::Name => texts.ours("Player name"),
        Item::Bind(b) => texts.ours(crate::bindings::ACTIONS[b].name),
        Item::Reset => texts.ours("Original keys"),
        Item::Slot(n) => n.to_string().into_bytes(),
        Item::Game(g) => texts.ours(g.label(o.tutorial)),
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
        Item::Slot(_) | Item::Game(_) => "",
    }
}

/// How a row is set: a toggle, segments (`wide`: under its label, the segments sharing the card's width), a stepper,
/// the UI scale's slider.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Toggle,
    Segments { wide: bool },
    Stepper,
    Scale,
}

fn kind(r: &Row, texts: &UiTexts) -> Kind {
    if r.key == crate::display::UI_SCALE {
        return Kind::Scale;
    }
    if r.states == 1 && r.labels.is_empty() {
        return Kind::Toggle;
    }
    if r.states > 3 {
        return Kind::Stepper;
    }
    // Three or four values with long words get the row's width (the board's display mode, autosave, WASD).
    let letters: usize = (0..=r.states).map(|v| value_text(texts, r, v).len()).sum();
    Kind::Segments { wide: r.states >= 2 && letters > 12 }
}

/// A line's height (`.orow` 50, `.krow` 40, a wide row 50 + the segments' 36).
fn line_height(o: &OptionsScreen, texts: &UiTexts, it: Item) -> f32 {
    match it {
        Item::Bind(_) => 40.0,
        Item::Row(k) if matches!(kind(&o.rows[k], texts), Kind::Segments { wide: true }) => 92.0,
        _ => 50.0,
    }
}

/// The board's panel (1640 × 920 at 1920 × 1080), its rail and the cards' gaps.
const PAGE: [f32; 2] = [1640.0, 920.0];
const RAIL: f32 = 280.0;
const GAP: f32 = 20.0;
/// The save and load board's panel and its right column.
const SAVES_PAGE: [f32; 2] = [1440.0, 880.0];
const ASIDE: f32 = 440.0;
/// The pause menu's panel width.
const MENU_W: f32 = 520.0;

impl Modern {
    /// One frame of the page: draws it and takes its pointer input.
    pub fn show(&mut self, o: &mut OptionsScreen, ctx: &egui::Context, texts: &UiTexts, _theme: &crate::theme::Theme, sound: &mut dyn FnMut(crate::audio::SoundId)) {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let tr = |en: &str| s(texts.ours(en));
        if ctx.input(|i| i.pointer.any_pressed()) {
            // A press anywhere ends the name entry and the wait for a key, as a click on the old screen did.
            (self.editing, self.capture, self.keyed) = (false, None, false);
        }
        let click = if self.menu {
            self.pause_menu(o, ctx, texts)
        } else if self.tab == Tab::Saves {
            self.saves(o, ctx, texts)
        } else {
            self.settings(o, ctx, texts, sound)
        };
        // Quit and Exit ask first.
        let g = self.ask.unwrap_or(Game::Quit);
        let (q, yes) = if g == Game::Exit { ("Exit the game? Play since the last save is lost.", "Exit") } else { ("Quit to the main menu? Play since the last save is lost.", "Quit") };
        match ds::confirm(ctx, "options-ask", self.ask.is_some(), &tr(g.label(false)), &tr(q), &tr(yes), &tr("Cancel")) {
            Some(true) => self.confirm(o),
            Some(false) => self.ask = None,
            None => {}
        }
        if click {
            sound(crate::audio::CLICK);
        }
        // The UI scale shows at once (a drag resizes the page when it ends, so the track stays under the pointer).
        if ctx.dragged_id().is_none() {
            self.preview_scale(o);
        }
    }

    /// The page's UI scale row into the egui layer when it changed: what the slider shows is what the next frame draws.
    fn preview_scale(&mut self, o: &OptionsScreen) {
        let v = scale_row(o);
        if let (Some(n), true) = (v, v != self.previewed) {
            crate::gui::set_ui_scale(50 + 10 * n.clamp(0, 15) as u32);
            self.previewed = v;
        }
    }

    /// ADR 0067: the pause menu (the in-game menu board: a 520-point deep panel, PAUSE, the last save, the entries
    /// with their keys). True when something was clicked.
    fn pause_menu(&mut self, o: &mut OptionsScreen, ctx: &egui::Context, texts: &UiTexts) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let tr = |en: &str| s(texts.ours(en));
        let k = ds::tokens();
        let screen = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));
        let entries = menu_items(o);
        let labels: Vec<String> = entries.iter().map(|g| tr(g.label(o.tutorial))).collect();
        let key = |id: &str| crate::bindings::ACTIONS.iter().position(|a| a.id == id).and_then(|i| crate::bindings::key_name(self.binds.0[i])).unwrap_or("");
        let hints: Vec<Option<&str>> = entries.iter().map(|g| match g {
            Game::Continue => Some("ESC"),
            Game::Save => Some(key("QuickSave")),
            Game::Options => Some(key("Options")),
            Game::Help => Some(key("Help")),
            _ => None,
        }).collect();
        // The last save: how long ago, its name.
        let last = o.slots.iter().flatten().filter_map(|sl| Some((std::fs::metadata(&sl.file).ok()?.modified().ok()?, sl))).max_by_key(|(t, _)| *t).map(|(_, sl)| sl);
        let sub = last.and_then(|sl| Some(format!("{}  ·  {}", s(ago(&sl.file, texts.lang)?), s(sl.name.clone()))));
        let n_items = entries.len() as f32;
        let quits = entries.iter().filter(|g| matches!(g, Game::Quit | Game::Exit)).count();
        let h = 28.0 + 34.0 + 4.0 + if sub.is_some() { 20.0 } else { 0.0 } + 18.0 + n_items * 52.0 + (n_items - 1.0) * 4.0 + 25.0 + 20.0;
        let size = egui::vec2(MENU_W, h).min(screen.size() - egui::vec2(16.0, 16.0));
        let t = ds::open_t(ctx, egui::Id::new("pause-menu"), true);
        let top = (screen.center().y - size.y / 2.0 - 50.0).max(screen.top() + 8.0);
        let mut picked = None;
        egui::Area::new(egui::Id::new("pause-menu")).fixed_pos(egui::pos2(screen.center().x - size.x / 2.0, top + (1.0 - t) * 12.0)).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(t);
            let at = ui.painter().add(egui::Shape::Noop);
            let r = egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 20, top: 28, bottom: 20 }).show(ui, |ui| {
                ui.set_width(size.x - 40.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                let w = ui.available_width();
                let title = Font::PAGE_TITLE.sized(28.0).tracking(0.3);
                Text::new(&tr("Pause"), Font { line: 34.0, ..title }).color(k.ink_silver).show(ui, egui::vec2(w, 38.0));
                if let Some(sub) = &sub {
                    let f = Font::DATA.sized(14.0).tracking(0.08);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        Text::new(sub, Font { line: 20.0, ..f }).color(k.info).show(ui, egui::vec2(w - 40.0, 20.0));
                    });
                }
                ui.add_space(18.0);
                let style = ds::MenuStyle { font: ds::MenuStyle::MENU.font.sized(20.0), height: 52.0 };
                let split = entries.len() - quits;
                let mk = |r: std::ops::Range<usize>| -> Vec<ds::MenuEntry> { r.map(|i| ds::MenuEntry { hint: hints[i], ..ds::MenuEntry::new(&labels[i]) }).collect() };
                let focus = |base: usize| self.focus.checked_sub(base).filter(|_| self.keyed);
                // The plate on the focused entry (the board's `.sel`), on Continue under the pointer.
                if let Some(i) = ds::menu(ui, "pause-a", style, &mk(0..split), Some(self.focus).filter(|&f| f < split), focus(0)) {
                    picked = Some(i);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ds::separator(ui, 0.0);
                });
                ui.add_space(4.0);
                if let Some(i) = ds::menu(ui, "pause-b", style, &mk(split..entries.len()), self.focus.checked_sub(split), focus(split)) {
                    picked = Some(split + i);
                }
            });
            ds::behind(ui, at, |p| ds::paint_panel(p, r.response.rect, ds::PanelKind::Deep, 0.90));
        });
        match picked {
            Some(i) => {
                self.focus = i;
                self.act(o, entries[i])
            }
            None => false,
        }
    }

    /// The settings' page (the options boards): the title, the rail of tabs, the tab's cards in columns, the footer's
    /// status line, Original keys (Controls), Back and Apply. True when something was clicked.
    fn settings(&mut self, o: &mut OptionsScreen, ctx: &egui::Context, texts: &UiTexts, sound: &mut dyn FnMut(crate::audio::SoundId)) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let tr = |en: &str| s(texts.ours(en));
        let k = ds::tokens();
        let mut click = false;
        let dim = if self.game { 0.55 } else { 0.5 };
        crate::gui::board(ctx, "options", egui::Vec2::from(PAGE), dim, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let w = ui.available_width();
            Text::new(&tr("OPTIONS"), Font::PAGE_TITLE).color(k.ink_silver).show(ui, egui::vec2(w, 36.0));
            ui.add_space(24.0);
            let footer = 20.0 + 44.0;
            let body = ui.available_height() - 24.0 - footer;
            let items = self.items(o);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                // The rail.
                ui.vertical(|ui| {
                    ui.set_width(RAIL);
                    let tabs = self.tabs();
                    let names: Vec<String> = tabs.iter().map(|t| tr(t.title())).collect();
                    let entries: Vec<ds::MenuEntry> = names.iter().map(|n| ds::MenuEntry::new(n)).collect();
                    let sel = tabs.iter().position(|&t| t == self.tab);
                    if let Some(i) = ds::menu(ui, "options-rail", ds::MenuStyle::RAIL, &entries, sel, None) {
                        if Some(i) != sel {
                            self.set_tab(tabs[i]);
                            click = true;
                        }
                    }
                });
                ui.add_space(32.0);
                // The cards in columns, each card to the shortest column (the boards: three; Sound two).
                let all = cards(&o.rows, &items);
                let n = if self.tab == Tab::Sound { 2 } else { 3 }.min(all.len().max(1));
                let width = ui.available_width();
                let col_w = (width - GAP * (n as f32 - 1.0)) / n as f32;
                let mut cols: Vec<(f32, Vec<usize>)> = vec![(0.0, Vec::new()); n];
                for (c, (_, lines)) in all.iter().enumerate() {
                    let h = 36.0 + 28.0 + lines.iter().map(|&i| line_height(o, texts, items[i])).sum::<f32>();
                    let shortest = (0..n).min_by(|&a, &b| cols[a].0.total_cmp(&cols[b].0)).unwrap();
                    cols[shortest].0 += h + GAP;
                    cols[shortest].1.push(c);
                }
                self.hovered = None;
                ui.vertical(|ui| {
                    ui.set_width(width);
                    egui::ScrollArea::vertical().id_salt(self.tab).scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).max_height(body).auto_shrink([false, false]).show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(GAP, 0.0);
                            for (_, cs) in &cols {
                                ui.vertical(|ui| {
                                    ui.set_width(col_w);
                                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
                                    for &c in cs {
                                        let (title, lines) = &all[c];
                                        ds::card(ui, Some(&tr(title)), |ui| {
                                            for &i in lines {
                                                click |= self.line(o, ui, texts, i, items[i], sound);
                                            }
                                        });
                                        ui.add_space(GAP - 2.0);
                                    }
                                });
                            }
                        });
                    });
                });
            });
            ui.add_space(24.0);
            click |= self.footer(o, ui, texts, &items);
        });
        click
    }

    /// The settings page's footer: a rim line, the status (what is wanted, or the line's description), the buttons.
    fn footer(&mut self, o: &mut OptionsScreen, ui: &mut egui::Ui, texts: &UiTexts, items: &[Item]) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let tr = |en: &str| s(texts.ours(en));
        let k = ds::tokens();
        let mut click = false;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 64.0), egui::Sense::hover());
        ui.painter().hline(rect.x_range(), rect.top() + 0.5, egui::Stroke::new(1.0f32, k.rim));
        let row = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + 20.0), rect.max);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = 16.0;
        let pending = self.pending(o);
        let apply = child.add_enabled(pending, ds::Plate::new(&tr("Apply")).primary(pending).width(180.0).focused(self.keyed && self.focus == items.len() + 1));
        let back = child.add(ds::Plate::new(&tr("Back")).width(180.0).focused(self.keyed && self.focus == items.len()));
        if self.tab == Tab::Controls {
            let reset = items.iter().position(|&it| it == Item::Reset);
            let r = child.add_enabled(!self.binds.is_default(), ds::Plate::new(&tr("Original keys")).width(220.0).focused(self.keyed && reset == Some(self.focus)));
            if r.clicked() {
                self.binds = Default::default();
                click = true;
            }
        }
        if apply.on_hover_text(tr("Save the changes and use them.")).clicked() {
            self.apply(o);
            click = true;
        }
        if back.on_hover_text(tr("Leave; changes not applied are taken back.")).clicked() {
            self.back(o);
            click = true;
        }
        // The status: the wait for a key, a key two actions share, the language's restart, else the line's description.
        let lang = o.rows.iter().find(|r| r.key == crate::display::LANGUAGE).map(|r| r.value);
        let line = self.hovered.or(Some(self.focus)).and_then(|i| items.get(i).copied());
        let clash = match items.get(self.focus) {
            Some(&Item::Bind(b)) => self.binds.clash(b),
            _ => None,
        };
        let (text, c) = if self.capture.is_some() {
            (tr("Press the new key; Esc keeps the old one."), k.focus)
        } else if let Some(c) = clash {
            (format!("{} {}", tr("Also the key of:"), s(label(o, texts, Item::Bind(c)))), k.focus)
        } else if self.tab == Tab::Accessibility && o.lang_at_start.is_some_and(|v| Some(v) != lang) {
            (tr("Restart to change the language"), k.focus)
        } else if let Some(it) = line {
            let d = describe(key_of(o, it));
            (if d.is_empty() { String::new() } else { tr(d) }, k.ink_silver)
        } else {
            (String::new(), k.ink_silver)
        };
        let used = child.min_rect().width();
        let f = Font::BODY.sized(15.0).tracking(0.05);
        Text::new(&text, Font { line: 22.0, ..f }).color(c).lines(2).paint(ui, row.with_max_x(row.right() - used - 16.0));
        click
    }

    /// A setting's line in its card: a settings row (`.orow`), a wide one (the label over the segments), a key's row
    /// (`.krow`). True when it was clicked.
    fn line(&mut self, o: &mut OptionsScreen, ui: &mut egui::Ui, texts: &UiTexts, i: usize, it: Item, sound: &mut dyn FnMut(crate::audio::SoundId)) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let live = Self::enabled(o, it);
        let focused = self.keyed && self.focus == i;
        let name = s(label(o, texts, it));
        let mut click = false;
        let resp = match it {
            Item::Reset => return false,
            Item::Bind(b) => {
                let f = Font { size: 15.0, line: 20.0, weight: ds::Weight::Regular, tracking: 0.05, upper: false, lcd: false };
                ds::option_row_in(ui, 40.0, f, &name, focused, live, |ui| click |= self.control(o, ui, texts, Item::Bind(b), sound)).0
            }
            Item::Row(k) if matches!(kind(&o.rows[k], texts), Kind::Segments { wide: true }) => {
                // `.orow.wide`: the label, 8 points, the segments in equal columns.
                let t = ds::tokens();
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 92.0), egui::Sense::click());
                let hover = ds::hover_t(ui.ctx(), resp.id, resp.hovered() && live);
                if focused {
                    ui.painter().rect_filled(rect, ds::RADIUS_PLATE, t.rim_bright.gamma_multiply(0.12));
                }
                ds::rim(ui.painter(), rect, ds::RADIUS_PLATE, t.rim_bright.gamma_multiply(if focused { 1.0 } else { hover * 0.6 }));
                let inner = rect.shrink(8.0);
                let f = Font { size: 16.0, line: 20.0, weight: ds::Weight::Regular, tracking: 0.05, upper: false, lcd: false };
                Text::new(&name, f).color(if live { t.ink } else { t.ink_muted }).paint(ui, inner.with_max_y(inner.top() + 32.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner.with_min_y(inner.bottom() - 36.0)).layout(egui::Layout::left_to_right(egui::Align::Center)));
                if !live {
                    child.disable();
                }
                click |= self.control(o, &mut child, texts, it, sound);
                resp
            }
            _ => ds::option_row(ui, &name, focused, live, |ui| click |= self.control(o, ui, texts, it, sound)).0,
        };
        if resp.hovered() || ui.rect_contains_pointer(resp.rect) {
            self.hovered = Some(i);
        }
        if resp.clicked() || (ui.rect_contains_pointer(resp.rect) && ui.input(|x| x.pointer.primary_pressed())) {
            self.focus = i;
        }
        if focused && self.shown != Some((self.tab, i)) {
            ui.scroll_to_rect(resp.rect, None);
            self.shown = Some((self.tab, i));
        }
        click
    }

    /// A setting's control: a toggle, segments, a stepper, a slider, the name field, a key. True when it was clicked.
    fn control(&mut self, o: &mut OptionsScreen, ui: &mut egui::Ui, texts: &UiTexts, it: Item, sound: &mut dyn FnMut(crate::audio::SoundId)) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let k = ds::tokens();
        match it {
            Item::Row(r) => {
                let row = &o.rows[r];
                let v = if row.locked { 0 } else { row.value };
                match kind(row, texts) {
                    Kind::Scale => {
                        let mut f = v as f32 / row.states.max(1) as f32;
                        let resp = ds::slider(ui, &mut f, &s(value_text(texts, row, v)));
                        if resp.changed() {
                            Self::set(o, r, (f * row.states as f32).round() as i32);
                        }
                        if resp.drag_started() {
                            sound(crate::audio::CLICK);
                        }
                    }
                    Kind::Toggle => {
                        let mut on = v != 0;
                        if ds::toggle(ui, &mut on, &s(texts.get(13, 3, 2)), &s(texts.get(13, 3, 1))).changed() {
                            Self::set(o, r, on as i32);
                            return true;
                        }
                    }
                    Kind::Segments { wide } => {
                        let names: Vec<String> = (0..=row.states).map(|n| s(value_text(texts, row, n))).collect();
                        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                        let mut sel = v.max(0) as usize;
                        if ds::segmented(ui, &mut sel, &refs, wide.then(|| ui.available_width())).changed() {
                            Self::set(o, r, sel as i32);
                            return true;
                        }
                    }
                    Kind::Stepper => {
                        let (_, d) = ds::stepper(ui, &s(value_text(texts, row, v)), 128.0);
                        if d != 0 {
                            Self::step(o, r, d);
                            return true;
                        }
                    }
                }
            }
            Item::Slider(i) => {
                let mut f = o.sliders[i].value as f32 / 100.0;
                let resp = ds::slider(ui, &mut f, &format!("{}%", o.sliders[i].value));
                if resp.changed() {
                    o.sliders[i].value = (f * 100.0).round() as i32;
                }
                if resp.drag_started() {
                    sound(crate::audio::CLICK);
                }
            }
            Item::Name => {
                // A recessed field (`.field input`), the caret while it is typed.
                let caret = self.editing && (ui.input(|x| x.time) * 2.0) as u64 % 2 == 0;
                let text = s(self.name.clone()) + if caret { "|" } else { "" };
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(180.0, 36.0), egui::Sense::click());
                let p = ui.painter();
                p.rect_filled(rect, ds::RADIUS_PLATE, k.glass_deep);
                ds::rim(p, rect, ds::RADIUS_PLATE, if self.editing { k.focus } else { k.rim });
                Text::new(&text, Font::DATA.sized(16.0).tracking(0.08)).paint(ui, rect.shrink2(egui::vec2(12.0, 0.0)));
                if resp.clicked() {
                    self.editing = true;
                    return true;
                }
            }
            Item::Bind(b) => {
                let listening = self.capture == Some(b);
                let text = if listening { "…" } else { crate::bindings::key_name(self.binds.0[b]).unwrap_or("?") };
                if ds::key_cap(ui, text, listening || self.binds.clash(b).is_some()).clicked() {
                    self.capture = Some(b);
                    return true;
                }
            }
            // Drawn by the footer and the save and load page.
            Item::Reset | Item::Slot(_) | Item::Game(_) => {}
        }
        false
    }

    /// ADR 0067: the save and load page (its board: the slots' table at the left, the selected save and the name with
    /// Save and Load at the right, the footer's hint and Back). True when something was clicked.
    fn saves(&mut self, o: &mut OptionsScreen, ctx: &egui::Context, texts: &UiTexts) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let tr = |en: &str| s(texts.ours(en));
        let k = ds::tokens();
        let mut click = false;
        let items = game_items(o);
        crate::gui::board(ctx, "saves", egui::Vec2::from(SAVES_PAGE), 0.55, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let w = ui.available_width();
            Text::new(&tr("SAVE & LOAD"), Font::PAGE_TITLE).color(k.ink_silver).show(ui, egui::vec2(w, 36.0));
            ui.add_space(20.0);
            let body = ui.available_height() - 20.0 - 64.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                let list_w = w - ASIDE - 32.0;
                ui.vertical(|ui| {
                    ui.set_width(list_w);
                    ui.set_height(body);
                    click |= self.slot_table(o, ui, texts, &items, body);
                });
                ui.add_space(32.0);
                ui.vertical(|ui| {
                    ui.set_width(ASIDE);
                    click |= self.aside(o, ui, texts, &items);
                });
            });
            ui.add_space(20.0);
            // The footer: the hint, Back.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 64.0), egui::Sense::hover());
            ui.painter().hline(rect.x_range(), rect.top() + 0.5, egui::Stroke::new(1.0f32, k.rim));
            let row = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + 20.0), rect.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::right_to_left(egui::Align::Center)));
            if child.add(ds::Plate::new(&tr("Back")).width(200.0).focused(self.keyed && self.focus == items.len())).clicked() {
                self.back(o);
                click = true;
            }
            // Why Load or Save is off (the original greys them without a word).
            let why = if o.entry.is_some() {
                "Enter saves, Esc cancels."
            } else if o.multiplayer && !o.tutorial {
                "Multiplayer: no loading; only the host saves."
            } else if o.battle_lock() && !o.load_only && !o.tutorial {
                "No saving during a battle."
            } else {
                ""
            };
            let f = Font::BODY.sized(15.0).tracking(0.05);
            Text::new(&tr(why), Font { line: 22.0, ..f }).color(k.ink_silver).paint(ui, row.with_max_x(row.right() - 216.0));
        });
        click
    }

    /// The slots' table (`.srow`: #, name, game date, saved; 40 high): the autosaves, a rim line, the slots.
    fn slot_table(&mut self, o: &mut OptionsScreen, ui: &mut egui::Ui, texts: &UiTexts, items: &[Item], body: f32) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let tr = |en: &str| s(texts.ours(en));
        let k = ds::tokens();
        let cols = |r: egui::Rect| {
            let x = r.left() + 12.0;
            let date = r.right() - 12.0 - 150.0 - 12.0 - 150.0;
            [egui::Rect::from_x_y_ranges(x..=x + 44.0, r.y_range()), egui::Rect::from_x_y_ranges(x + 56.0..=date - 12.0, r.y_range()), egui::Rect::from_x_y_ranges(date..=date + 150.0, r.y_range()), egui::Rect::from_x_y_ranges(r.right() - 162.0..=r.right() - 12.0, r.y_range())]
        };
        let (head, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::hover());
        for (c, t) in cols(head).into_iter().zip(["#", "Saves", "Game date", "Saved"]) {
            Text::new(&if t == "#" { t.to_owned() } else { tr(t) }, Font::LABEL).color(k.ink_muted).paint(ui, c);
        }
        ui.add_space(4.0);
        let mut click = false;
        let lcd = Font { size: 16.0, line: 18.0, ..Font::READOUT }.tracking(0.08);
        egui::ScrollArea::vertical().id_salt("slots").scroll_source(egui::scroll_area::ScrollSource { drag: false, ..Default::default() }).max_height(body - 32.0).auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            let mut autos = true;
            for (i, &it) in items.iter().enumerate() {
                let Item::Slot(n) = it else { continue };
                if autos && n <= 22 {
                    autos = false;
                    if i > 0 {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 9.0), egui::Sense::hover());
                        ui.painter().hline(r.x_range().shrink(12.0), r.center().y, egui::Stroke::new(1.0f32, k.rim));
                    }
                }
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::click());
                let on = o.slot == n;
                let hover = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
                let p = ui.painter();
                if on {
                    ds::hgrad(p, rect, ds::RADIUS_PLATE, &[(0.0, egui::Color32::from_white_alpha(41)), (1.0, egui::Color32::from_white_alpha(5))]);
                    ds::rim(p, rect, ds::RADIUS_PLATE, k.ink_silver);
                } else {
                    ds::rim(p, rect, ds::RADIUS_PLATE, k.rim_bright.gamma_multiply(hover));
                }
                if self.keyed && self.focus == i {
                    ds::focus_mark(p, rect, ds::RADIUS_PLATE as f32, 2.0);
                    if self.shown != Some((self.tab, i)) {
                        ui.scroll_to_rect(rect, None);
                        self.shown = Some((self.tab, i));
                    }
                }
                let [cn, cname, cdate, cage] = cols(rect);
                let body_f = Font::BODY.sized(17.0);
                Text::new(&if n > 22 { "A".to_owned() } else { n.to_string() }, lcd).color(k.ink_muted).paint(ui, cn);
                match o.slots.get(n).and_then(Option::as_ref) {
                    Some(slot) => {
                        let name = match n {
                            24 => tr("Autosave"),
                            23 => tr("Previous autosave"),
                            _ => s(slot.name.clone()),
                        };
                        Text::new(&name, body_f).color(k.ink).paint(ui, cname);
                        Text::new(&date(slot.date), lcd).color(k.info).paint(ui, cdate);
                        if let Some(a) = age(&slot.file, texts.lang) {
                            Text::new(&s(a), Font::BODY.sized(14.0)).color(k.ink_muted).paint(ui, cage);
                        }
                    }
                    None => {
                        Text::new(&tr("Empty"), body_f).color(k.ink_muted).paint(ui, cname);
                    }
                }
                // The slots are off while the save name is typed (`Options_MouseInit2`: id 4 only without `IOMode`).
                if o.entry.is_some() {
                    continue;
                }
                if resp.double_clicked() {
                    click |= self.act(o, Game::Load);
                } else if resp.clicked() {
                    (o.slot, self.focus) = (n, i);
                    click = true;
                }
            }
        });
        click
    }

    /// The right column: the selected save's card, the name field with Save and Load.
    fn aside(&mut self, o: &mut OptionsScreen, ui: &mut egui::Ui, texts: &UiTexts, items: &[Item]) -> bool {
        let s = |b: Vec<u8>| ig2_formats::text::decode(&b, texts.lang);
        let tr = |en: &str| s(texts.ours(en));
        let k = ds::tokens();
        let mut click = false;
        let kind = match o.slot {
            24 => texts.ours("Autosave"),
            23 => texts.ours("Previous autosave"),
            n => crate::tr::fill(texts.lang, "Slot {}", &[n.to_string().as_bytes()]),
        };
        let slot = o.slots.get(o.slot).and_then(Option::as_ref).cloned();
        ds::panel(ui, ds::PanelKind::Glass, 0.88, 24.0, |ui| {
            ui.set_width(ASIDE - 48.0);
            ui.spacing_mut().item_spacing.y = 14.0;
            ds::heading_label(ui, &tr("Selected save"));
            let w = ui.available_width();
            let title = Font::HEADING.sized(24.0).tracking(0.08);
            match &slot {
                Some(sl) => {
                    Text::new(&s(sl.name.clone()), Font { line: 30.0, ..title }).show(ui, egui::vec2(w, 30.0));
                    ds::kv(ui, &tr("Game date"), &date(sl.date), ds::Tone::Info, 150.0);
                    ds::kv(ui, &tr("Slot"), &s(kind.clone()), ds::Tone::Info, 150.0);
                    if let Some(a) = age(&sl.file, texts.lang) {
                        ds::kv(ui, &tr("Saved"), &s(a), ds::Tone::Info, 150.0);
                    }
                }
                None => {
                    Text::new(&tr("Empty slot"), Font { line: 30.0, ..title }).color(k.ink_muted).show(ui, egui::vec2(w, 30.0));
                    ds::kv(ui, &tr("Slot"), &s(kind.clone()), ds::Tone::Info, 150.0);
                }
            }
        });
        ui.add_space(20.0);
        ds::panel(ui, ds::PanelKind::Glass, 0.88, 24.0, |ui| {
            ui.set_width(ASIDE - 48.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            let w = ui.available_width();
            ds::heading_label(ui, &tr("Save name"));
            // The field: the name being typed with its caret, else what Save would start with.
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 44.0), egui::Sense::click());
            let p = ui.painter();
            p.rect_filled(rect, ds::RADIUS_PLATE, k.glass_deep);
            ds::rim(p, rect, ds::RADIUS_PLATE, if o.entry.is_some() { k.focus } else { k.rim });
            ds::edge_line(p, rect, ds::RADIUS_PLATE as f32, true, egui::Color32::from_black_alpha(200));
            let f = Font::DATA.sized(17.0).tracking(0.08);
            match &o.entry {
                Some(e) => {
                    let caret = (ui.input(|x| x.time) * 2.0) as u64 % 2 == 0;
                    let (a, b) = e.text.split_at(e.cursor.min(e.text.len()));
                    let text = format!("{}{}{}", s(a.to_vec()), if caret { "|" } else { " " }, s(b.to_vec()));
                    Text::new(&text, f).paint(ui, rect.shrink2(egui::vec2(14.0, 0.0)));
                }
                None => {
                    let shown = slot.as_ref().map_or_else(|| format!("Save Game {:02}", o.slot), |sl| s(sl.name.clone()));
                    Text::new(&shown, f).color(k.ink_muted).paint(ui, rect.shrink2(egui::vec2(14.0, 0.0)));
                }
            }
            if resp.clicked() && o.entry.is_none() && Self::enabled(o, Item::Game(Game::Save)) {
                o.start_entry();
                click = true;
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let half = (w - 12.0) / 2.0;
                for g in [Game::Save, Game::Load] {
                    let at = items.iter().position(|&it| it == Item::Game(g));
                    let Some(at) = at else { continue };
                    let live = Self::enabled(o, Item::Game(g)) || (g == Game::Save && o.entry.is_some());
                    let r = ui.add_enabled(live, ds::Plate::new(&tr(g.label(false))).primary(g == Game::Load || o.entry.is_some()).width(half).focused(self.keyed && self.focus == at));
                    if r.clicked() {
                        self.focus = at;
                        click |= self.act(o, g);
                    }
                }
            });
            let hint = if slot.is_some() && !o.load_only { "Saving here overwrites the old save." } else { "" };
            let f = Font::BODY.sized(14.0).tracking(0.04);
            Text::new(&tr(hint), Font { line: 20.0, ..f }).color(k.ink_muted).lines(2).show(ui, egui::vec2(w, 40.0));
        });
        click
    }
}

/// A save's game date (year, month, day).
fn date(d: [i32; 3]) -> String {
    format!("{}.{:02}.{:02}", d[0], d[1], d[2])
}

/// How long ago a save's file was written ("Saved 5 minutes ago"); none where the file system can't tell.
fn ago(file: &std::path::Path, lang: ig2_formats::text::Lang) -> Option<Vec<u8>> {
    let secs = std::fs::metadata(file).ok()?.modified().ok()?.elapsed().ok()?.as_secs();
    let (en, n) = match secs {
        0..60 => return Some(crate::tr::get(lang, "Saved just now")),
        60..3600 => ("Saved {} minutes ago", secs / 60),
        3600..86400 => ("Saved {} hours ago", secs / 3600),
        _ => ("Saved {} days ago", secs / 86400),
    };
    Some(crate::tr::fill(lang, en, &[n.to_string().as_bytes()]))
}

/// The table's short age ("12 minutes ago"), the column under "Saved".
fn age(file: &std::path::Path, lang: ig2_formats::text::Lang) -> Option<Vec<u8>> {
    let secs = std::fs::metadata(file).ok()?.modified().ok()?.elapsed().ok()?.as_secs();
    let (en, n) = match secs {
        0..60 => return Some(crate::tr::get(lang, "just now")),
        60..3600 => ("{} minutes ago", secs / 60),
        3600..86400 => ("{} hours ago", secs / 3600),
        _ => ("{} days ago", secs / 86400),
    };
    Some(crate::tr::fill(lang, en, &[n.to_string().as_bytes()]))
}

/// The UI scale row's value.
fn scale_row(o: &OptionsScreen) -> Option<i32> {
    o.rows.iter().find(|r| r.key == crate::display::UI_SCALE).map(|r| r.value)
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
            assert!(cards(&o.rows, &items(&o.rows, t)).iter().all(|(h, _)| crate::tr::has(h)), "{t:?}");
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
        for _ in 0..3 {
            m.key(&mut o, Nav::NextTab, &mut quiet);
        }
        assert_eq!(m.tab, Tab::Sound);
        m.key(&mut o, Nav::Right, &mut quiet);
        assert_eq!(o.sliders[0].value, 85);
        m.key(&mut o, Nav::NextTab, &mut quiet);
        m.focus = items(&o.rows, Tab::Game).iter().position(|&i| i == Item::Name).unwrap();
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
                g.run((1280, 720), &crate::gui::style(&theme), |ctx| {
                    let _ = o.modern_page(ctx, &texts, &theme, &mut |_| {});
                });
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

    /// ADR 0067: an in-game screen with slots 1 and 24 filled.
    fn game() -> OptionsScreen {
        let mut o = screen();
        o.load_only = false;
        o.slots = vec![None; SLOTS + 1];
        for n in [1, 24] {
            o.slots[n] = Some(super::super::Slot { name: format!("Game {n}").into_bytes(), date: [2350, 1, 2], file: Default::default(), ours: true });
        }
        o.modern = Some(Modern::in_game(&o, b"Kim".to_vec(), Default::default()));
        o
    }

    /// The save and load page's line `it` focused.
    fn focus(o: &mut OptionsScreen, it: Item) {
        let at = game_items(o).iter().position(|&i| i == it).unwrap();
        let m = o.modern.as_mut().unwrap();
        (m.menu, m.tab, m.focus) = (false, Tab::Saves, at);
    }

    /// The pause menu's entry `g` focused.
    fn menu_focus(o: &mut OptionsScreen, g: Game) {
        let at = menu_items(o).iter().position(|&x| x == g).unwrap();
        let m = o.modern.as_mut().unwrap();
        (m.menu, m.focus) = (true, at);
    }

    fn key(o: &mut OptionsScreen, k: Nav) -> Option<Act> {
        o.modern_key(k, &mut |_| {})
    }

    /// The pause menu first; the page's lines and the original's gates: the filled autosave 24 and the 22 slots; Load a
    /// filled slot, Save not on 23, 24, in a battle or a joiner's multiplayer game; the tutorial's menu; Load game's Load
    /// alone.
    #[test]
    fn save_and_load_lines_and_gates() {
        let mut o = game();
        let m = o.modern.as_ref().unwrap();
        assert!(m.menu && m.tabs() == Tab::ALL);
        assert_eq!(menu_items(&o).len(), 7);
        let lines = game_items(&o);
        assert_eq!(lines.len(), 23 + 2);
        assert!(lines[0] == Item::Slot(24) && !lines.contains(&Item::Slot(23)));
        let on = |o: &OptionsScreen, g| Modern::enabled(o, Item::Game(g));
        o.slot = 1;
        assert!(on(&o, Game::Load) && on(&o, Game::Save));
        o.slot = 2;
        assert!(!on(&o, Game::Load) && on(&o, Game::Save));
        o.slot = 24;
        assert!(on(&o, Game::Load) && !on(&o, Game::Save));
        o.slot = 1;
        o.battle = true;
        assert!(!on(&o, Game::Save));
        o.debug = true;
        assert!(on(&o, Game::Save));
        (o.battle, o.multiplayer) = (false, true);
        assert!(!on(&o, Game::Load) && !on(&o, Game::Save));
        o.mp_save = true;
        assert!(on(&o, Game::Save));
        o.multiplayer = false;
        o.tutorial = true;
        assert_eq!(menu_items(&o), [Game::Continue, Game::Options, Game::Quit]);
        o.tutorial = false;
        o.load_only = true;
        assert_eq!(game_items(&o).last(), Some(&Item::Game(Game::Load)));
        assert_eq!(game_items(&o).len(), 24);
        let m = Modern::in_game(&o, b"Kim".to_vec(), Default::default());
        assert!(!m.menu && m.tab == Tab::Saves);
    }

    /// The pause menu's Save opens the page with the name entry; Options the tabs; Back from either is the menu again;
    /// Help leaves for the help screen.
    #[test]
    fn pause_menu_opens_its_pages() {
        let mut o = game();
        o.slot = 3;
        menu_focus(&mut o, Game::Save);
        key(&mut o, Nav::Activate);
        assert!(o.entry.is_some() && o.modern.as_ref().unwrap().tab == Tab::Saves);
        key(&mut o, Nav::Back);
        key(&mut o, Nav::Back);
        assert!(o.modern.as_ref().unwrap().menu && !o.leaving());
        menu_focus(&mut o, Game::Options);
        key(&mut o, Nav::Activate);
        assert_eq!(o.modern.as_ref().unwrap().tab, Tab::Display);
        key(&mut o, Nav::Back);
        assert!(o.modern.as_ref().unwrap().menu);
        menu_focus(&mut o, Game::Help);
        key(&mut o, Nav::Activate);
        assert_eq!((0..3).find_map(|_| o.done()), Some(Act::Help));
    }

    /// Save: the slot's name or "Save Game nn", the first letter clears it, Enter hands the save back; Esc cancels.
    #[test]
    fn save_by_keys() {
        let mut o = game();
        focus(&mut o, Item::Slot(3));
        key(&mut o, Nav::Down);
        assert_eq!(o.slot, 4);
        key(&mut o, Nav::Activate);
        assert_eq!(o.entry.as_ref().unwrap().text, b"Save Game 04");
        assert!(o.typing());
        for c in *b"Ab" {
            key(&mut o, Nav::Char(c));
        }
        key(&mut o, Nav::Erase);
        key(&mut o, Nav::Char(b'x'));
        assert_eq!(key(&mut o, Nav::Activate), Some(Act::Save(4, b"Ax".to_vec())));
        assert!(o.entry.is_none() && !o.leaving());
        // The Save button on a filled slot: its name; Esc ends the entry, the next Esc leaves.
        o.slot = 1;
        focus(&mut o, Item::Game(Game::Save));
        key(&mut o, Nav::Activate);
        assert_eq!(o.entry.as_ref().unwrap().text, b"Game 1");
        key(&mut o, Nav::Back);
        assert!(o.entry.is_none() && !o.leaving());
        key(&mut o, Nav::Back);
        assert!(o.modern.as_ref().unwrap().menu && !o.leaving());
        key(&mut o, Nav::Back);
        assert!(o.leaving());
        assert_eq!(o.done().or_else(|| o.done()).or_else(|| o.done()), Some(Act::Back));
    }

    /// Quit and Exit ask first (Esc keeps the game); Load leaves at once with the settings not applied taken back.
    #[test]
    fn quit_exit_and_load() {
        let mut o = game();
        menu_focus(&mut o, Game::Quit);
        key(&mut o, Nav::Activate);
        assert_eq!(o.modern.as_ref().unwrap().ask, Some(Game::Quit));
        key(&mut o, Nav::Back);
        assert!(o.modern.as_ref().unwrap().ask.is_none() && !o.leaving());
        menu_focus(&mut o, Game::Exit);
        key(&mut o, Nav::Activate);
        key(&mut o, Nav::Activate);
        let done = |o: &mut OptionsScreen| (0..3).find_map(|_| o.done());
        assert_eq!(done(&mut o), Some(Act::Exit));
        let mut o = game();
        o.sliders[0].value = 10;
        focus(&mut o, Item::Slot(2));
        key(&mut o, Nav::Up);
        assert_eq!(o.slot, 1);
        key(&mut o, Nav::Activate);
        assert_eq!((done(&mut o), o.sliders[0].value), (Some(Act::Load(1)), 80));
        // Under the tutorial Quit goes without the question.
        let mut o = game();
        o.tutorial = true;
        menu_focus(&mut o, Game::Quit);
        key(&mut o, Nav::Activate);
        assert_eq!(done(&mut o), Some(Act::MainMenu));
    }

    /// The in-game page draws its Save & load tab in every state (egui without a GPU); its texts are in the table.
    #[test]
    fn game_page_draws_and_its_texts_are_in_the_table() {
        let texts = crate::ui::UiTexts { strings: Default::default(), kinds: Vec::new(), lang: ig2_formats::text::Lang::En };
        let theme = crate::theme::Theme::modern();
        let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut o = game();
        for step in 0..4 {
            match step {
                1 => {
                    let m = o.modern.as_mut().unwrap();
                    (m.menu, m.tab) = (false, Tab::Saves);
                    o.start_entry();
                }
                2 => (o.entry, o.modern.as_mut().unwrap().ask) = (None, Some(Game::Exit)),
                3 => (o.tutorial, o.modern.as_mut().unwrap().ask) = (true, None),
                _ => {}
            }
            for _ in 0..2 {
                g.run((1920, 1080), &crate::gui::style(&theme), |ctx| assert_eq!(o.modern_page(ctx, &texts, &theme, &mut |_| {}), None));
            }
        }
        assert!(!o.leaving());
        for t in ["SAVE & LOAD", "Saves", "Selected save", "Empty", "Empty slot", "Cancel", "Exit", "Quit", "Save name", "Autosave", "Previous autosave", "Slot {}", "Game date: {}"] {
            assert!(crate::tr::has(t), "{t}");
        }
        for t in ["Saved just now", "Saved {} minutes ago", "Saved {} hours ago", "Saved {} days ago", "Enter saves, Esc cancels.", "No saving during a battle."] {
            assert!(crate::tr::has(t), "{t}");
        }
        for t in ["Detail", "Readability", "Subtitles and colours", "Volume", "Music", "Battles and saving", "Enhancements", "Handling", "Original keys", "Game date", "Slot", "Saved"] {
            assert!(crate::tr::has(t), "{t}");
        }
        for t in ["Saving here overwrites the old save.", "just now", "{} minutes ago", "{} hours ago", "{} days ago", "Pause", "OPTIONS"] {
            assert!(crate::tr::has(t), "{t}");
        }
        for t in ["Multiplayer: no loading; only the host saves.", "Quit to the main menu? Play since the last save is lost.", "Exit the game? Play since the last save is lost.", describe("ModernGameMenu")] {
            assert!(crate::tr::has(t), "{t}");
        }
        for g in [Game::Load, Game::Save, Game::Quit, Game::Exit, Game::Continue, Game::Options, Game::Help] {
            assert!(crate::tr::has(g.label(false)) && crate::tr::has(g.label(true)), "{g:?}");
        }
    }

}
