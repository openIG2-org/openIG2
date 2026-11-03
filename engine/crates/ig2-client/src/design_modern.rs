//! Ours (ADR 0075): the ship and vehicle design screen (`AktKep` 5, re/screens/design.md) as modern panels on the
//! egui layer, the "Modern design screen" option, drawn with the design kit (ADR 0073, `gui::ds`) after the ship
//! design board. The same functions as the original's screen: the design list (spot 9, the keys' up and down), the
//! chosen design's data (hull, price, production time, units, tank capacity; name, speed, firepower, hit points),
//! the pods with their class, tech and count (spot 0x54 and its tech list, the cells' −/+ held with the original's
//! repeat), the name input (0x56), the four console buttons (new, redesign, modify, delete; while editing OK,
//! cancel, auto, clear) with the popups of new, delete and the modifying OK; the exit is the top bar's starmap pick.
//! The hull turns in the middle as the original's 3D view (the console is not drawn:
//! `crate::design_scene::DesignScene::modern`; the right button still turns it). The window fills a [`View`] from
//! the screen's state, runs [`show`] once a frame and hands the [`Act`]s to
//! [`crate::design::Design::modern_act`], which runs the original's hot-spot code; the keys stay the screen's.

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Ui};

use crate::gui::ds::{self, Font, Glyph, Icon, IconKey, ListRow, PanelKind, Plate, Text, Weight};
use crate::starmap_modern::{self as sm};

/// What the panels ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// The top bar's (the starmap HUD's acts).
    Bar(sm::Act),
    /// A row of the list chosen (`AktTervMutato`).
    Row(usize),
    /// The previous (`true`) or next design (the keys' up and down).
    Key(bool),
    /// A console button (1..=8).
    Button(u32),
    /// The open popup's row picked (`dSelected`: a hull, 1 the sale, 1 or 2 the refit choice, a tech, 0 none).
    Pick(i32),
    /// The open popup closed without a pick.
    Close,
    /// A pod row chosen (`DesignAktPod`).
    Pod(usize),
    /// A pod's −/+ held this frame (pod, `true` = plus), or none.
    Hold(Option<(usize, bool)>),
    /// The name field (spot 0x56).
    Name,
}

/// A row of the design list.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub n: usize,
    pub name: String,
    /// The hull's name.
    pub kind: String,
    /// Obsolete (the design or its hull).
    pub off: bool,
}

/// A pod of the shown design.
#[derive(Debug, Clone, Default)]
pub struct Pod {
    pub k: usize,
    /// The slot's class and how many it takes of the fitted tech ("WEAPON · MAX 4").
    pub class: String,
    /// The fitted tech's name, empty for none.
    pub tech: String,
    pub count: i32,
    /// Its −/+ are on (editing, a tech fitted, more than one fits).
    pub counts: bool,
}

/// The open popup: the console button it belongs to (1 new, 4 delete, 5 OK, 0x54 a pod's techs) and its rows.
#[derive(Debug, Clone, Default)]
pub struct Popup {
    pub button: u32,
    pub rows: Vec<(i32, String)>,
}

/// Labels: the list's, the data's and the pods' headings, "pod", empty, choose, exit.
#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub designs: String,
    pub parts: String,
    pub pods: String,
    pub empty: String,
    pub choose: String,
    pub exit: String,
    pub name: String,
}

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    pub rows: Vec<Row>,
    pub pointer: usize,
    /// The data panel's pairs: the hull and the price at the left; the production time, the units and the tank
    /// capacity at the right.
    pub left: Vec<(String, String)>,
    pub right: Vec<(String, String)>,
    pub name: String,
    /// The name input's text and cursor while it is up.
    pub entry: Option<(String, usize)>,
    /// Speed, firepower, hit points.
    pub stats: Vec<(String, String)>,
    pub pods: Vec<Pod>,
    pub pod: usize,
    pub editing: bool,
    /// The four console buttons: hot-spot, label, on.
    pub buttons: [(u32, String, bool); 4],
    pub popup: Option<Popup>,
    pub labels: Labels,
    /// The hot-spots' tooltips (`GetString(5, 1, n)` by hot-spot id).
    pub tips: Vec<String>,
}

/// What stays between frames.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub bar: sm::State,
    /// The pod −/+ pressed and still held (the original's latch).
    count: Option<(usize, bool)>,
}

const EDGE: f32 = 24.0;
const LIST_W: f32 = 400.0;
const DATA_W: f32 = 780.0;
const PARTS_W: f32 = 540.0;
const TITLE: Font = Font::HEADING.tracking(0.16).upper(true);
/// A pod row's class (600 12 / 0.14 em upper case) and value (500 17 / 0.06 em).
const CLASS: Font = Font::LABEL.sized(12.0);
const VALUE: Font = Font::BODY.weight(Weight::Medium).tracking(0.06);

/// The original's buttons act on the press (`Mouse_Lenyom`, design.md §4).
fn pressed(r: &egui::Response) -> bool {
    r.hovered() && r.enabled() && r.ctx.input(|i| i.pointer.any_pressed())
}

fn tip(v: &View, id: u32) -> &str {
    v.tips.get(id as usize).map_or("", |s| s.as_str())
}

/// One frame of the screen; the acts in the order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut bar = Vec::new();
    sm::bar(ctx, &v.bar, &mut st.bar, &mut bar);
    let mut acts: Vec<Act> = bar.into_iter().map(Act::Bar).collect();
    let top = ctx.memory(|m| m.area_rect(egui::Id::new("smh-left"))).map_or(44.0, |r| r.bottom()) + 20.0;
    let screen = ctx.screen_rect();
    // The bottom row: the properties, the console buttons, the exit.
    let props = egui::Area::new(egui::Id::new("dmh-props")).anchor(Align2::LEFT_BOTTOM, vec2(EDGE, -EDGE)).fade_in(false).show(ctx, |ui| {
        ds::panel(ui, PanelKind::Glass, 0.92, 16.0, |ui| properties(ui, v, &mut acts));
    });
    let room = props.response.rect.top() - 16.0 - top;
    egui::Area::new(egui::Id::new("dmh-list")).fixed_pos(pos2(EDGE, top)).fade_in(false).show(ctx, |ui| {
        ds::panel(ui, PanelKind::Deep, 0.88, 16.0, |ui| list(ui, v, room - 32.0 - 34.0, &mut acts));
    });
    // The data panel between the list and the parts, as wide as fits (the board's 780).
    // Its right edge 16 from the parts (the board: 560..1340); the console centred under it.
    let mid = (screen.width() - 2.0 * EDGE - LIST_W - PARTS_W - 2.0 * 16.0).clamp(360.0, DATA_W);
    let right = screen.right() - EDGE - PARTS_W - 16.0;
    // The console under the data panel, kept between the properties and the exit (4:3).
    let cw = ctx.memory(|m| m.area_rect(egui::Id::new("dmh-console"))).map_or(560.0, |r| r.width());
    let centre = (right - mid / 2.0).max(EDGE + LIST_W + 16.0 + cw / 2.0).min(screen.right() - EDGE - 320.0 - 16.0 - cw / 2.0) - screen.center().x;
    egui::Area::new(egui::Id::new("dmh-data")).anchor(Align2::RIGHT_TOP, vec2(right - screen.right(), top)).fade_in(false).show(ctx, |ui| {
        ds::panel(ui, PanelKind::Glass, 0.92, 16.0, |ui| data(ui, v, mid - 32.0));
    });
    egui::Area::new(egui::Id::new("dmh-parts")).anchor(Align2::RIGHT_TOP, vec2(-EDGE, top)).fade_in(false).show(ctx, |ui| {
        ds::panel(ui, PanelKind::Glass, 0.92, 16.0, |ui| parts(ui, v, st, &mut acts));
    });
    // Too narrow for it between them (a large UI scale): above the exit instead.
    let fits = cw <= screen.width() - 2.0 * EDGE - LIST_W - 320.0 - 32.0;
    let (pivot, at) = if fits { (Align2::CENTER_BOTTOM, vec2(centre, -EDGE)) } else { (Align2::RIGHT_BOTTOM, vec2(-EDGE, -EDGE - 56.0 - 12.0)) };
    let console = egui::Area::new(egui::Id::new("dmh-console")).anchor(pivot, at).fade_in(false).show(ctx, |ui| console(ui, v, &mut acts));
    egui::Area::new(egui::Id::new("dmh-exit")).anchor(Align2::RIGHT_BOTTOM, vec2(-EDGE, -EDGE)).fade_in(false).show(ctx, |ui| {
        if pressed(&ui.add(Plate::new(&v.labels.exit).width(320.0).height(56.0))) {
            acts.push(Act::Bar(sm::Act::Screen(2)));
        }
    });
    // A console button's popup above its plate.
    match &v.popup {
        Some(p) if p.button != 0x54 => {
            let at = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("dmh-button", p.button)))).unwrap_or(console.response.rect);
            popup(ctx, p, pos2(at.left(), at.top() - 8.0), Align2::LEFT_BOTTOM, &mut acts);
        }
        _ => {}
    }
    // A press on another control closes the popup itself (`Design::modern_act`).
    if acts.iter().any(|a| matches!(a, Act::Row(_) | Act::Button(_) | Act::Pod(_) | Act::Name)) {
        acts.retain(|a| *a != Act::Close);
    }
    if !ctx.input(|i| i.pointer.any_down()) {
        st.count = None;
    }
    acts.push(Act::Hold(st.count));
    acts
}

/// The list's title and the designs with their hull, the chosen one selected, obsolete ones muted.
fn list(ui: &mut Ui, v: &View, height: f32, acts: &mut Vec<Act>) {
    let w = LIST_W - 32.0;
    ui.set_width(w);
    ui.spacing_mut().item_spacing.y = 8.0;
    Text::new(&v.labels.designs, TITLE).show(ui, vec2(w, 26.0));
    egui::ScrollArea::vertical().max_height(height.max(26.0 * 4.0)).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let k = ds::tokens();
        for r in &v.rows {
            // A list row (`.ig-list li`) with the hull at its right in 14, muted (silver on the chosen row).
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            let on = r.n == v.pointer;
            ds::row_back(ui, rect, ds::hover_t(ui.ctx(), resp.id, resp.hovered()), on);
            let inner = rect.shrink2(vec2(8.0, 0.0));
            let base = ds::fit::baseline(rect, Font::BODY);
            let kf = Font::DATA.sized(14.0).weight(Weight::Regular).tracking(0.06);
            let kw = Text::new(&r.kind, kf).right().color(if on { k.ink_silver } else { k.ink_muted }).baseline(base).paint(ui, inner.with_min_x(inner.right() - inner.width() * 0.45)).width;
            Text::new(&r.name, Font::BODY).color(if r.off { k.ink_muted } else { k.ink }).baseline(base).paint(ui, inner.with_max_x(inner.right() - kw - 12.0));
            let resp = ds::tip(resp, tip(v, 9));
            if pressed(&resp) {
                acts.push(Act::Row(r.n));
            }
            if r.n == v.pointer && ui.ctx().input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowDown)) {
                resp.scroll_to_me(None);
            }
        }
    });
}

/// The data panel's two tables (`.ig-table`): the hull and the price; the time, the units, the tank capacity.
fn data(ui: &mut Ui, v: &View, w: f32) {
    let k = ds::tokens();
    ui.set_width(w);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 40.0;
        let half = (w - 40.0) / 2.0;
        ui.allocate_ui_with_layout(vec2(half, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for (i, (label, value)) in v.left.iter().enumerate() {
                let (r, _) = ui.allocate_exact_size(vec2(half, 20.0), Sense::hover());
                let base = ds::fit::baseline(r, Font::DATA);
                Text::new(label, Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, r.with_max_x(r.left() + 98.0));
                Text::new(value, Font::DATA).color(if i == 0 { k.ink } else { k.info }).baseline(base).paint(ui, r.with_min_x(r.left() + 106.0));
            }
        });
        ui.allocate_ui_with_layout(vec2(half, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for (label, value) in &v.right {
                let (r, _) = ui.allocate_exact_size(vec2(half, 20.0), Sense::hover());
                let base = ds::fit::baseline(r, Font::DATA);
                let vw = Text::new(value, Font::DATA).right().color(k.info).baseline(base).paint(ui, r.with_min_x(r.right() - 90.0)).width;
                Text::new(label, Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, r.with_max_x(r.right() - vw - 12.0));
            }
        });
    });
}

/// The pods: number, class and tech, the count (its −/+ while editing), the chosen pod's tech list under it.
fn parts(ui: &mut Ui, v: &View, st: &mut State, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let w = PARTS_W - 32.0;
    ui.set_width(w);
    ui.spacing_mut().item_spacing.y = 10.0;
    let (r, _) = ui.allocate_exact_size(vec2(w, 26.0), Sense::hover());
    let base = ds::fit::baseline(r, TITLE);
    let count = format!("{} {}", v.pods.len(), v.labels.pods);
    let cw = Text::new(&count, Font::LABEL).right().color(k.ink_muted).baseline(base).paint(ui, r.with_min_x(r.right() - 120.0)).width;
    Text::new(&v.labels.parts, TITLE).baseline(base).paint(ui, r.with_max_x(r.right() - cw - 12.0));
    for p in &v.pods {
        let open = v.popup.as_ref().is_some_and(|x| x.button == 0x54) && v.pod == p.k;
        let (r, resp) = ui.allocate_exact_size(vec2(w, 52.0), Sense::click());
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
        let chosen = v.pod == p.k;
        let pt = ui.painter();
        if !p.tech.is_empty() || chosen {
            pt.rect_filled(r, ds::RADIUS_PLATE, k.glass_raised);
        }
        ds::rim(pt, r, ds::RADIUS_PLATE, if open || chosen { k.ink_silver } else { k.rim.lerp_to_gamma(k.rim_bright, hot) });
        // The number in a dark disc.
        let c = pos2(r.left() + 10.0 + 14.0, r.center().y);
        pt.circle_filled(c, 14.0, k.metal_shadow);
        pt.circle_stroke(c, 14.0, egui::Stroke::new(1.0f32, k.metal_dark));
        let n = p.k.to_string();
        Text::new(&n, Font::LABEL.tracking(0.0)).center().color(k.ink_silver).paint(ui, Rect::from_center_size(c, vec2(28.0, 28.0)));
        // The controls at the right: the count (−/+ while it can change), or "Choose" on an empty pod while editing.
        let ctl = Rect::from_min_max(pos2(r.right() - 10.0 - 156.0, r.top()), pos2(r.right() - 10.0, r.bottom()));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(ctl).layout(egui::Layout::right_to_left(egui::Align::Center)));
        c.spacing_mut().item_spacing.x = 8.0;
        let mut ctl_w = 0.0;
        if p.counts {
            let plus = ds::tip(ds::round_button(&mut c, Glyph::Icon(Icon::Plus), true, false), tip(v, 0x34));
            ds::lcd(&mut c, &p.count.to_string(), 22.0, 2);
            let minus = ds::tip(ds::round_button(&mut c, Glyph::Icon(Icon::Minus), true, false), tip(v, 0x33));
            if pressed(&minus) {
                st.count = Some((p.k, false));
            } else if pressed(&plus) {
                st.count = Some((p.k, true));
            }
            ctl_w = 156.0;
        } else if !p.tech.is_empty() {
            ctl_w = ds::lcd(&mut c, &p.count.to_string(), 22.0, 1).rect.width();
        } else if v.editing {
            let b = c.add(Plate::new(&v.labels.choose).height(34.0).font(Plate::FONT.sized(14.0).tracking(0.10)));
            ctl_w = b.rect.width();
            if pressed(&ds::tip(b, tip(v, 0x54))) {
                acts.push(Act::Pod(p.k));
            }
        }
        let texts = Rect::from_min_max(pos2(r.left() + 10.0 + 28.0 + 12.0, r.top() + 6.0), pos2(r.right() - 10.0 - ctl_w - 12.0, r.bottom() - 6.0));
        let (a, b) = texts.split_top_bottom_at_y(texts.top() + 16.0);
        Text::new(&p.class, CLASS).color(k.ink_muted).baseline(a.top() + 12.0).paint(ui, a);
        let (val, col) = if p.tech.is_empty() { (&v.labels.empty, k.ink_muted) } else { (&p.tech, k.ink.lerp_to_gamma(Color32::WHITE, hot)) };
        Text::new(val, VALUE).color(col).baseline(b.top() + 17.0).paint(ui, b);
        if pressed(&ds::tip(resp.clone(), tip(v, 0x54))) {
            acts.push(Act::Pod(p.k));
        }
        // The chosen pod's tech list, inline under it (`DesigndSelectPodMode2`).
        if let (true, Some(pop)) = (open, &v.popup) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(40.0);
                let at = ui.painter().add(egui::Shape::Noop);
                // Top-down inside the row's `horizontal` (else the rows stand side by side, only the first one seen).
                let r = egui::Frame::new().inner_margin(6).show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(w - 40.0 - 12.0);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (sel, name) in &pop.rows {
                            if pressed(&ui.add(ListRow::new(name).font(Font::BODY.sized(16.0)).selected(*sel != 0 && *name == p.tech))) {
                                acts.push(Act::Pick(*sel));
                            }
                        }
                    });
                });
                let rect = r.response.rect;
                ds::behind(ui, at, |p| {
                    p.rect_filled(rect, ds::RADIUS_PLATE, k.glass_deep);
                    ds::rim(p, rect, ds::RADIUS_PLATE, k.rim);
                });
                // A press anywhere else closes it.
                if ui.input(|i| i.pointer.any_pressed()) && !ui.rect_contains_pointer(rect) && !resp.hovered() {
                    acts.push(Act::Close);
                }
            });
        }
    }
}

/// The name (a field: a click starts the input), speed, firepower and hit points.
fn properties(ui: &mut Ui, v: &View, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let w = LIST_W - 32.0;
    ui.set_width(w);
    ui.spacing_mut().item_spacing.y = 10.0;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        Text::new(&v.labels.name, Font::LABEL).color(k.ink_muted).show(ui, vec2(w, 18.0));
        let (r, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
        let resp = resp.on_hover_cursor(egui::CursorIcon::Text);
        let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
        let editing = v.entry.is_some();
        let p = ui.painter();
        p.rect_filled(r, ds::RADIUS_PLATE, k.glass_deep);
        ds::rim(p, r, ds::RADIUS_PLATE, if editing { k.focus } else { k.rim_bright.lerp_to_gamma(k.ink_silver, hot * 0.5) });
        let f = Font::BODY.sized(18.0).weight(Weight::Medium).tracking(0.08);
        let inner = r.shrink2(vec2(10.0, 0.0));
        let (text, cursor) = v.entry.as_ref().map_or((v.name.as_str(), None), |(t, c)| (t.as_str(), Some(*c)));
        let fit = Text::new(text, f).baseline(ds::fit::baseline(r, f)).paint(ui, inner);
        if let Some(c) = cursor {
            // The caret after the `c`th letter, blinking.
            let before: String = text.chars().take(c).collect();
            let x = inner.left() + Text::new(&before, f).layout(ui.ctx(), inner.width()).width.min(inner.width());
            let on = (ui.input(|i| i.time) * 2.0).fract() < 0.6;
            if on {
                ui.painter().vline(x + 1.0, r.shrink(8.0).y_range(), egui::Stroke::new(1.5f32, k.focus));
            }
            ui.ctx().request_repaint();
        }
        let _ = fit;
        if pressed(&ds::tip(resp, tip(v, 0x56))) {
            acts.push(Act::Name);
        }
    });
    // Speed, firepower, hit points on one baseline (`.ig-table`, three pairs).
    let (r, _) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
    let base = ds::fit::baseline(r, Font::DATA.sized(18.0));
    let third = w / 3.0;
    for (i, (label, value)) in v.stats.iter().enumerate() {
        let cell = Rect::from_min_size(pos2(r.left() + third * i as f32, r.top()), vec2(third - 12.0, 22.0));
        let vw = Text::new(value, Font::DATA.sized(18.0)).right().baseline(base).paint(ui, cell.with_min_x(cell.right() - 48.0)).width;
        Text::new(label, Font::LABEL).shrink(9.0).color(k.ink_muted).baseline(base).paint(ui, cell.with_max_x(cell.right() - vw - 6.0));
    }
}

/// The console: the previous and next design keys around the four plates (`.act`: 48 high, 17).
fn console(ui: &mut Ui, v: &View, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let key = |ui: &mut Ui, i: Icon| ui.add(IconKey::icon(i).size(vec2(36.0, 52.0)));
        if pressed(&key(ui, Icon::ChevronLeft)) {
            acts.push(Act::Key(true));
        }
        for (id, label, on) in &v.buttons {
            let open = v.popup.as_ref().is_some_and(|p| p.button == *id);
            let b = ui.add_enabled(*on, Plate::new(label).width(70.0).height(48.0).primary(open || (*id == 5 && v.editing)).font(Plate::FONT.sized(17.0)));
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("dmh-button", *id)), b.rect));
            if pressed(&ds::tip(b, tip(v, *id))) {
                acts.push(if open { Act::Close } else { Act::Button(*id) });
            }
        }
        if pressed(&key(ui, Icon::ChevronRight)) {
            acts.push(Act::Key(false));
        }
    });
}

/// A console button's popup (the hulls of "new", the sale, the refit choice) as a deep panel; a pick sends its row,
/// a press elsewhere closes it.
fn popup(ctx: &egui::Context, p: &Popup, at: egui::Pos2, pivot: Align2, acts: &mut Vec<Act>) {
    let t = ds::open_t(ctx, egui::Id::new("dmh-popup"), true);
    let r = egui::Area::new(egui::Id::new("dmh-popup")).pivot(pivot).fixed_pos(at + vec2(0.0, (1.0 - t) * 8.0)).order(egui::Order::Foreground).constrain(true).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(t);
        ds::panel(ui, PanelKind::Deep, 0.96, 8.0, |ui| {
            ui.set_width(320.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::vertical().max_height(26.0 * 14.0).show(ui, |ui| {
                for (sel, name) in &p.rows {
                    if pressed(&ui.add(ListRow::new(name).font(Font::BODY.sized(16.0)))) {
                        acts.push(Act::Pick(*sel));
                    }
                }
            });
        });
    });
    let on_button = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("dmh-button", p.button)))).is_some_and(|b| ctx.pointer_latest_pos().is_some_and(|q| b.contains(q)));
    if ctx.input(|i| i.pointer.any_pressed()) && !r.response.contains_pointer() && !on_button {
        acts.push(Act::Close);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str) -> View {
        View {
            rows: (1..=9).map(|n| Row { n, name: format!("{name} {n}"), kind: "Destroyer".into(), off: n == 4 }).collect(),
            pointer: 2,
            left: vec![("Hull".into(), name.into()), ("Price".into(), "$22800".into())],
            right: vec![("Time".into(), "266".into()), ("Units".into(), "0".into())],
            name: name.into(),
            stats: vec![("Speed".into(), "8".into()), ("Firepower".into(), "60".into()), ("HP".into(), "10".into())],
            pods: (1..=4).map(|k| Pod { k, class: "Weapon".into(), tech: if k < 4 { name.into() } else { String::new() }, count: 2, counts: k == 3 }).collect(),
            pod: 3,
            editing: true,
            buttons: [(5, "OK".into(), true), (6, "Cancel".into(), true), (7, "Auto".into(), true), (8, "Clear".into(), true)],
            popup: Some(Popup { button: 0x54, rows: vec![(0, "-".into()), (0x12, "Laser".into())] }),
            ..Default::default()
        }
    }

    fn run(v: &View, st: &mut State, width: f32) -> (egui::Context, Vec<Act>) {
        let ctx = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb).ctx;
        ds::set_motion(false);
        let mut acts = Vec::new();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 1080.0))), ..Default::default() }, |ctx| acts = show(ctx, v, st));
        }
        (ctx, acts)
    }

    /// The pod's tech list stands its rows one under the other: each more row makes the parts panel a row taller.
    #[test]
    fn the_pod_list_shows_every_row() {
        let height = |n: usize| {
            let mut v = view("Laser");
            v.popup = Some(Popup { button: 0x54, rows: (0..n as i32).map(|t| (t, format!("Tech {t}"))).collect() });
            let (ctx, _) = run(&v, &mut State::default(), 1920.0);
            ctx.memory(|m| m.area_rect(egui::Id::new("dmh-parts"))).unwrap().height()
        };
        assert!(height(3) - height(1) > 2.0 * 20.0, "{} {}", height(1), height(3));
    }

    /// The panels keep the board's widths with long names, at 4:3 and 21:9 too, and ask for nothing untouched.
    #[test]
    fn the_panels_keep_their_places() {
        for (w, name) in [(1380.0, "Fighter"), (1440.0, "Fighter"), (1920.0, &*"Very long translated design name ".repeat(4)), (2520.0, "Destroyer")] {
            let (ctx, acts) = run(&view(name), &mut State::default(), w);
            assert_eq!(acts, vec![Act::Hold(None)], "{w}");
            let r = |id: &str| ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap();
            let (l, d, p, pr, c) = (r("dmh-list"), r("dmh-data"), r("dmh-parts"), r("dmh-props"), r("dmh-console"));
            assert!((l.width() - LIST_W).abs() < 1.0 && (p.width() - PARTS_W).abs() < 1.0 && (pr.width() - LIST_W).abs() < 1.0, "{w}: {l:?} {p:?}");
            assert!(l.right() < d.left() && d.right() < p.left(), "{w}: {l:?} {d:?} {p:?}");
            assert!(l.bottom() < pr.top() && (c.left() > pr.right() - 1.0 || c.bottom() < pr.top()), "{w}: {c:?}");
        }
    }
}
