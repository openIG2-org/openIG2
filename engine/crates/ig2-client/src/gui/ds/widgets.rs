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
    // The hand over every enabled control, as the original's spots (`Mouse_Szoveg_Set`'s type 1, re/screens/input.md §8).
    if on && (r.hovered() || r.is_pointer_button_down_on()) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
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
pub fn hover_rim(p: &egui::Painter, rect: Rect, r: impl Into<egui::CornerRadius>, t: f32) {
    if t > 0.0 {
        rim(p, rect, r, tokens().rim_bright.gamma_multiply(t));
    }
}

/// A widget's whole text as its tooltip where its label was cut.
fn tip_if(r: Response, cut: bool, text: &str) -> Response {
    if cut { r.on_hover_text(text) } else { r }
}

// ---- The boards' input and placing ----

/// Clicked with the left or the right button (the original's spots take both).
pub fn either(r: &Response) -> bool {
    r.clicked() || r.secondary_clicked()
}

/// Pressed with either button while enabled: the original's buttons act on the press, not the release
/// (`Mouse_Lenyom`, the hot-spots' one-shot `AktIkon2`).
pub fn pressed(r: &Response) -> bool {
    r.hovered() && r.enabled() && r.ctx.input(|i| i.pointer.any_pressed())
}

/// Either button held (a game hot-spot drawn pressed).
pub fn any_down(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.pointer.primary_down() || i.pointer.secondary_down())
}

/// A board's tooltip `id` of its text list, empty where it has none.
pub fn tip_text(tips: &[String], id: u32) -> &str {
    tips.get(id as usize).map_or("", |s| s.as_str())
}

thread_local! {
    /// How far the [`place`]d panels are in ([`entering`]).
    static ENTRANCE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

/// How far a panel slides in from its side (points).
const ENTRANCE_SLIDE: f32 = 48.0;

/// Ours (ADR 0084): run `page` with its [`place`]d panels `t` (0..1) of the way in: each fades in and slides in from
/// the side of the screen it stands on (left or right, else from above or below), as a modern screen opens and
/// closes ([`crate::gui::Transition::panels`]).
pub fn entering<R>(t: f32, page: impl FnOnce() -> R) -> R {
    let old = ENTRANCE.with(|e| e.replace(t.clamp(0.0, 1.0)));
    let r = page();
    ENTRANCE.with(|e| e.set(old));
    r
}

/// The offset of a panel at `rect` on `screen` that is `t` of the way in.
pub(crate) fn entrance_offset(rect: Rect, screen: Rect, t: f32) -> Vec2 {
    if t >= 1.0 {
        return Vec2::ZERO;
    }
    let d = rect.center() - screen.center();
    let dir = if d.x.abs() > rect.width() * 0.25 { Vec2::new(d.x.signum(), 0.0) } else { Vec2::new(0.0, if d.y < 0.0 { -1.0 } else { 1.0 }) };
    dir * (1.0 - t) * ENTRANCE_SLIDE
}

/// Ours (ADR 0084): an area's own entrance while [`entering`] runs (for a page whose areas are anchored rather than
/// [`place`]d): call it first in the area's contents; the area fades and its layer slides in from its side of the
/// screen, as [`place`]'s panels do. Nothing at rest.
pub fn entrance(ui: &mut Ui) {
    let t = ENTRANCE.with(|e| e.get());
    let layer = ui.layer_id();
    let off = match ui.ctx().memory(|m| m.area_rect(layer.id)) {
        Some(r) if t < 1.0 => entrance_offset(r, ui.ctx().screen_rect(), t),
        _ => Vec2::ZERO,
    };
    ui.ctx().set_transform_layer(layer, egui::emath::TSTransform::from_translation(off));
    if t < 1.0 {
        ui.multiply_opacity(t);
    }
}

/// `add` in a fixed rectangle of the screen (an area `id` of its own, top-down).
pub fn place<R>(ctx: &egui::Context, id: impl std::hash::Hash, rect: Rect, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    place_in(ctx, id, rect, None, add)
}

/// [`place`] on a page that scrolls: `rect` may reach past the screen, what shows is cut to `clip`.
pub fn place_cut<R>(ctx: &egui::Context, id: impl std::hash::Hash, rect: Rect, clip: Rect, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    place_in(ctx, id, rect, Some(clip), add)
}

fn place_in<R>(ctx: &egui::Context, id: impl std::hash::Hash, rect: Rect, clip: Option<Rect>, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let t = ENTRANCE.with(|e| e.get());
    egui::Area::new(Id::new(id)).fixed_pos(rect.min + entrance_offset(rect, ctx.screen_rect(), t)).fade_in(false).constrain(clip.is_none()).show(ctx, |ui| {
        if t < 1.0 {
            ui.multiply_opacity(t);
        }
        if let Some(c) = clip {
            ui.set_clip_rect(ui.clip_rect().intersect(c));
        }
        ui.set_min_size(rect.size());
        ui.set_max_size(rect.size());
        add(ui)
    })
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
    pad: f32,
}

impl<'a> Plate<'a> {
    pub const FONT: Font = Font { size: 20.0, line: 24.0, weight: Weight::Medium, tracking: 0.16, upper: false, lcd: false };
    pub fn new(text: &'a str) -> Self {
        Self { text, sub: None, primary: false, focused: false, size: Vec2::new(0.0, 44.0), font: Self::FONT, pad: SPACE[3] }
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
    /// The room at each side of the label (24; a small plate's, the boards' lamps: 8).
    pub fn pad(mut self, p: f32) -> Self {
        self.pad = p;
        self
    }
}

impl Widget for Plate<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let natural = Text::new(self.text, self.font).layout(ui.ctx(), f32::INFINITY).width + 2.0 * self.pad;
        // At least the set width (CSS `min-width`), wider for the label while the row has room, then the label is cut.
        let w = natural.max(self.size.x).min(ui.available_width().max(self.size.x));
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, self.size.y), Sense::click());
        let l = look(ui, &resp, self.focused);
        let p = ui.painter();
        let k = tokens();
        plate_fill(p, rect, RADIUS_PANEL, self.primary || l.down);
        hover_rim(ui.painter(), rect, RADIUS_PANEL, l.hover);
        if l.focus {
            focus_mark(p, rect, RADIUS_PANEL as f32, 2.0);
        }
        let inner = rect.shrink2(Vec2::new(SPACE[2].min(self.pad), 0.0));
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
/// available), a key at the right (`kbd`: the in-game menu's Esc, O, F1).
#[derive(Debug, Clone, Copy, Default)]
pub struct MenuEntry<'a> {
    pub text: &'a str,
    pub sub: Option<&'a str>,
    pub off: bool,
    pub hint: Option<&'a str>,
}

impl<'a> MenuEntry<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, sub: None, off: false, hint: None }
    }
}

/// A column of menu items (`.mm-item`, `.rail`) 4 points apart, the selected one on a plate that glides to it; the
/// keyboard focus `focus` marked. Gives the clicked entry.
pub fn menu(ui: &mut Ui, id: impl std::hash::Hash, style: MenuStyle, entries: &[MenuEntry], selected: Option<usize>, focus: Option<usize>) -> Option<usize> {
    let id = ui.make_persistent_id(id);
    let k = tokens();
    let w = ui.available_width();
    let gap = SPACE[0];
    let sf = Font { line: 16.0, ..Font::LABEL.weight(Weight::Medium).tracking(0.1).upper(false) };
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
        hover_rim(ui.painter(), rect, RADIUS_PLATE, l.hover * (selected != Some(i)) as u8 as f32);
        if l.focus {
            focus_mark(ui.painter(), rect, RADIUS_PLATE as f32, 2.0);
        }
        let mut inner = rect.shrink2(Vec2::new(20.0, 0.0));
        let colour = if e.off || !l.on { k.ink_muted } else { k.ink };
        if let Some(h) = e.hint {
            let hf = Font::LABEL.tracking(0.1).upper(false);
            let fit = Text::new(h, hf).right().color(k.ink_silver).baseline(baseline(inner, style.font)).paint_only(ui, inner.with_min_x(inner.right() - 80.0));
            inner = inner.with_max_x(inner.right() - fit.width - SPACE[2]);
        }
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

const TAB_FONT: Font = Font { size: 16.0, line: 20.0, weight: Weight::Medium, tracking: 0.12, upper: false, lcd: false };

/// The width [`tabs`] of `labels` take uncut.
pub fn tabs_width(ctx: &egui::Context, labels: &[&str]) -> f32 {
    labels.iter().map(|s| Text::new(s, TAB_FONT).layout(ctx, f32::INFINITY).width + 36.0).sum::<f32>() + SPACE[0] * labels.len().saturating_sub(1) as f32
}

/// A horizontal strip of tabs (`.tab`: 36 high, 500 16 / 0.12 em; the chosen one on a gliding plate). Too many for
/// the width share it and shrink their labels, then cut them. Gives the clicked tab.
pub fn tabs(ui: &mut Ui, id: impl std::hash::Hash, labels: &[&str], selected: usize) -> Option<usize> {
    const FONT: Font = TAB_FONT;
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
        let cut = Text::new(s, FONT).center().shrink(12.0).color(ink(l.on, c)).paint_only(ui, rect.shrink2(Vec2::new(8.0, 0.0))).cut;
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
    /// A volume's mute button (`volume-2`, crossed out when muted).
    Speaker,
    SpeakerOff,
    /// Two opposed arrows (`arrow-left-right`): an exchange between two sides.
    Swap,
    /// Money (`coins`), energy (`zap`), workers (`user`).
    Coins,
    Bolt,
    User,
    /// Details (`info`), on/off (`power`), repair (`wrench`), demolish (`trash-2`), back (`undo-2`).
    Info,
    Power,
    Wrench,
    Trash,
    Undo,
    /// Zoom in and out (`zoom-in`, `zoom-out`), centre on (`crosshair`).
    ZoomIn,
    ZoomOut,
    Target,
    /// The screens' keys (the boards' top bar): `map`, `building-2`, `flask-conical`, `users`, `eye`, the
    /// trade's `arrow-left-right`, `activity`, `circle-help` and `mail`.
    Map,
    Building,
    Flask,
    Users,
    Eye,
    Trade,
    Activity,
    Help,
    Mail,
    /// A planet's richness on the starmap's card (`hammer`).
    Hammer,
    /// Production (`factory`).
    Factory,
    /// A colony (a dome under its flag).
    Colony,
    /// The original console's camera keys: rotate, tilt, the top and side views.
    RotateLeft,
    RotateRight,
    TiltUp,
    TiltDown,
    TopView,
    SideView,
    /// The vehicles (a tank), a fleet's split.
    Tanks,
    Split,
    /// A radial menu's icon by its number (`StarmapRopIkonok`; the kit's own where it has one).
    Rop(i32),
}

impl Icon {
    /// Every icon (the tests read each file).
    #[cfg(test)]
    pub(crate) const ALL: [Icon; 58] = [Icon::ChevronLeft, Icon::ChevronRight, Icon::ChevronDown, Icon::ChevronUp, Icon::Pause, Icon::Play, Icon::Fast, Icon::Fastest, Icon::Close, Icon::Check, Icon::Plus, Icon::Minus, Icon::Menu, Icon::Speaker, Icon::SpeakerOff, Icon::Swap, Icon::Coins, Icon::Bolt, Icon::User, Icon::Info, Icon::Power, Icon::Wrench, Icon::Trash, Icon::Undo, Icon::ZoomIn, Icon::ZoomOut, Icon::Target, Icon::Map, Icon::Building, Icon::Flask, Icon::Users, Icon::Eye, Icon::Trade, Icon::Activity, Icon::Help, Icon::Mail, Icon::Hammer, Icon::Factory, Icon::Colony, Icon::RotateLeft, Icon::RotateRight, Icon::TiltUp, Icon::TiltDown, Icon::TopView, Icon::SideView, Icon::Tanks, Icon::Split, Icon::Rop(3), Icon::Rop(5), Icon::Rop(6), Icon::Rop(7), Icon::Rop(8), Icon::Rop(9), Icon::Rop(0xe), Icon::Rop(0x10), Icon::Rop(0x11), Icon::Rop(0x12), Icon::Rop(0)];

    /// The icon in `rect` in `c`: its file stroked by [`super::svg`].
    pub fn paint(self, p: &egui::Painter, rect: Rect, c: Color32) {
        super::svg::paint(p, rect, self.file(), c);
    }

    /// The icon's file (`gui/icons/`: Lucide's, ISC, its notice beside them, and ours in its style).
    pub(crate) fn file(self) -> &'static str {
        match self {
            Icon::ChevronLeft => include_str!("../icons/chevron-left.svg"),
            Icon::ChevronRight => include_str!("../icons/chevron-right.svg"),
            Icon::ChevronDown => include_str!("../icons/chevron-down.svg"),
            Icon::ChevronUp => include_str!("../icons/chevron-up.svg"),
            Icon::Pause => include_str!("../icons/pause.svg"),
            Icon::Play => include_str!("../icons/chevron-right.svg"),
            Icon::Fast => include_str!("../icons/chevrons-right.svg"),
            Icon::Fastest => include_str!("../icons/chevrons-right-3.svg"),
            Icon::Close => include_str!("../icons/x.svg"),
            Icon::Check => include_str!("../icons/check.svg"),
            Icon::Plus => include_str!("../icons/plus.svg"),
            Icon::Minus => include_str!("../icons/minus.svg"),
            Icon::Menu => include_str!("../icons/menu.svg"),
            Icon::Speaker => include_str!("../icons/volume-2.svg"),
            Icon::SpeakerOff => include_str!("../icons/volume-x.svg"),
            Icon::Swap => include_str!("../icons/arrow-left-right.svg"),
            Icon::Coins => include_str!("../icons/coins.svg"),
            Icon::Bolt => include_str!("../icons/zap.svg"),
            Icon::User => include_str!("../icons/user.svg"),
            Icon::Info => include_str!("../icons/info.svg"),
            Icon::Power => include_str!("../icons/power.svg"),
            Icon::Wrench => include_str!("../icons/wrench.svg"),
            Icon::Trash => include_str!("../icons/trash-2.svg"),
            Icon::Undo => include_str!("../icons/undo-2.svg"),
            Icon::ZoomIn => include_str!("../icons/zoom-in.svg"),
            Icon::ZoomOut => include_str!("../icons/zoom-out.svg"),
            Icon::Target => include_str!("../icons/target.svg"),
            Icon::Map => include_str!("../icons/map.svg"),
            Icon::Building => include_str!("../icons/building-2.svg"),
            Icon::Flask => include_str!("../icons/flask-conical.svg"),
            Icon::Users => include_str!("../icons/users.svg"),
            Icon::Eye => include_str!("../icons/eye.svg"),
            Icon::Trade => include_str!("../icons/arrow-left-right.svg"),
            Icon::Activity => include_str!("../icons/activity.svg"),
            Icon::Help => include_str!("../icons/circle-help.svg"),
            Icon::Mail => include_str!("../icons/mail.svg"),
            Icon::Hammer => include_str!("../icons/hammer.svg"),
            Icon::Factory => include_str!("../icons/factory.svg"),
            Icon::Colony => include_str!("../icons/colony.svg"),
            Icon::RotateLeft => include_str!("../icons/rotate-ccw.svg"),
            Icon::RotateRight => include_str!("../icons/rotate-cw.svg"),
            Icon::TiltUp => include_str!("../icons/chevrons-up.svg"),
            Icon::TiltDown => include_str!("../icons/chevrons-down.svg"),
            Icon::TopView => include_str!("../icons/circle-dot.svg"),
            Icon::SideView => include_str!("../icons/side-view.svg"),
            Icon::Tanks => include_str!("../icons/tank.svg"),
            Icon::Split => include_str!("../icons/split.svg"),
            Icon::Rop(n) => match n {
                3 => include_str!("../icons/route.svg"),
                5 => include_str!("../icons/satellite.svg"),
                6 => include_str!("../icons/satellite-dish.svg"),
                7 => include_str!("../icons/globe.svg"),
                8 => include_str!("../icons/flag.svg"),
                9 => include_str!("../icons/tank.svg"),
                0xe => include_str!("../icons/merge.svg"),
                0x10 => include_str!("../icons/tornado.svg"),
                0x11 => include_str!("../icons/bug.svg"),
                0x12 => include_str!("../icons/bomb.svg"),
                _ => include_str!("../icons/info.svg"),
            },
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

/// The width of a [`key_group`] of `n` keys `w` wide.
pub fn key_group_w(n: usize, w: f32) -> f32 {
    n as f32 * w + n.saturating_sub(1) as f32 * 2.0 + 4.0
}

/// The cast-metal plate (`radial-gradient(ellipse at 50% 0%, metal-light, metal 35 %, metal-dark)` with the relief and
/// its shadow): the radar's, the battle's navigation.
pub fn metal_plate(p: &egui::Painter, r: Rect, radius: u8) {
    let k = tokens();
    shadow(p, r.translate(Vec2::new(0.0, 3.0)), radius, 0, 8, 0, Color32::from_black_alpha(204));
    radial(p, r, radius, [0.5, 0.0], &[(0.0, k.metal_light), (0.35, k.metal), (1.0, k.metal_dark)]);
    relief(p, r, radius);
}

/// The boards' radar: a metal plate (18 round, 10 inside) round a black well `well` big (10 round, sunk: a shadow
/// from its top edge, a dark ring) that `content` draws into, and a row of keys `keys_w` wide centred 8 under it.
/// `see_through`: no black and the metal only round the well (a picture under the panels shows in it). The well's
/// rectangle.
pub fn radar(ui: &mut Ui, well: Vec2, see_through: bool, keys_w: f32, content: impl FnOnce(&mut Ui, Rect), keys: impl FnOnce(&mut Ui)) -> Rect {
    let k = tokens();
    let (outer, _) = ui.allocate_exact_size(well + Vec2::new(20.0, 20.0 + 8.0 + 32.0), Sense::hover());
    let screen = Rect::from_min_size(outer.min + Vec2::splat(10.0), well);
    let parts = if see_through {
        let o = outer.expand(12.0);
        vec![o.with_max_y(screen.top()), o.with_min_y(screen.bottom()), Rect::from_x_y_ranges(o.left()..=screen.left(), screen.y_range()), Rect::from_x_y_ranges(screen.right()..=o.right(), screen.y_range())]
    } else {
        vec![ui.clip_rect()]
    };
    for part in parts {
        metal_plate(&ui.painter().with_clip_rect(part.intersect(ui.clip_rect())), outer, 18);
    }
    if !see_through {
        ui.painter().rect_filled(screen, 10, Color32::BLACK);
    }
    content(ui, screen);
    vgrad(ui.painter(), screen, 10, &[(0.0, Color32::from_black_alpha(220)), (6.0 / screen.height(), Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    rim(ui.painter(), screen.expand(1.0), 11, k.metal_shadow);
    let row = Rect::from_center_size(egui::pos2(outer.center().x, screen.bottom() + 8.0 + 16.0), Vec2::new(keys_w, 32.0));
    keys(&mut ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Min))));
    screen
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

/// A round action button `d` across with its label on a black tag under it (the boards' repair, demolish and the
/// ground war's strikes: 11 / 0.14 em upper case, silver, white while `active`); the response is the button's, its
/// tooltip `tip` unless that is the label. The tag is as wide as its label.
pub fn round_action(ui: &mut Ui, glyph: Glyph, d: f32, active: bool, label: &str, tip: &str) -> Response {
    const FONT: Font = Font { size: 11.0, line: 14.0, weight: Weight::SemiBold, tracking: 0.14, upper: true, lcd: false };
    let k = tokens();
    // The column is as wide as the wider of the button and its tag (`align-items: center`), so neighbours keep
    // their gap between tags.
    let outer = d + 2.0 * (if d < 48.0 { 3.0 } else { 5.0 } + 1.0);
    // The tag as wide as its label (never cut: an auto-sized row's room is only last frame's).
    let tag_w = Text::new(label, FONT).layout(ui.ctx(), f32::INFINITY).width + 16.0 + FONT.spacing();
    ui.allocate_ui_with_layout(Vec2::new(outer.max(tag_w), outer + 4.0 + 18.0), egui::Layout::top_down(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let r = round_button_sized(ui, glyph, d, active);
        let (row, _) = ui.allocate_exact_size(Vec2::new(outer.max(tag_w), 18.0), Sense::hover());
        let tag = Rect::from_center_size(row.center(), Vec2::new(tag_w, 18.0));
        ui.painter().rect_filled(tag, RADIUS_PLATE, Color32::BLACK);
        Text::new(label, FONT).center().color(ink(ui.is_enabled(), if active { k.ink } else { k.ink_silver })).paint(ui, tag.shrink2(Vec2::new(8.0, 0.0)));
        if tip == label { r } else { super::tip(r, tip) }
    })
    .inner
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
        hover_rim(ui.painter(), rect, RADIUS_PLATE, hover);
    }
}

/// A drop-down list's or a menu's back: deep glass, opaque (the field and the plates under it don't show through,
/// the boards' menus).
pub fn menu_back(p: &egui::Painter, rect: Rect) {
    p.rect_filled(rect, RADIUS_PANEL, tokens().void);
    super::paint_panel(p, rect, super::PanelKind::Deep, 0.97);
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
    lead: bool,
}

impl<'a> ListRow<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, value: None, selected: false, off: false, focused: false, height: 26.0, font: Font::BODY, lead: false }
    }
    /// The row leads on to a sub-menu: a chevron at its right while hovered or selected.
    pub fn lead(mut self, on: bool) -> Self {
        self.lead = on;
        self
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
        if self.lead {
            text_box = inner.with_max_x(inner.right() - 20.0);
            let t = if self.selected { 1.0 } else { l.hover };
            if t > 0.0 && !self.off {
                let c = Pos2::new(inner.right() - 8.0, rect.center().y);
                ui.painter().line(vec![c + Vec2::new(-3.0, -5.0), c + Vec2::new(2.0, 0.0), c + Vec2::new(-3.0, 5.0)], Stroke::new(1.5f32, k.ink_silver.gamma_multiply(t)));
            }
        }
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

/// A table's column heads (`th`: 400 16 / 0.1 em upper case, muted).
pub const TABLE_HEAD: Font = Font { size: 16.0, line: 20.0, weight: Weight::Regular, tracking: 0.1, upper: true, lcd: false };

/// A table's cells across `r`, `gap` apart: `widths` in points, a 0 shares what the others leave.
pub fn columns(r: Rect, widths: &[f32], gap: f32) -> Vec<Rect> {
    let fixed = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    let share = (r.width() - fixed).max(0.0) / widths.iter().filter(|&&w| w == 0.0).count().max(1) as f32;
    let mut x = r.left();
    widths
        .iter()
        .map(|&w| {
            let w = if w == 0.0 { share } else { w };
            let c = Rect::from_x_y_ranges(x..=x + w, r.y_range());
            x += w + gap;
            c
        })
        .collect()
}

/// A table's head row: `labels` in the cells `cols` ([`columns`]) on one baseline, a column of `right` at its right.
pub fn table_head(ui: &Ui, cols: &[Rect], labels: &[&str], right: &[bool]) {
    let k = tokens();
    for (i, (c, s)) in cols.iter().zip(labels).enumerate() {
        let t = Text::new(s, TABLE_HEAD).color(k.ink_muted).baseline(baseline(*c, TABLE_HEAD)).shrink(11.0);
        if right.get(i).copied().unwrap_or(false) { t.right() } else { t }.paint(ui, *c);
    }
}

/// A small square tag (`.badge`: raised glass, a rim, 600 12 / 0.12 em upper case, 26 high) in `c`: a row's state
/// ("Ready", "Password"), a title's mark ("Host").
pub fn tag(ui: &mut Ui, text: &str, c: Color32) -> Response {
    const FONT: Font = Font { size: 12.0, line: 14.0, weight: Weight::SemiBold, tracking: 0.12, upper: true, lcd: false };
    let k = tokens();
    let fit = Text::new(text, FONT).layout(ui.ctx(), 220.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(fit.width + 20.0, 26.0), Sense::hover());
    ui.painter().rect_filled(rect, RADIUS_PLATE, k.glass_raised);
    rim(ui.painter(), rect, RADIUS_PLATE, k.rim);
    Text::new(text, FONT).color(c).center().paint(ui, rect.shrink2(Vec2::new(6.0, 0.0)));
    resp
}

/// A text field (`.field input`: recessed, 500 17 / 0.08 em, 14 in) `size` big: `text` with the caret before its
/// character `caret` while the field takes the keys (`Some`: the line scrolls to keep the caret in view), `hint`
/// muted while it is empty. The keys are the caller's; a click is the response's.
pub fn text_field(ui: &mut Ui, text: &str, caret: Option<usize>, hint: &str, size: Vec2) -> Response {
    const FONT: Font = Font { size: 17.0, line: 20.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
    let k = tokens();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let l = look(ui, &resp, caret.is_some());
    let p = ui.painter();
    p.rect_filled(rect, RADIUS_PLATE, k.glass_deep);
    rim(p, rect, RADIUS_PLATE, k.rim.lerp_to_gamma(k.rim_bright, l.hover));
    edge_line(p, rect, RADIUS_PLATE as f32, true, Color32::from_black_alpha(200));
    if l.focus {
        focus_mark(p, rect, RADIUS_PLATE as f32, 2.0);
    }
    let inner = rect.shrink2(Vec2::new(14.0, 0.0));
    if text.is_empty() && caret.is_none() {
        Text::new(hint, FONT).color(k.ink_muted).paint(ui, inner);
        return resp;
    }
    let galley = ui.painter().layout_job(FONT.job(text, if l.on { k.ink } else { k.ink_muted }));
    let at_caret = galley.pos_from_cursor(egui::text::CCursor::new(caret.unwrap_or(0).min(text.chars().count()))).min.x;
    let shift = if caret.is_some() { (at_caret - inner.width() + 4.0).max(0.0) } else { 0.0 };
    let top = baseline(rect, FONT) - super::fit::Fitted { galley: galley.clone(), font: FONT, cut: false, width: 0.0 }.baseline();
    let at = Pos2::new(inner.left() - shift, top);
    let painter = ui.painter_at(inner.expand2(Vec2::new(2.0, 0.0)));
    if text.is_empty() {
        Text::new(hint, FONT).color(k.ink_muted).paint(ui, inner);
    }
    if caret.is_some() {
        if (ui.input(|i| i.time) * 2.0) as i64 % 2 == 0 {
            let x = at.x + at_caret;
            painter.line_segment([Pos2::new(x, rect.center().y - 11.0), Pos2::new(x, rect.center().y + 11.0)], Stroke::new(1.5f32, k.ink));
        }
        ui.ctx().request_repaint();
    }
    painter.galley(at, galley, k.ink);
    resp
}

/// A settings row (`.orow`: 50 high, its 16-point label at the left, its control at the right, the label cut to the
/// room the control leaves); `focused` lit (the keyboard's row), `enabled` false muted.
pub fn option_row<R>(ui: &mut Ui, label: &str, focused: bool, enabled: bool, control: impl FnOnce(&mut Ui) -> R) -> (Response, R) {
    const FONT: Font = Font { size: 16.0, line: 20.0, weight: Weight::Regular, tracking: 0.05, upper: false, lcd: false };
    option_row_in(ui, 50.0, FONT, label, focused, enabled, control)
}

/// An [`option_row`] `height` high with its label in `font` (the key rows, `.krow`: 40 high, 15 / 20).
pub fn option_row_in<R>(ui: &mut Ui, height: f32, font: Font, label: &str, focused: bool, enabled: bool, control: impl FnOnce(&mut Ui) -> R) -> (Response, R) {
    let k = tokens();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let hover = hover_t(ui.ctx(), resp.id, resp.hovered() && enabled);
    if focused {
        ui.painter().rect_filled(rect, RADIUS_PLATE, k.rim_bright.gamma_multiply(0.12));
    }
    hover_rim(ui.painter(), rect, RADIUS_PLATE, if focused { 1.0 } else { hover * 0.6 });
    let inner = rect.shrink2(Vec2::new(SPACE[1], 0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::right_to_left(Align::Center)));
    if !enabled {
        child.disable();
    }
    let r = control(&mut child);
    let used = child.min_rect().width();
    let cut = Text::new(label, font).shrink(font.size - 3.0).color(if enabled { k.ink } else { k.ink_muted }).paint_only(ui, inner.with_max_x(inner.right() - used - SPACE[2])).cut;
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
/// `plates`: [`segmented_plates`]' look, its plates this high.
fn segments(ui: &mut Ui, labels: &[&str], selected: usize, font: Font, width: Option<f32>, pad: f32, plates: Option<f32>) -> (Response, Option<usize>) {
    let k = tokens();
    let big = plates.is_some();
    let (inset, h) = plates.map_or((3.0, 30.0), |h| (4.0, h));
    let natural: Vec<f32> = labels.iter().map(|s| (Text::new(s, font).layout(ui.ctx(), f32::INFINITY).width + 2.0 * pad).max(46.0)).collect();
    let n = labels.len().max(1) as f32;
    let widths: Vec<f32> = match width {
        Some(w) => vec![((w - 2.0 * inset - inset * (n - 1.0)) / n).max(10.0); labels.len()],
        None => natural,
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(widths.iter().sum::<f32>() + 2.0 * inset + inset * (n - 1.0), h + 2.0 * inset), Sense::hover());
    if big {
        ui.painter().rect_filled(rect, RADIUS_PANEL, k.glass_deep);
        rim(ui.painter(), rect, RADIUS_PANEL, k.rim);
    } else {
        well(ui, rect);
    }
    let lefts: Vec<f32> = widths.iter().scan(rect.left() + inset, |x, w| {
        let l = *x;
        *x += w + inset;
        Some(l)
    }).collect();
    let id = resp.id;
    if selected < labels.len() {
        let x = glide(ui, id.with("x"), lefts[selected]);
        let w = glide(ui, id.with("w"), widths[selected]);
        let lit = Rect::from_min_size(Pos2::new(x, rect.top() + inset), Vec2::new(w, h));
        if big {
            plate_fill(ui.painter(), lit, RADIUS_PLATE, false);
        } else {
            ui.painter().rect_filled(lit, 3, if ui.is_enabled() { k.plate_high } else { k.plate });
        }
    }
    let mut clicked = None;
    for (i, s) in labels.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(lefts[i], rect.top() + inset), Vec2::new(widths[i], h));
        let sr = ui.interact(r, id.with(i), Sense::click());
        let l = look(ui, &sr, false);
        let c = if i == selected { k.ink } else { k.ink_muted.lerp_to_gamma(k.ink_silver, l.hover) };
        let cut = Text::new(s, font).center().shrink(font.size - 3.0).color(ink(l.on, c)).paint_only(ui, r.shrink2(Vec2::new(4.0, 0.0))).cut;
        if tip_if(sr, cut, s).clicked() && l.on {
            clicked = Some(i);
        }
    }
    (resp, clicked)
}

/// A segmented choice (`.seg`); `width` for a wide row's equal columns. Sets `selected` on a click.
pub fn segmented(ui: &mut Ui, selected: &mut usize, labels: &[&str], width: Option<f32>) -> Response {
    let (mut resp, c) = segments(ui, labels, *selected, SEG_FONT, width, 10.0, None);
    if let Some(c) = c.filter(|c| c != selected) {
        *selected = c;
        resp.mark_changed();
    }
    resp
}

/// A segmented choice of lit plates (`.seg` of the new game and end-of-game boards: plates `height` high, 4 apart in
/// a panel-round well, labels in `font`); `width` for equal columns, else each to its label.
pub fn segmented_plates(ui: &mut Ui, selected: &mut usize, labels: &[&str], width: Option<f32>, height: f32, font: Font) -> Response {
    let (mut resp, c) = segments(ui, labels, *selected, font, width, 18.0, Some(height));
    if let Some(c) = c.filter(|c| c != selected) {
        *selected = c;
        resp.mark_changed();
    }
    resp
}

/// An off / on toggle with its two words (`.tg`: "KI" "BE"); the lit block glides.
pub fn toggle(ui: &mut Ui, on: &mut bool, off_label: &str, on_label: &str) -> Response {
    let (mut resp, c) = segments(ui, &[off_label, on_label], *on as usize, TOGGLE_FONT, None, 8.0, None);
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
    slider_sized(ui, value, readout, 150.0)
}

/// [`slider`] with a `width`-point track (the volumes' rows stretch theirs).
pub fn slider_sized(ui: &mut Ui, value: &mut f32, readout: &str, width: f32) -> Response {
    let k = tokens();
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(width + 12.0 + 52.0, 24.0), Sense::click_and_drag());
    let track = Rect::from_min_size(Pos2::new(rect.left() + 6.0, rect.center().y - 4.0), Vec2::new(width - 12.0, 8.0));
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
    lcd_ink(ui, text, size, min_chars, tokens().lcd_ink)
}

/// An [`lcd`] with its digits in `ink` (a readout blinking after a change).
pub fn lcd_ink(ui: &mut Ui, text: &str, size: f32, min_chars: usize, ink: Color32) -> Response {
    let f = Font::READOUT.sized(size);
    let char_w = Text::new("0", f).layout(ui.ctx(), f32::INFINITY).width + f.spacing();
    let w = Text::new(text, f).layout(ui.ctx(), f32::INFINITY).width.max(char_w * min_chars as f32) + 2.0 * SPACE[1] + f.size * 0.2;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, f.line + 4.0), Sense::hover());
    lcd_window(ui.painter(), rect);
    Text::new(text, f).color(ink).paint_only(ui, rect.shrink2(Vec2::new(SPACE[1], 0.0)));
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
    let f = Font::OWNER;
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
    Text::new(label, Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, a.with_max_x(a.right() - SPACE[2]));
    Text::new(value, Font::DATA).color(tone.color()).baseline(base).paint(ui, b);
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

/// A menu column's rule between groups (`height: 1px; margin: 14px 20px` in the column's 4-point gaps: 18 above and
/// below, 20 in from each side).
pub fn menu_rule(ui: &mut Ui) {
    menu_rule_gap(ui, 18.0, 18.0);
}

/// [`menu_rule`] with `above` and `below` it (the in-game menu's 17 and 15).
pub fn menu_rule_gap(ui: &mut Ui, above: f32, below: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), above + 1.0 + below), Sense::hover());
    ui.painter().hline((rect.left() + 20.0)..=(rect.right() - 20.0), rect.top() + above + 0.5, Stroke::new(1.0f32, tokens().rim));
}

/// A menu column's group heading (`.mm-group`: the label style, 20 in like the entries, 24 high, 4 to the entries).
pub fn menu_heading(ui: &mut Ui, text: &str) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
    Text::new(text, Font::LABEL).color(tokens().ink_muted).paint(ui, Rect::from_min_max(Pos2::new(r.left() + 20.0, r.top()), Pos2::new(r.right() - 20.0, r.top() + 18.0)));
}

/// A recessed text field's back (`.field`: deep glass, a rim lit while it takes the keys).
pub fn field_back(p: &egui::Painter, rect: Rect, focused: bool) {
    let k = tokens();
    p.rect_filled(rect, RADIUS_PLATE, k.glass_deep);
    rim(p, rect, RADIUS_PLATE, if focused { k.focus } else { k.rim });
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
pub fn panel<R>(ui: &mut Ui, kind: PanelKind, alpha: f32, margin: impl Into<egui::Margin>, add: impl FnOnce(&mut Ui) -> R) -> R {
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
            // `h3`: 18 high, 8 in, 10 above the rows (a `horizontal` would take the interact height).
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), Font::LABEL.line), Sense::hover());
            Text::new(t, Font::LABEL).color(k.ink_muted).paint(ui, r.with_min_x(r.left() + 8.0));
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

/// A rich tooltip on `resp`: `text` wrapped at 320 points in the body style, at most 8 lines; none for no text.
pub fn tip(resp: Response, text: &str) -> Response {
    if text.is_empty() {
        return resp;
    }
    resp.on_hover_ui(|ui| {
        let f = Font::DATA.weight(Weight::Regular).tracking(0.05);
        let fit = Text::new(text, f).lines(8).layout(ui.ctx(), 320.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(fit.width.min(320.0), fit.galley.size().y), Sense::hover());
        ui.painter().galley(rect.min, fit.galley, Color32::WHITE);
    })
}

/// [`tip`] with the control's key in a small key cap after the text (a screen's key, a speed's). The text and the
/// key sit on one baseline, the text's capitals centred in the row and the cap centred on the row.
pub fn tip_key(resp: Response, text: &str, key: &str) -> Response {
    if text.is_empty() || key.is_empty() {
        return tip(resp, text);
    }
    resp.on_hover_ui(|ui| {
        let k = tokens();
        let (tf, kf) = (Font::DATA.weight(Weight::Regular).tracking(0.05), Font::LABEL.upper(false));
        let label = Text::new(text, tf).color(Color32::WHITE);
        let tw = label.layout(ui.ctx(), 320.0).width.min(320.0);
        let kw = Text::new(key, kf).layout(ui.ctx(), f32::INFINITY).width + 2.0 * SPACE[0];
        let (row, _) = ui.allocate_exact_size(Vec2::new(tw + SPACE[1] + kw, tf.line.max(kf.line)), Sense::hover());
        let base = baseline(row, tf);
        label.baseline(base).paint_only(ui, row.with_max_x(row.left() + tw));
        let cap = Rect::from_center_size(Pos2::new(row.right() - kw / 2.0, row.center().y), Vec2::new(kw, kf.line));
        ui.painter().rect_stroke(cap, RADIUS_PLATE, Stroke::new(1.0f32, k.rim_bright), egui::StrokeKind::Inside);
        Text::new(key, kf).center().baseline(base).color(k.info).paint_only(ui, cap);
    })
}

/// A mouse button or both together: a mouse action's trigger ([`mouse_hints`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mouse {
    Left,
    Right,
    Both,
}

/// A small mouse (12 × 16 points) centred in `r`: its outline `dim`, the button `b` (or both) filled `lit`.
pub fn paint_mouse(p: &egui::Painter, r: Rect, b: Mouse, lit: Color32, dim: Color32) {
    let body = Rect::from_center_size(r.center(), Vec2::new(12.0, 16.0));
    let (mid, split) = (body.center().x, body.top() + 7.0);
    let stroke = Stroke::new(1.0f32, dim);
    if matches!(b, Mouse::Left | Mouse::Both) {
        let left = Rect::from_min_max(body.min, Pos2::new(mid, split)).shrink(1.5);
        p.rect_filled(left, egui::CornerRadius { nw: 4, ..Default::default() }, lit);
    }
    if matches!(b, Mouse::Right | Mouse::Both) {
        let right = Rect::from_min_max(Pos2::new(mid, body.top()), Pos2::new(body.right(), split)).shrink(1.5);
        p.rect_filled(right, egui::CornerRadius { ne: 4, ..Default::default() }, lit);
    }
    p.rect_stroke(body, 6, stroke, egui::StrokeKind::Inside);
    p.line_segment([Pos2::new(body.left(), split), Pos2::new(body.right(), split)], stroke);
    p.line_segment([Pos2::new(mid, body.top()), Pos2::new(mid, split)], stroke);
}

/// A control's mouse actions, one row each: the button as a small mouse, the action's name in the tooltip's body
/// style ([`tip_mouse`]'s content).
pub fn mouse_hints(ui: &mut Ui, rows: &[(Mouse, String)]) {
    let k = tokens();
    let f = Font::DATA.weight(Weight::Regular).tracking(0.05);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = SPACE[0];
        for (b, name) in rows {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE[1];
                let (g, _) = ui.allocate_exact_size(Vec2::new(14.0, f.line), Sense::hover());
                paint_mouse(ui.painter(), g, *b, k.info, k.rim_bright);
                let fit = Text::new(name, f).layout(ui.ctx(), 300.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(fit.width.min(300.0), fit.galley.size().y), Sense::hover());
                ui.painter().galley(rect.min, fit.galley, Color32::WHITE);
            });
        }
    });
}

/// A tooltip of mouse actions on `resp` ([`mouse_hints`]); none for no rows.
pub fn tip_mouse(resp: Response, rows: &[(Mouse, String)]) -> Response {
    if rows.is_empty() {
        return resp;
    }
    resp.on_hover_ui(|ui| mouse_hints(ui, rows))
}

/// A tooltip at a point (a game hot-spot's text where no egui widget is under the pointer): a deep panel beside `at`,
/// kept on the screen, up to three lines of 320 points.
pub fn tip_at(ctx: &egui::Context, id: &str, at: egui::Pos2, text: &str) {
    egui::Area::new(egui::Id::new(id)).order(egui::Order::Tooltip).interactable(false).fixed_pos(at + Vec2::new(16.0, 18.0)).constrain(true).fade_in(false).show(ctx, |ui| {
        panel(ui, PanelKind::Deep, 0.97, 8.0, |ui| {
            ui.set_max_width(320.0);
            let fit = Text::new(text, Font::DATA).lines(3).layout(ui.ctx(), 320.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(fit.width.min(320.0), fit.galley.size().y), Sense::hover());
            ui.painter().galley(rect.min, fit.galley, Color32::WHITE);
        });
    });
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

/// A select field (the boards' dropdowns): the value in a dark glass box with a chevron; a press (either button)
/// opens its rows under it in a deep panel, greyed rows take no pick, a press elsewhere closes it. Gives
/// `Some(None)` when it opens and `Some(Some(value))` for a row picked.
pub fn select(ui: &mut Ui, id: impl std::hash::Hash, value: &str, rows: &[(i32, impl AsRef<str>, bool)], size: Vec2, font: Font, ink: Color32) -> (Response, Option<Option<i32>>) {
    let k = tokens();
    let id = ui.id().with(id);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let open = ui.memory(|m| m.data.get_temp::<bool>(id).unwrap_or(false));
    let live = ui.is_enabled();
    let hot = hover_t(ui.ctx(), resp.id, live && resp.hovered()).max(open as u8 as f32);
    let p = ui.painter();
    p.rect_filled(r, RADIUS_PLATE, k.glass_deep);
    rim(p, r, RADIUS_PLATE, k.rim.lerp_to_gamma(k.rim_bright, hot));
    Text::new(value, font).shrink(font.size * 0.7).color(if live { ink } else { k.ink_muted }).paint(ui, Rect::from_min_max(r.min + egui::vec2(12.0, 0.0), r.max - egui::vec2(32.0, 0.0)));
    let c = egui::pos2(r.right() - 18.0, r.center().y);
    let d = if open { -3.0 } else { 3.0 };
    ui.painter().line(vec![c + egui::vec2(-5.0, -d * 0.5), c + egui::vec2(0.0, d * 0.5), c + egui::vec2(5.0, -d * 0.5)], Stroke::new(1.5f32, k.ink_silver));
    let resp = if live { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };
    let mut out = None;
    let mut open = open;
    if live && (resp.clicked() || resp.secondary_clicked()) {
        open = !open;
        ui.memory_mut(|m| m.data.insert_temp(id, open));
        if open {
            out = Some(None);
        }
    }
    // The rows show in the frame the field opens (egui's first frame of an area only sizes it).
    if !open || !live {
        return (resp, out);
    }
    let rh = (size.y - 10.0).max(26.0);
    let list = egui::Area::new(id.with("rows")).order(egui::Order::Foreground).fixed_pos(r.left_bottom() + egui::vec2(0.0, 4.0)).show(ui.ctx(), |ui| {
        let hgt = (rows.len().max(1) as f32 * rh + 12.0).min(420.0);
        let (frame, _) = ui.allocate_exact_size(egui::vec2(size.x, hgt), Sense::hover());
        menu_back(ui.painter(), frame);
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(frame.shrink(6.0)).layout(egui::Layout::top_down(Align::Min)));
        egui::ScrollArea::vertical().id_salt(id.with("scroll")).auto_shrink([false; 2]).show(&mut c, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (val, text, off) in rows {
                let text = text.as_ref();
                let (rr, row) = ui.allocate_exact_size(egui::vec2(size.x - 24.0, rh), Sense::click());
                let hot = hover_t(ui.ctx(), row.id, row.hovered() && !off);
                row_back(ui, rr, hot, text == value);
                Text::new(text, font.sized(font.size - 2.0)).shrink(font.size * 0.6).color(if *off { k.ink_muted } else { k.ink }).paint(ui, rr.shrink2(egui::vec2(8.0, 0.0)));
                let row = row.on_hover_cursor(if *off { egui::CursorIcon::NotAllowed } else { egui::CursorIcon::PointingHand });
                if (row.clicked() || row.secondary_clicked()) && !off {
                    out = Some(Some(*val));
                }
            }
        });
    });
    let at = ui.input(|i| i.pointer.interact_pos()).unwrap_or_default();
    if matches!(out, Some(Some(_))) || (ui.input(|i| i.pointer.any_click()) && !r.contains(at) && !list.response.rect.contains(at)) {
        ui.memory_mut(|m| m.data.insert_temp(id, false));
    }
    (resp, out)
}

// ---- Board pages: the shell of a screen that places its own panels (ADR 0076 pages) ----

/// The plates' height on a board page's foot (Exit, "Full text").
pub const FOOT_H: f32 = 44.0;

/// The bottom of a board page's content: the board's 40 above the screen's foot, squeezed with a shorter window.
pub fn board_bottom(ctx: &egui::Context) -> f32 {
    let h = ctx.screen_rect().height();
    h - super::BOARD_EDGE * super::squeeze(h)
}

/// A board page's panel filling the `ds::place`d area `size` big: its `kind` background, then the content's
/// rectangle `pad` inside it (the panel's common left edge and top).
pub fn board_panel(ui: &Ui, size: Vec2, kind: PanelKind, pad: Vec2) -> Rect {
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    paint_panel(ui.painter(), rect, kind, 0.9);
    rect.shrink2(pad)
}

/// A recessed picture well's frame around `r` (a talking head's clip): a cast-metal ring six points wide; its inside
/// stays clear for the picture drawn there.
pub fn picture_well(p: &egui::Painter, r: Rect) {
    let k = tokens();
    p.rect_stroke(r.expand(3.0), RADIUS_PANEL + 2, Stroke::new(6.0f32, k.metal_shadow), egui::StrokeKind::Middle);
    rim(p, r.expand(1.0), RADIUS_PANEL, k.metal_dark);
    relief(p, r.expand(6.0), RADIUS_PANEL + 4);
}

/// A model's turntable in `r` (the 3D drawn under it): a faint rim-blue grid of 5 × 3 cells.
pub fn turntable_grid(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(1.0f32, tokens().rim.gamma_multiply(0.5));
    for i in 0..=5 {
        p.vline(r.left() + r.width() * i as f32 / 5.0, r.y_range(), s);
    }
    for i in 0..=3 {
        p.hline(r.x_range(), r.top() + r.height() * i as f32 / 3.0, s);
    }
}

/// A board page's foot plate `width` wide at `x` ([`FOOT_H`] high on [`board_bottom`]); true when pressed with either
/// button. The Exit plate is [`exit_plate`].
pub fn foot_plate(ctx: &egui::Context, id: &str, x: f32, width: f32, label: &str, enabled: bool) -> bool {
    let r = Rect::from_min_size(Pos2::new(x, board_bottom(ctx) - FOOT_H), Vec2::new(width, FOOT_H));
    place(ctx, id, r, |ui| either(&ui.add_enabled(enabled, Plate::new(label).width(width).height(FOOT_H)))).inner
}

/// A board page's Exit plate: [`super::EXIT_W`] wide at the right edge of the foot.
pub fn exit_plate(ctx: &egui::Context, id: &str, label: &str, enabled: bool) -> bool {
    let x = ctx.screen_rect().width() - super::BOARD_EDGE - super::EXIT_W;
    foot_plate(ctx, id, x, super::EXIT_W, label, enabled)
}

// ---- Table rows ----

/// The cells of a list table's line `row` ([`columns`] of `widths`, a row's text inset in, [`SPACE`]`[2]` apart): the
/// same for its [`table_head`] and its [`table_row`]s, so heads and cells share their edges.
pub fn table_cols(row: Rect, widths: &[f32]) -> Vec<Rect> {
    columns(row.shrink2(Vec2::new(SPACE[1], 0.0)), widths, SPACE[2])
}

/// A table's line `height` high: its `cells` (text, font, colour; right-aligned where `right`) in [`table_cols`] of
/// `widths` on the first cell's baseline; `selected` Some makes it a clickable list row (hover rim, the selected
/// one's sweep).
pub fn table_row(ui: &mut Ui, height: f32, widths: &[f32], cells: &[(&str, Font, Color32)], right: &[bool], selected: Option<bool>) -> Response {
    let sense = if selected.is_some() { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), sense);
    if let Some(on) = selected {
        let l = look(ui, &resp, false);
        row_back(ui, rect, l.hover, on);
    }
    let base = cells.first().map_or(rect.center().y, |c| baseline(rect, c.1));
    let mut cut = false;
    for (i, (c, (t, f, ink))) in table_cols(rect, widths).into_iter().zip(cells).enumerate() {
        let t = Text::new(t, *f).color(*ink).baseline(base);
        cut |= if right.get(i).copied().unwrap_or(false) { t.right() } else { t }.paint_only(ui, c).cut;
    }
    match cells.first() {
        Some((t, ..)) => tip_if(resp, cut, t),
        None => resp,
    }
}

// ---- Typed text ----

/// A dialogue's or a message's body in `rect`: the `pieces` (text, colour) wrapped in `font` (its `line` the lines'
/// pitch, at most `max_width` wide), scrolling and kept at their end; `caret` a blinking block after the last letter (the
/// typer still running). The area's response takes a press (to type the rest at once).
pub fn typed_text(ui: &mut Ui, id: &str, rect: Rect, pieces: &[(String, Color32)], font: Font, max_width: f32, caret: bool) -> Response {
    let line = font.line;
    let resp = ui.interact(rect, ui.id().with(id), Sense::click());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(Align::Min)));
    egui::ScrollArea::vertical().id_salt(id).auto_shrink([false; 2]).stick_to_bottom(true).show(&mut child, |ui| {
        let w = rect.width().min(max_width) - 16.0;
        let mut job = egui::text::LayoutJob { wrap: egui::text::TextWrapping { max_width: w, ..Default::default() }, ..Default::default() };
        for (t, c) in pieces {
            job.append(t, 0.0, egui::TextFormat { font_id: font.id(), color: *c, extra_letter_spacing: font.spacing(), line_height: Some(line), ..Default::default() });
        }
        let galley = ui.fonts(|x| x.layout_job(job));
        let (r, _) = ui.allocate_exact_size(Vec2::new(rect.width() - 16.0, galley.size().y.max(line)), Sense::hover());
        let last = galley.rows.last().map(|row| row.rect());
        ui.painter().galley(r.min, galley, tokens().ink);
        if caret && (ui.input(|i| i.time) * 2.5) as i64 % 2 == 0 {
            let (x, y) = last.map_or((0.0, 0.0), |l| (l.right(), l.top()));
            let h = font.size + 2.0;
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(r.left() + x + 2.0, r.top() + y + (line - h) / 2.0), Vec2::new(h / 2.0, h)), 0, tokens().ink);
        }
    });
    if caret {
        ui.ctx().request_repaint();
    }
    resp
}
