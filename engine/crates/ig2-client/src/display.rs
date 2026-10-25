//! The remake's display settings (an enhanced setting, docs/USER_GUIDE.md → Options): window, borderless or
//! fullscreen, the resolution, vsync and the 4:3 letterbox, with the live background (ADR 0015) and the names'
//! language (ADR 0017); kept in our settings file and shown as extra rows on the Options screen's game page
//! (`crate::options`, in the original's font and colours under a "Remake" heading). The original has a fixed
//! 640×480 or 800×600 screen and none of these rows.

use ig2_formats::options::Options as File;
use winit::monitor::MonitorHandle;
use winit::window::Fullscreen;

use crate::options::Row;

pub const MODE: &str = "DisplayMode";
pub const RESOLUTION: &str = "Resolution";
pub const VSYNC: &str = "VSync";
pub const LETTERBOX: &str = "Letterbox";
pub const LIVE_BACKGROUND: &str = "LiveBackground";
pub const ONE_LANGUAGE: &str = "OneLanguage";

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
    };
    let modes = ["Window", "Borderless", "Fullscreen"].map(|s| s.as_bytes().to_vec()).to_vec();
    let labels: Vec<Vec<u8>> = sizes.iter().map(|(w, h)| format!("{w}x{h}").into_bytes()).collect();
    let n = labels.len() as i32;
    vec![
        row(0, "Display", MODE, d.mode as i32, modes, 2),
        row(1, "Resolution", RESOLUTION, res as i32, labels, n - 1),
        row(2, "Vertical sync", VSYNC, d.vsync as i32, Vec::new(), 1),
        row(3, "Letterbox 4:3", LETTERBOX, d.letterbox as i32, Vec::new(), 1),
        row(4, "Live background", LIVE_BACKGROUND, d.live_background as i32, Vec::new(), 1),
        row(5, "Names in game language", ONE_LANGUAGE, d.one_language as i32, Vec::new(), 1),
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
        assert_eq!(Display::from_settings(&f), Display { mode: Mode::Window, size: None, vsync: true, letterbox: false, live_background: true, one_language: true });
        f.set(MODE, "2");
        f.set(RESOLUTION, "1280x720");
        f.set(VSYNC, "0");
        f.set(LETTERBOX, "1");
        assert_eq!(Display::from_settings(&f), Display { mode: Mode::Fullscreen, size: Some((1280, 720)), vsync: false, letterbox: true, live_background: true, one_language: true });
        assert_eq!(parse_size("12x7"), None);
    }

    #[test]
    fn rows_from_the_file() {
        let mut f = File::default();
        f.set(RESOLUTION, "1280x720");
        f.set(ONE_LANGUAGE, "0");
        let sizes = [(1920, 1080), (1600, 900), (1280, 720)];
        let r = rows(&f, &sizes, (1600, 900));
        assert_eq!(r.iter().map(|r| r.value).collect::<Vec<_>>(), [0, 2, 1, 0, 1, 0]);
        assert_eq!((r[1].states, r[1].labels[2].as_slice()), (2, &b"1280x720"[..]));
        // Without a size in the file: the window's; a size not offered: the next smaller one.
        assert_eq!(rows(&File::default(), &sizes, (1600, 900))[1].value, 1);
        assert_eq!(rows(&File::default(), &sizes, (1700, 1000))[1].value, 1);
        let mut r = r;
        r[0].value = 1;
        dependencies(&mut r);
        assert!(!r[1].enabled);
    }
}
