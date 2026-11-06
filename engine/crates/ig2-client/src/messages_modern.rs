//! Ours (ADR 0076): the modern messages screen on the egui layer (ADR 0062), the "Modern messages screen" option,
//! drawn with the design kit (ADR 0073, `gui::ds`) after the messages board. The same functions as the original's
//! (re/screens/messages.md §3–§5): the whole inbox as one list from the top, oldest first and the newest at the
//! bottom, scrolling (a row's press moves the original's four rows to it as its arrows would: hot-spots 1, 2, 3),
//! the talking head's clip, the open message typed out with its colours (a press in it hurries
//! the typer, as a press in the original's text box), its answers (hot-spot 4, the keys `a`..`d`) and the way out
//! (7..=9). The window fills a [`View`] from the classic screen's state, runs [`show`] once a frame and hands the
//! [`Act`]s to the classic `MessagesScreen` code at the virtual point the original's player would have pressed.

use egui::{pos2, vec2, Color32, Rect, Sense, Ui};

use crate::gui::ds::{self, Font, PanelKind, Text, Weight};
use crate::starmap_modern::{self as sm};

/// What the panels ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    Bar(sm::Act),
    /// The inbox entry `i` pressed (`MessagesScreen::reach` finds its row).
    Row(usize),
    /// Answer `k` (0..=3) picked.
    Answer(usize),
    /// A press in the text: type the rest at once.
    Hurry,
    /// Back to the screen before (7..=9).
    Leave,
}

/// One inbox row: its inbox entry, the sender, the date, the two subject lines, unread (`kept` clear and not opened
/// here) and open now.
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub i: usize,
    pub sender: String,
    pub date: String,
    pub subject: [String; 2],
    pub unread: bool,
    pub open: bool,
}

/// The open message: its subject, sender, date, the text typed so far in its colours, the typer still running, the
/// answers (the label, disabled by the story, under the pointer) once they show.
#[derive(Debug, Clone, Default)]
pub struct Open {
    pub subject: String,
    pub sender: String,
    pub date: String,
    pub body: Vec<(String, Color32)>,
    pub typing: bool,
    pub answers: Vec<(usize, String, bool)>,
}

#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub title: String,
    pub count: String,
    pub unread: String,
    pub sender: String,
    pub date: String,
    pub full_text: String,
    pub exit: String,
}

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    /// The whole inbox, oldest first.
    pub rows: Vec<Row>,
    pub open: Option<Open>,
    /// The way out is live (no answer pending and the text typed: `Messages_MouseInit2`'s 7..=9).
    pub leave: bool,
    pub labels: Labels,
}

/// What stays between frames: the talking head's well this frame (pixels).
#[derive(Default)]
pub struct State {
    pub bar: sm::State,
    pub well: Option<Rect>,
}

/// The board's measures (1920 × 1080).
const TOP: f32 = 72.0;
const INBOX_W: f32 = 540.0;
const HEAD: egui::Vec2 = egui::Vec2::new(720.0, 405.0);
const GAP: f32 = 40.0;
const ROW_H: f32 = 96.0;
const ROW_GAP: f32 = 6.0;

pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut acts = sm::board_bar(ctx, &v.bar, &mut st.bar, Act::Bar);
    let k = ds::tokens();
    let screen = ctx.screen_rect();
    let (w, h) = (screen.width(), screen.height());
    let top = sm::bar_bottom(ctx) + TOP - 44.0;
    let g = ds::squeeze(h);
    egui::CentralPanel::default().frame(egui::Frame::new().fill(k.void.gamma_multiply(0.94))).show(ctx, |ui| {
        ui.allocate_rect(ui.max_rect(), Sense::click_and_drag());
    });
    let bottom = h - 40.0 * g;
    let inbox = Rect::from_min_max(pos2(ds::BOARD_EDGE, top), pos2(ds::BOARD_EDGE + INBOX_W.min(w * 0.3), bottom));
    ds::place(ctx, "mmh-inbox", inbox, |ui| inbox_panel(ui, v, inbox.size(), &mut acts));
    // Right of the inbox: the head and the message's card side by side, the text under them, the plates at the foot.
    let x0 = inbox.right() + GAP;
    let room = w - ds::BOARD_EDGE - x0;
    let head_w = HEAD.x.min(room * 0.6);
    let head = Rect::from_min_size(pos2(x0, top), vec2(head_w, (head_w * 9.0 / 16.0).min(HEAD.y)));
    ds::place(ctx, "mmh-head", head, |ui| {
        well(ui, head);
        st.well = Some(Rect::from_min_max(head.min * ctx.pixels_per_point(), head.max * ctx.pixels_per_point()));
    });
    let card = Rect::from_min_max(pos2(head.right() + GAP, top), pos2(w - ds::BOARD_EDGE, head.bottom()));
    ds::place(ctx, "mmh-card", card, |ui| card_panel(ui, v, card.size(), &mut acts));
    let plates = 48.0;
    // The board's 430-high text panel, shorter where the window is.
    let text_top = head.bottom() + 28.0 * g;
    let text = Rect::from_min_max(pos2(x0, text_top), pos2(w - ds::BOARD_EDGE, (text_top + 430.0).min(bottom - plates - 24.0 * g)));
    ds::place(ctx, "mmh-text", text, |ui| text_panel(ui, v, text.size(), &mut acts));
    let full = Rect::from_min_size(pos2(x0, bottom - plates), vec2(260.0, plates));
    ds::place(ctx, "mmh-full", full, |ui| {
        let typing = v.open.as_ref().is_some_and(|o| o.typing);
        if ds::either(&ui.add_enabled(typing, ds::Plate::new(&v.labels.full_text).width(260.0).height(plates))) {
            acts.push(Act::Hurry);
        }
    });
    let exit = Rect::from_min_size(pos2(w - ds::BOARD_EDGE - 220.0, bottom - plates), vec2(220.0, plates));
    ds::place(ctx, "mmh-exit", exit, |ui| {
        if ds::either(&ui.add_enabled(v.leave, ds::Plate::new(&v.labels.exit).width(220.0).height(plates))) {
            acts.push(Act::Leave);
        }
    });
    acts
}

/// The head's well: black in a cast-metal frame; the window draws the clip into it ([`State::well`]).
fn well(ui: &Ui, r: Rect) {
    let k = ds::tokens();
    let p = ui.painter();
    p.rect_filled(r.expand(6.0), ds::RADIUS_PANEL + 4, k.metal_shadow);
    p.rect_filled(r, ds::RADIUS_PANEL, Color32::BLACK);
    ds::rim(p, r.expand(1.0), ds::RADIUS_PANEL, k.metal_dark);
    ds::relief(p, r.expand(6.0), ds::RADIUS_PANEL + 4);
}

/// The inbox: the title, the new and total counts, every message from the top (oldest first, the newest at the
/// bottom, scrolling with the newest kept in view), the colour hint.
fn inbox_panel(ui: &mut Ui, v: &View, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Deep, 0.88);
    let inner = rect.shrink(16.0);
    let head = Rect::from_min_size(inner.min, vec2(inner.width(), 40.0));
    let count_w = 130.0;
    Text::new(&v.labels.title, Font::HEADING).shrink(15.0).color(k.ink).paint(ui, head.with_max_x(head.right() - count_w - 8.0));
    Text::new(&v.labels.count, Font::LABEL).right().shrink(10.0).color(k.ink_muted).paint(ui, head.with_min_x(head.right() - count_w));
    let list = Rect::from_min_max(pos2(inner.left(), head.bottom() + 12.0), pos2(inner.right(), inner.bottom() - 30.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt("mmh-rows").auto_shrink([false; 2]).stick_to_bottom(true).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing.y = ROW_GAP;
        for r in &v.rows {
            inbox_row(ui, r, acts);
        }
    });
    let foot = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 18.0), inner.right_bottom());
    let dot = Rect::from_center_size(pos2(foot.left() + 5.0, foot.center().y), vec2(8.0, 8.0));
    ui.painter().circle_filled(dot.center(), 4.0, k.lcd_ink);
    Text::new(&v.labels.unread, Font::LABEL.upper(false)).shrink(10.0).color(k.ink_muted).paint(ui, foot.with_min_x(foot.left() + 16.0));
}

/// One inbox row, the next one down the list.
fn inbox_row(ui: &mut Ui, r: &Row, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let (at, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
    ds::row_back(ui, at, hot, r.open);
    let inner = at.shrink2(vec2(12.0, 10.0));
    let line = |n: f32, h: f32| Rect::from_min_size(pos2(inner.left(), inner.top() + n), vec2(inner.width(), h));
    let top = line(0.0, 24.0);
    let date_w = 110.0;
    Text::new(&r.sender, Font::HEADING.sized(18.0).weight(Weight::Medium)).shrink(13.0).color(if r.unread { k.lcd_ink } else { k.ink_silver }).paint(ui, top.with_max_x(top.right() - date_w - 8.0));
    Text::new(&r.date, Font::DATA).right().baseline(ds::fit::baseline(top, Font::HEADING.sized(18.0))).color(k.info).paint(ui, top.with_min_x(top.right() - date_w));
    Text::new(&r.subject[0], Font::BODY.sized(15.0)).shrink(12.0).color(if r.unread { k.ink } else { k.ink_silver }).paint(ui, line(28.0, 20.0));
    Text::new(&r.subject[1], Font::DATA).shrink(11.0).color(if r.unread { k.ink_silver } else { k.ink_muted }).paint(ui, line(50.0, 20.0));
    if ds::either(&resp.on_hover_cursor(egui::CursorIcon::PointingHand)) {
        acts.push(Act::Row(r.i));
    }
}

/// The open message's card: the subject, the sender and the date as table lines, the answers once they show.
fn card_panel(ui: &mut Ui, v: &View, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Glass, 0.88);
    let Some(o) = &v.open else { return };
    let inner = rect.shrink2(vec2(24.0, 20.0));
    Text::new(&o.subject, Font::HEADING).lines(2).color(k.ink).paint(ui, Rect::from_min_size(inner.min, vec2(inner.width(), 52.0)));
    for (n, (l, val)) in [(&v.labels.sender, &o.sender), (&v.labels.date, &o.date)].into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(inner.left(), inner.top() + 56.0 + n as f32 * 28.0), vec2(inner.width(), 22.0));
        let base = ds::fit::baseline(r, Font::DATA);
        Text::new(l, Font::LABEL).shrink(10.0).baseline(base).color(k.ink_muted).paint(ui, r.with_max_x(r.left() + 100.0));
        Text::new(val, Font::DATA).shrink(11.0).baseline(base).color(k.ink).paint(ui, r.with_min_x(r.left() + 110.0));
    }
    // The answers (`Valasztasok`, a..d), each a plate of two lines with its key, from the card's foot up.
    let keys = ["A", "B", "C", "D"];
    let ah = 52.0;
    let first = inner.bottom() - o.answers.len() as f32 * (ah + 8.0) + 8.0;
    for (j, (i, label, off)) in o.answers.iter().enumerate() {
        let i = *i;
        let r = Rect::from_min_size(pos2(inner.left(), (first + j as f32 * (ah + 8.0)).max(inner.top() + 124.0)), vec2(inner.width(), ah));
        // A disabled answer still takes the press: it stays asked for until the story allows it (`ValaszKivalasztva`).
        let resp = ui.interact(r, ui.id().with(("mmh-answer", i)), Sense::click());
        let hot = ds::hover_t(ui.ctx(), resp.id, !off && resp.hovered());
        ds::plate_fill(ui.painter(), r, ds::RADIUS_PLATE, resp.is_pointer_button_down_on());
        ds::hover_rim(ui.painter(), r, ds::RADIUS_PLATE, hot);
        let cap = Rect::from_min_size(r.min + vec2(10.0, (ah - 28.0) / 2.0), vec2(28.0, 28.0));
        ds::lcd_window(ui.painter(), cap);
        Text::new(keys[i.min(3)], Font::READOUT.sized(18.0)).center().color(if *off { k.ink_muted } else { k.lcd_ink }).paint(ui, cap);
        Text::new(label, Font::DATA).lines(2).color(if *off { k.ink_off } else { k.ink }).paint(ui, Rect::from_min_max(pos2(cap.right() + 12.0, r.top() + 6.0), r.max - vec2(10.0, 4.0)));
        if ds::either(&resp.on_hover_cursor(if *off { egui::CursorIcon::NotAllowed } else { egui::CursorIcon::PointingHand })) {
            acts.push(Act::Answer(i));
        }
    }
}

/// The text: the open message as wrapped body text in its colours (the newest lines kept in view), the typer's caret
/// blinking at its end; a press in it hurries the typer.
fn text_panel(ui: &mut Ui, v: &View, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Deep, 0.88);
    let Some(o) = &v.open else { return };
    let resp = ui.interact(rect, ui.id().with("mmh-text"), Sense::click());
    let inner = rect.shrink2(vec2(32.0, 24.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt("mmh-body").auto_shrink([false; 2]).stick_to_bottom(true).show(&mut child, |ui| {
        let f = Font::BODY.sized(19.0);
        let mut job = egui::text::LayoutJob { wrap: egui::text::TextWrapping { max_width: inner.width().min(1140.0) - 16.0, ..Default::default() }, ..Default::default() };
        for (t, c) in &o.body {
            job.append(&t.replace('\n', "\n\n"), 0.0, egui::TextFormat { font_id: f.id(), color: *c, extra_letter_spacing: f.spacing(), line_height: Some(32.0), ..Default::default() });
        }
        let galley = ui.fonts(|x| x.layout_job(job));
        let (r, _) = ui.allocate_exact_size(vec2(inner.width() - 16.0, galley.size().y.max(32.0)), Sense::hover());
        let end = galley.rect.right_bottom();
        let last = galley.rows.last().map_or(egui::Rect::NOTHING, |row| row.rect());
        ui.painter().galley(r.min, galley, k.ink);
        if o.typing && (ui.input(|i| i.time) * 2.5) as i64 % 2 == 0 {
            let x = r.left() + if last.is_finite() { last.right() } else { end.x } + 2.0;
            let y = r.top() + if last.is_finite() { last.top() } else { 0.0 };
            ui.painter().rect_filled(Rect::from_min_size(pos2(x, y + 6.0), vec2(10.0, 20.0)), 0, k.ink);
        }
    });
    if o.typing {
        ui.ctx().request_repaint();
    }
    if ds::either(&resp) && o.typing {
        acts.push(Act::Hurry);
    }
}

/// The date as the inbox shows it (`Ido2Ev/Honap/Nap`: year.month.day.).
fn date(t: i32) -> String {
    format!("{}.{:02}.{:02}.", t / 18000, t / 1500 % 12 + 1, t / 50 % 30 + 1)
}

/// What the modern messages screen shows this frame, from the classic screen's state.
/// The inbox's unread messages: not kept and not opened this session.
pub fn unread(h: &crate::ui::Hud, s: &crate::ui::UiSnap) -> usize {
    s.inbox.iter().enumerate().filter(|&(i, m)| m.1 & 1 == 0 && !h.messages.read.contains(&i)).count()
}

pub fn view(h: &crate::ui::Hud, state: &ig2_sim::save::SaveState, rules: &ig2_rules::Rules, s: &crate::ui::UiSnap, gates: Option<&[bool; 20]>, time: f32) -> View {
    let lang = h.texts.lang;
    let dec = |b: &[u8]| ig2_formats::text::decode(b.split(|&c| c == 0).next().unwrap_or(&[]), lang).trim().to_string();
    let col = |i: i32| crate::gui::ink(h.palette.get(i.max(0) as usize).copied().unwrap_or([1.0; 3]));
    let ours = |e: &str| dec(&h.texts.ours(e));
    let m = &h.messages;
    let n = s.inbox.len();
    let kind = |i: usize| s.inbox.get(i).and_then(|&(block, _, _)| h.texts.kinds.iter().find(|k| k.id as i32 == block));
    let alias = |i: usize, t: &str| dec(&crate::messages_screen::alias_conv(&h.texts.encode(t), s.inbox_params.get(i).map_or(&[][..], |p| &p[..])));
    let open_index = m.open.as_ref().map(|d| d.index);
    let rows = if m.list_up() {
        (0..n)
            .map(|i| {
                let (_, kept, t) = s.inbox[i];
                let k = kind(i);
                Row {
                    i,
                    sender: k.map(|k| alias(i, &k.sender)).unwrap_or_default(),
                    date: date(t),
                    subject: [0, 1].map(|j| k.map(|k| alias(i, &k.subject[j])).unwrap_or_default()),
                    unread: kept & 1 == 0 && !m.read.contains(&i),
                    open: open_index == Some(i),
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let unread = unread(h, s);
    let blink = (time * 1000.0 / 200.0) as i64 % 2 == 1;
    let open = m.open.as_ref().filter(|_| m.text_up()).map(|d| {
        let k = kind(d.index);
        Open {
            subject: k.map(|k| alias(d.index, &k.subject[0])).unwrap_or_default(),
            sender: k.map(|k| alias(d.index, &k.sender)).unwrap_or_default(),
            date: s.inbox.get(d.index).map(|e| date(e.2)).unwrap_or_default(),
            body: d.pieces(blink).into_iter().map(|(t, c)| (ig2_formats::text::decode(&t, lang), col(c))).collect(),
            typing: !d.typed_all,
            answers: if d.typed_all && !d.over { d.answers.iter().enumerate().filter(|(_, a)| a.part != -1).map(|(i, a)| (i, dec(&a.label), a.disabled)).collect() } else { Vec::new() },
        }
    });
    let spot = |at: [f32; 2], id: u32| m.spot_at(at) == Some(id);
    View {
        bar: h.bar_view(state, rules, s, s.human as u8, gates, time, 10),
        rows,
        open,
        leave: spot(LEAVE_AT, 7),
        labels: Labels {
            title: ours("Messages"),
            count: format!("{unread} {} · {n}", ours("new")),
            unread: ours("Yellow: unread"),
            sender: ours("Sender"),
            date: ours("Date"),
            full_text: ours("Full text"),
            exit: ours("Exit"),
        },
    }
}

/// The virtual points the acts press (`Messages_MouseInit`'s hot-spots, `messages_screen::SPOTS`): the way out 7,
/// the text 6, the inbox row `k`'s quarter of spot 3.
pub const LEAVE_AT: [f32; 2] = [20.0, 20.0];
pub const TEXT_AT: [f32; 2] = [300.0, 80.0];
pub fn row_at(k: usize) -> [f32; 2] {
    [60.0, 218.0 + (3 - k.min(3)) as f32 * 44.0 + 22.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str) -> View {
        with_rows(name, 4)
    }

    fn with_rows(name: &str, n: usize) -> View {
        let rows = (0..n).map(|i| Row { i, sender: name.into(), date: "2327.10.23.".into(), subject: [name.into(), name.into()], unread: i == 3, open: i == 2 }).collect();
        let open = Open { subject: name.into(), sender: name.into(), date: name.into(), body: vec![(name.repeat(40), Color32::WHITE)], typing: true, answers: vec![(0, name.into(), false), (2, name.into(), true)] };
        View { rows, open: Some(open), leave: true, labels: Labels { title: name.into(), exit: name.into(), full_text: name.into(), ..Default::default() }, ..Default::default() }
    }

    fn run(v: &View, click: Option<egui::Pos2>) -> (Vec<Rect>, Vec<Act>) {
        let gui = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb);
        let mut st = State::default();
        let (mut acts, mut rects) = (Vec::new(), Vec::new());
        for f in 0..6 {
            let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1920.0, 1080.0))), ..Default::default() };
            let button = |p, pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            match (click, f) {
                (Some(p), 2) => input.events = vec![egui::Event::PointerMoved(p)],
                (Some(p), 3) => input.events = vec![button(p, true)],
                (Some(p), 4) => input.events = vec![button(p, false)],
                _ => {}
            }
            let _ = gui.ctx.run(input, |ctx| acts.extend(show(ctx, v, &mut st)));
            rects = ["mmh-inbox", "mmh-head", "mmh-card", "mmh-text", "mmh-exit"].map(|id| gui.ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap()).to_vec();
        }
        (rects, acts)
    }

    /// Long texts never move a panel; the top row is the oldest message, the answers and the text answer a press.
    #[test]
    fn panels_and_presses() {
        let (short, _) = run(&view("Tra"), None);
        let (long, _) = run(&view(&"Kémelhárítási jelentés ".repeat(10)), None);
        assert_eq!(short, long);
        let inbox = short[0];
        let (_, a) = run(&view("Tra"), Some(short[4].center()));
        assert!(a.contains(&Act::Leave), "{a:?}");
        let (_, a) = run(&view("Tra"), Some(pos2(inbox.left() + 100.0, inbox.top() + 16.0 + 40.0 + 12.0 + 40.0)));
        assert!(a.contains(&Act::Row(0)), "{a:?}");
        let (_, a) = run(&view("Tra"), Some(short[3].center()));
        assert!(a.contains(&Act::Hurry), "{a:?}");
        let card = short[2];
        let (_, a) = run(&view("Tra"), Some(pos2(card.center().x, card.bottom() - 20.0 - 52.0 - 8.0 - 26.0)));
        assert!(a.contains(&Act::Answer(0)), "{a:?}");
        // A disabled answer is still asked for (the sim's `ValaszKivalasztva` keeps it pending).
        let (_, a) = run(&view("Tra"), Some(pos2(card.center().x, card.bottom() - 20.0 - 26.0)));
        assert!(a.contains(&Act::Answer(2)), "{a:?}");
    }

    /// The list starts at the panel's top, oldest first, one row under the other; a long list scrolls with the
    /// newest at the panel's bottom.
    #[test]
    fn rows_from_the_top_newest_at_the_bottom() {
        let (rects, _) = run(&with_rows("Tra", 2), None);
        let inbox = rects[0];
        let list_top = inbox.top() + 16.0 + 40.0 + 12.0;
        for (i, y) in [(0, list_top + 4.0), (1, list_top + ROW_H + ROW_GAP + 4.0)] {
            let (_, a) = run(&with_rows("Tra", 2), Some(pos2(inbox.left() + 100.0, y)));
            assert_eq!(a, [Act::Row(i)]);
        }
        // Nothing under the two rows.
        let (_, a) = run(&with_rows("Tra", 2), Some(pos2(inbox.left() + 100.0, list_top + 2.0 * (ROW_H + ROW_GAP) + 20.0)));
        assert!(a.is_empty(), "{a:?}");
        let list_bottom = inbox.bottom() - 16.0 - 30.0;
        let (_, a) = run(&with_rows("Tra", 30), Some(pos2(inbox.left() + 100.0, list_bottom - 4.0)));
        assert_eq!(a, [Act::Row(29)]);
    }
}
