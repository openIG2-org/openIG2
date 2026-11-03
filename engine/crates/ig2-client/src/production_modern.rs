//! Ours (ADR 0071): the starmap's production view (re/screens/starmap.md §12) as modern panels on the egui layer, under
//! the modern starmap HUD (ADR 0065). The same functions as the original's view: the planet with the previous and
//! next of the player's planets and the planet list, per line (ships, vehicles) the capacity, the head unit's bar and
//! the four slots with their design, count, price, repeat and "into a fleet" lamps, the count buttons (held: they
//! repeat as the original's), the slot's design list (design 0 removes) and the exit. The window fills a [`View`]
//! from the state `StarmapGfx2D_3` reads, runs [`show`] after the HUD and hands the [`Act`]s to the original's panel
//! code (`starmap_panels::Panels::press_id`, `hold_frame`, `release`, `modern_pick`).

use egui::{pos2, vec2, Align2, Color32, FontId, Margin, Rect, RichText, Sense, Stroke};

use crate::starmap_modern::{self as sm, icon_button, lerp, plate, tip, Ink, EDGE};

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
    pub count: i32,
    pub price: String,
    pub repeat: bool,
    /// The vehicles' "into a fleet" lamp.
    pub into_fleet: Option<bool>,
    /// A station (`+0x37`): no count buttons.
    pub station: bool,
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
    /// The design list: design 0 (remove) and the player's visible designs of the line's class, with a station's
    /// ninth dead.
    pub designs: Vec<(usize, String, bool)>,
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
}

/// What stays between frames: the slot whose design list is open.
#[derive(Debug, Clone, Default)]
pub struct State {
    list: Option<(usize, usize)>,
}

const LINE_W: f32 = 250.0;
const BTN: f32 = 22.0;

/// One frame of the production view (after the HUD's, under its bar, in the empire colour `race`); the acts in the
/// order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State, race: Color32) -> Vec<Act> {
    let ink = &Ink::new(ctx, race);
    let top = ctx.memory(|m| m.area_rect(egui::Id::new("smh-left"))).map_or(46.0, |r| r.bottom() + 6.0);
    let mut acts = Vec::new();
    let mut held = None;
    egui::Area::new(egui::Id::new("pmh-planet")).anchor(Align2::CENTER_TOP, vec2(0.0, top)).fade_in(false).show(ctx, |ui| {
        plate(ui, ink, Margin::symmetric(10, 6), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if sm::either(&tip(icon_button(ui, ink, sm::Icon::RotateLeft, false, true, BTN), &v.tips[0], "", ink)) {
                    acts.push(Act::Spot(0xc));
                }
                let name = ui.add(egui::Label::new(RichText::new(&v.planet).size(16.0).strong().color(Color32::WHITE)).sense(Sense::click()));
                // The planet list opens on the press and picks on the release, as the original's (hot-spot 0x32).
                if tip(name, &v.tips[7], "", ink).is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed()) {
                    acts.push(Act::Spot(0x32));
                }
                if sm::either(&tip(icon_button(ui, ink, sm::Icon::RotateRight, false, true, BTN), &v.tips[1], "", ink)) {
                    acts.push(Act::Spot(0xd));
                }
                ui.add_space(10.0);
                let exit = ui.add_enabled(!v.exit_locked, egui::Button::new(RichText::new(&v.exit).size(12.5)));
                if tip(exit, &v.tips[8], "", ink).clicked() {
                    acts.push(Act::Spot(9));
                }
            });
        });
    });
    for (l, line) in v.lines.iter().enumerate() {
        let (align, x) = if l == 0 { (Align2::LEFT_TOP, EDGE) } else { (Align2::RIGHT_TOP, -EDGE) };
        egui::Area::new(egui::Id::new(("pmh-line", l))).anchor(align, vec2(x, top)).fade_in(false).show(ctx, |ui| {
            plate(ui, ink, Margin::symmetric(10, 8), |ui| line_panel(ui, l, line, v, st, ink, &mut acts, &mut held));
        });
    }
    // The open design list beside its slot.
    if let Some((l, k)) = st.list {
        let line = &v.lines[l];
        let anchor = ctx.memory(|m| m.area_rect(egui::Id::new(("pmh-line", l))));
        let slot = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("pmh-slot", l, k))));
        let mut close = line.slots[k].is_none() && line.designs.is_empty();
        if let (Some(a), Some(s)) = (anchor, slot) {
            let (pivot, at) = if l == 0 { (Align2::LEFT_TOP, pos2(a.right() + 6.0, s.top())) } else { (Align2::RIGHT_TOP, pos2(a.left() - 6.0, s.top())) };
            let r = egui::Area::new(egui::Id::new("pmh-list")).pivot(pivot).fixed_pos(at).order(egui::Order::Foreground).constrain(true).fade_in(false).show(ctx, |ui| {
                plate(ui, ink, Margin::symmetric(8, 6), |ui| {
                    ui.set_min_width(170.0);
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (d, name, ok) in &line.designs {
                            let (r, resp) = ui.allocate_exact_size(vec2(170.0, 19.0), if *ok { Sense::click() } else { Sense::hover() });
                            let hot = ui.ctx().animate_bool_with_time(resp.id, *ok && resp.hovered(), 0.1);
                            let p = ui.painter();
                            if hot > 0.0 {
                                p.rect_filled(r, 2.0, Color32::from_white_alpha((16.0 * hot) as u8));
                            }
                            let c = if !ok { ink.dim.gamma_multiply(0.6) } else if *d == 0 { ink.accent } else { lerp(ink.text, Color32::WHITE, hot) };
                            p.with_clip_rect(r).text(pos2(r.left() + 6.0, r.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.5), c);
                            if resp.clicked() {
                                acts.push(Act::Pick(l, k, *d));
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
        }
    }
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("pmh-opened"), false));
    acts.push(Act::Hold(held));
    acts
}

/// A line's capacity and head bar, then its four slots.
#[allow(clippy::too_many_arguments)]
fn line_panel(ui: &mut egui::Ui, l: usize, line: &Line, v: &View, st: &mut State, ink: &Ink, acts: &mut Vec<Act>, held: &mut Option<(usize, usize, bool)>) {
    ui.set_width(LINE_W);
    ui.spacing_mut().item_spacing.y = 3.0;
    ui.horizontal(|ui| {
        ui.label(RichText::new(line.title.to_uppercase()).size(11.0).strong().color(ink.accent));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let c = if line.working { Color32::WHITE } else { ink.dim };
            ui.label(RichText::new(&line.capacity.1).size(13.0).strong().color(c));
            ui.label(RichText::new(&line.capacity.0).size(11.0).color(ink.dim));
        });
    });
    // The head unit's bar: the accent while it builds, dim without capacity.
    let (r, _) = ui.allocate_exact_size(vec2(LINE_W, 4.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 1.0, Color32::from_black_alpha(150));
    if let Some(f) = line.progress {
        let c = if line.working { ink.accent } else { ink.dim };
        p.rect_filled(Rect::from_min_size(r.min, vec2(r.width() * f.clamp(0.0, 1.0), r.height())), 1.0, c);
    }
    ui.add_space(2.0);
    for (k, s) in line.slots.iter().enumerate() {
        let resp = ui.horizontal(|ui| {
            ui.set_min_height(BTN + 4.0);
            ui.spacing_mut().item_spacing.x = 3.0;
            // The design (a click opens the list).
            let open = st.list == Some((l, k));
            let (name, price) = s.as_ref().map_or(("—", ""), |s| (s.name.as_str(), s.price.as_str()));
            let (r, resp) = ui.allocate_exact_size(vec2(LINE_W - 3.0 * (BTN + 3.0) - 44.0, BTN + 4.0), Sense::click());
            let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);
            let p = ui.painter();
            p.rect_filled(r, 2.0, if open { ink.accent.gamma_multiply(0.2) } else { Color32::from_white_alpha((8.0 + 10.0 * hot) as u8) });
            if k == 0 && s.is_some() {
                p.rect_filled(Rect::from_min_max(r.min, pos2(r.left() + 2.0, r.bottom())), 0.0, ink.accent);
            }
            let c = if s.is_some() { lerp(ink.text, Color32::WHITE, hot.max(open as u8 as f32)) } else { ink.dim };
            p.with_clip_rect(r.shrink(2.0)).text(pos2(r.left() + 6.0, r.top() + 2.0), Align2::LEFT_TOP, name, FontId::proportional(12.5), c);
            p.text(pos2(r.left() + 6.0, r.bottom() - 2.0), Align2::LEFT_BOTTOM, price, FontId::proportional(10.5), ink.accent);
            if tip(resp, &v.tips[6], "", ink).clicked() {
                st.list = if open { None } else { Some((l, k)) };
                ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("pmh-opened"), true));
            }
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("pmh-slot", l, k)), r));
            let Some(s) = s else { return };
            // The count and its buttons (held: the original's repeat; none for stations).
            let minus = ui.add_enabled_ui(!s.station, |ui| tip(count_button(ui, ink, false), &v.tips[4], "", ink)).inner;
            let (cr, _) = ui.allocate_exact_size(vec2(34.0, BTN), Sense::hover());
            ui.painter().text(cr.center(), Align2::CENTER_CENTER, s.count.to_string(), FontId::proportional(14.0), Color32::WHITE);
            let plus = ui.add_enabled_ui(!s.station, |ui| tip(count_button(ui, ink, true), &v.tips[5], "", ink)).inner;
            if minus.is_pointer_button_down_on() {
                *held = Some((l, k, false));
            } else if plus.is_pointer_button_down_on() {
                *held = Some((l, k, true));
            }
            ui.add_space(4.0);
            if sm::either(&tip(lamp(ui, ink, "R", s.repeat), &v.tips[2], "", ink)) {
                acts.push(Act::Spot(0xe + 4 * l as u32 + k as u32));
            }
            if let Some(f) = s.into_fleet {
                if sm::either(&tip(lamp(ui, ink, "F", f), &v.tips[3], "", ink)) {
                    acts.push(Act::Spot(0x16 + k as u32));
                }
            }
        });
        let _ = resp;
    }
}

/// A − or + button.
fn count_button(ui: &mut egui::Ui, ink: &Ink, plus: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(BTN, BTN), Sense::click_and_drag());
    let on = ui.is_enabled();
    let hot = ui.ctx().animate_bool_with_time(resp.id, on && resp.hovered(), 0.12);
    let p = ui.painter();
    if on && resp.is_pointer_button_down_on() {
        p.rect_filled(r, 2.0, ink.accent.gamma_multiply(0.35));
    } else if hot > 0.0 {
        p.rect_filled(r, 2.0, Color32::from_white_alpha((18.0 * hot) as u8));
    }
    p.rect_stroke(r, 2.0, Stroke::new(1.0f32, ink.edge), egui::StrokeKind::Inside);
    let c = if on { lerp(ink.text, Color32::WHITE, hot) } else { ink.dim.gamma_multiply(0.6) };
    let m = r.center();
    let k = r.width() * 0.25;
    p.line_segment([pos2(m.x - k, m.y), pos2(m.x + k, m.y)], Stroke::new(1.8f32, c));
    if plus {
        p.line_segment([pos2(m.x, m.y - k), pos2(m.x, m.y + k)], Stroke::new(1.8f32, c));
    }
    resp
}

/// A lamp toggle: lit when on.
fn lamp(ui: &mut egui::Ui, ink: &Ink, letter: &str, on: bool) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(BTN, BTN), Sense::click());
    let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let p = ui.painter();
    p.rect_filled(r, 2.0, if on { ink.accent.gamma_multiply(0.3) } else { Color32::from_white_alpha((8.0 + 12.0 * hot) as u8) });
    p.rect_stroke(r, 2.0, Stroke::new(1.0f32, if on { ink.accent } else { ink.edge }), egui::StrokeKind::Inside);
    let c = if on { Color32::WHITE } else { lerp(ink.dim, ink.text, hot) };
    p.text(r.center(), Align2::CENTER_CENTER, letter, FontId::proportional(11.5), c);
    resp
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

    fn view() -> View {
        let slot = Slot { name: "Fighter".into(), count: 5, price: "$500".into(), repeat: true, into_fleet: None, station: false };
        let mut v = View { planet: "Magellan 10".into(), exit: "Exit".into(), ..Default::default() };
        v.lines[0] = Line { title: "Ships".into(), capacity: ("Capacity".into(), "2".into()), working: true, progress: Some(0.5), slots: [Some(slot.clone()), None, None, None], designs: vec![(0, "Remove".into(), true), (3, "Fighter".into(), true)] };
        v.lines[1].slots[0] = Some(Slot { into_fleet: Some(false), ..slot });
        v
    }

    #[test]
    fn the_panels_draw_and_ask_for_nothing_untouched() {
        let ctx = egui::Context::default();
        let v = view();
        let mut st = State::default();
        let mut last = Vec::new();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 720.0))), ..Default::default() }, |ctx| last = show(ctx, &v, &mut st, Color32::from_rgb(200, 60, 60)));
        }
        assert_eq!(last, vec![Act::Hold(None)]);
        assert!(ctx.memory(|m| m.area_rect(egui::Id::new(("pmh-line", 1)))).is_some_and(|r| r.right() <= 1280.0));
    }
}
