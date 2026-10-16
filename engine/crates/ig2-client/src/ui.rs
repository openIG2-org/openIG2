//! The 2D screens drawn from the sim's state: the scrolling message line on top of every screen
//! (re/screens/messages.md §2), the info screen's charts (re/screens/info.md) and the message inbox
//! (re/screens/messages.md §3).

use ig2_formats::dialogue::MessageKind;
use ig2_formats::strings::UiStrings;
use ig2_formats::{dialogue, text, Install};
use ig2_sim::save::{va, SaveState};

use crate::font::{Align, Font, Virtual};
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
    /// `Viszony[EN][e].+0x25`: 1 war, 2 cease-fire, 3 neutral, 4 NAP, 5 alliance.
    pub level: [u8; 19],
    /// The inbox (`Uzenetek`, 0x88 each): block id, kept/read flag, date (`+0xc`, an `Ido`).
    pub inbox: Vec<(i32, u8, i32)>,
    /// The scrolling line's waiting texts (`ScrollUzenetek` 1..=db: text, important) and the history.
    pub queue: Vec<(Vec<u8>, bool)>,
    pub history: Vec<Vec<u8>>,
    /// The top bar (re/screens/hud.md §2); the speed buttons come from the client.
    pub bar: crate::hud::BarState,
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
        let inbox = (0..ib.count.max(0) as usize)
            .filter_map(|i| ib.messages.get(i * 0x88..(i + 1) * 0x88))
            .map(|m| (i32::from_le_bytes(m[0..4].try_into().unwrap()), m[4], i32::from_le_bytes(m[0xc..0x10].try_into().unwrap())))
            .collect();
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
            level: std::array::from_fn(|e| rel.get(en).map_or(0, |r| r[e].level)),
            inbox,
            queue: entries(&t.main, t.count).into_iter().map(|e| (cstr(e), e[0x3c] != 0)).collect(),
            history: entries(&t.history, t.history_count).into_iter().map(cstr).collect(),
            bar: bar_state(state, en),
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
        Self { strings, kinds }
    }

    /// `GetString(file, section, index)` as the font's bytes (the decoded text back in Latin-1 where it fits).
    pub fn get(&self, file: u16, section: u16, index: u16) -> Vec<u8> {
        latin1(self.strings.get(file, section, index))
    }
}

pub fn latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect()
}

/// The info screen's chart (`Info_PutChart` ELF 0x55dbf0): every known empire's smoothed row of chart `c`, cells
/// 4..=50 as line segments `(4k + 385, 426 − v/scale)`, scale = the largest such value / 153, in `DDColor[e + 15]`
/// dimmed by the distance to the row's sweeping highlight (`InfoChartSzamlalok`).
pub fn draw_chart(f: &mut Frame, white: TexId, v: &Virtual, s: &UiSnap, palette: &[[f32; 3]], chart: usize, sweep: f32) {
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
        for k in 4..0x32 {
            let bright = (1.0 - ((sweep - k as f32).abs() / 55.0) * 3.0).clamp(0.1, 1.0);
            let c = [base[0] * bright, base[1] * bright, base[2] * bright, 1.0];
            let p0 = v.px(k as f32 * 4.0 + 385.0, (426.0 - cell(e, k) / scale).trunc());
            let p1 = v.px((k + 1) as f32 * 4.0 + 385.0, (426.0 - cell(e, k + 1) / scale).trunc());
            f.push(white, Blend::Alpha, Instance::line(p0, p1, v.k.max(1.0), c));
        }
    }
}

/// A plain dark panel (the original's 3D frames aren't drawn) in virtual pixels.
pub fn panel(f: &mut Frame, white: TexId, v: &Virtual, x: f32, y: f32, w: f32, h: f32) {
    let c = v.px(x + w / 2.0, y + h / 2.0);
    f.push(white, Blend::Alpha, Instance::new(kind::SPRITE, c, [w * v.k / 2.0, h * v.k / 2.0], [0.03, 0.06, 0.12, 0.85]));
}

/// The info screen (`AktKep` 9, re/screens/info.md): race names on their columns (`KirakFajNeveket(40, 117)`),
/// the chart panel with the chart's name (`GetString(9, 1, 5 + c)`).
#[allow(clippy::too_many_arguments)]
pub fn draw_info(f: &mut Frame, font: &Font, white: TexId, v: &Virtual, s: &UiSnap, texts: &UiTexts, rules: &ig2_rules::Rules, palette: &[[f32; 3]], chart: usize, time: f32) {
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    for e in 1..=8 {
        if !s.known[e] {
            continue;
        }
        let mut c = match s.level[e] {
            1 => 6,
            2 => 3,
            4 => 0x29,
            5 => 7,
            _ => 1,
        };
        if e == s.human {
            c = 2;
        }
        if !s.alive[e] {
            c = 0xc;
        }
        // `Faj_Nev[e]` = `GetString(0x11, 1, race + 5)`.
        let name = texts.get(0x11, 1, s.races[e] as u16 + 5);
        let x = rules.race_name_x.get(e - 1).copied().unwrap_or(40) as f32;
        font.text(f, v, &name, x, 117.0, 0x27, 2, Align::Left, col(c), None);
    }
    // The chart panel (the original's 3D frame spans about 388..612 × 272..452 on the reference capture).
    panel(f, white, v, 388.0, 272.0, 224.0, 180.0);
    let title = texts.get(9, 1, 5 + chart as u16);
    font.text(f, v, &title, 500.0, 276.0, 0, 2, Align::Centre, col(2), None);
    // `FRISSIT_Info2`: each row's highlight runs along its cells (`InfoChartSzamlalok`, speed `rand() % 400 + 200`
    // per row) [inf: one shared sweep here].
    let sweep = (time * 40.0) % 80.0 - 10.0;
    draw_chart(f, white, v, s, palette, chart, sweep);
}

/// The inbox (`MessagesGfx2D_2` ELF 0x5a1d30, `AktKep` 10): the four entries from `AktUzenetPointer`, each 45 high
/// from `UBAY` = 352 upwards: the sender (`UzenetFajtak[KerUz(block)]+7`, size `UBAFont2` = 3, colour `UBASzin4`
/// = 34, or `UBASzin1` = 1 when kept), the date (`Ido2Ev/Honap/Nap`, size 2, colour 2) and the two subject lines.
#[allow(clippy::too_many_arguments)]
pub fn draw_inbox(f: &mut Frame, font: &Font, white: TexId, v: &Virtual, s: &UiSnap, texts: &UiTexts, palette: &[[f32; 3]], first: usize) {
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    panel(f, white, v, 0.0, 214.0, 160.0, 186.0);
    let n = s.inbox.len();
    let (x, w2) = (3.0, font.advance(2));
    for (i, slot) in inbox_rows(n, first) {
        let Some(&(block, kept, date)) = s.inbox.get(i) else { continue };
        let y = 352.0 - (3 - slot) as f32 * 45.0;
        let kind = texts.kinds.iter().find(|k| k.id as i32 == block);
        let sender = kind.map(|k| latin1(&k.sender)).unwrap_or_else(|| format!("#{block}").into_bytes());
        font.text(f, v, &sender, x, y, 0x13, 3, Align::Left, col(if kept & 1 != 0 { 1 } else { 34 }), None);
        let (year, month, day) = (date / 18000, date / 1500 % 12 + 1, date / 50 % 30 + 1);
        font.int(f, v, year as i64, x, y + 13.0, 4, 2, Align::Left, col(2));
        font.text(f, v, b".", x + w2 * 4.0, y + 13.0, 1, 2, Align::Left, col(2), None);
        font.int(f, v, month as i64, x + w2 * 5.0, y + 13.0, 2, 2, Align::Left, col(2));
        font.text(f, v, b".", x + w2 * (month / 10 + 6) as f32, y + 13.0, 1, 2, Align::Left, col(2), None);
        font.int(f, v, day as i64, x + w2 * (month / 10 + 7) as f32, y + 13.0, 2, 2, Align::Left, col(2));
        if let Some(k) = kind {
            font.text(f, v, &latin1(&k.subject[0]), x, y + 23.0, 0x14, 2, Align::Left, col(2), None);
            font.text(f, v, &latin1(&k.subject[1]), x, y + 32.0, 0x14, 2, Align::Left, col(2), None);
        }
    }
    if n == 0 {
        font.text(f, v, b"No messages", x + 4.0, 300.0, 0, 2, Align::Left, col(2), None);
    }
    // The message history (`GFX_ScrollHistory`): entries at (268, 16 + 10k), size 2, colour 0x21.
    for (k, t) in s.history.iter().enumerate() {
        font.text(f, v, t, 268.0, 26.0 + 10.0 * k as f32, 0x36, 2, Align::Left, col(0x21), None);
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

/// Which screen is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Starmap,
    Colony,
    Info,
    Messages,
}

/// The 2D layer's resources and UI state.
pub struct Hud {
    pub font: Option<Font>,
    pub texts: UiTexts,
    pub white: TexId,
    pub palette: Vec<[f32; 3]>,
    pub scroller: Scroller,
    /// `Info_SelectedChart` (0..=9).
    pub chart: usize,
    /// `AktUzenetPointer`.
    pub inbox_first: usize,
    /// The HUD's pictures (the bar's sheet, `COLONY_X.BMP`) and the colony and starmap brush tables.
    pub pics: crate::hud::Pictures,
    pub colony_brushes: Vec<ig2_rules::Brush>,
    /// The speed buttons: `IdoSpeed` 0..=2 and `IdoPause`, set by the client each frame.
    pub speed: usize,
    pub paused: bool,
    /// The colony build menu's building (`AktEpuletTipus`; 1 = the colony hub).
    pub build_kind: usize,
}

impl Hud {
    pub fn load(r: &mut crate::render::Renderer, install: &Install, rules: &ig2_rules::Rules) -> Self {
        let palette = crate::starmap::read_palette(install).unwrap_or_else(|e| {
            eprintln!("ig2-client: palette: {e}");
            vec![[0.8; 3]; 43]
        });
        Self {
            font: Font::load(r, install, rules),
            texts: UiTexts::load(install),
            white: r.add_texture(&crate::assets::Image::white()),
            palette,
            scroller: Scroller::default(),
            chart: 0,
            inbox_first: 0,
            pics: crate::hud::Pictures::load(r, install, &rules.pictures, &[14, 17]),
            colony_brushes: rules.colony_brushes.clone(),
            speed: 2,
            paused: false,
            build_kind: 1,
        }
    }

    /// A tech's localised name (`txt17.txt`, re/screens/hud.md §4.1).
    pub fn tech_name(&self, t: usize) -> Vec<u8> {
        crate::hud::tech_name(&self.texts, t)
    }

    /// The colony screen's HUD under the top bar: building names, the radar, the panels (re/screens/hud.md §3, §4.1).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_colony(&self, f: &mut Frame, state: &SaveState, rules: &ig2_rules::Rules, maps: &[ig2_sim::ground::GroundMap], view: &crate::colony::View, scene: &crate::colony::ColonyScene, curr: &crate::colony::ColonySnap) {
        let Some(font) = &self.font else { return };
        let by_kind = |k: u8| rules.buildings.get(k as usize).map_or_else(Vec::new, |b| self.tech_name(b.tech as usize));
        view.names(f, font, &self.palette, &by_kind, scene, maps, curr);
        let c = crate::colony::hud_state(state, rules, &|t| self.tech_name(t), curr, view, maps, self.build_kind);
        let v = Virtual::new(view.camera.size);
        crate::hud::draw_colony(f, &self.pics, font, self.white, &v, &self.palette, &self.colony_brushes, scene.radar, &c);
    }

    /// The starmap's planet and fleet names (re/screens/hud.md §4.2, §4.3).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_starmap_names(&self, f: &mut Frame, view: &crate::starmap::View, assets: &crate::starmap::Assets, prev: &crate::world::Snapshot, curr: &crate::world::Snapshot, alpha: f32, time: f32) {
        if let Some(font) = &self.font {
            view.names(f, font, assets, prev, curr, alpha, time);
        }
    }

    /// Draw `screen`'s 2D part and the message line over `f`.
    pub fn draw(&self, f: &mut Frame, size: [f32; 2], s: &UiSnap, rules: &ig2_rules::Rules, screen: Screen, time: f32) {
        let Some(font) = &self.font else { return };
        let v = Virtual::new(size);
        match screen {
            Screen::Info => draw_info(f, font, self.white, &v, s, &self.texts, rules, &self.palette, self.chart, time),
            Screen::Messages => draw_inbox(f, font, self.white, &v, s, &self.texts, &self.palette, self.inbox_first),
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
