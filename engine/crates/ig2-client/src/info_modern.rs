//! Ours (ADR 0072): the modern info screen on the egui layer (ADR 0062), the "Modern info screen" option. The same
//! functions as the original's (re/screens/info.md): the empires met (a click shows one, the original's pointer over
//! its column), their planets and fleets as one list (the original's order or sorted), a row's detail with the planet
//! settings (0x19 8/9/10), the spending limit (0x5c) and the money triangle (0x5b), the ten charts and, in a scenario
//! game, the swap to the star log. The window fills a [`View`] from the state the classic panels read, runs [`show`]
//! once a frame and hands the [`Act`]s to the classic `InfoScreen` code (`press`, `hover`, `orders`) at the hot-spot
//! the original's player would have used, so every order is the original's.
//!
//! The look is the modern starmap HUD's console (ADR 0065): dark plates with cut corners and the empire's colour.

use egui::{pos2, vec2, Align, Color32, Layout, Margin, Pos2, Rect, RichText, Sense, Shape, Stroke};

use crate::info_screen::{Entry, Put};
use crate::starmap_modern::{self as sm, plate, tip, Ink, EDGE};
use crate::ui::{Hud, UiSnap};

/// A button's part of a drag: pressed (left or right), held, let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    Left,
    Right,
    Drag,
    Release,
}

/// What the panels ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// The top bar's (the starmap HUD's acts).
    Bar(sm::Act),
    /// Show empire `e` (the race columns 1..=8).
    Empire(usize),
    /// A chart picked (16..=25), the star log or the charts (15, 14).
    Chart(usize),
    StarLog(bool),
    /// The list's entry `i` (`BolygoFlottaTomb[i]`) opened in detail (9..=12), and back (37).
    Open(usize),
    Back,
    /// A planet setting (hot-spot 26..=28) set to an item.
    Setting(u32, i32),
    /// The money triangle (29) at a virtual point, the spending limit (32) at a virtual x.
    Triangle([f32; 2], Press),
    Limit(f32, Press),
}

/// An empire met: its column's number, name, the relation's colour, its own colour and what the human sees of it.
#[derive(Debug, Clone, Default)]
pub struct EmpireRow {
    pub e: usize,
    pub name: String,
    pub colour: Color32,
    pub race: Color32,
    pub planets: u32,
    pub fleets: u32,
}

/// A list row: its entry, the name and lines (`KirakBolygoInfo`/`KirakFlottaInfo`'s), and the sort keys (−1 where the
/// human doesn't see the value).
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub index: usize,
    pub title: (String, Color32),
    pub lines: Vec<(String, Color32)>,
    pub size: i64,
    pub alert: i64,
}

/// A planet setting's menu: its hot-spot (26 tax, 27 auto-build, 28 priority), items and the current one.
#[derive(Debug, Clone, Default)]
pub struct Setting {
    pub hot: u32,
    pub items: Vec<(i32, String)>,
    pub current: i32,
}

/// A detail row: the label, the values in their colours, a setting's menu.
#[derive(Debug, Clone, Default)]
pub struct DetailRow {
    pub label: String,
    pub values: Vec<(String, Color32)>,
    pub setting: Option<Setting>,
}

/// The human's money: the spending limit (label, text, the bar's part) and the triangle (corner labels, shares, the
/// knob at its virtual point, the shares blinking after a drag).
#[derive(Debug, Clone, Default)]
pub struct Money {
    pub limit: (String, String, f32),
    pub corners: [String; 3],
    pub shares: [String; 3],
    pub knob: [f32; 2],
    pub blink: f32,
}

/// The remake's and the original's labels.
#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub empires: String,
    pub planets: String,
    pub fleets: String,
    pub name: String,
    pub order: String,
    pub size: String,
    pub alert: String,
    pub back: String,
    pub row_tip: String,
    pub charts: String,
    pub star_log: String,
}

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    pub empires: Vec<EmpireRow>,
    pub selected: usize,
    pub summary: Vec<(String, String)>,
    pub money: Option<Money>,
    pub rows: Vec<Row>,
    pub detail: Option<Vec<DetailRow>>,
    /// The ten charts' names, the one shown, each known empire's line (cells 4..=50).
    pub charts: Vec<String>,
    pub chart: usize,
    pub series: Vec<(Color32, Vec<f32>)>,
    pub star_log: bool,
    pub log: Vec<(String, Color32)>,
    /// The swap between the charts and the star log is on (a scenario game, hot-spots 14/15).
    pub swap: bool,
    pub labels: Labels,
}

/// The list's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// The original's: the planets, then the fleets, by number.
    #[default]
    Order,
    Name,
    /// Population or ships.
    Size,
    /// The worst problem or the fleet's power.
    Alert,
}

/// What stays between frames.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub bar: sm::State,
    pub sort: Sort,
    pub descending: bool,
    /// The empires' order: 0 their number, 1 the name, 2 planets, 3 fleets.
    pub empire_sort: u8,
    tri: bool,
    limit: bool,
}

/// The list's rows in `sort` order (stable: ties keep the original's order).
pub fn sorted(rows: &[Row], sort: Sort, descending: bool) -> Vec<usize> {
    let mut at: Vec<usize> = (0..rows.len()).collect();
    let dir = |o: std::cmp::Ordering| if descending { o.reverse() } else { o };
    match sort {
        Sort::Order if descending => at.reverse(),
        Sort::Order => {}
        Sort::Name => at.sort_by(|&a, &b| dir(rows[a].title.0.to_lowercase().cmp(&rows[b].title.0.to_lowercase()))),
        Sort::Size => at.sort_by(|&a, &b| dir(rows[a].size.cmp(&rows[b].size))),
        Sort::Alert => at.sort_by(|&a, &b| dir(rows[a].alert.cmp(&rows[b].alert))),
    }
    at
}

/// The empires in the chosen order (0 by number, 1 name, 2 planets, 3 fleets; the counts most first).
pub fn empire_order(rows: &[EmpireRow], by: u8) -> Vec<usize> {
    let mut at: Vec<usize> = (0..rows.len()).collect();
    match by {
        1 => at.sort_by_cached_key(|&i| rows[i].name.to_lowercase()),
        2 => at.sort_by_key(|&i| std::cmp::Reverse(rows[i].planets)),
        3 => at.sort_by_key(|&i| std::cmp::Reverse(rows[i].fleets)),
        _ => {}
    }
    at
}

/// The money triangle's corners on the virtual screen (re/screens/info.md §7.2): A (319, 50 + h) at the bottom, B
/// (271.9, 50) and C (366.1, 50), h = √2700.
const TRI_X: [f32; 2] = [271.9, 366.1];
fn tri_h() -> f32 {
    2700f32.sqrt()
}

/// A point of the virtual triangle's box to points in `r`, and back.
fn tri_to(r: Rect, v: [f32; 2]) -> Pos2 {
    pos2(r.left() + (v[0] - TRI_X[0]) / (TRI_X[1] - TRI_X[0]) * r.width(), r.top() + (v[1] - 50.0) / tri_h() * r.height())
}

pub fn tri_from(r: Rect, p: Pos2) -> [f32; 2] {
    [TRI_X[0] + (p.x - r.left()) / r.width() * (TRI_X[1] - TRI_X[0]), 50.0 + (p.y - r.top()) / r.height() * tri_h()]
}

/// The spending limit's bar (virtual x 280..=380, `Info_Main` hot-spot 32) from a point across `r`.
pub fn limit_from(r: Rect, x: f32) -> f32 {
    280.0 + ((x - r.left()) / r.width()).clamp(0.0, 1.0) * 100.0
}

/// A press, hold or release of `resp`'s area as one drag: the original's hot-spots 29 and 32 hold the button.
fn grab(ui: &egui::Ui, resp: &egui::Response, held: &mut bool) -> Option<Press> {
    let (left, right, down, up) = ui.input(|i| (i.pointer.primary_pressed(), i.pointer.secondary_pressed(), i.pointer.any_down(), i.pointer.any_released()));
    match () {
        _ if resp.contains_pointer() && (left || right) => {
            *held = true;
            Some(if left { Press::Left } else { Press::Right })
        }
        _ if *held && up => {
            *held = false;
            Some(Press::Release)
        }
        _ if *held && down => Some(Press::Drag),
        _ => None,
    }
}

const LEFT_W: f32 = 300.0;
const LIST_W: f32 = 340.0;
const GAP: f32 = 8.0;

pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut bar = Vec::new();
    let ink = sm::bar(ctx, &v.bar, &mut st.bar, &mut bar);
    let mut acts: Vec<Act> = bar.into_iter().map(Act::Bar).collect();
    let top = ctx.memory(|m| m.area_rect(egui::Id::new("smh-left"))).map_or(46.0, |r| r.bottom() + 6.0);
    // The game's picture behind, dimmed; the panel takes every press the plates don't.
    egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_black_alpha(150)).inner_margin(Margin::same(EDGE as i8))).show(ctx, |ui| {
        ui.add_space((top - EDGE).max(0.0));
        let h = ui.available_height();
        let w = ui.available_width();
        let left = LEFT_W.min(w * 0.3);
        let list = LIST_W.min(w * 0.34);
        let chart = (w - left - list - 2.0 * GAP).max(160.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            column(ui, left, h, &ink, |ui| empires(ui, v, st, &ink, &mut acts));
            column(ui, list, h, &ink, |ui| match &v.detail {
                Some(d) => detail(ui, v, d, &ink, &mut acts),
                None => list_panel(ui, v, st, &ink, &mut acts),
            });
            column(ui, chart, h, &ink, |ui| charts(ui, v, &ink, &mut acts));
        });
    });
    acts
}

/// A full-height plate `w` wide.
fn column(ui: &mut egui::Ui, w: f32, h: f32, ink: &Ink, add: impl FnOnce(&mut egui::Ui)) {
    let m = Margin::symmetric(10, 8);
    ui.allocate_ui_with_layout(vec2(w, h), Layout::top_down(Align::Min), |ui| {
        plate(ui, ink, m, |ui| {
            ui.set_width(w - m.sum().x);
            ui.set_height(h - m.sum().y);
            add(ui);
        });
    });
}

fn heading(ui: &mut egui::Ui, ink: &Ink, t: &str) {
    ui.label(RichText::new(t).heading().color(ink.accent));
    hairline(ui, ink);
}

fn hairline(ui: &mut egui::Ui, ink: &Ink) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 5.0), Sense::hover());
    ui.painter().line_segment([pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)], Stroke::new(1.0f32, ink.edge));
}

/// A sort header's button: the arrow on the chosen one.
fn sort_button(ui: &mut egui::Ui, label: &str, on: bool, descending: bool) -> bool {
    let t = if on { format!("{label}    ") } else { label.to_string() };
    let resp = ui.selectable_label(on, RichText::new(t).small());
    if on {
        let c = pos2(resp.rect.right() - 9.0, resp.rect.center().y);
        let (tip, a, b) = if descending { (c + vec2(0.0, 3.0), c + vec2(-3.5, -2.5), c + vec2(3.5, -2.5)) } else { (c - vec2(0.0, 3.0), c + vec2(-3.5, 2.5), c + vec2(3.5, 2.5)) };
        ui.painter().add(Shape::convex_polygon(vec![tip, a, b], ui.visuals().strong_text_color(), Stroke::NONE));
    }
    resp.clicked()
}

fn empires(ui: &mut egui::Ui, v: &View, st: &mut State, ink: &Ink, acts: &mut Vec<Act>) {
    heading(ui, ink, &v.labels.empires);
    ui.horizontal(|ui| {
        for (k, l) in [(1u8, &v.labels.name), (2, &v.labels.planets), (3, &v.labels.fleets)] {
            if sort_button(ui, l, st.empire_sort == k, true) {
                st.empire_sort = if st.empire_sort == k { 0 } else { k };
            }
        }
    });
    for i in empire_order(&v.empires, st.empire_sort) {
        let e = &v.empires[i];
        let on = e.e == v.selected;
        let resp = ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(4.0, 16.0), Sense::hover());
            ui.painter().rect_filled(r, 1.0, e.race);
            let name = RichText::new(&e.name).color(if on { Color32::WHITE } else { e.colour });
            let resp = ui.selectable_label(on, if on { name.strong() } else { name });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(format!("{} / {}", e.planets, e.fleets)).small().color(ink.dim)));
            resp
        });
        if resp.inner.clicked() && !on {
            acts.push(Act::Empire(e.e));
        }
    }
    ui.add_space(6.0);
    hairline(ui, ink);
    egui::Grid::new("imh-summary").num_columns(2).spacing(vec2(8.0, 2.0)).show(ui, |ui| {
        for (l, val) in &v.summary {
            ui.label(RichText::new(l).color(ink.dim));
            ui.label(RichText::new(val).color(ink.text));
            ui.end_row();
        }
    });
    if let Some(m) = &v.money {
        ui.add_space(6.0);
        money(ui, m, st, ink, acts);
    }
}

fn money(ui: &mut egui::Ui, m: &Money, st: &mut State, ink: &Ink, acts: &mut Vec<Act>) {
    let (label, text, part) = &m.limit;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(ink.dim));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(text).color(ink.text)));
    });
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::click_and_drag());
    let p = ui.painter();
    p.rect_filled(r.shrink2(vec2(0.0, 4.0)), 2.0, Color32::from_black_alpha(120));
    p.rect_filled(Rect::from_min_size(r.min + vec2(0.0, 4.0), vec2(r.width() * part, r.height() - 8.0)), 2.0, ink.accent);
    p.circle_filled(pos2(r.left() + r.width() * part, r.center().y), 5.0, if resp.hovered() || st.limit { Color32::WHITE } else { ink.text });
    if let Some(press) = grab(ui, &resp, &mut st.limit) {
        let x = ui.input(|i| i.pointer.latest_pos()).map_or(r.left(), |p| p.x);
        acts.push(Act::Limit(limit_from(r, x), press));
    }
    ui.add_space(8.0);
    // The triangle: B and C on top with their labels above, A at the bottom with its label below.
    let w = ui.available_width().min(190.0);
    let (outer, resp) = ui.allocate_exact_size(vec2(ui.available_width(), w * tri_h() / (TRI_X[1] - TRI_X[0]) + 56.0), Sense::click_and_drag());
    let r = Rect::from_center_size(outer.center(), vec2(w, w * tri_h() / (TRI_X[1] - TRI_X[0])));
    let p = ui.painter();
    let [a, b, c] = [[319.0, 50.0 + tri_h()], [TRI_X[0], 50.0], [TRI_X[1], 50.0]].map(|v| tri_to(r, v));
    p.add(Shape::convex_polygon(vec![b, c, a], ink.accent.gamma_multiply(0.14), Stroke::new(1.2f32, ink.accent)));
    let k = tri_to(r, m.knob);
    for corner in [a, b, c] {
        p.line_segment([k, corner], Stroke::new(1.0f32, ink.edge));
    }
    p.circle_filled(k, 5.0, if resp.hovered() || st.tri { Color32::WHITE } else { ink.text });
    let share = ink.text.gamma_multiply(m.blink);
    let font = egui::TextStyle::Small.resolve(ui.style());
    for (i, (at, align)) in [(a + vec2(0.0, 6.0), egui::Align2::CENTER_TOP), (b - vec2(0.0, 6.0), egui::Align2::LEFT_BOTTOM), (c - vec2(0.0, 6.0), egui::Align2::RIGHT_BOTTOM)].into_iter().enumerate() {
        let t = format!("{} {}", m.corners[i], m.shares[i]);
        p.text(at, align, t, font.clone(), share);
    }
    if let Some(press) = grab(ui, &resp, &mut st.tri) {
        let at = ui.input(|i| i.pointer.latest_pos()).unwrap_or(k);
        acts.push(Act::Triangle(tri_from(r, at), press));
    }
}

fn list_panel(ui: &mut egui::Ui, v: &View, st: &mut State, ink: &Ink, acts: &mut Vec<Act>) {
    let name = v.empires.iter().find(|e| e.e == v.selected).map_or("", |e| &e.name);
    heading(ui, ink, &format!("{} — {} / {}", name, v.labels.planets, v.labels.fleets));
    ui.horizontal(|ui| {
        for (s, l) in [(Sort::Order, &v.labels.order), (Sort::Name, &v.labels.name), (Sort::Size, &v.labels.size), (Sort::Alert, &v.labels.alert)] {
            if sort_button(ui, l, st.sort == s, st.descending) {
                st.descending = st.sort == s && !st.descending;
                st.sort = s;
            }
        }
    });
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        for i in sorted(&v.rows, st.sort, st.descending) {
            row(ui, &v.rows[i], &v.labels.row_tip, ink, acts);
        }
    });
}

fn row(ui: &mut egui::Ui, r: &Row, tip_text: &str, ink: &Ink, acts: &mut Vec<Act>) {
    let under = ui.painter().add(Shape::Noop);
    let w = ui.available_width();
    let f = egui::Frame::new().inner_margin(Margin::symmetric(6, 3)).show(ui, |ui| {
        ui.set_width(w - 12.0);
        ui.label(RichText::new(&r.title.0).color(r.title.1).strong());
        for (t, c) in &r.lines {
            ui.label(RichText::new(t).small().color(*c));
        }
    });
    let resp = ui.interact(f.response.rect, ui.id().with(("imh-row", r.index)), Sense::click());
    let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let rect = f.response.rect;
    ui.painter().set(under, Shape::Vec(vec![
        Shape::rect_filled(rect, 2.0, Color32::from_white_alpha((6.0 + 16.0 * hot) as u8)),
        Shape::line_segment([pos2(rect.left(), rect.top() + 2.0), pos2(rect.left(), rect.bottom() - 2.0)], Stroke::new(2.0f32, r.title.1.gamma_multiply(0.4 + 0.6 * hot))),
    ]));
    if tip(resp, tip_text, "", ink).clicked() {
        acts.push(Act::Open(r.index));
    }
    ui.add_space(2.0);
}

fn detail(ui: &mut egui::Ui, v: &View, rows: &[DetailRow], ink: &Ink, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        if ui.button(&v.labels.back).clicked() {
            acts.push(Act::Back);
        }
    });
    hairline(ui, ink);
    egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| {
        egui::Grid::new("imh-detail").num_columns(2).spacing(vec2(10.0, 3.0)).show(ui, |ui| {
            for r in rows {
                ui.label(RichText::new(&r.label).color(ink.dim));
                match &r.setting {
                    Some(s) => {
                        let cur = s.items.iter().find(|i| i.0 == s.current).map_or(String::new(), |i| i.1.clone());
                        egui::ComboBox::from_id_salt(("imh-setting", s.hot)).selected_text(cur).show_ui(ui, |ui| {
                            for (i, n) in &s.items {
                                if ui.selectable_label(*i == s.current, n).clicked() {
                                    acts.push(Act::Setting(s.hot, *i));
                                }
                            }
                        });
                    }
                    None => {
                        ui.horizontal_wrapped(|ui| {
                            for (t, c) in &r.values {
                                ui.label(RichText::new(t).color(*c));
                            }
                        });
                    }
                }
                ui.end_row();
            }
        });
    });
}

fn charts(ui: &mut egui::Ui, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        let title = if v.star_log { &v.labels.star_log } else { v.charts.get(v.chart).unwrap_or(&v.labels.charts) };
        ui.label(RichText::new(title).heading().color(ink.accent));
        if v.swap {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.selectable_label(v.star_log, &v.labels.star_log).clicked() && !v.star_log {
                    acts.push(Act::StarLog(true));
                }
                if ui.selectable_label(!v.star_log, &v.labels.charts).clicked() && v.star_log {
                    acts.push(Act::StarLog(false));
                }
            });
        }
    });
    hairline(ui, ink);
    if v.star_log {
        egui::ScrollArea::vertical().auto_shrink([false; 2]).stick_to_bottom(true).show(ui, |ui| {
            for (t, c) in &v.log {
                ui.label(RichText::new(t).color(*c));
            }
        });
        return;
    }
    // The ten charts in two columns of five, as the original's buttons.
    egui::Grid::new("imh-charts").num_columns(2).spacing(vec2(6.0, 2.0)).show(ui, |ui| {
        for k in 0..5 {
            for c in [k, k + 5] {
                let n = v.charts.get(c).map_or("", |s| s.as_str());
                if ui.selectable_label(v.chart == c, n).clicked() {
                    acts.push(Act::Chart(c));
                }
            }
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    plot(ui, v, ink);
}

/// `Info_PutChart`'s lines: every known empire's cells on one scale (the largest value at the top).
fn plot(ui: &mut egui::Ui, v: &View, ink: &Ink) {
    let size = vec2(ui.available_width(), ui.available_height().max(80.0));
    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 2.0, Color32::from_black_alpha(90));
    for k in 1..4 {
        let y = r.top() + r.height() * k as f32 / 4.0;
        p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0f32, ink.edge.gamma_multiply(0.6)));
    }
    let peak = v.series.iter().flat_map(|s| s.1.iter().copied()).fold(0.0f32, f32::max);
    let scale = if peak > 0.0 { peak } else { 1.0 };
    let r = r.shrink(6.0);
    for (c, cells) in &v.series {
        let n = cells.len().max(2) - 1;
        let pts: Vec<Pos2> = cells.iter().enumerate().map(|(i, &y)| pos2(r.left() + r.width() * i as f32 / n as f32, r.bottom() - r.height() * (y.max(0.0) / scale))).collect();
        p.add(Shape::line(pts, Stroke::new(1.6f32, *c)));
    }
    let font = egui::TextStyle::Small.resolve(ui.style());
    p.text(r.left_top(), egui::Align2::LEFT_TOP, sm::money(peak as f64).trim_start_matches('$'), font, ink.dim);
}

/// The detail lines (`crate::info_screen::detail_lines`) as rows: a new row per virtual y, its first text in the label
/// colour the label, a planet setting's value its menu.
fn detail_rows(lines: &[Put], dec: &dyn Fn(&[u8]) -> String, col: &dyn Fn(usize) -> Color32, setting: &dyn Fn(u32) -> Setting) -> Vec<DetailRow> {
    let mut out: Vec<DetailRow> = Vec::new();
    let mut y = f32::NAN;
    for p in lines {
        if p.y != y {
            y = p.y;
            out.push(DetailRow::default());
        }
        let r = out.last_mut().unwrap();
        let t = dec(&p.text);
        if let Some(k) = p.setting {
            r.setting = Some(setting(k));
        } else if p.colour == 1 && r.label.is_empty() && r.values.is_empty() {
            r.label = t;
        } else if t == "%" {
            if let Some(last) = r.values.last_mut() {
                last.0.push('%');
            }
        } else {
            r.values.push((t, col(p.colour)));
        }
    }
    out
}

/// What the modern info screen shows this frame, from the state the classic panels read.
pub fn view(h: &Hud, state: &ig2_sim::save::SaveState, rules: &ig2_rules::Rules, s: &UiSnap, gates: Option<&[bool; 20]>, time: f32) -> View {
    let lang = h.texts.lang;
    let dec = |b: &[u8]| ig2_formats::text::decode(b.split(|&c| c == 0).next().unwrap_or(&[]), lang).trim().to_string();
    let col = |i: usize| crate::gui::ink(h.palette.get(i).copied().unwrap_or([1.0; 3]));
    let ours = |e: &str| dec(&h.texts.ours(e));
    let ui = &h.info;
    let d = &s.info;
    let en = s.human;
    let e = ui.selected.min(18);
    let empires = (1..=8)
        .filter(|&k| s.alive[k] && s.known[k])
        .map(|k| {
            // The race names' colours by `Viszony[InfoSelected][e]` (`KirakFajNeveket`).
            let c = match s.levels.get(ui.selected).map_or(0, |r| r[k]) {
                1 => 6,
                2 => 3,
                4 => 0x29,
                5 => 7,
                _ => 1,
            };
            EmpireRow { e: k, name: h.empire_names.get(k).map(|n| dec(n)).unwrap_or_default(), colour: col(c), race: col(k + 15), planets: d.known_planets[k], fleets: d.known_fleets[k] }
        })
        .collect();
    let label = |k: u16| dec(&h.texts.get(9, 3, k));
    let mut summary = vec![
        (label(2), h.empire_names.get(e).map(|n| dec(n)).unwrap_or_default()),
        (label(5), dec(&crate::ui::sprintf(&h.texts.get(9, 3, 9), &[d.known_planets[e] as i64, d.known_fleets[e] as i64]))),
    ];
    let mut money = None;
    if e == en {
        // `KirakBirodalomInfo`'s income rows: `(9,3,6) ": %d - " (9,3,7) ", %d - " (9,3,8)` and `(9,3,4) ": %d"`.
        summary.push((label(3), format!("{}: {} - {}, {} - {}", label(6), d.trade_last, label(7), d.tax_last, label(8))));
        summary.push((label(4), d.month_income.to_string()));
        let part = (((d.limit / 10500.0 + 1.0).ln() / 3.0) as f32).clamp(0.0, 1.0);
        let pct = |x: f64| format!("{}%", (x * 100.0) as i32);
        money = Some(Money {
            limit: (label(1), format!("${}", d.limit as i32), part),
            corners: [10, 11, 12].map(label),
            shares: d.shares.map(pct),
            knob: ui.tri,
            blink: if ui.tri_novelo != 100.0 { crate::hud::blink(time, 300.0) } else { 1.0 },
        });
    }
    let list = d.list(ui.selected, en);
    let rows = list
        .iter()
        .enumerate()
        .map(|(j, &entry)| {
            // The problem line steps every 30 `Counter` through the morale and the planet's own problems.
            let problem = match entry {
                Entry::Planet(i) => {
                    let mut rendezo = [0usize; 5];
                    let n = crate::info_screen::InfoData::sort_problems(&d.planets[i], d.debug, &mut rendezo);
                    let step = (ui.counter / 30).max(0) as usize % (n + 1);
                    (step, rendezo[step])
                }
                Entry::Fleet(_) => (0, 0),
            };
            let puts = crate::info_screen::entry_lines(d, entry, &h.texts, &h.txt17, &h.empire_names, en, problem, ui.counter);
            let mut lines = puts.iter().map(|p| (dec(&p.text), col(p.colour)));
            let title = lines.next().unwrap_or_default();
            let (size, alert) = match entry {
                Entry::Planet(i) => {
                    let p = &d.planets[i];
                    let mine = p.owner as usize == en || (d.debug && p.owner != 0);
                    (if p.owner != 0 && p.details { p.pop as i64 } else { -1 }, if mine { p.problems[1..].iter().copied().max().unwrap_or(0) as i64 } else { -1 })
                }
                Entry::Fleet(i) => {
                    let f = &d.fleets[i];
                    let mine = f.owner as usize == en || d.debug;
                    (if mine || d.radar > 1 { f.ships as i64 } else { -1 }, if f.owner as usize == en { f.power as i64 } else { -1 })
                }
            };
            Row { index: j, title, lines: lines.collect(), size, alert }
        })
        .collect();
    let detail = (ui.detail && !ui.leaving).then(|| {
        let lines = crate::info_screen::detail_lines(s, d, &h.texts, &h.infostr, &h.txt17, rules, ui, true);
        let rec = match ui.detail_entry(d, en) {
            Some(Entry::Planet(i)) => d.planets[i].rec.clone(),
            _ => Vec::new(),
        };
        let setting = |k: u32| {
            let (base, off) = match k {
                26 => (22, 0x34),
                27 => (30, 0x6c),
                _ => (41, 0x6d),
            };
            let items = crate::info_screen::select_items(k, d.race).map(|i| (i, dec(&h.txt17.get(0x11, 5, (base + i) as u16)))).collect();
            Setting { hot: k, items, current: rec.get(off).copied().unwrap_or(0) as i32 }
        };
        detail_rows(&lines, &dec, &col, &setting)
    });
    let series = (1..=8)
        .filter(|&k| s.known[k])
        .map(|k| (col(k + 15), (4..=50).map(|c| s.charts.get(ui.chart * 440 + (k - 1) * 55 + c).copied().unwrap_or(-1.0)).collect()))
        .collect();
    let log = ui.log.iter().skip(200usize.saturating_sub(ui.log_filled)).map(|(c, t)| (dec(t), col(*c as usize))).collect();
    let tip = |k: u16| dec(&h.texts.get(9, 1, k));
    View {
        bar: h.bar_view(state, rules, s, en as u8, gates, time, 10),
        empires,
        selected: ui.selected,
        summary,
        money,
        rows,
        detail,
        charts: (0..10).map(|c| tip(5 + c)).collect(),
        chart: ui.chart,
        series,
        star_log: ui.star_log,
        log,
        swap: s.scenario,
        labels: Labels {
            empires: ours("Empires"),
            planets: ours("Planets"),
            fleets: ours("Fleets"),
            name: ours("Name"),
            order: ours("List order"),
            size: ours("Size"),
            alert: ours("Alerts"),
            back: ours("Back"),
            row_tip: tip(1),
            charts: tip(3),
            star_log: tip(0x1a),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, size: i64, alert: i64) -> Row {
        Row { title: (name.into(), Color32::WHITE), size, alert, ..Default::default() }
    }

    #[test]
    fn sorting_keeps_the_original_order_on_ties() {
        let rows = [row("Beta", 5, -1), row("alpha", 9, 3), row("Gamma", 5, 7), row("delta", -1, 3)];
        assert_eq!(sorted(&rows, Sort::Order, false), [0, 1, 2, 3]);
        assert_eq!(sorted(&rows, Sort::Order, true), [3, 2, 1, 0]);
        assert_eq!(sorted(&rows, Sort::Name, false), [1, 0, 3, 2]);
        assert_eq!(sorted(&rows, Sort::Name, true), [2, 3, 0, 1]);
        assert_eq!(sorted(&rows, Sort::Size, false), [3, 0, 2, 1]);
        assert_eq!(sorted(&rows, Sort::Size, true), [1, 0, 2, 3]);
        assert_eq!(sorted(&rows, Sort::Alert, true), [2, 1, 3, 0]);
    }

    #[test]
    fn empires_sort_by_name_and_counts() {
        let e = |n: &str, p: u32, f: u32| EmpireRow { name: n.into(), planets: p, fleets: f, ..Default::default() };
        let rows = [e("Morgath", 3, 9), e("dribs", 12, 1), e("Ychom", 3, 4)];
        assert_eq!(empire_order(&rows, 0), [0, 1, 2]);
        assert_eq!(empire_order(&rows, 1), [1, 0, 2]);
        assert_eq!(empire_order(&rows, 2), [1, 0, 2]);
        assert_eq!(empire_order(&rows, 3), [0, 2, 1]);
    }

    /// The widgets hand the classic code its virtual points: the triangle's corners and the limit bar's ends.
    #[test]
    fn widgets_map_to_the_original_hot_spots() {
        let r = Rect::from_min_size(pos2(100.0, 40.0), vec2(188.4, 103.9));
        for v in [[319.0, 50.0 + tri_h()], [TRI_X[0], 50.0], [TRI_X[1], 50.0], [320.0, 70.0]] {
            let back = tri_from(r, tri_to(r, v));
            assert!((back[0] - v[0]).abs() < 1e-3 && (back[1] - v[1]).abs() < 1e-3, "{v:?} → {back:?}");
        }
        let bar = Rect::from_min_size(pos2(10.0, 0.0), vec2(200.0, 14.0));
        assert_eq!(limit_from(bar, 10.0), 280.0);
        assert_eq!(limit_from(bar, 110.0), 330.0);
        assert_eq!(limit_from(bar, 500.0), 380.0);
        assert_eq!(crate::info_screen::limit_at(limit_from(bar, 210.0) as i32), crate::info_screen::limit_at(380));
    }

    #[test]
    fn detail_lines_become_label_value_rows() {
        let put = |t: &str, x: f32, y: f32, c: usize, setting: Option<u32>| Put { text: t.as_bytes().to_vec(), x, y, len: 0x1a, size: 2, align: crate::font::Align::Left, colour: c, setting };
        let lines = [
            put("Name", 71.0, 253.0, 1, None),
            put("Terra", 141.0, 253.0, 3, None),
            put("Population", 71.0, 266.0, 1, None),
            put("900", 141.0, 266.0, 0x21, None),
            put("Needed", 176.0, 266.0, 1, None),
            put("950", 229.0, 266.0, 6, None),
            put("Tax", 71.0, 279.0, 1, None),
            put("Normal", 141.0, 279.0, 0x21, Some(26)),
            put("Problems", 71.0, 292.0, 1, None),
            put("12", 141.0, 292.0, 6, None),
            put("%", 154.0, 292.0, 6, None),
            put("Housing", 167.0, 292.0, 0x21, None),
            put("Ship (2)", 137.0, 305.0, 2, None),
        ];
        let dec = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        let col = |c: usize| Color32::from_gray(c as u8);
        let rows = detail_rows(&lines, &dec, &col, &|k| Setting { hot: k, items: vec![(0, "Low".into()), (1, "Normal".into())], current: 1 });
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].label, "Name");
        assert_eq!(rows[1].values.iter().map(|v| v.0.as_str()).collect::<Vec<_>>(), ["900", "Needed", "950"]);
        assert_eq!(rows[2].setting.as_ref().map(|s| (s.hot, s.current)), Some((26, 1)));
        assert_eq!(rows[3].values.iter().map(|v| v.0.as_str()).collect::<Vec<_>>(), ["12%", "Housing"]);
        assert_eq!((rows[4].label.as_str(), rows[4].values[0].0.as_str()), ("", "Ship (2)"));
    }
}
