//! The 2D screens drawn from the sim's state: the scrolling message line on top of every screen
//! (re/screens/messages.md §2), the info screen's charts (re/screens/info.md) and the message inbox
//! (re/screens/messages.md §3).

use ig2_formats::dialogue::MessageKind;
use ig2_formats::strings::UiStrings;
use ig2_formats::{dialogue, text, Install};
use ig2_sim::save::{va, SaveState};

use crate::font::{Align, Font, Virtual};
pub use crate::info_screen::InfoScreen;
use crate::render::{kind, Blend, Frame, Instance, TexId};

/// What the 2D screens show, read from the sim after each tick.
#[derive(Debug, Clone, Default)]
pub struct UiSnap {
    pub human: usize,
    /// `InfoChartsSmoothed` (`f32 [10][8][55]`).
    pub charts: Vec<f32>,
    /// `BirodalomIsmeri[EN][e]`.
    pub known: [bool; 19],
    pub alive: [bool; 19],
    pub races: [u8; 19],
    /// `Viszony[a][e].+0x25`: 1 war, 2 cease-fire, 3 neutral, 4 NAP, 5 alliance.
    pub levels: Vec<[u8; 19]>,
    /// The inbox (`Uzenetek`, 0x88 each): block id, kept/read flag, date (`+0xc`, an `Ido`).
    pub inbox: Vec<(i32, u8, i32)>,
    /// The scrolling line's waiting texts (`ScrollUzenetek` 1..=db: text, important) and the history.
    pub queue: Vec<(Vec<u8>, bool)>,
    pub history: Vec<Vec<u8>>,
    /// The top bar (re/screens/hud.md §2); the speed buttons come from the client.
    pub bar: crate::hud::BarState,
    /// The inbox entries' `#k#` parameters (`+0x10 + 0x14·k`, five NUL-ended strings).
    pub inbox_params: Vec<Vec<Vec<u8>>>,
    /// The info screen's panels.
    pub info: crate::info_screen::InfoData,
}

fn cstr(b: &[u8]) -> Vec<u8> {
    b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())].to_vec()
}

impl UiSnap {
    pub fn read(state: &SaveState) -> Self {
        let en = state.mem.i32(va::EN).clamp(0, 18) as usize;
        let charts = state.info_charts().smoothed;
        let met = state.starmap().met;
        let rel = state.relations();
        let ib = state.inbox();
        let rows: Vec<&[u8]> = (0..ib.count.max(0) as usize).filter_map(|i| ib.messages.get(i * 0x88..(i + 1) * 0x88)).collect();
        let inbox = rows.iter().map(|m| (i32::from_le_bytes(m[0..4].try_into().unwrap()), m[4], i32::from_le_bytes(m[0xc..0x10].try_into().unwrap()))).collect();
        let inbox_params = rows.iter().map(|m| (0..5).map(|k| cstr(&m[0x10 + 0x14 * k..0x24 + 0x14 * k])).collect()).collect();
        let t = state.ticker();
        fn entries(list: &[u8], n: i32) -> Vec<&[u8]> {
            (1..=n.max(0) as usize).filter_map(|k| list.get(k * 0x40..(k + 1) * 0x40)).collect()
        }
        Self {
            human: en,
            charts,
            known: std::array::from_fn(|e| met.get(en).is_some_and(|r| r[e] != 0)),
            alive: state.alive(),
            races: state.races(),
            levels: (0..19).map(|a| std::array::from_fn(|e| rel.get(a).map_or(0, |r| r[e].level))).collect(),
            inbox,
            queue: entries(&t.main, t.count).into_iter().map(|e| (cstr(e), e[0x3c] != 0)).collect(),
            history: entries(&t.history, t.history_count).into_iter().map(cstr).collect(),
            bar: bar_state(state, en),
            inbox_params,
            info: crate::info_screen::InfoData::read(state),
        }
    }
}

/// The top bar's money, date and research button (`GFX_IdospeedPenzDatum`, `GFX_AllandoCallBack1`).
fn bar_state(state: &SaveState, en: usize) -> crate::hud::BarState {
    use ig2_sim::save::{TECH_SIZE, TECHS};
    let m = &state.mem;
    let t = ig2_sim::time::GameTime(m.i32(va::IDO));
    let status = |k: usize| m.u8(va::TECHS + (k * TECH_SIZE) as u32 + 0x45 + en as u32);
    crate::hud::BarState {
        money: state.treasuries()[en].available(0),
        date: (t.year(), t.month(), t.day()),
        researching: m.i32(va::RESEARCHING + 4 * en as u32) > 0,
        researchable: (1..TECHS).any(|k| matches!(status(k), 3 | 4)),
        ..Default::default()
    }
}

/// The scrolling message line's motion (`FRISSIT_Scroll2` ELF 0x66ebd0, one step per 10 ms UI frame), in
/// characters: a text comes in from the right, slows to a stop at `limit` (centred when shorter than the line),
/// stays 400 frames, then speeds away to the left.
#[derive(Debug, Clone, Default)]
pub struct Scroller {
    pub text: Vec<u8>,
    pub important: bool,
    /// `UzenetStart`, `UzenetSebesseg`, `UzenetHatar`.
    pub start: f32,
    pub speed: f32,
    pub limit: f32,
}

/// The line's width in characters.
const LINE: f32 = 53.0;

impl Scroller {
    /// One UI frame; `next` pops the sim's next message (`ScrollUzenetek[1]`) when the line is free.
    pub fn step(&mut self, next: &mut dyn FnMut() -> Option<(Vec<u8>, bool)>) {
        if self.text.is_empty() || self.text.len() as f32 <= self.start {
            match next() {
                Some((t, imp)) => {
                    self.speed = (((t.len() as f64 + 53.0) * 0.0146).sqrt()) as f32;
                    // `UzenetHatar = −(53 − len)/2`; a longer text (never posted: `ScrollUzenet` cuts at 53) would
                    // take the unsigned conversion of a negative count and never stop.
                    self.limit = if t.len() <= 53 { -((53.0 - t.len() as f32) / 2.0) } else { -1e19 };
                    self.text = t;
                    self.important = imp;
                    self.start = -LINE;
                }
                None => {
                    self.text.clear();
                    return;
                }
            }
        }
        if self.limit <= self.start {
            if self.start != self.limit {
                self.start += self.speed;
                self.speed += 0.0146;
            } else {
                self.speed -= 1.0;
                if self.speed == 0.0 {
                    self.start += 0.01;
                    self.speed = 0.2;
                }
            }
        } else {
            self.start += self.speed;
            self.speed = (self.speed - 0.0146).max(0.02);
            if self.limit <= self.start {
                self.start = self.limit;
                self.speed = 400.0;
            }
        }
    }

    /// `GFX_Scroll` ELF 0x66f270: the 54-character window of 53 spaces + the text at `(start + 53)·6` pixels,
    /// drawn from x = 280 − offset % 6, y 7, size 2, clipped to 270..593; colour 0x21, or 6 blinking for an
    /// important one.
    pub fn draw(&self, f: &mut Frame, font: &Font, v: &Virtual, palette: &[[f32; 3]], time: f32) {
        if self.text.is_empty() {
            return;
        }
        let off = ((self.start + LINE) * 6.0) as i32;
        let mut line = vec![b' '; 53];
        line.extend_from_slice(&self.text[..self.text.len().min(0xb4)]);
        let from = (off / 6).max(0) as usize;
        let window: Vec<u8> = line.iter().skip(from).take(0x36).copied().collect();
        let mut c = palette.get(if self.important { 6 } else { 0x21 }).copied().unwrap_or([1.0; 3]);
        if self.important {
            // `KiirStrSzinSzorzo = AktIdo300Szazalek` [inf: a 300 ms blink].
            let b = if (time * 1000.0 / 300.0) as i64 % 2 == 0 { 1.0 } else { 0.35 };
            c = c.map(|x| x * b);
        }
        font.text(f, v, &window, 280.0 - (off % 6) as f32, 7.0, 0x34, 2, Align::Left, c, Some((270.0, 593.0)));
    }
}

/// Texts the 2D screens need from the install.
pub struct UiTexts {
    pub strings: UiStrings,
    pub kinds: Vec<MessageKind>,
    /// The texts' language (their code page).
    pub lang: ig2_formats::text::Lang,
}

impl UiTexts {
    pub fn load(install: &Install) -> Self {
        let mut strings = UiStrings::default();
        for n in 1..=19 {
            if let Ok(b) = install.read(&format!("dataraw/text/txt{n:02}.txt")) {
                strings.add_file(&text::decode(&b, install.lang()));
            }
        }
        let kinds = install.read("dataraw/uzenetek/uzenetek.txt").map(|b| dialogue::parse_message_index(&text::decode(&b, install.lang()))).unwrap_or_default();
        Self { strings, kinds, lang: install.lang() }
    }

    /// `GetString(file, section, index)` as the font's bytes: the file's own bytes (its language's code page; the
    /// Hungarian ő/ű are not Latin-1).
    pub fn get(&self, file: u16, section: u16, index: u16) -> Vec<u8> {
        self.encode(self.strings.get(file, section, index))
    }

    /// A text of these files as the font's bytes (their code page).
    pub fn encode(&self, s: &str) -> Vec<u8> {
        self.lang.encoding().encode(s).0.into_owned()
    }
}

/// The Hungarian `txt17.txt`: `StringReadAll` fills its name tables from it whatever the game's language [obs: the
/// empire names in the live table and the morale name on the starmap of an English run].
pub fn hungarian_txt17(install: &Install) -> UiStrings {
    let mut hu = UiStrings::default();
    let file = install.read("localization/hu/dataraw/text/txt17.txt").or_else(|_| install.read("dataraw/text/txt17.txt"));
    if let Ok(b) = file {
        hu.add_file(&text::decode(&b, ig2_formats::text::Lang::Hu));
    }
    hu
}

/// `BirodalomNev[e]` = `GetString(0x11, 1, 33 + e)` of the Hungarian `txt17.txt`.
pub fn empire_names(hu17: &UiStrings) -> Vec<Vec<u8>> {
    (0..19u16).map(|e| if e == 0 { Vec::new() } else { hu_bytes(hu17.get(0x11, 1, 33 + e)) }).collect()
}

/// A Hungarian text's bytes as the game reads them (code page 1250).
pub fn hu_bytes(s: &str) -> Vec<u8> {
    ig2_formats::text::Lang::Hu.encoding().encode(s).0.into_owned()
}

/// `sprintf` for the screens' formats: `%d` takes the next number, `%%` is a per cent sign.
pub fn sprintf(fmt: &[u8], args: &[i64]) -> Vec<u8> {
    let (mut out, mut i, mut a) = (Vec::new(), 0, args.iter());
    while i < fmt.len() {
        match (fmt[i], fmt.get(i + 1)) {
            (b'%', Some(b'd')) => {
                out.extend(a.next().copied().unwrap_or(0).to_string().bytes());
                i += 2;
            }
            (b'%', Some(b'%')) => {
                out.push(b'%');
                i += 2;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// The info screen's chart (`Info_PutChart` ELF 0x55dbf0): every known empire's smoothed row of chart `c`, cells
/// 4..=50 as line segments `(4k + 385, 426 − v/scale)`, scale = the largest such value / 153, in `DDColor[e + 15]`
/// dimmed by the distance to the row's sweeping highlight (`InfoChartSzamlalok`).
pub fn draw_chart(f: &mut Frame, white: TexId, v: &Virtual, s: &UiSnap, palette: &[[f32; 3]], chart: usize, sweep: &dyn Fn(usize) -> f32) {
    let cell = |e: usize, k: usize| s.charts.get(chart * 440 + (e - 1) * 55 + k).copied().unwrap_or(-1.0);
    let mut peak = 0.0f32;
    for e in 1..=8 {
        if s.known[e] {
            for k in 4..0x33 {
                peak = peak.max(cell(e, k));
            }
        }
    }
    let mut scale = peak / 153.0;
    if scale == 0.0 {
        scale = 1.0;
    }
    for e in 1..=8 {
        if !s.known[e] {
            continue;
        }
        let base = palette.get(e + 15).copied().unwrap_or([0.7; 3]);
        let sweep = sweep(e - 1);
        for k in 4..0x32 {
            let bright = (1.0 - ((sweep - k as f32).abs() / 55.0) * 3.0).clamp(0.1, 1.0);
            let c = [base[0] * bright, base[1] * bright, base[2] * bright, 1.0];
            let p0 = v.px(k as f32 * 4.0 + 385.0, (426.0 - cell(e, k) / scale).trunc());
            let p1 = v.px((k + 1) as f32 * 4.0 + 385.0, (426.0 - cell(e, k + 1) / scale).trunc());
            f.push(white, Blend::Alpha, Instance::line(p0, p1, v.k.max(1.0), c));
        }
    }
}

/// The info screen (`AktKep` 9, re/screens/info.md) over its frame: the panels (`InfoGfx2D_2`), the chart in chart
/// mode, the race names on their columns (`KirakFajNeveket(40, 117)`, `InfoGfx2D_3`).
#[allow(clippy::too_many_arguments)]
pub fn draw_info(f: &mut Frame, font: &Font, white: TexId, v: &Virtual, s: &UiSnap, texts: &UiTexts, infostr: &[(i16, Vec<u8>)], hu17: &UiStrings, rules: &ig2_rules::Rules, palette: &[[f32; 3]], names: &[Vec<u8>], ui: &crate::info_screen::InfoScreen) {
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    crate::info_screen::draw(f, font, white, v, s, &s.info, texts, infostr, hu17, rules, names, palette, ui);
    // `Info_PutChart` draws at `infoframe` 45 in chart mode (and fading out between 45 and 60 [not drawn here]).
    if !ui.star_log && ui.frame <= crate::screen_frames::INFO_MAX_FRAMES {
        draw_chart(f, white, v, s, palette, ui.chart, &|k| ui.sweep(k));
    }
    for e in 1..=8 {
        if !s.known[e] {
            continue;
        }
        // The colour by `Viszony[InfoSelected][e]`; the selected empire white in a frame (`DesignKeret1(x − 5,
        // 114, 0x36, 0xc, 2)`, settled: its outline [inf: the morph and the blink aren't drawn]).
        let level = s.levels.get(ui.selected).map_or(0, |r| r[e]);
        let mut c = match level {
            1 => 6,
            2 => 3,
            4 => 0x29,
            5 => 7,
            _ => 1,
        };
        let x = rules.race_name_x.get(e - 1).copied().unwrap_or(40) as f32;
        if e == ui.selected {
            c = 2;
            let (bx, by, bw, bh) = (x - 5.0, 114.0, 0x36 as f32, 0xc as f32);
            for (rx, ry, rw, rh) in [(bx, by, bw, 1.0), (bx, by + bh - 1.0, bw, 1.0), (bx, by, 1.0, bh), (bx + bw - 1.0, by, 1.0, bh)] {
                let p = v.px(rx + rw / 2.0, ry + rh / 2.0);
                let k = col(2);
                f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, p, [rw * v.k / 2.0, rh * v.k / 2.0], [k[0], k[1], k[2], 1.0]));
            }
        }
        if !s.alive[e] {
            c = 0xc;
        }
        // `Faj_Nev[e]` = `GetString(0x11, 1, race + 5)`.
        let name = texts.get(0x11, 1, s.races[e] as u16 + 5);
        font.text(f, v, &name, x, 117.0, 0x27, 2, Align::Left, col(c), None);
    }
}

/// The inbox (`MessagesGfx2D_2` ELF 0x5a1d30, `AktKep` 10): the four entries from `AktUzenetPointer`, each 45 high
/// from `UBAY` = 352 upwards: the sender (`UzenetFajtak[KerUz(block)]+7`, size `UBAFont2` = 3, colour `UBASzin4`
/// = 34, or `UBASzin1` = 1 when kept), the date (`Ido2Ev/Honap/Nap`, size 2, colour 2) and the two subject lines.
#[allow(clippy::too_many_arguments)]
pub fn draw_inbox(f: &mut Frame, font: &Font, v: &Virtual, s: &UiSnap, texts: &UiTexts, palette: &[[f32; 3]], first: usize, read: &[usize]) {
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    let n = s.inbox.len();
    let (x, w2) = (3.0, font.advance(2));
    for (i, slot) in inbox_rows(n, first) {
        let Some(&(block, kept, date)) = s.inbox.get(i) else { continue };
        let y = 352.0 - (3 - slot) as f32 * 45.0;
        let kind = texts.kinds.iter().find(|k| k.id as i32 == block);
        let sender = kind.map(|k| texts.encode(&k.sender)).unwrap_or_else(|| format!("#{block}").into_bytes());
        font.text(f, v, &sender, x, y, 0x13, 3, Align::Left, col(if kept & 1 != 0 || read.contains(&i) { 1 } else { 34 }), None);
        let (year, month, day) = (date / 18000, date / 1500 % 12 + 1, date / 50 % 30 + 1);
        font.int(f, v, year as i64, x, y + 13.0, 4, 2, Align::Left, col(2));
        font.text(f, v, b".", x + w2 * 4.0, y + 13.0, 1, 2, Align::Left, col(2), None);
        font.int(f, v, month as i64, x + w2 * 5.0, y + 13.0, 2, 2, Align::Left, col(2));
        font.text(f, v, b".", x + w2 * (month / 10 + 6) as f32, y + 13.0, 1, 2, Align::Left, col(2), None);
        font.int(f, v, day as i64, x + w2 * (month / 10 + 7) as f32, y + 13.0, 2, 2, Align::Left, col(2));
        if let Some(k) = kind {
            font.text(f, v, &texts.encode(&k.subject[0]), x, y + 23.0, 0x14, 2, Align::Left, col(2), None);
            font.text(f, v, &texts.encode(&k.subject[1]), x, y + 32.0, 0x14, 2, Align::Left, col(2), None);
        }
    }
}

/// `MessagesGfx2D_2`'s rows: (inbox index, row 0..=3 from the top) for `n` messages from `AktUzenetPointer` =
/// `first`: indices `first + 3` down to `first` fill rows 0..=3; with fewer than four messages they take the bottom
/// rows, newest (highest index) first.
pub fn inbox_rows(n: usize, first: usize) -> Vec<(usize, usize)> {
    let (mut i, mut slot) = (first as i64 + 3, 0usize);
    if n < 4 {
        slot = 4 - n;
        i = first as i64 + n as i64 - 1;
    }
    let mut out = Vec::new();
    while i >= 0 && slot < 4 {
        out.push((i as usize, slot));
        slot += 1;
        i -= 1;
    }
    out
}

/// What the colony screen's HUD takes from its input state.
pub struct ColonyInput<'a> {
    pub ui: &'a crate::colony_ui::ColonyUi,
    pub sim: &'a ig2_sim::game::GameRules,
    /// The panel's turning building (`crate::colony_ui::PanelBuilding`), rendered this frame.
    pub panel: Option<TexId>,
    /// The pointer in virtual pixels.
    pub cursor: [f32; 2],
    pub now_ms: u32,
    pub time: f32,
}

/// Which screen is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Starmap,
    Colony,
    Info,
    Messages,
    /// `AktKep` 6 (re/screens/diplomacy.md), drawn by [`crate::diplomacy::Open`].
    Diplomacy,
    /// The spy screen (`crate::spy_screen`): only the top bar here.
    Spy,
    /// `AktKep` 4 (re/screens/research.md), drawn by [`crate::research::Research`].
    Research,
}

/// `DesignKeret1` (ELF 0x53f780): the animated frame around a chosen row (the research lists, the info screen's
/// race names). The frame morphs to a new place: `dkmcel = π/2` starts it, `Frissit2_Grafika` (ELF 0x5481c0, every
/// UI frame) runs `dkm` up by 0.05 to π/2 (the old frame fades out and widens), jumps to the new place and runs back
/// down to 0 (it fades in); `GfxNovelo` (+0.02 a frame) runs a light around its border.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignFrame {
    /// `dkm`, `dkmcel`, the frame shown (`dkoldx/y/sz/ma`) and asked for (`dkujx/y/sz/ma`), `GfxNovelo`.
    m: f32,
    target: f32,
    old: [i32; 4],
    new: [i32; 4],
    novelo: f32,
}

impl Default for DesignFrame {
    /// As `Frissit2_Grafika` leaves it on a new screen (`DesignKeret1OldKep != AktKep`): no frame yet.
    fn default() -> Self {
        Self { m: 0.0, target: 0.0, old: [0, 0, 1, 1], new: [0; 4], novelo: 0.0 }
    }
}

impl DesignFrame {
    /// `DesignKeret1(x, y, w, h)`'s bookkeeping: a new place starts the morph.
    pub fn ask(&mut self, r: [i32; 4]) {
        if r != self.new {
            self.new = r;
            self.target = std::f32::consts::FRAC_PI_2;
        }
    }

    /// `Frissit2_Grafika`'s step (one UI frame).
    pub fn tick(&mut self) {
        if self.target <= self.m {
            if self.m <= self.target {
                self.m = std::f32::consts::FRAC_PI_2;
                if self.old[0] != 0 {
                    self.m = self.target;
                    (self.old, self.target) = (self.new, 0.0);
                }
            } else {
                self.m -= 0.05;
                if self.m <= self.target {
                    self.m = self.target;
                }
            }
        } else {
            self.m += 0.05;
            if self.target <= self.m {
                self.m = self.target;
                (self.old, self.target) = (self.new, 0.0);
            }
        }
        self.novelo += 0.02;
    }

    /// Draw it in colour `c` (`DDColor[1]` × `KiirStrSzinSzorzo`, gamma 0..1): per row a glow (half the colour times
    /// a triangle peaking mid-height, fading out over the first and last tenth), and the border as 18 one-pixel
    /// segments, each `frac((sin(GfxNovelo + k·20°) + 1)/2)` bright; all additive (`line3(…, true)`), scaled by
    /// `1 − sin(dkm)` and widened by `w·sin(dkm)` (a third of it to the left).
    pub fn draw(&self, f: &mut Frame, white: TexId, v: &Virtual, c: [f32; 3]) {
        let fade = 1.0 - (self.m as f64).sin() as f32;
        let [x0, y0, w0, h0] = self.old;
        let w = (w0 as f32 + w0 as f32 * (1.0 - fade)) as i32;
        let x = (x0 as f32 - (w as f32 * (1.0 - fade)) / 3.0) as i32;
        let (y, h) = (y0 - 1, h0 + 1);
        let byte = |k: f32| c.map(|v| ((v * 255.0).round() * k).trunc().clamp(0.0, 255.0) / 255.0);
        let mut seg = |a: [i32; 2], b: [i32; 2], ca: [f32; 3], cb: [f32; 3]| {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let (n, xmajor) = if dx.abs() >= dy.abs() { (dx.abs(), true) } else { (dy.abs(), false) };
            for k in 0..n {
                let t = (k as f32 + 0.5) / n as f32;
                let (px, py) = if xmajor {
                    (if dx > 0 { a[0] + k } else { a[0] - 1 - k }, (a[1] as f32 + t * dy as f32).floor() as i32)
                } else {
                    ((a[0] as f32 + t * dx as f32).floor() as i32, if dy > 0 { a[1] + k } else { a[1] - 1 - k })
                };
                let col: [f32; 3] = std::array::from_fn(|i| ca[i] + (cb[i] - ca[i]) * t);
                if col.iter().any(|&v| v > 0.0) {
                    f.push(white, Blend::GammaAdditive, Instance::new(kind::SPRITE, v.px(px as f32 + 0.5, py as f32 + 0.5), [v.k / 2.0, v.k / 2.0], [col[0], col[1], col[2], 1.0]));
                }
            }
        };
        let zero = [0.0; 3];
        let tenth = (w as f64 / 10.0) as i32;
        for i in 0..h {
            let k = (1.0 - (h as f32 / 2.0 - i as f32).abs() / (h as f32 / 2.0)) * fade;
            let row = c.map(|v| (((v * 255.0).round() * k) / 2.0).trunc() / 255.0);
            // `TscVonal(x, y, row, x + w/10, y + 1, 0)`, …: `line3` gives each segment's first colour to its second
            // end [obs: dark at both ends, bright inside].
            seg([x, y + i], [x + tenth, y + i + 1], zero, row);
            seg([x + tenth, y + i], [x + w - tenth, y + i + 1], row, row);
            seg([x + w - tenth, y + i], [x + w, y + i + 1], row, zero);
        }
        let (xe, ye) = (x - 1 + w, y - 1 + h);
        let q = |n: i32| x + 4 + n * (w - 8) / 4;
        let border: [([i32; 2], [i32; 2]); 18] = [
            ([q(3), y], [x - 4 + w, y + 1]),
            ([q(2), y], [q(3), y + 1]),
            ([q(1), y], [q(2), y + 1]),
            ([x + 4, y], [q(1), y + 1]),
            ([x + 4, y + 1], [x + 2, y + 2]),
            ([x + 1, y + 2], [x + 2, y + 3]),
            ([x, y + 3], [x + 1, h - 3 + y]),
            ([x + 1, ye - 2], [x + 2, ye - 1]),
            ([x + 4, ye - 1], [x + 2, ye]),
            ([x + 4, ye], [q(1), ye + 1]),
            ([q(1), ye], [q(2), ye + 1]),
            ([q(2), ye], [q(3), ye + 1]),
            ([q(3), ye], [x - 4 + w, ye + 1]),
            ([xe - 3, ye - 1], [xe - 1, ye]),
            ([xe - 1, ye - 2], [xe, ye - 1]),
            ([xe, y + 3], [xe + 1, h - 3 + y]),
            ([xe - 1, y + 2], [xe, y + 3]),
            ([xe - 3, y + 1], [xe - 1, y + 2]),
        ];
        for (k, (a, b)) in border.into_iter().enumerate() {
            let s = ((self.novelo as f64 + k as f64 * std::f64::consts::PI / 9.0).sin() + 1.0) / 2.0;
            let col = byte(fade * (s % 1.0) as f32);
            seg(a, b, col, col);
        }
    }
}

/// A hot-spot's tooltip, as every screen shows it: `UpdateFrame` (ELF 0x578e40) picks the hot-spot under the pointer
/// (`AktIkonKiir`, the lowest id of several) once per drawn frame, `Mouse_Lekerdez` restarts the wait on a move of
/// more than a pixel, and `GFX_DateMoneyScrollFade` (ELF 0x66d160, fader pass 3) draws the box with the text over
/// everything (re/screens/spy.md §6.1).
#[derive(Debug, Clone, Copy, Default)]
pub struct Tooltip {
    /// The hot-spot shown (`OldIkonKiir`, 0 none) and its place (`Mouse_Kiiras_x/y`).
    pub shown: u32,
    pub at: [i32; 2],
    /// The pointer (virtual pixels), its last move and the clock (`LastMouseMove`, `AktIdo`, ms).
    pub mouse: [f32; 2],
    last_move: f64,
    now: f64,
}

impl Tooltip {
    /// The pointer is at `at` (`Mouse_Lekerdez`).
    pub fn moved(&mut self, at: [f32; 2]) {
        if (at[0] - self.mouse[0]).abs() as i32 > 1 || (at[1] - self.mouse[1]).abs() as i32 > 1 {
            self.last_move = self.now;
        }
        self.mouse = at;
    }

    /// Once per drawn frame at clock `now` (ms): `under` = the enabled hot-spot under the pointer (0 none) and the
    /// length of its text. It shows once the pointer rested 400 ms (or stays on the one shown); a held button hides
    /// it and restarts the wait. The box follows the pointer, kept on the 640-pixel screen (`len·6 + 0x18` wide).
    pub fn update(&mut self, now: f64, under: u32, len: usize, held: bool) {
        self.now = now;
        if under < 1 || (now <= self.last_move + 400.0 && under != self.shown) || held {
            self.shown = 0;
            if held {
                self.last_move = now;
            }
        } else {
            let w = len as i32 * 6 + 0x18;
            let [x, y] = [self.mouse[0] as i32, self.mouse[1] as i32];
            self.at = [if x + w > 0x280 { 0x280 - w } else { x }, y];
            self.shown = under;
        }
    }

    /// The box: `FadeKerettelKerekit(x + 15, y − 1, len·6 + 10, 13)` in (0, 10, 30) at 30 with a (0, 13, 31) edge at
    /// 15, the text at `(x + 21, y + 2)`, 99 cells, size `0x1a % 8` = 2, left, clipped to the screen, colour 2.
    pub fn draw(&self, f: &mut Frame, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], text: &[u8]) {
        let s = text.split(|&b| b == 0).next().unwrap_or_default();
        if self.shown == 0 || s.is_empty() {
            return;
        }
        let [x, y] = self.at.map(|c| c as f32);
        crate::starmap_panels::fader(f, white, v, [x + 15.0, y - 1.0, s.len() as f32 * 6.0 + 10.0, 13.0], [0.0, 10.0, 30.0], 30.0, [0.0, 13.0, 31.0], 15.0);
        let c = palette.get(2).copied().unwrap_or([1.0; 3]);
        font.text(f, v, s, x + 21.0, y + 2.0, 99, 2, Align::Left, c, Some((-v.x0 / v.k, 640.0 + v.x0 / v.k)));
    }
}

/// The 2D layer's resources and UI state.
pub struct Hud {
    pub font: Option<Font>,
    pub texts: UiTexts,
    pub white: TexId,
    pub palette: Vec<[f32; 3]>,
    pub scroller: Scroller,
    /// The info screen's and the messages screen's UI state, and the frame texture of the screen shown
    /// ([`crate::screen_frames::FrameModels`]).
    pub info: crate::info_screen::InfoScreen,
    pub messages: crate::messages_screen::MessagesScreen,
    pub frame_tex: Option<TexId>,
    /// The HUD's pictures (the bar's sheet, `COLONY_X.BMP`) and the colony and starmap brush tables.
    pub pics: crate::hud::Pictures,
    pub colony_brushes: Vec<ig2_rules::Brush>,
    pub starmap_brushes: Vec<ig2_rules::Brush>,
    /// The speed buttons: `IdoSpeed` 0..=2 and `IdoPause`, set by the client each frame.
    pub speed: usize,
    pub paused: bool,
    /// The starmap's panel, faders, short info and minimap (re/screens/starmap.md §11).
    pub starmap: crate::starmap_hud::StarmapHud,
    /// `BirodalomNev[e]`: the empire names (the Hungarian `txt17` §1 even in other languages [obs]).
    pub empire_names: Vec<Vec<u8>>,
    /// The Hungarian `txt17.txt` `StringReadAll` fills its name tables from (`Moral_Nev`, `Problemak_Nev`, …).
    pub hu17: UiStrings,
    /// `infostr.txt` (`InfoString_Get`: the planets' info texts in the starmap's long info), in the game's code page.
    pub infostr: Vec<(i16, Vec<u8>)>,
}

impl Hud {
    pub fn load(r: &mut crate::render::Renderer, install: &Install, rules: &ig2_rules::Rules) -> Self {
        let palette = crate::starmap::read_palette(install).unwrap_or_else(|e| {
            eprintln!("ig2-client: palette: {e}");
            vec![[0.8; 3]; 43]
        });
        crate::starmap_panels::load_fader_texture(r, install);
        let console = crate::starmap_hud::Console::load(r, install).map_err(|e| eprintln!("ig2-client: kezel.xvb: {e}")).ok();
        let hu17 = hungarian_txt17(install);
        let infostr = install
            .read("dataraw/text/infostr.txt")
            .map(|b| ig2_formats::misc_text::parse_infostr(&text::decode(&b, install.lang())).0)
            .unwrap_or_default()
            .into_iter()
            .map(|s| (s.id, install.lang().encoding().encode(&s.text).0.into_owned()))
            .collect();
        Self {
            infostr,
            starmap: crate::starmap_hud::StarmapHud { console, rotation: Default::default() },
            empire_names: empire_names(&hu17),
            hu17,
            font: Font::load(r, install, rules),
            texts: UiTexts::load(install),
            white: r.add_texture(&crate::assets::Image::white()),
            palette,
            scroller: Scroller::default(),
            info: crate::info_screen::InfoScreen::enter(0),
            messages: Default::default(),
            frame_tex: None,
            pics: crate::hud::Pictures::load(r, install, &rules.pictures, &[14, 17, 33, 97]),
            colony_brushes: rules.colony_brushes.clone(),
            starmap_brushes: rules.starmap_brushes.clone(),
            speed: 2,
            paused: false,
        }
    }

    /// Open inbox entry `i` (`KiertekelUjuzenet`; re/screens/messages.md §4).
    pub fn open_message(&mut self, install: &Install, s: &UiSnap, i: usize) {
        let Some(&(id, kept, _)) = s.inbox.get(i) else { return };
        let conversation = self.texts.kinds.iter().find(|k| k.id as i32 == id).is_some_and(|k| k.conversation);
        let params = s.inbox_params.get(i).cloned().unwrap_or_default();
        self.messages.open = Some(crate::messages_screen::Dialogue::open(install, i, id, params, conversation, kept & 1 != 0));
        if !self.messages.read.contains(&i) {
            self.messages.read.push(i);
        }
    }

    /// Show message `id` as if opened from the inbox (a check of the dialogue's drawing; no inbox entry).
    pub fn show_message(&mut self, install: &Install, id: i32) {
        let conversation = self.texts.kinds.iter().find(|k| k.id as i32 == id).is_some_and(|k| k.conversation);
        self.messages.open = Some(crate::messages_screen::Dialogue::open(install, usize::MAX, id, Vec::new(), conversation, false));
    }

    /// A tech's localised name (`txt17.txt`, re/screens/hud.md §4.1).
    pub fn tech_name(&self, t: usize) -> Vec<u8> {
        crate::hud::tech_name(&self.texts, t)
    }

    /// The colony screen's HUD under the top bar: building names, the radar, the panels (re/screens/hud.md §3, §4.1).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_colony(&self, f: &mut Frame, state: &SaveState, rules: &ig2_rules::Rules, maps: &[ig2_sim::ground::GroundMap], view: &crate::colony::View, scene: &crate::colony::ColonyScene, curr: &crate::colony::ColonySnap, input: &ColonyInput) {
        let Some(font) = &self.font else { return };
        let by_kind = |k: u8| rules.buildings.get(k as usize).map_or_else(Vec::new, |b| self.tech_name(b.tech as usize));
        view.names(f, font, &self.palette, &by_kind, scene, maps, curr);
        let ui = input.ui;
        let mut c = crate::colony::hud_state(state, rules, &|t| self.tech_name(t), curr, view, maps, ui.kind, ui.selected);
        c.panel_model = input.panel;
        let v = Virtual::new(view.camera.size);
        crate::hud::draw_colony(f, &self.pics, font, self.white, &v, &self.palette, &self.colony_brushes, scene.radar, &c);
        // `ColonyGfx2D_2`: `StarmapRovidInfoKiir(50, 424, 0)` with `StarmapMode` 1, the planet's short info.
        let t = crate::starmap_hud::InfoTexts { texts: &self.texts, hu17: &self.hu17, empire_names: &self.empire_names };
        let counter = (input.time * 100.0) as i32;
        let sel = crate::orders::Selection { mode: 1, planet: curr.planet, fleet: 0 };
        let ctx = crate::colony_ui::Ctx { state, snap: curr, rules, sim: input.sim, texts: &self.texts, tech_name: &|t| self.tech_name(t) };
        // `ColonyInfoMod` 2 (a building selected) shows `ColonyKisInfo` instead.
        if !ui.draw_building_info(f, &ctx, font, self.white, &v, &self.palette) {
            let info = crate::starmap_hud::InfoBox::new(state, rules, &t, &sel, self.starmap.step(curr.planet, counter), counter);
            for (s, x, y, size, k) in &info.lines {
                font.text(f, &v, s, *x, *y, 0x13, *size, crate::font::Align::Left, self.palette.get(*k).copied().unwrap_or([1.0; 3]), None);
            }
        }
        ui.draw(f, &ctx, font, self.white, &v, &self.palette, &self.hu17, input.cursor, input.now_ms);
    }

    /// The starmap's planet and fleet names (re/screens/hud.md §4.2, §4.3).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_starmap_names(&self, f: &mut Frame, view: &crate::starmap::View, assets: &crate::starmap::Assets, prev: &crate::world::Snapshot, curr: &crate::world::Snapshot, alpha: f32, time: f32) {
        if let Some(font) = &self.font {
            view.names(f, font, assets, prev, curr, alpha, time);
        }
    }

    /// The starmap's production view and transfer panel (re/screens/starmap.md §12–§13): the faders, the models'
    /// texture `models` (rendered by [`crate::starmap_panels::PanelModels`]), the texts.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_starmap_panels(&self, f: &mut Frame, p: &crate::starmap_panels::Panels, state: &SaveState, sel: &crate::orders::Selection, models: Option<TexId>, size: [f32; 2], time: f32) {
        let Some(font) = &self.font else { return };
        let d = crate::starmap_panels::Draw2d { font, white: self.white, v: Virtual::new(size), palette: &self.palette, texts: &self.texts, time };
        p.draw_under(f, &d, state);
        p.draw_over(f, &d, state, sel, models);
    }

    /// The starmap's radial menu, the planet's long info and the select menus (re/screens/starmap.md §14–§17) at the
    /// camera's zoom.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_starmap_menus(&self, f: &mut Frame, m: &crate::starmap_menus::Menus, state: &SaveState, rules: &ig2_rules::Rules, size: [f32; 2], zoom: f32, time: f32) {
        let Some(font) = &self.font else { return };
        let d = crate::starmap_panels::Draw2d { font, white: self.white, v: Virtual::new(size), palette: &self.palette, texts: &self.texts, time };
        let t = crate::starmap_menus::MenuTexts { texts: &self.texts, hu17: &self.hu17, empire_names: &self.empire_names, infostr: &self.infostr };
        m.draw(f, &d, &t, state, rules, &self.pics, &self.starmap_brushes, zoom, (time * 100.0) as i32);
    }

    /// The starmap's HUD under the top bar (re/screens/starmap.md §11) for the local selection.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_starmap_hud(&self, f: &mut Frame, state: &SaveState, rules: &ig2_rules::Rules, view: &crate::starmap::View, prev: &crate::world::Snapshot, curr: &crate::world::Snapshot, sel: &crate::orders::Selection, alpha: f32, time: f32) {
        let Some(font) = &self.font else { return };
        let t = crate::starmap_hud::InfoTexts { texts: &self.texts, hu17: &self.hu17, empire_names: &self.empire_names };
        let counter = (time * 100.0) as i32;
        let step = self.starmap.step(if sel.mode == 1 { sel.planet } else { 0 }, counter);
        let info = crate::starmap_hud::InfoBox::new(state, rules, &t, sel, step, counter);
        let fleets = curr.fleets_at(prev, alpha);
        let mini = crate::starmap_hud::Minimap::new(curr, &fleets, sel, &view.camera);
        let v = Virtual::new(view.camera.size);
        self.starmap.draw(f, font, self.white, &v, &self.palette, &self.pics, &self.starmap_brushes, &info, &mini, time);
    }

    /// The research screen's 2D passes around its models (re/screens/research.md §3).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_research(&self, f: &mut Frame, r: &crate::research::Research, infos: &[[Vec<u8>; 3]], tips: &[Vec<u8>], state: &SaveState, rules: &ig2_rules::Rules, models: (Option<TexId>, Option<TexId>), lines: Vec<crate::render::Instance>, size: [f32; 2], time: f32) {
        let (models, glow) = models;
        let Some(font) = &self.font else { return };
        let d = crate::starmap_panels::Draw2d { font, white: self.white, v: Virtual::new(size), palette: &self.palette, texts: &self.texts, time };
        // The info box's glass (alpha, first), then the wireframes (additive), the models, the faders and lasers.
        for (i, l) in lines.into_iter().enumerate() {
            let blend = if i == 0 && l.kind == crate::render::kind::SPRITE { crate::render::Blend::GammaAlpha } else { crate::render::Blend::GammaAdditive };
            f.push(self.white, blend, l);
        }
        if let Some(m) = models {
            r.draw_models(f, &d, m);
        }
        r.draw_faders(f, &d, state, glow);
        r.draw_over(f, &d, state, rules, &crate::research::Texts { infos });
        if let Some(t) = tips.get(r.tip.shown as usize) {
            r.tip.draw(f, font, self.white, &d.v, &self.palette, t);
        }
    }

    /// Draw `screen`'s 2D part and the message line over `f`.
    pub fn draw(&mut self, f: &mut Frame, size: [f32; 2], s: &UiSnap, rules: &ig2_rules::Rules, screen: Screen, time: f32) {
        let Some(font) = &self.font else { return };
        let v = Virtual::new(size);
        // `PutFaders` queues the info screen's fills for `fader1`, drawn before the frame model (whose panels are
        // transparent inside).
        if screen == Screen::Info {
            crate::info_screen::draw_faders(f, self.white, &v, s, &self.info);
        }
        if matches!(screen, Screen::Info | Screen::Messages) {
            if let Some(t) = self.frame_tex {
                crate::screen_frames::draw(f, t, &v);
            }
        }
        match screen {
            Screen::Info => draw_info(f, font, self.white, &v, s, &self.texts, &self.infostr, &self.hu17, rules, &self.palette, &self.empire_names, &self.info),
            Screen::Messages => {
                draw_inbox(f, font, &v, s, &self.texts, &self.palette, self.messages.first, &self.messages.read);
                crate::messages_screen::draw(f, font, self.white, &v, &self.palette, &mut self.messages, s.inbox.len(), time);
            }
            _ => {}
        }
        // The top bar (re/screens/hud.md §2), then its line (`GFX_Scroll` ELF 0x66f270; message windows aren't drawn
        // yet, so it always shows).
        let bar = crate::hud::BarState { speed: self.speed, paused: self.paused, ..s.bar };
        crate::hud::draw_bar(f, &self.pics, font, &v, &self.palette, &bar);
        self.scroller.draw(f, font, &v, &self.palette, time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_glides_in_holds_and_leaves() {
        let mut q = vec![(b"Short news".to_vec(), false)];
        let mut sc = Scroller::default();
        sc.step(&mut || q.pop());
        assert_eq!(sc.limit, -(53.0 - 10.0) / 2.0);
        let mut frames = 1;
        while sc.start < sc.limit && frames < 10_000 {
            sc.step(&mut || None);
            frames += 1;
        }
        assert_eq!(sc.start, sc.limit, "stops exactly at its place");
        // 400 frames of hold, then it moves on and leaves.
        for _ in 0..400 {
            sc.step(&mut || None);
        }
        assert!(sc.start > sc.limit);
        while !sc.text.is_empty() && frames < 20_000 {
            sc.step(&mut || None);
            frames += 1;
        }
        assert!(sc.text.is_empty());
    }

    #[test]
    fn inbox_rows_fill_from_the_bottom_newest_first() {
        assert_eq!(inbox_rows(2, 0), vec![(1, 2), (0, 3)]);
        assert_eq!(inbox_rows(6, 1), vec![(4, 0), (3, 1), (2, 2), (1, 3)]);
        assert!(inbox_rows(0, 0).is_empty());
    }
}

