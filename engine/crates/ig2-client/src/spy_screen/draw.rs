//! The spy screen's 2D pass: `SpyGfx2D_1` (ELF 0x6bb6e0), the faders `GFX_Spy` (ELF 0x6c6080, exe 0x586aa0)
//! queues, `SpyGfx2D_2` (ELF 0x6bb7c0, exe 0x588bd0) and `SpyGfx2D_3` (ELF 0x6c3a10, exe 0x58c1d0); re/screens/spy.md
//! §3–§5.

use super::{applicant_cost, cat, cstr, get, List, Popup, Report, SpyData, SpyScreen, SpyTexts};
use crate::font::{Align, Font, Virtual};
use crate::render::{Frame, TexId};
use crate::spy_scene::Spot;
use ig2_sim::spy::Spy;

/// What the 2D pass draws with.
pub struct Draw2d<'a> {
    pub font: &'a Font,
    pub white: TexId,
    pub v: Virtual,
    pub palette: &'a [[f32; 3]],
    pub ui: &'a crate::ui::UiTexts,
    pub t: &'a SpyTexts,
    /// The game's language is Hungarian (`Lang == 4` [inf]): dialogs 0x12/0x13 name the race + "i".
    pub hungarian: bool,
}

impl Draw2d<'_> {
    /// `GetString(7, section, index)`.
    fn s(&self, section: u16, index: u16) -> Vec<u8> {
        self.ui.get(7, section, index)
    }
    /// `KiirStr(text, x, y, len, size, flags, colour)` with `KiirStrSzinSzorzo` = `mult`/100 (flag 8 clips to the
    /// virtual screen).
    #[allow(clippy::too_many_arguments)]
    fn text(&self, f: &mut Frame, s: &[u8], x: i32, y: i32, len: usize, size: usize, flags: u32, col: usize, mult: f32) {
        let a = match flags & 3 {
            0 => Align::Left,
            1 => Align::Field,
            2 => Align::Right,
            _ => Align::Centre,
        };
        let c = self.palette.get(col).copied().unwrap_or([1.0; 3]).map(|x| x * mult / 100.0);
        let clip = (flags & 8 != 0).then(|| (-self.v.x0 / self.v.k, 640.0 + self.v.x0 / self.v.k));
        self.font.text(f, &self.v, s, x as f32, y as f32, len, size, a, c, clip);
    }
    fn t2(&self, f: &mut Frame, s: &[u8], x: i32, y: i32, len: usize, col: usize) {
        self.text(f, s, x, y, len, 2, 4, col, 100.0);
    }
    /// `FadeKerettelKerekit(x, y, w, h, fill, t, edge, t')`.
    fn fader(&self, f: &mut Frame, r: [i32; 4], fill: [f32; 3], ft: f32, edge: [f32; 3], et: f32) {
        crate::starmap_panels::fader(f, self.white, &self.v, r.map(|x| x as f32), fill, ft, edge, et);
    }
    /// A button's box (`0, 10, 30, 100, 0, 10, 50, 100`).
    fn button(&self, f: &mut Frame, r: [i32; 4]) {
        self.fader(f, r, [0.0, 10.0, 30.0], 100.0, [0.0, 10.0, 50.0], 100.0);
    }
    /// A pick list's box (`0, 10, 0, 60, 0, 5, 0, 30`).
    fn list_box(&self, f: &mut Frame, r: [i32; 4]) {
        self.fader(f, r, [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
    }
}

/// `sprintf` of one `%d`/`%s`.
fn fmt(s: &[u8], v: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut done = false;
    while i < s.len() {
        if !done && s[i] == b'%' && i + 1 < s.len() && matches!(s[i + 1], b'd' | b's') {
            out.extend_from_slice(v);
            i += 2;
            done = true;
        } else {
            out.push(s[i]);
            i += 1;
        }
    }
    out
}

/// Report rows start at x 470 (0x1d6).
const X: i32 = 0x1d6;

impl SpyScreen {
    /// The 2D pass in the original's order: `SpyGfx2D_1` (the panels' frames, the names on the pedestals), the
    /// faders `GFX_Spy` queued (buttons; the pick list's box), `SpyGfx2D_2` (the texts), `SpyGfx2D_3` (the pick
    /// list's rows).
    pub fn draw(&self, f: &mut Frame, d: &SpyData, g: &Draw2d, spots: &[Spot; 12]) {
        // `SpyGfx2D_1` queues `FadeKeret(6, 248, 172, 217)` and `(464, 248, …)` in (0, 45, 90) at 128 past frame 35,
        // but no capture shows them [obs: the panels' edges are the fill's (2, 20, 26)]: not drawn.
        if self.frame < 50.0 {
            return;
        }
        self.names(f, d, g, spots);
        self.faders(f, d, g);
        if let Some(p) = self.popup {
            self.popup_box(f, d, g, &p);
        }
        self.texts(f, d, g);
        if let Some(p) = self.popup {
            self.popup_rows(f, d, g, &p);
        }
    }

    /// `KemekFeliratai` (ELF 0x6b95d0): each pedestal's spy name (or "---"), size 1, centred, faded with its place
    /// on the ring and `spyKiirSzinSzorzo`; the selected one blinks with `KemNevVillogtato`.
    fn names(&self, f: &mut Frame, d: &SpyData, g: &Draw2d, spots: &[Spot; 12]) {
        let peds = Self::pedestals(d);
        let sel = self.selected_pedestal(d);
        for (i, s) in spots.iter().enumerate() {
            if s.transparency >= 0.8 {
                continue;
            }
            let mut mult = ((1.0 - s.transparency) * 100.0 * self.text_fade / 100.0).trunc();
            let [x, y] = [s.plate[0] as i32, s.plate[1] as i32];
            match peds[i] {
                // `p2 − 40` with the text centred in its 14 cells (flags 0xd & 3 = 1): centred on the pedestal.
                None => g.text(f, b"---", x - 40, y - 2, 14, 1, 0xd, 2, mult),
                Some(k) => {
                    if sel == Some(i) {
                        mult = (mult as f64 * (self.blink as f64 / 100.0)) as i32 as f32;
                    }
                    g.text(f, cstr(&d.spy(k).name), x - 40, y - 2, 14, 1, 0x4d, 2, mult);
                }
            }
        }
    }

    /// `GFX_Spy`'s button faders.
    fn faders(&self, f: &mut Frame, d: &SpyData, g: &Draw2d) {
        if self.dialog > 0 {
            let ys: &[i32] = match self.dialog {
                2 => &[298, 332, 366],
                3 => &[298, 332, 366, 400],
                _ => &[],
            };
            for &y in ys {
                g.button(f, [0x1db, y, 0x96, 0x19]);
            }
            if !matches!(self.dialog, 2 | 3) {
                if self.repeatable(d) {
                    g.button(f, [0x1db, 0x1a8, 0x3c, 0x19]);
                }
                g.button(f, [0x235, 0x1a8, 0x3c, 0x19]);
            }
            return;
        }
        let s = d.spy(self.akt);
        let p = self.plan;
        let rule = d.mission(p.mission);
        if p.on && p.mission > 0 {
            g.button(f, [0x1db, 0x1a8, 0x3c, 0x19]);
            if p.race > 0 || !rule.needs_target {
                g.button(f, [0x235, 0x1a8, 0x3c, 0x19]);
            }
        }
        if !p.on {
            if s.state == 3 {
                g.button(f, [0x1db, 0x1a8, 0x3c, 0x19]);
            }
            return;
        }
        g.button(f, [0x1d6, 0x10d, 0xa2, 0xf]);
        if p.mission < 1 {
            if s.state > 0 && s.state != 3 && s.state != 4 {
                g.button(f, [0x1db, 0x1a8, 0x3c, 0x19]);
            }
            return;
        }
        if rule.needs_target {
            g.button(f, [0x1d6, 0x12f, 0xa2, 0xf]);
        }
        if (p.race > 0 || p.mission == 1) && rule.kind > 0 {
            g.button(f, [0x20a, 0x140, 0x6e, 0xf]);
        }
        if (p.race > 0 || p.mission == 1) && d.second_kind(p.mission) > 0 {
            g.button(f, [0x20a, 0x151, 0x6e, 0xf]);
        }
    }

    fn popup_box(&self, f: &mut Frame, d: &SpyData, g: &Draw2d, p: &Popup) {
        let (x, y) = (p.x, p.y);
        match p.list {
            List::Mission => g.list_box(f, [x - 5, y - 2, 0x8e, p.rows * 13 + 4]),
            List::Race => g.list_box(f, [x - 6, y - 2, 0x9a, d.targets().len().max(1) as i32 * 13 + 4]),
            List::Target { .. } => g.list_box(f, [x - 6, y - 2, p.cols * 0x72 + 3, p.rows * 13 + 4]),
            List::Kill => g.list_box(f, [x - 6, y - 2, p.chars * 6 + 10, 3 * 13 + 4]),
        }
    }

    /// `SpyGfx2D_3` (ELF 0x6c3a10): the open list's rows; the one under the pointer in 6, what costs more than the
    /// treasury in 0xc (0x25 under the pointer).
    fn popup_rows(&self, f: &mut Frame, d: &SpyData, g: &Draw2d, p: &Popup) {
        let (x, y) = (p.x, p.y);
        let money_col = |sel: bool, poor: bool| match (sel, poor) {
            (true, true) => 0x25,
            (false, true) => 0xc,
            (true, false) => 6,
            _ => 3,
        };
        match p.list {
            List::Mission => {
                for (n, (m, text, _)) in self.rows(d, g.t, p.list).into_iter().enumerate() {
                    let col = money_col(m == p.selected, d.money < d.cost(m, 1) as f64);
                    g.t2(f, &text, x, y + n as i32 * 13 + 3, 0x16, col);
                }
            }
            List::Race => {
                let rows = self.rows(d, g.t, p.list);
                for (n, (e, text, _)) in rows.iter().enumerate() {
                    let col = if d.races[*e as usize] == 5 { 1 } else if *e == p.selected { 6 } else { 3 };
                    g.t2(f, text, x, y + n as i32 * 13 + 3, 0x18, col);
                }
                if rows.is_empty() {
                    g.t2(f, &g.s(5, 20), x, y + 3, 0x18, 3);
                }
            }
            List::Target { mode, .. } => {
                let ma = p.rows.max(1);
                for (n, (v, text, _)) in self.rows(d, g.t, p.list).into_iter().enumerate() {
                    let n = n as i32;
                    let (cx, cy) = (x + (n / ma) * 0x72, y + (n % ma) * 13 + 3);
                    if mode == 5 {
                        let col = money_col(v == p.selected, d.money < d.cost(self.plan.mission, v) as f64);
                        g.t2(f, &text, cx, cy, 0x13, col);
                    } else {
                        let col = if v + 1 == p.selected { 6 } else { 3 };
                        let text = if v == 0 { g.s(5, 7) } else { text };
                        g.t2(f, &text, cx, cy, 0x12, col);
                    }
                }
            }
            List::Kill => {
                g.text(f, get(&g.t.sure, 0), x, y + 3, p.chars as usize, 2, 5, 2, 100.0);
                for i in 1..3 {
                    let col = if i == p.selected { 6 } else { 1 };
                    g.text(f, get(&g.t.sure, i as i64), x, y + i * 13 + 3, p.chars as usize, 2, 5, col, 100.0);
                }
            }
        }
    }

    /// `SpyGfx2D_2`: the dialog or the planner on the right, the spy on the left, the roster count at the bottom.
    fn texts(&self, f: &mut Frame, d: &SpyData, g: &Draw2d) {
        if self.dialog > 0 {
            if let Some(r) = d.report() {
                self.dialog_texts(f, d, g, &r);
            }
        }
        if self.akt > 0 {
            let s = d.spy(self.akt);
            let (y0, dy) = (0x100, 0x10);
            g.t2(f, &g.s(5, 1), 0xc, y0, 7, 1);
            g.t2(f, cstr(&s.name), 0x3b, y0, 0xe, 2);
            g.t2(f, &g.s(5, 2), 0xc, y0 + dy, 7, 1);
            g.t2(f, get(&g.t.races, s.race as i64), 0x3b, y0 + dy, 0x10, 2);
            g.t2(f, &g.s(5, 3), 0xc, y0 + 2 * dy, 7, 1);
            g.t2(f, s.xp.to_string().as_bytes(), 0x3b, y0 + 2 * dy, 6, 2);
            g.t2(f, &g.s(5, 4), 0xc, y0 + 3 * dy, 7, 1);
            g.t2(f, get(&g.t.ranks, s.rank as i64), 0x3b, y0 + 3 * dy, 0xe, 2);
            if self.dialog == 0 && !self.plan.on {
                if s.state == 3 {
                    self.mission_texts(f, d, g, &s);
                } else {
                    g.t2(f, &g.s(5, 10), X, 0x100, 7, 1);
                    g.t2(f, get(&g.t.statuses, s.state as i64), 0x212, 0x100, 0x16, 2);
                }
            }
            let (y0, dy) = (0x142, 0x10);
            let plus = s.points > 0 && matches!(s.state, 1..=3);
            for i in 0..4 {
                let y = y0 + dy * i as i32;
                g.t2(f, &g.s(5, 11 + i as u16), 0xc, y, 0xe, 1);
                g.text(f, s.skills[i].to_string().as_bytes(), 0x65, y, 2, 2, 6, 2, 100.0);
                if plus {
                    g.text(f, b"+", 0x70, y - 2, 3, 3, 5, 1, 100.0);
                }
            }
            if plus {
                g.t2(f, b"Skill Points:", 0xc, y0 + 5 * dy, 0xe, 1);
                g.text(f, s.points.to_string().as_bytes(), 0x65, y0 + 5 * dy, 2, 2, 6, 2, 100.0);
            }
        }
        let count = cat(&[&g.s(5, 15), b" ", d.working.to_string().as_bytes(), b"  ", &g.s(5, 16), b" ", d.capacity.to_string().as_bytes()]);
        g.text(f, &count, 0xc, 0x1bf, 0x1b, 3, 4, 1, 100.0);
        if self.dialog == 0 {
            if !self.plan.on {
                if d.spy(self.akt).state == 3 {
                    g.text(f, &g.s(5, 28), 0x1fb, 0x1b1, 10, 2, 7, 2, 100.0);
                }
            } else {
                self.plan_texts(f, d, g);
            }
        }
    }

    /// A spy on a mission (state 3): the mission, its target and parameters, the days left.
    fn mission_texts(&self, f: &mut Frame, d: &SpyData, g: &Draw2d, s: &Spy) {
        g.t2(f, &g.s(5, 5), X, 0x100, 0x1b, 1);
        g.t2(f, get(&g.t.missions, s.mission as i64), 0x1de, 0x111, 0x1a, 2);
        let rule = d.mission(s.mission as i32);
        if rule.needs_target {
            g.t2(f, &g.s(5, 6), X, 0x122, 0x1b, 1);
            g.t2(f, get(&g.t.empires, s.target as i64), 0x1de, 0x133, 0x1a, 2);
        }
        for (kind, value, y) in [(rule.kind, s.param1, 0x144), (d.second_kind(s.mission as i32), s.param2, 0x155)] {
            if kind > 0 {
                g.t2(f, get(&g.t.target_kinds, kind as i64), X, y, 9, 1);
                if value < 1 {
                    g.t2(f, &g.s(5, 7), 0x212, y, 0x12, 2);
                } else {
                    self.param_value(f, d, g, kind, value as i32, y);
                }
            }
        }
        if !rule.no_countdown {
            let label = g.s(5, 8);
            g.t2(f, &label, X, 0x166, 0x16, 1);
            let days = cat(&[((s.left + 49) / 50).to_string().as_bytes(), b" ", &g.s(5, 9)]);
            g.t2(f, &days, label.len() as i32 * 6 + X, 0x166, 0xf, 2);
        }
    }

    /// A parameter's value: the planet, tech, hull's tech or training level.
    fn param_value(&self, f: &mut Frame, d: &SpyData, g: &Draw2d, kind: i32, v: i32, y: i32) {
        let text = match kind {
            1 => d.planet_name(v),
            2 | 4 => d.tech_name(v),
            3 => d.tech_name(d.tables.hulls.iter().find(|h| h.index == v as usize).map_or(0, |h| h.tech)),
            5 => get(&g.t.training, v as i64).to_vec(),
            _ => Vec::new(),
        };
        g.t2(f, &text, 0x212, y, if kind == 5 { 0x13 } else { 0x12 }, 2);
    }

    /// The planner (`SpyNewMissionMode`): mission, target empire, parameters, days, chance, cost and buttons.
    fn plan_texts(&self, f: &mut Frame, d: &SpyData, g: &Draw2d) {
        let p = self.plan;
        g.t2(f, &g.s(5, 17), X, 0x100, 0x1b, 1);
        if p.mission < 1 {
            g.t2(f, &g.s(5, 18), 0x1de, 0x111, 0x1a, 2);
            let s = d.spy(self.akt);
            if s.state > 0 && s.state != 3 && s.state != 4 {
                g.text(f, &g.s(5, 27), 0x1fb, 0x1b1, 10, 2, 7, 2, 100.0);
            }
            return;
        }
        g.t2(f, get(&g.t.missions, p.mission as i64), 0x1de, 0x111, 0x1a, 2);
        let rule = d.mission(p.mission);
        if rule.needs_target {
            g.t2(f, &g.s(5, 19), X, 0x122, 0x1b, 1);
            let t = if p.race < 1 { g.s(5, 18) } else { get(&g.t.empires, p.race as i64).to_vec() };
            g.t2(f, &t, 0x1de, 0x133, 0x1a, 2);
        }
        if (p.race > 0 || p.mission == 1) && rule.kind > 0 {
            g.t2(f, get(&g.t.target_kinds, rule.kind as i64), X, 0x144, 9, 1);
            match p.target {
                v if v < 0 => g.t2(f, &g.s(5, 18), 0x212, 0x144, 0x10, 2),
                0 => g.t2(f, &g.s(5, 7), 0x212, 0x144, 0x10, 2),
                v => self.param_value(f, d, g, rule.kind, v, 0x144),
            }
        }
        let k2 = d.second_kind(p.mission);
        if p.race > 0 && k2 > 0 {
            g.t2(f, get(&g.t.target_kinds, k2 as i64), X, 0x155, 9, 1);
            match p.target2 {
                v if v < 0 => g.t2(f, &g.s(5, 18), 0x212, 0x155, 0x10, 2),
                0 => g.t2(f, &g.s(5, 7), 0x212, 0x155, 0x10, 2),
                v => g.t2(f, &d.tech_name(v), 0x212, 0x155, 0x10, 2),
            }
        }
        if !rule.no_countdown {
            g.t2(f, &g.s(5, 21), X, 0x177, 0x16, 1);
            if p.days < 1 {
                g.t2(f, b"---", 0x230, 0x177, 3, 2);
            } else {
                g.t2(f, &cat(&[p.days.to_string().as_bytes(), b" ", &g.s(5, 9)]), 0x230, 0x177, 0xf, 2);
            }
            g.t2(f, &g.s(5, 22), X, 0x188, 0xc, 1);
            if rule.story {
                g.t2(f, b"???", 0x230, 0x188, 3, 2);
            } else if p.chance1 * p.chance2 < 100 {
                g.t2(f, b"---", 0x230, 0x188, 3, 2);
            } else {
                g.t2(f, &cat(&[(p.chance1 * p.chance2 / 100).to_string().as_bytes(), b"%"]), 0x230, 0x188, 4, 2);
            }
        }
        g.t2(f, &g.s(5, 23), X, 0x199, 0x16, 1);
        if p.cost < 1 {
            g.t2(f, b"---", 0x230, 0x199, 5, 2);
        } else {
            g.t2(f, &cat(&[b"$", p.cost.to_string().as_bytes()]), 0x230, 0x199, 7, 2);
        }
        g.text(f, &g.s(5, 24), 0x1fb, 0x1b1, 0x14, 2, 7, 2, 100.0);
        if p.race > 0 || !rule.needs_target {
            let ok = if d.money < p.cost as f64 { g.s(5, 26) } else { g.s(5, 25) };
            g.text(f, &ok, 0x255, 0x1b1, 9, 2, 7, 2, 100.0);
        }
    }

    /// The dialog of the first report (`SpyBeszolasMi`).
    fn dialog_texts(&self, f: &mut Frame, d: &SpyData, g: &Draw2d, r: &Report) {
        let line = |f: &mut Frame, s: &[u8], y: i32, col: usize| g.text(f, s, X, y, 0x1c, 2, 4, col, 100.0);
        let empire = |e: i32| get(&g.t.empires, e as i64).to_vec();
        let (y0, dy) = (0x100, 0x11);
        match self.dialog {
            1 | 7 => self.report_texts(f, d, g, r),
            2 => {
                line(f, &g.s(3, 20), y0, 7);
                line(f, &g.s(3, 21), y0 + dy, 7);
                let cost = applicant_cost(d, r.a.max(0) as usize);
                line(f, &cat(&[&g.s(3, 22), cost.to_string().as_bytes()]), y0 + 2 * dy, 7);
            }
            3..=6 => {
                let (k, col) = match self.dialog {
                    3 => (26, 7),
                    4 => (28, 7),
                    5 => (30, 6),
                    _ => (32, 6),
                };
                line(f, &g.s(3, k), y0, col);
                line(f, &fmt(&g.s(3, k + 1), &empire(r.x[1])), y0 + dy, 7);
            }
            8..=0x13 => {
                let two = |f: &mut Frame, a: u16, b: Option<Vec<u8>>, col: usize| {
                    line(f, &g.s(4, a), y0, col);
                    if let Some(b) = b {
                        line(f, &b, y0 + dy, 7);
                    }
                };
                let race_or_empire = |e: i32| {
                    if g.hungarian {
                        cat(&[get(&g.t.races, d.races.get(e.max(0) as usize).copied().unwrap_or(0) as i64), b"i"])
                    } else {
                        empire(e)
                    }
                };
                match self.dialog {
                    8 => two(f, 2, None, 7),
                    9 => two(f, 3, Some(g.s(4, 4)), 7),
                    10 => two(f, 5, None, 6),
                    0xb => two(f, 6, Some(g.s(4, 7)), 7),
                    0xc => {
                        two(f, 8, Some(g.s(4, 9)), 7);
                        line(f, get(&g.t.ranks, r.x[2] as i64), y0 + 2 * dy, 2);
                    }
                    0xd => two(f, 10, Some(g.s(4, 11)), 7),
                    0xe => two(f, 12, None, 7),
                    0xf => two(f, 13, Some(fmt(&g.s(4, 14), &empire(r.x[1]))), 7),
                    0x10 => two(f, 15, Some(fmt(&g.s(4, 16), &empire(r.x[1]))), 7),
                    0x11 => two(f, 17, Some(fmt(&g.s(4, 18), &empire(r.x[1]))), 7),
                    0x12 => two(f, 19, Some(fmt(&g.s(4, 20), &race_or_empire(r.x[1]))), 7),
                    _ => {
                        two(f, 21, Some(g.s(4, 22)), 7);
                        line(f, &fmt(&g.s(4, 23), &race_or_empire(r.x[1])), y0 + 2 * dy, 7);
                    }
                }
            }
            _ => {}
        }
        // The buttons' captions.
        match self.dialog {
            2 => {
                let cost = applicant_cost(d, r.a.max(0) as usize);
                let (k, len) = if d.working >= d.capacity {
                    (24, 0x19)
                } else if (cost as f64) <= d.money || cost < 1 {
                    (26, 0x18)
                } else {
                    (25, 0x18)
                };
                g.text(f, &g.s(4, k), 0x226, 0x133, len, 2, 7, 2, 100.0);
                g.text(f, &g.s(4, 27), 0x226, 0x155, 0x18, 2, 7, 2, 100.0);
                g.text(f, &g.s(4, 28), 0x226, 0x177, 0x18, 2, 7, 2, 100.0);
            }
            3 => {
                for (i, y) in [0x133, 0x155, 0x177, 0x199].into_iter().enumerate() {
                    g.text(f, &g.s(4, 29 + i as u16), 0x226, y, 0x18, 2, 7, 2, 100.0);
                }
            }
            _ => {
                if self.repeatable(d) {
                    g.text(f, &g.s(4, 33), 0x1fb, 0x1b1, 7, 2, 7, 2, 100.0);
                }
                g.text(f, &g.s(4, 34), 0x255, 0x1b1, 7, 2, 7, 2, 100.0);
            }
        }
    }

    /// A mission report (dialog 1) or a double agent's (7): the outcome, the mission, the target and the result.
    fn report_texts(&self, f: &mut Frame, d: &SpyData, g: &Draw2d, r: &Report) {
        let line = |f: &mut Frame, s: &[u8], y: i32, col: usize| g.text(f, s, X, y, 0x1c, 2, 4, col, 100.0);
        let (y0, dy) = (0x100, 0x11);
        let (m, c) = (r.mission(), r.c != 0);
        if self.dialog == 7 {
            let (a, b, col) = if r.b == 4 && !c { (g.s(2, 3), g.s(2, 4), 6) } else { (g.s(2, 5), g.s(2, 6), 7) };
            line(f, &a, y0, col);
            line(f, &b, y0 + dy, col);
        } else {
            let o = if r.b == 4 && !c { 5 } else { r.b as i64 };
            g.text(f, get(&g.t.outcomes, o), X, y0, 0x1f, 2, 4, 6, 100.0);
        }
        line(f, &g.s(2, 7), 0x122, 1);
        line(f, get(&g.t.missions, m as i64), 0x133, 2);
        if d.mission(m).needs_target {
            line(f, &g.s(2, 8), 0x144, 1);
            line(f, get(&g.t.empires, r.x[1] as i64), 0x155, 2);
        }
        let [_, _, p1, _, e0, e4, e8, ec, code] = r.x;
        if !c && code == 0x3e5 {
            line(f, &g.s(2, 9), 0x166, 1);
            line(f, &g.s(2, 10), 0x177, 6);
        }
        let reason = |f: &mut Frame, a: (u16, u16), b: (u16, u16), when: i32| {
            if !c && code == when {
                line(f, &g.s(a.0, a.1), 0x166, 1);
                line(f, &g.s(b.0, b.1), 0x177, 6);
            }
        };
        match m {
            1 if c => line(f, &g.s(2, 11), 0x166, 2),
            2 if c => {
                for (k, v) in [e0, e4, e8, ec].into_iter().enumerate() {
                    line(f, &fmt(&g.s(2, 12 + k as u16), v.to_string().as_bytes()), 0x166 + 0x11 * k as i32, 2);
                }
            }
            3 if !c => reason(f, (2, 22), (2, 23), 999),
            3 if p1 == 0 => {
                if e0 > 0 {
                    line(f, &fmt(&g.s(2, 16), &d.planet_name(e0)), 0x166, 2);
                }
                if e4 > 0 {
                    line(f, &fmt(&g.s(2, 17), &d.planet_name(e4)), 0x177, 2);
                }
            }
            3 => {
                line(f, &fmt(&g.s(2, 18), &d.planet_name(e0)), 0x166, 2);
                line(f, &fmt(&g.s(2, 19), d.planet_i32(e0, 0x28).to_string().as_bytes()), 0x177, 2);
                let morale = get(&g.t.morale, (d.planet_i32(e0, 0x30) / 100) as i64).to_vec();
                line(f, &fmt(&g.s(2, 20), &morale), 0x188, 2);
                line(f, &fmt(&g.s(2, 21), d.planet_i32(e0, 0x58).to_string().as_bytes()), 0x199, 2);
            }
            4 if !c => reason(f, (2, 25), (2, 26), 999),
            4 => {
                if e0 > 0 {
                    line(f, &g.s(2, 24), 0x166, 1);
                    line(f, &d.tech_name(e0), 0x177, 2);
                }
                for (v, y) in [(e4, 0x188), (e8, 0x199)] {
                    if v > 0 {
                        line(f, &d.tech_name(v), y, 2);
                    }
                }
            }
            5 if c => {
                line(f, &g.s(2, 27), 0x166, 1);
                line(f, e0.to_string().as_bytes(), 0x177, 2);
            }
            6 if !c => reason(f, (3, 1), (3, 2), 999),
            6 if e0 > 0 && e4 > 0 => {
                line(f, &g.s(2, 28), 0x166, 1);
                line(f, &d.planet_name(e0), 0x177, 2);
                line(f, &g.s(2, 29), 0x188, 1);
                let tech = d.rules.side.colony.buildings.get(e8.max(0) as usize).map_or(0, |b| b.tech as i32);
                line(f, &d.tech_name(tech), 0x199, 2);
            }
            7 if !c => reason(f, (3, 4), (3, 5), 999),
            7 => {
                line(f, &g.s(3, 3), 0x166, 1);
                line(f, &d.tech_name(e4), 0x177, 2);
            }
            8 if !c => reason(f, (3, 7), (3, 8), 999),
            8 => {
                line(f, &g.s(3, 6), 0x166, 1);
                line(f, &d.tech_name(p1), 0x177, 2);
            }
            9 if !c => reason(f, (3, 10), (3, 11), 999),
            9 => {
                line(f, &g.s(3, 9), 0x166, 1);
                line(f, &d.planet_name(e0), 0x177, 2);
                line(f, get(&g.t.rebellion, e4 as i64), 0x188, 2);
            }
            10 if !c => {
                reason(f, (3, 12), (3, 13), 0x3e6);
                reason(f, (3, 14), (3, 15), 999);
            }
            10 => line(f, &fmt(&g.s(3, 23), e0.to_string().as_bytes()), 0x166, 2),
            11 if !c => reason(f, (3, 16), (3, 17), 999),
            11 => {
                let a = g.s(3, 24);
                let b = fmt(&g.s(3, 25), get(&g.t.family, e0 as i64));
                if a.len() + b.len() < 0x1c {
                    g.text(f, &cat(&[&a, b" ", &b]), X, 0x166, 0x1b, 2, 4, 2, 100.0);
                } else {
                    g.text(f, &a, X, 0x166, 0x1b, 2, 4, 2, 100.0);
                    g.text(f, &b, 0x1e2, 0x177, 0x1b, 2, 4, 2, 100.0);
                }
            }
            12 if c => {
                line(f, &g.s(3, 18), 0x177, 2);
                line(f, &g.s(3, 19), 0x188, 2);
            }
            _ => {}
        }
    }
}
