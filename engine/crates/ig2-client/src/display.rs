//! The remake's display settings (an enhanced setting, docs/USER_GUIDE.md → Options): window, borderless or
//! fullscreen, the resolution, vsync and the 4:3 letterbox, with the live background (ADR 0015) and the names'
//! language (ADR 0017), the clean name entry (ADR 0024), the info boxes' models (ADR 0025), the battle's slider (ADR 0027), the Hungarian articles (ADR 0028) the centred Exit label (ADR 0029) the W/A/S/D camera (ADR 0030), the autosave's cadence (ADR 0032), the menu Settings label and Language row (ADR 0031), the fleet groups (ADR 0035), the game statistics (ADR 0034) the replays (ADR 0038), the event log (ADR 0040), the subtitles and the colour-blind empire colours (ADR 0036), the sharp UI (ADR 0043), the sharp text (ADR 0046), the texture upscaling (ADR 0054), the starmap's mouse camera (ADR 0049), the high-score entry (ADR 0050), the title's music (ADR 0051), the campaign story's ruleset (ADR 0052), the modern lighting (ADR 0055), the menu navigation (ADR 0059); kept in our settings file and shown as rows on the Options screen's Remake page (ADR 0048)
//! (`crate::options`, in the original's font and colours on the Remake page under group headings). The original has a fixed
//! 640×480 or 800×600 screen and none of these rows.

use ig2_formats::options::Options as File;
use ig2_formats::text::Lang;
use winit::monitor::MonitorHandle;
use winit::window::Fullscreen;

use crate::options::{Group, Row};

pub const MODE: &str = "DisplayMode";
pub const RESOLUTION: &str = "Resolution";
pub const VSYNC: &str = "VSync";
pub const LETTERBOX: &str = "Letterbox";
pub const LIVE_BACKGROUND: &str = "LiveBackground";
pub const ONE_LANGUAGE: &str = "OneLanguage";
/// ADR 0024: the save slot being named doesn't show its old name under the entry.
pub const CLEAN_ENTRY: &str = "CleanNameEntry";
/// ADR 0025: the info box's turning model is cut to the box.
pub const BOX_MODELS: &str = "BoxModelsInside";
/// ADR 0027: the space battle's defend/attack slider sits under its label.
pub const SLIDER_UNDER: &str = "SliderUnderLabel";
/// ADR 0028: a Hungarian game's article markers become "a"/"az".
pub const ARTICLES: &str = "HungarianArticles";
/// ADR 0029: the starmap panels' Exit label is centred on its box.
pub const CENTRED_EXIT: &str = "CentredExitLabel";
/// ADR 0030: W/A/S/D pan the camera; 0 off, 1 on the starmap (the default), 2 both, also on the colony screen.
pub const WASD: &str = "WasdCamera";
/// ADR 0032: the autosave's cadence, an index into `crate::options::AUTOSAVE_DAYS` (0 = the original's).
pub const AUTOSAVE: &str = "AutosaveEvery";
/// ADR 0031: the main menu's "Settings" label and its `O` key (on by default).
pub const MENU_SETTINGS: &str = "MenuSettings";
/// ADR 0031: the texts' language at the next start; 0 the client's own choice, else [`LANGS`]`[n - 1]`.
pub const LANGUAGE: &str = "Language";
/// ADR 0035: Ctrl+1..9 store the selected fleet in a group, Alt+1..9 select it.
pub const FLEET_GROUPS: &str = "FleetGroups";
/// ADR 0034: the statistics screen after a game's end and the match history.
pub const STATISTICS: &str = "GameStatistics";
/// ADR 0040: the messages screen's event log.
pub const EVENT_LOG: &str = "EventLog";
/// The language row's choices after "Default", in its order.
pub const LANGS: [Lang; 6] = [Lang::En, Lang::Hu, Lang::De, Lang::Fr, Lang::Es, Lang::It];

/// The language the settings file chooses (ADR 0031), if any.
pub fn language(f: &File) -> Option<Lang> {
    f.int(LANGUAGE).and_then(|n| LANGS.get((n as usize).checked_sub(1)?).copied())
}
/// ADR 0038: each game writes its replay into `replays/` beside the slots (on by default).
pub const REPLAYS: &str = "RecordReplays";
/// ADR 0036: the spoken message line's text at the bottom while its voice plays (off by default).
pub const SUBTITLES: &str = "Subtitles";
/// ADR 0036: colour-blind-friendly colours for empires 1..=8 (off by default).
pub const COLOUR_BLIND: &str = "ColourBlindColours";
/// ADR 0043: the 2D pictures and text sampled sharp bilinear (off by default).
pub const SHARP_UI: &str = "SharpUi";
/// ADR 0046: the glyphs' edges one screen pixel wide at any window size (off by default).
pub const SHARP_TEXT: &str = "SharpText";
/// ADR 0049: the wheel zooms and the right button drags the starmap's camera (on by default).
pub const STARMAP_MOUSE: &str = "StarmapMouseCamera";
/// ADR 0050: a won scenario's time goes into its high-score table (on by default).
pub const HIGH_SCORES: &str = "HighScoreEntry";
/// ADR 0045: the computer empires of the next single-player game play the enhanced ruleset (0 classic, the default;
/// 1 enhanced). The game keeps its own choice (`Game::enhanced_ai`, saved with it).
pub const ENHANCED_AI: &str = "EnhancedAi";
/// ADR 0052: the campaign story of the next new game plays the enhanced ruleset's fixes (0 classic, the default;
/// 1 enhanced). The game keeps its own choice (`Game::story_fixes`, saved with it).
pub const STORY_FIXES: &str = "StoryFixes";
/// ADR 0051: the title video's music starts with the intro, not under the logos (on by default).
pub const TITLE_MUSIC: &str = "TitleMusicWithIntro";
/// ADR 0054: the textures upscaled 2× or 4× at load (0 = off, the default; 1 = 2×, 2 = 4×).
pub const TEXTURE_UPSCALE: &str = "TextureUpscale";
/// The UI theme by name (ADR 0056): Classic, Modern or a mod folder's.
pub const UI_THEME: &str = "UiTheme";

/// ADR 0055: the meshes drawn 4× multisampled (off by default).
pub const ANTI_ALIASING: &str = "AntiAliasing";
/// ADR 0055: the space battle's ships lit per pixel (off by default).
pub const PIXEL_LIGHTING: &str = "PixelLighting";
/// ADR 0055: the shadow map read percentage-closer filtered (off by default).
pub const SOFT_SHADOWS: &str = "SoftShadows";
/// ADR 0055: the additive translucent parts glow (off by default).
pub const BLOOM: &str = "Bloom";
/// ADR 0057: the battles' effects with the enhanced particle layers on top (0 = classic, the default; 1 = enhanced).
pub const BATTLE_EFFECTS: &str = "BattleEffects";
/// ADR 0064: the starmap's and backgrounds' glows and particles drawn without a texel grid or square points (off by
/// default).
pub const SMOOTH_GLOWS: &str = "SmoothGlows";
/// ADR 0065: the starmap's HUD drawn as the modern egui panels (on by default; 0 = the original's).
pub const MODERN_STARMAP: &str = "ModernStarmapHud";
/// ADR 0067: Esc's in-game menu (the Options screen) as the modern egui page (on by default; 0 = the original's).
pub const MODERN_MENU: &str = "ModernGameMenu";

/// ADR 0058: the speech's volume; 0 = the sound volume's (the original's, the default), n = (n − 1) × 10 per cent.
pub const VOICE_VOLUME: &str = "VoiceVolume";
/// ADR 0058: the keys for the faster speeds ×32 and ×64 in single player (off by default).
pub const EXTRA_SPEEDS: &str = "ExtraSpeeds";

/// The speech's volume the settings file chooses (ADR 0058), `None` = the sound volume's.
pub fn voice_volume(f: &File) -> Option<i32> {
    f.int(VOICE_VOLUME).filter(|n| (1..=11).contains(n)).map(|n| (n - 1) * 10)
}

/// The rows' ids (past the exe's 101..=107 and 201..=211).
const ID_MODE: i32 = 301;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Window,
    Borderless,
    Fullscreen,
}

/// The display as the settings file has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Display {
    pub mode: Mode,
    /// The window's (or the fullscreen mode's) size; `None`: the default window.
    pub size: Option<(u32, u32)>,
    pub vsync: bool,
    pub letterbox: bool,
    pub live_background: bool,
    pub one_language: bool,
    pub box_models: bool,
    pub slider_under: bool,
    pub articles: bool,
    pub centred_exit: bool,
    pub wasd: Wasd,
    pub fleet_groups: bool,
    pub statistics: bool,
    pub event_log: bool,
    pub subtitles: bool,
    pub colour_blind: bool,
    pub sharp_ui: bool,
    pub sharp_text: bool,
    pub starmap_mouse: bool,
    pub high_scores: bool,
    pub title_music: bool,
    /// The texture upscaling's factor: 1 (off), 2 or 4.
    pub upscale: u8,
    pub lighting: crate::render3d::Lighting,
    pub battle_effects: bool,
    pub smooth_glows: bool,
    pub modern_starmap: bool,
    pub modern_menu: bool,
}

/// Where W/A/S/D pan the camera (ADR 0030).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wasd {
    Off,
    Starmap,
    /// Also on the colony screen, where they replace the original's `a` (select all) and `s` (stop).
    Everywhere,
}

/// `"1920x1080"`.
pub fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.trim().split_once('x')?;
    let (w, h) = (w.parse().ok()?, h.parse().ok()?);
    (w >= 320 && h >= 240).then_some((w, h))
}

impl Display {
    pub fn from_settings(f: &File) -> Self {
        Display {
            mode: match f.int(MODE) {
                Some(1) => Mode::Borderless,
                Some(2) => Mode::Fullscreen,
                _ => Mode::Window,
            },
            size: f.get(RESOLUTION).and_then(parse_size),
            vsync: f.int(VSYNC) != Some(0),
            letterbox: f.int(LETTERBOX) == Some(1),
            live_background: f.int(LIVE_BACKGROUND) != Some(0),
            one_language: f.int(ONE_LANGUAGE) != Some(0),
            box_models: f.int(BOX_MODELS) != Some(0),
            slider_under: f.int(SLIDER_UNDER) != Some(0),
            articles: f.int(ARTICLES) != Some(0),
            centred_exit: f.int(CENTRED_EXIT) != Some(0),
            wasd: match f.int(WASD) {
                Some(0) => Wasd::Off,
                Some(2) => Wasd::Everywhere,
                _ => Wasd::Starmap,
            },
            fleet_groups: f.int(FLEET_GROUPS) != Some(0),
            statistics: f.int(STATISTICS) != Some(0),
            event_log: f.int(EVENT_LOG) != Some(0),
            subtitles: f.int(SUBTITLES) == Some(1),
            colour_blind: f.int(COLOUR_BLIND) == Some(1),
            sharp_ui: f.int(SHARP_UI) == Some(1),
            sharp_text: f.int(SHARP_TEXT) == Some(1),
            starmap_mouse: f.int(STARMAP_MOUSE) != Some(0),
            high_scores: f.int(HIGH_SCORES) != Some(0),
            title_music: f.int(TITLE_MUSIC) != Some(0),
            upscale: match f.int(TEXTURE_UPSCALE) {
                Some(1) => 2,
                Some(2) => 4,
                _ => 1,
            },
            lighting: crate::render3d::Lighting {
                msaa: f.int(ANTI_ALIASING) == Some(1),
                per_pixel: f.int(PIXEL_LIGHTING) == Some(1),
                soft_shadows: f.int(SOFT_SHADOWS) == Some(1),
                bloom: f.int(BLOOM) == Some(1),
            },
            battle_effects: f.int(BATTLE_EFFECTS) == Some(1),
            smooth_glows: f.int(SMOOTH_GLOWS) != Some(0),
            modern_starmap: f.int(MODERN_STARMAP) != Some(0),
            modern_menu: f.int(MODERN_MENU) != Some(0),
        }
    }

    /// The window's fullscreen state on `monitor`: an exclusive mode of the size (the highest refresh rate), else
    /// (a size the monitor has no mode for) borderless.
    pub fn fullscreen(&self, monitor: Option<MonitorHandle>) -> Option<Fullscreen> {
        match self.mode {
            Mode::Window => None,
            Mode::Borderless => Some(Fullscreen::Borderless(monitor)),
            Mode::Fullscreen => {
                let exclusive = monitor.as_ref().zip(self.size).and_then(|(m, (w, h))| {
                    m.video_modes().filter(|v| (v.size().width, v.size().height) == (w, h)).max_by_key(|v| (v.refresh_rate_millihertz(), v.bit_depth()))
                });
                Some(exclusive.map_or(Fullscreen::Borderless(monitor), Fullscreen::Exclusive))
            }
        }
    }
}

/// Window sizes offered beside the monitor's own modes (drivers often list few), where they fit on it.
const COMMON: [(u32, u32); 14] = [
    (3840, 2160), (2560, 1440), (1920, 1200), (1920, 1080), (1680, 1050), (1600, 900), (1440, 900), (1366, 768),
    (1280, 960), (1280, 800), (1280, 720), (1024, 768), (800, 600), (640, 480),
];

/// The monitor's modes and the common sizes that fit on it, largest first, none under 640×480.
pub fn sizes(monitor: Option<&MonitorHandle>) -> Vec<(u32, u32)> {
    let mut v: Vec<(u32, u32)> = monitor.map(|m| m.video_modes().map(|v| (v.size().width, v.size().height)).collect()).unwrap_or_default();
    if let Some(m) = monitor.map(MonitorHandle::size) {
        v.extend(COMMON.iter().filter(|&&(w, h)| w <= m.width && h <= m.height));
    }
    v.retain(|&(w, h)| w >= 640 && h >= 480);
    v.sort_by_key(|&(w, h)| std::cmp::Reverse((w * h, w)));
    v.dedup();
    v
}

/// The remake's rows for the Remake page: `sizes` the resolutions to offer, `current` the window's size (the
/// resolution row's value when the file names none or a size not offered); on/off texts come from the game's
/// strings (an empty label list).
pub fn rows(f: &File, sizes: &[(u32, u32)], current: (u32, u32)) -> Vec<Row> {
    let d = Display::from_settings(f);
    let mut sizes = sizes.to_vec();
    if sizes.is_empty() {
        sizes.push(current);
    }
    let want = d.size.unwrap_or(current);
    let res = sizes.iter().position(|&s| s == want).or_else(|| sizes.iter().position(|&s| s.0 <= want.0 && s.1 <= want.1)).unwrap_or(0);
    let row = |n: i32, group: Group, name: &str, key: &str, value: i32, labels: Vec<Vec<u8>>, states: i32| Row {
        name: name.as_bytes().to_vec(),
        key: key.into(),
        enabled: true,
        value,
        states,
        page: crate::options::REMAKE_PAGE,
        id: ID_MODE + n,
        dep: 0,
        mp_ok: true,
        labels,
        ours: true,
        group,
        locked: false,
    };
    let modes = ["Window", "Borderless", "Fullscreen"].map(|s| s.as_bytes().to_vec()).to_vec();
    let labels: Vec<Vec<u8>> = sizes.iter().map(|(w, h)| format!("{w}x{h}").into_bytes()).collect();
    let n = labels.len() as i32;
    let themes = crate::theme::offered();
    let theme = f.get(UI_THEME).and_then(|t| themes.iter().position(|x| x == t)).unwrap_or(0) as i32;
    let n_themes = themes.len() as i32;
    let themes: Vec<Vec<u8>> = themes.into_iter().map(String::into_bytes).collect();
    let voices: Vec<Vec<u8>> = std::iter::once(b"Same as sound".to_vec()).chain((0..=10).map(|n| format!("{}%", n * 10).into_bytes())).collect();
    let langs = ["Default", "English", "Magyar", "Deutsch", "Francais", "Espanol", "Italiano"].map(|s| s.as_bytes().to_vec()).to_vec();
    vec![
        row(0, Group::Display, "Display", MODE, d.mode as i32, modes, 2),
        row(1, Group::Display, "Resolution", RESOLUTION, res as i32, labels, n - 1),
        row(2, Group::Display, "Vertical sync", VSYNC, d.vsync as i32, Vec::new(), 1),
        row(3, Group::Display, "Letterbox 4:3", LETTERBOX, d.letterbox as i32, Vec::new(), 1),
        row(4, Group::Display, "Live background", LIVE_BACKGROUND, d.live_background as i32, Vec::new(), 1),
        row(5, Group::Language, "Names in game language", ONE_LANGUAGE, d.one_language as i32, Vec::new(), 1),
        row(6, Group::Interface, "Clean name entry", CLEAN_ENTRY, (f.int(CLEAN_ENTRY) != Some(0)) as i32, Vec::new(), 1),
        row(7, Group::Interface, "Models inside boxes", BOX_MODELS, d.box_models as i32, Vec::new(), 1),
        row(8, Group::Interface, "Slider under label", SLIDER_UNDER, d.slider_under as i32, Vec::new(), 1),
        row(9, Group::Language, "Hungarian articles", ARTICLES, d.articles as i32, Vec::new(), 1),
        row(10, Group::Interface, "Centred Exit label", CENTRED_EXIT, d.centred_exit as i32, Vec::new(), 1),
        row(11, Group::Interface, "WASD camera", WASD, d.wasd as i32, ["Off", "Starmap", "Both"].map(|s| s.as_bytes().to_vec()).to_vec(), 2),
        row(12, Group::Game, "Autosave every", AUTOSAVE, f.int(AUTOSAVE).unwrap_or(0).clamp(0, 3), ["Original", "10 days", "30 days", "90 days"].map(|s| s.as_bytes().to_vec()).to_vec(), 3),
        row(13, Group::Interface, "Settings in menu", MENU_SETTINGS, (f.int(MENU_SETTINGS) != Some(0)) as i32, Vec::new(), 1),
        row(14, Group::Language, "Language", LANGUAGE, language(f).map_or(0, |l| 1 + LANGS.iter().position(|&x| x == l).unwrap() as i32), langs, 6),
        row(15, Group::Interface, "Fleet groups", FLEET_GROUPS, d.fleet_groups as i32, Vec::new(), 1),
        row(16, Group::Game, "Game statistics", STATISTICS, d.statistics as i32, Vec::new(), 1),
        row(17, Group::Game, "Record replays", REPLAYS, (f.int(REPLAYS) != Some(0)) as i32, Vec::new(), 1),
        row(18, Group::Interface, "Event log", EVENT_LOG, d.event_log as i32, Vec::new(), 1),
        row(19, Group::Accessibility, "Subtitles", SUBTITLES, d.subtitles as i32, Vec::new(), 1),
        row(20, Group::Accessibility, "Colour-blind colours", COLOUR_BLIND, d.colour_blind as i32, Vec::new(), 1),
        row(21, Group::Display, "Sharp UI", SHARP_UI, d.sharp_ui as i32, Vec::new(), 1),
        row(22, Group::Display, "Sharp text", SHARP_TEXT, d.sharp_text as i32, Vec::new(), 1),
        row(23, Group::Interface, "Starmap mouse camera", STARMAP_MOUSE, d.starmap_mouse as i32, Vec::new(), 1),
        row(24, Group::Game, "High-score entry", HIGH_SCORES, d.high_scores as i32, Vec::new(), 1),
        row(25, Group::Game, "Computer empires", ENHANCED_AI, (f.int(ENHANCED_AI) == Some(1)) as i32, ["Classic", "Enhanced"].map(|s| s.as_bytes().to_vec()).to_vec(), 1),
        row(26, Group::Game, "Title music with intro", TITLE_MUSIC, d.title_music as i32, Vec::new(), 1),
        row(27, Group::Display, "Texture upscaling", TEXTURE_UPSCALE, d.upscale as i32 / 2, ["Off", "2x", "4x"].map(|s| s.as_bytes().to_vec()).to_vec(), 2),
        row(28, Group::Graphics, "Anti-aliasing", ANTI_ALIASING, d.lighting.msaa as i32, Vec::new(), 1),
        row(29, Group::Graphics, "Per-pixel lighting", PIXEL_LIGHTING, d.lighting.per_pixel as i32, Vec::new(), 1),
        row(30, Group::Graphics, "Soft shadows", SOFT_SHADOWS, d.lighting.soft_shadows as i32, Vec::new(), 1),
        row(31, Group::Graphics, "Bloom", BLOOM, d.lighting.bloom as i32, Vec::new(), 1),
        row(32, Group::Game, "Campaign story", STORY_FIXES, (f.int(STORY_FIXES) == Some(1)) as i32, ["Classic", "Enhanced"].map(|s| s.as_bytes().to_vec()).to_vec(), 1),
        row(33, Group::Interface, "UI theme", UI_THEME, theme, themes, n_themes - 1),
        row(34, Group::Graphics, "Battle effects", BATTLE_EFFECTS, d.battle_effects as i32, ["Classic", "Enhanced"].map(|s| s.as_bytes().to_vec()).to_vec(), 1),
        row(35, Group::Sound, "Voice volume", VOICE_VOLUME, voice_volume(f).map_or(0, |v| v / 10 + 1), voices, 11),
        row(36, Group::Game, "Extra game speeds", EXTRA_SPEEDS, (f.int(EXTRA_SPEEDS) == Some(1)) as i32, Vec::new(), 1),
        row(37, Group::Interface, "Menu navigation", crate::menu_flow::KEY, (f.int(crate::menu_flow::KEY) == Some(1)) as i32, ["Classic", "Enhanced"].map(|s| s.as_bytes().to_vec()).to_vec(), 1),
        row(38, Group::Graphics, "Smooth glows", SMOOTH_GLOWS, d.smooth_glows as i32, Vec::new(), 1),
        row(39, Group::Interface, "Modern starmap HUD", MODERN_STARMAP, d.modern_starmap as i32, Vec::new(), 1),
        row(40, Group::Interface, "Modern in-game menu", MODERN_MENU, d.modern_menu as i32, Vec::new(), 1),
    ]
}

/// Ours: the resolution row is off while borderless (the desktop's size).
pub fn dependencies(rows: &mut [Row]) {
    let mode = rows.iter().find(|r| r.key == MODE).map(|r| r.value);
    if let Some(r) = rows.iter_mut().find(|r| r.key == RESOLUTION && r.ours) {
        r.enabled = mode != Some(Mode::Borderless as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let mut f = File::default();
        assert_eq!(Display::from_settings(&f), Display { mode: Mode::Window, size: None, vsync: true, letterbox: false, live_background: true, one_language: true, box_models: true, slider_under: true, articles: true, centred_exit: true, wasd: Wasd::Starmap, fleet_groups: true, statistics: true, event_log: true, subtitles: false, colour_blind: false, sharp_ui: false, sharp_text: false, starmap_mouse: true, high_scores: true, title_music: true, upscale: 1, lighting: Default::default(), battle_effects: false, smooth_glows: true, modern_starmap: true, modern_menu: true });
        f.set(MODE, "2");
        f.set(RESOLUTION, "1280x720");
        f.set(VSYNC, "0");
        f.set(LETTERBOX, "1");
        assert_eq!(Display::from_settings(&f), Display { mode: Mode::Fullscreen, size: Some((1280, 720)), vsync: false, letterbox: true, live_background: true, one_language: true, box_models: true, slider_under: true, articles: true, centred_exit: true, wasd: Wasd::Starmap, fleet_groups: true, statistics: true, event_log: true, subtitles: false, colour_blind: false, sharp_ui: false, sharp_text: false, starmap_mouse: true, high_scores: true, title_music: true, upscale: 1, lighting: Default::default(), battle_effects: false, smooth_glows: true, modern_starmap: true, modern_menu: true });
        assert_eq!(parse_size("12x7"), None);
    }

    #[test]
    fn rows_from_the_file() {
        let mut f = File::default();
        f.set(RESOLUTION, "1280x720");
        f.set(ONE_LANGUAGE, "0");
        let sizes = [(1920, 1080), (1600, 900), (1280, 720)];
        let r = rows(&f, &sizes, (1600, 900));
        assert_eq!(r.iter().map(|r| r.value).collect::<Vec<_>>(), [0, 2, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1]);
        // The theme row: Classic first, Modern by its stored name.
        let theme = r.iter().find(|r| r.key == UI_THEME).unwrap();
        assert_eq!((theme.value, &theme.labels[..2]), (0, &[b"Classic".to_vec(), b"Modern".to_vec()][..]));
        f.set(UI_THEME, "Modern");
        assert_eq!(rows(&f, &sizes, (1600, 900)).iter().find(|r| r.key == UI_THEME).unwrap().value, 1);
        assert_eq!((r[1].states, r[1].labels[2].as_slice()), (2, &b"1280x720"[..]));
        // Without a size in the file: the window's; a size not offered: the next smaller one.
        assert_eq!(rows(&File::default(), &sizes, (1600, 900))[1].value, 1);
        assert_eq!(rows(&File::default(), &sizes, (1700, 1000))[1].value, 1);
        let mut r = r;
        r[0].value = 1;
        dependencies(&mut r);
        assert!(!r[1].enabled);
    }

    #[test]
    fn language_row() {
        let mut f = File::default();
        assert_eq!(language(&f), None);
        f.set(LANGUAGE, "3");
        assert_eq!(language(&f), Some(Lang::De));
        let r = rows(&f, &[(800, 600)], (800, 600));
        let row = r.iter().find(|r| r.key == LANGUAGE).unwrap();
        assert_eq!((row.value, row.labels[row.value as usize].as_slice()), (3, &b"Deutsch"[..]));
        f.set(LANGUAGE, "7");
        assert_eq!(language(&f), None);
    }

    #[test]
    fn voice_volume_row() {
        let mut f = File::default();
        assert_eq!(voice_volume(&f), None);
        f.set(VOICE_VOLUME, "1");
        assert_eq!(voice_volume(&f), Some(0));
        f.set(VOICE_VOLUME, "9");
        assert_eq!(voice_volume(&f), Some(80));
        let r = rows(&f, &[(800, 600)], (800, 600));
        let row = r.iter().find(|r| r.key == VOICE_VOLUME).unwrap();
        assert_eq!((row.value, row.labels[row.value as usize].as_slice(), row.labels.len() as i32), (9, &b"80%"[..], row.states + 1));
        f.set(VOICE_VOLUME, "12");
        assert_eq!(voice_volume(&f), None);
    }
}
