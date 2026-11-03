//! Ours (ADR 0071): the starmap's production view (re/screens/starmap.md §12) as modern panels on the egui
//! layer, under the modern starmap HUD (ADR 0065), drawn with the design kit (ADR 0073, `gui::ds`) after the
//! production board. The same functions as the original's view: the planet with the previous and next of the
//! player's planets and the planet list, per line (ships, vehicles) the capacity, the head unit's bar and the four
//! slots with their design, count, price, repeat and "into a fleet" lamps, the count buttons (held: they repeat as the
//! original's), the slot's design list (design 0 removes) and the exit. Each slot shows its design's hull turning in
//! a picture well ([`State::wells`], drawn by the window from `starmap_panels::PanelModels::render_thumbs`). The
//! window fills a [`View`] from the state `StarmapGfx2D_3` reads, runs [`show`] after the HUD and hands the [`Act`]s
//! to the original's panel code (`starmap_panels::Panels::press_id`, `hold_frame`, `release`, `modern_pick`).

use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Ui};

use crate::gui::ds::{self, Font, Glyph, Icon, IconKey, ListRow, PanelKind, Plate, Text, Weight};

/// What the panels ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// A hot-spot pressed: 0xc/0xd the previous/next planet, 0xe..=0x15 repeat, 0x16..=0x19 "into a fleet", 9 exit,
    /// 0x32 the planet list.
    Spot(u32),
    /// A count button held this frame (line, slot, `true` = plus), or none.
    Hold(Option<(usize, usize, bool)>),
    /// A design picked for a slot (line, slot, design; 0 removes).
    Pick(usize, usize, usize),
}

/// One queue slot.
#[derive(Debug, Clone, Default)]
pub struct Slot {
    pub name: String,
    /// The design's hull (its research entry's name).
    pub kind: String,
    pub count: i32,
    pub price: String,
    pub repeat: bool,
    /// The vehicles' "into a fleet" lamp.
    pub into_fleet: Option<bool>,
    /// A station (`+0x37`): no count buttons.
    pub station: bool,
}

/// One entry of a line's design list.
#[derive(Debug, Clone, Default)]
pub struct Entry {
    pub design: usize,
    pub name: String,
    /// The unit price (`+0x24`), empty for design 0.
    pub price: String,
    /// Not a ninth station.
    pub ok: bool,
}

/// One line: ships (0) or vehicles (1).
#[derive(Debug, Clone, Default)]
pub struct Line {
    pub title: String,
    /// The capacity's label and value (`GetString(2, 4, 3)`, below 1 `GetString(2, 4, 2)`).
    pub capacity: (String, String),
    pub working: bool,
    /// The head unit's bar, 0..1.
    pub progress: Option<f32>,
    pub slots: [Option<Slot>; 4],
    /// Each slot's design (0 none), to mark it in the list.
    pub designs_in: [usize; 4],
    /// The design list: design 0 (remove) and the player's visible designs of the line's class, with a station's
    /// ninth dead.
    pub designs: Vec<Entry>,
}

/// The remake's labels: an empty slot's well, its two lines and its button, the list's title ("Slot {}"), the repeat
/// and "into a fleet" lamps.
#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub empty: String,
    pub free: String,
    pub no_design: String,
    pub design: String,
    pub slot: String,
    pub repeat: String,
    pub into_fleet: String,
}

/// What the view shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub planet: String,
    pub lines: [Line; 2],
    pub exit: String,
    /// The exit is under the tutorial's lock.
    pub exit_locked: bool,
    /// Tooltips (`GetString(2, 1, n)`): previous, next, repeat, into a fleet, minus, plus, design list, planet list,
    /// exit.
    pub tips: [String; 9],
    pub labels: Labels,
}

/// What stays between frames: the slot whose design list is open, the picture wells.
#[derive(Debug, Clone, Default)]
pub struct State {
    list: Option<(usize, usize)>,
    /// This frame's picture wells in pixels: slot `k + 4L`, its rectangle, its opacity.
    pub wells: Vec<(usize, Rect, f32)>,
    /// The pass the view was last shown in and when it opened (its slide-in).
    seen: Option<(u64, f64)>,
    /// The count button pressed and still held (`Mouse_Megfogva` keeps it, also with the pointer off it).
    count: Option<(usize, usize, bool)>,
}

/// The original's buttons act on the press of either button (`Starmap_Main` on the latched hot-spot, §12.4).
fn pressed(r: &egui::Response) -> bool {
    r.hovered() && r.enabled() && r.ctx.input(|i| i.pointer.any_pressed())
}

/// The boards' gap to the screen's edges, a line panel's width, a slot's height and the gap between slots.
const EDGE: f32 = 24.0;
const PANEL_W: f32 = 600.0;
const SLOT_H: f32 = 90.0;
const SLOT_GAP: f32 = 10.0;
/// The planet's title plate and the keys around it (`.ig-title-plate` 386 wide, 12 apart).
const PLATE_W: f32 = 386.0;
const PLATE_ROW_W: f32 = PLATE_W + 2.0 * 12.0 + 38.0 + 74.0;
/// A slot's columns (`30px 128px 1fr auto`, 12 apart, 6 / 10 inside): the number, the picture, the texts.
const NUM_W: f32 = 30.0;
const THUMB: egui::Vec2 = vec2(128.0, 72.0);
/// The design list's width and its rows' height.
const LIST_W: f32 = 340.0;
const ROW_H: f32 = 26.0;

/// The type of the board's slots: the name (500 17 / 0.06 em), the hull (600 12 / 0.14 em upper case), the price
/// (500 15 / 0.1 em), the line's title (600 20 / 0.16 em), the lamps (11 / 0.12 em).
const NAME: Font = Font::BODY.weight(Weight::Medium).tracking(0.06);
const KIND: Font = Font::LABEL.sized(12.0);
const PRICE: Font = Font::DATA.tracking(0.10);
const TITLE: Font = Font::HEADING.tracking(0.16).upper(true);
const LAMP: Font = Font::LABEL.sized(11.0).tracking(0.12);

/// One frame of the production view (after the HUD's, under its bar); the acts in the order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State, _race: Color32) -> Vec<Act> {
    let top = ctx.memory(|m| m.area_rect(egui::Id::new("smh-left"))).map_or(44.0, |r| r.bottom()) + 20.0;
    let mut acts = Vec::new();
    st.wells.clear();
    // The panels slide in from their sides as the original's (`(1 − sin(f/12·π/2))·200`), over the open time.
    let pass = ctx.cumulative_pass_nr();
    let now = ctx.input(|i| i.time);
    let since = match st.seen {
        Some((p, at)) if p + 1 >= pass => at,
        _ => now,
    };
    st.seen = Some((pass, since));
    let dur = ds::anim(ds::OPEN_TIME * 2.0);
    let t = if dur > 0.0 { egui::emath::easing::cubic_out(((now - since) as f32 / dur).clamp(0.0, 1.0)) } else { 1.0 };
    if t < 1.0 {
        ctx.request_repaint();
    }
    let slide = (1.0 - t) * 200.0;
    // Narrower than the board's two panels and the plate between them (4:3, a large UI scale): the plate goes to the
    // bottom, between the starmap HUD's card (380 wide) and the exit.
    let narrow = ctx.screen_rect().width() < 2.0 * (EDGE + PANEL_W + 12.0) + PLATE_ROW_W;
    let (pivot, at) = if narrow { (Align2::CENTER_BOTTOM, vec2((EDGE + 380.0 + 16.0 - EDGE - 320.0 - 16.0) / 2.0, -EDGE + slide * 0.3)) } else { (Align2::CENTER_TOP, vec2(0.0, top - slide * 0.3)) };
    egui::Area::new(egui::Id::new("pmh-planet")).anchor(pivot, at).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(t);
        planet_plate(ui, v, &mut acts);
    });
    let panels_top = top;
    for (l, line) in v.lines.iter().enumerate() {
        let (align, x) = if l == 0 { (Align2::LEFT_TOP, EDGE - slide) } else { (Align2::RIGHT_TOP, -EDGE + slide) };
        egui::Area::new(egui::Id::new(("pmh-line", l))).anchor(align, vec2(x, panels_top)).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(t);
            ds::panel(ui, PanelKind::Glass, 0.92, 16.0, |ui| line_panel(ui, l, line, v, st, &mut acts));
        });
    }
    egui::Area::new(egui::Id::new("pmh-exit")).anchor(Align2::RIGHT_BOTTOM, vec2(-EDGE, -EDGE + slide * 0.3)).order(egui::Order::Foreground).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(t);
        let exit = ui.add_enabled(!v.exit_locked, Plate::new(&v.exit).width(320.0).height(56.0));
        if pressed(&ds::tip(exit, &v.tips[8])) {
            acts.push(Act::Spot(9));
        }
    });
    design_list(ctx, v, st, &mut acts);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("pmh-opened"), false));
    if !ctx.input(|i| i.pointer.any_down()) {
        st.count = None;
    }
    let held = st.count;
    let ppp = ctx.pixels_per_point();
    for w in &mut st.wells {
        w.1 = Rect::from_min_max((w.1.min.to_vec2() * ppp).to_pos2(), (w.1.max.to_vec2() * ppp).to_pos2());
    }
    acts.push(Act::Hold(held));
    acts
}

/// The planet's name on the title plate between the previous key and the next and planet-list keys.
fn planet_plate(ui: &mut Ui, v: &View, acts: &mut Vec<Act>) {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let prev = ds::key_group(ui, |ui| ui.add(IconKey::icon(Icon::ChevronLeft)));
        if pressed(&ds::tip(prev, &v.tips[0])) {
            acts.push(Act::Spot(0xc));
        }
        let plate = ds::title_plate(ui, &v.planet, PLATE_W);
        let plate = ui.interact(plate.rect, ui.id().with("pmh-name"), Sense::click());
        // The planet list opens on the press and picks on the release, as the original's (hot-spot 0x32).
        let list = |r: &egui::Response| r.is_pointer_button_down_on() && r.ctx.input(|i| i.pointer.primary_pressed());
        if list(&ds::tip(plate, &v.tips[7])) {
            acts.push(Act::Spot(0x32));
        }
        ds::key_group(ui, |ui| {
            if pressed(&ds::tip(ui.add(IconKey::icon(Icon::ChevronRight)), &v.tips[1])) {
                acts.push(Act::Spot(0xd));
            }
            if list(&ds::tip(ui.add(IconKey::icon(Icon::ChevronDown)), &v.tips[7])) {
                acts.push(Act::Spot(0x32));
            }
        });
    });
}

/// A line's title and capacity, then its four slots.
#[allow(clippy::too_many_arguments)]
fn line_panel(ui: &mut Ui, l: usize, line: &Line, v: &View, st: &mut State, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let w = PANEL_W - 32.0;
    ui.set_width(w);
    ui.spacing_mut().item_spacing = vec2(0.0, SLOT_GAP);
    ui.allocate_ui_with_layout(vec2(w, 30.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let value = ds::lcd(ui, &line.capacity.1, 22.0, 5);
        if !line.working {
            ui.painter().rect_filled(value.rect, 2, Color32::from_black_alpha(110));
        }
        let rest = ui.available_width();
        let (r, _) = ui.allocate_exact_size(vec2(rest, 30.0), Sense::hover());
        let base = ds::fit::baseline(r, TITLE);
        let label = Text::new(&line.capacity.0, Font::LABEL).right().color(k.ink_muted).baseline(base).layout(ui.ctx(), rest * 0.5).width;
        Text::new(&line.capacity.0, Font::LABEL).right().color(k.ink_muted).baseline(base).paint(ui, r.with_min_x(r.right() - label));
        Text::new(&line.title, TITLE).baseline(base).paint(ui, r.with_max_x(r.right() - label - 12.0));
    });
    for (s, slot) in line.slots.iter().enumerate() {
        slot_row(ui, l, s, slot.as_ref(), line, v, st, acts);
    }
}

/// One slot (`.slot`): its number, the picture well, the design's name, hull and price (the head unit's bar), the
/// count with its buttons and the lamps; an empty one its "Design" plate. The name and the picture open the list.
#[allow(clippy::too_many_arguments)]
fn slot_row(ui: &mut Ui, l: usize, s: usize, slot: Option<&Slot>, line: &Line, v: &View, st: &mut State, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), SLOT_H), Sense::click());
    let open = st.list == Some((l, s));
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
    let p = ui.painter();
    if slot.is_some() {
        p.rect_filled(rect, ds::RADIUS_PLATE, k.glass_raised);
    }
    let head = s == 0 && slot.is_some();
    ds::rim(p, rect, ds::RADIUS_PLATE, if open { k.ink_silver } else if head { k.rim_bright } else { k.rim.lerp_to_gamma(k.rim_bright, hot) });
    if ds::tip(resp, &v.tips[6]).clicked() {
        toggle_list(ui.ctx(), st, l, s);
    }
    let inner = rect.shrink2(vec2(0.0, 8.0));
    let num = Rect::from_min_size(pos2(rect.left() + 6.0, inner.top()), vec2(NUM_W, inner.height()));
    let n = (s + 1).to_string();
    Text::new(&n, Font::DATA.weight(Weight::SemiBold).tracking(0.0)).center().color(if slot.is_some() { k.ink_silver } else { k.ink_muted }).paint(ui, num);
    // The picture: the hull turning in a black well, or "EMPTY".
    let well = Rect::from_center_size(pos2(num.right() + 12.0 + THUMB.x / 2.0, rect.center().y), THUMB);
    let p = ui.painter();
    p.rect_filled(well, ds::RADIUS_PLATE, Color32::BLACK);
    ds::vgrad(p, well, ds::RADIUS_PLATE, &[(0.0, Color32::BLACK), (6.0 / THUMB.y, Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    ds::rim(p, well, ds::RADIUS_PLATE, k.metal_dark);
    match slot {
        Some(_) => st.wells.push((s + 4 * l, well, ui.opacity())),
        None => _ = Text::new(&v.labels.empty, Font::DATA.sized(13.0).tracking(0.12).upper(true)).center().color(k.ink_muted).paint(ui, well),
    }
    let text_l = well.right() + 12.0;
    let Some(slot) = slot else {
        // An empty slot: "Free slot" and the button that opens the list.
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(rect.right() - 10.0 - 110.0, rect.center().y - 17.0), pos2(rect.right() - 10.0, rect.center().y + 17.0))));
        let label = format!("+ {}", v.labels.design);
        let b = c.add(Plate::new(&label).width(110.0).height(34.0).font(Plate::FONT.sized(14.0).tracking(0.10)));
        if ds::tip(b, &v.tips[6]).clicked() {
            toggle_list(ui.ctx(), st, l, s);
        }
        let texts = Rect::from_min_max(pos2(text_l, inner.top()), pos2(rect.right() - 10.0 - 110.0 - 12.0, inner.bottom()));
        let (a, b) = texts.split_top_bottom_at_y(texts.center().y + 4.0);
        Text::new(&v.labels.free, NAME).color(k.ink_muted).baseline(a.bottom() - 4.0).paint(ui, a);
        Text::new(&v.labels.no_design, KIND).color(k.ink_muted).baseline(b.top() + 14.0).paint(ui, b);
        return;
    };
    // The controls at the right: −, the count on an LCD, + (held: the original's repeat; none for stations), the lamps.
    let ctl_w = 170.0;
    let ctl = Rect::from_min_max(pos2(rect.right() - 10.0 - ctl_w, inner.top()), pos2(rect.right() - 10.0, inner.bottom()));
    let mut row = |top: f32, h: f32| ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(ctl.left(), top), pos2(ctl.right(), top + h))).layout(egui::Layout::right_to_left(egui::Align::Center)));
    let mut step = row(ctl.top() - 2.0, 42.0);
    step.spacing_mut().item_spacing.x = 4.0;
    let plus = step.add_enabled_ui(!slot.station, |ui| ds::tip(ds::round_button(ui, Glyph::Icon(Icon::Plus), true, false), &v.tips[5])).inner;
    ds::lcd(&mut step, &slot.count.to_string(), 22.0, 3);
    let minus = step.add_enabled_ui(!slot.station, |ui| ds::tip(ds::round_button(ui, Glyph::Icon(Icon::Minus), true, false), &v.tips[4])).inner;
    if pressed(&minus) {
        st.count = Some((l, s, false));
    } else if pressed(&plus) {
        st.count = Some((l, s, true));
    }
    let mut lamps = row(ctl.bottom() - 24.0, 24.0);
    lamps.spacing_mut().item_spacing.x = 6.0;
    if let Some(f) = slot.into_fleet {
        if pressed(&ds::tip(lamp(&mut lamps, &v.labels.into_fleet, f), &v.tips[3])) {
            acts.push(Act::Spot(0x16 + s as u32));
        }
    }
    if pressed(&ds::tip(lamp(&mut lamps, &v.labels.repeat, slot.repeat), &v.tips[2])) {
        acts.push(Act::Spot(0xe + 4 * l as u32 + s as u32));
    }
    // The texts between, on the board's three baselines; the head unit's bar under them.
    let texts = Rect::from_min_max(pos2(text_l, inner.top()), pos2(ctl.left() - 8.0, inner.bottom()));
    let y = texts.top() + if head && line.progress.is_some() { 2.0 } else { 8.0 };
    let name_c = k.ink.lerp_to_gamma(Color32::WHITE, hot);
    Text::new(&slot.name, NAME).color(name_c).baseline(y + 16.0).paint(ui, Rect::from_min_size(pos2(texts.left(), y), vec2(texts.width(), 22.0)));
    Text::new(&slot.kind, KIND).color(k.ink_muted).baseline(y + 34.0).paint(ui, Rect::from_min_size(pos2(texts.left(), y + 22.0), vec2(texts.width(), 16.0)));
    Text::new(&slot.price, PRICE).color(k.info).baseline(y + 52.0).paint(ui, Rect::from_min_size(pos2(texts.left(), y + 38.0), vec2(texts.width(), 20.0)));
    if let (true, Some(f)) = (head, line.progress) {
        let pct = format!("{}%", (f.clamp(0.0, 1.0) * 100.0).round() as i32);
        let row = Rect::from_min_size(pos2(texts.left(), y + 62.0), vec2(texts.width(), 14.0));
        let mut b = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
        b.spacing_mut().item_spacing.x = 8.0;
        let fill = if line.working { k.lcd_ink } else { k.ink_muted };
        ds::bar(&mut b, (row.width() - 44.0).max(20.0), 6.0, f, fill);
        Text::new(&pct, Font::READOUT.sized(14.0).tracking(0.04)).right().color(fill).show(&mut b, vec2(36.0, 14.0));
    }
}

/// Open line `l`'s list for slot `s`, or close it when it is open there.
fn toggle_list(ctx: &egui::Context, st: &mut State, l: usize, s: usize) {
    st.list = if st.list == Some((l, s)) { None } else { Some((l, s)) };
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("pmh-opened"), true));
}

/// A lamp plate (`.lamp`: 26 high, 11 / 0.12 em), lit (`plate-high`) when on.
fn lamp(ui: &mut Ui, label: &str, on: bool) -> egui::Response {
    let k = ds::tokens();
    let w = Text::new(label, LAMP).layout(ui.ctx(), 120.0).width + 16.0;
    let (r, resp) = ui.allocate_exact_size(vec2(w.max(56.0), 24.0), Sense::click());
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
    let lit = ui.ctx().animate_bool_with_time(resp.id.with("lit"), on, ds::anim(ds::HOVER_TIME));
    let p = ui.painter();
    ds::plate_fill(p, r, ds::RADIUS_PLATE, resp.is_pointer_button_down_on());
    if lit > 0.0 {
        p.rect_filled(r, ds::RADIUS_PLATE, k.plate_high.gamma_multiply(lit));
        ds::shadow(p, r, ds::RADIUS_PLATE, 0, 6, 0, k.info.gamma_multiply(0.25 * lit));
    }
    if hot > 0.0 {
        ds::rim(p, r, ds::RADIUS_PLATE, k.rim_bright.gamma_multiply(hot));
    }
    Text::new(label, LAMP).center().color(k.ink_silver.lerp_to_gamma(k.ink, lit.max(hot))).paint(ui, r.shrink2(vec2(4.0, 0.0)));
    resp
}

/// The open design list under its slot: the title ("Slot n"), design 0 and the line's designs with their price, the
/// slot's own design selected, a ninth station muted; a pick sends it, a press elsewhere closes it.
fn design_list(ctx: &egui::Context, v: &View, st: &mut State, acts: &mut Vec<Act>) {
    let Some((l, s)) = st.list else { return };
    let line = &v.lines[l];
    let anchor = ctx.memory(|m| m.area_rect(egui::Id::new(("pmh-line", l))));
    let mut close = line.slots[s].is_none() && line.designs.is_empty();
    if let Some(a) = anchor {
        // The slot's rectangle inside its panel: 16 + 30 + 10 down, then the slots.
        let slot_bottom = a.top() + 16.0 + 30.0 + SLOT_GAP + (s as f32 + 1.0) * (SLOT_H + SLOT_GAP) - SLOT_GAP;
        let (pivot, at) = if l == 0 { (Align2::LEFT_TOP, pos2(a.right() - 184.0, slot_bottom + 8.0)) } else { (Align2::RIGHT_TOP, pos2(a.left() + 184.0, slot_bottom + 8.0)) };
        let t = ds::open_t(ctx, egui::Id::new("pmh-list"), true);
        let r = egui::Area::new(egui::Id::new("pmh-list")).pivot(pivot).fixed_pos(at + vec2(0.0, (1.0 - t) * -8.0)).order(egui::Order::Foreground).constrain(true).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(t);
            ds::panel(ui, PanelKind::Deep, 0.96, 8.0, |ui| {
                ui.set_width(LIST_W - 16.0);
                ui.spacing_mut().item_spacing.y = 0.0;
                let title = v.labels.slot.replace("{}", &(s + 1).to_string());
                Text::new(&title, Font::LABEL.sized(12.0)).color(ds::tokens().ink_muted).show(ui, vec2(LIST_W - 16.0, 24.0));
                let room = (ctx.screen_rect().bottom() - at.y - 60.0).max(ROW_H * 4.0);
                egui::ScrollArea::vertical().max_height(room.min(ROW_H * 14.0)).show(ui, |ui| {
                    for e in &line.designs {
                        let row = ListRow::new(&e.name).value(&e.price).font(Font::BODY.sized(16.0)).selected(e.design != 0 && e.design == line.designs_in[s]).off(!e.ok);
                        let resp = ui.add_enabled(e.ok || e.design == 0, row);
                        if resp.clicked() {
                            acts.push(Act::Pick(l, s, e.design));
                            close = true;
                        }
                    }
                });
            });
        });
        // A press anywhere else closes it.
        if ctx.input(|i| i.pointer.any_pressed()) && !r.response.contains_pointer() && !ctx.data(|d| d.get_temp::<bool>(egui::Id::new("pmh-opened")).unwrap_or(false)) {
            close = true;
        }
    }
    if close {
        st.list = None;
        ds::open_t(ctx, egui::Id::new("pmh-list"), false);
    }
}

/// The slot pictures in the wells [`show`] left (`wells` in pixels): each slot's 160 × 90 cell of
/// `starmap_panels::PanelModels::render_thumbs`'s texture, filling its 16:9 well, drawn over the panels.
pub fn pictures(f: &mut crate::render::Frame, wells: &[(usize, Rect, f32)], tex: Option<crate::render::TexId>, v: &crate::font::Virtual) {
    let Some(tex) = tex else { return };
    let wide = crate::kit::Wide::new(v);
    for &(slot, r, alpha) in wells {
        let [cx, cy] = crate::starmap_panels::thumb_cell(slot);
        let (w0, h0) = (160.0, 90.0);
        let mut i = crate::render::Instance::new(crate::render::kind::SPRITE, [r.center().x, r.center().y], [r.width() / 2.0, r.height() / 2.0], [alpha; 4]);
        i.uv = [wide.u(cx - w0 / 2.0), (cy - h0 / 2.0) / 480.0, w0 / (640.0 * wide.sx), h0 / 480.0];
        f.push(tex, crate::render::Blend::GammaPremul, i);
    }
}

/// The hot-spot the original presses for a held count button, and the virtual point inside it that picks minus or
/// plus (`Mouse_x` < 164 / 479, re/screens/starmap.md §12.4).
pub fn count_spot(l: usize, k: usize, plus: bool) -> (u32, [f32; 2]) {
    let id = 0x1a + 8 * plus as u32 + 4 * l as u32 + k as u32;
    let x = [[149.0, 177.0], [464.0, 492.0]][l][plus as usize];
    (id, [x, [157.0, 230.0, 284.0, 339.0][k]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_buttons_press_the_originals_halves() {
        // The left half lowers (x < 164 / 479), the right half raises; ids 0x1a.. minus, 0x22.. plus.
        assert_eq!(count_spot(0, 0, false), (0x1a, [149.0, 157.0]));
        assert_eq!(count_spot(0, 3, true), (0x25, [177.0, 339.0]));
        assert_eq!(count_spot(1, 2, false), (0x20, [464.0, 284.0]));
        assert_eq!(count_spot(1, 1, true), (0x27, [492.0, 230.0]));
        for l in 0..2 {
            for k in 0..4 {
                let (_, [x, _]) = count_spot(l, k, false);
                assert!(x < [164.0, 479.0][l]);
                let (_, [x, _]) = count_spot(l, k, true);
                assert!(x >= [164.0, 479.0][l]);
            }
        }
    }

    fn view(name: &str) -> View {
        let slot = Slot { name: name.into(), kind: "Destroyer".into(), count: 5, price: "$500".into(), repeat: true, into_fleet: None, station: false };
        let mut v = View { planet: "Magellan 10".into(), exit: "Exit".into(), ..Default::default() };
        let designs = vec![Entry { design: 0, name: "Remove".into(), ok: true, ..Default::default() }, Entry { design: 3, name: name.into(), price: "$100".into(), ok: true }];
        v.lines[0] = Line { title: "Ships".into(), capacity: ("Capacity".into(), "2".into()), working: true, progress: Some(0.5), slots: [Some(slot.clone()), None, None, None], designs_in: [3, 0, 0, 0], designs };
        v.lines[1].slots[0] = Some(Slot { into_fleet: Some(false), ..slot });
        v
    }

    fn run(v: &View, st: &mut State, width: f32) -> (egui::Context, Vec<Act>) {
        let ctx = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb).ctx;
        ds::set_motion(false);
        let mut last = Vec::new();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 1080.0))), ..Default::default() }, |ctx| last = show(ctx, v, st, Color32::RED));
        }
        (ctx, last)
    }

    #[test]
    fn the_panels_draw_and_ask_for_nothing_untouched() {
        let mut st = State::default();
        let (ctx, last) = run(&view("Fighter"), &mut st, 1920.0);
        assert_eq!(last, vec![Act::Hold(None)]);
        let r = ctx.memory(|m| m.area_rect(egui::Id::new(("pmh-line", 1)))).unwrap();
        assert!(r.right() <= 1920.0 - EDGE + 0.5 && (r.width() - PANEL_W).abs() < 1.0, "{r:?}");
        // A well per filled slot.
        assert_eq!(st.wells.iter().map(|w| w.0).collect::<Vec<_>>(), vec![0, 4]);
    }

    /// A long design name is cut inside its slot: the panels keep the board's width, at 4:3 too.
    #[test]
    fn long_names_keep_the_panels() {
        for w in [1440.0, 1920.0, 2520.0] {
            let mut st = State::default();
            let (ctx, _) = run(&view(&"Very Long Interstellar Destroyer Mk II ".repeat(4)), &mut st, w);
            for l in 0..2 {
                let r = ctx.memory(|m| m.area_rect(egui::Id::new(("pmh-line", l)))).unwrap();
                assert!((r.width() - PANEL_W).abs() < 1.0, "{w}: {r:?}");
            }
        }
    }
}
