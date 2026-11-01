//! The remake's display settings (an enhanced setting, docs/USER_GUIDE.md → Options): window, borderless or
//! fullscreen, the resolution, vsync and the 4:3 letterbox, with the live background (ADR 0015) and the names'
//! language (ADR 0017), the clean name entry (ADR 0024), the info boxes' models (ADR 0025), the battle's slider (ADR 0027), the Hungarian articles (ADR 0028) the centred Exit label (ADR 0029) the W/A/S/D camera (ADR 0030), the autosave's cadence (ADR 0032), the menu Settings label and Language row (ADR 0031), the fleet groups (ADR 0035), the game statistics (ADR 0034) the replays (ADR 0038), the event log (ADR 0040), the subtitles and the colour-blind empire colours (ADR 0036), the sharp UI (ADR 0043), the sharp text (ADR 0046); kept in our settings file and shown as extra rows on the Options screen's game page
//! (`crate::options`, in the original's font and colours under a "Remake" heading). The original has a fixed
//! 640×480 or 800×600 screen and none of these rows.

use ig2_formats::options::Options as File;
use ig2_formats::text::Lang;
use winit::monitor::MonitorHandle;
use winit::window::Fullscreen;

use crate::options::Row;

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

/// The remake's rows for the game page: `sizes` the resolutions to offer, `current` the window's size (the
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
    let row = |n: i32, name: &str, key: &str, value: i32, labels: Vec<Vec<u8>>, states: i32| Row {
        name: name.as_bytes().to_vec(),
        key: key.into(),
        enabled: true,
        value,
        states,
        page: 2,
        id: ID_MODE + n,
        dep: 0,
        mp_ok: true,
        labels,
        ours: true,
        locked: false,
    };
    let modes = ["Window", "Borderless", "Fullscreen"].map(|s| s.as_bytes().to_vec()).to_vec();
    let labels: Vec<Vec<u8>> = sizes.iter().map(|(w, h)| format!("{w}x{h}").into_bytes()).collect();
    let n = labels.len() as i32;
    let langs = ["Default", "English", "Magyar", "Deutsch", "Francais", "Espanol", "Italiano"].map(|s| s.as_bytes().to_vec()).to_vec();
    vec![
        row(0, "Display", MODE, d.mode as i32, modes, 2),
        row(1, "Resolution", RESOLUTION, res as i32, labels, n - 1),
        row(2, "Vertical sync", VSYNC, d.vsync as i32, Vec::new(), 1),
        row(3, "Letterbox 4:3", LETTERBOX, d.letterbox as i32, Vec::new(), 1),
        row(4, "Live background", LIVE_BACKGROUND, d.live_background as i32, Vec::new(), 1),
        row(5, "Names in game language", ONE_LANGUAGE, d.one_language as i32, Vec::new(), 1),
        row(6, "Clean name entry", CLEAN_ENTRY, (f.int(CLEAN_ENTRY) != Some(0)) as i32, Vec::new(), 1),
        row(7, "Models inside boxes", BOX_MODELS, d.box_models as i32, Vec::new(), 1),
        row(8, "Slider under label", SLIDER_UNDER, d.slider_under as i32, Vec::new(), 1),
        row(9, "Hungarian articles", ARTICLES, d.articles as i32, Vec::new(), 1),
        row(10, "Centred Exit label", CENTRED_EXIT, d.centred_exit as i32, Vec::new(), 1),
        row(11, "WASD camera", WASD, d.wasd as i32, ["Off", "Starmap", "Both"].map(|s| s.as_bytes().to_vec()).to_vec(), 2),
        row(12, "Autosave every", AUTOSAVE, f.int(AUTOSAVE).unwrap_or(0).clamp(0, 3), ["Original", "10 days", "30 days", "90 days"].map(|s| s.as_bytes().to_vec()).to_vec(), 3),
        row(13, "Settings in menu", MENU_SETTINGS, (f.int(MENU_SETTINGS) != Some(0)) as i32, Vec::new(), 1),
        row(14, "Language", LANGUAGE, language(f).map_or(0, |l| 1 + LANGS.iter().position(|&x| x == l).unwrap() as i32), langs, 6),
        row(15, "Fleet groups", FLEET_GROUPS, d.fleet_groups as i32, Vec::new(), 1),
        row(16, "Game statistics", STATISTICS, d.statistics as i32, Vec::new(), 1),
        row(17, "Record replays", REPLAYS, (f.int(REPLAYS) != Some(0)) as i32, Vec::new(), 1),
        row(18, "Event log", EVENT_LOG, d.event_log as i32, Vec::new(), 1),
        row(19, "Subtitles", SUBTITLES, d.subtitles as i32, Vec::new(), 1),
        row(20, "Colour-blind colours", COLOUR_BLIND, d.colour_blind as i32, Vec::new(), 1),
        row(21, "Sharp UI", SHARP_UI, d.sharp_ui as i32, Vec::new(), 1),
        row(22, "Sharp text", SHARP_TEXT, d.sharp_text as i32, Vec::new(), 1),
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
        assert_eq!(Display::from_settings(&f), Display { mode: Mode::Window, size: None, vsync: true, letterbox: false, live_background: true, one_language: true, box_models: true, slider_under: true, articles: true, centred_exit: true, wasd: Wasd::Starmap, fleet_groups: true, statistics: true, event_log: true, subtitles: false, colour_blind: false, sharp_ui: false, sharp_text: false });
        f.set(MODE, "2");
        f.set(RESOLUTION, "1280x720");
        f.set(VSYNC, "0");
        f.set(LETTERBOX, "1");
        assert_eq!(Display::from_settings(&f), Display { mode: Mode::Fullscreen, size: Some((1280, 720)), vsync: false, letterbox: true, live_background: true, one_language: true, box_models: true, slider_under: true, articles: true, centred_exit: true, wasd: Wasd::Starmap, fleet_groups: true, statistics: true, event_log: true, subtitles: false, colour_blind: false, sharp_ui: false, sharp_text: false });
        assert_eq!(parse_size("12x7"), None);
    }

    #[test]
    fn rows_from_the_file() {
        let mut f = File::default();
        f.set(RESOLUTION, "1280x720");
        f.set(ONE_LANGUAGE, "0");
        let sizes = [(1920, 1080), (1600, 900), (1280, 720)];
        let r = rows(&f, &sizes, (1600, 900));
        assert_eq!(r.iter().map(|r| r.value).collect::<Vec<_>>(), [0, 2, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 1, 1, 1, 0, 0, 0, 0]);
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
}
