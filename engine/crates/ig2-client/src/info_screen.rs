//! The info screen (`AktKep` 9, re/screens/info.md): its UI state (`Info_Belep` ELF 0x563fc0, `FRISSIT_Info2`
//! 0x5686f0), hot-spots (`Info_MouseInit` 0x5667b0) and input (`Info_Main` 0x5647f0), and the 2D passes beside
//! the chart: the empire panel (`KirakBirodalomInfo` 0x559df0), the spending limit (`PutLimitCucc` 0x5683b0), the
//! money triangle's texts (`PutHaromszog` 0x55a8b0) and the lines (`MeghuzIzeVonalak` 0x55c7c0).

use ig2_sim::save::{va, SaveState};

use crate::font::{Align, Font, Virtual};
use crate::render::{Blend, Frame, Instance, TexId};
use crate::ui::{UiSnap, UiTexts};

/// `sqrtf(2700)`: the money triangle's height is `√2700` (side 60): corners A (319, 50 + h), B (271.9, 50),
/// C (366.1, 50).
fn tri_h() -> f32 {
    2700f32.sqrt()
}

/// What the panels show, read from the sim after each tick.
#[derive(Debug, Clone, Default)]
pub struct InfoData {
    /// Per empire: the planets the human sees (`+0xd8 + 2·EN` > 0) and the fleets it sees (`+0x19 + EN` & 1).
    pub known_planets: [u32; 19],
    pub known_fleets: [u32; 19],
    /// The human's `TradeIncomeLastYear`, `TaxIncomeLastYear`, its planets' `+0x44` summed, `MoneyHatar[EN][0]`,
    /// `MoneyTeruletSzorzo[EN][1..=3]`.
    pub trade_last: i32,
    pub tax_last: i32,
    pub month_income: i32,
    pub limit: f64,
    pub shares: [f64; 3],
}

impl InfoData {
    pub fn read(state: &SaveState) -> Self {
        use ig2_sim::save::PLANET_SIZE;
        let m = &state.mem;
        let en = m.i32(va::EN).clamp(0, 18) as usize;
        let mut d = Self::default();
        for p in 1..=state.planet_count() {
            let base = va::PLANETS + (p * PLANET_SIZE) as u32;
            let owner = m.u8(base + 0x20) as usize;
            if owner < 19 && m.i16(base + 0xd8 + 2 * en as u32) > 0 {
                d.known_planets[owner] += 1;
            }
            if owner == en {
                d.month_income = d.month_income.wrapping_add(m.i32(base + 0x44));
            }
        }
        for f in state.fleets.iter().skip(1) {
            let owner = f[0x15] as usize;
            if owner < 19 && f[0x19 + en] & 1 != 0 {
                d.known_fleets[owner] += 1;
            }
        }
        d.trade_last = m.i32(va::TRADE_LAST + 4 * en as u32);
        d.tax_last = m.i32(va::TAX_LAST + 4 * en as u32);
        let t = state.treasuries()[en];
        d.limit = t.reserve[0];
        d.shares = [t.share[1], t.share[2], t.share[3]];
        d
    }
}

/// The screen's UI state.
#[derive(Debug, Clone)]
pub struct InfoScreen {
    /// `InfoSelected`: the empire the panels show (the human on entry).
    pub selected: usize,
    /// `Info_SelectedChart` 0..=9.
    pub chart: usize,
    /// `Info_StarLogMode` (else `Info_ChartMode`).
    pub star_log: bool,
    /// `infoframe` (45 open; the star log opens it on to 75 at 0.3 a frame).
    pub frame: f32,
    /// `InfoVillogtato` (+0.04 a frame).
    pub blink: f32,
    /// `infokeringeto` (+0.0006 a frame) and `InfoChartSzamlaloAdderek` (`rand() % 400 + 200` on entry).
    pub turn: f32,
    pub adders: [f32; 10],
    /// `InfoTriX`, `InfoTriY` (`Info_InitializeTriangleToMidPosition` ELF 0x55e5d0: (319, 50 + 2h/5.6)).
    pub tri: [f32; 2],
}

impl InfoScreen {
    /// `Info_Belep`: the human's empire, chart 0, chart mode. The ten sweep speeds are UI-only libc draws: a fixed
    /// MSVC `rand()` sequence here [inf: the seed is the game's].
    pub fn enter(human: usize) -> Self {
        let mut seed = 1u32;
        let adders = std::array::from_fn(|_| {
            seed = seed.wrapping_mul(214013).wrapping_add(2531011);
            ((seed >> 16) & 0x7fff) as f32 % 400.0 + 200.0
        });
        Self {
            selected: human,
            chart: 0,
            star_log: false,
            frame: crate::screen_frames::INFO_MAX_FRAMES,
            blink: 0.0,
            turn: 0.0,
            adders,
            tri: [319.0, ((50.0 + tri_h()) - tri_h() + tri_h() * 2.0 / 5.6).trunc()],
        }
    }

    /// One 10 ms UI frame (`FRISSIT_Info2`): the blink, the sweeps, the star log's opening.
    pub fn ui_frame(&mut self) {
        self.blink += 0.04;
        self.turn += 0.0006;
        let max = crate::screen_frames::INFO_MAX_FRAMES;
        if self.star_log && self.frame < max + 30.0 {
            self.frame = (self.frame + 0.3).min(max + 30.0);
        }
        if !self.star_log && max < self.frame {
            self.frame = (self.frame - 0.3).max(max);
        }
    }

    /// `InfoChartSzamlalok[k]` = `((int)(infokeringeto·adder[k]) % 55 << 1) − 27`.
    pub fn sweep(&self, k: usize) -> f32 {
        ((((self.turn * self.adders[k]) as i32) % 0x37) << 1) as f32 - 27.0
    }

    /// The frame model's view of this state.
    pub fn frame_view(&self, s: &UiSnap) -> crate::screen_frames::InfoFrame {
        crate::screen_frames::InfoFrame {
            known: std::array::from_fn(|k| s.known[k + 1]),
            races: std::array::from_fn(|k| s.races[k + 1]),
            chart: (!self.star_log).then_some(self.chart),
            blink: self.blink,
            tri: self.tri,
            frame: self.frame,
        }
    }

    /// The mouse over hot-spot `id` (`Info_Main`): 1..=8 pick the empire as soon as the mouse is over its column.
    pub fn hover(&mut self, id: Option<u32>) {
        if let Some(e @ 1..=8) = id {
            self.selected = e as usize;
        }
    }

    /// A left press on hot-spot `id` (`Info_Main`): 14 the charts, 15 the star log, 16..=25 a chart.
    pub fn press(&mut self, id: Option<u32>) {
        match id {
            Some(14) => self.star_log = false,
            Some(15) => self.star_log = true,
            Some(c @ 16..=25) => self.chart = (c - 16) as usize,
            _ => {}
        }
    }
}

/// `Info_MouseInit`'s hot-spots in id order (`Mouse_Szoveg_Set(id, 0, x, y, w, h, …)`, virtual pixels): 1..=8
/// the race columns, 9..=12 the planet/fleet rows, 13 the list, 14 charts, 15 star log, 16..=25 the chart buttons,
/// 26..=28 the tax/auto-build/priority selectors, 29 the triangle, 30/31 list up/down, 32 the spending limit, 33
/// the list's left edge, 34 its scroll bar, 35/36 star log up/down, 37 back from the detail (the list's height
/// is `min(HanyBolygoFlotta, 4)·56`; 4 rows here).
pub fn spots(name_x: &[i32]) -> Vec<(u32, [i32; 4])> {
    let h = tri_h();
    let mut v = Vec::new();
    for e in 1..=8u32 {
        v.push((e, [name_x.get(e as usize - 1).copied().unwrap_or(40) - 2, 0x30, 0x32, 0x4f]));
    }
    for k in 0..4 {
        v.push((9 + k as u32, [0x3c, 0xf9 + 0x38 * k, 200, 0x35]));
    }
    v.push((13, [0x3c, 0xf9, 200, 4 * 0x38]));
    v.push((14, [0x24c, 0x1ae, 0x2d, 0x14]));
    v.push((15, [0x15e, 0x1ae, 0x2d, 0x14]));
    for (k, y) in [0x11b, 0x136, 0x151, 0x16c, 0x187].into_iter().enumerate() {
        v.push((16 + k as u32, [0x15e, y, 0x2d, 0x14]));
    }
    for (k, y) in [0x11b, 0x136, 0x151, 0x16c, 0x187].into_iter().enumerate() {
        v.push((21 + k as u32, [0x24c, y, 0x2d, 0x14]));
    }
    v.push((26, [0x87, 0x162, 100, 0xd]));
    v.push((27, [0x87, 0x170, 100, 0xd]));
    v.push((28, [0x87, 0x17e, 100, 0xd]));
    let top = (h + 50.0) - h;
    v.push((29, [0x10a, top as i32, 0x66, (((h + 50.0) - top) + 10.0) as i32]));
    v.push((30, [0x122, 0xf7, 0x12, 0x13]));
    v.push((31, [0x122, 0x1c2, 0x12, 0x13]));
    v.push((32, [0x118, 0xd1, 100, 8]));
    v.push((33, [0, 0xf9, 0x37, 4 * 0x38]));
    v.push((34, [0x122, 0x10a, 0x12, 0xcb]));
    v.push((35, [0x26d, 0x112, 0x12, 0x13]));
    v.push((36, [0x26d, 0x1af, 0x12, 0x13]));
    v.push((37, [0x3c, 0xf9, 200, 4 * 0x38]));
    v
}

/// The first hot-spot under the virtual point `at` (`Mouse_Darabol` [inf: the lowest id wins]).
pub fn spot_at(name_x: &[i32], at: [f32; 2]) -> Option<u32> {
    spots(name_x).into_iter().find(|(_, [x, y, w, h])| at[0] >= *x as f32 && at[0] < (x + w) as f32 && at[1] >= *y as f32 && at[1] < (y + h) as f32).map(|(id, _)| id)
}

#[allow(clippy::too_many_arguments)]
fn fill(f: &mut Frame, white: TexId, v: &Virtual, x: f32, y: f32, w: f32, h: f32, c: [f32; 3]) {
    use crate::render::kind;
    let p = v.px(x + w / 2.0, y + h / 2.0);
    f.push(white, Blend::Alpha, Instance::new(kind::SPRITE, p, [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], 1.0]));
}

/// `InfoTscVonal` + `line2d`: a line from `a` to `b` with the colours blended along it (additive [inf]).
fn line(f: &mut Frame, white: TexId, v: &Virtual, a: [f32; 2], b: [f32; 2], ca: [f32; 3], cb: [f32; 3]) {
    // Two halves carry the gradient (the original's vertex colours) [inf].
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let cm = std::array::from_fn(|k| (ca[k] + cb[k]) / 2.0);
    for (p, q, c) in [(a, mid, mix(ca, cm)), (mid, b, mix(cm, cb))] {
        f.push(white, Blend::Additive, Instance::line(v.px(p[0], p[1]), v.px(q[0], q[1]), v.k.max(1.0), [c[0], c[1], c[2], 1.0]));
    }
}

fn mix(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| (a[k] + b[k]) / 2.0)
}

/// The 2D passes beside the chart (`InfoGfx2D_2`, `_3`), drawn over the frame.
#[allow(clippy::too_many_arguments)]
pub fn draw(f: &mut Frame, font: &Font, white: TexId, v: &Virtual, s: &UiSnap, d: &InfoData, texts: &UiTexts, names: &[Vec<u8>], name_x: &[i32], palette: &[[f32; 3]], ui: &InfoScreen) {
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    let label = |k: u16| {
        let mut t = texts.get(9, 3, k);
        t.push(b':');
        t
    };
    // `MeghuzIzeVonalak`: the four list rows' top and bottom lines in (34, 50, 0) [`0x00RRGGBB` 0x322200]…
    let row = [50.0 / 255.0, 34.0 / 255.0, 0.0];
    for k in 0..4 {
        let y = (k * 0x37) as f32;
        line(f, white, v, [0x1d as f32, y + 0xf9 as f32], [0x104 as f32, y + 0xf9 as f32], row, row);
        line(f, white, v, [0x1d as f32, y + 0x12d as f32], [0x104 as f32, y + 0x12d as f32], row, row);
    }
    // …and under each race column a line fading in and out of the race's colour `DDColor[e + 15]` at y 107.
    for e in 1..=8 {
        let x = name_x.get(e - 1).copied().unwrap_or(40) as f32;
        let c = col(e + 15);
        line(f, white, v, [x, 107.0], [x + 27.0, 107.0], [0.0; 3], c);
        line(f, white, v, [x + 27.0, 107.0], [x + 54.0, 107.0], c, [0.0; 3]);
    }
    // `KirakBirodalomInfo` from (180, 150), rows 15 apart, values at x 240: the name; the planets and fleets the
    // human sees; for the human also last year's trade and tax income and last month's.
    let e = ui.selected.min(18);
    let (x, y) = (0xb4 as f32, 0x96 as f32);
    font.text(f, v, &label(2), x, y, 10, 2, Align::Left, col(1), None);
    font.text(f, v, names.get(e).map_or(&[][..], |n| &n[..]), x + 60.0, y, 0x14, 2, Align::Left, col(2), None);
    let known = crate::ui::sprintf(&texts.get(9, 3, 9), &[d.known_planets[e] as i64, d.known_fleets[e] as i64]);
    font.text(f, v, &label(5), x, y + 15.0, 10, 2, Align::Left, col(1), None);
    font.text(f, v, &known, x + 60.0, y + 15.0, known.len(), 2, Align::Left, col(2), None);
    if e == s.human {
        let mut fmt = texts.get(9, 3, 6);
        fmt.extend_from_slice(b": %d - ");
        fmt.extend(texts.get(9, 3, 7));
        fmt.extend_from_slice(b", %d - ");
        fmt.extend(texts.get(9, 3, 8));
        let income = crate::ui::sprintf(&fmt, &[d.trade_last as i64, d.tax_last as i64]);
        font.text(f, v, &label(3), x, y + 30.0, 10, 2, Align::Left, col(1), None);
        font.text(f, v, &income, x + 60.0, y + 30.0, income.len(), 2, Align::Left, col(2), None);
        let mut fmt = texts.get(9, 3, 4);
        fmt.extend_from_slice(b": %d");
        let month = crate::ui::sprintf(&fmt, &[d.month_income as i64]);
        font.text(f, v, &month, x + 60.0, y + 45.0, month.len(), 2, Align::Left, col(2), None);
        // `PutLimitCucc`: "Spending limit:" at (180, 210), "$" and the limit at (386, 210), the bar of 48 ticks
        // at x 283..377 (y 210..215, the ends 2 shorter), lit (0x25) up to `ln(limit/10500 + 1)/3`.
        font.text(f, v, &label(1), 180.0, 210.0, 0x12, 2, Align::Left, col(1), None);
        // "$" + the limit (`operator+("$", CString(limit))`; the capture shows "$0").
        let mut lim = b"$".to_vec();
        lim.extend((d.limit as i32).to_string().bytes());
        font.text(f, v, &lim, 386.0, 210.0, 8, 2, Align::Left, col(2), None);
        let lit = ((((d.limit / 10500.0 + 1.0).ln() / 3.0 * 200000.0) as i32 / 2000) * 100) / 100;
        for c in (1..0x61).step_by(2) {
            let o = if c == 1 || c == 0x5f { 2 } else { 0 };
            let colour = col(if c < lit { 0x25 } else { 0xc });
            fill(f, white, v, (c + 0x11a) as f32, (0xd2 + o) as f32, 1.0, (0xd7 - 2 * o - 0xd2 + 1) as f32, colour);
        }
        // `PutHaromszog`: the triangle's corners and their income shares in per cent.
        font.text(f, v, &texts.get(9, 3, 10), 290.0, 127.0, 0x14, 1, Align::Left, col(2), None);
        font.text(f, v, &texts.get(9, 3, 11), 269.0, 40.0, 0x14, 1, Align::Left, col(2), None);
        font.text(f, v, &texts.get(9, 3, 12), 325.0, 40.0, 0x14, 1, Align::Left, col(2), None);
        let pct = |x: f64| format!("{}%", (x * 100.0) as i32).into_bytes();
        font.text(f, v, &pct(d.shares[0]), 310.0, 118.0, 5, 1, Align::Left, col(2), None);
        font.text(f, v, &pct(d.shares[1]), 243.0, 40.0, 0x14, 1, Align::Left, col(2), None);
        font.text(f, v, &pct(d.shares[2]), 380.0, 40.0, 0x14, 1, Align::Left, col(2), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_buttons_follow_info_mouse_init() {
        let x = [41, 97, 157, 214, 382, 439, 496, 552];
        assert_eq!(spot_at(&x, [360.0, 290.0]), Some(16));
        assert_eq!(spot_at(&x, [600.0, 400.0]), Some(25));
        assert_eq!(spot_at(&x, [600.0, 435.0]), Some(14));
        assert_eq!(spot_at(&x, [360.0, 435.0]), Some(15));
        assert_eq!(spot_at(&x, [160.0, 60.0]), Some(3));
        let mut s = InfoScreen::enter(6);
        s.press(Some(23));
        assert_eq!(s.chart, 7);
        s.press(Some(15));
        assert!(s.star_log);
        s.hover(Some(3));
        assert_eq!(s.selected, 3);
    }

    #[test]
    fn sweeps_wrap_like_the_exe() {
        let mut s = InfoScreen::enter(6);
        s.adders = [300.0; 10];
        s.turn = 0.5;
        // (int)(150) % 55 = 40 → 80 − 27.
        assert_eq!(s.sweep(0), 53.0);
    }
}
