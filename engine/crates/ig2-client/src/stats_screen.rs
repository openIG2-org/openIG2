//! Ours (ADR 0034, re/rules/game-end.md §11): the statistics screen a finished game shows after its end videos, and
//! the match history the main menu lists. A game's record ([`Match`]) is frozen when it ends for this machine from
//! the sim's statistics (`ig2_sim::stats`, the same on every multiplayer peer) and appended to [`HISTORY_FILE`]
//! beside the saves. Drawn in the screen kit's panels and the game's font over the main menu's scene; the texts are
//! English only (the game has no strings for them).

use std::path::{Path, PathBuf};

use ig2_sim::game::{Game, GameOver};
use ig2_sim::info::ROWS;
use ig2_sim::save::va;
use ig2_sim::stats::{Row, Sample};
use serde::{Deserialize, Serialize};

use crate::font::Align;
use crate::options::Draw;
use crate::render::{Blend, Frame, Instance};

/// The match history, beside the saves.
pub const HISTORY_FILE: &str = "match-history.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Campaign,
    Single,
    Multiplayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Won,
    Lost,
    /// `MEGNYERES(0)`: the campaign's end without a victory.
    Ended,
}

/// One empire of a finished game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Empire {
    /// 1..=8.
    pub empire: u8,
    /// `BirodalomNev[e]` in the game's encoding.
    pub name: Vec<u8>,
    pub race: u8,
    pub human: bool,
    pub built: i32,
    pub lost: i32,
}

/// A finished game's record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    /// When it ended (seconds since 1970, UTC).
    pub when: u64,
    /// The game's date at its end (year, month, day).
    pub date: [i32; 3],
    pub mode: Mode,
    /// For this machine's empire.
    pub outcome: Outcome,
    pub empire: u8,
    /// The game days played.
    pub days: u32,
    pub empires: Vec<Empire>,
    /// The sim's samples and a last one of the end.
    pub samples: Vec<Sample>,
}

impl Match {
    /// The record of `game` ending as `o` for `empire`.
    pub fn record(game: &Game, o: GameOver, empire: u8, multiplayer: bool, date: [i32; 3], when: u64) -> Self {
        let s = &game.state;
        let st = &game.stats;
        let mut samples = st.samples.clone();
        samples.push(ig2_sim::stats::sample(s));
        let (human, races) = (s.human_controlled(), s.races());
        let seen = |i: usize| samples.iter().any(|x| x.rows[i].planets > 0 || x.rows[i].ships > 0);
        let empires = (0..ROWS)
            .filter(|&i| seen(i) || i + 1 == empire as usize)
            .map(|i| {
                let e = i + 1;
                Empire { empire: e as u8, name: game.empire_names.get(e).cloned().unwrap_or_default(), race: races[e], human: human[e], built: st.built[i], lost: st.lost[i] }
            })
            .collect();
        let mode = if multiplayer {
            Mode::Multiplayer
        } else if s.mem.i32(va::CAMPAIGN_TYPE) > 0 {
            Mode::Campaign
        } else {
            Mode::Single
        };
        let outcome = match o {
            GameOver::Won => Outcome::Won,
            GameOver::Lost => Outcome::Lost,
            GameOver::Ended => Outcome::Ended,
        };
        Match { when, date, mode, outcome, empire, days: st.days, empires, samples }
    }

    fn last(&self) -> Option<&Sample> {
        self.samples.last()
    }

    fn title(&self) -> &'static str {
        match self.outcome {
            Outcome::Won => "Victory",
            Outcome::Lost => "Defeat",
            Outcome::Ended => "Game over",
        }
    }
}

fn mode_name(m: Mode) -> &'static str {
    match m {
        Mode::Campaign => "Campaign",
        Mode::Single => "Single player",
        Mode::Multiplayer => "Multiplayer",
    }
}

/// Seconds since 1970 → `YYYY-MM-DD` (UTC; the civil calendar from the day count).
pub fn day_of(when: u64) -> String {
    let z = (when / 86400) as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02}")
}

/// Where the history lives: `IG2_SAVE_DIR`, else the save slots' folder.
pub fn history_dir() -> Option<PathBuf> {
    std::env::var_os("IG2_SAVE_DIR").map(PathBuf::from).or_else(crate::options::slot_dir)
}

/// The history in `dir`, oldest first (none without the file; an unreadable file is reported and reads as none).
pub fn load(dir: &Path) -> Result<Vec<Match>, String> {
    let p = dir.join(HISTORY_FILE);
    match ig2_formats::userfs::read(&p) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

/// `m` added to the history in `dir`. A file that doesn't parse is left alone (nothing is lost): the error.
pub fn append(dir: &Path, m: &Match) -> Result<(), String> {
    let mut all = load(dir)?;
    all.push(m.clone());
    let json = serde_json::to_vec(&all).map_err(|e| e.to_string())?;
    let p = dir.join(HISTORY_FILE);
    let tmp = dir.join(format!("{HISTORY_FILE}.tmp"));
    ig2_formats::userfs::create_dir_all(dir).and_then(|_| ig2_formats::userfs::write(&tmp, json)).and_then(|_| ig2_formats::userfs::rename(&tmp, &p)).map_err(|e| format!("{}: {e}", p.display()))
}

/// A graph's name and value.
type Metric = (&'static str, fn(&Row) -> f64);

/// The graphs' values.
const METRICS: [Metric; 6] = [
    ("Planets", |r| r.planets as f64),
    ("Population", |r| r.population as f64),
    ("Credits", |r| r.credits as f64),
    ("Ships", |r| r.ships as f64),
    ("Techs", |r| r.techs as f64),
    ("Score", |r| r.score as f64),
];

/// The match table's columns: head and right edge.
const COLS: [(&str, f32); 9] = [("Planets", 214.0), ("Pop.", 262.0), ("Credits", 330.0), ("Ships", 378.0), ("Built", 420.0), ("Lost", 462.0), ("Techs", 504.0), ("Wins", 546.0), ("Score", 600.0)];
/// The match list's heads and their x.
const LIST_HEADS: [(f32, &str); 5] = [(40.0, "Date"), (120.0, "Mode"), (220.0, "Result"), (290.0, "Days"), (336.0, "Empires")];

/// The empires' line colours (ours), by empire − 1.
const COLOURS: [[f32; 3]; ROWS] = [
    [0.95, 0.30, 0.25],
    [0.30, 0.55, 1.00],
    [0.35, 0.90, 0.35],
    [1.00, 0.85, 0.25],
    [0.85, 0.40, 0.95],
    [0.25, 0.90, 0.90],
    [1.00, 0.60, 0.20],
    [0.85, 0.85, 0.85],
];

/// The panel and its parts (virtual pixels).
const PANEL: [f32; 4] = [20.0, 16.0, 600.0, 448.0];
const TABLE_Y: f32 = 78.0;
const ROW_H: f32 = 13.0;
const TABS_Y: f32 = 206.0;
const TAB_W: f32 = 92.0;
const GRAPH: [f32; 4] = [64.0, 236.0, 530.0, 178.0];
const BACK: [f32; 4] = [260.0, 426.0, 120.0, 24.0];
const NEWER: [f32; 4] = [40.0, 426.0, 120.0, 24.0];
const OLDER: [f32; 4] = [480.0, 426.0, 120.0, 24.0];
/// The history's rows per page.
const PAGE: usize = 24;
const LIST_Y: f32 = 64.0;
const LIST_H: f32 = 14.0;

/// The main menu's "History" item (ours), bottom left: the Settings label's box (`mainscr::SETTINGS_BOX`) mirrored.
pub const HISTORY_ITEM: [f32; 4] = [8.0, 448.0, 96.0, 22.0];

/// The "History" item as the Settings label is drawn: the button font, colour 2 under the pointer, faded in with the
/// menu's texts (`fade` = `MainScr::gt`).
pub fn draw_history_item(f: &mut Frame, d: &Draw, hot: bool, fade: f32) {
    let [x, y, w, _] = HISTORY_ITEM;
    d.font.text(f, &d.v, &d.texts.ours("History"), x + w / 2.0, y + 4.0, 0, 3, Align::Centre, d.col(if hot { 2 } else { 1 }, 100.0 * fade), None);
}

/// What a press or key asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// The screen closes (to the main menu).
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    Metric(usize),
    Row(usize),
    Back,
    Newer,
    Older,
}

fn inside(r: [f32; 4], at: [f32; 2]) -> bool {
    crate::kit::inside_f(r, at)
}

/// The statistics screen: one game's (after its end, or opened from the history), or the history's list.
#[derive(Debug, Clone)]
pub struct StatsScreen {
    /// Newest first.
    list: Vec<Match>,
    /// The match on screen; `None`: the list.
    shown: Option<usize>,
    /// Opened from the menu: Back from a match returns to the list.
    from_list: bool,
    metric: usize,
    /// The list's first row.
    top: usize,
    hover: Option<Spot>,
    /// The history couldn't be read.
    pub error: Option<String>,
}

impl StatsScreen {
    /// After a game's end.
    pub fn end(m: Match) -> Self {
        Self { list: vec![m], shown: Some(0), from_list: false, metric: 0, top: 0, hover: None, error: None }
    }

    /// The main menu's history (`list` oldest first, as the file holds it).
    pub fn history(mut list: Vec<Match>, error: Option<String>) -> Self {
        list.reverse();
        Self { list, shown: None, from_list: true, metric: 0, top: 0, hover: None, error }
    }

    pub fn shown(&self) -> Option<&Match> {
        self.list.get(self.shown?)
    }

    fn spot(&self, at: [f32; 2]) -> Option<Spot> {
        if inside(BACK, at) {
            return Some(Spot::Back);
        }
        match self.shown() {
            Some(_) => (0..METRICS.len()).find(|&k| inside([40.0 + k as f32 * TAB_W, TABS_Y, TAB_W - 4.0, 18.0], at)).map(Spot::Metric),
            None => {
                if inside(NEWER, at) && self.top > 0 {
                    return Some(Spot::Newer);
                }
                if inside(OLDER, at) && self.top + PAGE < self.list.len() {
                    return Some(Spot::Older);
                }
                let n = self.list.len().saturating_sub(self.top).min(PAGE);
                (0..n).find(|&i| inside([36.0, LIST_Y + 16.0 + i as f32 * LIST_H, 568.0, LIST_H], at)).map(Spot::Row)
            }
        }
    }

    pub fn hover(&mut self, at: [f32; 2]) {
        self.hover = self.spot(at);
    }

    /// A spot is under `at` (ADR 0059: elsewhere the right button is Esc).
    pub fn on_spot(&self, at: [f32; 2]) -> bool {
        self.spot(at).is_some()
    }

    /// A press at `at` (virtual pixels).
    pub fn press(&mut self, at: [f32; 2]) -> Option<Act> {
        match self.spot(at)? {
            Spot::Metric(k) => self.metric = k,
            Spot::Row(i) => {
                self.shown = Some(self.top + i);
                self.metric = 0;
            }
            Spot::Back => return self.back(),
            Spot::Newer => self.top = self.top.saturating_sub(PAGE),
            Spot::Older => self.top += PAGE,
        }
        self.hover = self.spot(at);
        None
    }

    /// Esc or Enter: back to the list, or the screen closes.
    pub fn back(&mut self) -> Option<Act> {
        if self.shown.is_some() && self.from_list {
            self.shown = None;
            return None;
        }
        Some(Act::Close)
    }

    /// Left/Right pick the graph's value.
    pub fn step_metric(&mut self, forward: bool) {
        let n = METRICS.len();
        self.metric = if forward { (self.metric + 1) % n } else { (self.metric + n - 1) % n };
    }

    pub fn draw(&self, f: &mut Frame, d: &Draw) {
        // Opaque: the menu's bright ring labels would show through.
        crate::theme::look(d).panel(f, d.white, &d.v, PANEL);
        match self.shown() {
            Some(m) => self.draw_match(f, d, m),
            None => self.draw_list(f, d),
        }
        let label = if self.shown.is_some() && self.from_list { "Back" } else if self.from_list { "Main menu" } else { "Continue" };
        self.button(f, d, BACK, label, self.hover == Some(Spot::Back), true);
    }

    fn button(&self, f: &mut Frame, d: &Draw, r: [f32; 4], label: &str, hot: bool, live: bool) {
        let t = crate::theme::look(d);
        t.button(f, d.white, &d.v, r);
        let c = if !live { t.off } else if hot { t.hot } else { t.text };
        d.font.text(f, &d.v, &d.texts.ours(label), r[0] + r[2] / 2.0, r[1] + 8.0, 0, t.small, Align::Centre, c, None);
    }

    #[allow(clippy::too_many_arguments)]
    /// `Align::Right`: the text ends at `x` (`KiirStr`'s own right alignment needs a field width).
    fn text(f: &mut Frame, d: &Draw, s: &[u8], x: f32, y: f32, size: usize, align: Align, c: [f32; 3]) {
        match align {
            Align::Right => d.font.text(f, &d.v, s, x - s.len() as f32 * d.font.advance(size), y, 0, size, Align::Left, c, None),
            _ => d.font.text(f, &d.v, s, x, y, 0, size, align, c, None),
        }
    }

    fn draw_match(&self, f: &mut Frame, d: &Draw, m: &Match) {
        let t = crate::theme::look(d);
        let (white, dim, hot) = (t.text, t.dim, t.hot);
        Self::text(f, d, &d.texts.ours(m.title()), 320.0, 26.0, t.big, Align::Centre, hot);
        let [y, mo, dd] = m.date;
        let to = format!("{y}-{mo:02}-{dd:02}");
        let sub = crate::tr::fill(d.texts.lang, "{}  -  {} game days (to {})  -  ended {}", &[&d.texts.ours(mode_name(m.mode)), m.days.to_string().as_bytes(), to.as_bytes(), day_of(m.when).as_bytes()]);
        Self::text(f, d, &sub, 320.0, 46.0, t.small, Align::Centre, dim);
        let cols = COLS;
        Self::text(f, d, &d.texts.ours("Empire"), 40.0, TABLE_Y - 14.0, t.small, Align::Left, dim);
        for (name, x) in cols {
            Self::text(f, d, &d.texts.ours(name), x, TABLE_Y - 14.0, t.small, Align::Right, dim);
        }
        let Some(last) = m.last() else { return };
        for (k, e) in m.empires.iter().enumerate() {
            let i = e.empire as usize - 1;
            let y = TABLE_Y + k as f32 * ROW_H;
            let r = &last.rows[i];
            f.push(d.white, Blend::Alpha, Instance::line(d.v.px(30.0, y + 3.5), d.v.px(36.0, y + 3.5), 5.0 * d.v.k, rgba(COLOURS[i])));
            let mut name = e.name.clone();
            if e.empire == m.empire {
                name.extend(d.texts.ours(" (you)"));
            }
            Self::text(f, d, &name, 40.0, y, t.small, Align::Left, if e.empire == m.empire { hot } else { white });
            let vals = [r.planets as i64, r.population as i64, r.credits, r.ships as i64, e.built as i64, e.lost as i64, r.techs as i64, r.battles as i64, r.score as i64];
            for ((_, x), v) in cols.iter().zip(vals) {
                Self::text(f, d, v.to_string().as_bytes(), *x, y, t.small, Align::Right, white);
            }
        }
        for (k, (name, _)) in METRICS.iter().enumerate() {
            let r = [40.0 + k as f32 * TAB_W, TABS_Y, TAB_W - 4.0, 18.0];
            if k == self.metric {
                t.button(f, d.white, &d.v, r);
            }
            let c = if k == self.metric || self.hover == Some(Spot::Metric(k)) { hot } else { dim };
            Self::text(f, d, &d.texts.ours(name), r[0] + r[2] / 2.0, r[1] + 5.0, t.small, Align::Centre, c);
        }
        self.graph(f, d, m);
    }

    fn graph(&self, f: &mut Frame, d: &Draw, m: &Match) {
        let [gx, gy, gw, gh] = GRAPH;
        let value = METRICS[self.metric].1;
        let max = m.samples.iter().flat_map(|s| m.empires.iter().map(move |e| value(&s.rows[e.empire as usize - 1]))).fold(0.0f64, f64::max);
        let t = crate::theme::look(d);
        let dim = if crate::theme::current().is_some() { t.dim } else { d.col(1, 50.0) };
        let line = |f: &mut Frame, a: [f32; 2], b: [f32; 2], c: [f32; 4], w: f32| f.push(d.white, Blend::Alpha, Instance::line(d.v.px(a[0], a[1]), d.v.px(b[0], b[1]), w * d.v.k.max(1.0), c));
        let axis = [dim[0], dim[1], dim[2], 1.0];
        line(f, [gx, gy + gh], [gx + gw, gy + gh], axis, 1.0);
        line(f, [gx, gy], [gx, gy + gh], axis, 1.0);
        Self::text(f, d, fmt_value(max).as_bytes(), gx - 4.0, gy, t.small, Align::Right, dim);
        Self::text(f, d, b"0", gx - 4.0, gy + gh - 6.0, t.small, Align::Right, dim);
        Self::text(f, d, &crate::tr::fill(d.texts.lang, "day {}", &[b"0"]), gx, gy + gh + 4.0, t.small, Align::Left, dim);
        Self::text(f, d, &crate::tr::fill(d.texts.lang, "day {}", &[m.days.to_string().as_bytes()]), gx + gw, gy + gh + 4.0, t.small, Align::Right, dim);
        let n = m.samples.len();
        let first = m.samples.first().map_or(0, |s| s.ido);
        let span = m.samples.last().map_or(0, |s| s.ido) - first;
        let px = |k: usize, v: f64| {
            let t = if span > 0 { (m.samples[k].ido - first) as f32 / span as f32 } else if n > 1 { k as f32 / (n - 1) as f32 } else { 1.0 };
            let h = if max > 0.0 { (v / max) as f32 } else { 0.0 };
            [gx + t * gw, gy + gh - h * gh]
        };
        for e in &m.empires {
            let i = e.empire as usize - 1;
            let c = rgba(COLOURS[i]);
            let w = if e.empire == m.empire { 2.0 } else { 1.0 };
            for k in 1..n {
                line(f, px(k - 1, value(&m.samples[k - 1].rows[i])), px(k, value(&m.samples[k].rows[i])), c, w);
            }
        }
    }

    fn draw_list(&self, f: &mut Frame, d: &Draw) {
        let t = crate::theme::look(d);
        let (white, dim, hot) = (t.text, t.dim, t.hot);
        Self::text(f, d, &d.texts.ours("Match history"), 320.0, 26.0, t.big, Align::Centre, hot);
        if let Some(e) = &self.error {
            Self::text(f, d, e.as_bytes(), 320.0, 46.0, t.small, Align::Centre, dim);
        } else if self.list.is_empty() {
            Self::text(f, d, &d.texts.ours("No finished games yet."), 320.0, 46.0, t.small, Align::Centre, dim);
        }
        for (x, h) in LIST_HEADS {
            Self::text(f, d, &d.texts.ours(h), x, LIST_Y, t.small, Align::Left, dim);
        }
        for (i, m) in self.list.iter().skip(self.top).take(PAGE).enumerate() {
            let y = LIST_Y + 16.0 + i as f32 * LIST_H;
            let c = if self.hover == Some(Spot::Row(i)) { hot } else { white };
            let empires: Vec<u8> = m.empires.iter().map(|e| e.name.as_slice()).collect::<Vec<_>>().join(&b", "[..]);
            Self::text(f, d, day_of(m.when).as_bytes(), 40.0, y, t.small, Align::Left, c);
            Self::text(f, d, &d.texts.ours(mode_name(m.mode)), 120.0, y, t.small, Align::Left, c);
            Self::text(f, d, &d.texts.ours(m.title()), 220.0, y, t.small, Align::Left, c);
            Self::text(f, d, m.days.to_string().as_bytes(), 290.0, y, t.small, Align::Left, c);
            d.font.text(f, &d.v, &empires, 336.0, y, 44, t.small, Align::Left, c, None);
        }
        if self.top > 0 {
            self.button(f, d, NEWER, "Newer", self.hover == Some(Spot::Newer), true);
        }
        if self.top + PAGE < self.list.len() {
            self.button(f, d, OLDER, "Older", self.hover == Some(Spot::Older), true);
        }
    }
}

fn rgba(c: [f32; 3]) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}

/// 12345678 → "12.3M", 4500 → "4500".
fn fmt_value(v: f64) -> String {
    match v {
        v if v >= 1e7 => format!("{:.1}M", v / 1e6),
        v if v >= 1e5 => format!("{:.0}k", v / 1e3),
        v => format!("{v:.0}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR 0042: every label of the screen has its Hungarian.
    #[test]
    fn labels_are_translated() {
        let outcomes = [Outcome::Won, Outcome::Lost, Outcome::Ended].map(|o| Match { outcome: o, ..game_match(1) }.title());
        let modes = [Mode::Campaign, Mode::Single, Mode::Multiplayer].map(mode_name);
        let all = METRICS.iter().map(|m| m.0).chain(COLS.iter().map(|c| c.0)).chain(LIST_HEADS.iter().map(|h| h.1)).chain(outcomes).chain(modes);
        let missing: Vec<&str> = all.chain(["Back", "Main menu", "Continue", "Newer", "Older"]).filter(|s| !crate::tr::has(s)).collect();
        assert!(missing.is_empty(), "{missing:?}");
    }

    fn game_match(days: u32) -> Match {
        let row = Row { planets: 3, ships: 2, ..Row::default() };
        let mut rows = [Row::default(); ROWS];
        rows[0] = row;
        Match {
            when: 1_793_491_200,
            date: [3011, 4, 2],
            mode: Mode::Single,
            outcome: Outcome::Won,
            empire: 1,
            days,
            empires: vec![Empire { empire: 1, name: b"Solarian".to_vec(), race: 1, human: true, built: 4, lost: 2 }],
            samples: vec![Sample { ido: 0, rows }, Sample { ido: 100, rows }],
        }
    }

    #[test]
    fn dates() {
        assert_eq!(day_of(0), "1970-01-01");
        assert_eq!(day_of(951_782_400), "2000-02-29");
        assert_eq!(day_of(1_793_491_200), "2026-11-01");
    }

    /// Appending keeps the older records; a file that doesn't parse is left as it was.
    #[test]
    fn history_round_trip() {
        let dir = std::env::temp_dir().join(format!("ig2-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load(&dir), Ok(Vec::new()));
        append(&dir, &game_match(10)).unwrap();
        append(&dir, &game_match(20)).unwrap();
        let all = load(&dir).unwrap();
        assert_eq!(all.iter().map(|m| m.days).collect::<Vec<_>>(), [10, 20]);
        std::fs::write(dir.join(HISTORY_FILE), b"not json").unwrap();
        assert!(append(&dir, &game_match(30)).is_err());
        assert_eq!(std::fs::read(dir.join(HISTORY_FILE)).unwrap(), b"not json");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The list opens a row's match; Back returns to the list, then closes; an end screen closes at once.
    #[test]
    fn list_and_back() {
        let mut s = StatsScreen::history(vec![game_match(10), game_match(20)], None);
        assert!(s.shown().is_none());
        assert_eq!(s.press([100.0, LIST_Y + 16.0 + 2.0]), None);
        assert_eq!(s.shown().map(|m| m.days), Some(20), "newest first");
        s.press([40.0 + 2.0 * TAB_W + 5.0, TABS_Y + 5.0]);
        assert_eq!(s.metric, 2);
        assert_eq!(s.back(), None);
        assert!(s.shown().is_none());
        assert_eq!(s.press([300.0, 430.0]), Some(Act::Close));
        let mut e = StatsScreen::end(game_match(5));
        assert_eq!(e.back(), Some(Act::Close));
    }
}
