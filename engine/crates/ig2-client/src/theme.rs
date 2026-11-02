//! Ours (ADR 0056): UI themes for the remake's own screens. A theme is colours, the faders' frame style and the
//! font's sizes, read from a small TOML file: the built-in Modern (`themes/modern.toml`) or a mod
//! folder's `theme.toml`. Classic is no theme: every screen draws as before.

use crate::font::Virtual;
use crate::render::{Frame, TexId};
use std::sync::RwLock;

pub const CLASSIC: &str = "Classic";
pub const MODERN: &str = "Modern";
const MODERN_FILE: &str = include_str!("themes/modern.toml");
/// A mod folder's theme file.
pub const FILE: &str = "theme.toml";

/// The look. Box colours and transparencies 0..255 (the faders'), text and quad colours 0..1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub panel: [f32; 3],
    pub panel_edge: [f32; 3],
    /// The fill's and the edge's transparency.
    pub panel_clear: [f32; 2],
    pub tab: [f32; 3],
    pub tab_on: [f32; 3],
    pub button: [f32; 3],
    pub button_edge: [f32; 3],
    pub button_clear: [f32; 2],
    pub text: [f32; 3],
    pub dim: [f32; 3],
    pub off: [f32; 3],
    pub accent: [f32; 3],
    pub hot: [f32; 3],
    /// The switch's and the slider's track when on and off, their knob.
    pub on_track: [f32; 3],
    pub off_track: [f32; 3],
    pub knob: [f32; 3],
    /// The font's sizes: titles, everything else (at most 2: the screens' columns are laid out for it).
    pub big: usize,
    pub small: usize,
}

impl Theme {
    /// Built-in Modern.
    pub fn modern() -> Self {
        Self::parse_over(&Self::ZERO, MODERN_FILE).expect("themes/modern.toml")
    }

    /// A theme file over Modern's values.
    pub fn parse(text: &str) -> Result<Self, String> {
        Self::parse_over(&Self::modern(), text)
    }

    const ZERO: Theme = Theme {
        panel: [0.0; 3], panel_edge: [0.0; 3], panel_clear: [0.0; 2], tab: [0.0; 3], tab_on: [0.0; 3], button: [0.0; 3], button_edge: [0.0; 3], button_clear: [0.0; 2],
        text: [0.0; 3], dim: [0.0; 3], off: [0.0; 3], accent: [0.0; 3], hot: [0.0; 3], on_track: [0.0; 3], off_track: [0.0; 3], knob: [0.0; 3], big: 0, small: 0,
    };

    fn parse_over(base: &Theme, text: &str) -> Result<Self, String> {
        let doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string().trim_end().to_owned())?;
        let mut t = *base;
        for (key, item) in doc.iter() {
            let v = item.as_value().ok_or_else(|| format!("{key}: expected a value, not a table"))?;
            let int = |lo: i64, hi: i64| v.as_integer().filter(|n| (lo..=hi).contains(n)).ok_or_else(|| format!("{key}: expected a whole number {lo}..{hi}"));
            let list = |n: usize| -> Result<Vec<f32>, String> {
                let a = v.as_array().filter(|a| a.len() == n).ok_or_else(|| format!("{key}: expected [{}] of whole numbers 0..255", vec!["n"; n].join(", ")))?;
                a.iter().map(|x| x.as_integer().filter(|n| (0..=255).contains(n)).map(|n| n as f32).ok_or_else(|| format!("{key}: expected whole numbers 0..255"))).collect()
            };
            let rgb = || list(3).map(|c| [c[0], c[1], c[2]]);
            let ink = || rgb().map(|c| c.map(|c| c / 255.0));
            let two = || list(2).map(|c| [c[0], c[1]]);
            match key {
                "panel" => t.panel = rgb()?,
                "panel_edge" => t.panel_edge = rgb()?,
                "panel_transparency" => t.panel_clear = two()?,
                "tab" => t.tab = rgb()?,
                "tab_selected" => t.tab_on = rgb()?,
                "button" => t.button = rgb()?,
                "button_edge" => t.button_edge = rgb()?,
                "button_transparency" => t.button_clear = two()?,
                "text" => t.text = ink()?,
                "text_dim" => t.dim = ink()?,
                "text_off" => t.off = ink()?,
                "accent" => t.accent = ink()?,
                "hover" => t.hot = ink()?,
                "track_on" => t.on_track = ink()?,
                "track_off" => t.off_track = ink()?,
                "knob" => t.knob = ink()?,
                "font_title" => t.big = int(0, 4)? as usize,
                "font_text" => t.small = int(0, 2)? as usize,
                _ => return Err(format!("{key}: unknown key (keys: {})", KEYS.join(", "))),
            }
        }
        Ok(t)
    }

    /// The Options screen's Classic look: the remake's other menu screens' boxes, the game's palette for text.
    pub fn classic(col: impl Fn(usize, f32) -> [f32; 3]) -> Self {
        Theme {
            panel: [0.0, 14.0, 22.0],
            panel_edge: [0.0, 40.0, 60.0],
            panel_clear: [0.0, 20.0],
            tab: [0.0, 10.0, 0.0],
            tab_on: [0.0, 40.0, 60.0],
            button: [0.0, 10.0, 0.0],
            button_edge: [0.0, 5.0, 0.0],
            button_clear: [60.0, 30.0],
            text: col(1, 100.0),
            dim: col(1, 60.0),
            off: col(1, 40.0),
            accent: col(2, 100.0),
            hot: col(0x25, 100.0),
            on_track: [0.0, 0.35, 0.5],
            off_track: [0.1, 0.13, 0.14],
            knob: col(1, 100.0),
            big: 4,
            small: 2,
        }
    }

    /// A panel: the box, its edge.
    pub fn panel(&self, f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4]) {
        crate::kit::fader::fader(f, white, v, r, self.panel, self.panel_clear[0], self.panel_edge, self.panel_clear[1]);
    }

    /// A button's box.
    pub fn button(&self, f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4]) {
        crate::kit::fader::fader(f, white, v, r, self.button, self.button_clear[0], self.button_edge, self.button_clear[1]);
    }
}

const KEYS: [&str; 18] = [
    "panel", "panel_edge", "panel_transparency", "tab", "tab_selected", "button", "button_edge", "button_transparency", "text", "text_dim", "text_off",
    "accent", "hover", "track_on", "track_off", "knob", "font_title", "font_text",
];

/// The chosen theme, else Classic's look (the Options screen's; the other screens' Classic draws with it too).
pub fn look(d: &crate::options::Draw) -> Theme {
    current().unwrap_or_else(|| Theme::classic(|i, m| d.col(i, m)))
}

/// The chosen theme (None = Classic), set from the settings (`UiTheme`).
static CURRENT: RwLock<Option<Theme>> = RwLock::new(None);

pub fn current() -> Option<Theme> {
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

/// Chooses the theme by its name: Classic, Modern or a mod folder's; one that doesn't load is Classic (the error
/// written to the log).
pub fn set(name: &str) {
    let t = match name {
        "" | CLASSIC => None,
        MODERN => Some(Theme::modern()),
        _ => load(name).map_err(|e| eprintln!("ig2-client: theme {name}: {e}")).ok(),
    };
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = t;
}

fn load(name: &str) -> Result<Theme, String> {
    let m = ig2_formats::vfs::Mod::available().into_iter().find(|m| m.name == name).ok_or("no such mod")?;
    load_from(&m)
}

/// A mod's theme file, a folder's or a zip's (ADR 0068).
fn load_from(m: &ig2_formats::vfs::Mod) -> Result<Theme, String> {
    let text = String::from_utf8(m.read(FILE)?).map_err(|_| format!("{FILE}: not UTF-8 text"))?;
    Theme::parse(&text)
}

/// The themes offered: Classic, Modern, then each mod (folder or zip) with a theme file that parses, in name order.
pub fn offered() -> Vec<String> {
    let mut v = vec![CLASSIC.to_owned(), MODERN.to_owned()];
    let mods: Vec<String> = ig2_formats::vfs::Mod::available()
        .into_iter()
        .filter(|m| m.name != CLASSIC && m.name != MODERN && load_from(m).is_ok())
        .map(|m| m.name)
        .collect();
    v.extend(mods);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Modern is the Options screen's shipped look (ADR 0053's constants, to the file's 1/255).
    #[test]
    fn modern_is_the_options_look() {
        let t = Theme::modern();
        assert_eq!((t.panel, t.panel_edge, t.tab, t.tab_on, t.button), ([4.0, 12.0, 24.0], [40.0, 90.0, 130.0], [10.0, 30.0, 50.0], [20.0, 70.0, 110.0], [10.0, 40.0, 70.0]));
        let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.5 / 255.0 + 1e-6);
        for (a, b) in [(t.text, [0.86, 0.9, 0.96]), (t.dim, [0.5, 0.58, 0.68]), (t.accent, [0.4, 0.78, 1.0]), (t.hot, [1.0, 0.86, 0.45]), (t.on_track, [0.16, 0.5, 0.75]), (t.knob, [0.9, 0.94, 1.0])] {
            assert!(near(a, b), "{a:?} {b:?}");
        }
        assert_eq!((t.big, t.small, t.panel_clear, t.button_clear), (4, 2, [0.0; 2], [0.0; 2]));
    }

    /// A theme file sets only its keys over Modern; a bad one errors naming the key.
    #[test]
    fn a_theme_file_parses_and_a_bad_one_errors() {
        let t = Theme::parse("# amber\npanel = [30, 20, 0]\nhover = [255, 255, 255]\nfont_text = 1\n").unwrap();
        assert_eq!((t.panel, t.hot, t.small, t.tab), ([30.0, 20.0, 0.0], [1.0; 3], 1, Theme::modern().tab));
        assert_eq!(Theme::parse("").unwrap(), Theme::modern());
        for (bad, says) in [
            ("panel = [1, 2]", "panel: expected [n, n, n]"),
            ("text = [0, 0, 300]", "text: expected whole numbers 0..255"),
            ("font_text = 3", "font_text: expected a whole number 0..2"),
            ("font_title = \"big\"", "font_title: expected a whole number 0..4"),
            ("colour = [1, 2, 3]", "colour: unknown key"),
            ("[panel]\nr = 1", "panel: expected a value"),
            ("panel = [1, 2, 3", "TOML parse error"),
        ] {
            let e = Theme::parse(bad).unwrap_err();
            assert!(e.contains(says), "{bad}: {e}");
        }
    }
}
