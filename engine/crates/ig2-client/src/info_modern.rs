//! Ours (ADR 0072, ADR 0076): the modern info screen on the egui layer (ADR 0062), the "Modern info screen" option,
//! drawn with the design kit (ADR 0073, `gui::ds`) after the info boards (list, detail mode). The same functions as
//! the original's (re/screens/info.md): the eight race columns as rings with the leaders' heads (a click shows an
//! empire, the original's pointer over its column), the money triangle (0x5b) and the spending limit (0x5c), the
//! selected empire's rows, its planets and fleets as one list (the original's order or sorted), a row's detail with
//! the planet settings (0x19 8/9/10), the ten charts on the original's button plates and, in a scenario game, the
//! swap to the star log. The window fills a [`View`] from the state the classic panels read, runs [`show`] once a
//! frame and hands the [`Act`]s to the classic `InfoScreen` code (`press`, `hover`, `orders`) at the hot-spot the
//! original's player would have used, so every order is the original's.

use egui::{pos2, vec2, Color32, Pos2, Rect, Sense, Shape, Stroke, TextureHandle, Ui};

use crate::gui::ds::{self, Font, PanelKind, Text, Weight};
use crate::info_screen::{Entry, Put};
use crate::starmap_modern::{self as sm};
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
    /// The top bar's (the starmap HUD's acts); the exit plate is its starmap key.
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
    /// Ours (ADR 0076): the colony screen of the human's planet `p` (the board's Colony under the globe).
    Colony(usize),
}

/// An empire met: its column's number, name, the relation's colour, its own colour, the original race whose leader
/// it shows (1..=8) and what the human sees of it.
#[derive(Debug, Clone, Default)]
pub struct EmpireRow {
    pub e: usize,
    pub name: String,
    pub colour: Color32,
    pub race: Color32,
    pub head: usize,
    /// The race's name (the ring's label) and the empire is the human's (its label in the own colour).
    pub short: String,
    pub own: bool,
    pub planets: u32,
    pub fleets: u32,
}

/// A list row: its entry, a planet's or a fleet's, the globe's surface (1..=8, 0 none), the name, the owner line,
/// the three columns (a planet's population, morale and worst problem, a fleet's ships, order and speed) and a
/// planet's production line (`KirakBolygoInfo`/`KirakFlottaInfo`'s lines), and the sort keys (−1 where the human
/// doesn't see the value).
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub index: usize,
    pub fleet: bool,
    pub globe: u8,
    pub title: (String, Color32),
    pub owner: (String, Color32),
    pub cells: [(String, Color32); 3],
    pub queue: Option<(String, Color32)>,
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

/// The detail mode's entry: its name, owner line and globe (0 a fleet), and the rows.
#[derive(Debug, Clone, Default)]
pub struct Detail {
    pub name: (String, Color32),
    pub owner: (String, Color32),
    pub globe: u8,
    pub rows: Vec<DetailRow>,
    /// The human's planet's number: its colony screen is a press away.
    pub colony: Option<usize>,
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
    /// The planet columns' heads (population, morale, problems) and the fleet columns' (ships, orders, speed).
    pub planet_cols: [String; 3],
    pub fleet_cols: [String; 3],
    pub back: String,
    pub colony: String,
    pub exit: String,
    pub row_tip: String,
    pub charts: String,
    pub star_log: String,
    pub unknown: String,
}

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    pub empires: Vec<EmpireRow>,
    pub selected: usize,
    pub summary: Vec<(String, String)>,
    /// The selected empire's name in its colour.
    pub selected_name: (String, Color32),
    pub money: Option<Money>,
    pub rows: Vec<Row>,
    pub detail: Option<Detail>,
    /// The ten charts' names, the one shown, each known empire's line (cells 4..=50) and the empires' names.
    pub charts: Vec<String>,
    pub chart: usize,
    pub series: Vec<(Color32, Vec<f32>)>,
    pub series_names: String,
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

/// The pictures from the install (loaded once, [`Pics::load`]): the leaders' heads (`textures/diplomac/<r>a.bmp`,
/// 128 × 128, race 1 first), the planets' surface maps (`textures/starmap/p<n>.bmp`'s map half, a small level) and
/// the chart buttons' plates (`textures/info/info_b1..6`, `info_j1..6`, 64 × 32: the original's button faces).
#[derive(Default)]
pub struct Pics {
    heads: Vec<Option<TextureHandle>>,
    globes: Vec<Option<TextureHandle>>,
    left: Vec<Option<TextureHandle>>,
    right: Vec<Option<TextureHandle>>,
}

impl Pics {
    pub fn load(ctx: &egui::Context, install: &ig2_formats::Install) -> Self {
        let tex = |dir: &str, name: String, map: bool| -> Option<TextureHandle> {
            let p = install.resolve_texture(dir, &name)?;
            let img = crate::assets::Image::load(install, &p).ok()?;
            let (mut w, mut h, mut px) = (img.width as usize, img.height as usize, &img.rgba);
            // A globe's map: a level about 256 wide; the remaster's skins use the top half (`uv_mul` 1, 0.5).
            for m in &img.mips {
                if w <= 256 || !map {
                    break;
                }
                (w, h, px) = (w / 2, h / 2, m);
            }
            let rows = if map && crate::assets::Image::uv_mul(install, &p)[1] < 0.75 { h / 2 } else { h };
            let c = egui::ColorImage::from_rgba_unmultiplied([w, rows], &px[..w * rows * 4]);
            let opt = egui::TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..egui::TextureOptions::LINEAR };
            Some(ctx.load_texture(format!("info-{dir}{name}"), c, if map { opt } else { egui::TextureOptions::LINEAR }))
        };
        Self {
            heads: (1..=8).map(|r| tex("textures/diplomac/", format!("{r}a.bmp"), false)).collect(),
            globes: (1..=8).map(|n| tex("textures/starmap/", format!("p{n}.bmp"), true)).collect(),
            left: (1..=6).map(|k| tex("textures/info/", format!("info_b{k}.bmp"), false)).collect(),
            right: (1..=6).map(|k| tex("textures/info/", format!("info_j{k}.bmp"), false)).collect(),
        }
    }

    pub(crate) fn head(&self, r: usize) -> Option<&TextureHandle> {
        self.heads.get(r.wrapping_sub(1)).and_then(Option::as_ref)
    }

    fn globe(&self, n: u8) -> Option<&TextureHandle> {
        self.globes.get((n as usize).wrapping_sub(1)).and_then(Option::as_ref)
    }
}

/// What stays between frames.
#[derive(Default)]
pub struct State {
    pub bar: sm::State,
    pub sort: Sort,
    pub descending: bool,
    /// The list's tab: 0 the planets, 1 the fleets, 2 the empires met.
    pub tab: usize,
    pub pics: Option<Pics>,
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
fn grab(ui: &Ui, resp: &egui::Response, held: &mut bool) -> Option<Press> {
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

/// The board's measures (1920 × 1080, one pixel one point): the race rings, the money panel, the selected empire's
/// panel, the list's and the charts' tops.
const RING: f32 = 112.0;
const RING_GAP: f32 = 22.0;
const MONEY_W: f32 = 400.0;
const EMPIRE_W: f32 = 960.0;
const ROW_H: f32 = 92.0;
const GLYPH: egui::Vec2 = egui::Vec2::new(76.0, 40.0);
/// The part of a leader's picture the rings show: the face (the 128-point picture has a wide margin).
const HEAD_UV: Rect = Rect { min: pos2(0.14, 0.08), max: pos2(0.86, 0.80) };
/// The list's columns after the picture (92): the name, then the three cells.
const COLS: [f32; 4] = [92.0, 170.0, 130.0, 170.0];

pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut acts = sm::board_bar(ctx, &v.bar, &mut st.bar, Act::Bar);
    let k = ds::tokens();
    let screen = ctx.screen_rect();
    let (w, h) = (screen.width(), screen.height());
    let top = sm::bar_bottom(ctx) + 20.0;
    // Shorter windows (a larger UI scale) close the board's gaps; the panels keep their sizes.
    let g = ds::squeeze(h);
    // The void behind the panels; the panel takes every press the parts don't.
    egui::CentralPanel::default().frame(egui::Frame::new().fill(k.void.gamma_multiply(0.94))).show(ctx, |ui| {
        let r = ui.max_rect();
        ui.allocate_rect(r, Sense::click_and_drag());
    });
    let ring = RING.min(((w - 2.0 * ds::BOARD_EDGE - MONEY_W - 2.0 * RING_GAP) / 2.0 - 3.0 * RING_GAP) / 4.0).max(56.0);
    rings(ctx, v, st, top, w, ring, &mut acts);
    let money = ds::place(ctx, "imh-money", Rect::from_min_size(pos2((w - MONEY_W) / 2.0, top), vec2(MONEY_W, 186.0)), |ui| {
        if let Some(m) = &v.money {
            money_panel(ui, m, st, &mut acts);
        }
    });
    let money_bottom = if v.money.is_some() { money.response.rect.bottom() } else { top + ring + 24.0 };
    let ew = EMPIRE_W.min(w - 2.0 * ds::BOARD_EDGE);
    let ey = (top + 228.0 * g).max(money_bottom + 16.0);
    let emp = ds::place(ctx, "imh-empire", Rect::from_min_size(pos2((w - ew) / 2.0, ey), vec2(ew, 0.0)), |ui| empire_panel(ui, v, st, ew, &mut acts));
    let low = (top + 406.0 * g).max(emp.response.rect.bottom() + 20.0 * g);
    let bottom = h - 80.0 * g;
    let half = (w - 3.0 * ds::BOARD_EDGE) / 2.0;
    let list = Rect::from_min_max(pos2(ds::BOARD_EDGE, low), pos2(ds::BOARD_EDGE + half, bottom.max(low + 240.0)));
    let t = ctx.animate_bool_with_time(egui::Id::new("imh-detail"), v.detail.is_some(), ds::anim(ds::OPEN_TIME));
    if t < 1.0 {
        ds::place(ctx, "imh-list", list.translate(vec2(-12.0 * t, 0.0)), |ui| {
            ui.set_opacity(1.0 - t);
            list_panel(ui, v, st, list.size(), &mut acts)
        });
    }
    if let Some(d) = &v.detail {
        ds::place(ctx, "imh-detail", list.translate(vec2(12.0 * (1.0 - t), 0.0)), |ui| {
            ui.set_opacity(t);
            detail_panel(ui, v, d, st, list.size(), &mut acts)
        });
    }
    let charts = Rect::from_min_size(pos2(w - ds::BOARD_EDGE - half, low), vec2(half, (list.height() - 60.0).max(200.0)));
    ds::place(ctx, "imh-charts", charts, |ui| charts_panel(ui, v, st, charts.size(), &mut acts));
    let exit = Rect::from_min_size(pos2(w - ds::BOARD_EDGE - ds::EXIT_W, h - 40.0 * g - 48.0), vec2(ds::EXIT_W, 48.0));
    ds::place(ctx, "imh-exit", exit, |ui| {
        if ui.add(ds::Plate::new(&v.labels.exit).width(ds::EXIT_W).height(48.0)).clicked() {
            acts.push(Act::Bar(sm::Act::Screen(2)));
        }
    });
    acts
}

/// The eight race columns as rings, four left of the money panel and four right of it: a met empire's leader in its
/// ring and its name under it in the relation's colour, the others empty and faded.
fn rings(ctx: &egui::Context, v: &View, st: &State, top: f32, w: f32, d: f32, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let side = 4.0 * d + 3.0 * RING_GAP;
    for e in 1..=8usize {
        let x = if e <= 4 { ds::BOARD_EDGE + (e - 1) as f32 * (d + RING_GAP) } else { w - ds::BOARD_EDGE - side + (e - 5) as f32 * (d + RING_GAP) };
        let slot = Rect::from_min_size(pos2(x, top), vec2(d, d + 24.0));
        let row = v.empires.iter().find(|r| r.e == e);
        ds::place(ctx, ("imh-ring", e), slot, |ui| {
            let resp = ui.allocate_rect(slot, if row.is_some() { Sense::click() } else { Sense::hover() });
            let ring = Rect::from_min_size(slot.min, vec2(d, d));
            let on = v.selected == e;
            let hot = ds::hover_t(ui.ctx(), resp.id, row.is_some() && resp.hovered());
            let fade = if row.is_some() { 1.0 } else { 0.45 };
            let p = ui.painter();
            ds::shadow(p, ring.translate(vec2(0.0, 3.0)), 255, 0, 10, 0, Color32::from_black_alpha((180.0 * fade) as u8));
            let stops = [(0.0, Color32::BLACK), (0.58, Color32::BLACK), (0.59, k.metal_shadow), (0.65, k.metal_light), (0.74, k.metal), (1.0, k.metal_dark)].map(|(t, c)| (t, c.gamma_multiply(fade)));
            ds::radial(p, ring, 255, [0.5, 0.5], &stops);
            ds::relief(p, ring, 255);
            if let Some(r) = row {
                // The leader in the ring's dark window, lit up a little under the pointer.
                if let Some(t) = st.pics.as_ref().and_then(|p| p.head(r.head)) {
                    // The board's head over the ring, a little above the centre (its chin on the ring's dark window).
                    let face = Rect::from_center_size(ring.center() - vec2(0.0, d * 0.04), vec2(d * 0.94, d * 0.94));
                    let c = Color32::from_gray((215.0 + 40.0 * hot.max(on as u8 as f32)) as u8);
                    p.add(egui::epaint::RectShape::filled(face, d * 0.12, c).with_texture(t.id(), HEAD_UV));
                } else {
                    p.circle_filled(ring.center(), d * 0.22, r.race);
                }
                p.circle_stroke(ring.center(), d * 0.41, Stroke::new(2.0f32, r.race.gamma_multiply(0.35 + 0.4 * hot)));
            }
            if on {
                p.circle_stroke(ring.center(), d / 2.0 + 1.0, Stroke::new(2.0f32, k.ink_silver));
                ds::shadow(p, ring, 255, 0, 14, 0, k.ink_silver.gamma_multiply(0.18));
            } else if hot > 0.0 {
                p.circle_stroke(ring.center(), d / 2.0 + 1.0, Stroke::new(1.0f32, k.rim_bright.gamma_multiply(hot)));
            }
            let label = Rect::from_min_size(pos2(slot.left() - RING_GAP / 2.0 + 1.0, ring.bottom() + 6.0), vec2(d + RING_GAP - 2.0, 18.0));
            let (name, c) = row.map_or((v.labels.unknown.as_str(), k.ink_muted), |r| (if r.short.is_empty() { r.name.as_str() } else { r.short.as_str() }, if r.own { k.own } else { r.colour }));
            Text::new(name, Font::LABEL).center().shrink(10.0).color(c.gamma_multiply(fade)).paint(ui, label);
            if resp.clicked() && !on {
                acts.push(Act::Empire(e));
            }
            if let Some(r) = row {
                ds::tip(resp.on_hover_cursor(egui::CursorIcon::PointingHand), &r.name);
            }
        });
    }
}

/// The money panel (the deep panel between the rings): the triangle with its knob, the corners' names and shares on
/// LCDs (blinking after a drag).
fn money_panel(ui: &mut Ui, m: &Money, st: &mut State, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, vec2(MONEY_W, 186.0));
    ds::paint_panel(ui.painter(), rect, PanelKind::Deep, 0.88);
    let inner = rect.shrink2(vec2(16.0, 12.0));
    let tri = Rect::from_min_size(pos2(inner.center().x - 75.0, inner.top()), vec2(150.0, 130.0));
    // B (top left, `corners[1]`) and C (top right, `corners[2]`) at the sides, A (`corners[0]`) under the triangle.
    let side_w = (inner.width() - 150.0) / 2.0 - 8.0;
    let lcd = Font::READOUT.sized(20.0);
    let blink = k.lcd_ink.gamma_multiply(m.blink);
    for (i, x, right) in [(1usize, inner.left(), false), (2, inner.right() - side_w, true)] {
        let col = Rect::from_min_size(pos2(x, inner.top() + 28.0), vec2(side_w, 18.0));
        let t = Text::new(&m.corners[i], Font::LABEL).shrink(10.0).color(k.ink_muted);
        if right { t.right() } else { t }.paint(ui, col);
        let well = Rect::from_min_size(pos2(x, col.bottom() + 6.0), vec2(side_w, 28.0));
        ds::lcd_window(ui.painter(), well);
        Text::new(&m.shares[i], lcd).center().color(blink).paint(ui, well.shrink2(vec2(6.0, 0.0)));
    }
    let base = Rect::from_min_size(pos2(inner.left(), tri.bottom() + 8.0), vec2(inner.width(), 28.0));
    let lw = Text::new(&m.corners[0], Font::LABEL).layout(ui.ctx(), f32::INFINITY).width.min(base.width() - 110.0);
    let (lab, well) = (Rect::from_min_size(pos2(base.center().x - (lw + 106.0) / 2.0, base.top()), vec2(lw, 28.0)), Rect::from_min_size(pos2(base.center().x - (lw + 106.0) / 2.0 + lw + 10.0, base.top()), vec2(96.0, 28.0)));
    Text::new(&m.corners[0], Font::LABEL).color(k.ink_muted).paint(ui, lab);
    ds::lcd_window(ui.painter(), well);
    Text::new(&m.shares[0], lcd).center().color(blink).paint(ui, well.shrink2(vec2(6.0, 0.0)));
    // The triangle: the virtual box's corners on the board's 10..140 × 10..122.
    let box_ = Rect::from_min_max(tri.min + vec2(10.0, 10.0), tri.min + vec2(140.0, 122.0));
    let resp = ui.interact(tri, ui.id().with("imh-tri"), Sense::click_and_drag());
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered() || st.tri);
    let p = ui.painter();
    let [a, b, c] = [[319.0, 50.0 + tri_h()], [TRI_X[0], 50.0], [TRI_X[1], 50.0]].map(|v| tri_to(box_, v));
    p.add(Shape::convex_polygon(vec![b, c, a], k.info.gamma_multiply(0.08 + 0.06 * hot), Stroke::NONE));
    let knob = tri_to(box_, m.knob);
    for corner in [a, b, c] {
        p.line_segment([knob, corner], Stroke::new(1.0f32, k.rim.gamma_multiply(0.8)));
    }
    p.add(Shape::closed_line(vec![b, c, a], Stroke::new(3.0f32, k.metal_light)));
    if hot > 0.0 {
        ds::shadow(p, Rect::from_center_size(knob, vec2(14.0, 14.0)), 255, 0, 12, 2, k.lcd_ink.gamma_multiply(0.5 * hot));
    }
    p.circle_filled(knob, 7.0, k.lcd_ink);
    p.circle_stroke(knob, 11.0, Stroke::new(2.0f32, k.lcd));
    if let Some(press) = grab(ui, &resp, &mut st.tri) {
        let at = ui.input(|i| i.pointer.latest_pos()).unwrap_or(knob);
        acts.push(Act::Triangle(tri_from(box_, at), press));
    }
    resp.on_hover_cursor(egui::CursorIcon::Grab);
}

/// The selected empire's panel: its rows (`KirakBirodalomInfo`) as two table columns and, for the human, the spending
/// limit's bar with its LCD.
fn empire_panel(ui: &mut Ui, v: &View, st: &mut State, w: f32, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    ds::panel(ui, PanelKind::Glass, 0.88, 0.0, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 16)).show(ui, |ui| {
            ui.set_width(w - 48.0);
            let col = (w - 48.0 - 40.0) / 2.0;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 40.0;
                for (c, half) in v.summary.chunks(2).take(2).enumerate() {
                    ui.allocate_ui_with_layout(vec2(col, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        // The board's `.ig-table` lines: the labels muted in a column as wide as the widest (at most a
                        // third), the values in info blue, the empire's name in its colour.
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let lw = half.iter().map(|(l, _)| Text::new(l, Font::LABEL).layout(ui.ctx(), f32::INFINITY).width).fold(0.0, f32::max).min(col / 3.0);
                        for (n, (l, val)) in half.iter().enumerate() {
                            let (rect, _) = ui.allocate_exact_size(vec2(col, 22.0), Sense::hover());
                            let base = ds::fit::baseline(rect, Font::DATA);
                            Text::new(l, Font::LABEL).shrink(10.0).baseline(base).color(k.ink_muted).paint(ui, rect.with_max_x(rect.left() + lw));
                            let c = if c == 0 && n == 0 { v.selected_name.1 } else { k.info };
                            Text::new(val, Font::DATA).shrink(11.0).baseline(base).color(c).paint(ui, rect.with_min_x(rect.left() + lw + 16.0));
                        }
                    });
                }
            });
            if let Some(m) = &v.money {
                ui.add_space(12.0);
                limit_row(ui, m, st, acts, k);
            }
        });
    });
}

/// The spending limit (hot-spot 32): its label, the bar (pressed or dragged at the virtual x) and the value's LCD.
fn limit_row(ui: &mut Ui, m: &Money, st: &mut State, acts: &mut Vec<Act>, k: ds::Tokens) {
    let (label, text, part) = &m.limit;
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::hover());
    let lw = 150.0;
    Text::new(label, Font::LABEL).shrink(10.0).color(k.ink_muted).paint(ui, row.with_max_x(row.left() + lw));
    let lcd = Rect::from_min_size(pos2(row.right() - 120.0, row.top()), vec2(120.0, 28.0));
    ds::lcd_window(ui.painter(), lcd);
    Text::new(text, Font::READOUT.sized(20.0)).right().shrink(14.0).color(k.lcd_ink).paint(ui, lcd.shrink2(vec2(8.0, 0.0)));
    let track = Rect::from_min_max(pos2(row.left() + lw + 16.0, row.top()), pos2(lcd.left() - 16.0, row.bottom()));
    let resp = ui.interact(track, ui.id().with("imh-limit"), Sense::click_and_drag());
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered() || st.limit);
    let bar = Rect::from_center_size(track.center(), vec2(track.width(), 6.0));
    let p = ui.painter();
    p.rect_filled(bar, 3, k.glass_deep);
    let f = ui.ctx().animate_value_with_time(resp.id.with("fill"), *part, ds::anim(ds::GLIDE_TIME));
    p.rect_filled(bar.with_max_x(bar.left() + bar.width() * f), 3, k.info);
    ds::rim(p, bar, 3, k.rim);
    let x = bar.left() + bar.width() * f;
    let thumb = Rect::from_center_size(pos2(x, bar.center().y), vec2(18.0, 18.0));
    ds::shadow(p, thumb.translate(vec2(0.0, 2.0)), 255, 0, 6, 0, Color32::from_black_alpha(160));
    ds::radial(p, thumb, 255, [0.35, 0.3], &[(0.0, k.metal_shine), (0.5, k.metal_light), (1.0, k.metal_dark)]);
    if hot > 0.0 {
        p.circle_stroke(thumb.center(), 10.0, Stroke::new(2.0f32, k.info.gamma_multiply(hot)));
    }
    if let Some(press) = grab(ui, &resp, &mut st.limit) {
        let at = ui.input(|i| i.pointer.latest_pos()).map_or(bar.left(), |p| p.x);
        acts.push(Act::Limit(limit_from(bar, at), press));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand);
}

/// A sort head: its label (upper case, muted; the chosen one bright with its chevron).
fn sort_head(ui: &Ui, rect: Rect, label: &str, on: bool, descending: bool, id: egui::Id) -> bool {
    let k = ds::tokens();
    let resp = ui.interact(rect, id, Sense::click());
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
    let c = if on { k.ink } else { k.ink_muted.lerp_to_gamma(k.ink_silver, hot) };
    let fit = Text::new(label, Font::LABEL).shrink(10.0).color(c).paint(ui, rect.with_max_x(rect.right() - 16.0));
    if on {
        let m = pos2(rect.left() + fit.width + 10.0, rect.center().y);
        let s = Stroke::new(2.0f32, c);
        let (a, b) = if descending { (-3.0, 3.0) } else { (3.0, -3.0) };
        ui.painter().line(vec![m + vec2(-4.0, a / 1.5), m + vec2(0.0, b / 1.5 + a / 3.0), m + vec2(4.0, a / 1.5)], s);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// The list panel: the tabs (planets, fleets, the empires met, with their counts), the selected empire's name, the
/// column heads (sortable), the rows, and under them the range shown, a scroll bar and the row's hint.
fn list_panel(ui: &mut Ui, v: &View, st: &mut State, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Deep, 0.88);
    let inner = rect.shrink2(vec2(16.0, 12.0));
    let planets = v.rows.iter().filter(|r| !r.fleet).count();
    let fleets = v.rows.len() - planets;
    let tabs = [format!("{}  {planets}", v.labels.planets), format!("{}  {fleets}", v.labels.fleets), format!("{}  {}", v.labels.empires, v.empires.len())];
    let mut strip = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(inner.min, vec2(inner.width() * 0.62, 36.0))));
    if let Some(t) = ds::tabs(&mut strip, "imh-tabs", &tabs.iter().map(String::as_str).collect::<Vec<_>>(), st.tab) {
        st.tab = t;
    }
    let name = Rect::from_min_max(pos2(inner.left() + inner.width() * 0.64, inner.top()), pos2(inner.right(), inner.top() + 36.0));
    Text::new(&v.selected_name.0, Font::LABEL).right().shrink(10.0).color(k.ink_muted).paint(ui, name);
    let head = Rect::from_min_size(pos2(inner.left(), inner.top() + 46.0), vec2(inner.width(), 26.0));
    let foot = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 18.0), inner.right_bottom());
    let body = Rect::from_min_max(pos2(inner.left(), head.bottom() + 6.0), pos2(inner.right(), foot.top() - 10.0));
    ui.painter().hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0f32, k.rim));
    let (shown, total, frac) = if st.tab == 2 { empire_list(ui, v, st, head, body, acts) } else { entry_list(ui, v, st, head, body, acts) };
    // The range shown, the scroll bar, the hint.
    let lab = format!("{}–{} / {total}", (shown.0 + 1).min(total), shown.1.min(total));
    let lw = 120.0;
    Text::new(&lab, Font::LABEL).color(k.ink_muted).paint(ui, foot.with_max_x(foot.left() + lw));
    let hint_w = (foot.width() * 0.34).min(300.0);
    Text::new(&v.labels.row_tip, Font::LABEL).right().shrink(10.0).color(k.ink_muted).paint(ui, foot.with_min_x(foot.right() - hint_w));
    let track = Rect::from_center_size(pos2((foot.left() + lw + 12.0 + foot.right() - hint_w - 12.0) / 2.0, foot.center().y), vec2(foot.width() - lw - hint_w - 24.0, 6.0));
    let p = ui.painter();
    p.rect_filled(track, 3, Color32::BLACK);
    ds::rim(p, track, 3, k.rim);
    let (a, b) = frac;
    p.rect_filled(Rect::from_min_max(pos2(track.left() + track.width() * a, track.top()), pos2(track.left() + track.width() * b, track.bottom())), 3, k.rim_bright);
}

/// The planets' or the fleets' rows of the selected empire: (the first and last shown, the count, the bar's part).
fn entry_list(ui: &mut Ui, v: &View, st: &mut State, head: Rect, body: Rect, acts: &mut Vec<Act>) -> ((usize, usize), usize, (f32, f32)) {
    let fleet = st.tab == 1;
    let cols = if fleet { &v.labels.fleet_cols } else { &v.labels.planet_cols };
    let x = cols_x(head);
    // The middle column (morale, orders) has no sort of its own; a second click on a head turns its order, a third
    // goes back to the original's.
    let heads = [(Some(Sort::Name), &v.labels.name, x[1]), (Some(Sort::Size), &cols[0], x[2]), (None, &cols[1], x[3]), (Some(Sort::Alert), &cols[2], x[4])];
    for (i, (s, l, at)) in heads.into_iter().enumerate() {
        let end = x.get(i + 2).copied().unwrap_or(head.right());
        let r = Rect::from_min_max(pos2(at, head.top()), pos2(end - 8.0, head.bottom() - 8.0));
        let Some(s) = s else {
            Text::new(l, Font::LABEL).shrink(10.0).color(ds::tokens().ink_muted).paint(ui, r);
            continue;
        };
        if sort_head(ui, r, l, st.sort == s, st.descending, ui.id().with(("imh-sort", i))) {
            (st.sort, st.descending) = match (st.sort == s, st.descending) {
                (false, _) => (s, s != Sort::Name),
                (true, d) if d == (s != Sort::Name) => (s, !d),
                _ => (Sort::Order, false),
            };
        }
    }
    let order: Vec<usize> = sorted(&v.rows, st.sort, st.descending).into_iter().filter(|&i| v.rows[i].fleet == fleet).collect();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    let out = egui::ScrollArea::vertical().id_salt(("imh-rows", fleet)).auto_shrink([false; 2]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for &i in &order {
            row(ui, &v.rows[i], st, acts);
        }
    });
    range(out.state.offset.y, body.height(), order.len())
}

/// The first and last row shown and the scroll bar's part for `n` rows of 96 at `offset`.
fn range(offset: f32, height: f32, n: usize) -> ((usize, usize), usize, (f32, f32)) {
    let pitch = ROW_H + 4.0;
    let first = (offset / pitch).floor().max(0.0) as usize;
    let last = (((offset + height) / pitch).round() as usize).min(n);
    let all = (n as f32 * pitch).max(height);
    ((first, last.max(first)), n, (offset / all, ((offset + height) / all).min(1.0)))
}

/// The list's column lefts: the picture, the name, the three cells (the board's 92 / 1fr / 170 / 130 / 170).
fn cols_x(r: Rect) -> [f32; 5] {
    // A narrower list (4:3) narrows the cells with the name, the name keeping at least 38 % of the room.
    let room = r.width() - 8.0 - COLS[0];
    let f = (room * 0.62 / COLS[1..].iter().sum::<f32>()).min(1.0);
    let c = COLS.map(|w| w * f);
    let name = room - c[1] - c[2] - c[3];
    let x0 = r.left() + 8.0;
    [x0, x0 + COLS[0], x0 + COLS[0] + name, x0 + COLS[0] + name + c[1], x0 + COLS[0] + name + c[1] + c[2]]
}

/// A planet's or a fleet's row: the globe (or the fleet's mark), the name over its owner (and a planet's production
/// line), the three cells on one baseline; a click opens its detail.
fn row(ui: &mut Ui, r: &Row, st: &State, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
    ds::row_back(ui, rect, hot, resp.is_pointer_button_down_on());
    let x = cols_x(rect);
    let pic = Rect::from_center_size(pos2(x[0] + 40.0, rect.center().y), vec2(80.0, 80.0));
    picture(ui, pic, r.globe, r.title.1, st, ui.ctx().input(|i| i.time) as f32 * (0.01 + 0.02 * hot));
    let name = Rect::from_min_max(pos2(x[1], rect.top() + 14.0), pos2(x[2] - 12.0, rect.top() + 42.0));
    Text::new(&r.title.0, Font::HEADING.weight(Weight::Medium).tracking(0.06)).shrink(14.0).color(k.ink).paint(ui, name);
    let sub = Rect::from_min_size(pos2(x[1], name.bottom() + 2.0), vec2(name.width(), 20.0));
    Text::new(&r.owner.0, Font::DATA.upper(true)).shrink(11.0).color(r.owner.1).paint(ui, sub);
    if let Some((q, c)) = &r.queue {
        Text::new(q, Font::DATA.sized(13.0)).shrink(10.0).color(*c).paint(ui, sub.translate(vec2(0.0, 20.0)));
    }
    let base = ds::fit::baseline(rect, Font::DATA);
    for (i, (t, c)) in r.cells.iter().enumerate() {
        let end = x.get(i + 3).copied().unwrap_or(rect.right()) - 10.0;
        let f = if i == 2 && !r.fleet && !t.is_empty() { Font::DATA.weight(Weight::SemiBold) } else { Font::DATA };
        Text::new(t, f).shrink(11.0).baseline(base).color(*c).paint(ui, Rect::from_min_max(pos2(x[i + 2], rect.top()), pos2(end, rect.bottom())));
    }
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        acts.push(Act::Open(r.index));
    }
}

/// A planet's turning globe in `rect` (its surface's map), or a fleet's mark: a chevron in its owner's colour.
fn picture(ui: &Ui, rect: Rect, globe: u8, colour: Color32, st: &State, spin: f32) {
    let k = ds::tokens();
    let p = ui.painter();
    match st.pics.as_ref().and_then(|p| p.globe(globe)) {
        Some(t) => ds::globe(p, rect.center(), rect.width() / 2.0 - 4.0, t.id(), spin, k.info, ui.opacity()),
        None => {
            let c = rect.center();
            let d = rect.width() * 0.3;
            p.circle_filled(c, rect.width() / 2.0 - 6.0, k.glass_deep);
            ds::rim(p, rect.shrink(6.0), 255, k.rim);
            if globe == 0 {
                p.add(Shape::convex_polygon(vec![c + vec2(d, 0.0), c + vec2(-d * 0.8, -d * 0.6), c + vec2(-d * 0.4, 0.0), c + vec2(-d * 0.8, d * 0.6)], colour.gamma_multiply(0.85), Stroke::NONE));
            }
        }
    }
}

/// The empires met as rows (the race columns' set): the leader, the name in the relation's colour, the planets and
/// fleets seen; a click shows that empire.
fn empire_list(ui: &mut Ui, v: &View, st: &State, head: Rect, body: Rect, acts: &mut Vec<Act>) -> ((usize, usize), usize, (f32, f32)) {
    let k = ds::tokens();
    let x = cols_x(head);
    for (l, at) in [(&v.labels.name, x[1]), (&v.labels.planets, x[2]), (&v.labels.fleets, x[3])] {
        Text::new(l, Font::LABEL).shrink(10.0).color(k.ink_muted).paint(ui, Rect::from_min_max(pos2(at, head.top()), pos2(at + 160.0, head.bottom() - 8.0)));
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    let out = egui::ScrollArea::vertical().id_salt("imh-empires").auto_shrink([false; 2]).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        for e in &v.empires {
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
            let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
            ds::row_back(ui, rect, hot, e.e == v.selected);
            let x = cols_x(rect);
            let face = Rect::from_center_size(pos2(x[0] + 40.0, rect.center().y), vec2(72.0, 72.0));
            ds::radial(ui.painter(), face.expand(4.0), 255, [0.5, 0.5], &[(0.0, Color32::BLACK), (0.62, Color32::BLACK), (0.66, k.metal_light), (1.0, k.metal_dark)]);
            match st.pics.as_ref().and_then(|p| p.head(e.head)) {
                Some(t) => _ = ui.painter().add(egui::epaint::RectShape::filled(face, 255, Color32::from_gray(230)).with_texture(t.id(), HEAD_UV)),
                None => _ = ui.painter().circle_filled(face.center(), 8.0, e.race),
            }
            let base = ds::fit::baseline(rect, Font::DATA);
            Text::new(&e.name, Font::HEADING.weight(Weight::Medium).tracking(0.06)).shrink(14.0).baseline(base + 2.0).color(e.colour).paint(ui, Rect::from_min_max(pos2(x[1], rect.top()), pos2(x[2] - 12.0, rect.bottom())));
            for (n, at) in [(e.planets, x[2]), (e.fleets, x[3])] {
                Text::new(&n.to_string(), Font::DATA).baseline(base).color(k.info).paint(ui, Rect::from_min_max(pos2(at, rect.top()), pos2(at + 120.0, rect.bottom())));
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && e.e != v.selected {
                acts.push(Act::Empire(e.e));
            }
        }
    });
    range(out.state.offset.y, body.height(), v.empires.len())
}

/// The detail mode's panel: the entry's globe, name and owner, Back (hot-spot 37), and its rows as a table with the
/// planet settings as drop-downs.
fn detail_panel(ui: &mut Ui, v: &View, d: &Detail, st: &mut State, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Deep, 0.88);
    let inner = rect.shrink2(vec2(24.0, 18.0));
    let left = Rect::from_min_size(inner.min, vec2(236.0, inner.height()));
    // Back (the board's "‹ List") over the column, the globe under it.
    let back = Rect::from_min_size(left.min, vec2(left.width(), 34.0));
    let label = format!("‹  {}", v.labels.back);
    if ds::either(&ui.put(back, ds::Plate::new(&label).font(ds::Plate::FONT.sized(16.0)).height(34.0).width(left.width()))) {
        acts.push(Act::Back);
    }
    let pic = Rect::from_min_size(pos2(left.center().x - 110.0, back.bottom() + 16.0), vec2(220.0, 220.0));
    picture(ui, pic, d.globe, d.name.1, st, ui.ctx().input(|i| i.time) as f32 * 0.008);
    let name = Rect::from_min_size(pos2(left.left(), pic.bottom() + 14.0), vec2(left.width(), 32.0));
    Text::new(&d.name.0, Font::HEADING.sized(24.0).weight(Weight::Medium)).center().shrink(16.0).color(k.ink).paint(ui, name);
    Text::new(&d.owner.0, Font::DATA.upper(true)).center().shrink(11.0).color(d.owner.1).paint(ui, name.translate(vec2(0.0, 32.0)).with_max_y(name.bottom() + 52.0));
    if let Some(p) = d.colony {
        let b = Rect::from_center_size(pos2(left.center().x, name.bottom() + 76.0), vec2(120.0, 34.0));
        if ds::either(&ui.put(b, ds::Plate::new(&v.labels.colony).font(ds::Plate::FONT.sized(15.0)).height(34.0).width(120.0))) {
            acts.push(Act::Colony(p));
        }
    }
    let table = Rect::from_min_max(pos2(left.right() + 24.0, inner.top()), inner.right_bottom());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(table).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt("imh-detail-rows").auto_shrink([false; 2]).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing.y = 7.0;
        for r in &d.rows {
            detail_row(ui, r, acts);
        }
    });
}

/// One detail row: the label (150 wide) and the values on one baseline, or the setting's drop-down.
fn detail_row(ui: &mut Ui, r: &DetailRow, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    const LABEL_W: f32 = 150.0;
    let h = if r.setting.is_some() { 26.0 } else { 22.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let base = ds::fit::baseline(rect, Font::DATA);
    Text::new(r.label.trim_end_matches([':', ' ']), Font::LABEL).shrink(10.0).baseline(base).color(k.ink_muted).paint(ui, rect.with_max_x(rect.left() + LABEL_W - 12.0));
    let vals = rect.with_min_x(rect.left() + LABEL_W);
    match &r.setting {
        Some(s) => {
            let cur = s.items.iter().find(|i| i.0 == s.current).map_or("", |i| i.1.as_str());
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(vals));
            let rows: Vec<(i32, &str, bool)> = s.items.iter().map(|(i, n)| (*i, n.as_str(), false)).collect();
            if let (_, Some(Some(i))) = ds::select(&mut c, ("imh-setting", s.hot), cur, &rows, vec2(vals.width() - 8.0, 24.0), Font::DATA.sized(15.0), k.info) {
                if i != s.current {
                    acts.push(Act::Setting(s.hot, i));
                }
            }
        }
        None => {
            // The values side by side, each as wide as it needs, the last cut to what is left; none: the board's dash.
            if r.values.iter().all(|v| v.0.trim().is_empty()) {
                Text::new("—", Font::DATA).baseline(base).color(k.ink_muted).paint(ui, vals);
            }
            let mut x = vals.left();
            for (t, c) in &r.values {
                let fit = Text::new(t, Font::DATA).baseline(base).color(*c).paint(ui, Rect::from_min_max(pos2(x, rect.top()), rect.right_bottom()));
                x += fit.width + 10.0;
                if x >= vals.right() {
                    break;
                }
            }
        }
    }
}

/// The charts: the original's ten button plates in two columns (charts 0..=4 left, the star log's swap under them,
/// 5..=9 right) around the deep panel with the chosen chart (every known empire's line on one scale, a bright spot
/// running along each, as the original's) or the star log.
fn charts_panel(ui: &mut Ui, v: &View, st: &State, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    let left = Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(GLYPH.x, rect.height() - 6.0));
    let right = Rect::from_min_size(pos2(rect.right() - GLYPH.x, rect.top() + 6.0), vec2(GLYPH.x, rect.height() - 6.0));
    let pics = st.pics.as_ref();
    for c in 0..10usize {
        let (col, j) = if c < 5 { (left, c) } else { (right, c - 5) };
        let at = Rect::from_min_size(col.min + vec2(0.0, j as f32 * (GLYPH.y + 10.0)), GLYPH);
        let tex = pics.and_then(|p| if c < 5 { p.left.get(c) } else { p.right.get(c - 5) }).and_then(Option::as_ref);
        let name = v.charts.get(c).map_or("", String::as_str);
        if glyph(ui, at, tex, name, !v.star_log && v.chart == c, !v.star_log, ("imh-chart", c)) {
            acts.push(Act::Chart(c));
        }
    }
    if v.swap {
        let at = Rect::from_min_size(pos2(left.left(), left.bottom() - GLYPH.y), GLYPH);
        let (tex, name) = if v.star_log { (pics.and_then(|p| p.right.get(5)), &v.labels.charts) } else { (pics.and_then(|p| p.left.get(5)), &v.labels.star_log) };
        if glyph(ui, at, tex.and_then(Option::as_ref), name, false, true, "imh-swap") {
            acts.push(Act::StarLog(!v.star_log));
        }
    }
    let mid = Rect::from_min_max(pos2(left.right() + 16.0, rect.top()), pos2(right.left() - 16.0, rect.bottom()));
    ds::paint_panel(ui.painter(), mid, PanelKind::Deep, 0.88);
    let inner = mid.shrink2(vec2(18.0, 14.0));
    let title = if v.star_log { &v.labels.star_log } else { v.charts.get(v.chart).unwrap_or(&v.labels.charts) };
    let head = Rect::from_min_size(inner.min, vec2(inner.width(), 26.0));
    let tw = Text::new(title, Font::HEADING).shrink(15.0).color(k.ink).paint(ui, head.with_max_x(head.left() + head.width() * 0.55)).width;
    if !v.star_log {
        Text::new(&v.series_names, Font::LABEL).right().shrink(10.0).baseline(ds::fit::baseline(head, Font::HEADING)).color(k.ink_muted).paint(ui, head.with_min_x(head.left() + tw + 16.0));
    }
    let body = Rect::from_min_max(pos2(inner.left(), head.bottom() + 14.0), inner.right_bottom());
    if v.star_log {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
        egui::ScrollArea::vertical().id_salt("imh-log").auto_shrink([false; 2]).stick_to_bottom(true).show(&mut child, |ui| {
            for (t, c) in &v.log {
                Text::new(t, Font::DATA).lines(3).color(*c).show(ui, vec2(ui.available_width(), Font::DATA.line));
            }
        });
        return;
    }
    plot(ui, v, body.with_max_y(body.bottom() - 22.0));
}

/// One chart plate (the original's button face, 76 × 40 on black): lit with a silver ring when chosen, a rim under
/// the pointer, sunk a point while pressed; the chart's name as the tooltip (or as the face without the picture).
fn glyph(ui: &Ui, at: Rect, tex: Option<&TextureHandle>, name: &str, on: bool, live: bool, id: impl std::hash::Hash) -> bool {
    let k = ds::tokens();
    let resp = ui.interact(at, ui.id().with(id), if live { Sense::click() } else { Sense::hover() });
    let hot = ds::hover_t(ui.ctx(), resp.id, live && resp.hovered());
    let r = if resp.is_pointer_button_down_on() { at.translate(vec2(0.0, 1.0)) } else { at };
    let p = ui.painter();
    p.rect_filled(r, 6, Color32::BLACK);
    match tex {
        Some(t) => {
            let c = Color32::from_gray(if on { 255 } else { (200.0 + 40.0 * hot) as u8 });
            p.add(egui::epaint::RectShape::filled(Rect::from_min_size(r.min, vec2(r.width(), r.height() - 2.0)), 6, c).with_texture(t.id(), Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0))));
        }
        None => _ = Text::new(name, Font::LABEL.sized(10.0)).center().shrink(8.0).color(k.ink_silver).paint_only(ui, r.shrink2(vec2(4.0, 0.0))),
    }
    if on {
        ds::shadow(p, r, 6, 0, 10, 0, k.ink_silver.gamma_multiply(0.25));
        p.rect_stroke(r.expand(1.0), 7, Stroke::new(2.0f32, k.ink_silver), egui::StrokeKind::Inside);
    } else {
        p.rect_stroke(r.expand(1.0), 7, Stroke::new(1.0f32, k.metal_dark.lerp_to_gamma(k.rim_bright, hot)), egui::StrokeKind::Inside);
    }
    let resp = ds::tip(resp, name);
    live && resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// `Info_PutChart`'s lines: every known empire's cells on one scale (the largest value at the top), on the board's
/// axes, with the original's bright spot running along each line.
fn plot(ui: &Ui, v: &View, r: Rect) {
    let k = ds::tokens();
    let p = ui.painter();
    for j in 1..4 {
        let y = r.top() + r.height() * j as f32 / 4.0;
        p.hline(r.x_range(), y, Stroke::new(1.0f32, k.rim.gamma_multiply(0.35)));
    }
    p.vline(r.left(), r.y_range(), Stroke::new(2.0f32, k.rim_bright));
    p.hline(r.x_range(), r.bottom(), Stroke::new(2.0f32, k.rim_bright));
    let peak = v.series.iter().flat_map(|s| s.1.iter().copied()).fold(0.0f32, f32::max);
    let scale = if peak > 0.0 { peak } else { 1.0 };
    let r = r.shrink2(vec2(8.0, 6.0));
    let time = ui.ctx().input(|i| i.time) as f32;
    for (j, (c, cells)) in v.series.iter().enumerate() {
        let n = cells.len().max(2) - 1;
        let pts: Vec<Pos2> = cells.iter().enumerate().map(|(i, &y)| pos2(r.left() + r.width() * i as f32 / n as f32, r.bottom() - r.height() * (y.max(0.0) / scale))).collect();
        p.add(Shape::line(pts.clone(), Stroke::new(2.0f32, *c)));
        if ds::anim(1.0) > 0.0 && !pts.is_empty() {
            let at = ((time * 9.0 + j as f32 * 11.0) % (n as f32 + 12.0)) as usize;
            if let Some(&q) = pts.get(at) {
                ds::shadow(p, Rect::from_center_size(q, vec2(6.0, 6.0)), 255, 0, 10, 2, c.gamma_multiply(0.7));
                p.circle_filled(q, 3.0, Color32::WHITE.lerp_to_gamma(*c, 0.3));
            }
        }
    }
    if ds::anim(1.0) > 0.0 {
        ui.ctx().request_repaint();
    }
    Text::new(sm::money(peak as f64).trim_start_matches('$'), Font::LABEL).color(k.ink_muted).paint(ui, Rect::from_min_size(r.left_top() + vec2(6.0, 0.0), vec2(160.0, 18.0)));
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
            EmpireRow { e: k, name: h.empire_names.get(k).map(|n| dec(n)).unwrap_or_default(), colour: col(c), race: col(k + 15), head: s.roles[k].clamp(1, 8) as usize, short: dec(&h.texts.race(s.races[k])), own: k == en, planets: d.known_planets[k], fleets: d.known_fleets[k] }
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
            let (mut cells, mut queue, mut owner) = (<[(String, Color32); 3]>::default(), None, Default::default());
            let puts = crate::info_screen::entry_lines(d, entry, &h.texts, &h.txt17, &h.empire_names, en, (0, 0), ui.counter);
            let mut title = Default::default();
            // The original's lines by their place (`KirakBolygoInfo` / `KirakFlottaInfo`: y 3, 15, 24, 33, 42).
            for p in &puts {
                let t = (dec(&p.text), col(p.colour));
                match (p.y as i32, entry) {
                    (3, _) => title = t,
                    (15, _) => owner = t,
                    (24, Entry::Fleet(_)) => cells[1] = t,
                    (33, Entry::Fleet(_)) => cells[0] = t,
                    (42, Entry::Fleet(_)) => cells[2] = t,
                    (42, Entry::Planet(_)) => queue = Some(t),
                    _ => {}
                }
            }
            let (size, alert, globe) = match entry {
                Entry::Planet(i) => {
                    let p = &d.planets[i];
                    let mine = p.owner as usize == en || (d.debug && p.owner != 0);
                    if p.owner != 0 && p.details {
                        cells[0] = (format!("{} [{:+}]", p.pop, p.growth), col(0x23));
                    }
                    if mine {
                        // The morale and the worst problem side by side (the original's line steps through them).
                        cells[1] = (dec(&h.txt17.get(0x11, 4, (41 + p.morale / 100).clamp(0, 99) as u16)), col(0x23));
                        let mut rendezo = [0usize; 5];
                        if crate::info_screen::InfoData::sort_problems(p, d.debug, &mut rendezo) > 0 {
                            let slot = rendezo[1];
                            cells[2] = (format!("{}% {}", p.problems[slot] / 10000, dec(&h.txt17.get(0x11, 4, (21 + slot) as u16))), col(6));
                        }
                    }
                    let surface = d.surface_like.get(p.surface as usize).copied().filter(|&s| s != 0).unwrap_or(p.surface);
                    (if p.owner != 0 && p.details { p.pop as i64 } else { -1 }, if mine { p.problems[1..].iter().copied().max().unwrap_or(0) as i64 } else { -1 }, if (1..=8).contains(&surface) { surface } else { 1 })
                }
                Entry::Fleet(i) => {
                    let f = &d.fleets[i];
                    let mine = f.owner as usize == en || d.debug;
                    (if mine || d.radar > 1 { f.ships as i64 } else { -1 }, if f.owner as usize == en { f.power as i64 } else { -1 }, 0)
                }
            };
            // The human's own entries name their owner in the own colour (the board's green).
            let owned = match entry {
                Entry::Planet(i) => d.planets[i].owner as usize == en,
                Entry::Fleet(i) => d.fleets[i].owner as usize == en,
            };
            if owned {
                owner.1 = ds::tokens().own;
            }
            Row { index: j, fleet: matches!(entry, Entry::Fleet(_)), globe, title, owner, cells, queue, size, alert }
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
        let (name, owner, globe) = match ui.detail_entry(d, en) {
            Some(Entry::Planet(i)) => {
                let p = &d.planets[i];
                let surface = d.surface_like.get(p.surface as usize).copied().filter(|&s| s != 0).unwrap_or(p.surface);
                ((dec(&p.name), col(0x22)), (h.empire_names.get(p.owner as usize).map(|n| dec(n)).unwrap_or_default(), col(0x23)), if (1..=8).contains(&surface) { surface } else { 1 })
            }
            Some(Entry::Fleet(i)) => {
                let f = &d.fleets[i];
                ((dec(&f.name), col(3)), (h.empire_names.get(f.owner as usize).map(|n| dec(n)).unwrap_or_default(), col(0x21)), 0)
            }
            None => Default::default(),
        };
        // The board's table: the values in the ink (the original's value colour 0x21), the rest in their colours; the
        // name's row is the left column's title.
        let ink = |c: usize| if c == 0x21 { ds::tokens().ink } else { col(c) };
        let mut rows = detail_rows(&lines, &dec, &ink, &setting);
        rows.retain(|r| r.values.first().is_none_or(|v| v.0 != name.0));
        let owner = match ui.detail_entry(d, en) {
            Some(Entry::Planet(i)) if d.planets[i].owner as usize == en => (owner.0, ds::tokens().own),
            Some(Entry::Fleet(i)) if d.fleets[i].owner as usize == en => (owner.0, ds::tokens().own),
            _ => owner,
        };
        let colony = match ui.detail_entry(d, en) {
            Some(Entry::Planet(i)) if d.planets[i].owner as usize == en => Some(i + 1),
            _ => None,
        };
        Detail { name, owner, globe, rows, colony }
    });
    let series = (1..=8)
        .filter(|&k| s.known[k])
        .map(|k| (col(k + 15), (4..=50).map(|c| s.charts.get(ui.chart * 440 + (k - 1) * 55 + c).copied().unwrap_or(-1.0)).collect()))
        .collect();
    let series_names = (1..=8).filter(|&k| s.known[k]).filter_map(|k| h.empire_names.get(k).map(|n| dec(n))).collect::<Vec<_>>().join(" · ");
    let log = ui.log.iter().skip(200usize.saturating_sub(ui.log_filled)).map(|(c, t)| (dec(t), col(*c as usize))).collect();
    let tip = |k: u16| dec(&h.texts.get(9, 1, k));
    View {
        bar: h.bar_view(state, rules, s, en as u8, gates, time, 10),
        empires,
        selected: ui.selected,
        summary,
        selected_name: (h.empire_names.get(e).map(|n| dec(n)).unwrap_or_default(), if e == en { ds::tokens().own } else { col(1) }),
        money,
        rows,
        detail,
        charts: (0..10).map(|c| tip(5 + c)).collect(),
        chart: ui.chart,
        series,
        series_names,
        star_log: ui.star_log,
        log,
        swap: s.scenario,
        labels: Labels {
            empires: ours("Empires"),
            planets: ours("Planets"),
            fleets: ours("Fleets"),
            name: ours("Name"),
            planet_cols: [ours("Population"), ours("Morale"), ours("Alerts")],
            fleet_cols: [ours("Ships"), ours("Orders"), ours("Speed")],
            back: ours("Back"),
            colony: ours("Colony"),
            exit: ours("Exit"),
            unknown: "—".into(),
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

    /// Long or translated names never move or widen a panel, and a click on a row opens its entry.
    #[test]
    fn long_texts_keep_the_panels() {
        let gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb);
        let run = |name: &str, click: Option<Pos2>| {
            let rows = (0..3).map(|i| Row { index: i, globe: 1, title: (format!("{name} {i}"), Color32::WHITE), owner: (name.into(), Color32::WHITE), cells: [(name.into(), Color32::WHITE), (name.into(), Color32::WHITE), (name.into(), Color32::WHITE)], ..Default::default() }).collect();
            let v = View { empires: vec![EmpireRow { e: 3, name: name.into(), ..Default::default() }], selected: 3, summary: vec![(name.into(), name.into()); 4], selected_name: (name.into(), Color32::WHITE), rows, charts: vec![name.into(); 10], series_names: name.into(), labels: Labels { exit: name.into(), row_tip: name.into(), ..Default::default() }, ..Default::default() };
            let mut st = State::default();
            let mut acts = Vec::new();
            let mut rects = Vec::new();
            for f in 0..3 {
                let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))), ..Default::default() };
                if let (Some(p), 1) = (click, f) {
                    input.events = vec![egui::Event::PointerMoved(p), egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }, egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }];
                }
                let _ = gui.ctx.run(input, |ctx| acts.extend(show(ctx, &v, &mut st)));
                rects = ["imh-empire", "imh-list", "imh-charts", "imh-exit"].map(|id| gui.ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap()).to_vec();
            }
            (rects, acts)
        };
        let (short, _) = run("Tra", None);
        let (long, _) = run(&"Kizsákmányoló birodalom ".repeat(12), None);
        assert_eq!(short, long);
        // The first row of the list: under the tabs and the heads.
        let (_, acts) = run("Tra", Some(pos2(400.0, short[1].top() + 12.0 + 82.0 + 40.0)));
        assert!(acts.contains(&Act::Open(0)), "{acts:?}");
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
