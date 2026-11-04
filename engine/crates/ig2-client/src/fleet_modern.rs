//! Ours (ADR 0077): the starmap's fleet info panel (re/screens/starmap.md §19) and transfer panel (§13) as the
//! design boards' panels on the design kit (ADR 0073), under the modern starmap HUD (ADR 0065, 0079). The panels
//! take no input themselves: the window hands the pointer to the original's hot-spot code moved where the panels
//! drew their controls ([`State::pointer`]), so the press-and-hold sale menu, the held transfer cells (left one
//! more each step, right all), the scroll bar, the exit, the keys and the orders stay the original's. The units'
//! models are the original's render target, cut out of it into the cards' wells ([`pictures`]).

use egui::{pos2, vec2, Color32, Pos2, Rect, Stroke, Vec2};
use ig2_sim::save::{va, SaveState, TECH_SIZE};

use crate::gui::ds::{self, Font, Text, Weight};
use crate::orders::Selection;
use crate::starmap_panels::Panels;

/// One unit card: the design's name, how many, the original's highlight.
#[derive(Debug, Clone, Default)]
pub struct Card {
    pub name: String,
    pub count: i32,
    pub selected: bool,
    /// The count in the warning colour (a fleet's vehicles over its capacity).
    pub warn: bool,
}

/// A side's vehicle capacity: the label (`GetString(2, 5, 2)`), the value and how full.
#[derive(Debug, Clone, Default)]
pub struct Cap {
    pub label: String,
    pub value: String,
    pub fraction: Option<f32>,
    pub over: bool,
}

/// The selected design's data (`FlottaInfoText2d`'s lower half).
#[derive(Debug, Clone, Default)]
pub struct Detail {
    pub name: String,
    pub rows: Vec<(String, String)>,
    /// The pod slots: the tech's name and count, the count at the slot's most; `None` = empty.
    pub pods: Vec<Option<(String, String, bool)>>,
    pub empty: String,
    pub pods_label: String,
    /// The hull's name beside the design's (the vehicles' panel).
    pub kind: String,
}

/// The sale menu open (`StarmapSelectDeleteMode`): its rows, the one under the pointer, its virtual box.
#[derive(Debug, Clone, Default)]
pub struct Sale {
    pub rows: Vec<(String, bool)>,
    pub hovered: i32,
    pub at: [i32; 2],
}

/// The fleet info panel.
#[derive(Debug, Clone, Default)]
pub struct FleetInfo {
    pub title: String,
    pub owner: String,
    pub colour: Color32,
    pub status: String,
    /// The eight shown places (k = item − first).
    pub cards: [Option<Card>; 8],
    pub first: i32,
    pub total: i32,
    pub detail: Option<Detail>,
    pub cap: Option<Cap>,
    pub exit: String,
    pub sale: Option<Sale>,
}

/// One side of the transfer panel.
#[derive(Debug, Clone, Default)]
pub struct Side {
    pub title: String,
    pub owner: String,
    pub colour: Color32,
    pub cards: [Option<Card>; 8],
    pub cap: Cap,
}

/// The transfer panel: the left holder (`f1`) and the right one, the shared list's scroll.
#[derive(Debug, Clone, Default)]
pub struct Transfer {
    pub sides: [Side; 2],
    pub first: i32,
    pub total: i32,
    pub exit: String,
}

/// The vehicles' panel (`StarmapTanksInfo`, re/screens/starmap.md §20): the planet's designs as rows, the selected
/// one's model and data.
#[derive(Debug, Clone, Default)]
pub struct TankInfo {
    pub title: String,
    /// "Stationed tanks · n".
    pub heading: String,
    pub cards: [Option<Card>; 8],
    pub first: i32,
    pub total: i32,
    pub detail: Option<Detail>,
    pub exit: String,
    pub sale: Option<Sale>,
}

/// What the panels show this frame.
#[derive(Debug, Clone)]
pub enum View {
    Fleet(FleetInfo),
    Transfer(Transfer),
    Tanks(TankInfo),
}

/// Where the panels put the original's controls this frame (points), and the wells for [`pictures`].
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Virtual → points: x0, y0, scale.
    map: [f32; 3],
    panels: Vec<Rect>,
    /// A control's rect and the virtual point of the original's hot-spot it is.
    spots: Vec<(Rect, [f32; 2])>,
    /// The scroll bar's track: the virtual y runs 30..359 along it.
    track: Option<Rect>,
    /// The sale menu's rows and the virtual points of the original's.
    menu: Option<(Rect, Vec<Spot>)>,
    /// Where the sale menu opened (points).
    sale_at: Option<Pos2>,
    /// The wells: the source rectangle on the virtual 640 × 480 screen, the well in pixels, the opacity.
    pub wells: Vec<([f32; 4], Rect, f32)>,
    /// The original's tooltip text shown now (the window's, `window/pointer.rs`).
    pub tip: Option<String>,
}

/// A control's rect and the virtual point of the original's it is.
type Spot = (Rect, [f32; 2]);

/// A point of the original's panels that is none of their controls (hot-spot 0x58: the panel takes the press).
const NEUTRAL: [f32; 2] = [30.0, 40.0];

impl State {
    /// The pointer `at` (virtual) as the original's panel code should see it: over a card or a plate the middle of
    /// its hot-spot, along the scroll bar the original's bar at that height, over the sale menu the original's row,
    /// over the rest of the panels the original panel's empty part. Elsewhere unchanged.
    /// The scroll bar held (`Mouse_Megfogva` latches hot-spot 0x57, which reads the pointer's height only): the
    /// original's bar at the pointer's height along the drawn track, wherever across.
    pub fn scroll_pointer(&self, at: [f32; 2]) -> [f32; 2] {
        let Some(t) = self.track else { return self.pointer(at) };
        let y = self.map[1] + at[1] * self.map[2];
        [320.0, 30.0 + ((y - t.top()) / t.height()).clamp(0.0, 1.0) * 329.0]
    }

    pub fn pointer(&self, at: [f32; 2]) -> [f32; 2] {
        if self.panels.is_empty() {
            return at;
        }
        let m = self.map;
        let p = pos2(m[0] + at[0] * m[2], m[1] + at[1] * m[2]);
        // While the sale menu is open only its rows answer: a card beside it would land in the original's rows.
        if let Some((r, rows)) = &self.menu {
            return if r.contains(p) { rows.iter().find(|x| x.0.contains(p)).map_or(NEUTRAL, |x| x.1) } else { NEUTRAL };
        }
        if let Some(t) = self.track.filter(|t| t.expand2(vec2(8.0, 0.0)).contains(p)) {
            return [320.0, 30.0 + ((p.y - t.top()) / t.height()).clamp(0.0, 1.0) * 329.0];
        }
        if let Some(s) = self.spots.iter().find(|s| s.0.contains(p)) {
            return s.1;
        }
        // The original's cells, scroll bar and exit button where they were answer no more.
        if self.panels.iter().any(|r| r.contains(p)) || crate::kit::inside([0, 24, 640, 416], at) {
            NEUTRAL
        } else {
            at
        }
    }
}

const EDGE: f32 = 24.0;
const HEAD: f32 = 64.0;
const FOOT: f32 = 64.0;
const PAD: Vec2 = vec2(22.0, 20.0);
const GAP: f32 = 14.0;
/// The fleet board's panel: 1320 wide, its unit column 640.
const FLEET_W: f32 = 1320.0;
const FLEET_LEFT: f32 = 640.0;
/// The transfer board's two panels, 620 wide, 80 apart.
const SIDE_W: f32 = 620.0;
const SIDES_GAP: f32 = 80.0;
const TITLE: Font = Font { size: 26.0, line: 30.0, weight: Weight::SemiBold, tracking: 0.12, upper: false, lcd: false };
const STATUS: Font = Font { size: 14.0, line: 18.0, weight: Weight::Medium, tracking: 0.1, upper: true, lcd: false };
const CARD_NAME: Font = Font { size: 15.0, line: 20.0, weight: Weight::Medium, tracking: 0.1, upper: false, lcd: false };
const DETAIL_NAME: Font = Font { size: 22.0, line: 26.0, weight: Weight::SemiBold, tracking: 0.1, upper: false, lcd: false };
const OWNER: Font = Font { size: 13.0, line: 16.0, weight: Weight::SemiBold, tracking: 0.16, upper: true, lcd: false };
const EXIT: Font = Font { size: 18.0, line: 22.0, weight: Weight::Medium, tracking: 0.16, upper: false, lcd: false };


/// The virtual centre of the original's cell `k` (left panel; `right` the transfer's right one) and its model's
/// place on the render target (`InfoPanel::poses`, `PanelModels::render`: x 91 + 129·(k mod 2), y 84 + 70·(k div 2),
/// the right panel 328 further).
fn cell(k: usize, right: bool) -> ([f32; 2], [f32; 4]) {
    let x = 91.0 + 129.0 * (k % 2) as f32 + if right { 328.0 } else { 0.0 };
    let y = 84.0 + 70.0 * (k / 2) as f32;
    ([x, y + 8.0], [x - 48.0, y - 26.0, 96.0, 52.0])
}

/// The fleet panel's big model on the render target (`InfoForgatAlap` at (150, −150): about (470, 90)).
const PREVIEW: [f32; 4] = [375.0, 40.0, 190.0, 100.0];

fn hover(ui: &egui::Ui, r: Rect) -> bool {
    ui.ctx().input(|i| i.pointer.hover_pos()).is_some_and(|p| r.contains(p))
}

/// One frame of the panels over the starmap (after the HUD's).
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) {
    let ppp = ctx.pixels_per_point();
    let screen = ctx.screen_rect();
    let virt = crate::font::Virtual::new([screen.width() * ppp, screen.height() * ppp]);
    *st = State { map: [virt.x0 / ppp, virt.y0 / ppp, virt.k / ppp], sale_at: st.sale_at, tip: st.tip.take(), ..Default::default() };
    let top = crate::starmap_modern::bar_bottom(ctx) + 28.0;
    // Room down to the selection card (24 from the bottom, about 150 high).
    let room = (screen.bottom() - top - EDGE - 200.0).max(360.0);
    let t = ds::open_t(ctx, egui::Id::new("fmh-open"), true);
    egui::Area::new(egui::Id::new("fmh")).order(egui::Order::Background).interactable(false).fixed_pos(screen.min).fade_in(false).show(ctx, |ui| {
        ui.set_opacity(t);
        let slide = vec2(0.0, (1.0 - t) * 12.0);
        match v {
            View::Fleet(f) => fleet(ui, f, st, screen, top, room, slide),
            View::Transfer(tr) => transfer(ui, tr, st, screen, top, room, slide),
            View::Tanks(ti) => tanks(ui, ti, st, screen, top, slide),
        }
    });
    // The original's tooltip of the spot under the pointer (`Starmap_MouseInit2`'s texts, its rest timer), in the
    // kit's look.
    if let (Some(text), Some(p)) = (&st.tip, ctx.input(|i| i.pointer.hover_pos())) {
        ds::tip_at(ctx, "fmh-tip", p, text);
    }
    let o = t;
    // The sale menu lies over the models (drawn after the panels): the wells under it stay empty while it is open.
    if let Some((m, _)) = &st.menu {
        st.wells.retain(|w| !w.1.intersects(*m));
    }
    for w in &mut st.wells {
        w.1 = Rect::from_min_max((w.1.min.to_vec2() * ppp).to_pos2(), (w.1.max.to_vec2() * ppp).to_pos2());
        w.2 *= o;
    }
}

/// A panel with the boards' header strip: the background, the header, its rim lines; the header's rect.
fn panel(ui: &egui::Ui, r: Rect, title: &str, owner: &str, colour: Color32, status: &str) {
    let k = ds::tokens();
    let p = ui.painter();
    ds::paint_panel(p, r, ds::PanelKind::Glass, 0.97);
    let head = Rect::from_min_size(r.min, vec2(r.width(), HEAD));
    ds::header_strip(p, head);
    let inner = head.shrink2(vec2(PAD.x, 0.0));
    let base = ds::fit::baseline(inner, TITLE);
    let w = Text::new(title, TITLE).color(k.ink).baseline(base).paint(ui, inner.with_max_x(inner.left() + inner.width() * 0.5)).width;
    let mut x = inner.left() + w + 18.0;
    // The owner and the status centred on the title's capitals (the board's header row centres its items).
    let mid = |f: Font| base - (TITLE.cap() - f.cap()) / 2.0;
    if !owner.is_empty() {
        let f = Text::new(owner, OWNER).color(colour).layout(ui.ctx(), (inner.right() - x) * 0.5);
        ds::glowing_galley(p, pos2(x, mid(OWNER) - f.baseline()), &f.galley, colour);
        x += f.width + 18.0;
    }
    if !status.is_empty() {
        Text::new(status, STATUS).color(k.ink_muted).baseline(mid(STATUS)).paint(ui, Rect::from_min_max(pos2(x, inner.top()), inner.max));
    }
}

/// A unit card: the well with the model and the count (or the count under it), the name; the original's highlight
/// as the list's selected look, the pointer's as the hover rim. `round` = the transfer's button glyph.
fn card(ui: &mut egui::Ui, r: Rect, c: &Card, src: [f32; 4], alpha: f32, round: Option<(ds::Icon, bool)>, st: &mut State) {
    let k = ds::tokens();
    let p = ui.painter();
    let hot = ds::hover_t(ui.ctx(), ui.id().with(("fmh-card", r.min.x as i32, r.min.y as i32)), hover(ui, r));
    if c.selected {
        ds::vgrad(p, r, ds::RADIUS_PANEL, &[(0.0, Color32::from_white_alpha(36)), (1.0, Color32::from_white_alpha(5))]);
        ds::rim(p, r, ds::RADIUS_PANEL, k.ink_silver);
    } else {
        ds::rim(p, r, ds::RADIUS_PANEL, if c.warn { k.enemy } else { k.rim.lerp_to_gamma(k.rim_bright, hot) });
    }
    let inner = r.shrink(8.0);
    // The board's 130-point well in a 196-point card; the transfer's button row under the name.
    let (foot, well_h) = if round.is_some() { (42.0 + 8.0 + 14.0, 118.0) } else { (0.0, 130.0) };
    let well = Rect::from_min_max(inner.min, pos2(inner.right(), (inner.top() + well_h).min(inner.bottom() - 20.0 - 8.0 - foot)));
    p.rect_filled(well, ds::RADIUS_PLATE, k.glass_deep);
    ds::radial(p, well, ds::RADIUS_PLATE, [0.5, 0.6], &[(0.0, k.rim_bright.gamma_multiply(0.18 + 0.12 * hot)), (0.7, Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    ds::rim(p, well, ds::RADIUS_PLATE, k.rim);
    st.wells.push((src, well.shrink(2.0), alpha * ui.opacity()));
    let name = Rect::from_min_size(pos2(inner.left(), well.bottom() + 8.0), vec2(inner.width(), 20.0));
    let name_text = Text::new(&c.name, CARD_NAME).color(k.ink).shrink(12.0);
    if round.is_some() { name_text.paint(ui, name) } else { name_text.center().paint(ui, name) };
    let count = c.count.to_string();
    let lcd = |at: Rect| {
        ds::lcd_window(p, at);
        Text::new(&count, Font::READOUT.sized(18.0)).center().color(if c.warn { k.enemy } else { k.lcd_ink }).paint_only(ui, at.shrink2(vec2(4.0, 0.0)));
    };
    match round {
        None => {
            let w = 14.0 + 11.0 * count.len().max(1) as f32;
            lcd(Rect::from_min_size(well.right_bottom() - vec2(6.0 + w, 6.0 + 24.0), vec2(w, 24.0)));
        }
        Some((icon, right)) => {
            let row = Rect::from_min_size(pos2(inner.left(), name.bottom() + 8.0), vec2(inner.width(), 42.0));
            let w = 14.0 + 11.0 * count.len().max(3) as f32;
            let (l, b) = if right { (Rect::from_min_size(pos2(row.right() - w, row.center().y - 12.0), vec2(w, 24.0)), row.left()) } else { (Rect::from_min_size(pos2(row.left(), row.center().y - 12.0), vec2(w, 24.0)), row.right() - 42.0) };
            lcd(l);
            let br = Rect::from_min_size(pos2(b, row.center().y - 21.0), vec2(42.0, 42.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(br));
            ds::forced(hot > 0.5, hot > 0.5 && ds::any_down(ui.ctx()), || ds::round_button(&mut child, ds::Glyph::Icon(icon), true, false));
        }
    }
}

/// A plate drawn without input (the pointer's remap presses the original's spot), lit by the pointer.
fn plate(ui: &mut egui::Ui, r: Rect, text: &str) {
    let hot = hover(ui, r);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
    ds::forced(hot, hot && ds::any_down(ui.ctx()), || child.add_sized(r.size(), ds::Plate::new(text).width(r.width()).height(r.height()).font(EXIT)));
}

/// The capacity footer's label, bar and value from `x`.
fn capacity(ui: &egui::Ui, foot: Rect, c: &Cap, bar_w: f32) -> f32 {
    let k = ds::tokens();
    let p = ui.painter();
    let base = ds::fit::baseline(foot, Font::DATA);
    let label = c.label.trim().trim_end_matches(':');
    let w = Text::new(label, Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, foot.with_max_x(foot.left() + 180.0)).width;
    let mut x = foot.left() + w + 16.0;
    let tone = if c.over { k.enemy } else { k.info };
    if let Some(f) = c.fraction {
        let bar = Rect::from_min_size(pos2(x, foot.center().y - 4.0), vec2(bar_w, 8.0));
        p.rect_filled(bar, 4, k.glass_deep);
        let f = ui.ctx().animate_value_with_time(ui.id().with(("fmh-cap", bar.min.x as i32)), f.clamp(0.0, 1.0), ds::anim(ds::GLIDE_TIME));
        if f > 0.0 {
            p.rect_filled(bar.with_max_x(bar.left() + (bar_w * f).max(8.0)), 4, tone);
        }
        ds::rim(p, bar, 4, if c.over { k.enemy } else { k.rim });
        x += bar_w + 16.0;
    }
    x + Text::new(&c.value, Font::DATA).color(tone).baseline(base).paint(ui, Rect::from_min_max(pos2(x, foot.top()), pos2(x + 120.0, foot.bottom()))).width
}

/// The cards' grid in `body`: 3 across, the board's `row_h` high while two rows hold the cards, three rows sharing
/// the height when the original's eight places fill more.
fn grid(body: Rect, n: usize, row_h: f32, three: bool) -> (Vec<Rect>, f32) {
    let w = ((body.width() - 2.0 * GAP) / 3.0).floor();
    let rows = if three { 3.0 } else { 2.0 };
    let h = row_h.min(((body.height() - (rows - 1.0) * GAP) / rows).floor());
    let cells = (0..n).map(|k| Rect::from_min_size(body.min + vec2((k % 3) as f32 * (w + GAP), (k / 3) as f32 * (h + GAP)), vec2(w, h))).collect();
    (cells, h)
}

fn scroll(ui: &egui::Ui, body: Rect, first: i32, total: i32, st: &mut State) {
    if total <= 8 {
        return;
    }
    let k = ds::tokens();
    let track = Rect::from_min_max(pos2(body.right() + 6.0, body.top()), pos2(body.right() + 10.0, body.bottom()));
    let p = ui.painter();
    p.rect_filled(track, 2, k.glass_deep);
    ds::rim(p, track, 2, k.rim);
    let h = (track.height() * 8.0 / total as f32).max(24.0);
    let y = track.top() + (track.height() - h) * first as f32 / (total - 8).max(1) as f32;
    let hot = hover(ui, track.expand2(vec2(8.0, 0.0)));
    p.rect_filled(Rect::from_min_size(pos2(track.left(), y), vec2(track.width(), h)), 2, if hot { k.ink_silver } else { k.rim_bright });
    st.track = Some(track);
}

fn fleet(ui: &mut egui::Ui, f: &FleetInfo, st: &mut State, screen: Rect, top: f32, room: f32, slide: Vec2) {
    let k = ds::tokens();
    let w = FLEET_W.min(screen.width() - 2.0 * EDGE);
    let h = 700.0f32.min(room);
    let r = Rect::from_min_size(pos2(screen.center().x - w / 2.0, top), vec2(w, h)).translate(slide);
    panel(ui, r, &f.title, &f.owner, f.colour, &f.status);
    st.panels.push(r);
    let p = &ui.painter().clone();
    let split = r.left() + (FLEET_LEFT * w / FLEET_W).round();
    let body = Rect::from_min_max(pos2(r.left(), r.top() + HEAD), pos2(r.right(), r.bottom() - FOOT));
    p.vline(split - 0.5, body.y_range(), Stroke::new(1.0f32, k.rim));
    p.hline(r.x_range(), body.bottom() + 0.5, Stroke::new(1.0f32, k.rim));
    // The units: 3 across, the original's eight places.
    let cards = Rect::from_min_max(body.min + PAD, pos2(split - PAD.x - if f.total > 8 { 12.0 } else { 0.0 }, body.bottom() - PAD.y));
    let (cells, _) = grid(cards, 8, 196.0, f.cards[6].is_some());
    for (i, c) in f.cards.iter().enumerate() {
        let Some(c) = c else { continue };
        let (centre, src) = cell(i, false);
        card(ui, cells[i], c, src, 1.0, None, st);
        st.spots.push((cells[i], centre));
    }
    scroll(ui, cards, f.first, f.total, st);
    // The selected design: its model on the void, its name, the data table and the pods.
    let right = Rect::from_min_max(pos2(split, body.top()), body.max);
    // The model's view gives way to the data where the screen is short (a large UI scale).
    let need = f.detail.as_ref().map_or(0.0, |d| 58.0 + 12.0 + (d.rows.len() as f32 * 28.0).max(24.0 + d.pods.len() as f32 * 28.0 + 12.0));
    let pic = Rect::from_min_size(right.min, vec2(right.width(), (body.height() - need).clamp(110.0, 290.0).floor()));
    p.rect_filled(pic, 0, k.void);
    p.hline(pic.x_range(), pic.bottom() - 0.5, Stroke::new(1.0f32, k.rim));
    if let Some(d) = &f.detail {
        let wf = (PREVIEW[2] / PREVIEW[3]).min(pic.width() / pic.height());
        let ph = (pic.height() - 8.0).min((pic.width() - 48.0) / wf);
        st.wells.push((PREVIEW, Rect::from_center_size(pic.center(), vec2(ph * wf, ph)), ui.opacity()));
        let inner = Rect::from_min_max(pic.left_bottom() + vec2(24.0, 18.0), right.max - vec2(24.0, 12.0));
        Text::new(&d.name, DETAIL_NAME).color(k.ink).shrink(15.0).paint(ui, Rect::from_min_size(inner.min, vec2(inner.width(), 26.0)));
        let cols = Rect::from_min_max(inner.min + vec2(0.0, 40.0), inner.max);
        let half = ((cols.width() - 32.0) / 2.0).floor();
        let pitch = (cols.height() / d.rows.len().max(1) as f32).clamp(20.0, 28.0);
        for (n, (label, value)) in d.rows.iter().enumerate() {
            let row = Rect::from_min_size(cols.min + vec2(0.0, n as f32 * pitch), vec2(half, 20.0));
            let base = ds::fit::baseline(row, Font::DATA);
            let (a, b) = row.split_left_right_at_x(row.left() + 146.0_f32.min(half * 0.6));
            Text::new(label.trim().trim_end_matches(':'), Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, a.with_max_x(a.right() - 8.0));
            Text::new(value, Font::DATA).color(k.info).baseline(base).paint(ui, b);
        }
        // The pods: a recessed list, an empty slot muted.
        let list = Rect::from_min_size(cols.min + vec2(half + 32.0, 0.0), vec2(half, 0.0));
        let mut y = list.top();
        if !d.pods_label.is_empty() {
            Text::new(&d.pods_label, Font::LABEL).color(k.ink_muted).paint(ui, Rect::from_min_size(pos2(list.left(), y), vec2(half, 18.0)));
            y += 24.0;
        }
        let row_h = ((cols.bottom() - y - 12.0) / d.pods.len().max(1) as f32).clamp(20.0, 28.0);
        let well = Rect::from_min_size(pos2(list.left(), y), vec2(half, row_h * d.pods.len() as f32 + 12.0));
        if !d.pods.is_empty() {
            p.rect_filled(well, ds::RADIUS_PLATE, k.glass_deep);
            ds::rim(p, well, ds::RADIUS_PLATE, k.rim);
        }
        for (n, pod) in d.pods.iter().enumerate() {
            let row = Rect::from_min_size(pos2(well.left() + 4.0, well.top() + 6.0 + n as f32 * row_h), vec2(half - 8.0, row_h)).shrink2(vec2(8.0, 0.0));
            let base = ds::fit::baseline(row, Font::DATA);
            match pod {
                None => _ = Text::new(&d.empty, Font::DATA).color(k.ink_muted).baseline(base).paint(ui, row),
                Some((name, count, full)) => {
                    let (a, b) = row.split_left_right_at_x(row.right() - 36.0);
                    Text::new(name, Font::DATA).color(k.ink).baseline(base).paint(ui, a.with_max_x(a.right() - 6.0));
                    // The original's slot at its most in its own colour (0x22 against 3): here the brighter ink.
                    Text::new(count, Font::DATA).right().color(if *full { k.info } else { k.info.gamma_multiply(0.8) }).baseline(base).paint(ui, b);
                }
            }
        }
    }
    // The footer: the capacity, the exit plate (hot-spot 9).
    let foot = Rect::from_min_max(pos2(r.left() + PAD.x, body.bottom()), pos2(r.right() - PAD.x, r.bottom()));
    if let Some(c) = &f.cap {
        capacity(ui, foot, c, 200.0);
    }
    let ew = Text::new(&f.exit, EXIT).layout(ui.ctx(), f32::INFINITY).width.clamp(80.0, 220.0) + 80.0;
    let exit = Rect::from_min_size(pos2(foot.right() - ew, foot.center().y - 22.0), vec2(ew, 44.0));
    plate(ui, exit, &f.exit);
    st.spots.push((exit, [527.0, 410.0]));
    if let Some(s) = &f.sale {
        sale(ui, s, st, screen);
    } else {
        st.sale_at = None;
    }
}

/// The sale menu at the pointer of the press (`smSelectX/Y` = the pointer + (14, −6)), the original's rows lit by
/// the pointer, a row that can't be had muted.
fn sale(ui: &egui::Ui, s: &Sale, st: &mut State, screen: Rect) {
    let k = ds::tokens();
    let at = *st.sale_at.get_or_insert_with(|| ui.ctx().input(|i| i.pointer.hover_pos()).unwrap_or(screen.center()) + vec2(14.0, -6.0));
    let font = Font::DATA;
    let w = s.rows.iter().map(|r| Text::new(&r.0, font).layout(ui.ctx(), f32::INFINITY).width).fold(120.0, f32::max) + 24.0;
    let row_h = 30.0;
    let mut r = Rect::from_min_size(at, vec2(w + 12.0, row_h * s.rows.len() as f32 + 12.0));
    r = r.translate(vec2((screen.right() - 8.0 - r.right()).min(0.0), (screen.bottom() - 8.0 - r.bottom()).min(0.0)));
    let p = ui.painter().clone().with_layer_id(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("fmh-sale")));
    ds::shadow(&p, r, ds::RADIUS_PANEL, 8, 24, 0, Color32::from_black_alpha(204));
    ds::paint_panel(&p, r, ds::PanelKind::Deep, 0.97);
    let mut rows = Vec::new();
    for (n, (text, ok)) in s.rows.iter().enumerate() {
        let row = Rect::from_min_size(r.min + vec2(6.0, 6.0 + n as f32 * row_h), vec2(w, row_h));
        let on = *ok && s.hovered == n as i32 + 1;
        if on {
            ds::hgrad(&p, row, ds::RADIUS_PLATE, &[(0.0, Color32::from_white_alpha(46)), (1.0, Color32::from_white_alpha(5))]);
            ds::rim(&p, row, ds::RADIUS_PLATE, k.ink_silver);
        }
        let inner = row.shrink2(vec2(10.0, 0.0));
        let f = Text::new(text, font).color(if *ok { k.ink } else { k.ink_muted }).layout(ui.ctx(), inner.width());
        p.galley(pos2(inner.left(), ds::fit::baseline(inner, font) - f.baseline()), f.galley, Color32::WHITE);
        rows.push((row, [s.at[0] as f32 + 1.0, s.at[1] as f32 + 13.0 * n as f32 + 6.0]));
    }
    st.menu = Some((r, rows));
}

fn transfer(ui: &mut egui::Ui, t: &Transfer, st: &mut State, screen: Rect, top: f32, room: f32, slide: Vec2) {
    let k = ds::tokens();
    let sw = SIDE_W.min((screen.width() - 2.0 * EDGE - SIDES_GAP) / 2.0);
    let h = 700.0f32.min(room);
    let x0 = screen.center().x - sw - SIDES_GAP / 2.0;
    for (s, side) in t.sides.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x0 + s as f32 * (sw + SIDES_GAP), top), vec2(sw, h)).translate(slide);
        panel(ui, r, &side.title, &side.owner, side.colour, "");
        st.panels.push(r);
        let body = Rect::from_min_max(pos2(r.left(), r.top() + HEAD), pos2(r.right(), r.bottom() - FOOT));
        ui.painter().hline(r.x_range(), body.bottom() + 0.5, Stroke::new(1.0f32, k.rim));
        let cards = Rect::from_min_max(body.min + PAD, body.max - PAD - vec2(if t.total > 8 { 12.0 } else { 0.0 }, 0.0));
        let (cells, _) = grid(cards, 8, 224.0, side.cards[6].is_some());
        let right = s == 1;
        for (i, c) in side.cards.iter().enumerate() {
            let Some(c) = c else { continue };
            let (centre, src) = cell(i, right);
            let icon = if right { ds::Icon::ChevronLeft } else { ds::Icon::ChevronRight };
            // An empty side's model is already three quarters transparent on the target.
            card(ui, cells[i], c, src, 1.0, Some((icon, right)), st);
            st.spots.push((cells[i], centre));
        }
        if right {
            scroll(ui, cards, t.first, t.total, st);
        }
        let foot = Rect::from_min_max(pos2(r.left() + PAD.x, body.bottom()), pos2(r.right() - PAD.x, r.bottom()));
        capacity(ui, foot, &side.cap, 160.0);
        if right {
            let ew = Text::new(&t.exit, EXIT).layout(ui.ctx(), f32::INFINITY).width.clamp(80.0, 200.0) + 72.0;
            let exit = Rect::from_min_size(pos2(foot.right() - ew, foot.center().y - 22.0), vec2(ew, 44.0));
            plate(ui, exit, &t.exit);
            st.spots.push((exit, [527.0, 410.0]));
        }
    }
    // The exchange mark between the panels.
    let c = pos2(screen.center().x, top + h * 0.5 - 28.0) + slide;
    ds::Icon::Swap.paint(ui.painter(), Rect::from_center_size(c, vec2(28.0, 28.0)), k.ink_silver);
}

/// The tank info board: the list panel 560 wide, the data panel 760, 40 apart, under a title plate.
const TANKS_LIST_W: f32 = 560.0;
const TANKS_DATA_W: f32 = 760.0;
const TANKS_GAP: f32 = 40.0;
const ROW_NAME: Font = Font { size: 19.0, line: 24.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
const ROW_COUNT: Font = Font { size: 24.0, line: 28.0, weight: Weight::SemiBold, tracking: 0.0, upper: false, lcd: false };
const TANK_NAME: Font = Font { size: 30.0, line: 34.0, weight: Weight::SemiBold, tracking: 0.1, upper: false, lcd: false };
const TANK_KIND: Font = Font { size: 16.0, line: 20.0, weight: Weight::Medium, tracking: 0.1, upper: false, lcd: false };
const TANK_DATA: Font = Font { size: 17.0, line: 20.0, weight: Weight::Regular, tracking: 0.1, upper: false, lcd: false };
const TANK_POD: Font = Font { size: 17.0, line: 22.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };

/// A vehicle row: the model's well, the name, the count; the original's highlight as the list's selected look.
fn tank_row(ui: &egui::Ui, r: Rect, c: &Card, src: [f32; 4], st: &mut State) {
    let k = ds::tokens();
    let p = ui.painter();
    let hot = ds::hover_t(ui.ctx(), ui.id().with(("fmh-row", r.min.y as i32)), hover(ui, r));
    if c.selected {
        ds::hgrad(p, r, ds::RADIUS_PLATE, &[(0.0, Color32::from_white_alpha(46)), (1.0, Color32::from_white_alpha(5))]);
        ds::rim(p, r, ds::RADIUS_PLATE, k.ink_silver);
    } else {
        p.rect_filled(r, ds::RADIUS_PLATE, k.glass_deep);
        if hot > 0.0 {
            ds::hgrad(p, r, ds::RADIUS_PLATE, &[(0.0, Color32::from_white_alpha((20.0 * hot) as u8)), (1.0, Color32::TRANSPARENT)]);
        }
        ds::rim(p, r, ds::RADIUS_PLATE, k.rim.lerp_to_gamma(k.rim_bright, hot));
    }
    // `grid-template-columns: 220px 1fr 56px`, gap 16, padding 10 × 14.
    let inner = r.shrink2(vec2(14.0, 10.0));
    let well = Rect::from_min_size(inner.min, vec2(216.0, inner.height()));
    st.wells.push((src, well, ui.opacity()));
    let name = Rect::from_min_max(pos2(inner.left() + 220.0 + 16.0, inner.top()), pos2(inner.right() - 56.0 - 16.0, inner.bottom()));
    let base = ds::fit::baseline(inner, ROW_NAME);
    Text::new(&c.name, ROW_NAME).color(k.ink).shrink(14.0).baseline(base).paint(ui, name);
    let count = Rect::from_min_max(pos2(inner.right() - 56.0, inner.top()), inner.max);
    Text::new(&c.count.to_string(), ROW_COUNT).right().color(if c.warn { k.enemy } else { k.info }).shrink(16.0).baseline(base).paint(ui, count);
}

fn tanks(ui: &mut egui::Ui, t: &TankInfo, st: &mut State, screen: Rect, top: f32, slide: Vec2) {
    let k = ds::tokens();
    // The title plate 24 under the top bar, the panels 42 under it, the exit plate 28 under them, all over the
    // selection card (about 160 high, 24 from the bottom).
    let plate_top = top - 8.0;
    // The board dims the starmap under the panels (`brightness(0.45)`); the HUD's own areas lie over it.
    ui.painter().rect_filled(screen, 0, Color32::from_black_alpha((140.0 * ui.opacity()) as u8));
    let tw = Text::new(&t.title, Font::TITLE_PLATE.upper(false)).layout(ui.ctx(), f32::INFINITY).width;
    let pw = (tw + 2.0 * (ds::SPACE[4] + ds::SPACE[3])).clamp(320.0, (screen.width() - 2.0 * EDGE).min(900.0));
    let pr = Rect::from_min_size(pos2(screen.center().x - pw / 2.0, plate_top), vec2(pw, 60.0)).translate(slide);
    ds::title_plate(&mut ui.new_child(egui::UiBuilder::new().max_rect(pr)), &t.title, pw);
    let y0 = pr.bottom() + 42.0;
    let h = (screen.bottom() - y0 - 230.0).clamp(300.0, 680.0);
    let total = TANKS_LIST_W + TANKS_GAP + TANKS_DATA_W;
    let s = ((screen.width() - 2.0 * EDGE) / total).min(1.0);
    let x0 = screen.center().x - total * s / 2.0;
    let list = Rect::from_min_size(pos2(x0, y0), vec2((TANKS_LIST_W * s).round(), h));
    let data = Rect::from_min_size(pos2(list.right() + (TANKS_GAP * s).round(), y0), vec2((TANKS_DATA_W * s).round(), h));
    st.panels.extend([list, data]);
    let p = &ui.painter().clone();
    // The list: the heading, then the original's eight places as rows 10 apart, as high as the board's (110) while
    // they fit.
    ds::paint_panel(p, list, ds::PanelKind::Glass, 0.97);
    let inner = list.shrink(18.0);
    Text::new(&t.heading, Font::LABEL).color(k.ink_muted).paint(ui, Rect::from_min_size(inner.min, vec2(inner.width(), 18.0)));
    let rows = Rect::from_min_max(inner.min + vec2(0.0, 30.0), inner.max - vec2(if t.total > 8 { 12.0 } else { 0.0 }, 0.0));
    let n = t.cards.iter().filter(|c| c.is_some()).count().max(1) as f32;
    let row_h = 110.0f32.min(((rows.height() - (n - 1.0) * 10.0) / n).floor());
    for (i, c) in t.cards.iter().enumerate() {
        let Some(c) = c else { continue };
        let r = Rect::from_min_size(rows.min + vec2(0.0, i as f32 * (row_h + 10.0)), vec2(rows.width(), row_h));
        // The cell's middle (the turning model without the room between the cells) fills the wide well.
        // (the model stands in the cell's lower part: y − 8 … y + 25).
        let (centre, [x, y, ..]) = cell(i, false);
        tank_row(ui, r, c, [x + 8.0, y + 14.0, 80.0, 40.0], st);
        st.spots.push((r, centre));
    }
    scroll(ui, rows, t.first, t.total, st);
    // The selected design: its model in a 330-point view, the name and hull, the data table, the pods.
    ds::paint_panel(p, data, ds::PanelKind::Glass, 0.97);
    let pic = Rect::from_min_size(data.min, vec2(data.width(), 330.0f32.min(h * 0.49).floor()));
    p.rect_filled(pic, egui::CornerRadius { nw: ds::RADIUS_PANEL, ne: ds::RADIUS_PANEL, sw: 0, se: 0 }, k.glass_deep);
    ds::radial(p, pic, 0, [0.5, 0.6], &[(0.0, Color32::from_rgba_unmultiplied(43, 93, 130, 89)), (0.7, Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    p.hline(pic.x_range(), pic.bottom() - 0.5, Stroke::new(1.0f32, k.rim));
    if let Some(d) = &t.detail {
        let wf = PREVIEW[2] / PREVIEW[3];
        let ph = (pic.height() - 56.0).min((pic.width() - 120.0) / wf);
        st.wells.push((PREVIEW, Rect::from_center_size(pic.center(), vec2(ph * wf, ph)), ui.opacity()));
        let inner = Rect::from_min_max(pic.left_bottom() + vec2(28.0, 22.0), data.max - vec2(28.0, 22.0));
        let head = Rect::from_min_size(inner.min, vec2(inner.width(), 34.0));
        let base = ds::fit::baseline(head, TANK_NAME);
        let kw = Text::new(&d.kind, TANK_KIND).right().color(k.ink_silver).baseline(base).paint(ui, head.with_min_x(head.center().x + 8.0)).width;
        Text::new(&d.name, TANK_NAME).color(k.ink).shrink(20.0).baseline(base).paint(ui, head.with_max_x(head.right() - kw - 16.0));
        let cols = Rect::from_min_max(pos2(inner.left(), head.bottom() + 18.0), inner.max);
        let half = ((cols.width() - 28.0) / 2.0).floor();
        for (n, (label, value)) in d.rows.iter().enumerate() {
            let row = Rect::from_min_size(cols.min + vec2(0.0, n as f32 * 30.0), vec2(half, 20.0));
            let base = ds::fit::baseline(row, TANK_DATA);
            let (a, b) = row.split_left_right_at_x(row.left() + 156.0_f32.min(half * 0.6));
            Text::new(label.trim().trim_end_matches(':'), Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, a.with_max_x(a.right() - 8.0));
            Text::new(value, TANK_DATA).color(k.info).baseline(base).paint(ui, b);
        }
        let x = cols.left() + half + 28.0;
        Text::new(&d.pods_label, Font::LABEL).color(k.ink_muted).paint(ui, Rect::from_min_size(pos2(x, cols.top()), vec2(half, 18.0)));
        for (n, pod) in d.pods.iter().enumerate() {
            let row = Rect::from_min_size(pos2(x, cols.top() + 26.0 + n as f32 * 42.0), vec2(half, 42.0));
            p.hline(row.x_range(), row.top() + 0.5, Stroke::new(1.0f32, k.rim));
            p.hline(row.x_range(), row.bottom() - 0.5, Stroke::new(1.0f32, k.rim));
            let base = ds::fit::baseline(row, TANK_POD);
            match pod {
                None => _ = Text::new(&d.empty, TANK_POD).color(k.ink_muted).baseline(base).paint(ui, row),
                Some((name, count, full)) => {
                    let (a, b) = row.split_left_right_at_x(row.right() - 48.0);
                    Text::new(name, TANK_POD).color(k.lcd_ink).shrink(13.0).baseline(base).paint(ui, a.with_max_x(a.right() - 8.0));
                    Text::new(count, TANK_POD).right().color(if *full { k.info } else { k.info.gamma_multiply(0.8) }).baseline(base).paint(ui, b);
                }
            }
        }
    }
    // The exit plate (hot-spot 9) 28 under the data panel at its right, clear of the radar.
    let radar = ui.ctx().memory(|m| m.area_rect(egui::Id::new("smh-mini"))).map_or(f32::INFINITY, |r| r.left() - 16.0);
    let exit = Rect::from_min_size(pos2(data.right().min(radar) - 260.0 * s, data.bottom() + 28.0), vec2(260.0 * s, 44.0));
    plate(ui, exit, &t.exit);
    st.spots.push((exit, [527.0, 410.0]));
    if let Some(sl) = &t.sale {
        sale(ui, sl, st, screen);
    } else {
        st.sale_at = None;
    }
}

/// The models of the original's render target (`PanelModels::render` over the virtual 640 × 480 screen) in the wells
/// [`show`] left, each fitted into its well.
pub fn pictures(f: &mut crate::render::Frame, wells: &[([f32; 4], Rect, f32)], models: Option<crate::render::TexId>, v: &crate::font::Virtual) {
    let Some(tex) = models else { return };
    let wide = crate::kit::Wide::new(v);
    for &(src, r, alpha) in wells {
        let [x0, y0, w0, h0] = src;
        let s = (r.width() / (w0 * v.k)).min(r.height() / (h0 * v.k));
        let half = [w0 * v.k * s / 2.0, h0 * v.k * s / 2.0];
        let mut i = crate::render::Instance::new(crate::render::kind::SPRITE, [r.center().x, r.center().y], half, [alpha; 4]);
        i.uv = [wide.u(x0), y0 / 480.0, w0 / (640.0 * wide.sx), h0 / 480.0];
        f.push(tex, crate::render::Blend::Alpha, i);
    }
}

fn i16_at(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

impl crate::ui::Hud {
    /// What the modern panels show: the fleet info panel (`Kind::Fleet`) or the transfer panel from the state the
    /// original's texts read (`FlottaInfoText2d` ELF 0x6e74b0, `StarmapGfx2D_3`'s transfer texts); `card` = the HUD's
    /// short info of the selection (the owner and the status lines); the vehicles' panel (`Kind::Tanks`,
    /// `StarmapTanksInfoSzovegKiirasok`, `DrawSpecificInfoOnTank`) as the tank info board. `None` with nothing open.
    pub fn fleet_modern_view(&self, state: &SaveState, rules: &ig2_rules::Rules, panels: &Panels, sel: &Selection, card: Option<&crate::starmap_modern::Info>) -> Option<View> {
        use crate::starmap_info::Kind;
        let lang = self.texts.lang;
        let dec = |b: &[u8]| ig2_formats::text::decode(cstr(b), lang).trim().to_string();
        let col = |i: usize| crate::gui::ink(self.palette.get(i).copied().unwrap_or([1.0; 3]));
        let en = state.mem.i32(va::EN).clamp(0, 18) as u8;
        let design = |e: u8, d: usize| state.designs.get(e as usize).and_then(|x| x.get(d)).map(|r| &r[..]);
        let fleet = |f: i32| usize::try_from(f).ok().and_then(|f| state.fleets.get(f)).map(|r| &r[..]);
        let suffixed = |rec: &[u8], n: u16| {
            let mut s = dec(&rec[..0x14]);
            if i32_at(rec, 0x1c) > 0 {
                s = format!("{s} ({})", dec(&self.texts.get(2, 4, n)));
            }
            s
        };
        let fleet_cap = |r: &[u8]| {
            let (have, cap) = (i32_at(r, 0x5c), i32_at(r, 0x78));
            let label = dec(&self.texts.get(2, 5, 2));
            if cap == 0 {
                Cap { label, value: dec(&self.texts.get(2, 5, 3)), fraction: None, over: false }
            } else {
                Cap { label, value: format!("{have}/{cap}"), fraction: Some(have as f32 / cap as f32), over: have > cap }
            }
        };
        let owner_of = |o: u8| (self.empire_names.get(o as usize).map(|n| dec(n)).unwrap_or_default(), col(if o == 0 { 12 } else { o as usize + 15 }));
        if let Some(t) = &panels.transfer {
            let rows = crate::starmap_panels::rows(state, t);
            let full = |h: i32| h > 0 && fleet(h).is_some_and(|r| i32_at(r, 0x78) <= i32_at(r, 0x5c));
            let side = |h: i32, right: bool| {
                let (title, owner, cap) = if h < 1 {
                    let r = state.mem.read(va::PLANETS + ((-h).max(0) as usize * ig2_sim::save::PLANET_SIZE) as u32, ig2_sim::save::PLANET_SIZE);
                    let cap = Cap { label: dec(&self.texts.get(2, 5, 4)), value: i32_at(&r, 0x58).to_string(), fraction: None, over: false };
                    (dec(&r[..0x18]), r[0x20], cap)
                } else {
                    let r = fleet(h).unwrap_or(&[0u8; 0x80][..]);
                    (dec(&r[..0x14]), r[0x15], fleet_cap(r))
                };
                let (owner, colour) = owner_of(owner);
                let cards = std::array::from_fn(|k| {
                    let row = rows.get(t.first as usize + k)?;
                    let rec = design(en, row.design)?;
                    let count = if right { row.right } else { row.left };
                    Some(Card { name: suffixed(rec, 1), count, selected: false, warn: row.group == 3 && full(h) && count > 0 })
                });
                Side { title, owner, colour, cards, cap }
            };
            return Some(View::Transfer(Transfer { sides: [side(t.f1, false), side(t.f2, true)], first: t.first, total: rows.len() as i32, exit: dec(&self.texts.get(2, 4, 4)) }));
        }
        let i = panels.info.as_ref()?;
        let tanks = i.kind == Kind::Tanks;
        let rows = i.rows(state, sel);
        let owner = i.owner(state, sel);
        let fr = fleet(sel.fleet as i32);
        let cards = std::array::from_fn(|k| {
            let item = i.first as usize + k;
            let row = rows.get(item)?;
            let rec = design(owner, row.design)?;
            // The vehicles' rows carry no upgrade suffix (`StarmapTanksInfoSzovegKiirasok`).
            let name = if tanks { dec(&rec[..0x14]) } else { suffixed(rec, 1) };
            Some(Card { name, count: row.count, selected: item as i32 == i.selected, warn: false })
        });
        // The vehicles' data only while the selected one is on the page (`DrawSpecificInfoOnTank`).
        let shown = if tanks { (0..8).contains(&(i.selected - i.first)) } else { !rows.is_empty() };
        let detail = shown.then(|| rows.get(i.selected.max(0) as usize)).flatten().and_then(|r| design(owner, r.design)).map(|rec| {
            let race = i16_at(rec, 0x40);
            let hull = i32_at(rec, 0x44).max(0) as usize;
            let table = rules.race_pod_table.get(race.max(0) as usize).copied().unwrap_or(0) - 1;
            let slots = rules.pod_slots.get(table.max(0) as usize).and_then(|t| t.get(hull));
            let n = slots.map_or(0, |s| s.iter().filter(|p| p.pod_type > 0).count());
            let pods = (1..=n)
                .map(|p| {
                    let tech = rec[0x50 + p] as usize;
                    (tech != 0).then(|| {
                        let max = slots.and_then(|s| s.get(p - 1)).map_or(0, |s| s.max_count).min(state.mem.u8(va::TECHS + (tech * TECH_SIZE) as u32 + 0x24) as i32);
                        let cnt = rec[0x5d + p] as i32;
                        (dec(&crate::hud::tech_name(&self.texts, tech)), if cnt == 0 { "-".into() } else { cnt.to_string() }, cnt >= max && cnt > 0)
                    })
                })
                .collect();
            // `+0x3c ·f32 100 + 0.5`, a quarter for `+0x38` bit 0 (ELF 0x6e8557..0x6e85d2).
            // The vehicles' `(int)+0x3c`, truncated (§20.3).
            let speed = ((f32_at(rec, 0x3c) * 100.0) as f64 + 0.5) as i32;
            let speed = if tanks { f32_at(rec, 0x3c) as i32 } else if rec[0x38] & 1 != 0 { speed / 4 } else { speed };
            let hull = dec(&crate::hud::tech_name(&self.texts, i32_at(rec, 0x48).max(0) as usize));
            let mut lines = if tanks { vec![] } else { vec![(dec(&self.texts.get(2, 5, 6)), hull.clone())] };
            lines.extend([
                (dec(&self.texts.get(2, 5, 7)), i32_at(rec, 0x30).to_string()),
                (dec(&self.texts.get(2, 5, 8)), i32_at(rec, 0x2c).to_string()),
                (dec(&self.texts.get(2, 5, 9)), speed.to_string()),
            ]);
            if i16_at(rec, 0x16) == 1 && rec[0x36] & 1 == 0 {
                lines.push((dec(&self.texts.get(2, 5, 10)), rec[0x28].to_string()));
            }
            let kind = if tanks { hull } else { String::new() };
            Detail { name: suffixed(rec, 5), rows: lines, pods, empty: dec(&self.texts.get(2, 5, 5)), pods_label: dec(&self.texts.ours("Equipment")), kind }
        });
        let sale = i.sale.as_ref().map(|s| Sale { rows: s.rows.iter().map(|(t, ok)| (dec(t), *ok)).collect(), hovered: s.hovered, at: [s.x, s.y] });
        if tanks {
            let name = dec(&state.mem.read(va::PLANETS + (sel.planet * ig2_sim::save::PLANET_SIZE) as u32, 0x18));
            let held: i32 = rows.iter().map(|r| r.count).sum();
            return Some(View::Tanks(TankInfo {
                title: format!("{name} · {}", dec(&self.texts.ours("Tanks"))),
                heading: format!("{} · {held}", dec(&self.texts.ours("Stationed tanks"))),
                cards,
                first: i.first,
                total: rows.len() as i32,
                detail,
                exit: dec(&self.texts.get(2, 4, 4)),
                sale,
            }));
        }
        let (owner_name, colour) = owner_of(owner);
        // The card's status line and its last (the speed) line, as the board's header: "WAITING · SPEED 6".
        let status = card.map(|c| {
            let lines: Vec<&str> = c.lines.iter().skip(1).map(|l| l.0.trim()).filter(|l| !l.is_empty()).collect();
            let mut v: Vec<String> = lines.first().map(|l| l.to_string()).into_iter().collect();
            if lines.len() > 2 {
                v.push(lines[lines.len() - 1].replace(':', ""));
            }
            v.join(" · ")
        }).unwrap_or_default();
        Some(View::Fleet(FleetInfo {
            title: fr.map(|r| dec(&r[..0x14])).unwrap_or_default(),
            owner: owner_name,
            colour,
            status,
            cards,
            first: i.first,
            total: rows.len() as i32,
            detail,
            cap: fr.map(fleet_cap),
            exit: dec(&self.texts.get(2, 4, 4)),
            sale,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fleet_view() -> View {
        let c = |n: &str, count, selected| Some(Card { name: n.into(), count, selected, warn: false });
        View::Fleet(FleetInfo {
            title: "Fleet 1".into(),
            owner: "Godan Kingdom".into(),
            colour: Color32::GREEN,
            status: "Waiting".into(),
            cards: [c("Standard destroyer", 2, true), c("Colony ship", 1, false), c("A very long design name indeed", 12, false), None, None, None, None, None],
            first: 0,
            total: 3,
            detail: Some(Detail { name: "Standard destroyer".into(), rows: vec![("Type:".into(), "Destroyer".into()), ("Armour:".into(), "10".into())], pods: vec![None, Some(("Radar".into(), "1".into(), true))], empty: "empty".into(), pods_label: String::new(), kind: String::new() }),
            cap: Some(Cap { label: "Tank capacity:".into(), value: "2/2".into(), fraction: Some(1.0), over: false }),
            exit: "Exit".into(),
            sale: None,
        })
    }

    fn run(v: &View, st: &mut State, at: Option<Pos2>) {
        let ctx = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb).ctx;
        for _ in 0..3 {
            let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))), ..Default::default() };
            if let Some(p) = at {
                input.events.push(egui::Event::PointerMoved(p));
            }
            let _ = ctx.run(input, |ctx| show(ctx, v, st));
        }
    }

    /// The pointer over a card reaches the original's cell, over the exit plate hot-spot 9, over the rest of the
    /// panel the panel's empty part; off the panels it is unchanged.
    #[test]
    fn the_pointer_reaches_the_original_spots() {
        let mut st = State::default();
        run(&fleet_view(), &mut st, None);
        let virt = |p: Pos2| [(p.x - st.map[0]) / st.map[2], (p.y - st.map[1]) / st.map[2]];
        let (r0, c0) = st.spots[0];
        assert_eq!(st.pointer(virt(r0.center())), c0);
        assert_eq!(c0, cell(0, false).0);
        let (r2, c2) = st.spots[2];
        assert_eq!(st.pointer(virt(r2.center())), c2);
        let exit = st.spots.last().unwrap();
        assert_eq!(st.pointer(virt(exit.0.center())), [527.0, 410.0]);
        assert!(crate::kit::inside([445, 380, 164, 60], [527.0, 410.0]));
        let panel = st.panels[0];
        assert_eq!(st.pointer(virt(panel.right_top() + vec2(-40.0, 30.0))), NEUTRAL);
        // Under the selection card: the original's; the original's exit button's old place: no more.
        assert_eq!(st.pointer([60.0, 460.0]), [60.0, 460.0]);
        assert_eq!(st.pointer([500.0, 400.0]), NEUTRAL);
        // Every card centre is the original's cell (hot-spots 0x43 + 2k).
        for k in 0..8 {
            let c = cell(k, false).0;
            let want = [0x43, 0x45, 0x47, 0x49, 0x4b, 0x4d, 0x4f, 0x51][k];
            let spot = [[28, 60, 124, 70], [152, 60, 139, 70]][k % 2];
            assert!(crate::kit::inside([spot[0], spot[1] + 70 * (k as i32 / 2), spot[2], spot[3]], c), "{want:#x}");
        }
    }

    fn tank_view(name: &str) -> View {
        let c = |n: &str, count, selected| Some(Card { name: n.into(), count, selected, warn: false });
        View::Tanks(TankInfo {
            title: "Magellan 10 · Tanks".into(),
            heading: "Stationed tanks · 12".into(),
            cards: [c("Standard tank", 3, true), c(name, 4, false), c("Hover tank", 5, false), None, None, None, None, None],
            first: 0,
            total: 3,
            detail: Some(Detail { name: "Standard tank".into(), rows: vec![("Armour:".into(), "10".into())], pods: vec![Some(("Machine gun".into(), "1".into(), true))], empty: "empty".into(), pods_label: "Equipment".into(), kind: "Wheeled tank".into() }),
            exit: "Exit".into(),
            sale: None,
        })
    }

    /// The vehicles' rows reach the original's cells (hot-spots 0x43 + 2k), the exit plate hot-spot 9; each row's
    /// well and the preview take the models; a long name leaves the rows as they were.
    #[test]
    fn the_tank_rows_reach_the_original_cells() {
        let mut st = State::default();
        run(&tank_view("Amphibious tank"), &mut st, None);
        let virt = |p: Pos2| [(p.x - st.map[0]) / st.map[2], (p.y - st.map[1]) / st.map[2]];
        for k in 0..3 {
            assert_eq!(st.pointer(virt(st.spots[k].0.center())), cell(k, false).0);
        }
        assert_eq!(st.pointer(virt(st.spots[3].0.center())), [527.0, 410.0]);
        assert_eq!(st.wells.len(), 4);
        let rows: Vec<Rect> = st.spots[..3].iter().map(|s| s.0).collect();
        let mut long = State::default();
        run(&tank_view("An amphibious tank with a very long design name indeed"), &mut long, None);
        assert_eq!(rows, long.spots[..3].iter().map(|s| s.0).collect::<Vec<_>>());
        // The two panels at the board's 1920 × 1080 places.
        assert_eq!((st.panels[0].left(), st.panels[0].width(), st.panels[1].left(), st.panels[1].width()), (280.0, 560.0, 880.0, 760.0));
    }

    /// Long names never widen a card: the wells keep their size whatever the text.
    #[test]
    fn long_names_keep_the_cards() {
        let mut st = State::default();
        run(&fleet_view(), &mut st, None);
        let (a, b) = (st.spots[0].0, st.spots[2].0);
        assert_eq!(a.size(), b.size());
        assert_eq!(st.wells.len(), 4);
    }
}
