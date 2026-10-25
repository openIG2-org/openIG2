//! The main menu's scenario / skirmish list (`Mode_Scenario_Valasztas`, re/screens/mainscr.md §9.3): opened by the
//! single-player menu's scenario ring (`ScenarioGombraRanyomtak` exe 0x503bf0), a 2D panel over the scene: the list
//! (`LoadScenarioList` exe 0x553400: a "Skirmish" entry, then the 14 `scenario\*.dsc` the exe names), the picture, the
//! description or the high scores, and for the skirmish entry its seven settings (`MainScr_Main` exe 0x4fefc0, boxes
//! `MainScr_MouseInit` exe 0x4ff980; faders `MainScrGfx2D_1` exe 0x4fab30, texts `MainScrGfx2D_2` exe 0x4fc8d0;
//! timers `FRISSIT_MainScr2` exe 0x4f65f0).

use ig2_formats::misc_text::{self, DefaultScore, HIGHSCORE_SLOTS};
use ig2_formats::Install;

use crate::font::Align;
use crate::kit::fader;
use crate::options::Draw;
use crate::render::{kind, Blend, Frame, Instance, TexId};
use crate::kit::DesignFrame;
use crate::ui::UiTexts;

/// The exe's table of description files (`LoadScenarioList`: 14 `char*` at exe 0x919774).
const DSC_TABLE: u32 = 0x919774;
const DSC_COUNT: usize = 14;
/// Rows the list shows (`ScenarioStart` .. +22).
pub const ROWS: usize = 23;

/// One list entry (`ScenarioDescription`, 0x9c bytes; exe +0 name, +4 file, +0xc lines, +0x34 difficulty, +0x38 race,
/// +0x3c registry name, +0x40 scores, +0x68 names, +0x90 skirmish kind).
#[derive(Debug, Clone, Default)]
pub struct Entry {
    /// `title` (the name in the list).
    pub name: Vec<u8>,
    /// The scenario file (`scenario/<name>.scn`); empty for the skirmish entry.
    pub file: String,
    /// The `desc` lines (at most 10).
    pub lines: Vec<Vec<u8>>,
    /// `difficulty` (an index into the difficulty names), `race` (1..=8).
    pub difficulty: i32,
    pub race: i32,
    /// The registry name (the file's base name): the high-score table's key.
    pub registry: String,
    /// The picture (`pic`, `..\scenario\` prefixed; read as `dataraw/scenario/…`).
    pub pic: String,
    /// The high-score table: name and score (× 50 as stored).
    pub scores: Vec<(Vec<u8>, i32)>,
    /// > 0: a skirmish.
    pub kind: i32,
}

impl Entry {
    pub fn skirmish(&self) -> bool {
        self.kind > 0
    }
}

/// `LoadScenarioList`: each description the exe names (`LoadScenarioDescription` exe 0x552fc0: a second entry with
/// the same registry name is dropped; an entry goes before the first of a higher `difficulty`, or of the same one with a
/// greater title, byte by byte), then the skirmish entry (`GetString(0x13, 5, 1)`: a language without the line takes
/// the English one [obs: "Skirmish" in the Hungarian exe], `pic\spwar\c30068.bmp`, kind 1) put first. The high
/// scores come from `fabscr.txt` (`LoadHighscores`' defaults; the registry tables aren't kept yet).
pub fn load_entries(install: &Install, exe: &ig2_rules::PeImage, texts: &UiTexts) -> Vec<Entry> {
    let defaults = install.read("dataraw/text/fabscr.txt").map(|b| misc_text::parse_fabscr(&ig2_formats::text::decode(&b, install.lang()))).unwrap_or_default();
    let mut skirmish = texts.get(0x13, 5, 1);
    if skirmish.is_empty() {
        if let Ok(en) = Install::open(install.root(), ig2_formats::text::Lang::En) {
            skirmish = UiTexts::load(&en).get(0x13, 5, 1);
        }
    }
    let mut out: Vec<Entry> = Vec::new();
    for k in 0..DSC_COUNT as u32 {
        let cstr = |p: u32| exe.read_loaded(p, 32).ok().map(|b| String::from_utf8_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]).into_owned());
        let Some(name) = exe.read_loaded(DSC_TABLE + 4 * k, 4).ok().map(|b| u32::from_le_bytes(b.try_into().unwrap())).and_then(cstr) else { continue };
        let base = name.rsplit_once('.').map_or(name.as_str(), |(b, _)| b).to_string();
        if out.iter().any(|e| e.registry.eq_ignore_ascii_case(&base)) {
            continue;
        }
        let Ok(b) = install.read(&format!("dataraw/scenario/{name}")) else { continue };
        let d = misc_text::parse_dsc(&ig2_formats::text::decode(&b, install.lang()));
        let enc = |s: &str| texts.encode(s);
        let scores = high_scores(&defaults, &base, texts);
        let name = enc(&d.title);
        let at = out.iter().position(|e| e.difficulty > d.difficulty || (e.difficulty == d.difficulty && e.name > name)).unwrap_or(out.len());
        out.insert(at, Entry {
            name,
            file: format!("scenario/{base}.scn"),
            lines: d.desc.iter().take(10).map(|l| enc(l)).collect(),
            difficulty: d.difficulty,
            race: d.race,
            registry: base,
            pic: d.pic,
            scores,
            kind: 0,
        });
    }
    out.insert(0, Entry { name: skirmish, pic: "pic\\spwar\\c30068.bmp".into(), kind: 1, race: 1, ..Default::default() });
    out
}

/// `LoadHighscores`' default table of `scenario`: the `fabscr.txt` lines naming it, in file order, at most 10.
fn high_scores(defaults: &[DefaultScore], scenario: &str, texts: &UiTexts) -> Vec<(Vec<u8>, i32)> {
    defaults.iter().filter(|d| d.scenario.eq_ignore_ascii_case(scenario)).take(HIGHSCORE_SLOTS).map(|d| (texts.encode(&d.name), d.score)).collect()
}

/// The skirmish settings (`SelectedScirmish*`, exe 0xbbb358.., spec §1) and their ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Skirmish {
    /// 1..=8.
    pub race: i32,
    /// Computer players 1..=7.
    pub computers: i32,
    /// 1..=4.
    pub difficulty: i32,
    /// Index 0..=4 into `ScenarioKezdoBolygoSzamok`.
    pub planets: i32,
    /// 0..=3.
    pub technology: i32,
    /// Index 0..=5 into `ScenarioKezdoPenzek`.
    pub money: i32,
    /// 0..=3.
    pub population: i32,
}

/// The settings' boxes (`MainScr_MouseInit`: ids 0x18/0x19 race, 0x1a/0x1b computers, 0x1d/0x1e difficulty, 0x1f/0x20
/// planets, 0x21/0x22 technology, 0x23/0x24 money, 0x25/0x26 population): `<-` / `->` x, y and the range.
const SETTINGS: [(i32, i32, i32, i32); 7] = [(39, 408, 1, 8), (39, 446, 1, 7), (114, 446, 1, 4), (189, 446, 0, 4), (264, 446, 0, 3), (339, 446, 0, 5), (114, 390, 0, 3)];

impl Skirmish {
    /// `InitScirmishValtozok` (exe 0x4f8f80): the entry's race, 3 computers, every other setting 2 (the full set when
    /// exe byte 0xbad106 is set, a debug switch the remake leaves off).
    pub fn new(race: i32) -> Self {
        Self { race, computers: 3, difficulty: 2, planets: 2, technology: 2, money: 2, population: 2 }
    }

    fn get(&mut self, k: usize) -> &mut i32 {
        match k {
            0 => &mut self.race,
            1 => &mut self.computers,
            2 => &mut self.difficulty,
            3 => &mut self.planets,
            4 => &mut self.technology,
            5 => &mut self.money,
            _ => &mut self.population,
        }
    }

    fn value(&self, k: usize) -> i32 {
        let mut c = *self;
        *c.get(k)
    }
}

/// The list's state (`Mode_Scenario_Valasztas` and its variables).
#[derive(Debug, Clone)]
pub struct ScenarioList {
    pub entries: Vec<Entry>,
    /// `ScenarioValasztasNyitottsag` (0xbbb2ac): the panel's fade 0..100, +-1 a tick.
    pub open: f32,
    /// `Mode_Scenario_Valasztas_Becsukas` (0xbbb229).
    pub closing: bool,
    /// `ScenarioKepNyitottsag` (0xbbb048) and its target (0xbbb04c): the picture's fade, +-2 a tick.
    pub pic: f32,
    pub pic_cel: f32,
    /// `ScenarioStart` (0xbbb188), `SelectedScenario` (0xbbb350), `SelectedScenarioOld` (0xbbb354: the entry the
    /// picture and the lower panel show).
    pub first: usize,
    pub selected: usize,
    pub shown: usize,
    /// `ScenarioHighScoreMode` (0xbbb22e).
    pub high_score: bool,
    pub sk: Skirmish,
    /// `Mode_Scenario_Vagy_Scirmish_Start` (0xbbb22d): start was pressed.
    pub starting: bool,
    /// The scroll knob is held (`Mouse_Megfogva` on box 0x1c).
    drag: bool,
    /// The buttons held (bit 0 left, bit 1 right: `Mouse_Gomb` 0xdb4650 = either).
    held: u8,
    /// Frames box 0x1c still acts after the drag ended: the release frame keeps `AktIkon` (row 360).
    latch: u8,
    /// The pointer (virtual pixels) and the hot-spot id under it (`AktIkon2`).
    pub at: [f32; 2],
    /// `DesignKeret1` round the selected row.
    pub frame: DesignFrame,
}

/// What a press asks of the main menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// `PlaySoundGlobal(0)` (a skirmish arrow).
    Click,
    /// Start (box 0x15): the menu leaves for the starmap and starts the entry.
    Start,
}

impl ScenarioList {
    /// `ScenarioGombraRanyomtak("")`: the list from its top, the first entry selected, its picture fading in.
    pub fn open(entries: Vec<Entry>) -> Self {
        let race = entries.first().map_or(1, |e| e.race);
        let mut s = Self {
            entries,
            open: 0.0,
            closing: false,
            pic: 0.0,
            pic_cel: 100.0,
            first: 0,
            selected: 0,
            shown: 0,
            high_score: false,
            sk: Skirmish::new(race),
            starting: false,
            drag: false,
            held: 0,
            latch: 0,
            at: [0.0; 2],
            frame: DesignFrame::default(),
        };
        s.init_skirmish();
        s
    }

    fn init_skirmish(&mut self) {
        if let Some(e) = self.entries.get(self.selected).filter(|e| e.skirmish()) {
            self.sk = Skirmish::new(e.race);
        }
    }

    /// The entry the picture and the lower panel show.
    pub fn entry(&self) -> Option<&Entry> {
        self.entries.get(self.shown)
    }

    /// The selected entry (what start starts).
    pub fn chosen(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    /// The panel is fully shut (`Nyitottsag` back at 0 while closing): the menu returns to the single-player rings.
    pub fn closed(&self) -> bool {
        self.closing && self.open <= 0.0
    }

    /// One 10 ms tick (`FRISSIT_MainScr2`: the panel's fade by 1, the picture's by 2; the picture swaps to the new
    /// selection when its fade-out ends). `leaving` = `Mode_Kilepes` (no fade-in while the exit animation runs).
    pub fn tick(&mut self, leaving: bool) {
        if !self.closing {
            if self.open < 100.0 {
                self.open += 1.0;
                if self.open >= 100.0 {
                    self.open = 100.0;
                }
            }
        } else if self.open > 0.0 {
            self.open -= 1.0;
            if self.open <= 0.0 {
                self.open = 0.0;
            }
        }
        if self.pic_cel <= self.pic {
            if self.pic_cel < self.pic {
                self.pic -= 2.0;
                if self.pic <= self.pic_cel {
                    self.pic = 0.0;
                    self.pic_cel = 100.0;
                    self.shown = self.selected;
                }
            }
        } else if !leaving && !self.closing {
            self.pic += 2.0;
            if self.pic_cel <= self.pic {
                self.pic = 100.0;
            }
        }
        self.frame.tick();
    }

    /// The boxes switched on this frame (`MainScr_MouseInit2` exe 0x4ff7b0): the list's three while the panel is
    /// fully open, the settings while a skirmish entry is shown at full picture, else the info toggle; the scroll bar
    /// at full picture.
    fn boxes(&self) -> Vec<(u32, [f32; 4])> {
        let mut v = Vec::new();
        let list = !self.entries.is_empty();
        if self.open > 99.0 {
            v.extend([(0x15, [430.0, 438.0, 87.0, 34.0]), (0x16, [523.0, 438.0, 87.0, 34.0]), (0x17, [435.0, 65.0, 168.0, 355.0])]);
        }
        let full = list && self.pic == 100.0;
        if full && self.entry().is_some_and(Entry::skirmish) {
            for (k, &(x, y, _, _)) in SETTINGS.iter().enumerate() {
                let id = [0x18, 0x1a, 0x1d, 0x1f, 0x21, 0x23, 0x25][k];
                v.push((id, [x as f32, y as f32, 33.0, 15.0]));
                v.push((id + 1, [x as f32 + 37.0, y as f32, 33.0, 15.0]));
            }
        } else {
            v.push((0x27, [300.0, 446.0, 110.0, 19.0]));
        }
        if full {
            v.push((0x1c, [615.0, 64.0, 14.0, 364.0]));
        }
        v
    }

    /// The hot-spot under `at` (0 none; the boxes in id order, the first that holds the point).
    pub fn spot_at(&self, at: [f32; 2]) -> u32 {
        let (x, y) = (at[0].floor(), at[1].floor());
        let mut b = self.boxes();
        b.sort_by_key(|&(id, _)| id);
        b.into_iter().find(|&(_, r)| r[0] <= x && x < r[0] + r[2] && r[1] <= y && y < r[1] + r[3]).map_or(0, |(id, _)| id)
    }

    /// The hot-spot that acts (`AktIkon`): box 0x1c while the knob is held, `IG2_gameUpdate` (exe 0x4e0440) skips the
    /// hot-spot test while `Mouse_Megfogva` is set; else the one under the pointer.
    pub fn hot(&self) -> u32 {
        if self.drag || self.latch > 0 { 0x1c } else { self.spot_at(self.at) }
    }

    /// A press of either button (`MainScr_Main`'s `AktIkon2` cases with `Mouse_Lenyom`); while the knob is held the
    /// drag keeps the press (no hot-spot test).
    pub fn press(&mut self, at: [f32; 2], left: bool) -> Option<Act> {
        self.at = at;
        self.held |= if left { 1 } else { 2 };
        if self.drag {
            self.scroll_to(at[1]);
            return None;
        }
        if self.closing || self.starting {
            return None;
        }
        let id = self.spot_at(at);
        let n = self.entries.len();
        match id {
            0x15 if n > 0 => {
                self.closing = true;
                self.starting = true;
                self.pic_cel = 0.0;
                Some(Act::Start)
            }
            0x16 => {
                self.cancel();
                None
            }
            0x17 => {
                let r = (((at[1].floor() as i32) - 0x41) / 0xf).clamp(0, 0x16) as usize + self.first;
                if r < n && r != self.selected {
                    self.selected = r;
                    if self.entries[r].skirmish() {
                        self.high_score = false;
                    }
                    self.init_skirmish();
                    if self.pic != 0.0 {
                        self.pic_cel = 0.0;
                    }
                }
                None
            }
            0x18..=0x26 if id != 0x1c => {
                let k = match id {
                    0x18 | 0x19 => 0,
                    0x1a | 0x1b => 1,
                    0x1d | 0x1e => 2,
                    0x1f | 0x20 => 3,
                    0x21 | 0x22 => 4,
                    0x23 | 0x24 => 5,
                    _ => 6,
                };
                let up = matches!(id, 0x19 | 0x1b | 0x1e | 0x20 | 0x22 | 0x24 | 0x26);
                let (lo, hi) = (SETTINGS[k].2, SETTINGS[k].3);
                let v = self.sk.get(k);
                *v = if up { (*v + 1).min(hi) } else { (*v - 1).max(lo) };
                Some(Act::Click)
            }
            0x1c => {
                self.drag = true;
                self.scroll_to(at[1]);
                None
            }
            0x27 => {
                self.high_score = !self.high_score;
                None
            }
            _ => None,
        }
    }

    /// The pointer moved (with the knob held: `Mouse_Megfogva` keeps box 0x1c's case running).
    pub fn hover(&mut self, at: [f32; 2]) {
        self.at = at;
        self.latch = self.latch.saturating_sub(1);
        if self.drag {
            self.scroll_to(at[1]);
        }
    }

    /// A button's release: `Mouse_Megfogva` clears once no button is held (`Mouse_Gomb == 0`, exe 0x4e0440), so with
    /// both held the drag outlives the first release.
    pub fn release(&mut self, left: bool) {
        self.held &= if left { !1 } else { !2 };
        if self.held == 0 {
            self.end_drag();
        }
    }

    /// The drag's end: `IG2_gameUpdate` tests the hot-spots (line 51, skipped while `Mouse_Megfogva` is set) before it
    /// clears the flag (lines 62-64), so the release frame still has `AktIkon` 0x1c (its hover and pointer); the
    /// window's per-frame [`Self::hover`] counts that frame off. The release moves nothing (input.md §7).
    fn end_drag(&mut self) {
        if self.drag {
            self.latch = 2;
        }
        self.drag = false;
    }

    /// A middle press or release: `MainCallBackEventHandler` (exe 0x4e2db0) sends any button but 1 and 3 to the block
    /// at 0x4e345a, which zeroes the held bytes 0xdb467d..f (and Shift/Ctrl/Alt): the drag ends.
    pub fn middle(&mut self) {
        self.held = 0;
        self.end_drag();
    }

    /// Box 0x1c: `ScenarioStart = (int)(((float)Mouse_y − 64)/364·count − 11.5)`, clamped to the list.
    fn scroll_to(&mut self, y: f32) {
        let n = self.entries.len() as i32;
        let mut s = ((y.floor() - 64.0) / 364.0 * n as f32 - 11.5) as i32;
        if s < 0 {
            s = 0;
        }
        if n < s + ROWS as i32 {
            s = n - ROWS as i32;
        }
        self.first = s.max(0) as usize;
        // `DesignKeretAzonnalMorph = 1`: the row frame follows the scroll without a morph.
        self.frame.at_once();
    }

    /// Cancel (box 0x16) or Esc (`MainScr_ESCPressedWhenMultiplayerselect` case 1): the panel fades out; the caller
    /// sets `MaxMainKorok = 3`.
    pub fn cancel(&mut self) {
        if !self.closing {
            self.closing = true;
            self.pic_cel = 0.0;
        }
    }
}

/// The settings' value texts and their tables (`ScenarioKezdoBolygoSzamok` exe 0x9140d0, `ScenarioKezdoPenzek`
/// 0x9140b8; the difficulty, technology and population names at exe 0xbbb248, 0xbbb2b0, 0xbbb300 are filled from
/// `GetString` at start-up).
pub struct Tables {
    pub planets: [i32; 5],
    pub money: [i32; 6],
}

impl Tables {
    pub fn read(exe: &ig2_rules::PeImage) -> Self {
        let i = |a: u32| exe.read_loaded(a, 4).ok().map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()));
        Self { planets: std::array::from_fn(|k| i(0x9140d0 + 4 * k as u32)), money: std::array::from_fn(|k| i(0x9140b8 + 4 * k as u32)) }
    }
}

/// The textures the panel draws: the scenario picture (`kep` 0x3e) and the race pictures (keps 65..71, brushes 4..10).
#[derive(Default)]
pub struct Pictures {
    pub scenario: Option<(String, TexId, [f32; 2])>,
    pub races: Vec<Option<TexId>>,
}

/// What the list draws from, read once: the entries, the settings' tables, the pictures.
pub struct Assets {
    pub entries: Vec<Entry>,
    pub tables: Tables,
    pub pics: Pictures,
}

impl Assets {
    pub fn load(install: &Install, texts: &UiTexts) -> Self {
        let exe = install.read_raw("ig2.exe").unwrap_or_default();
        let (entries, tables) = match ig2_rules::PeImage::parse(&exe) {
            Ok(pe) => (load_entries(install, &pe, texts), Tables::read(&pe)),
            Err(_) => (Vec::new(), Tables { planets: [0; 5], money: [0; 6] }),
        };
        Self { entries, tables, pics: Pictures::default() }
    }

    /// The race pictures (`MainScrBrushok` 4..10: keps 65..71, `_ESIG_S1..6`, `_ESIG_0`, 64 × 64): the remaster loads
    /// them from `textures/main/fejek3/<race>a.bmp` (exe string 0x8568e8, its one reference 0x4f961b) [obs: the
    /// Antari's head as the capture shows it, 1:1].
    pub fn race_pictures(&mut self, r: &mut crate::render::Renderer, install: &Install) {
        if !self.pics.races.is_empty() {
            return;
        }
        self.pics.races = (1..=8).map(|k| install.resolve_texture("textures/main/fejek3/", &format!("{k}a.bmp")).and_then(|p| crate::assets::Image::load(install, &p).ok()).map(|img| r.add_texture(&img))).collect();
    }

    /// `SetKepName(0x3e, entry + 8)`, `LoadKepToMemory(0x3e)`: the shown entry's picture, loaded when it changes.
    pub fn picture(&mut self, r: &mut crate::render::Renderer, install: &Install, pic: &str) {
        if self.pics.scenario.as_ref().is_some_and(|(p, ..)| p == pic) {
            return;
        }
        let (dir, name) = pic.rsplit_once('\\').unwrap_or(("", pic));
        let path = install.resolve_texture(&format!("scenario/{}/", dir.replace('\\', "/")), name);
        self.pics.scenario = path.and_then(|p| crate::assets::Image::load(install, &p).ok()).map(|img| {
            let size = [img.width as f32, img.height as f32];
            (pic.to_string(), r.add_texture(&img), size)
        });
    }
}

impl ScenarioList {
    /// `MainScrGfx2D_1`'s list part (spec §7.1): the faders, the scroll bar, the picture sliding in from the left.
    pub fn draw(&mut self, f: &mut Frame, d: &Draw, pics: &Pictures, tables: &Tables) {
        let (white, v) = (d.white, &d.v);
        let t = 1.0 - self.open / 100.0;
        let (t1, t2) = (t * 155.0 + 100.0, t * 225.0 + 30.0);
        let s = 1.0;
        let box_ = |f: &mut Frame, r: [f32; 4], t1: f32, t2: f32| fader::kirak_fader(f, white, v, r, [0.0, 54.0 * s, 90.0 * s], t1, [0.0, 90.0 * s, 130.0 * s], t2, [0.0, 0.0]);
        for r in [[430.0, 30.0, 180.0, 400.0], [435.0, 35.0, 170.0, 21.0], [435.0, 62.0, 170.0, 362.0], [430.0, 438.0, 87.0, 34.0], [523.0, 438.0, 87.0, 34.0], [35.0, 30.0, 380.0, 27.0], [35.0, 340.0, 380.0, 130.0]] {
            box_(f, r, t1, t2);
        }
        if !self.high_score {
            box_(f, [39.0, 345.0, 70.0, 60.0], t1, t2);
        }
        let e = self.entry().cloned();
        let pt = 1.0 - (self.open / 100.0) * self.pic / 100.0;
        let (p1, p2) = (pt * 155.0 + 100.0, pt * 225.0 + 30.0);
        match &e {
            Some(e) if e.skirmish() => {
                let sk = self.sk;
                let arrow = |f: &mut Frame, x: f32, y: f32, on: bool| {
                    if on {
                        box_(f, [x, y, 33.0, 15.0], p1, p2);
                    }
                };
                arrow(f, 39.0, 408.0, sk.race > 1);
                arrow(f, 76.0, 408.0, sk.race < 8);
                box_(f, [39.0, 427.0, 70.0, 15.0], p1, p2);
                arrow(f, 39.0, 446.0, sk.computers > 1);
                arrow(f, 76.0, 446.0, sk.computers < 7);
                box_(f, [114.0, 345.0, 70.0, 40.0], p1, p2);
                for x in [114.0, 189.0, 264.0, 339.0] {
                    box_(f, [x, 410.0, 70.0, 32.0], p1, p2);
                }
                for (k, x) in [(2, 114.0), (3, 189.0), (4, 264.0), (5, 339.0)] {
                    let (_, _, lo, hi) = SETTINGS[k];
                    arrow(f, x, 446.0, sk.value(k) > lo);
                    arrow(f, x + 37.0, 446.0, sk.value(k) < hi);
                }
                arrow(f, 114.0, 390.0, sk.population > 0);
                arrow(f, 151.0, 390.0, sk.population < 3);
            }
            Some(_) => box_(f, [300.0, 446.0, 110.0, 19.0], p1, p2),
            None => {}
        }
        // The scroll bar: the track and the knob.
        let n = self.entries.len();
        box_(f, [613.0, 62.0, 18.0, 368.0], t1, t2);
        let h = if n == 0 { 364.0 } else { (23.00023 / n as f32 * 364.0).min(364.0) };
        let off = if n == 0 { 0.0 } else { (self.first as f32 / (n as f32 * 1.00001 - 23.0) * (364.0 - h)).max(0.0) };
        box_(f, [615.0, 64.0 + off, 14.0, h], t1, t2);
        // The scenario picture slides in from the left (`BrushClipToScreenMelyikXY(3, 1, 35 − sin((1 − K/100)·π/2)·600,
        // 0x50)`), 380 × 238.
        if let (Some((_, tex, _)), true) = (&pics.scenario, n > 0) {
            let x = (35.0 - (((1.0 - self.pic / 100.0) as f64 * std::f64::consts::FRAC_PI_2).sin() * 600.0) as f32) as i32 as f32;
            let r = [x, 80.0, 380.0, 238.0];
            f.push(*tex, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0), [r[2] * v.k / 2.0, r[3] * v.k / 2.0], [1.0; 4]));
        }
        self.draw_texts(f, d, pics, tables);
    }

    /// The selected row's `DesignKeret1(0x1b5, 0x43 + 0xf·(Sel − Start), 0xa5, 0xf)` place, asked for while the row
    /// is in view (false: not drawn).
    pub fn ask_frame(&mut self) -> bool {
        let on = self.selected >= self.first && self.selected < self.first + ROWS;
        if on {
            self.frame.ask([0x1b5, 0x43 + 0xf * (self.selected - self.first) as i32, 0xa5, 0xf]);
        }
        on
    }

    /// `MainScrGfx2D_2`'s list part (spec §7.2).
    fn draw_texts(&mut self, f: &mut Frame, d: &Draw, pics: &Pictures, tables: &Tables) {
        let v = &d.v;
        // `KiirStrSzinSzorzo` takes integers: `O` (`(int)Nyitottsag`) for the panel's own lines, `P =
        // (int)(KepNyitottsag·Nyitottsag/100.0)` for the entry's (exe 0x4fc8d0).
        let fade = self.open.trunc();
        let pfade = (self.pic as f64 * self.open as f64 / 100.0) as i32 as f32;
        let hover = self.hot();
        let text = |f: &mut Frame, s: &[u8], x: i32, y: i32, len: usize, size: usize, a: Align, c: usize, m: f32| d.font.text(f, v, s, x as f32, y as f32, len, size, a, d.col(c, m), None);
        let colon = |mut s: Vec<u8>| {
            s.push(b':');
            s
        };
        let e = self.entry().cloned();
        // The race picture (unless high-score mode): `Brush3(kep, a, a, a + 42, a + 340, c·64, c·64)`.
        if let (false, Some(e)) = (self.high_score, &e) {
            let race = if e.skirmish() { self.sk.race } else { e.race };
            if let Some(Some(tex)) = pics.races.get((race - 1).max(0) as usize) {
                // `a = (int)((1 − P/100)·32)`, `w = (int)(P/100·64)`: the picture's centre `w × w` at 1:1 (it grows
                // from the middle, unscaled), white, no fade of its own.
                let c = pfade / 100.0;
                let a = ((1.0 - c) * 32.0) as i32 as f32;
                let w = (c * 64.0) as i32 as f32;
                let r = [a + 42.0, a + 340.0, w, w];
                let mut i = Instance::new(kind::SPRITE, v.px(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0), [r[2] * v.k / 2.0, r[3] * v.k / 2.0], [1.0; 4]);
                i.uv = [a / 64.0, a / 64.0, w / 64.0, w / 64.0];
                f.push(*tex, Blend::GammaAlpha, i);
            }
        }
        text(f, &colon(d.texts.get(15, 2, 1)), 0x1b8, 0x28, 0x1e, 3, Align::Left, 1, fade);
        text(f, &d.texts.get(15, 2, 2), 0x1d9, 0x1c1, 7, 3, Align::Centre, if hover == 0x15 { 2 } else { 1 }, fade);
        text(f, &d.texts.get(15, 2, 3), 0x236, 0x1c1, 7, 3, Align::Centre, if hover == 0x16 { 2 } else { 1 }, fade);
        for k in self.first..(self.first + ROWS).min(self.entries.len()) {
            let en = &self.entries[k];
            let c = if k == self.selected { 2 } else if en.skirmish() { 0x25 } else { 0x22 };
            text(f, &en.name, 0x1b9, 0x46 + 0xf * (k - self.first) as i32, en.name.len(), 2, Align::Left, c, fade);
        }
        if self.ask_frame() {
            self.frame.draw(f, d.white, v, d.col(0x22, fade));
        }
        let Some(e) = e else { return };
        text(f, &colon(d.texts.get(15, 2, 4)), 0x28, 0x28, 0x1e, 2, Align::Left, 0x25, fade);
        // The difficulty label for every entry, skirmish too (fade `O`).
        text(f, &colon(d.texts.get(15, 2, 6)), 0xfa, 0x28, 0x1e, 2, Align::Left, 0x25, fade);
        text(f, &e.name, 100, 0x28, 0x1e, 2, Align::Left, 0x22, pfade);
        if !self.high_score {
            let race = if e.skirmish() { self.sk.race } else { e.race };
            text(f, &d.texts.get(0x11, 1, (race.clamp(0, 19) + 5) as u16), 0x2c, 0x18c, 10, 1, Align::Field, 0x22, pfade);
        }
        if e.skirmish() {
            let sk = self.sk;
            let arrows = |f: &mut Frame, x: i32, y: i32, k: usize, lo: u32| {
                let (_, _, mn, mx) = SETTINGS[k];
                if sk.value(k) > mn {
                    text(f, b"<-", x, y, 5, 2, Align::Field, if hover == lo { 2 } else { 1 }, pfade);
                }
                if sk.value(k) < mx {
                    text(f, b"->", x + 0x25, y, 5, 2, Align::Field, if hover == lo + 1 { 2 } else { 1 }, pfade);
                }
            };
            arrows(f, 0x29, 0x19c, 0, 0x18);
            arrows(f, 0x29, 0x1c2, 1, 0x1a);
            arrows(f, 0x74, 0x1c2, 2, 0x1d);
            arrows(f, 0xbf, 0x1c2, 3, 0x1f);
            arrows(f, 0x10a, 0x1c2, 4, 0x21);
            arrows(f, 0x155, 0x1c2, 5, 0x23);
            arrows(f, 0x74, 0x18b, 6, 0x25);
            let val = |f: &mut Frame, s: &[u8], x: i32, y: i32| text(f, s, x, y, 10, 1, Align::Field, 0x22, pfade);
            let label = |f: &mut Frame, s: Vec<u8>, x: i32, y: i32| text(f, &s, x, y, 10, 1, Align::Field, 0x25, pfade);
            val(f, &fmt1(&d.texts.get(15, 2, 5), sk.computers + 1), 0x2c, 0x1b1);
            label(f, d.texts.get(15, 2, 6), 0x77, 0x1a1);
            val(f, &d.texts.get(15, 4, sk.difficulty as u16), 0x77, 0x1b1);
            label(f, colon(d.texts.get(15, 2, 8)), 0xc2, 0x1a1);
            val(f, &fmt1(&d.texts.get(15, 2, 7), tables.planets[sk.planets.clamp(0, 4) as usize]), 0xc2, 0x1b1);
            label(f, colon(d.texts.get(15, 2, 10)), 0x10d, 0x1a1);
            val(f, &fmts(&d.texts.get(15, 2, 9), &d.texts.get(15, 5, sk.technology as u16 + 1)), 0x10d, 0x1b1);
            label(f, colon(d.texts.get(15, 2, 0xb)), 0x158, 0x1a1);
            val(f, format!("$ {}", tables.money[sk.money.clamp(0, 5) as usize]).as_bytes(), 0x158, 0x1b1);
            label(f, colon(d.texts.get(15, 2, 0xc)), 0x77, 0x160);
            val(f, &d.texts.get(15, 6, sk.population as u16 + 1), 0x77, 0x170);
        } else {
            text(f, &d.texts.get(15, 2, if self.high_score { 0xe } else { 0xd }), 0x163, 0x1c4, 0x1e, 2, Align::Centre, if hover == 0x27 { 2 } else { 0x25 }, fade);
            text(f, &d.texts.get(15, 4, e.difficulty.max(0) as u16), 0x143, 0x28, 0x1e, 2, Align::Left, 0x22, pfade);
        }
        if self.high_score && !e.skirmish() {
            text(f, &fmts(&d.texts.get(15, 2, 0xf), &e.name), 0x2d, 0x15a, 0x32, 3, Align::Left, 1, pfade);
            for k in 0..HIGHSCORE_SLOTS {
                let (x, y) = (if k >= 5 { 0xe1 } else { 0x2d }, 0x173 + 0xd * (k % 5) as i32);
                text(f, format!("{:02}.", k + 1).as_bytes(), x, y, 3, 2, Align::Left, 2, pfade);
                match e.scores.get(k) {
                    Some((name, score)) => {
                        text(f, name, x + 0x16, y, 0x14, 2, Align::Left, 2, pfade);
                        let days = [format!("{} ", score / 0x32).into_bytes(), d.texts.get(15, 2, 0x10)].concat();
                        text(f, &days, x + 0x5c, y, 0xc, 2, Align::Left, 2, pfade);
                    }
                    None => text(f, &d.texts.get(15, 2, 0x11), x + 0x16, y, 0x14, 2, Align::Left, 1, pfade),
                }
            }
        } else {
            // The ten description strings (exe 0x4fc8d0, entry +0xc..+0x30, empty ones skipped): rows 0..=4 beside the
            // race picture at x 0x2a + 0x48, the rest under it at 0x2a; a skirmish entry's all at 0x2a + 0x48.
            for (k, l) in e.lines.iter().enumerate().filter(|(_, l)| !l.is_empty()) {
                let x = 0x2a + if k < 5 || e.skirmish() { 0x48 } else { 0 };
                text(f, l, x, 0x15a + 0xd * k as i32, l.len(), 2, Align::Left, 0x22, pfade);
            }
        }
    }
}

/// The name entry's dialog (`MainScrGfx2D_1`: the fader (111, 163, 420 × 98); `MainScrGfx2D_2` `IOAzonosito` 0x98:
/// `GetString(15, 3, 2)` centred at (0x140, 0xbc) size 4 colour 1, the name in 9 cells centred at (0x11c, 0xe6 − 9)
/// size 3 colour 2, the cursor `FillToScreen` blinking with `AktIdo200`).
pub fn draw_name(f: &mut Frame, d: &Draw, n: &crate::mainscr::NameEntry, blink: bool) {
    let v = &d.v;
    fader::kirak_fader(f, d.white, v, [111.0, 163.0, 420.0, 98.0], [0.0, 54.0, 90.0], 100.0, [0.0, 90.0, 130.0], 30.0, [0.0, 0.0]);
    d.font.text(f, v, &d.texts.get(15, 3, 2), 0x140 as f32, 0xbc as f32, 0x28, 4, Align::Centre, d.col(1, 100.0), None);
    let (x0, y) = (0x11c as f32, (0xe6 - 9) as f32);
    let max = crate::mainscr::NAME_MAX;
    d.font.text(f, v, &n.text, x0, y, max, 3, Align::Field, d.col(2, 100.0), None);
    if blink {
        let cw = d.font.advance(3) as i32;
        let x = if n.cursor < max { n.cursor as i32 * cw + (((max - n.text.len().min(max)) as i32 * cw) >> 1) + x0 as i32 } else { (max as i32 - 1) * cw + x0 as i32 };
        let (w, h) = (cw as f32, d.font.height(3));
        let c = d.col(1, 100.0);
        f.push(d.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(x as f32 + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
    }
}

/// The disconnected box (`YouAreDisconnectedVolt`): `MainScrGfx2D_1`'s two faders (111, 163, 420 × 98) and the OK
/// button (270, 220, 100 × 30), then `FUN_004fe190` (exe, a `SetRenderOther` box of `MainScrInitNullBoxes`):
/// `GetString(0x13, 6, 1)` at (0x140, 0xb4) and the reason `GetString(0x12, 4, n)` at (0x140, 200), both 0x28 cells,
/// size 3, centred, colour 2; "OK" (exe 0x856adc) at (0x140, 0xe5), colour 2 with the pointer on it (x 270..370,
/// y 220..250), else 1.
pub fn draw_disconnected(f: &mut Frame, d: &Draw, reason: u16, at: [f32; 2]) {
    let v = &d.v;
    for r in [[111.0, 163.0, 420.0, 98.0], [270.0, 220.0, 100.0, 30.0]] {
        fader::kirak_fader(f, d.white, v, r, [0.0, 54.0, 90.0], 100.0, [0.0, 90.0, 130.0], 30.0, [0.0, 0.0]);
    }
    d.font.text(f, v, &d.texts.get(0x13, 6, 1), 0x140 as f32, 0xb4 as f32, 0x28, 3, Align::Centre, d.col(2, 100.0), None);
    d.font.text(f, v, &d.texts.get(0x12, 4, reason), 0x140 as f32, 200.0, 0x28, 3, Align::Centre, d.col(2, 100.0), None);
    let (x, y) = (at[0].floor() as i32, at[1].floor() as i32);
    let on = ((x - 0x10e) as u32) < 0x65 && ((y - 0xdc) as u32) < 0x1f;
    d.font.text(f, v, b"OK", 0x140 as f32, 0xe5 as f32, 0x28, 3, Align::Centre, d.col(if on { 2 } else { 1 }, 100.0), None);
}

/// `sprintf(fmt, n)` for the one `%d` the texts carry (the texts are code-page bytes, kept as they are).
fn fmt1(fmt: &[u8], n: i32) -> Vec<u8> {
    match fmt.windows(2).position(|w| w == b"%d") {
        Some(i) => [&fmt[..i], n.to_string().as_bytes(), &fmt[i + 2..]].concat(),
        None => fmt.to_vec(),
    }
}

/// `sprintf(fmt, s)` for one `%s`.
fn fmts(fmt: &[u8], s: &[u8]) -> Vec<u8> {
    match fmt.windows(2).position(|w| w == b"%s") {
        Some(i) => [&fmt[..i], s, &fmt[i + 2..]].concat(),
        None => fmt.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The texts' accented bytes stay single bytes (the `%d` line "JÁTÉKOS" in Latin-2: the exe's `sprintf` copies
    /// them, `KiirStr` draws one glyph each).
    #[test]
    fn formats_keep_code_page_bytes() {
        assert_eq!(fmt1(b"%d J\xc1T\xc9KOS", 5), b"5 J\xc1T\xc9KOS");
        assert_eq!(fmts(b"%s BOLYG\xd3", b"60"), b"60 BOLYG\xd3");
    }

    fn list(n: usize) -> ScenarioList {
        let mut e: Vec<Entry> = (0..n).map(|i| Entry { name: format!("s{i}").into_bytes(), registry: format!("s{i}"), ..Default::default() }).collect();
        e[0].kind = 1;
        e[0].race = 1;
        ScenarioList::open(e)
    }

    /// The panel opens by 1 a tick; the picture by 2; a new row fades the picture out, then swaps it.
    #[test]
    fn open_and_select() {
        let mut s = list(15);
        for _ in 0..100 {
            s.tick(false);
        }
        assert_eq!((s.open, s.pic), (100.0, 100.0));
        // Row 2 (y 0x41 + 2·15).
        assert_eq!(s.press([500.0, 0x41 as f32 + 30.0], true), None);
        assert_eq!((s.selected, s.shown, s.pic_cel), (2, 0, 0.0));
        for _ in 0..50 {
            s.tick(false);
        }
        assert_eq!((s.shown, s.pic, s.pic_cel), (2, 0.0, 100.0));
    }

    /// The skirmish arrows step within their ranges; the knob scrolls by `(y − 64)/364·n − 11.5`.
    #[test]
    fn skirmish_and_scroll() {
        let mut s = list(40);
        for _ in 0..100 {
            s.tick(false);
        }
        assert_eq!(s.sk, Skirmish::new(1));
        assert_eq!(s.press([40.0, 409.0], true), Some(Act::Click));
        assert_eq!(s.sk.race, 1, "race 1 can't go lower");
        assert_eq!(s.press([77.0, 409.0], true), Some(Act::Click));
        assert_eq!(s.sk.race, 2);
        s.press([620.0, 427.0], true);
        assert_eq!(s.first, 17);
        s.release(true);
        s.press([620.0, 64.0], true);
        assert_eq!(s.first, 0);
    }

    /// The knob drag (rows 344-346): exclusive while held (a press elsewhere scrolls, `AktIkon` stays 0x1c), it ends
    /// only once neither button is held, and a middle press drops both.
    #[test]
    fn knob_drag_holds() {
        let mut s = list(40);
        for _ in 0..100 {
            s.tick(false);
        }
        s.press([620.0, 64.0], true);
        assert_eq!(s.hot(), 0x1c);
        // The right button over the race arrow: the drag keeps it, nothing steps.
        assert_eq!(s.press([77.0, 409.0], false), None);
        assert_eq!(s.sk.race, 1);
        s.release(true);
        s.hover([300.0, 427.0]);
        assert_eq!((s.first, s.hot()), (17, 0x1c), "right still held");
        s.release(false);
        // The release frame keeps 0x1c (row 360), the next frame tests the spots again.
        s.hover([300.0, 427.0]);
        assert_eq!((s.first, s.hot()), (17, 0x1c));
        s.hover([300.0, 427.0]);
        assert_eq!(s.hot(), 0);
        s.press([620.0, 64.0], true);
        s.middle();
        s.hover([620.0, 427.0]);
        s.hover([620.0, 427.0]);
        assert_eq!(s.first, 0, "a middle press ends the drag");
        s.release(true);
    }
}
