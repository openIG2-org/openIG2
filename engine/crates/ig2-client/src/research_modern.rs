//! Ours (ADR 0071): the research screen (`AktKep` 4, re/screens/research.md) as modern panels on the egui layer, the
//! "Modern research screen" option. The same functions as the original's screen without its 3D console: the twelve
//! categories (the wheels' buttons), the category's known techs in their status with the progress per cent (the
//! lists), the shown tech's info lines, prerequisites, labs needed against the player's, days and daily cost (the info
//! box and the panels' texts), start or stop (hot-spot 1), the running project with its money and days left and the
//! jump to it (hot-spot 0x19), the urgent button (hot-spot 4: double money, double speed) and the auto-research
//! switches (§5.8). The window fills a [`View`] from what `ResearchGfx2D_2` reads, runs [`show`] once a frame and hands
//! the [`Act`]s to [`crate::research::Research::modern_act`], which runs the original's code; the keys stay the
//! screen's.

use egui::{pos2, vec2, Align2, Color32, FontId, Margin, Rect, RichText, Sense, Stroke};

use crate::starmap_modern::{self as sm, lerp, plate, tip, Ink, EDGE};

/// What the panels ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// The top bar's (the starmap HUD's acts).
    Bar(sm::Act),
    /// A category chosen (`TT_*`).
    Area(usize),
    /// A field of the shown category chosen (`TalalmanyKivalasztva`).
    Field(i32),
    /// Start or stop (`ResearchStartStop`).
    StartStop,
    /// The urgent button (`ResearchDoubleGomb`).
    Urgent,
    /// The running project shown (hot-spot 0x19).
    Current,
    /// An auto-research switch flipped (0 ships, 1 buildings, 2 tanks).
    Auto(usize),
}

/// A category.
#[derive(Debug, Clone, Default)]
pub struct Area {
    pub n: usize,
    pub name: String,
    /// Known techs (status ≥ 2) and the owned ones.
    pub known: usize,
    pub owned: usize,
    /// The running project is in it.
    pub running: bool,
}

/// One row of the category's list.
#[derive(Debug, Clone, Default)]
pub struct Tech {
    pub field: i32,
    pub name: String,
    /// 2 seen, 3 researchable, 4 researching, 5 owned.
    pub status: u8,
    /// The progress per cent (0 or 100 shows none).
    pub pct: u32,
}

/// The shown tech.
#[derive(Debug, Clone, Default)]
pub struct Detail {
    pub name: String,
    pub status: u8,
    pub info: Vec<String>,
    /// "Required: …".
    pub need: String,
    /// Per lab (ships, tanks, buildings): needed, the player's, the tooltips (required, operational).
    pub labs: Vec<(i32, i32, String, String)>,
    /// Days required and the cost, unless owned.
    pub days: Option<String>,
    pub cost: Option<String>,
}

/// The running project.
#[derive(Debug, Clone, Default)]
pub struct Current {
    pub name: String,
    pub money: String,
    pub days: String,
    pub part: f32,
}

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    pub areas: Vec<Area>,
    pub area: usize,
    pub techs: Vec<Tech>,
    pub field: i32,
    pub detail: Option<Detail>,
    pub current: Option<Current>,
    /// Start (`false`) or stop (`true`) and whether it acts now.
    pub stop: bool,
    pub start_on: bool,
    pub urgent: bool,
    pub auto: [(String, bool); 3],
    /// Labels: start, stop, the running project's jump, the info box's title.
    pub labels: [String; 4],
    /// Tooltips: start/stop, the jump, urgent, the auto switches.
    pub tips: [String; 6],
}

/// What stays between frames.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub bar: sm::State,
}

const AREA_W: f32 = 210.0;
const LIST_W: f32 = 300.0;
const INFO_W: f32 = 330.0;

/// The rows' colours by status: owned, researching, researchable, seen.
fn status_ink(ink: &Ink, s: u8) -> Color32 {
    match s {
        5 => lerp(ink.text, Color32::from_rgb(110, 205, 120), 0.5),
        4 => lerp(ink.accent, Color32::WHITE, 0.25),
        3 => Color32::WHITE,
        _ => ink.dim,
    }
}

/// One frame of the screen; the acts in the order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut bar = Vec::new();
    let ink = sm::bar(ctx, &v.bar, &mut st.bar, &mut bar);
    let mut acts: Vec<Act> = bar.into_iter().map(Act::Bar).collect();
    let top = ctx.memory(|m| m.area_rect(egui::Id::new("smh-left"))).map_or(46.0, |r| r.bottom() + 6.0);
    let screen = ctx.screen_rect();
    let total = AREA_W + LIST_W + INFO_W + 2.0 * 14.0 + 2.0 * 8.0 + 2.0 * 6.0;
    let left = ((screen.width() - total) / 2.0).max(EDGE);
    let height = (screen.height() - top - EDGE).clamp(200.0, 640.0);
    let a = egui::Area::new(egui::Id::new("rmh-areas")).fixed_pos(pos2(left, top)).fade_in(false).show(ctx, |ui| {
        plate(ui, &ink, Margin::symmetric(7, 7), |ui| areas(ui, v, &ink, &mut acts));
    });
    let l = egui::Area::new(egui::Id::new("rmh-list")).fixed_pos(pos2(a.response.rect.right() + 6.0, top)).fade_in(false).show(ctx, |ui| {
        plate(ui, &ink, Margin::symmetric(7, 7), |ui| list(ui, v, &ink, height, &mut acts));
    });
    egui::Area::new(egui::Id::new("rmh-info")).fixed_pos(pos2(l.response.rect.right() + 6.0, top)).fade_in(false).show(ctx, |ui| {
        plate(ui, &ink, Margin::symmetric(12, 9), |ui| info(ui, v, &ink, &mut acts));
    });
    acts
}

/// The twelve categories, the ships', the buildings', the tanks' and the special one apart.
fn areas(ui: &mut egui::Ui, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    ui.set_width(AREA_W);
    ui.spacing_mut().item_spacing.y = 0.0;
    for (i, a) in v.areas.iter().enumerate() {
        if i > 0 && matches!(a.n, 6 | 10 | 12) {
            ui.add_space(4.0);
            let (r, _) = ui.allocate_exact_size(vec2(AREA_W, 1.0), Sense::hover());
            ui.painter().rect_filled(r, 0.0, ink.edge);
            ui.add_space(4.0);
        }
        let on = a.n == v.area;
        let (r, resp) = ui.allocate_exact_size(vec2(AREA_W, 26.0), Sense::click());
        let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
        let p = ui.painter();
        if on {
            p.rect_filled(r, 2.0, ink.accent.gamma_multiply(0.2));
            p.rect_filled(Rect::from_min_max(r.min, pos2(r.left() + 2.0, r.bottom())), 0.0, ink.accent);
        } else if hot > 0.0 {
            p.rect_filled(r, 2.0, Color32::from_white_alpha((14.0 * hot) as u8));
        }
        let c = if on { Color32::WHITE } else { lerp(ink.text.gamma_multiply(0.85), ink.text, hot) };
        p.with_clip_rect(Rect::from_min_max(r.min, pos2(r.right() - 40.0, r.bottom()))).text(pos2(r.left() + 9.0, r.center().y), Align2::LEFT_CENTER, &a.name, FontId::proportional(13.0), c);
        p.text(pos2(r.right() - 6.0, r.center().y), Align2::RIGHT_CENTER, format!("{}/{}", a.owned, a.known), FontId::proportional(10.5), ink.dim);
        if a.running {
            p.circle_filled(pos2(r.right() - 44.0, r.center().y), 3.0, ink.accent);
        }
        if resp.clicked() {
            acts.push(Act::Area(a.n));
        }
    }
}

/// The category's known techs with their status and per cent.
fn list(ui: &mut egui::Ui, v: &View, ink: &Ink, height: f32, acts: &mut Vec<Act>) {
    ui.set_width(LIST_W);
    ui.set_min_height(height.min(16.0 * 26.0));
    let name = v.areas.iter().find(|a| a.n == v.area).map_or("", |a| a.name.as_str());
    ui.label(RichText::new(name.to_uppercase()).size(11.0).strong().color(ink.accent));
    ui.add_space(3.0);
    egui::ScrollArea::vertical().max_height(height - 24.0).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for t in &v.techs {
            let on = t.field == v.field;
            let (r, resp) = ui.allocate_exact_size(vec2(LIST_W, 26.0), Sense::click());
            let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
            let p = ui.painter();
            if on {
                p.rect_filled(r, 2.0, ink.accent.gamma_multiply(0.2));
                p.rect_stroke(r, 2.0, Stroke::new(1.0f32, ink.accent.gamma_multiply(0.7)), egui::StrokeKind::Inside);
            } else if hot > 0.0 {
                p.rect_filled(r, 2.0, Color32::from_white_alpha((14.0 * hot) as u8));
            }
            let c = lerp(status_ink(ink, t.status), Color32::WHITE, hot * 0.4);
            p.with_clip_rect(Rect::from_min_max(r.min, pos2(r.right() - 44.0, r.bottom()))).text(pos2(r.left() + 9.0, r.center().y), Align2::LEFT_CENTER, &t.name, FontId::proportional(13.0), c);
            if t.status == 5 {
                let m = pos2(r.right() - 14.0, r.center().y);
                p.line_segment([pos2(m.x - 4.0, m.y), pos2(m.x - 1.0, m.y + 3.0)], Stroke::new(1.6f32, c));
                p.line_segment([pos2(m.x - 1.0, m.y + 3.0), pos2(m.x + 5.0, m.y - 4.0)], Stroke::new(1.6f32, c));
            } else if t.pct > 0 && t.pct < 100 {
                p.text(pos2(r.right() - 8.0, r.center().y), Align2::RIGHT_CENTER, format!("{}%", t.pct), FontId::proportional(11.5), c);
                let b = Rect::from_min_size(pos2(r.left() + 9.0, r.bottom() - 4.0), vec2(LIST_W - 18.0, 2.0));
                p.rect_filled(b, 0.0, Color32::from_black_alpha(140));
                p.rect_filled(Rect::from_min_size(b.min, vec2(b.width() * t.pct as f32 / 100.0, 2.0)), 0.0, c);
            }
            if resp.clicked() {
                acts.push(Act::Field(t.field));
            }
        }
    });
}

/// The shown tech, start/stop, the running project, urgent and the auto switches.
fn info(ui: &mut egui::Ui, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    ui.set_width(INFO_W);
    ui.spacing_mut().item_spacing.y = 4.0;
    if let Some(d) = &v.detail {
        ui.add(egui::Label::new(RichText::new(&d.name).size(17.0).strong().color(status_ink(ink, d.status).max_white())).truncate());
        for l in &d.info {
            ui.label(RichText::new(l).size(12.5).color(ink.text));
        }
        ui.label(RichText::new(&d.need).size(11.5).color(ink.dim));
        if !d.labs.is_empty() {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                for (i, (need, have, tip_need, tip_have)) in d.labs.iter().enumerate() {
                    let short = *have < *need;
                    let resp = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        lab_mark(ui, ink, i);
                        let c = if short { Color32::from_rgb(235, 96, 84) } else { Color32::WHITE };
                        ui.label(RichText::new(format!("{have} / {need}")).size(12.5).strong().color(c));
                    }).response;
                    tip(resp, &format!("{tip_need}: {need}\n{tip_have}: {have}"), "", ink);
                }
            });
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            for s in [&d.days, &d.cost].into_iter().flatten() {
                ui.label(RichText::new(s).size(12.0).color(ink.accent));
            }
        });
    }
    ui.add_space(4.0);
    let label = if v.stop { &v.labels[1] } else { &v.labels[0] };
    let b = ui.add_enabled(v.start_on, egui::Button::new(RichText::new(label).size(13.5).strong()).min_size(vec2(INFO_W, 28.0)));
    if tip(b, &v.tips[0], "", ink).clicked() {
        acts.push(Act::StartStop);
    }
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(INFO_W, 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, ink.edge);
    ui.add_space(2.0);
    if let Some(c) = &v.current {
        let resp = ui.horizontal(|ui| {
            ui.add(egui::Label::new(RichText::new(&c.name).size(14.0).strong().color(lerp(ink.accent, Color32::WHITE, 0.3))).truncate().sense(Sense::click()))
        });
        if tip(resp.inner, &v.tips[1], "", ink).clicked() {
            acts.push(Act::Current);
        }
        let (r, _) = ui.allocate_exact_size(vec2(INFO_W, 4.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(r, 1.0, Color32::from_black_alpha(150));
        p.rect_filled(Rect::from_min_size(r.min, vec2(r.width() * c.part.clamp(0.0, 1.0), 4.0)), 1.0, ink.accent);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            ui.label(RichText::new(&c.money).size(12.0).color(ink.text));
            ui.label(RichText::new(&c.days).size(12.0).color(ink.text));
        });
    } else {
        ui.label(RichText::new(&v.labels[3]).size(12.5).color(ink.dim));
    }
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if tip(toggle(ui, ink, "×2", v.urgent, 44.0), &v.tips[2], "", ink).clicked() {
            acts.push(Act::Urgent);
        }
        for (i, (name, on)) in v.auto.iter().enumerate() {
            if tip(toggle(ui, ink, name, *on, (INFO_W - 44.0 - 18.0) / 3.0), &v.tips[3 + i], "", ink).clicked() {
                acts.push(Act::Auto(i));
            }
        }
    });
}

/// A lab's mark: a flask with the lab's letter-free shape (ships, tanks, buildings: one, two, three bars).
fn lab_mark(ui: &mut egui::Ui, ink: &Ink, i: usize) {
    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
    let p = ui.painter();
    let s = Stroke::new(1.3f32, ink.accent);
    let m = r.center();
    p.add(egui::Shape::closed_line(vec![pos2(m.x - 2.0, r.top() + 1.0), pos2(m.x + 2.0, r.top() + 1.0), pos2(m.x + 2.0, m.y - 1.0), pos2(r.right() - 1.0, r.bottom() - 1.0), pos2(r.left() + 1.0, r.bottom() - 1.0), pos2(m.x - 2.0, m.y - 1.0)], s));
    for j in 0..=i {
        let y = r.bottom() - 3.0 - 2.0 * j as f32;
        p.line_segment([pos2(m.x - 3.0 + j as f32, y), pos2(m.x + 3.0 - j as f32, y)], s);
    }
}

/// A labelled switch, lit when on.
fn toggle(ui: &mut egui::Ui, ink: &Ink, label: &str, on: bool, w: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(w, 26.0), Sense::click());
    let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let p = ui.painter();
    p.rect_filled(r, 2.0, if on { ink.accent.gamma_multiply(0.3) } else { Color32::from_white_alpha((8.0 + 12.0 * hot) as u8) });
    p.rect_stroke(r, 2.0, Stroke::new(1.0f32, if on { ink.accent } else { ink.edge }), egui::StrokeKind::Inside);
    let c = if on { Color32::WHITE } else { lerp(ink.dim, ink.text, hot) };
    let mut g = p.layout_no_wrap(label.to_string(), FontId::proportional(11.5), c);
    if g.size().x > w - 6.0 {
        g = p.layout_no_wrap(label.to_string(), FontId::proportional(11.5 * (w - 6.0) / g.size().x), c);
    }
    p.galley(r.center() - g.size() / 2.0, g, c);
    resp
}

trait MaxWhite {
    fn max_white(self) -> Color32;
}

impl MaxWhite for Color32 {
    /// A dim (seen) title still reads as a title.
    fn max_white(self) -> Color32 {
        lerp(self, Color32::WHITE, 0.3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(v: &View, st: &mut State, events: Vec<egui::Event>) -> (egui::Context, Vec<Act>) {
        let ctx = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb).ctx;
        let mut acts = Vec::new();
        for i in 0..4 {
            let ev = if i == 3 { events.clone() } else { Vec::new() };
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1080.0))), events: ev, ..Default::default() };
            let _ = ctx.run(input, |ctx| acts = show(ctx, v, st));
        }
        (ctx, acts)
    }

    fn view() -> View {
        View {
            areas: (1..=12).map(|n| Area { n, name: format!("Area {n}"), known: 3, owned: 1, running: n == 2 }).collect(),
            area: 2,
            techs: vec![Tech { field: 1, name: "Laser".into(), status: 5, pct: 100 }, Tech { field: 2, name: "Ion".into(), status: 4, pct: 40 }],
            field: 2,
            detail: Some(Detail { name: "Ion".into(), status: 4, info: vec!["A gun".into()], need: "Required: Laser".into(), labs: vec![(1, 2, "need".into(), "have".into())], days: Some("Days: 4".into()), cost: Some("Cost: 9".into()) }),
            stop: true,
            start_on: true,
            ..Default::default()
        }
    }

    #[test]
    fn the_three_panels_sit_side_by_side_under_the_bar() {
        let (ctx, acts) = run(&view(), &mut State::default(), Vec::new());
        assert!(acts.is_empty());
        let r = |id: &str| ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap();
        let (a, l, i) = (r("rmh-areas"), r("rmh-list"), r("rmh-info"));
        assert!(a.right() < l.left() && l.right() < i.left() && i.right() <= 1920.0);
        assert_eq!(a.top(), l.top());
    }

    #[test]
    fn a_click_on_a_category_and_a_row_asks_for_them() {
        let v = view();
        let mut st = State::default();
        let (ctx, _) = run(&v, &mut st, Vec::new());
        let a = ctx.memory(|m| m.area_rect(egui::Id::new("rmh-areas"))).unwrap();
        // The fourth category's row: 7 margin, 26 a row, none of the gaps before category 6.
        let at = pos2(a.left() + 40.0, a.top() + 7.0 + 26.0 * 3.0 + 13.0);
        let click = |p: egui::Pos2| vec![egui::Event::PointerMoved(p), egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }, egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }];
        let ctx2 = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb).ctx;
        let mut acts = Vec::new();
        for ev in [Vec::new(), Vec::new(), click(at)] {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1080.0))), events: ev, ..Default::default() };
            let _ = ctx2.run(input, |ctx| acts = show(ctx, &v, &mut st));
        }
        assert_eq!(acts, vec![Act::Area(4)]);
    }
}
