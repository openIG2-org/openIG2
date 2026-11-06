//! Ours (ADR 0076): the modern trade screen on the egui layer (ADR 0062), the "Modern trade screen" option, drawn
//! with the design kit (ADR 0073, `gui::ds`) after the trade board. The chosen offer's hull keeps turning in a well
//! between the two lists (`TradeScene::set_modern`: no frame, no wire sphere); the ships' and vehicles' offers, the
//! hull's card, the order bar, the message line and the deliveries are panels. Their presses go to the original's
//! code (`Trade::modern_pick`, `modern_qty`, `modern_destination`, `modern_buy`: hot-spots 0x19..0x1c, 0x23/0x24 of
//! re/screens/trade.md), so the orders stay the original's `Command::Buy`.

use egui::{pos2, vec2, Rect, Ui};

use crate::gui::ds::{self, Font, PanelKind, Text};
use crate::starmap_modern::{self as sm};

/// What the panels ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    Bar(sm::Act),
    /// Row `row` of list `kind` (0 ships, 1 vehicles) pressed.
    Row(usize, i32),
    /// The quantity's + (`true`) or −.
    Qty(bool),
    /// Port `p` picked as the destination.
    Dest(i32),
    Buy,
    /// The destination's list opened (the original's press on it clicks).
    Click,
}

/// An offer's row: name, unit price, count on offer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    pub name: String,
    pub price: i32,
    pub count: i32,
}

/// The offer being made: its list and row, name, quantity and how many are left, total, destination (−1 none, its
/// name and days), OK live.
#[derive(Debug, Clone, Default)]
pub struct Offer {
    pub kind: usize,
    pub row: i32,
    pub name: String,
    pub qty: i32,
    pub left: i32,
    pub total: i32,
    pub dest: i32,
    pub dest_name: String,
    pub eta: i32,
    pub ok: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub lists: [String; 2],
    pub price: String,
    pub count: String,
    pub none: String,
    pub item: String,
    pub amount: String,
    pub dest: String,
    pub eta: String,
    pub total: String,
    pub buy: String,
    pub under_way: String,
    pub nothing: String,
    pub exit: String,
}

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    pub live: bool,
    pub lists: [Vec<Row>; 2],
    pub offer: Option<Offer>,
    /// The shown hull's card: label and value pairs, the weapons.
    pub card: Vec<(String, String)>,
    pub weapons: String,
    /// The ports a delivery can go to.
    pub ports: Vec<(i32, String)>,
    /// The message line (no port: "build a trade port").
    pub line: String,
    /// The deliveries: planet, days left, item, amount.
    pub deliveries: Vec<(String, i32, String, i32)>,
    /// The hot-spots' tooltips by id (`Trade_MouseInit`).
    pub tips: Vec<String>,
    pub labels: Labels,
}

/// What stays between frames.
#[derive(Default)]
pub struct State {
    pub bar: sm::State,
}

/// The board's measures (1920 × 1080).
const LIST: egui::Vec2 = egui::Vec2::new(560.0, 560.0);
const WELL: egui::Vec2 = egui::Vec2::new(560.0, 310.0);
/// The well's centre on the original's 640 × 480 screen (the board's (960, 247) at 1080 lines).
pub const WELL_AT: [f32; 2] = [320.0, 110.0];
const ROW_H: f32 = 30.0;
/// The panels' insides.
const PAD: egui::Vec2 = egui::Vec2::new(20.0, 18.0);
/// The offers' columns: the name takes the rest, the price, the count.
const OFFER_COLS: [f32; 3] = [0.0, 96.0, 48.0];
const OFFER_RIGHT: [bool; 3] = [false, true, true];
const CARD_PAD: egui::Vec2 = egui::Vec2::new(24.0, 16.0);

pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut acts = sm::board_bar(ctx, &v.bar, &mut st.bar, Act::Bar);
    if !v.live {
        return acts;
    }
    let screen = ctx.screen_rect();
    let (w, h) = (screen.width(), screen.height());
    let g = ds::squeeze(h);
    let top = 72.0 * g;
    let list_w = LIST.x.min(w * 0.29);
    let list_h = LIST.y * g;
    for k in 0..2 {
        let x = if k == 0 { ds::BOARD_EDGE } else { w - ds::BOARD_EDGE - list_w };
        let r = Rect::from_min_size(pos2(x, top), vec2(list_w, list_h));
        ds::place(ctx, format!("tmh-list{k}"), r, |ui| list_panel(ui, v, k, r.size(), &mut acts));
    }
    // The hull's well: the scene draws the model there; a faint grid behind it.
    let ppp = ctx.pixels_per_point();
    let [cx, cy] = crate::font::Virtual::new([w * ppp, h * ppp]).px(WELL_AT[0], WELL_AT[1]);
    let well = Rect::from_center_size(pos2(cx / ppp, cy / ppp), WELL * g);
    ds::place(ctx, "tmh-well", well, |ui| ds::turntable_grid(ui.painter(), well));
    if !v.card.is_empty() {
        let r = Rect::from_min_size(pos2(well.center().x - 300.0, well.bottom() + 28.0 * g), vec2(600.0, 78.0 + if v.weapons.is_empty() { 0.0 } else { 26.0 }));
        ds::place(ctx, "tmh-card", r, |ui| card_panel(ui, v, r.size()));
    }
    let order = Rect::from_min_size(pos2(ds::BOARD_EDGE, 664.0 * g), vec2(w - 2.0 * ds::BOARD_EDGE, 150.0 * g.max(0.8)));
    ds::place(ctx, "tmh-order", order, |ui| order_panel(ui, v, order.size(), &mut acts));
    let line = Rect::from_min_size(pos2(ds::BOARD_EDGE, order.bottom() + 12.0 * g), vec2(w - 2.0 * ds::BOARD_EDGE, 34.0));
    if !v.line.is_empty() {
        ds::place(ctx, "tmh-line", line, |ui| {
            let k = ds::tokens();
            ui.painter().rect_filled(line, ds::RADIUS_PLATE, egui::Color32::BLACK);
            ds::rim(ui.painter(), line, ds::RADIUS_PLATE, k.info.gamma_multiply(0.7));
            Text::new(&v.line, Font::BODY.sized(17.0)).shrink(12.0).color(k.info).paint(ui, line.shrink2(vec2(16.0, 0.0)));
        });
    }
    let ships = Rect::from_min_max(pos2(ds::BOARD_EDGE, line.bottom() + 20.0 * g), pos2(w - ds::BOARD_EDGE - ds::EXIT_W - 40.0, ds::board_bottom(ctx)));
    ds::place(ctx, "tmh-ships", ships, |ui| deliveries_panel(ui, v, ships.size()));
    if ds::exit_plate(ctx, "tmh-exit", &v.labels.exit, true) {
        acts.push(Act::Bar(sm::Act::Screen(2)));
    }
    acts
}

/// An offers list: the header, every offer of the class (the original shows eight at a time; this one scrolls), the
/// chosen one framed.
fn list_panel(ui: &mut Ui, v: &View, kind: usize, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let inner = ds::board_panel(ui, size, PanelKind::Deep, PAD);
    let head = Rect::from_min_size(inner.min, vec2(inner.width(), 20.0));
    ds::table_head(ui, &ds::table_cols(head, &OFFER_COLS), &[&v.labels.lists[kind], &v.labels.price, &v.labels.count], &OFFER_RIGHT);
    ui.painter().hline(inner.x_range(), head.bottom() + 6.0, egui::Stroke::new(1.0f32, k.rim));
    let body = Rect::from_min_max(pos2(inner.left(), inner.top() + 32.0), inner.max);
    let rows = &v.lists[kind];
    if rows.is_empty() {
        Text::new(&v.labels.none, Font::BODY.sized(17.0)).color(k.ink_muted).paint(ui, Rect::from_min_size(body.min + vec2(8.0, 0.0), vec2(body.width(), ROW_H)));
        return;
    }
    let chosen = v.offer.as_ref().filter(|o| o.kind == kind).map(|o| o.row);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt(("tmh-rows", kind)).auto_shrink([false; 2]).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        for (i, row) in rows.iter().enumerate() {
            let (price, count) = (row.price.to_string(), row.count.to_string());
            let cells = [(row.name.as_str(), Font::BODY.sized(18.0), k.ink), (&price, Font::DATA.sized(17.0), k.info), (&count, Font::DATA.sized(17.0), k.info)];
            let resp = ds::table_row(ui, ROW_H, &OFFER_COLS, &cells, &OFFER_RIGHT, Some(chosen == Some(i as i32)));
            // Hot-spots 0x23/0x24 take either button (`Trade_Main`).
            if ds::either(&ds::tip(resp, ds::tip_text(&v.tips, 0x23 + kind as u32))) {
                acts.push(Act::Row(kind, i as i32));
            }
        }
    });
}

/// The shown hull's card: two columns of label and value, the weapons under them.
fn card_panel(ui: &mut Ui, v: &View, size: egui::Vec2) {
    let k = ds::tokens();
    let inner = ds::board_panel(ui, size, PanelKind::Glass, CARD_PAD);
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    c.spacing_mut().item_spacing = vec2(24.0, 6.0);
    c.columns(2, |cols| {
        for (i, (l, val)) in v.card.iter().enumerate() {
            ds::kv(&mut cols[i % 2], l, val, if i == 0 { ds::Tone::Neutral } else { ds::Tone::Info }, 76.0);
        }
    });
    if !v.weapons.is_empty() {
        Text::new(&v.weapons, Font::DATA.sized(15.0)).shrink(11.0).color(k.ink_silver).paint(ui, Rect::from_min_size(pos2(inner.left(), inner.top() + 52.0), vec2(inner.width(), 24.0)));
    }
}

/// The order bar: the item, the quantity with − and +, the destination's dropdown (its days), the total, the order.
fn order_panel(ui: &mut Ui, v: &View, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let inner = ds::board_panel(ui, size, PanelKind::Glass, vec2(28.0, 20.0));
    let o = v.offer.clone().unwrap_or_default();
    let has = v.offer.is_some();
    let cy = inner.center().y + 10.0;
    let label = |ui: &Ui, x: f32, w: f32, t: &str| Text::new(t, Font::LABEL).shrink(10.0).color(k.ink_muted).paint(ui, Rect::from_min_size(pos2(x, cy - 44.0), vec2(w, 18.0)));
    // The item.
    let x0 = inner.left();
    label(ui, x0, 300.0, &v.labels.item);
    Text::new(&o.name, Font::HEADING.sized(22.0)).shrink(14.0).color(k.ink).paint(ui, Rect::from_min_size(pos2(x0, cy - 18.0), vec2(300.0, 32.0)));
    // The quantity: − [n / left] +.
    let x1 = x0 + 332.0;
    label(ui, x1, 220.0, &v.labels.amount);
    let minus = Rect::from_center_size(pos2(x1 + 18.0, cy), vec2(38.0, 38.0));
    let lcd = Rect::from_min_size(pos2(minus.right() + 10.0, cy - 16.0), vec2(72.0, 32.0));
    let plus = Rect::from_center_size(pos2(lcd.right() + 28.0, cy), vec2(38.0, 38.0));
    for (r, more, glyph, id) in [(minus, false, "−", 0x19), (plus, true, "+", 0x1a)] {
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r));
        c.add_enabled_ui(has, |ui| {
            let resp = ds::tip(ds::round_button(ui, ds::Glyph::Text(glyph), true, false), ds::tip_text(&v.tips, id));
            // `Trade_Main` 0x19/0x1a: a step on the press, every frame while the right button is held.
            let (pressed, right) = ui.input(|i| (i.pointer.any_pressed(), i.pointer.secondary_down()));
            if resp.is_pointer_button_down_on() && (pressed || right) {
                acts.push(Act::Qty(more));
                ui.ctx().request_repaint();
            }
        });
    }
    ds::lcd_window(ui.painter(), lcd);
    Text::new(&if has { format!("{}/{}", o.qty, o.left) } else { String::new() }, Font::READOUT.sized(20.0)).center().shrink(13.0).color(k.lcd_ink).paint(ui, lcd);
    // The destination: a dropdown of the ports, its days under it.
    let x2 = x1 + 252.0;
    let dw = (inner.right() - 280.0 - 260.0 - 32.0 - x2).clamp(220.0, 460.0);
    label(ui, x2, dw, &v.labels.dest);
    let dd = Rect::from_min_size(pos2(x2, cy - 20.0), vec2(dw, 40.0));
    let shown = if o.dest < 0 { "—" } else { &o.dest_name };
    let rows: Vec<(i32, &str, bool)> = if v.ports.is_empty() { vec![(-1, &v.line, true)] } else { v.ports.iter().map(|(p, n)| (*p, n.as_str(), false)).collect() };
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(dd));
    let (resp, picked) = c.add_enabled_ui(has, |ui| ds::select(ui, "tmh-dest", shown, &rows, dd.size(), Font::BODY.sized(18.0), k.ink)).inner;
    ds::tip(resp, ds::tip_text(&v.tips, 0x1b));
    match picked {
        Some(Some(p)) => acts.push(Act::Dest(p)),
        Some(None) => acts.push(Act::Click),
        None => {}
    }
    if o.dest >= 0 {
        Text::new(&format!("{} {}", v.labels.eta, o.eta), Font::LABEL.sized(13.0)).shrink(10.0).color(k.info).paint(ui, Rect::from_min_size(pos2(x2, dd.bottom() + 4.0), vec2(dw, 16.0)));
    }
    // The total and the order.
    let x3 = dd.right() + 32.0;
    label(ui, x3, 200.0, &v.labels.total);
    let tot = Rect::from_min_size(pos2(x3, cy - 16.0), vec2(120.0, 32.0));
    ds::lcd_window(ui.painter(), tot);
    Text::new(&if has { format!("${}", o.total) } else { String::new() }, Font::READOUT.sized(20.0)).center().shrink(13.0).color(k.lcd_ink).paint(ui, tot);
    let buy = Rect::from_min_size(pos2(inner.right() - 280.0, cy - 22.0), vec2(280.0, 44.0));
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(buy));
    if ds::tip(c.add_enabled(o.ok, ds::Plate::new(&v.labels.buy).width(280.0).height(44.0).primary(true)), ds::tip_text(&v.tips, 0x1c)).clicked() {
        acts.push(Act::Buy);
    }
}

/// The deliveries under way: planet, days, item, amount (the original's block B, at most ten shown; this scrolls).
fn deliveries_panel(ui: &mut Ui, v: &View, size: egui::Vec2) {
    let k = ds::tokens();
    let inner = ds::board_panel(ui, size, PanelKind::Deep, PAD);
    let w = inner.width();
    let cols = [0.0, w * 0.12, w * 0.24, w * 0.12];
    let right = [false, true, true, true];
    let head = Rect::from_min_size(inner.min, vec2(w, 20.0));
    ds::table_head(ui, &ds::table_cols(head, &cols), &[&v.labels.under_way, &v.labels.amount, &v.labels.dest, &v.labels.eta], &right);
    ui.painter().hline(inner.x_range(), head.bottom() + 6.0, egui::Stroke::new(1.0f32, k.rim));
    let body = Rect::from_min_max(pos2(inner.left(), inner.top() + 32.0), inner.max);
    if v.deliveries.is_empty() {
        Text::new(&v.labels.nothing, Font::BODY.sized(17.0)).color(k.ink_muted).paint(ui, Rect::from_min_size(body.min + vec2(8.0, 0.0), vec2(w, 26.0)));
        return;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt("tmh-del").auto_shrink([false; 2]).show(&mut child, |ui| {
        for (planet, days, item, amount) in &v.deliveries {
            let (amount, days) = (amount.to_string(), days.to_string());
            let d = Font::DATA.sized(16.0);
            ds::table_row(ui, 26.0, &cols, &[(item.as_str(), Font::BODY.sized(16.0), k.ink), (&amount, d, k.info), (planet, d, k.info), (&days, d, k.info)], &right, None);
        }
    });
}

/// What the modern trade screen shows this frame, from the classic screen's state.
pub fn view(h: &crate::ui::Hud, state: &ig2_sim::save::SaveState, rules: &ig2_rules::Rules, s: &crate::ui::UiSnap, tr: &crate::trade::Trade, gates: Option<&[bool; 20]>, time: f32) -> View {
    let lang = h.texts.lang;
    let dec = |b: &[u8]| ig2_formats::text::decode(b.split(|&c| c == 0).next().unwrap_or(&[]), lang).trim().to_string();
    let ours = |e: &str| dec(&h.texts.ours(e));
    let label = |k: u16| dec(&h.texts.get(8, 2, k)).trim_end_matches(':').to_string();
    let lists = [0, 1].map(|k| crate::trade::Trade::modern_rows(state, k).into_iter().map(|(n, price, count)| Row { name: dec(&n), price, count }).collect::<Vec<_>>());
    let ports: Vec<(i32, String)> = crate::trade::Trade::destinations(state).into_iter().map(|(p, n)| (p, dec(&n))).collect();
    let offer = tr.modern_offer(state).map(|(kind, row, qty, left, total, dest, eta, ok)| Offer {
        kind,
        row,
        name: lists[kind.min(1)].get(row.max(0) as usize).map(|r| r.name.clone()).unwrap_or_default(),
        qty,
        left,
        total,
        dest,
        dest_name: ports.iter().find(|p| p.0 == dest).map(|p| p.1.clone()).unwrap_or_default(),
        eta,
        ok,
    });
    let card_label = |k: u16| dec(&h.texts.get(8, 3, k)).trim_end_matches(':').to_string();
    let (card, weapons) = match tr.card_stats(state, rules).filter(|_| tr.card_shown()) {
        Some((hull, value, fire, speed, pods)) => (
            vec![(card_label(1), dec(&crate::hud::tech_name(&h.texts, hull.min(192)))), (card_label(2), value.to_string()), (card_label(3), fire.to_string()), (card_label(4), speed.to_string())],
            if pods.is_empty() {
                format!("{}: {}", card_label(6), card_label(7))
            } else {
                format!("{}: {}", card_label(6), pods.iter().map(|&t| dec(&crate::hud::tech_name(&h.texts, t.min(192)))).collect::<Vec<_>>().join(", "))
            },
        ),
        None => (Vec::new(), String::new()),
    };
    View {
        bar: h.bar_view(state, rules, s, s.human as u8, gates, time, 8),
        live: tr.opened && !tr.closing,
        lists,
        offer,
        card,
        weapons,
        line: if ports.is_empty() { crate::trade::ticker_lines(&h.texts).first().map(|l| dec(l)).unwrap_or_default() } else { String::new() },
        ports,
        deliveries: crate::trade::Trade::deliveries(state).into_iter().map(|(p, d, n, a)| (dec(&p), d, dec(&n), a)).collect(),
        tips: crate::trade::tips(&h.texts).iter().map(|t| dec(t)).collect(),
        labels: Labels {
            lists: [label(3), label(6)],
            price: label(4),
            count: label(5),
            none: ours("No offers"),
            item: label(12),
            amount: label(13),
            dest: label(11),
            eta: label(9),
            total: ours("Total"),
            buy: label(7),
            under_way: ours("Deliveries under way"),
            nothing: ours("None"),
            exit: ours("Exit"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str) -> View {
        let row = |n: &str| Row { name: n.into(), price: 940, count: 5 };
        View {
            live: true,
            lists: [vec![row(name), row(name)], Vec::new()],
            offer: Some(Offer { kind: 0, row: 0, name: name.into(), qty: 1, left: 5, total: 940, dest: -1, ok: true, ..Default::default() }),
            card: vec![(name.into(), name.into()); 4],
            ports: vec![(3, name.into())],
            line: name.into(),
            labels: Labels { exit: name.into(), buy: name.into(), ..Default::default() },
            ..Default::default()
        }
    }

    fn run(v: &View, clicks: &[egui::Pos2]) -> (Vec<Rect>, Vec<Act>) {
        let gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb);
        let mut st = State::default();
        let (mut acts, mut rects) = (Vec::new(), Vec::new());
        for f in 0..(6 + 3 * clicks.len()) {
            let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1920.0, 1080.0))), ..Default::default() };
            let button = |p, pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            if f >= 2 && let Some(&p) = clicks.get((f - 2) / 3) {
                input.events = match (f - 2) % 3 {
                    0 => vec![egui::Event::PointerMoved(p)],
                    1 => vec![button(p, true)],
                    _ => vec![button(p, false)],
                };
            }
            let _ = gui.ctx.run(input, |ctx| acts.extend(show(ctx, v, &mut st)));
            rects = ["tmh-list0", "tmh-list1", "tmh-order", "tmh-ships", "tmh-exit", "tmh-well"].map(|id| gui.ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap()).to_vec();
        }
        (rects, acts)
    }

    /// Long names never move a panel; a row, +, the destination and the order answer a press; the well sits at
    /// the board's place.
    #[test]
    fn panels_and_presses() {
        let (short, _) = run(&view("Tra"), &[]);
        let (long, _) = run(&view(&"Kémelhárítási jelentés ".repeat(10)), &[]);
        assert_eq!(short, long);
        assert!((short[5].center() - pos2(960.0, 247.0)).length() < 1.5, "{:?}", short[5]);
        let list = short[0];
        let (_, a) = run(&view("Tra"), &[pos2(list.left() + 100.0, list.top() + 18.0 + 32.0 + ROW_H + 2.0 + 10.0)]);
        assert!(a.contains(&Act::Row(0, 1)), "{a:?}");
        let o = short[2];
        let cy = o.center().y + 10.0;
        let x1 = o.left() + 28.0 + 332.0;
        let (_, a) = run(&view("Tra"), &[pos2(x1 + 18.0 + 38.0 + 10.0 + 72.0 + 28.0 - 19.0, cy)]);
        assert!(a.contains(&Act::Qty(true)), "{a:?}");
        let x2 = x1 + 252.0;
        let (_, a) = run(&view("Tra"), &[pos2(x2 + 40.0, cy), pos2(x2 + 40.0, cy + 20.0 + 4.0 + 6.0 + 15.0)]);
        assert!(a.contains(&Act::Dest(3)), "{a:?}");
        let (_, a) = run(&view("Tra"), &[pos2(o.right() - 28.0 - 100.0, cy)]);
        assert!(a.contains(&Act::Buy), "{a:?}");
        let (_, a) = run(&view("Tra"), &[short[4].center()]);
        assert!(a.contains(&Act::Bar(sm::Act::Screen(2))), "{a:?}");
    }
}
