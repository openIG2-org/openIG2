//! Ours (ADR 0059): the enhanced menu navigation (`MenuNavigation` 1, re/screens/mainscr.md §15): the main menu's
//! grouped panel, the Back label of the screens without a back of their own, and the save Continue loads.

use std::path::{Path, PathBuf};

use crate::font::Align;
use crate::options::Draw;
use crate::render::Frame;

/// The option's key in our settings file (0 classic, the default; 1 enhanced).
pub const KEY: &str = "MenuNavigation";

/// A panel entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Continue,
    QuickGame,
    NewGame,
    Load,
    Multiplayer,
    History,
    Replays,
    Settings,
    Quit,
}

impl Item {
    /// The remake text table's key (ADR 0042).
    fn label(self) -> &'static str {
        ["Continue", "Quick game", "New game", "Load game", "Multiplayer", "History", "Replays", "Settings", "Quit"][self as usize]
    }
}

/// The panel's lines: group headings and entries.
const LINES: [(Option<&str>, Option<Item>); 11] = [
    (Some("Play"), None),
    (None, Some(Item::Continue)),
    (None, Some(Item::QuickGame)),
    (None, Some(Item::NewGame)),
    (None, Some(Item::Load)),
    (None, Some(Item::Multiplayer)),
    (Some("Extras"), None),
    (None, Some(Item::History)),
    (None, Some(Item::Replays)),
    (None, Some(Item::Settings)),
    (None, Some(Item::Quit)),
];

/// The panel's box: left of the rings (ring 4's box starts at x 120), its lines [`LINE`] apart from `PANEL[1] + 6`.
pub const PANEL: [f32; 4] = [4.0, 110.0, 112.0, 262.0];
const LINE: f32 = 22.0;
/// The gap before Extras and before Settings.
const GAP: f32 = 6.0;
/// The Back label, where the History item sits on the classic menu (`stats_screen::HISTORY_ITEM`).
pub const BACK: [f32; 4] = [8.0, 448.0, 96.0, 22.0];

/// The lines' boxes, History left out when `history` is off.
fn layout(history: bool) -> Vec<(Option<&'static str>, Option<Item>, [f32; 4])> {
    let mut y = PANEL[1] + 6.0;
    let mut out = Vec::new();
    for (head, item) in LINES {
        if item == Some(Item::History) && !history {
            continue;
        }
        if head == Some("Extras") || item == Some(Item::Settings) {
            y += GAP;
        }
        out.push((head, item, [PANEL[0] + 4.0, y, PANEL[2] - 8.0, LINE - 2.0]));
        y += LINE;
    }
    out
}

/// The entry under `at`.
pub fn item_at(at: [f32; 2], history: bool) -> Option<Item> {
    layout(history).into_iter().find(|&(_, i, r)| i.is_some() && crate::kit::inside_f(r, at)).and_then(|(_, i, _)| i)
}

/// The panel: in the chosen UI theme (ADR 0056), else as the Settings label (the button font, colour 1, colour 2
/// under the pointer, faded in with the scene). Continue is dimmed without a save.
pub fn draw_panel(f: &mut Frame, d: &Draw, history: bool, can_continue: bool, at: [f32; 2], fade: f32) {
    let theme = crate::theme::current();
    if let Some(t) = &theme {
        t.panel(f, d.white, &d.v, PANEL);
    }
    for (head, item, r) in layout(history) {
        let (text, c, size) = match (head, item, &theme) {
            (Some(h), _, Some(t)) => (h, t.accent, t.small),
            (Some(h), _, None) => (h, d.col(1, 60.0 * fade), 3),
            (_, Some(i), t) => {
                let on = i != Item::Continue || can_continue;
                let hot = on && crate::kit::inside_f(r, at);
                if let Some(t) = t {
                    t.button(f, d.white, &d.v, r);
                }
                let c = match t {
                    Some(t) => if !on { t.off } else if hot { t.hot } else { t.text },
                    None => d.col(if hot { 2 } else { 1 }, if on { 100.0 } else { 40.0 } * fade),
                };
                (i.label(), c, t.map_or(3, |t| t.small))
            }
            _ => continue,
        };
        d.font.text(f, &d.v, &d.texts.ours(text), r[0] + r[2] / 2.0, r[1] + 4.0, 0, size, Align::Centre, c, None);
    }
}

/// The Back label, drawn as the panel's entries.
pub fn draw_back(f: &mut Frame, d: &Draw, at: [f32; 2], fade: f32) {
    let hot = crate::kit::inside_f(BACK, at);
    let (c, size) = match crate::theme::current() {
        Some(t) => {
            t.button(f, d.white, &d.v, BACK);
            (if hot { t.hot } else { t.text }, t.small)
        }
        None => (d.col(if hot { 2 } else { 1 }, 100.0 * fade), 3),
    };
    d.font.text(f, &d.v, &d.texts.ours("Back"), BACK[0] + BACK[2] / 2.0, BACK[1] + 4.0, 0, size, Align::Centre, c, None);
}

/// Continue's saves: `dir`'s slots (`save1..=24` in ours or the original's format, the autosaves in 23 and 24 among
/// them) and the quick save, newest first by modification time (Continue loads the first that reads).
pub fn saves_newest_first(dir: &Path) -> Vec<PathBuf> {
    let save = |name: &str| {
        if name.eq_ignore_ascii_case(crate::options::QUICKSAVE) {
            return true;
        }
        let lower = name.to_ascii_lowercase();
        let n = lower.strip_prefix("save").and_then(|s| s.strip_suffix(".ig2s").or_else(|| s.strip_suffix(".sav")));
        n.and_then(|n| n.parse::<usize>().ok()).is_some_and(|n| (1..=crate::options::SLOTS).contains(&n))
    };
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_str().is_some_and(save))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.0));
    v.into_iter().map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_items_and_history() {
        let centre = |i: Item, h: bool| layout(h).into_iter().find(|&(_, x, _)| x == Some(i)).map(|(_, _, r)| [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]).unwrap();
        for i in [Item::Continue, Item::QuickGame, Item::NewGame, Item::Load, Item::Multiplayer, Item::History, Item::Replays, Item::Settings, Item::Quit] {
            assert_eq!(item_at(centre(i, true), true), Some(i));
        }
        assert!(layout(false).iter().all(|&(_, i, _)| i != Some(Item::History)));
        // The headings take no click; the panel stays left of the rings' boxes and above the Back label.
        assert_eq!(item_at([60.0, PANEL[1] + 12.0], true), None);
        let last = layout(true).last().unwrap().2;
        assert!(PANEL[0] + PANEL[2] < 120.0 && last[1] + last[3] <= PANEL[1] + PANEL[3] && PANEL[1] + PANEL[3] < BACK[1]);
    }

    #[test]
    fn continue_takes_the_newest_save() {
        let dir = std::env::temp_dir().join(format!("ig2-menu-flow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(saves_newest_first(&dir).is_empty());
        let t0 = std::time::SystemTime::now() - std::time::Duration::from_secs(100);
        for (k, name) in ["save3.sav", "save24.ig2s", "quicksave.ig2s", "save25.ig2s", "notes.txt"].into_iter().enumerate() {
            let f = std::fs::File::create(dir.join(name)).unwrap();
            f.set_modified(t0 + std::time::Duration::from_secs(10 * k as u64)).unwrap();
        }
        // save25 and notes.txt are newer but no slots.
        assert_eq!(saves_newest_first(&dir), ["quicksave.ig2s", "save24.ig2s", "save3.sav"].map(|n| dir.join(n)));
        std::fs::File::options().write(true).open(dir.join("save3.sav")).unwrap().set_modified(t0 + std::time::Duration::from_secs(90)).unwrap();
        assert_eq!(saves_newest_first(&dir)[0], dir.join("save3.sav"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
