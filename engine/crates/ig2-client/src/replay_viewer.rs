//! Ours (ADR 0038 §6): the replay viewer. The main menu's "Replays" item lists the recorded games ([`ReplayList`]);
//! one opens read-only: the world plays the replay back step by step ([`Viewer`], `World::watch`) and draws it as a
//! spectator sees a game (ADR 0026), while the screens' orders are dropped. A bar under the top bar ([`Bar`]) holds
//! play/pause, the speed steps and the progress line; at the end the window goes back to the main menu. The texts are
//! English only (the game has no strings for them).

use std::path::{Path, PathBuf};

use ig2_net::replay::{Diverged, Locals, Playback, Replay};

use crate::font::Align;
use crate::options::Draw;
use crate::render::{Blend, Frame, Instance};

/// The speed steps: steps per window tick, times the replay's ticks per step.
pub const SPEEDS: [u32; 4] = [1, 2, 4, 8];

/// How a playback ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    Finished,
    Diverged(Diverged),
}

/// A replay being watched: its playback (whose game is a stand-in while the world holds the real one) and the
/// viewer's own controls.
pub struct Viewer<'r> {
    pub playback: Playback<'r>,
    pub total: u64,
    pub ticks_per_step: u32,
    pub multiplayer: bool,
    /// Index into [`SPEEDS`].
    pub speed: usize,
    pub paused: bool,
    /// Window ticks owed to the next step (a multiplayer step is a turn of two ticks).
    due: u32,
    /// Single player: the recorded screen state after the last step. The window's own writes (its view, screen,
    /// pause) are put aside around each step, so they never reach the recorded game.
    pub locals: Locals,
    pub end: Option<End>,
    /// Single player: the message line's text last shown (the recorded pops move the line, [`crate::world::World::pop_message`]).
    pub line: Vec<u8>,
}

impl<'r> Viewer<'r> {
    /// `game`: the replay's start, which the world holds (the playback holds a stand-in).
    pub fn new(playback: Playback<'r>, replay: &Replay, game: &ig2_sim::game::Game) -> Self {
        let locals = Locals::read(game);
        Self { playback, total: replay.steps(), ticks_per_step: replay.ticks_per_step(), multiplayer: replay.meta.multiplayer, speed: 0, paused: false, due: 0, locals, end: None, line: Vec::new() }
    }

    /// The steps one window tick (50 ms) runs: at 1× a step per its ticks, as the game ran.
    pub fn steps_due(&mut self) -> u32 {
        if self.paused || self.end.is_some() {
            return 0;
        }
        self.due += SPEEDS[self.speed];
        let n = self.due / self.ticks_per_step;
        self.due %= self.ticks_per_step;
        n
    }

    pub fn bar(&self, hover: Option<Spot>) -> Bar {
        Bar { pos: self.playback.position(), total: self.total, ticks_per_step: self.ticks_per_step, paused: self.paused, speed: self.speed, hover }
    }

    pub fn act(&mut self, a: Act) {
        match a {
            Act::Pause => self.paused = !self.paused,
            Act::Speed(k) => self.speed = k.min(SPEEDS.len() - 1),
            Act::Exit => {}
        }
    }
}

/// What the bar or a key asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Pause,
    Speed(usize),
    Exit,
}

/// The viewer's keys: Space plays or pauses, 1–4 pick the speed.
pub fn key_act(c: &str) -> Option<Act> {
    match c {
        " " => Some(Act::Pause),
        "1" | "2" | "3" | "4" => Some(Act::Speed(c.as_bytes()[0] as usize - b'1' as usize)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    Pause,
    Speed(usize),
    Exit,
}

/// The bar (virtual pixels), under the top bar.
const BAR: [f32; 4] = [160.0, 24.0, 320.0, 42.0];
const PAUSE: [f32; 4] = [166.0, 29.0, 56.0, 16.0];
const EXIT: [f32; 4] = [418.0, 29.0, 56.0, 16.0];
const LINE_Y: f32 = 56.0;
const LINE_X: [f32; 2] = [168.0, 392.0];

fn speed_box(k: usize) -> [f32; 4] {
    [232.0 + k as f32 * 46.0, 29.0, 40.0, 16.0]
}

/// The bar as one frame draws it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub pos: u64,
    pub total: u64,
    pub ticks_per_step: u32,
    pub paused: bool,
    pub speed: usize,
    pub hover: Option<Spot>,
}

/// `m:ss` of `ticks` 50 ms ticks.
fn clock(ticks: u64) -> String {
    let s = ticks / 20;
    format!("{}:{:02}", s / 60, s % 60)
}

impl Bar {
    pub fn spot(at: [f32; 2]) -> Option<Spot> {
        let inside = |r| crate::kit::inside_f(r, at);
        if inside(PAUSE) {
            return Some(Spot::Pause);
        }
        if inside(EXIT) {
            return Some(Spot::Exit);
        }
        (0..SPEEDS.len()).find(|&k| inside(speed_box(k))).map(Spot::Speed)
    }

    pub fn press(at: [f32; 2]) -> Option<Act> {
        Some(match Self::spot(at)? {
            Spot::Pause => Act::Pause,
            Spot::Speed(k) => Act::Speed(k),
            Spot::Exit => Act::Exit,
        })
    }

    /// The bar's whole box: presses there go to nothing under it.
    pub fn covers(at: [f32; 2]) -> bool {
        crate::kit::inside_f(BAR, at)
    }

    pub fn draw(&self, f: &mut Frame, d: &Draw) {
        let (white, dim, hot) = (d.col(1, 100.0), d.col(1, 60.0), d.col(0x25, 100.0));
        crate::kit::fader::fader(f, d.white, &d.v, BAR, [0.0, 14.0, 22.0], 0.0, [0.0, 40.0, 60.0], 20.0);
        let button = |f: &mut Frame, r: [f32; 4], label: &str, lit: bool, hover: bool| {
            if lit {
                crate::kit::fader::fader(f, d.white, &d.v, r, [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
            }
            let c = if hover || lit { hot } else { white };
            d.font.text(f, &d.v, label.as_bytes(), r[0] + r[2] / 2.0, r[1] + 5.0, 0, 2, Align::Centre, c, None);
        };
        button(f, PAUSE, if self.paused { "Play" } else { "Pause" }, false, self.hover == Some(Spot::Pause));
        for (k, s) in SPEEDS.iter().enumerate() {
            button(f, speed_box(k), &format!("{s}x"), k == self.speed, self.hover == Some(Spot::Speed(k)));
        }
        button(f, EXIT, "Exit", false, self.hover == Some(Spot::Exit));
        let [x0, x1] = LINE_X;
        let t = if self.total > 0 { (self.pos as f32 / self.total as f32).min(1.0) } else { 1.0 };
        let line = |f: &mut Frame, a: f32, b: f32, c: [f32; 3], w: f32| f.push(d.white, Blend::Alpha, Instance::line(d.v.px(a, LINE_Y), d.v.px(b, LINE_Y), w * d.v.k.max(1.0), [c[0], c[1], c[2], 1.0]));
        line(f, x0, x1, dim, 2.0);
        line(f, x0, x0 + t * (x1 - x0), hot, 3.0);
        let k = u64::from(self.ticks_per_step);
        let time = format!("{} / {}", clock(self.pos * k), clock(self.total * k));
        d.font.text(f, &d.v, time.as_bytes(), 398.0, LINE_Y - 4.0, 0, 2, Align::Left, white, None);
    }
}

// ------------------------------------------------------------------------------------------------ the list

/// Where the replays live (`World::record_replay`'s folder).
pub fn replays_dir() -> Option<PathBuf> {
    crate::options::slot_dir().map(|d| d.join("replays"))
}

/// A replay file in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    /// "2026-11-01 04:08:26" from the name (`options::replay_name`, UTC).
    pub when: String,
    pub multiplayer: bool,
    pub kb: u64,
}

impl Entry {
    fn of(path: PathBuf, len: u64) -> Option<Self> {
        let stem = path.file_stem()?.to_str()?;
        let (date, rest) = stem.split_once('_')?;
        let (time, mode) = rest.split_once('_')?;
        if time.len() != 6 || !matches!(mode, "sp" | "mp") {
            return None;
        }
        let when = format!("{date} {}:{}:{}", &time[..2], &time[2..4], &time[4..]);
        Some(Self { when, multiplayer: mode == "mp", kb: len.div_ceil(1024), path })
    }
}

/// The replays in `dir`, newest first (none without the folder).
pub fn list(dir: &Path) -> Result<Vec<Entry>, String> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    let mut out: Vec<Entry> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "ig2r"))
        .filter_map(|e| Entry::of(e.path(), e.metadata().map_or(0, |m| m.len())))
        .collect();
    out.sort_by(|a, b| b.path.cmp(&a.path));
    Ok(out)
}

/// The main menu's "Replays" item (ours), above "History" (`stats_screen::HISTORY_ITEM`).
pub const REPLAYS_ITEM: [f32; 4] = [8.0, 424.0, 96.0, 22.0];

/// The "Replays" item as the "History" item is drawn.
pub fn draw_item(f: &mut Frame, d: &Draw, hot: bool, fade: f32) {
    let [x, y, w, _] = REPLAYS_ITEM;
    d.font.text(f, &d.v, b"Replays", x + w / 2.0, y + 4.0, 0, 3, Align::Centre, d.col(if hot { 2 } else { 1 }, 100.0 * fade), None);
}

const PANEL: [f32; 4] = [20.0, 16.0, 600.0, 448.0];
const BACK: [f32; 4] = [260.0, 426.0, 120.0, 24.0];
const NEWER: [f32; 4] = [40.0, 426.0, 120.0, 24.0];
const OLDER: [f32; 4] = [480.0, 426.0, 120.0, 24.0];
const PAGE: usize = 24;
const LIST_Y: f32 = 64.0;
const LIST_H: f32 = 14.0;

/// What the list asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListAct {
    Open(PathBuf),
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListSpot {
    Row(usize),
    Back,
    Newer,
    Older,
}

/// The replays list over the main menu.
#[derive(Debug, Clone, Default)]
pub struct ReplayList {
    list: Vec<Entry>,
    top: usize,
    hover: Option<ListSpot>,
    /// The folder couldn't be read, or the last replay didn't open or play to its end.
    pub error: Option<String>,
}

impl ReplayList {
    /// The list of the replays folder now.
    pub fn open(error: Option<String>) -> Self {
        let (list, read) = match replays_dir().map(|d| list(&d)) {
            Some(Ok(l)) => (l, None),
            Some(Err(e)) => (Vec::new(), Some(e)),
            None => (Vec::new(), None),
        };
        Self { list, top: 0, hover: None, error: error.or(read) }
    }

    fn spot(&self, at: [f32; 2]) -> Option<ListSpot> {
        let inside = |r| crate::kit::inside_f(r, at);
        if inside(BACK) {
            return Some(ListSpot::Back);
        }
        if inside(NEWER) && self.top > 0 {
            return Some(ListSpot::Newer);
        }
        if inside(OLDER) && self.top + PAGE < self.list.len() {
            return Some(ListSpot::Older);
        }
        let n = self.list.len().saturating_sub(self.top).min(PAGE);
        (0..n).find(|&i| inside([36.0, LIST_Y + 16.0 + i as f32 * LIST_H, 568.0, LIST_H])).map(ListSpot::Row)
    }

    pub fn hover(&mut self, at: [f32; 2]) {
        self.hover = self.spot(at);
    }

    pub fn press(&mut self, at: [f32; 2]) -> Option<ListAct> {
        match self.spot(at)? {
            ListSpot::Row(i) => return Some(ListAct::Open(self.list[self.top + i].path.clone())),
            ListSpot::Back => return Some(ListAct::Close),
            ListSpot::Newer => self.top = self.top.saturating_sub(PAGE),
            ListSpot::Older => self.top += PAGE,
        }
        self.hover = self.spot(at);
        None
    }

    pub fn draw(&self, f: &mut Frame, d: &Draw) {
        let (white, dim, hot) = (d.col(1, 100.0), d.col(1, 60.0), d.col(0x25, 100.0));
        crate::kit::fader::fader(f, d.white, &d.v, PANEL, [0.0, 14.0, 22.0], 0.0, [0.0, 40.0, 60.0], 20.0);
        d.font.text(f, &d.v, b"Replays", 320.0, 26.0, 0, 4, Align::Centre, hot, None);
        if let Some(e) = &self.error {
            d.font.text(f, &d.v, e.as_bytes(), 320.0, 46.0, 0, 2, Align::Centre, dim, None);
        } else if self.list.is_empty() {
            d.font.text(f, &d.v, b"No recorded games yet.", 320.0, 46.0, 0, 2, Align::Centre, dim, None);
        }
        for (x, h) in [(40.0, "Started (UTC)"), (220.0, "Mode"), (340.0, "Size")] {
            d.font.text(f, &d.v, h.as_bytes(), x, LIST_Y, 0, 2, Align::Left, dim, None);
        }
        for (i, e) in self.list.iter().skip(self.top).take(PAGE).enumerate() {
            let y = LIST_Y + 16.0 + i as f32 * LIST_H;
            let c = if self.hover == Some(ListSpot::Row(i)) { hot } else { white };
            d.font.text(f, &d.v, e.when.as_bytes(), 40.0, y, 0, 2, Align::Left, c, None);
            d.font.text(f, &d.v, if e.multiplayer { b"Multiplayer" as &[u8] } else { b"Single player" }, 220.0, y, 0, 2, Align::Left, c, None);
            d.font.text(f, &d.v, format!("{} KB", e.kb).as_bytes(), 340.0, y, 0, 2, Align::Left, c, None);
        }
        let button = |f: &mut Frame, r: [f32; 4], label: &str, hover: bool| {
            crate::kit::fader::fader(f, d.white, &d.v, r, [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
            d.font.text(f, &d.v, label.as_bytes(), r[0] + r[2] / 2.0, r[1] + 8.0, 0, 2, Align::Centre, if hover { hot } else { white }, None);
        };
        button(f, BACK, "Main menu", self.hover == Some(ListSpot::Back));
        if self.top > 0 {
            button(f, NEWER, "Newer", self.hover == Some(ListSpot::Newer));
        }
        if self.top + PAGE < self.list.len() {
            button(f, OLDER, "Older", self.hover == Some(ListSpot::Older));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replay_file_name_reads_as_a_list_entry() {
        let e = Entry::of(PathBuf::from("r/2026-11-01_040826_mp.ig2r"), 2049).unwrap();
        assert_eq!((e.when.as_str(), e.multiplayer, e.kb), ("2026-11-01 04:08:26", true, 3));
        assert!(Entry::of(PathBuf::from("r/notes.ig2r"), 1).is_none());
    }

    #[test]
    fn the_keys_and_the_bar_ask_for_their_acts() {
        assert_eq!(key_act("3"), Some(Act::Speed(2)));
        assert_eq!(key_act(" "), Some(Act::Pause));
        assert_eq!(Bar::press([240.0, 35.0]), Some(Act::Speed(0)));
        assert_eq!(Bar::press([430.0, 35.0]), Some(Act::Exit));
    }
}
