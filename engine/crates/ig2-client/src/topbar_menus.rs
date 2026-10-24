//! The top bar's two drop-downs (re/screens/hud.md §2.1): the screen menu under the arrow (hot-spot 6) and the
//! quick-research menu under the lightning (hot-spot 8). Input `MOUSE_SelectScreens` (ELF 0x66ba60, exe 0x555470),
//! texts `GFX_SelectScreens` (ELF 0x66ace0, exe 0x554d90), boxes `GFX_DateMoneyScrollFade` (ELF 0x66d160, exe
//! 0x554500, fader pass 2). Neither plays a sound.
use ig2_sim::save::{va, SaveState};

use crate::font::{Align, Font, Virtual};
use crate::render::{Frame, TexId};

/// Hot-spot 6 (`Mouse_Init` exe 0x555df0: `(0x26b, 0, 21, 20)`), the screen menu's arrow.
pub const SCREENS_SPOT: [i32; 4] = [0x26b, 0, 21, 20];
/// Hot-spot 8 (`(0x256, 0, 21, 20)`), the quick-research lightning.
pub const RESEARCH_SPOT: [i32; 4] = [0x256, 0, 21, 20];
/// `HanyTaskbarElem` (exe 0x9197b0, static).
pub const TASKBAR: i32 = 13;
/// `Taskbar_Key_Names` (exe 0x9197b4, static literals; 11 and 12 are hidden).
const KEY_NAMES: [&[u8]; 14] = [b"", b"F1-", b"F2-", b"F3-", b"F4-", b"F5-", b"F6-", b"F7-", b"F8-", b"F9-", b"F10", b"   ", b"   ", b"o -"];
/// `FastResearchHanyadikSor` (exe 0x9198bc, static): a category's (`(t − 1)/16`) column.
const COLUMN: [usize; 12] = [0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2];
/// `SelectSz`: the research menu's columns.
const COLUMNS: i32 = 3;
/// The menus' box colours (`FadeKerettelKerekit(.., 0, 10, 0, 0x3c, 0, 5, 0, 0x1e, ..)`).
const FILL: ([f32; 3], f32) = ([0.0, 10.0, 0.0], 60.0);
const EDGE: ([f32; 3], f32) = ([0.0, 5.0, 0.0], 30.0);

fn inside(r: [i32; 4], at: [i32; 2]) -> bool {
    at[0] >= r[0] && at[0] < r[0] + r[2] && at[1] >= r[1] && at[1] < r[1] + r[3]
}

/// The top-bar hot-spot under `at` (6 or 8; `Mouse_Darabol`), none of them else.
pub fn spot(at: [i32; 2]) -> Option<u32> {
    if inside(SCREENS_SPOT, at) {
        Some(6)
    } else if inside(RESEARCH_SPOT, at) {
        Some(8)
    } else {
        None
    }
}

/// One status-3 tech of the research menu.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Item {
    pub tech: i32,
    pub col: usize,
    pub row: usize,
    pub name: Vec<u8>,
    /// Some lab level short of the need (and no cheat).
    pub short: bool,
    /// The cost line's amount for this tech (`GetString(0x12, 3, 4)`), shown when it is `Selected` and idle.
    pub cost: i32,
}

/// What the menus show, read from the game state each frame they are open.
#[derive(Debug, Clone, Default)]
pub struct Data {
    /// `AktKep` and `KepernyokLetiltva[0..20]`.
    pub screen: i32,
    pub disabled: [bool; 20],
    /// `Taskbar_Names[1..=13]` (`GetString(0x12, 6, i)`).
    pub names: Vec<Vec<u8>>,
    /// The research menu: the items, the most rows in one column (`SelectMa`), the headers.
    pub items: Vec<Item>,
    pub rows: i32,
    pub headers: Vec<Vec<u8>>,
    /// The running project (0 none), its name, percent and money left.
    pub current: i32,
    pub current_name: Vec<u8>,
    pub pct: i32,
    pub left: i32,
    /// `GetString(0x12, 3, 1..=4)`: Researching, STOP, Money left, Cost.
    pub footer: [Vec<u8>; 4],
}

/// Tech `t`'s `(Ido·t − prog)·(cost·Money)/(Ido·t)` in f32, doubled when urgent (`GFX_SelectScreens`, exe
/// 0x555236): the research screen's daily cost formula.
fn money_left(base_time: i32, time_mult: f32, prog: i32, cost: i32, money_mult: f32, urgent: bool) -> i32 {
    let total = base_time as f32 * time_mult;
    let n = ((total - prog as f32) * (cost as f32 * money_mult) / total) as i32;
    if urgent {
        n << 1
    } else {
        n
    }
}

/// The running project's percent: the quotient in f32, the half added in f64 (exe 0x555078).
fn percent(prog: i32, base_time: i32, time_mult: f32) -> i32 {
    (((prog * 100) as f32 / (base_time as f32 * time_mult)) as f64 + 0.5) as i32
}

/// The research menu's layout (`MOUSE_SelectScreens`' first pass): the status-3 techs in column order of
/// `FastResearchHanyadikSor`, each column's rows counted from 0; and `SelectMa`, the longest column.
pub fn layout(researchable: impl Fn(usize) -> bool) -> (Vec<(i32, usize, usize)>, i32) {
    let mut out = Vec::new();
    let mut rows = [0usize; 3];
    for t in 1..=0xc0usize {
        if researchable(t) {
            let c = COLUMN[(t - 1) / 16];
            out.push((t as i32, c, rows[c]));
            rows[c] += 1;
        }
    }
    (out, rows.into_iter().max().unwrap_or(0) as i32)
}

impl Data {
    /// Read the menus' data for the human (`EN`) from `state`; `screen` = `AktKep`, `disabled` = the tutorial's
    /// `KepernyokLetiltva`, `battle` = a real-time space battle runs (`SpaceWar_Belep` sets them all).
    pub fn read(state: &SaveState, rules: &ig2_rules::Rules, texts: &crate::ui::UiTexts, screen: i32, tutorial: Option<&[bool; 20]>, battle: bool) -> Self {
        let m = &state.mem;
        let en = crate::research::en(state);
        let mut disabled = tutorial.copied().unwrap_or([false; 20]);
        // `Fake_FRISSITES` (exe 0x4de720), every frame.
        if !disabled[2] {
            disabled[6] = m.i32(MAX_PLAYEREK) == 0;
        }
        if m.u8(PLAYER_FAJ + en as u32) == 5 {
            disabled[6..=8].fill(true);
        }
        disabled[6] |= m.u8(va::DIPLOMACY_ENABLED) == 0;
        disabled[7] |= m.u8(va::SPY_ENABLED) == 0;
        disabled[8] |= m.u8(TRADE_ENABLED) == 0;
        if battle {
            disabled = [true; 20];
        }
        let techs = state.tech();
        let labs = crate::research::labs(state, en);
        let cheat = m.i32(va::RESEARCH_CHEAT) != 0;
        let (time_mult, money_mult) = crate::research::race_mults(state, rules, en);
        let urgent = m.u8(va::URGENT + en as u32) & 1 != 0;
        let current = m.i32(va::RESEARCHING + 4 * en as u32);
        let short = |t: usize| !cheat && techs.labs_needed(t).iter().enumerate().any(|(i, &n)| labs[i + 1] < n as i32);
        let cost = |t: usize| money_left(techs.base_research_time(t), time_mult, techs.research_time(t, en), techs.research_cost(t), money_mult, urgent);
        let (cells, rows) = layout(|t| techs.status(t, en) == 3);
        let items = cells.into_iter().map(|(t, col, row)| {
            let u = t as usize;
            Item { tech: t, col, row, name: crate::hud::tech_name(texts, u), short: short(u), cost: cost(u) }
        }).collect();
        let c = current.clamp(0, 0xc0) as usize;
        Data {
            screen,
            disabled,
            names: (0..=TASKBAR as u16).map(|i| if i == 0 { Vec::new() } else { texts.get(0x12, 6, i) }).collect(),
            items,
            rows,
            headers: (1..=3).map(|c| texts.get(0x11, 3, 0x26 + c)).collect(),
            current,
            current_name: crate::hud::tech_name(texts, c),
            pct: if c > 0 { percent(techs.research_time(c, en), techs.base_research_time(c), time_mult) } else { 0 },
            left: if c > 0 { cost(c) } else { 0 },
            footer: [1, 2, 3, 4].map(|k| texts.get(0x12, 3, k)),
        }
    }

    fn item(&self, t: i32) -> Option<&Item> {
        self.items.iter().find(|i| i.tech == t)
    }

    /// `Selected` can start: idle, status 3 and its labs enough (`ok` of `GFX_DateMoneyScrollFade`).
    fn startable(&self, t: i32) -> bool {
        self.current < 1 && self.item(t).is_some_and(|i| !i.short)
    }
}

/// `MaxPlayerek` (exe 0xAB64DC, saved): the human's contacts; `KigyujtPlayerek` (exe 0x4c6610) returns it.
const MAX_PLAYEREK: u32 = 0xAB64DC;
/// `PlayerFaj` (exe 0xBAE574).
const PLAYER_FAJ: u32 = 0xBAE574;
/// `TradeEngedelyezve` (exe 0xBAE3AE).
const TRADE_ENABLED: u32 = 0xBAE3AE;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Open {
    #[default]
    None,
    /// `ScreensSelectScreenMode` (exe 0xe40c5c).
    Screens,
    /// `ScreensSelectFastResearchMode` (exe 0xe40c5d).
    Research,
}

/// What a press or release asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// Not the menus' input: it goes on to the screen.
    Pass,
    /// Taken, nothing else happens.
    Taken,
    /// `AktKepKell = n`.
    Screen(i32),
    /// `ResearchAtvaltTalalmanyra(t)` and `Multi_Send(0x2a, t)`.
    Start(i32),
    /// `Multi_Send(0x29, t)`.
    Stop(i32),
}

/// The menus' state (`SelectX/Y` exe 0x912ad4/0x912f38, `SelectMa` 0x912f40, `Selected` 0xbaf330).
#[derive(Debug, Clone, Default)]
pub struct TopMenus {
    pub open: Open,
    pub x: i32,
    pub y: i32,
    pub rows: i32,
    pub selected: i32,
    pub mouse: [i32; 2],
}

impl TopMenus {
    /// The screen menu's rows: screens 1..=13 without the battles (11, 12).
    fn screen_rows() -> impl Iterator<Item = (i32, i32)> {
        (1..=TASKBAR).filter(|&i| i != 11 && i != 12).enumerate().map(|(r, i)| (r as i32, i))
    }

    /// The STOP button (`[SelectX + 0x66·Sz − 0x3f, +0x37] × [SelectY + M − 0x20, +0x19]`, M = `16·Ma + 0x30`).
    fn stop_rect(&self) -> [i32; 4] {
        let m = self.rows * 16 + 0x30;
        [self.x + COLUMNS * 0x66 - 0x3f, self.y + m - 0x20, 0x38, 0x1a]
    }

    /// Every held frame: `Selected` under the pointer (and the research menu's layout again).
    fn track(&mut self, d: &Data) {
        let at = self.mouse;
        self.selected = -1;
        match self.open {
            Open::Screens => {
                for (r, i) in Self::screen_rows() {
                    if inside([self.x, self.y + 16 * r, 0x7f, 16], at) {
                        self.selected = i;
                    }
                }
            }
            Open::Research => {
                self.rows = d.rows;
                self.x = 0x140;
                self.y = if self.rows * 16 + 0x16 > 0x1e0 { 0x1de - self.rows * 16 } else { 0x14 };
                for it in &d.items {
                    let x = 0x140 + it.col as i32 * 0x66;
                    let y = self.y + it.row as i32 * 16 + 0xe;
                    if inside([x, y, 0x61, 16], at) {
                        self.selected = it.tech;
                    }
                }
            }
            Open::None => {}
        }
    }

    /// The pointer moved to virtual `at`.
    pub fn moved(&mut self, at: [i32; 2], d: &Data) {
        self.mouse = at;
        if self.open != Open::None {
            self.track(d);
        }
    }

    /// A button went down at `at`; `left` = the left button is down (this press or held, `Mouse_Bal`).
    pub fn press(&mut self, at: [i32; 2], left: bool, d: &Data) -> Pick {
        self.mouse = at;
        if self.open != Open::None {
            self.track(d);
            return Pick::Taken;
        }
        match spot(at) {
            // A press without the left button (a right one) asks for the starmap.
            Some(6) if !left => {
                if d.screen != 2 && !d.disabled[2] {
                    Pick::Screen(2)
                } else {
                    Pick::Taken
                }
            }
            Some(6) => {
                (self.x, self.y) = (at[0] - 0x81, 0x14);
                self.open = Open::Screens;
                self.track(d);
                Pick::Taken
            }
            Some(8) => {
                // It opens only with something to list.
                if d.rows > 0 {
                    self.open = Open::Research;
                    self.track(d);
                }
                Pick::Taken
            }
            _ => Pick::Pass,
        }
    }

    /// A button went up at `at` (`Mouse_Felenged`): the pick, and the menu closes.
    pub fn release(&mut self, at: [i32; 2], d: &Data) -> Pick {
        self.mouse = at;
        let open = std::mem::take(&mut self.open);
        if open == Open::None {
            return Pick::Pass;
        }
        // `Selected` stays the last held frame's.
        let s = self.selected;
        match open {
            Open::Screens if s > 0 && s != d.screen && !d.disabled[s as usize] => Pick::Screen(s),
            Open::Research if s > 0 && d.startable(s) => Pick::Start(s),
            Open::Research if s < 1 && d.current > 0 && inside(self.stop_rect(), at) => Pick::Stop(d.current),
            _ => Pick::Taken,
        }
    }

    /// The boxes (`GFX_DateMoneyScrollFade`'s fader pass 2) and the texts (`GFX_SelectScreens`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, f: &mut Frame, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], d: &Data) {
        let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
        let bx = |f: &mut Frame, r: [i32; 4]| crate::kit::fader::fader(f, white, v, r.map(|c| c as f32), FILL.0, FILL.1, EDGE.0, EDGE.1);
        let (x, y) = (self.x, self.y);
        let (xf, yf) = (x as f32, y as f32);
        match self.open {
            Open::None => {}
            Open::Screens => {
                let n = Self::screen_rows().count() as i32;
                bx(f, [x - 6, y - 2, 0x88, n * 16 + 4]);
                for (r, i) in Self::screen_rows() {
                    let mut c = if i == self.selected { 6 } else { 0x21 };
                    if d.disabled[i as usize] {
                        c = 1;
                    }
                    let ry = yf + (16 * r) as f32;
                    font.text(f, v, KEY_NAMES[i as usize], xf, ry + 6.0, 0xd, 1, Align::Left, col(c), None);
                    let name = d.names.get(i as usize).map_or(&[][..], |n| &n[..]);
                    font.text(f, v, name, xf + 19.0, ry + 3.0, 0xd, 3, Align::Left, col(c), None);
                }
            }
            Open::Research => {
                let ma = self.rows;
                let w = COLUMNS * 0x66;
                let ok = self.selected > 0 && d.startable(self.selected);
                let h = ma * 16 + if d.current > 0 { 0x4e } else if ok { 0x30 } else { 0x12 };
                bx(f, [x - 6, y - 2, w + 3, h]);
                if d.current > 0 {
                    bx(f, [x - 1, y + h - 0x3e, w - 0x43, 0x19]);
                    bx(f, [x + w - 0x3f, y + h - 0x3e, 0x37, 0x19]);
                    bx(f, [x - 1, y + h - 0x20, w - 0x43, 0x19]);
                } else if ok {
                    bx(f, [x - 1, y + h - 0x20, w - 7, 0x19]);
                }
                for (c, hd) in d.headers.iter().enumerate() {
                    font.text(f, v, hd, xf + (c as i32 * 0x66) as f32, yf + 4.0, 0x10, 2, Align::Field, col(6), None);
                }
                for it in &d.items {
                    let mut c = if it.tech == self.selected { 6 } else { 3 };
                    if d.current > 0 {
                        c = 0x21;
                    }
                    if it.short {
                        c = 1;
                    }
                    font.text(f, v, &it.name, xf + (it.col as i32 * 0x66) as f32, yf + (it.row as i32 * 16 + 0x11) as f32, 0x10, 2, Align::Left, col(c), None);
                }
                let m = (ma * 16 + 0x30) as f32;
                if d.current > 0 {
                    let mut line = d.footer[0].clone();
                    line.extend_from_slice(b":  ");
                    line.extend(d.current_name.iter().take_while(|&&b| b != 0));
                    line.extend_from_slice(format!("  {}%", d.pct).as_bytes());
                    font.text(f, v, &line, xf + 9.0, yf + m - 23.0, 0x28, 2, Align::Left, col(3), None);
                    let on = inside(self.stop_rect(), self.mouse);
                    font.text(f, v, &d.footer[1], (x + w - 0x22) as f32, yf + m - 23.0, 10, 2, Align::Centre, col(if on { 6 } else { 3 }), None);
                    let mut left = d.footer[2].clone();
                    left.extend_from_slice(format!("   {}", d.left).as_bytes());
                    font.text(f, v, &left, xf + 9.0, yf + m + 7.0, 0, 2, Align::Left, col(3), None);
                } else if ok {
                    let mut cost = d.footer[3].clone();
                    cost.extend_from_slice(format!(":   {}", d.item(self.selected).map_or(0, |i| i.cost)).as_bytes());
                    font.text(f, v, &cost, xf + 9.0, yf + m - 23.0, 0, 2, Align::Left, col(3), None);
                }
            }
        }
    }
}

/// `--top-menu`: `screens:X` (the arrow pressed at x X), `research` or `research+T` (tech T started first), held open
/// with the pointer at `mouse`, on the starmap (for the capture comparison).
pub fn stage(hud: &mut crate::ui::Hud, world: &mut crate::world::World, rules: &ig2_rules::Rules, spec: &str, mouse: [f32; 2]) -> Result<(), String> {
    let (kind, arg) = spec.split_once([':', '+']).unwrap_or((spec, ""));
    if let ("research", Ok(t)) = (kind, arg.parse::<i32>()) {
        world.send(ig2_net::Command::Research { order: ig2_net::ResearchCmd::Start { tech: t } });
        world.pump();
    }
    let press = match kind {
        "screens" => [arg.parse::<i32>().map_err(|e| format!("--top-menu screens:X: {e}"))?, 10],
        "research" => [608, 10],
        _ => return Err(format!("--top-menu {spec}: screens:X, research or research+T")),
    };
    let d = Data::read(&world.game.state, rules, &hud.texts, 2, None, false);
    hud.top.press(press, true, &d);
    hud.top.moved(mouse.map(|c| c as i32), &d);
    hud.top_data = d;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(items: &[(i32, bool)], current: i32) -> Data {
        let (cells, rows) = layout(|t| items.iter().any(|&(i, _)| i as usize == t));
        let items = cells.into_iter().map(|(t, col, row)| Item { tech: t, col, row, short: items.iter().any(|&(i, s)| i == t && s), ..Default::default() }).collect();
        Data { screen: 2, items, rows, current, ..Default::default() }
    }

    #[test]
    fn hot_spots_are_the_mouse_init_boxes() {
        assert_eq!(spot([619, 0]), Some(6));
        assert_eq!(spot([639, 19]), Some(6));
        assert_eq!(spot([618, 5]), Some(8));
        assert_eq!(spot([598, 5]), Some(8));
        assert_eq!(spot([597, 5]), None);
        assert_eq!(spot([620, 20]), None);
    }

    #[test]
    fn columns_follow_the_category_table() {
        let (cells, rows) = layout(|t| matches!(t, 1 | 17 | 80 | 81 | 144 | 145 | 192));
        assert_eq!(cells, vec![(1, 0, 0), (17, 0, 1), (80, 0, 2), (81, 1, 0), (144, 1, 1), (145, 2, 0), (192, 2, 1)]);
        assert_eq!(rows, 3);
    }

    #[test]
    fn the_screen_menu_opens_under_the_pointer_and_picks_on_release() {
        let d = data(&[], 0);
        let mut m = TopMenus::default();
        assert_eq!(m.press([630, 10], true, &d), Pick::Taken);
        assert_eq!((m.open, m.x, m.y), (Open::Screens, 630 - 0x81, 0x14));
        // Row 3 (screen 4, research) at y 20 + 48.
        m.moved([520, 0x14 + 3 * 16 + 5], &d);
        assert_eq!(m.selected, 4);
        // Rows past 10 skip the battles: row 10 is options (13).
        m.moved([520, 0x14 + 10 * 16 + 1], &d);
        assert_eq!(m.selected, 13);
        m.moved([520, 0x14 + 3 * 16 + 5], &d);
        assert_eq!(m.release([520, 0x14 + 3 * 16 + 5], &d), Pick::Screen(4));
        assert_eq!(m.open, Open::None);
    }

    #[test]
    fn disabled_and_current_screens_are_not_picked() {
        let mut d = data(&[], 0);
        d.disabled[6] = true;
        let mut m = TopMenus::default();
        m.press([630, 10], true, &d);
        m.moved([520, 0x14 + 5 * 16 + 2], &d);
        assert_eq!(m.release([520, 0x14 + 5 * 16 + 2], &d), Pick::Taken);
        m.press([630, 10], true, &d);
        m.moved([520, 0x14 + 16 + 2], &d);
        assert_eq!(m.release([520, 0x14 + 16 + 2], &d), Pick::Taken, "the starmap is up");
        // A right press asks for the starmap, unless it is up.
        d.screen = 4;
        assert_eq!(m.press([630, 10], false, &d), Pick::Screen(2));
    }

    #[test]
    fn quick_research_starts_a_startable_tech_and_stops_the_running_one() {
        let d = data(&[(3, false), (4, true), (90, false)], 0);
        let mut m = TopMenus::default();
        assert_eq!(m.press([605, 10], true, &d), Pick::Taken);
        assert_eq!((m.open, m.x, m.y, m.rows), (Open::Research, 0x140, 0x14, 2));
        // Tech 4's row: y 20 + 16 + 14 .. +16, x 0x140..=0x1a0.
        m.moved([0x150, 0x14 + 16 + 0xe], &d);
        assert_eq!(m.release([0x150, 0x14 + 16 + 0xe], &d), Pick::Taken, "labs short");
        m.press([605, 10], true, &d);
        m.moved([0x140 + 0x66, 0x14 + 0xe], &d);
        assert_eq!(m.release([0x140 + 0x66, 0x14 + 0xe], &d), Pick::Start(90));
        let d = data(&[(3, false)], 7);
        m.press([605, 10], true, &d);
        let r = m.stop_rect();
        assert_eq!(r, [0x140 + 0x132 - 0x3f, 0x14 + 16 + 0x10, 0x38, 0x1a]);
        assert_eq!(m.release([r[0], r[1]], &d), Pick::Stop(7));
        m.press([605, 10], true, &d);
        m.moved([0x140, 0x14 + 0xe], &d);
        assert_eq!(m.release([0x140, 0x14 + 0xe], &d), Pick::Taken, "nothing starts while researching");
    }

    #[test]
    fn an_empty_research_menu_does_not_open() {
        let d = data(&[], 0);
        let mut m = TopMenus::default();
        assert_eq!(m.press([605, 10], true, &d), Pick::Taken);
        assert_eq!(m.open, Open::None);
    }

    #[test]
    fn a_tall_list_moves_up() {
        let all: Vec<_> = (1..=80).map(|t| (t, false)).collect();
        let d = data(&all, 0);
        let mut m = TopMenus::default();
        m.press([605, 10], true, &d);
        assert_eq!((m.rows, m.y), (80, 0x1de - 80 * 16));
    }

    #[test]
    fn money_and_percent_round_as_the_exe() {
        assert_eq!(money_left(100, 1.0, 25, 1000, 1.0, false), 750);
        assert_eq!(money_left(100, 1.0, 25, 1000, 1.0, true), 1500);
        assert_eq!(percent(1, 3, 1.0), 33);
        assert_eq!(percent(1, 2, 1.0), 50);
    }
}
