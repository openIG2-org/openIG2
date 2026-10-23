//! The main menu's race choice (`Mode_Fajvalasztas`, re/screens/mainscr.md §9.2): the campaign ring opens it, the
//! race scene (`kezdo_.xv`) plays its opening, three heads spread onto a turning circle, a click types a race's blurb
//! (`fajinfo.txt`), a second click confirms it and the rings come back as the difficulty set; a difficulty click closes
//! the heads and leaves for the campaign. State and timers: `FRISSIT_MainScr2` (exe 0x4f65f0); placement:
//! `ForgatFajvalasztasDolgai` (exe 0x4f7390) and `MozgatKor` (exe 0x503540); the blurb: exe 0x4f7c60 / 0x4f9040.

use crate::font::Align;
use crate::options::Draw;
use crate::render::{kind, Blend, Frame, Instance};

/// The blurb's grid: 5 rows of 50 cells.
const COLS: usize = 50;
const ROWS: usize = 5;

/// `FajVInitPosok` (exe 0x914088): each head group's start (x, y, z), before the spread. Read from the user's exe.
pub type Starts = [[f32; 3]; 3];

/// The start table from the user's exe.
pub fn read_starts(exe: &ig2_rules::PeImage) -> Starts {
    std::array::from_fn(|k| std::array::from_fn(|c| exe.read_loaded(0x914088 + 0x10 * k as u32 + 4 * c as u32, 4).ok().map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()))))
}

/// The race blurb being typed (exe 0xbbb3f8..: 5 × 50 cells of a byte and an f32 alpha; `col` 0xbbbbc8, the text left
/// to type 0xbbc788, `typeCounter` 0xbbb194).
#[derive(Debug, Clone, PartialEq)]
pub struct Typer {
    pub cells: [[(u8, f32); COLS]; ROWS],
    pub col: usize,
    pub pending: Vec<u8>,
    counter: i32,
}

impl Default for Typer {
    fn default() -> Self {
        Self { cells: [[(b' ', 0.0); COLS]; ROWS], col: 0, pending: Vec::new(), counter: 0 }
    }
}

impl Typer {
    /// `InitKiirInfoSzoveg` (0x4f8ea0) + `InfoBelerakSzoveg(n)` (0x4f8910): a blank grid, blurb `text` to type.
    pub fn start(&mut self, text: &[u8]) {
        for row in &mut self.cells {
            for c in row.iter_mut().filter(|c| c.0 != b' ') {
                *c = (b' ', 0.0);
            }
        }
        self.col = 0;
        self.pending = text.to_vec();
    }

    /// `NewLine` (0x4f8960): rows move up, the last is blank.
    fn new_line(&mut self) {
        self.col = 0;
        self.cells.rotate_left(1);
        self.cells[ROWS - 1] = [(b' ', 0.0); COLS];
    }

    /// `FrissitFajInfoSzoveg` (0x4f7c60), one tick: the typed cells fade toward 0.5 (`alpha − 0.05` in f64 while above
    /// 0.5f); every second tick (not while `closing`) one character goes in, words wrapped at 50 columns.
    pub fn tick(&mut self, closing: bool) {
        if !closing {
            self.counter += 1;
        }
        for row in &mut self.cells {
            for c in row.iter_mut() {
                if c.0 != b' ' && c.1 > 0.5 {
                    c.1 = (c.1 as f64 - 0.05) as f32;
                }
            }
        }
        if self.counter <= 1 {
            return;
        }
        self.counter = 0;
        while self.pending.first() == Some(&b'\r') {
            self.pending.remove(0);
            self.new_line();
        }
        if self.pending.is_empty() {
            return;
        }
        // The word's length: to a space, `\r` or the end (a word starting with a space counts 0).
        let word = self.pending.iter().take_while(|&&b| b != b' ' && b != b'\r').count();
        if COLS - self.col < word {
            self.new_line();
        }
        while self.col == 0 && self.pending.first() == Some(&b' ') {
            self.pending.remove(0);
        }
        if !self.pending.is_empty() {
            let b = self.pending.remove(0);
            self.cells[ROWS - 1][self.col] = (b, 1.0);
            self.col += 1;
        }
        if self.col == COLS {
            self.new_line();
        }
    }
}

/// The race choice's state (exe 0xbbb161..0xbbb3f5).
#[derive(Debug, Clone, PartialEq)]
pub struct RaceChoice {
    /// `FajVFrame` (0xbbb168): the opening animation's frame 0..90; `FajVKINYITVA` (0xbbb162) once it ends.
    pub frame: f32,
    pub opened: bool,
    /// `MainKorMozgatoValtozo` (0xbbb174): the circle's turn, from 2.5.
    pub theta: f32,
    /// `FajVSzetnyilas` (0xbbb16c) and its target (0xbbb170): the spread 0..100.
    pub s: f32,
    pub s_cel: f32,
    /// `MozgatKorYadder` (0xbbb3e8): every head's y offset, toward −50 while a race is highlighted.
    pub y: f32,
    /// `LastKijelolt` (0xbbb3e0).
    pub last: Option<usize>,
    /// `FajValasztasFajKivalasztva` (0xbbb163), `…FajKivalsztvaBecsukas` (0xbbb3f5), `…StartGame` (0xbbb17c).
    pub confirmed: bool,
    pub unconfirming: bool,
    pub start_game: bool,
    /// `FajValasztasSelectedRace` (0xbbb178), `…NehezsegiSzint` (0xbbb180).
    pub selected: Option<usize>,
    pub level: u8,
    /// `Mode_Fajvalasztas_Becsukas` (0xbbb228).
    pub closing: bool,
    pub typer: Typer,
    /// The heads' rings' own roll (`RotateObjr(sub, 0, 0, ±0.02)`).
    pub roll: f32,
}

/// What a tick tells the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The closing ended (`Mode_Fajvalasztas = 0`, `MaxMainKorok = 3`, `Cel = 1.0`): back to the single-player set.
    Closed,
    /// The un-confirming ended (`MaxMainKorok = 3`).
    Unconfirmed,
}

/// One head group's pose (`MozgatKor(k, x, y, z, a)`): where its three sub-meshes go and their transparency
/// (`a > 0.99999`: hidden).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub at: [f32; 3],
    pub a: f32,
}

impl RaceChoice {
    /// `VisszaAllitFajvalasztasKezdoPoziciok` (exe 0x4f89b0).
    pub fn open() -> Self {
        Self {
            frame: 0.0,
            opened: false,
            theta: 2.5,
            s: 0.0,
            s_cel: 100.0,
            y: 0.0,
            last: None,
            confirmed: false,
            unconfirming: false,
            start_game: false,
            selected: None,
            level: 0,
            closing: false,
            typer: Typer::default(),
            roll: 0.0,
        }
    }

    /// One 10 ms tick (exe 0x4f65f0, the `Mode_Fajvalasztas` block in its order); `blurbs` = `fajinfo.txt`'s three.
    pub fn tick(&mut self, blurbs: &[Vec<u8>; 3]) -> Option<Event> {
        let mut ev = None;
        let mut y_cel = 0.0f32;
        if self.last.is_some() {
            self.typer.tick(self.closing);
            if !self.confirmed {
                y_cel = -50.0;
            }
        }
        if y_cel <= self.y {
            if y_cel < self.y {
                self.y = (self.y as f64 - 0.3) as f32;
                if y_cel >= self.y {
                    self.y = y_cel;
                }
            }
        } else {
            self.y = (self.y as f64 + 0.3) as f32;
            if self.y >= y_cel {
                self.y = y_cel;
            }
        }
        if self.opened {
            self.theta = (self.theta as f64 + 0.001) as f32;
            self.roll += 0.02;
            if self.s_cel < self.s {
                self.s = (self.s as f64 - 0.6) as f32;
                if self.s_cel >= self.s {
                    self.s = self.s_cel;
                    if self.closing {
                        return Some(Event::Closed);
                    }
                }
            }
            if self.s < self.s_cel {
                self.s += 1.0;
                if self.s_cel <= self.s {
                    if self.unconfirming {
                        self.unconfirming = false;
                        ev = Some(Event::Unconfirmed);
                    }
                    self.s = self.s_cel;
                    if !self.start_game {
                        self.selected = None;
                    }
                }
            }
        }
        if self.frame < 90.0 {
            self.frame = (self.frame as f64 + 0.3) as f32;
            if self.frame >= 90.0 {
                self.frame = 90.0;
                self.opened = true;
                self.last = Some(0);
                self.typer.start(&blurbs[0]);
            }
        }
        ev
    }

    /// A press on race box `n` (`AktIkon` 0x2c + n): a second press on the highlighted race confirms it
    /// (`S.Cel = 0`, `MaxMainKorok = 3`, `Cel = 1.3`); else it is highlighted and its blurb typed. `true` = confirmed.
    pub fn press_race(&mut self, n: usize, blurbs: &[Vec<u8>; 3]) -> bool {
        if self.last == Some(n) {
            self.confirmed = true;
            self.s_cel = 0.0;
            self.selected = Some(n);
            return true;
        }
        self.last = Some(n);
        self.typer.start(&blurbs[n]);
        false
    }

    /// A difficulty ring (`AktIkon` 0x2f + level): `StartGame`, the spread back to 100.
    pub fn press_level(&mut self, level: u8) {
        self.start_game = true;
        self.confirmed = false;
        self.s_cel = 100.0;
        self.level = level;
    }

    /// Esc (`MainScr_ESCPressedWhenMultiplayerselect` case 5): only at rest (`S == Cel`), opened, not starting: a confirmed
    /// race goes back to the three heads (`true`: `Cel` 6), else the choice closes (`Cel` 5).
    pub fn escape(&mut self) -> Option<bool> {
        if self.closing || self.unconfirming || !self.opened || self.start_game || self.s != self.s_cel {
            return None;
        }
        if self.confirmed {
            self.confirmed = false;
            self.unconfirming = true;
            self.s_cel = 100.0;
            Some(true)
        } else {
            self.s_cel = 0.0;
            self.closing = true;
            Some(false)
        }
    }

    /// The race boxes exist (state (a) with `(double)t > 0.999`).
    pub fn boxes_on(&self) -> bool {
        self.opened && !self.confirmed && !self.unconfirming && !self.start_game && !self.closing && self.t() as f64 > 0.999
    }

    fn t(&self) -> f32 {
        (self.s as f64 / 100.0) as f32
    }

    /// The three groups' poses this frame (`ForgatFajvalasztasDolgai` after the opening; spec §9.2's table, in its order).
    pub fn poses(&self, starts: &Starts) -> [Option<Pose>; 3] {
        let t = self.t();
        let u = 1.0 - t;
        let (td, ud) = (t as f64, u as f64);
        let pi = std::f64::consts::PI;
        std::array::from_fn(|k| {
            let th = (k as f64 / 3.0) * pi * 2.0 + self.theta as f64;
            let (s, c) = (th.sin() * 230.0, th.cos() * 230.0);
            let p = starts[k];
            let pose = |at: [f32; 3], a: f32| Some(Pose { at, a });
            if self.confirmed || self.unconfirming {
                if self.selected == Some(k) {
                    let z = ((((ud * pi).sin() * 800.0 + 700.0) * ud) + (p[2] * t) as f64) as f32;
                    pose([(s * td + (u * 0.0) as f64) as f32, (c * td + (u * 0.0) as f64) as f32, z], 0.001)
                } else {
                    let (sf, cf) = (s as f32, c as f32);
                    pose([sf * 5.0 * u + sf * t, cf * 5.0 * u + cf * t, 0.0], u)
                }
            } else if self.start_game {
                if self.selected == Some(k) {
                    pose([0.0, 0.0, u * 700.0 + t * 1400.0], t)
                } else {
                    None
                }
            } else if self.closing {
                let (sf, cf) = (s as f32, c as f32);
                let z = ((((ud * pi).sin() * 800.0 + 900.0) * ud) + (p[2] * t) as f64) as f32;
                pose([(sf as f64 * 1.5 * ud + (t * sf) as f64) as f32, (cf as f64 * 1.5 * ud + (t * cf) as f64) as f32, z], u)
            } else {
                pose([(s * td + (p[0] * u) as f64) as f32, (c * td + (u * p[1]) as f64) as f32, p[2]], 0.001)
            }
        })
    }

    /// `KiirFajInfoSzoveg` (exe 0x4f9040): the blurb's five rows centred on their used width, size 4 colour 2, each
    /// character at its alpha (closing or confirmed: by the spread), and the cursor.
    pub fn draw_blurb(&self, f: &mut Frame, d: &Draw, blink: bool) {
        if self.last.is_none() || self.start_game {
            return;
        }
        let v = &d.v;
        let (cw, ch) = (d.font.advance(4) as i32, d.font.height(4) as i32);
        let y0 = (410.0f32 - (self.typer.col as f32 / 50.0) * (ch + 2) as f32) as i32;
        let x0 = (640 - cw * COLS as i32) / 2;
        let off = |r: usize| {
            let m = self.typer.cells[r].iter().rposition(|c| c.0 != b' ').unwrap_or(0) as i32;
            ((49 - m) * cw) / 2
        };
        for r in 0..ROWS {
            let o = off(r);
            for (c, &(b, a)) in self.typer.cells[r].iter().enumerate() {
                if b == b' ' {
                    continue;
                }
                let mut br = (a * 100.0) as i32;
                if self.closing || self.confirmed {
                    br = ((self.s as f64 / 100.0) * br as f64) as i32;
                }
                d.font.text(f, v, &[b], (c as i32 * cw + o + x0) as f32, ((ch + 2) * r as i32 + y0) as f32, 1, 4, Align::Left, d.col(2, br as f32), None);
            }
        }
        if !self.closing && !self.confirmed {
            let typing = !self.typer.pending.is_empty();
            let last = if self.typer.col > 0 { self.typer.cells[ROWS - 1][self.typer.col - 1].0 } else { self.typer.cells[ROWS - 2][COLS - 1].0 };
            if (!typing && blink) || (typing && last != b' ') {
                let x = self.typer.col as i32 * cw + off(ROWS - 1) + x0;
                let (w, h) = ((cw / 2) as f32, ch as f32);
                let c = d.col(2, 100.0);
                f.push(d.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(x as f32 + w / 2.0, (y0 + ch * 4 + 8) as f32 + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
            }
        }
    }
}

/// `LoadFajInfoText` (exe 0x4f9250): the three blurbs of `text/fajinfo.txt` in the font's code page.
pub fn load_blurbs(install: &ig2_formats::Install, texts: &crate::ui::UiTexts) -> [Vec<u8>; 3] {
    let t = install.read("dataraw/text/fajinfo.txt").map(|b| ig2_formats::text::decode(&b, install.lang())).unwrap_or_default();
    let b = ig2_formats::misc_text::parse_fajinfo(&t);
    std::array::from_fn(|k| b[k].as_deref().map(|s| texts.encode(s)).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blurbs() -> [Vec<u8>; 3] {
        [b"The Solarian Empire has a turbulent history".to_vec(), b"B".to_vec(), b"C".to_vec()]
    }

    /// The opening runs 300 ticks (0.3 a tick in f64) and highlights race 0; then the spread takes 100 ticks.
    #[test]
    fn opening_and_spread() {
        let mut r = RaceChoice::open();
        let b = blurbs();
        let mut n = 0;
        while !r.opened {
            r.tick(&b);
            n += 1;
            assert!(n < 1000);
        }
        assert_eq!((n, r.frame, r.last), (300, 90.0, Some(0)));
        for _ in 0..100 {
            r.tick(&b);
        }
        assert_eq!(r.s, 100.0);
        assert!(r.boxes_on());
        // Confirm race 0, the spread closes at 0.6 a tick (167 ticks).
        assert!(!r.press_race(1, &b));
        assert!(r.press_race(1, &b));
        let mut n = 0;
        while r.s != 0.0 {
            r.tick(&b);
            n += 1;
        }
        assert_eq!(n, 167);
        assert_eq!(r.selected, Some(1));
    }

    /// One character every second tick, words wrapped at 50 columns.
    #[test]
    fn typer() {
        let mut t = Typer::default();
        t.start(b"aaaa bbbb");
        for _ in 0..8 {
            t.tick(false);
        }
        assert_eq!(t.col, 4);
        let mut t = Typer::default();
        let long = [vec![b'x'; 48], b" yyyy".to_vec()].concat();
        t.start(&long);
        for _ in 0..200 {
            t.tick(false);
        }
        // "yyyy" didn't fit after 49 columns: a new line, the space dropped.
        assert_eq!(t.col, 4);
        assert_eq!(t.cells[ROWS - 2][47].0, b'x');
        assert_eq!(&t.cells[ROWS - 1][..4].iter().map(|c| c.0).collect::<Vec<_>>(), b"yyyy");
    }
}
