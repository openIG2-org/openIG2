//! Ours (ADR 0076): the modern spy screen on the egui layer (ADR 0062), the "Modern spy screen" option, drawn with
//! the design kit (ADR 0073, `gui::ds`) after the spy board. The 3D roster (the agents on their pedestals turning
//! on the ring) and the head ring stay; the pedestals' names, the agent's card, the planner and the reports are
//! panels. Their texts are the original's own 2D pass recorded instead of drawn (`SpyView::capture`), laid out as
//! rows; a field opens the original's pick list as a dropdown and a pick replays the press and release on its row
//! (`SpyView::modern_press`), so every order, report and story hand-off stays the original's (re/screens/spy.md).

use egui::{pos2, vec2, Color32, Rect, Sense, Ui};

use crate::gui::ds::{self, Font, PanelKind, Text};
use crate::spy_screen::draw::Piece;
use crate::starmap_modern::{self as sm};

/// What the panels ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    Bar(sm::Act),
    /// The arrows: ← the next agent (`left`), → the previous one (`SpyKeyboardHandler`).
    Key(bool),
    /// Pedestal `i` pressed.
    Pedestal(usize),
    /// Hot-spot `id` pressed (a button, a skill's +).
    Press(u32),
    /// Row `value` picked from the list hot-spot `id` opens.
    Pick(u32, i32),
    /// A dropdown opened (the original's press on its field clicks).
    Click,
}

/// A pedestal: its name plate's and figure's places (virtual), the name, its brightness, an agent on it.
#[derive(Debug, Clone, Default)]
pub struct Pedestal {
    pub i: usize,
    pub plate: [f32; 2],
    pub figure: [f32; 2],
    pub name: String,
    pub alpha: f32,
    pub agent: bool,
}

/// A row of the planner or report panel: a label, a value, the list field the value opens.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Line {
    pub label: String,
    pub value: String,
    pub field: Option<u32>,
}

/// A button: its hot-spot, caption, live, and whether it is the panel's main one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Button {
    pub id: u32,
    pub text: String,
    pub on: bool,
    pub primary: bool,
}

/// The agent's card: name, race, experience and rank as rows, the four skills (with their + live), the points.
#[derive(Debug, Clone, Default)]
pub struct Agent {
    pub rows: Vec<(String, String)>,
    pub skills: Vec<(String, String)>,
    pub plus: bool,
    pub points: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub points: String,
    pub exit: String,
}

/// A list's rows: value, text, greyed.
pub type Choices = Vec<(i32, String, bool)>;

/// The panels' rows laid out: the agent's card, the count, the right panel's rows, the buttons' captions.
pub type Laid = (Option<Agent>, String, Vec<Line>, Vec<(u32, String)>);

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    /// Open and not closing: the panels show.
    pub live: bool,
    pub pedestals: Vec<Pedestal>,
    /// The arrows turn the ring (no report up).
    pub arrows: bool,
    pub agent: Option<Agent>,
    pub count: String,
    pub lines: Vec<Line>,
    /// Each list field's rows: value, text, greyed.
    pub fields: Vec<(u32, Choices)>,
    pub buttons: Vec<Button>,
    /// A report's answers stacked (applicant, captured spy).
    pub answers: Vec<Button>,
    /// The hot-spots' tooltips by id.
    pub tips: Vec<String>,
    pub labels: Labels,
}

/// What stays between frames: the kill question up.
#[derive(Default)]
pub struct State {
    pub bar: sm::State,
    kill: bool,
}

/// The board's measures (1920 × 1080).
const EDGE: f32 = 40.0;
const PANEL_TOP: f32 = 520.0;
const PANEL_W: f32 = 600.0;
const EXIT_W: f32 = 220.0;

/// A point of the original's 640 × 480 screen in this page's points.
fn at(ctx: &egui::Context, p: [f32; 2]) -> egui::Pos2 {
    let ppp = ctx.pixels_per_point();
    let r = ctx.screen_rect();
    let [x, y] = crate::font::Virtual::new([r.width() * ppp, r.height() * ppp]).px(p[0], p[1]);
    pos2(x / ppp, y / ppp)
}

pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut bar = Vec::new();
    sm::bar(ctx, &v.bar, &mut st.bar, &mut bar);
    let mut acts: Vec<Act> = bar.into_iter().map(Act::Bar).collect();
    if !v.live {
        st.kill = false;
        return acts;
    }
    let k = ds::tokens();
    let screen = ctx.screen_rect();
    let (w, h) = (screen.width(), screen.height());
    let g = (h / 1080.0).min(1.0);
    // The pedestals: the name on the plate, the figure and plate take a press.
    for p in &v.pedestals {
        let plate = at(ctx, p.plate);
        let fig = at(ctx, p.figure);
        let scale = at(ctx, [100.0, 0.0]).x - at(ctx, [0.0, 0.0]).x;
        let body = Rect::from_min_max(pos2(fig.x - scale * 0.45, fig.y - scale * 0.7), pos2(fig.x + scale * 0.45, plate.y + scale * 0.12));
        let name = Rect::from_center_size(plate, vec2(scale * 0.8, 24.0));
        ds::place(ctx, ("ksh-ped", p.i), body.union(name), |ui| {
            let resp = ui.interact(body.union(name), ui.id().with("ped"), Sense::click());
            Text::new(&p.name, Font::LABEL.sized(15.0)).center().shrink(10.0).color(k.ink.gamma_multiply(p.alpha)).paint(ui, name);
            if p.agent {
                let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                if ds::either(&resp) {
                    acts.push(Act::Pedestal(p.i));
                }
            }
        });
    }
    if v.arrows {
        for (left, x) in [(false, 247.0), (true, w - 247.0)] {
            let r = Rect::from_center_size(pos2(x, 317.0 * g), vec2(40.0, 40.0));
            ds::place(ctx, if left { "ksh-next" } else { "ksh-prev" }, r, |ui| {
                let draw = move |p: &egui::Painter, r: Rect, c: Color32| {
                    let m = r.center();
                    let d = r.width() * 0.12 * if left { 1.0 } else { -1.0 };
                    p.line(vec![m + vec2(-d, -r.width() * 0.2), m + vec2(d, 0.0), m + vec2(-d, r.width() * 0.2)], egui::Stroke::new(2.0f32, c));
                };
                if ds::round_button(ui, ds::Glyph::Custom(&draw), true, false).clicked() {
                    acts.push(Act::Key(left));
                }
            });
        }
    }
    let top = PANEL_TOP * g;
    let left = Rect::from_min_size(pos2(EDGE, top), vec2(PANEL_W.min(w * 0.32), h - 40.0 * g - top));
    ds::place(ctx, "ksh-agent", left, |ui| agent_panel(ui, v, left.size(), &mut acts));
    let right_w = PANEL_W.min(w * 0.32);
    let exit = Rect::from_min_size(pos2(w - EDGE - EXIT_W, h - 40.0 * g - 52.0), vec2(EXIT_W, 52.0));
    let right = Rect::from_min_max(pos2(w - EDGE - right_w, top), pos2(w - EDGE, exit.top() - 28.0 * g));
    ds::place(ctx, "ksh-plan", right, |ui| plan_panel(ui, v, st, right.size(), &mut acts));
    ds::place(ctx, "ksh-exit", exit, |ui| {
        if ui.add(ds::Plate::new(&v.labels.exit).width(EXIT_W).height(52.0)).clicked() {
            acts.push(Act::Bar(sm::Act::Screen(2)));
        }
    });
    // The kill list's question (hot-spot 3) as a confirm box: row 1 yes, row 2 no.
    if st.kill {
        let rows = v.fields.iter().find(|f| f.0 == 3).map(|f| &f.1[..]).unwrap_or(&[]);
        let text = |i: usize| rows.get(i).map(|r| r.1.as_str()).unwrap_or("");
        match ds::confirm(ctx, "ksh-kill", true, text(0), "", text(1), text(2)) {
            Some(true) => {
                acts.push(Act::Pick(3, 1));
                st.kill = false;
            }
            Some(false) => st.kill = false,
            None => {}
        }
    }
    acts
}

/// The agent's card: name, race, experience and rank, the skills with their + buttons, the points, the count.
fn agent_panel(ui: &mut Ui, v: &View, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Deep, 0.9);
    let inner = rect.shrink2(vec2(28.0, 20.0));
    if let Some(a) = &v.agent {
        let mut y = inner.top();
        for (i, (l, val)) in a.rows.iter().enumerate() {
            let r = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 26.0));
            let base = ds::fit::baseline(r, Font::BODY.sized(19.0));
            Text::new(l, Font::LABEL).shrink(10.0).baseline(base).color(k.ink_muted).paint(ui, r.with_max_x(r.left() + 130.0));
            let f = if i == 0 { Font::BODY.sized(19.0) } else { Font::DATA.sized(17.0) };
            Text::new(val, f).shrink(12.0).baseline(base).color(if i == 0 { k.ink } else { k.info }).paint(ui, r.with_min_x(r.left() + 136.0));
            y += 26.0;
        }
        y += 10.0;
        ui.painter().hline(inner.x_range(), y, egui::Stroke::new(1.0f32, k.rim));
        y += 10.0;
        for (i, (l, val)) in a.skills.iter().enumerate() {
            let r = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 40.0));
            let base = ds::fit::baseline(r, Font::DATA.sized(17.0));
            let name = r.with_max_x(r.right() - 120.0);
            Text::new(l, Font::LABEL).shrink(10.0).baseline(base).color(k.ink_muted).paint(ui, name);
            ds::tip(ui.interact(name, ui.id().with(("ksh-skill", i)), Sense::hover()), ds::tip_text(&v.tips, 0x22 + i as u32));
            Text::new(val, Font::DATA.sized(17.0)).right().baseline(base).color(k.info).paint(ui, Rect::from_min_max(pos2(r.right() - 120.0, r.top()), pos2(r.right() - 50.0, r.bottom())));
            let b = Rect::from_center_size(pos2(r.right() - 20.0, r.center().y), vec2(38.0, 38.0));
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(b));
            c.add_enabled_ui(a.plus, |ui| {
                if ds::either(&ds::tip(ds::round_button(ui, ds::Glyph::Text("+"), true, false), ds::tip_text(&v.tips, 0x19 + i as u32))) {
                    acts.push(Act::Press(0x19 + i as u32));
                }
            });
            y += 40.0;
        }
        if let Some(p) = &a.points {
            y += 6.0;
            let r = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), 30.0));
            let base = ds::fit::baseline(r, Font::READOUT.sized(20.0));
            Text::new(&v.labels.points, Font::LABEL).shrink(10.0).baseline(base).color(k.ink_muted).paint(ui, r.with_max_x(r.left() + 150.0));
            let lcd = Rect::from_min_size(pos2(r.left() + 154.0, r.top()), vec2(48.0, 30.0));
            ds::lcd_window(ui.painter(), lcd);
            Text::new(p, Font::READOUT.sized(20.0)).center().color(k.lcd_ink).paint(ui, lcd);
        }
    }
    let foot = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 24.0), inner.max);
    ui.painter().hline(inner.x_range(), foot.top() - 12.0, egui::Stroke::new(1.0f32, k.rim));
    Text::new(&v.count, Font::LABEL.sized(14.0)).shrink(10.0).color(k.ink_silver).paint(ui, foot);
}

/// The planner, the mission under way or the report: its rows (a list field as a dropdown), the report's answers
/// stacked, the buttons along the foot.
fn plan_panel(ui: &mut Ui, v: &View, st: &mut State, size: egui::Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let rect = Rect::from_min_size(ui.max_rect().min, size);
    ds::paint_panel(ui.painter(), rect, PanelKind::Deep, 0.9);
    let inner = rect.shrink2(vec2(28.0, 20.0));
    let foot = if v.buttons.is_empty() { 0.0 } else { 56.0 };
    let body = Rect::from_min_max(inner.min, pos2(inner.right(), inner.bottom() - foot));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::ScrollArea::vertical().id_salt("ksh-plan").auto_shrink([false; 2]).show(&mut child, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        let wdt = body.width() - 12.0;
        for l in &v.lines {
            match (l.label.is_empty(), l.field) {
                // A field: its label above, the dropdown with the value.
                (_, Some(id)) => {
                    if !l.label.is_empty() {
                        Text::new(&l.label, Font::LABEL).shrink(10.0).color(k.ink_muted).show(ui, vec2(wdt, 20.0));
                    }
                    let rows = v.fields.iter().find(|f| f.0 == id).map(|f| &f.1[..]).unwrap_or(&[]);
                    match field(ui, v, id, &l.value, rows, wdt) {
                        Some(Some(row)) => acts.push(Act::Pick(id, row)),
                        Some(None) => acts.push(Act::Click),
                        None => {}
                    }
                    ui.add_space(6.0);
                }
                (true, None) => {
                    Text::new(&l.value, Font::BODY.sized(17.0)).lines(3).color(k.ink).show(ui, vec2(wdt, 26.0 * l.value.len().div_ceil(56).clamp(1, 3) as f32));
                }
                (false, None) if l.value.is_empty() => {
                    Text::new(&l.label, Font::LABEL).shrink(10.0).color(k.ink_muted).show(ui, vec2(wdt, 20.0));
                }
                (false, None) => {
                    let (r, _) = ui.allocate_exact_size(vec2(wdt, 26.0), Sense::hover());
                    let base = ds::fit::baseline(r, Font::DATA.sized(17.0));
                    Text::new(&l.label, Font::LABEL).shrink(10.0).baseline(base).color(k.ink_muted).paint(ui, r.with_max_x(r.left() + wdt * 0.45));
                    Text::new(&l.value, Font::DATA.sized(17.0)).shrink(12.0).baseline(base).color(k.info).paint(ui, r.with_min_x(r.left() + wdt * 0.47));
                }
            }
        }
        if !v.answers.is_empty() {
            ui.add_space(10.0);
            for b in &v.answers {
                if ds::either(&ds::tip(ui.add_enabled(b.on, ds::Plate::new(&b.text).width(wdt).height(44.0)), ds::tip_text(&v.tips, b.id))) {
                    acts.push(Act::Press(b.id));
                }
            }
        }
    });
    // The buttons: the main one widest, the others to its right.
    let n = v.buttons.len() as f32;
    let gap = 10.0;
    let side = 126.0;
    let main = (inner.width() - (n - 1.0).max(0.0) * (side + gap)).max(side);
    let mut x = inner.left();
    for b in &v.buttons {
        let bw = if b.primary || n < 2.0 { main } else { side };
        let r = Rect::from_min_size(pos2(x, inner.bottom() - 44.0), vec2(bw, 44.0));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r));
        if ds::either(&ds::tip(c.add_enabled(b.on, ds::Plate::new(&b.text).width(bw).height(44.0).primary(b.primary)), ds::tip_text(&v.tips, b.id))) {
            if b.id == 3 {
                st.kill = true;
            } else {
                acts.push(Act::Press(b.id));
            }
        }
        x = r.right() + gap;
    }
}

/// A list field: the kit's select with the original's rows and the field's tooltip.
fn field(ui: &mut Ui, v: &View, id: u32, value: &str, rows: &[(i32, String, bool)], width: f32) -> Option<Option<i32>> {
    let k = ds::tokens();
    let (resp, out) = ds::select(ui, ("ksh-field", id), value, rows, vec2(width, 40.0), Font::BODY.sized(18.0), k.ink);
    ds::tip(resp, ds::tip_text(&v.tips, id));
    out
}

/// The panels' rows from the original's recorded texts (`SpyGfx2D_2`'s places, re/screens/spy.md §4): the left
/// column (x < 0x1d0) is the agent's card, the right one the planner or report; texts on a button's box are its
/// caption; texts on a list field's box are its value.
pub fn layout(pieces: &[Piece], spots: &[([i32; 4], u32)], dec: &dyn Fn(&[u8]) -> String) -> Laid {
    let inside = |p: &Piece, r: [i32; 4]| p.at[0] + 4 >= r[0] && p.at[0] < r[0] + r[2] && p.at[1] + 5 >= r[1] && p.at[1] < r[1] + r[3];
    let mut captions: Vec<(u32, String)> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut skills: Vec<(String, String)> = Vec::new();
    let mut points = None;
    let mut plus = false;
    let mut count = String::new();
    let mut by_y: Vec<&Piece> = pieces.iter().filter(|p| !p.text.is_empty()).collect();
    by_y.sort_by_key(|p| (p.at[1], p.at[0]));
    let mut i = 0;
    while i < by_y.len() {
        let y = by_y[i].at[1];
        let row: Vec<&Piece> = by_y[i..].iter().take_while(|p| (p.at[1] - y).abs() <= 2).copied().collect();
        i += row.len();
        let (left, right): (Vec<&Piece>, Vec<&Piece>) = row.into_iter().partition(|p| p.at[0] < 0x1d0);
        // The left column: four rows, the skills (with their "+"), "Skill Points:", the count at the foot.
        if !left.is_empty() {
            let texts: Vec<String> = left.iter().filter(|p| p.text != b"+").map(|p| dec(&p.text)).collect();
            plus |= left.iter().any(|p| p.text == b"+");
            match (y, &texts[..]) {
                (0x1bf.., [c]) => count = c.clone(),
                (0x140..=0x172, [l, v]) => skills.push((l.trim_end_matches(':').to_string(), v.clone())),
                (0x173.., [_, v]) => points = Some(v.clone()),
                (_, [l, v]) => rows.push((l.trim_end_matches(':').to_string(), v.clone())),
                _ => {}
            }
        }
        // The right column: captions, then label and value rows.
        let mut label = String::new();
        let mut value = Vec::new();
        let mut field = None;
        for p in right {
            if let Some(&(_, id)) = spots.iter().find(|(r, id)| inside(p, *r) && !(0x10..=0x13).contains(id)) {
                match captions.iter_mut().find(|c| c.0 == id) {
                    Some(c) => c.1 = format!("{} {}", c.1, dec(&p.text)),
                    None => captions.push((id, dec(&p.text))),
                }
                continue;
            }
            if let Some(&(_, id)) = spots.iter().find(|(r, id)| inside(p, *r) && (0x10..=0x13).contains(id)) {
                field = Some(id);
            }
            if p.col == 1 && label.is_empty() && value.is_empty() {
                label = dec(&p.text).trim_end_matches(':').to_string();
            } else {
                value.push(dec(&p.text));
            }
        }
        if !label.is_empty() || !value.is_empty() {
            lines.push(Line { label, value: value.join(" "), field });
        }
    }
    // A label alone, then its field on the next row: one field row.
    let mut merged: Vec<Line> = Vec::new();
    for l in lines {
        match merged.last_mut() {
            Some(m) if l.label.is_empty() && m.value.is_empty() && m.field.is_none() && l.field.is_some() => {
                m.value = l.value;
                m.field = l.field;
            }
            _ => merged.push(l),
        }
    }
    let agent = (!rows.is_empty()).then_some(Agent { rows, skills, plus, points: points.filter(|_| plus) });
    (agent, count, merged, captions)
}

/// What the modern spy screen shows this frame, from the classic screen's state and its recorded texts.
#[allow(clippy::too_many_arguments)]
pub fn view(h: &crate::ui::Hud, state: &ig2_sim::save::SaveState, rules: &ig2_rules::Rules, s: &crate::ui::UiSnap, spy: &crate::spy_screen::SpyView, d: &crate::spy_screen::SpyData, size: [f32; 2], gates: Option<&[bool; 20]>, time: f32) -> View {
    let lang = h.texts.lang;
    let dec = |b: &[u8]| ig2_formats::text::decode(b.split(|&c| c == 0).next().unwrap_or(&[]), lang).trim().to_string();
    let ours = |e: &str| dec(&h.texts.ours(e));
    let sc = &spy.screen;
    let (pieces, spots) = spy.capture(h, d, size, lang == ig2_formats::text::Lang::Hu);
    let hot = sc.hot_spots(d);
    let on = |id: u32| hot.iter().any(|&(i, _)| i == id);
    // The buttons' and fields' boxes, enabled or not (a caption shows on a greyed button too).
    let (button, ok) = ([475, 424, 60, 25], [565, 424, 60, 25]);
    let mut boxes: Vec<([i32; 4], u32)> = Vec::new();
    if sc.dialog == 0 {
        let left = [3, 4, 0xf].into_iter().find(|&id| on(id)).unwrap_or(if sc.plan.on { 0xf } else { 4 });
        boxes.extend([(button, left), (ok, 0xe)]);
        boxes.extend([([470, 269, 170, 16], 0x10), ([470, 303, 170, 16], 0x11), ([522, 320, 118, 16], 0x12), ([522, 337, 118, 16], 0x13)].into_iter().filter(|b| on(b.1)));
    } else {
        boxes.extend([(button, 0x21), (ok, 0x18)]);
        let stack: &[(i32, u32)] = match sc.dialog {
            2 => &[(298, 0x1e), (332, 0x1f), (366, 0x20)],
            3 => &[(298, 0x14), (332, 0x15), (366, 0x16), (400, 0x17)],
            _ => &[],
        };
        boxes.extend(stack.iter().map(|&(y, id)| ([475, y, 150, 25], id)));
    }
    // The names on the plates (`KemekFeliratai`, above y 0x100) apart from the panels' texts.
    let (names, panels): (Vec<Piece>, Vec<Piece>) = pieces.into_iter().partition(|p| p.at[1] < 0xf0);
    let (agent, count, lines, captions) = layout(&panels, &boxes, &dec);
    let stacked = |id: u32| (0x14..=0x17).contains(&id) || (0x1e..=0x20).contains(&id);
    // A report's buttons wait while its orders are on their way (`awaiting`, multiplayer).
    let waits = |id: u32| spy.awaiting() && matches!(id, 0x14..=0x18 | 0x1e..=0x21);
    let mk = |&(id, ref text): &(u32, String)| Button { id, text: text.clone(), on: on(id) && !waits(id), primary: matches!(id, 0xe | 0x18) };
    let mut buttons: Vec<Button> = captions.iter().filter(|c| !stacked(c.0)).map(mk).collect();
    buttons.sort_by_key(|b| !b.primary);
    let answers = captions.iter().filter(|c| stacked(c.0)).map(mk).collect();
    let peds = crate::spy_screen::SpyScreen::pedestals(d);
    let pedestals = spots
        .iter()
        .enumerate()
        .filter(|(_, sp)| sp.transparency < 0.8)
        .map(|(i, sp)| {
            // `KemekFeliratai` writes each plate's name at `plate − (40, 2)` in 14 cells.
            let piece = names.iter().find(|p| p.at == [sp.plate[0] as i32 - 40, sp.plate[1] as i32 - 2]);
            // `pick_spy`: only a pedestal in front (z > −880) takes the press.
            Pedestal { i, plate: sp.plate, figure: sp.screen, name: piece.map(|p| dec(&p.text)).unwrap_or_default(), alpha: piece.map_or(0.0, |p| (p.mult / 100.0).clamp(0.0, 1.0)), agent: peds[i].is_some() && sp.centre[2] > -880.0 }
        })
        .collect();
    let fields = [0x10, 0x11, 0x12, 0x13, 3].into_iter().filter(|&id| on(id)).map(|id| (id, spy.modern_rows(d, id).into_iter().map(|(v, t, off)| (v, if t.is_empty() { dec(&h.texts.get(7, 5, 7)) } else { dec(&t) }, off)).collect())).collect();
    View {
        bar: h.bar_view(state, rules, s, s.human as u8, gates, time, 7),
        live: sc.opened && !sc.closing,
        pedestals,
        arrows: sc.dialog == 0 && d.count() > 1,
        agent,
        count,
        lines,
        fields,
        buttons,
        answers,
        tips: spy.texts().tips.iter().map(|t| dec(t)).collect(),
        labels: Labels { points: ours("Skill points"), exit: ours("Exit") },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(s: &str, x: i32, y: i32, col: usize) -> Piece {
        Piece { text: s.as_bytes().to_vec(), at: [x, y], len: 14, flags: 4, col, mult: 100.0 }
    }

    /// The recorded texts become the card's rows, the skills, the planner's field rows and the buttons' captions.
    #[test]
    fn layout_rows() {
        let pieces = vec![
            piece("Name:", 0xc, 0x100, 1),
            piece("Agent", 0x3b, 0x100, 2),
            piece("Race:", 0xc, 0x110, 1),
            piece("Human", 0x3b, 0x110, 2),
            piece("Infiltration", 0xc, 0x142, 1),
            piece("40", 0x65, 0x142, 2),
            piece("+", 0x70, 0x140, 1),
            piece("Skill Points:", 0xc, 0x192, 1),
            piece("2", 0x65, 0x192, 2),
            piece("Spies: 5  Max: 2", 0xc, 0x1bf, 1),
            piece("Mission:", 0x1d6, 0x100, 1),
            piece("Empire info", 0x1de, 0x111, 2),
            piece("Days:", 0x1d6, 0x177, 1),
            piece("12 days", 0x230, 0x177, 2),
            piece("Send", 0x255, 0x1b1, 2),
            piece("Cancel", 0x1fb, 0x1b1, 2),
        ];
        let boxes = [([475, 424, 60, 25], 0xf), ([565, 424, 60, 25], 0xe), ([470, 269, 170, 16], 0x10)];
        let (agent, count, lines, captions) = layout(&pieces, &boxes, &|b| String::from_utf8_lossy(b).into_owned());
        let a = agent.unwrap();
        assert_eq!(a.rows, vec![("Name".into(), "Agent".into()), ("Race".into(), "Human".into())]);
        assert_eq!(a.skills, vec![("Infiltration".into(), "40".into())]);
        assert!(a.plus);
        assert_eq!(a.points.as_deref(), Some("2"));
        assert_eq!(count, "Spies: 5  Max: 2");
        assert_eq!(lines, vec![Line { label: "Mission".into(), value: "Empire info".into(), field: Some(0x10) }, Line { label: "Days".into(), value: "12 days".into(), field: None }]);
        assert_eq!(captions, vec![(0xf, "Cancel".into()), (0xe, "Send".into())]);
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
            rects = ["ksh-agent", "ksh-plan", "ksh-exit"].map(|id| gui.ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap()).to_vec();
        }
        (rects, acts)
    }

    /// Long texts never move a panel; the main button presses its hot-spot, the exit leaves.
    #[test]
    fn panels_and_presses() {
        let view = |s: &str| View {
            live: true,
            agent: Some(Agent { rows: vec![(s.into(), s.into()); 4], skills: vec![(s.into(), "40".into()); 4], plus: true, points: Some("2".into()) }),
            count: s.into(),
            lines: vec![Line { label: s.into(), value: s.into(), field: None }],
            buttons: vec![Button { id: 0xe, text: s.into(), on: true, primary: true }, Button { id: 0xf, text: s.into(), on: true, primary: false }],
            labels: Labels { exit: s.into(), points: s.into() },
            ..Default::default()
        };
        let (short, _) = run(&view("Tra"), None);
        let (long, _) = run(&view(&"Kémelhárítási jelentés ".repeat(10)), None);
        assert_eq!(short, long);
        let plan = short[1];
        let (_, a) = run(&view("Tra"), Some(pos2(plan.left() + 60.0, plan.bottom() - 20.0 - 22.0)));
        assert!(a.contains(&Act::Press(0xe)), "{a:?}");
        let (_, a) = run(&view("Tra"), Some(short[2].center()));
        assert!(a.contains(&Act::Bar(sm::Act::Screen(2))), "{a:?}");
        let agent = short[0];
        let (_, a) = run(&view("Tra"), Some(pos2(agent.right() - 28.0 - 20.0, agent.top() + 20.0 + 4.0 * 26.0 + 20.0 + 20.0)));
        assert!(a.contains(&Act::Press(0x19)), "{a:?}");
    }
}
