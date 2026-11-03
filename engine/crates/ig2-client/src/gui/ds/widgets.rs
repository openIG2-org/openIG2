//! The kit's widgets (ADR 0073 §4), `bundle.css` and the boards' recurring parts drawn with the painter. Each has its
//! idle, hover (a ~120 ms fade), pressed, selected (gliding where several share one mark), disabled (`ui.add_enabled`
//! or `Ui::disable`) and keyboard-focus state; a page with its own navigation passes its focus in (`focused`).
//! Labels go through [`super::fit`]: a widget's size is the caller's or its design's, never its text's beyond that.

use super::fit::{baseline, Text};
use super::{edge_line, focus_mark, glass, hover_t, hgrad, lcd_window, plate_fill, radial, relief, rim, shadow, tokens, vgrad, Font, Weight, GLIDE_TIME, RADIUS_PANEL, RADIUS_PLATE, SPACE};
use egui::{Align, Color32, Id, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};

/// The common states of a widget's response.
struct Look {
    hover: f32,
    down: bool,
    on: bool,
    focus: bool,
}

fn look(ui: &Ui, r: &Response, focused: bool) -> Look {
    let on = ui.is_enabled();
    let (hovered, down) = FORCED.with(std::cell::Cell::get).unwrap_or((r.hovered(), r.is_pointer_button_down_on()));
    Look { hover: hover_t(ui.ctx(), r.id, on && hovered), down: on && down, on, focus: focused || r.has_focus() }
}

thread_local! {
    /// The gallery's forced pointer state (hovered, pressed) for the widgets drawn in [`forced`].
    static FORCED: std::cell::Cell<Option<(bool, bool)>> = const { std::cell::Cell::new(None) };
}

/// `add`'s kit widgets drawn as if hovered (and pressed): the gallery's way of showing every state at once.
pub fn forced<R>(hovered: bool, pressed: bool, add: impl FnOnce() -> R) -> R {
    FORCED.with(|f| f.set(Some((hovered, pressed))));
    let r = add();
    FORCED.with(|f| f.set(None));
    r
}

/// The text colour of a widget's state.
fn ink(on: bool, c: Color32) -> Color32 {
    if on { c } else { tokens().ink_muted }
}

/// A glide of the selection mark `id` to `to` (instant on the first frame).
fn glide(ui: &Ui, id: Id, to: f32) -> f32 {
    ui.ctx().animate_value_with_time(id.with("glide"), to, super::anim(GLIDE_TIME))
}

/// The hover rim (`inset 0 0 0 1px --rim-bright`) at `t`.
fn hover_rim(ui: &Ui, rect: Rect, r: u8, t: f32) {
    if t > 0.0 {
        rim(ui.painter(), rect, r, tokens().rim_bright.gamma_multiply(t));
    }
}

/// A widget's whole text as its tooltip where its label was cut.
fn tip_if(r: Response, cut: bool, text: &str) -> Response {
    if cut { r.on_hover_text(text) } else { r }
}

// ---- Plates and menu items ----

/// The command plate (`.ig-plate`): a bevelled blue slab, 44 high, its label 500 20 / 0.16 em; `primary` lit
/// (`plate-high`, the board's "Apply"), a second line under it (`.stack .sub`).
pub struct Plate<'a> {
    text: &'a str,
    sub: Option<&'a str>,
    primary: bool,
    focused: bool,
    size: Vec2,
    font: Font,
}

impl<'a> Plate<'a> {
    pub const FONT: Font = Font { size: 20.0, line: 24.0, weight: Weight::Medium, tracking: 0.16, upper: false, lcd: false };
    pub fn new(text: &'a str) -> Self {
        Self { text, sub: None, primary: false, focused: false, size: Vec2::new(0.0, 44.0), font: Self::FONT }
    }
    pub fn primary(mut self, on: bool) -> Self {
        self.primary = on;
        self
    }
    pub fn sub(mut self, s: &'a str) -> Self {
        self.sub = Some(s);
        self
    }
    pub fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }
    /// The width at least; a longer label widens it while the row has room, then is shrunk and cut.
    pub fn width(mut self, w: f32) -> Self {
        self.size.x = w;
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.size.y = h;
        self
    }
    pub fn font(mut self, f: Font) -> Self {
        self.font = f;
        self
    }
}

impl Widget for Plate<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let natural = Text::new(self.text, self.font).layout(ui.ctx(), f32::INFINITY).width + 2.0 * SPACE[3];
        // At least the set width (CSS `min-width`), wider for the label while the row has room, then the label is cut.
        let w = natural.max(self.size.x).min(ui.available_width().max(self.size.x));
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, self.size.y), Sense::click());
        let l = look(ui, &resp, self.focused);
        let p = ui.painter();
        let k = tokens();
        plate_fill(p, rect, RADIUS_PANEL, self.primary || l.down);
        hover_rim(ui, rect, RADIUS_PANEL, l.hover);
        if l.focus {
            focus_mark(p, rect, RADIUS_PANEL as f32, 2.0);
        }
        let inner = rect.shrink2(Vec2::new(SPACE[2], 0.0));
        let cut = match self.sub {
            None => Text::new(self.text, self.font).center().shrink(self.font.size * 0.75).color(ink(l.on, k.ink)).paint_only(ui, inner).cut,
            Some(sub) => {
                let sf = Font::LABEL.upper(false);
                let (top, bottom) = inner.split_top_bottom_at_y(rect.center().y + 3.0);
                let a = Text::new(self.text, self.font).center().shrink(self.font.size * 0.75).color(ink(l.on, k.ink)).baseline(top.bottom() - 2.0).paint_only(ui, top).cut;
                let b = Text::new(sub, sf).center().color(ink(l.on, k.ink_silver)).baseline(bottom.top() + sf.cap() + 4.0).paint_only(ui, bottom).cut;
                a || b
            }
        };
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, l.on, self.text));
        tip_if(resp, cut, self.text)
    }
}

/// A menu's or a rail's look: the label's style and the item's height.
#[derive(Debug, Clone, Copy)]
pub struct MenuStyle {
    pub font: Font,
    pub height: f32,
}

impl MenuStyle {
    /// The main menu's items (`.mm-item`: 500 21 / 0.14 em upper case, 48 high).
    pub const MENU: MenuStyle = MenuStyle { font: Font { size: 21.0, line: 24.0, weight: Weight::Medium, tracking: 0.14, upper: true, lcd: false }, height: 48.0 };
    /// A page's side rail of tabs (`.rail button`: 19 / 0.12 em, 52 high).
    pub const RAIL: MenuStyle = MenuStyle { font: Font { size: 19.0, line: 22.0, weight: Weight::Medium, tracking: 0.12, upper: true, lcd: false }, height: 52.0 };
}

/// One entry of a [`menu`]: its label, a second line (`.sub`, the save's name and date), muted (`.off`: shown, not
/// available).
#[derive(Debug, Clone, Copy, Default)]
pub struct MenuEntry<'a> {
    pub text: &'a str,
    pub sub: Option<&'a str>,
    pub off: bool,
}

impl<'a> MenuEntry<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, sub: None, off: false }
    }
}

/// A column of menu items (`.mm-item`, `.rail`) 4 points apart, the selected one on a plate that glides to it; the
/// keyboard focus `focus` marked. Gives the clicked entry.
pub fn menu(ui: &mut Ui, id: impl std::hash::Hash, style: MenuStyle, entries: &[MenuEntry], selected: Option<usize>, focus: Option<usize>) -> Option<usize> {
    let id = ui.make_persistent_id(id);
    let k = tokens();
    let w = ui.available_width();
    let gap = SPACE[0];
    let sf = Font::LABEL.weight(Weight::Medium).tracking(0.1).upper(false);
    // A second line fits the item's height while it can (the board's 48: 21 + 6 + 16).
    let heights: Vec<f32> = entries.iter().map(|e| if e.sub.is_some() { style.height.max(style.font.size + 6.0 + sf.line + 4.0) } else { style.height }).collect();
    let (whole, _) = ui.allocate_exact_size(Vec2::new(w, heights.iter().sum::<f32>() + gap * entries.len().saturating_sub(1) as f32), Sense::hover());
    let tops: Vec<f32> = heights.iter().scan(whole.top(), |y, h| {
        let t = *y;
        *y += h + gap;
        Some(t)
    }).collect();
    // The plate, under the items: it glides between them.
    if let Some(s) = selected.filter(|&s| s < entries.len()) {
        let (y, h) = (glide(ui, id.with("y"), tops[s]), glide(ui, id.with("h"), heights[s]));
        plate_fill(ui.painter(), Rect::from_min_size(Pos2::new(whole.left(), y), Vec2::new(w, h)), RADIUS_PLATE, false);
    }
    let mut clicked = None;
    for (i, e) in entries.iter().enumerate() {
        let rect = Rect::from_min_size(Pos2::new(whole.left(), tops[i]), Vec2::new(w, heights[i]));
        let resp = ui.interact(rect, id.with(i), Sense::click());
        let l = look(ui, &resp, focus == Some(i));
        if l.down && selected != Some(i) {
            ui.painter().rect_filled(rect, RADIUS_PLATE, k.plate_low);
        }
        hover_rim(ui, rect, RADIUS_PLATE, l.hover * (selected != Some(i)) as u8 as f32);
        if l.focus {
            focus_mark(ui.painter(), rect, RADIUS_PLATE as f32, 2.0);
        }
        let inner = rect.shrink2(Vec2::new(20.0, 0.0));
        let colour = if e.off || !l.on { k.ink_muted } else { k.ink };
        let cut = match e.sub {
            None => Text::new(e.text, style.font).color(colour).paint_only(ui, inner).cut,
            Some(sub) => {
                // The two lines centred together: the label's capitals, 6 points, the second line's box.
                let base = inner.top() + (inner.height() - style.font.cap() - 6.0 - sf.line) / 2.0 + style.font.cap();
                let a = Text::new(e.text, style.font).color(colour).baseline(base).paint_only(ui, inner).cut;
                let b = Text::new(sub, sf).color(k.info).baseline(base + 6.0 + (sf.line + sf.cap()) / 2.0).paint_only(ui, inner).cut;
                a || b
            }
        };
        let resp = tip_if(resp, cut, e.text);
        if resp.clicked() && l.on {
            clicked = Some(i);
        }
    }
    clicked
}

/// A horizontal strip of tabs (`.tab`: 36 high, 500 16 / 0.12 em; the chosen one on a gliding plate). Too many for
/// the width share it and cut their labels. Gives the clicked tab.
pub fn tabs(ui: &mut Ui, id: impl std::hash::Hash, labels: &[&str], selected: usize) -> Option<usize> {
    const FONT: Font = Font { size: 16.0, line: 20.0, weight: Weight::Medium, tracking: 0.12, upper: false, lcd: false };
    let id = ui.make_persistent_id(id);
    let k = tokens();
    let mut widths: Vec<f32> = labels.iter().map(|s| Text::new(s, FONT).layout(ui.ctx(), f32::INFINITY).width + 36.0).collect();
    let gap = SPACE[0];
    let avail = ui.available_width() - gap * labels.len().saturating_sub(1) as f32;
    let total: f32 = widths.iter().sum();
    if total > avail {
        widths.iter_mut().for_each(|w| *w *= avail / total);
    }
    let (whole, _) = ui.allocate_exact_size(Vec2::new(widths.iter().sum::<f32>() + gap * labels.len().saturating_sub(1) as f32, 36.0), Sense::hover());
    let lefts: Vec<f32> = widths.iter().scan(whole.left(), |x, w| {
        let l = *x;
        *x += w + gap;
        Some(l)
    }).collect();
    if selected < labels.len() {
        let (x, w) = (glide(ui, id.with("x"), lefts[selected]), glide(ui, id.with("w"), widths[selected]));
        plate_fill(ui.painter(), Rect::from_min_size(Pos2::new(x, whole.top()), Vec2::new(w, 36.0)), RADIUS_PLATE, false);
    }
    let mut clicked = None;
    for (i, s) in labels.iter().enumerate() {
        let rect = Rect::from_min_size(Pos2::new(lefts[i], whole.top()), Vec2::new(widths[i], 36.0));
        let resp = ui.interact(rect, id.with(i), Sense::click());
        let l = look(ui, &resp, false);
        let c = if i == selected { k.ink } else { k.ink_muted.lerp_to_gamma(k.ink_silver, l.hover) };
        let cut = Text::new(s, FONT).center().color(ink(l.on, c)).paint_only(ui, rect.shrink2(Vec2::new(8.0, 0.0))).cut;
        if tip_if(resp, cut, s).clicked() && l.on {
            clicked = Some(i);
        }
    }
    clicked
}

// ---- Icons and icon buttons ----

/// The line icons the kit draws (the boards' lucide strokes, 2 points wide in a 24-unit box).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ChevronUp,
    Pause,
    Play,
    Fast,
    Fastest,
    Close,
    Check,
    Plus,
    Minus,
    Menu,
}

impl Icon {
    /// The icon in `rect` in `c`.
    pub fn paint(self, p: &egui::Painter, rect: Rect, c: Color32) {
        let s = rect.width().min(rect.height()) / 24.0;
        let at = |x: f32, y: f32| rect.center() + Vec2::new(x - 12.0, y - 12.0) * s;
        let st = Stroke::new(2.0 * s.max(0.75), c);
        let poly = |pts: &[(f32, f32)]| p.add(egui::Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), st));
        let chev = |dx: f32| poly(&[(9.0 + dx, 18.0), (15.0 + dx, 12.0), (9.0 + dx, 6.0)]);
        match self {
            Icon::ChevronLeft => _ = poly(&[(15.0, 18.0), (9.0, 12.0), (15.0, 6.0)]),
            Icon::ChevronRight | Icon::Play => _ = chev(0.0),
            Icon::ChevronDown => _ = poly(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)]),
            Icon::ChevronUp => _ = poly(&[(6.0, 15.0), (12.0, 9.0), (18.0, 15.0)]),
            Icon::Fast => {
                poly(&[(6.0, 17.0), (11.0, 12.0), (6.0, 7.0)]);
                poly(&[(13.0, 17.0), (18.0, 12.0), (13.0, 7.0)]);
            }
            Icon::Fastest => {
                for x in [3.0, 10.0, 17.0] {
                    poly(&[(x, 17.0), (x + 5.0, 12.0), (x, 7.0)]);
                }
            }
            Icon::Pause => {
                for x in [6.0, 14.0] {
                    p.rect_stroke(Rect::from_min_max(at(x, 4.0), at(x + 4.0, 20.0)), s, st, egui::StrokeKind::Middle);
                }
            }
            Icon::Close => {
                poly(&[(18.0, 6.0), (6.0, 18.0)]);
                poly(&[(6.0, 6.0), (18.0, 18.0)]);
            }
            Icon::Check => _ = poly(&[(20.0, 6.0), (9.0, 17.0), (4.0, 12.0)]),
            Icon::Plus => {
                poly(&[(5.0, 12.0), (19.0, 12.0)]);
                poly(&[(12.0, 5.0), (12.0, 19.0)]);
            }
            Icon::Minus => _ = poly(&[(5.0, 12.0), (19.0, 12.0)]),
            Icon::Menu => {
                for y in [6.0, 12.0, 18.0] {
                    poly(&[(4.0, y), (20.0, y)]);
                }
            }
        }
    }
}

/// What an [`IconKey`] shows: one of the kit's icons or the page's own drawing (`(painter, rect, colour)`).
pub enum Glyph<'a> {
    Icon(Icon),
    Text(&'a str),
    Custom(&'a dyn Fn(&egui::Painter, Rect, Color32)),
}

/// A metal key (`.ig-key`, `.ig-speed`: 34 × 28): its icon dark on the metal, silver under the pointer; `on`
/// pressed in and lit in LCD ink with a glow (the running speed).
pub struct IconKey<'a> {
    glyph: Glyph<'a>,
    on: bool,
    size: Vec2,
    tip: Option<&'a str>,
}

impl<'a> IconKey<'a> {
    pub fn new(glyph: Glyph<'a>) -> Self {
        Self { glyph, on: false, size: Vec2::new(34.0, 28.0), tip: None }
    }
    pub fn icon(i: Icon) -> Self {
        Self::new(Glyph::Icon(i))
    }
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }
    pub fn size(mut self, s: Vec2) -> Self {
        self.size = s;
        self
    }
    /// Its tooltip (an icon has no label).
    pub fn tip(mut self, t: &'a str) -> Self {
        self.tip = Some(t);
        self
    }
}

impl Widget for IconKey<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, resp) = ui.allocate_exact_size(self.size, Sense::click());
        let l = look(ui, &resp, false);
        let (p, k) = (ui.painter(), tokens());
        let lit = self.on || l.down;
        if lit {
            shadow(p, rect, 3, 0, 6, 0, k.lcd_ink.gamma_multiply(0.45 * self.on as u8 as f32));
            vgrad(p, rect, 3, &[(0.0, k.metal_shadow), (1.0, k.lcd)]);
            vgrad(p, rect, 3, &[(0.0, Color32::BLACK), (4.0 / rect.height(), Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
        } else {
            vgrad(p, rect, 3, &[(0.0, k.metal_light), (0.45, k.metal), (1.0, k.metal_dark)]);
            edge_line(p, rect, 3.0, true, k.metal_shine);
            edge_line(p, rect, 3.0, false, k.metal_shadow);
        }
        let c = if !l.on { k.metal_dark } else if self.on { k.lcd_ink } else { k.metal_shadow.lerp_to_gamma(k.ink_silver, l.hover) };
        let g = Rect::from_center_size(rect.center(), Vec2::splat(18.0));
        match self.glyph {
            Glyph::Icon(i) => i.paint(p, g, c),
            Glyph::Text(t) => _ = Text::new(t, Font::LABEL).center().color(c).paint_only(ui, rect.shrink(2.0)),
            Glyph::Custom(f) => f(p, g, c),
        }
        if l.focus {
            focus_mark(p, rect, 3.0, 1.0);
        }
        match self.tip {
            Some(t) => resp.on_hover_text(t),
            None => resp,
        }
    }
}

/// A segmented metal block holding keys (`.ig-speeds`: 2 points of padding and gap on the darkest metal).
pub fn key_group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let k = tokens();
    let at = ui.painter().add(egui::Shape::Noop);
    let r = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
        egui::Frame::new().inner_margin(2).show(ui, add).inner
    });
    let rect = r.response.rect;
    ui.painter().set(at, egui::epaint::RectShape::new(rect, RADIUS_PLATE, k.metal_shadow, Stroke::new(1.0f32, k.metal_dark), egui::StrokeKind::Inside));
    r.inner
}

/// The round rust button in its metal ring (`.ig-round`, 72 across; `small` 34, the dark one with a red glyph);
/// `active` lit.
pub fn round_button(ui: &mut Ui, glyph: Glyph, small: bool, active: bool) -> Response {
    round_button_sized(ui, glyph, if small { 34.0 } else { 72.0 }, active)
}

/// [`round_button`] `d` points across (the boards' 60-point action buttons); below 48 the small dark one.
pub fn round_button_sized(ui: &mut Ui, glyph: Glyph, d: f32, active: bool) -> Response {
    let small = d < 48.0;
    let ring = if small { 3.0 } else { 5.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(d + 2.0 * (ring + 1.0)), Sense::click());
    let l = look(ui, &resp, false);
    let (p, k) = (ui.painter(), tokens());
    let c = rect.center();
    shadow(p, Rect::from_center_size(c + Vec2::new(0.0, 3.0), Vec2::splat(d + 2.0 * ring)), 255, 0, 8, 0, Color32::from_black_alpha(204));
    p.circle_filled(c, d / 2.0 + ring + 1.0, k.metal_shadow);
    p.circle_filled(c, d / 2.0 + ring, if l.hover > 0.5 { k.metal_light } else { k.metal });
    let disc = Rect::from_center_size(c, Vec2::splat(d));
    let r = (d / 2.0) as u8;
    match (small, active || l.down) {
        (true, _) => radial(p, disc, r, [0.4, 0.3], &[(0.0, k.metal_dark), (1.0, k.metal_shadow)]),
        (false, false) => radial(p, disc, r, [0.38, 0.3], &[(0.0, k.rust_light), (0.45, k.rust), (1.0, k.rust_dark)]),
        (false, true) => radial(p, disc, r, [0.38, 0.3], &[(0.0, k.metal_shine), (0.5, k.rust_light), (1.0, k.rust)]),
    }
    relief(p, disc, r);
    let col = if small { k.signal_red } else if active { k.ink } else { k.metal_shadow };
    let g = Rect::from_center_size(c, Vec2::splat(if small { 18.0 } else { 28.0 }));
    match glyph {
        Glyph::Icon(i) => i.paint(p, g, ink(l.on, col)),
        Glyph::Text(t) => _ = Text::new(t, Font { size: 18.0, line: 20.0, weight: Weight::SemiBold, tracking: 0.04, upper: false, lcd: false }).center().color(ink(l.on, col)).paint_only(ui, disc.shrink(6.0)),
        Glyph::Custom(f) => f(p, g, ink(l.on, col)),
    }
    if l.focus {
        p.circle_stroke(c, d / 2.0 + ring + 8.0, Stroke::new(2.0f32, k.focus));
    }
    resp
}

// ---- Rows ----

/// A list row's background in its state (`.ig-list li`): the hover rim, the selected row's fading white sweep and
/// silver rim. For a page's own rows (a ship card, a planet line).
pub fn row_back(ui: &Ui, rect: Rect, hover: f32, selected: bool) {
    let k = tokens();
    if selected {
        hgrad(ui.painter(), rect, RADIUS_PLATE, &[(0.0, Color32::from_white_alpha(46)), (1.0, Color32::from_white_alpha(5))]);
        rim(ui.painter(), rect, RADIUS_PLATE, k.ink_silver);
    } else {
        hover_rim(ui, rect, RADIUS_PLATE, hover);
    }
}

/// A list row (`.ig-list li`: 26 high, 17-point text 8 from the edge), a value at its right (data style, info blue).
pub struct ListRow<'a> {
    text: &'a str,
    value: Option<&'a str>,
    selected: bool,
    off: bool,
    focused: bool,
    height: f32,
    font: Font,
}

impl<'a> ListRow<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, value: None, selected: false, off: false, focused: false, height: 26.0, font: Font::BODY }
    }
    pub fn value(mut self, v: &'a str) -> Self {
        self.value = Some(v);
        self
    }
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }
    /// Shown muted: not available (`.unavailable`).
    pub fn off(mut self, on: bool) -> Self {
        self.off = on;
        self
    }
    pub fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    pub fn font(mut self, f: Font) -> Self {
        self.font = f;
        self
    }
}

impl Widget for ListRow<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), self.height), Sense::click());
        let l = look(ui, &resp, self.focused);
        let k = tokens();
        row_back(ui, rect, l.hover, self.selected);
        if l.focus {
            focus_mark(ui.painter(), rect, RADIUS_PLATE as f32, 1.0);
        }
        let inner = rect.shrink2(Vec2::new(SPACE[1], 0.0));
        let base = baseline(rect, self.font);
        let mut text_box = inner;
        let mut cut = false;
        if let Some(v) = self.value {
            let vf = Text::new(v, Font::DATA).right().color(ink(l.on && !self.off, k.info)).baseline(base);
            let w = vf.layout(ui.ctx(), inner.width() * 0.45).width;
            cut |= vf.paint_only(ui, inner.with_min_x(inner.right() - inner.width() * 0.45)).cut;
            text_box = inner.with_max_x(inner.right() - w - SPACE[2]);
        }
        cut |= Text::new(self.text, self.font).color(ink(l.on && !self.off, k.ink)).baseline(base).paint_only(ui, text_box).cut;
        tip_if(resp, cut, self.text)
    }
}

/// A settings row (`.orow`: 50 high, its 16-point label at the left, its control at the right, the label cut to the
/// room the control leaves); `focused` lit (the keyboard's row), `enabled` false muted.
pub fn option_row<R>(ui: &mut Ui, label: &str, focused: bool, enabled: bool, control: impl FnOnce(&mut Ui) -> R) -> (Response, R) {
    const FONT: Font = Font { size: 16.0, line: 20.0, weight: Weight::Regular, tracking: 0.05, upper: false, lcd: false };
    let k = tokens();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 50.0), Sense::click());
    let hover = hover_t(ui.ctx(), resp.id, resp.hovered() && enabled);
    if focused {
        ui.painter().rect_filled(rect, RADIUS_PLATE, k.rim_bright.gamma_multiply(0.12));
    }
    hover_rim(ui, rect, RADIUS_PLATE, if focused { 1.0 } else { hover * 0.6 });
    let inner = rect.shrink2(Vec2::new(SPACE[1], 0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::right_to_left(Align::Center)));
    if !enabled {
        child.disable();
    }
    let r = control(&mut child);
    let used = child.min_rect().width();
    let cut = Text::new(label, FONT).color(if enabled { k.ink } else { k.ink_muted }).paint_only(ui, inner.with_max_x(inner.right() - used - SPACE[2])).cut;
    (tip_if(resp, cut, label), r)
}

// ---- Toggles, segments, steppers, sliders ----

const SEG_FONT: Font = Font { size: 14.0, line: 16.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
const TOGGLE_FONT: Font = Font { size: 13.0, line: 16.0, weight: Weight::SemiBold, tracking: 0.14, upper: true, lcd: false };

/// The recessed track of segments, toggles and steppers (`.tg`, `.seg`, `.step`).
fn well(ui: &Ui, rect: Rect) {
    let k = tokens();
    ui.painter().rect_filled(rect, RADIUS_PLATE, k.glass_deep);
    rim(ui.painter(), rect, RADIUS_PLATE, k.rim);
}

/// Segments in a well, the chosen one on a gliding lit block; `width` stretches them into equal columns (a wide row).
fn segments(ui: &mut Ui, labels: &[&str], selected: usize, font: Font, width: Option<f32>, pad: f32) -> (Response, Option<usize>) {
    let k = tokens();
    let natural: Vec<f32> = labels.iter().map(|s| (Text::new(s, font).layout(ui.ctx(), f32::INFINITY).width + 2.0 * pad).max(46.0)).collect();
    let n = labels.len().max(1) as f32;
    let widths: Vec<f32> = match width {
        Some(w) => vec![((w - 6.0 - 3.0 * (n - 1.0)) / n).max(10.0); labels.len()],
        None => natural,
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(widths.iter().sum::<f32>() + 6.0 + 3.0 * (n - 1.0), 36.0), Sense::hover());
    well(ui, rect);
    let lefts: Vec<f32> = widths.iter().scan(rect.left() + 3.0, |x, w| {
        let l = *x;
        *x += w + 3.0;
        Some(l)
    }).collect();
    let id = resp.id;
    if selected < labels.len() {
        let x = glide(ui, id.with("x"), lefts[selected]);
        let w = glide(ui, id.with("w"), widths[selected]);
        ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x, rect.top() + 3.0), Vec2::new(w, 30.0)), 3, if ui.is_enabled() { k.plate_high } else { k.plate });
    }
    let mut clicked = None;
    for (i, s) in labels.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(lefts[i], rect.top() + 3.0), Vec2::new(widths[i], 30.0));
        let sr = ui.interact(r, id.with(i), Sense::click());
        let l = look(ui, &sr, false);
        let c = if i == selected { k.ink } else { k.ink_muted.lerp_to_gamma(k.ink_silver, l.hover) };
        let cut = Text::new(s, font).center().color(ink(l.on, c)).paint_only(ui, r.shrink2(Vec2::new(4.0, 0.0))).cut;
        if tip_if(sr, cut, s).clicked() && l.on {
            clicked = Some(i);
        }
    }
    (resp, clicked)
}

/// A segmented choice (`.seg`); `width` for a wide row's equal columns. Sets `selected` on a click.
pub fn segmented(ui: &mut Ui, selected: &mut usize, labels: &[&str], width: Option<f32>) -> Response {
    let (mut resp, c) = segments(ui, labels, *selected, SEG_FONT, width, 10.0);
    if let Some(c) = c.filter(|c| c != selected) {
        *selected = c;
        resp.mark_changed();
    }
    resp
}

/// An off / on toggle with its two words (`.tg`: "KI" "BE"); the lit block glides.
pub fn toggle(ui: &mut Ui, on: &mut bool, off_label: &str, on_label: &str) -> Response {
    let (mut resp, c) = segments(ui, &[off_label, on_label], *on as usize, TOGGLE_FONT, None, 8.0);
    if let Some(c) = c.filter(|&c| (c == 1) != *on) {
        *on = c == 1;
        resp.mark_changed();
    }
    resp
}

/// A stepper (`.step`: ‹ value › in a well, 32 + `value_width` + 32 by 36). Gives −1 or +1 for a click on an arrow.
pub fn stepper(ui: &mut Ui, value: &str, value_width: f32) -> (Response, i32) {
    let k = tokens();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(64.0 + value_width, 36.0), Sense::hover());
    well(ui, rect);
    let mut step = 0;
    for (dir, r, icon) in [(-1, rect.with_max_x(rect.left() + 32.0), Icon::ChevronLeft), (1, rect.with_min_x(rect.right() - 32.0), Icon::ChevronRight)] {
        let b = ui.interact(r, resp.id.with(dir), Sense::click());
        let l = look(ui, &b, false);
        if l.down {
            ui.painter().rect_filled(r.shrink(3.0), 3, k.plate);
        }
        icon.paint(ui.painter(), Rect::from_center_size(r.center(), Vec2::splat(16.0)), ink(l.on, k.ink_silver.lerp_to_gamma(k.ink, l.hover)));
        if b.clicked() && l.on {
            step = dir;
        }
    }
    let vf = Font { size: 15.0, line: 18.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
    let cut = Text::new(value, vf).center().shrink(12.0).color(ink(ui.is_enabled(), k.info)).paint_only(ui, rect.shrink2(Vec2::new(32.0, 0.0))).cut;
    (tip_if(resp, cut, value), step)
}

/// A slider (`.sl`: a 150 × 8 track filled in the lit plate colour, a metal knob 12 × 20, its value as a small LCD
/// readout at the right). `value` 0..1; a click or drag on the track sets it.
pub fn slider(ui: &mut Ui, value: &mut f32, readout: &str) -> Response {
    let k = tokens();
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(150.0 + 12.0 + 52.0, 24.0), Sense::click_and_drag());
    let track = Rect::from_min_size(Pos2::new(rect.left() + 6.0, rect.center().y - 4.0), Vec2::new(150.0 - 12.0, 8.0));
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let v = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
        if v != *value {
            *value = v;
            resp.mark_changed();
        }
    }
    let l = look(ui, &resp, false);
    let p = ui.painter();
    p.rect_filled(track, 4, k.glass_deep);
    let x = track.left() + track.width() * value.clamp(0.0, 1.0);
    p.rect_filled(track.with_max_x(x), 4, if l.on { k.plate_high } else { k.plate });
    rim(p, track, 4, k.rim);
    let knob = Rect::from_center_size(Pos2::new(x, track.center().y), Vec2::new(12.0, 20.0));
    shadow(p, knob.translate(Vec2::new(0.0, 1.0)), 3, 0, 3, 0, Color32::BLACK);
    vgrad(p, knob, 3, &[(0.0, k.metal_light.lerp_to_gamma(k.metal_shine, l.hover)), (1.0, k.metal_dark)]);
    edge_line(p, knob, 3.0, true, k.metal_shine);
    if l.focus {
        focus_mark(p, knob, 3.0, 2.0);
    }
    let rf = Font { size: 18.0, line: 20.0, ..Font::READOUT }.tracking(0.1);
    Text::new(readout, rf).right().color(if l.on { k.lcd_ink } else { k.ink_muted }).paint_only(ui, rect.with_min_x(rect.right() - 52.0));
    resp
}

/// A key cap (`.key`: metal, 700 14, at least 72 × 32); `listening` the waiting-for-a-key look.
pub fn key_cap(ui: &mut Ui, text: &str, listening: bool) -> Response {
    let k = tokens();
    let f = Font { size: 14.0, line: 16.0, weight: Weight::SemiBold, tracking: 0.08, upper: false, lcd: false };
    let w = (Text::new(text, f).layout(ui.ctx(), f32::INFINITY).width + 20.0).clamp(72.0, 160.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
    let l = look(ui, &resp, false);
    let p = ui.painter();
    if listening {
        p.rect_filled(rect, RADIUS_PLATE, k.glass_deep);
        rim(p, rect, RADIUS_PLATE, k.focus);
    } else {
        vgrad(p, rect, RADIUS_PLATE, &[(0.0, if l.down { k.metal_dark } else { k.metal }), (1.0, k.metal_dark)]);
        edge_line(p, rect, 4.0, true, k.metal_shine.gamma_multiply(0.6 + 0.4 * l.hover));
        edge_line(p, rect, 4.0, false, k.metal_shadow);
    }
    let cut = Text::new(text, f).center().color(ink(l.on, if listening { k.focus } else { k.ink })).paint_only(ui, rect.shrink2(Vec2::new(6.0, 0.0))).cut;
    tip_if(resp, cut, text)
}

// ---- Readouts, bars, labels ----

/// An LCD readout (`.ig-lcd`): olive digits, slanted, in a recessed black window at least `min_chars` wide.
pub fn lcd(ui: &mut Ui, text: &str, size: f32, min_chars: usize) -> Response {
    let f = Font::READOUT.sized(size);
    let k = tokens();
    let char_w = Text::new("0", f).layout(ui.ctx(), f32::INFINITY).width + f.spacing();
    let w = Text::new(text, f).layout(ui.ctx(), f32::INFINITY).width.max(char_w * min_chars as f32) + 2.0 * SPACE[1] + f.size * 0.2;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, f.line + 4.0), Sense::hover());
    lcd_window(ui.painter(), rect);
    Text::new(text, f).color(k.lcd_ink).paint_only(ui, rect.shrink2(Vec2::new(SPACE[1], 0.0)));
    resp
}

/// A progress or charge bar (`width` × `height`, the topbar's research bar: 6 high, LCD ink) filled `fraction`.
pub fn bar(ui: &mut Ui, width: f32, height: f32, fraction: f32, fill: Color32) -> Response {
    let k = tokens();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let r = (height / 2.0) as u8;
    let p = ui.painter();
    p.rect_filled(rect, r, k.glass_deep);
    let f = ui.ctx().animate_value_with_time(resp.id.with("fill"), fraction.clamp(0.0, 1.0), super::anim(GLIDE_TIME));
    if f > 0.0 {
        p.rect_filled(rect.with_max_x(rect.left() + (rect.width() * f).max(height)), r, fill);
    }
    rim(p, rect, r, k.rim);
    resp
}

/// The tone of a badge or a table value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Own,
    Enemy,
    Neutral,
    Info,
    Warn,
}

impl Tone {
    pub fn color(self) -> Color32 {
        let k = tokens();
        match self {
            Tone::Own => k.own,
            Tone::Enemy => k.enemy,
            Tone::Neutral => k.ink_silver,
            Tone::Info => k.info,
            Tone::Warn => k.lcd_ink,
        }
    }
}

/// An empire's or a state's label (`.ig-star`: 600 15 / 0.16 em, upper case, its colour glowing), cut to
/// `max_width`; `pill` on a faint tinted plate.
pub fn badge(ui: &mut Ui, text: &str, tone: Tone, pill: bool, max_width: f32) -> Response {
    let f = Font { size: 13.0, line: 16.0, weight: Weight::SemiBold, tracking: 0.16, upper: true, lcd: false };
    let c = tone.color();
    let t = Text::new(text, f).color(c);
    let fit = t.layout(ui.ctx(), max_width - if pill { 16.0 } else { 0.0 });
    let pad = if pill { 8.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(fit.width + 2.0 * pad, 22.0), Sense::hover());
    if pill {
        ui.painter().rect_filled(rect, 11, c.gamma_multiply(0.14));
        rim(ui.painter(), rect, 11, c.gamma_multiply(0.45));
    }
    let base = baseline(rect, fit.font);
    let pos = Pos2::new(rect.left() + pad, base - fit.baseline());
    if tone == Tone::Neutral || pill {
        ui.painter().galley(pos, fit.galley.clone(), Color32::WHITE);
    } else {
        super::glowing_galley(ui.painter(), pos, &fit.galley, c);
    }
    tip_if(resp, fit.cut, text)
}

/// A data table's line (`.ig-table`: the label upper case and muted in a `label_width` column, the value in the data
/// style, info blue or its tone), on one baseline.
pub fn kv(ui: &mut Ui, label: &str, value: &str, tone: Tone, label_width: f32) -> Response {
    let k = tokens();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
    let base = baseline(rect, Font::DATA);
    let (a, b) = rect.split_left_right_at_x(rect.left() + label_width);
    let x = Text::new(label, Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, a.with_max_x(a.right() - SPACE[2]));
    let y = Text::new(value, Font::DATA).color(tone.color()).baseline(base).paint(ui, b);
    let _ = (x, y);
    resp
}

/// The title plate (`.ig-title-plate`): the name engraved on a cast-metal pill.
pub fn title_plate(ui: &mut Ui, text: &str, width: f32) -> Response {
    let k = tokens();
    let f = Font::TITLE_PLATE.upper(false);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 32.0 + 12.0 + 16.0), Sense::hover());
    let p = ui.painter();
    shadow(p, rect.translate(Vec2::new(0.0, 3.0)), 255, 0, 8, 0, Color32::from_black_alpha(204));
    radial(p, rect, 255, [0.5, 0.0], &[(0.0, k.metal_light), (0.4, k.metal), (1.0, k.metal_dark)]);
    relief(p, rect, 255);
    let inner = rect.shrink2(Vec2::new(SPACE[4], SPACE[1]));
    p.rect_filled(inner, 255, k.metal_shadow);
    vgrad(p, inner, 255, &[(0.0, Color32::BLACK), (6.0 / inner.height(), Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    let cut = Text::new(text, f).center().shrink(18.0).color(k.ink_silver).paint_only(ui, inner.shrink2(Vec2::new(SPACE[3], 0.0))).cut;
    tip_if(resp, cut, text)
}

/// A one-point rim-coloured line across the available width, `margin` above and below (`height: 1px; background:
/// var(--rim)`).
pub fn separator(ui: &mut Ui, margin: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0 + 2.0 * margin), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y.floor() + 0.5, Stroke::new(1.0f32, tokens().rim));
}

/// A label line (`.label`, the boards' section headings: 13 / 0.14 em upper case, muted) in the available width.
pub fn heading_label(ui: &mut Ui, text: &str) -> Response {
    Text::new(text, Font::LABEL).color(tokens().ink_muted).show(ui, Vec2::new(ui.available_width(), Font::LABEL.line))
}

// ---- Panels, cards, tooltips, drop-downs, modals ----

/// A panel's kind (`.ig-panel`, `.deep`, `.raised`, `.frame`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelKind {
    Glass,
    Deep,
    Raised,
    Frame,
}

/// A panel around `add` with `margin` inside (`.ig-panel`'s 16), its fill at `alpha` (the boards' deep panels over
/// a scene: 0.80, 0.88). The background is drawn behind what `add` drew.
pub fn panel<R>(ui: &mut Ui, kind: PanelKind, alpha: f32, margin: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let at = ui.painter().add(egui::Shape::Noop);
    let r = egui::Frame::new().inner_margin(margin).show(ui, add);
    behind(ui, at, |p| paint_panel(p, r.response.rect, kind, alpha));
    r.inner
}

/// What `paint` draws, moved back to the placeholder `at` (reserved before a container's content, so it lies behind).
pub fn behind(ui: &Ui, at: egui::layers::ShapeIdx, paint: impl FnOnce(&egui::Painter)) {
    let p = ui.painter();
    let start = ui.ctx().graphics(|g| g.get(p.layer_id()).map_or(0, |l| l.next_idx().0));
    paint(p);
    ui.ctx().graphics_mut(|g| {
        let l = g.entry(p.layer_id());
        let end = l.next_idx().0;
        let moved: Vec<egui::Shape> = l.all_entries().skip(start).take(end - start).map(|c| c.shape.clone()).collect();
        for i in start..end {
            l.reset_shape(egui::layers::ShapeIdx(i));
        }
        l.set(at, p.clip_rect(), egui::Shape::Vec(moved));
    });
}

/// A panel's background in `rect` (for a page that places its panels itself).
pub fn paint_panel(p: &egui::Painter, rect: Rect, kind: PanelKind, alpha: f32) {
    let k = tokens();
    match kind {
        PanelKind::Glass => glass(p, rect, k.glass.gamma_multiply(alpha), true),
        PanelKind::Deep => glass(p, rect, k.glass_deep.gamma_multiply(alpha), true),
        PanelKind::Raised => glass(p, rect, k.glass_raised.gamma_multiply(alpha), true),
        PanelKind::Frame => {
            p.rect_filled(rect, RADIUS_PANEL, k.glass_frame.gamma_multiply(alpha));
            rim(p, rect, RADIUS_PANEL, k.metal_dark);
        }
    }
}

/// A panel's header strip (the fleet board's: a rim-blue wash fading down, a rim line under it).
pub fn header_strip(p: &egui::Painter, rect: Rect) {
    let k = tokens();
    vgrad(p, rect, 0, &[(0.0, k.rim_bright.gamma_multiply(0.25)), (1.0, Color32::TRANSPARENT)]);
    p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0f32, k.rim));
}

/// A titled settings card (`.card` of the options boards: glass at 0.72, 18 × 16 inside, its heading in the label
/// style).
pub fn card<R>(ui: &mut Ui, title: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    let k = tokens();
    let at = ui.painter().add(egui::Shape::Noop);
    let r = egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 16, top: 18, bottom: 18 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if let Some(t) = title {
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                heading_label(ui, t);
            });
            ui.add_space(10.0 - ui.spacing().item_spacing.y);
        }
        add(ui)
    });
    let rect = r.response.rect;
    let fill = k.glass.gamma_multiply(0.72);
    ui.painter().set(at, egui::Shape::Vec(vec![
        egui::Shadow { offset: [0, 0], blur: 18, spread: 0, color: Color32::from_black_alpha(153) }.as_shape(rect, RADIUS_PANEL).into(),
        egui::epaint::RectShape::new(rect, RADIUS_PANEL, fill, Stroke::new(1.0f32, k.rim), egui::StrokeKind::Inside).into(),
        egui::Shape::line_segment([Pos2::new(rect.left() + 5.0, rect.top() + 0.5), Pos2::new(rect.right() - 5.0, rect.top() + 0.5)], Stroke::new(1.0f32, k.rim_bright)),
    ]));
    r.inner
}

/// A rich tooltip on `resp`: `text` wrapped at 320 points in the body style, at most 8 lines.
pub fn tip(resp: Response, text: &str) -> Response {
    resp.on_hover_ui(|ui| {
        let f = Font::DATA.weight(Weight::Regular).tracking(0.05);
        let fit = Text::new(text, f).lines(8).layout(ui.ctx(), 320.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(fit.width.min(320.0), fit.galley.size().y), Sense::hover());
        ui.painter().galley(rect.min, fit.galley, Color32::WHITE);
    })
}

/// A drop-down (egui's combo box in the kit's look: a well with a chevron, the list in a deep panel).
pub fn dropdown<R>(ui: &mut Ui, id: impl std::hash::Hash, selected: &str, width: f32, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    egui::ComboBox::from_id_salt(id).selected_text(selected).width(width).show_ui(ui, add).inner
}

/// A modal confirm box over a dimmed screen (`open`: it fades and slides in and out): its title, its body (wrapped,
/// at most 6 lines), Cancel and the action. Gives `Some(true)` for the action, `Some(false)` for Cancel or Esc.
pub fn confirm(ctx: &egui::Context, id: impl std::hash::Hash, open: bool, title: &str, body: &str, yes: &str, no: &str) -> Option<bool> {
    let id = Id::new(id);
    let t = super::open_t(ctx, id, open);
    if t <= 0.0 {
        return None;
    }
    let screen = ctx.screen_rect();
    let back = egui::LayerId::new(egui::Order::Foreground, id.with("back"));
    ctx.layer_painter(back).rect_filled(screen, 0.0, Color32::from_black_alpha((128.0 * t) as u8));
    let size = Vec2::new(560.0, 0.0).min(screen.size() - Vec2::splat(32.0));
    let mut answer = None;
    egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(Pos2::new(screen.center().x - size.x / 2.0, screen.center().y - 120.0 + (1.0 - t) * 16.0)).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(t);
        ui.set_width(size.x);
        answer = confirm_box(ui, size.x, title, body, yes, no);
    });
    if open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        answer = Some(false);
    }
    answer.filter(|_| open)
}

/// The confirm box itself, `width` points wide, where `ui` puts it ([`confirm`] centres it over a dimmed screen).
pub fn confirm_box(ui: &mut Ui, width: f32, title: &str, body: &str, yes: &str, no: &str) -> Option<bool> {
    let mut answer = None;
    ui.set_width(width);
    panel(ui, PanelKind::Deep, 0.96, 28.0, |ui| {
        ui.set_width(width - 56.0);
        Text::new(title, Font::HEADING).show(ui, Vec2::new(ui.available_width(), Font::HEADING.line));
        ui.add_space(SPACE[2]);
        let fit = Text::new(body, Font::BODY.sized(16.0)).color(tokens().ink_silver).lines(6).layout(ui.ctx(), ui.available_width());
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), fit.galley.size().y), Sense::hover());
        ui.painter().galley(rect.min, fit.galley, Color32::WHITE);
        ui.add_space(SPACE[3]);
        ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 44.0), egui::Layout::right_to_left(Align::Center), |ui| {
            if ui.add(Plate::new(yes).primary(true).width(160.0)).clicked() {
                answer = Some(true);
            }
            if ui.add(Plate::new(no).width(160.0)).clicked() {
                answer = Some(false);
            }
        });
    });
    answer
}
