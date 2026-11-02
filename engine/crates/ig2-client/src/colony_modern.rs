//! Ours (ADR 0070): the modern colony screen on the egui layer (ADR 0062), the "Modern colony screen" option. The
//! same functions as the original's 2D parts (re/screens/colony.md §6, §10, §11): the top bar (the modern starmap
//! HUD's, [`crate::starmap_modern::bar`]), the build menu with its costs and the build cursor, the short info (the
//! planet, the selected building or units) with the selected building's buttons, the radar, the ground war's buttons
//! with their charge, the battle's sides, the attack banner, and the repair, demolish and self-destruct windows. The 3D
//! view, its keys and its clicks stay the original's. The window fills a [`View`] from the state the classic panels
//! read, runs [`show`] once a frame and hands the [`Act`]s to [`crate::colony_ui::ColonyUi::modern_act`], which runs
//! the classic hot-spots' own code. The planet info mode keeps the original's long info.
//!
//! The look is the modern starmap HUD's console (ADR 0065): dark plates with cut corners and the empire's colour.

use egui::{pos2, vec2, Align2, Color32, FontId, Margin, Pos2, Rect, RichText, Sense, Shape, Stroke};

use crate::starmap_modern::{self as sm, icon_button, lerp, plate, tip, Ink, EDGE};

/// What the panels ask for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// The top bar's (the starmap HUD's acts).
    Bar(sm::Act),
    /// A button's hot-spot pressed: 1 repair, 2 demolish, 0x11 on/off, 10 self-destruct, 8 call it off, 0x13 build,
    /// the war's 0xb..=0x12.
    Spot(u32),
    /// The short info (hot-spot 4) clicked: left the planet info mode, right (`true`) the starmap.
    Card(bool),
    /// The radar pressed (`true`) or dragged at a point 0..1 across it (hot-spot 5).
    Radar([f32; 2], bool),
    /// A build menu entry picked (its tech), or −1: the build cursor toggled for the current kind.
    Pick(i32),
    /// The repair or demolish window answered.
    Answer(bool),
}

/// One build menu entry.
#[derive(Debug, Clone, Default)]
pub struct Entry {
    pub tech: i32,
    pub column: usize,
    pub name: String,
    pub count: i32,
    pub price: String,
    /// Pickable (a working hub or the hub itself, not under the tutorial's lock).
    pub ok: bool,
    /// The current kind (`AktEpuletTipus`).
    pub current: bool,
}

/// The build menu (hot-spot 6's pick list and the planet panel's building).
#[derive(Debug, Clone, Default)]
pub struct Build {
    /// `Epulet_Terulet_Nev[1..=4]`.
    pub headers: Vec<String>,
    pub entries: Vec<Entry>,
    /// The current kind: name, price, energy, workers.
    pub current: Option<(String, String, i32, i32)>,
    pub cursor: bool,
    /// Build orders on their way (`NemSzabadEpiteni`): the list waits.
    pub pending: bool,
    /// The "Build" button's tooltip and the cursor's.
    pub build_tip: String,
    pub cursor_tip: String,
}

/// The short info: the planet's, the selected building's or the selected units'.
#[derive(Debug, Clone, Default)]
pub struct Card {
    pub title: String,
    pub colour: Color32,
    pub lines: Vec<(String, Color32)>,
    /// The product line: its label, the icon byte (1 workers, 2 energy, 3 money) and the amount.
    pub product: Option<(String, u8, i32)>,
    /// Bars: label, the part done 0..1.
    pub bars: Vec<(String, f32)>,
    /// Hot-spot 4's tooltip.
    pub tip: String,
    /// Workers' and energy's surplus (EN's planet at peace).
    pub surplus: Option<(i32, i32)>,
    /// The planet (not a building or units): the globe mark.
    pub planet: bool,
}

/// The selected building's buttons (on when their hot-spot is): repair, demolish, on/off, self-destruct.
#[derive(Debug, Clone, Default)]
pub struct Actions {
    pub on: [bool; 4],
    pub switched_off: bool,
    pub tips: [String; 4],
}

/// The ground war's strip.
#[derive(Debug, Clone, Default)]
pub struct War {
    /// Shown buttons: id, the brush's frame (2 = on), the tooltip.
    pub buttons: Vec<(u32, usize, String)>,
    /// The fighters' and big ships' charge, 0..1.
    pub charge: (f32, f32),
    /// The sides on the field: colour, name, units.
    pub sides: Vec<(Color32, String, i32)>,
    /// The retreat and landing lines.
    pub notices: Vec<String>,
}

/// The self-destruct's window: its two lines, the seconds left, the cancel label (EN's planet).
#[derive(Debug, Clone, Default)]
pub struct Destruct {
    pub lines: [String; 2],
    pub seconds: i32,
    pub cancel: Option<String>,
}

/// The repair or demolish window: title, price line, yes, no.
#[derive(Debug, Clone, Default)]
pub struct Confirm {
    pub title: String,
    pub price: Option<String>,
    pub yes: String,
    pub no: String,
}

/// The radar's dots (0..1 across) and the view's corners.
#[derive(Debug, Clone, Default)]
pub struct Radar {
    pub buildings: Vec<([f32; 2], Color32)>,
    pub units: Vec<([f32; 2], bool)>,
    pub view: Option<[[f32; 2]; 4]>,
    pub tip: String,
}

/// What the screen shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub bar: sm::View,
    pub card: Card,
    pub actions: Actions,
    pub build: Option<Build>,
    pub war: Option<War>,
    /// The attack banner and its brightness.
    pub banner: Option<(String, f32)>,
    pub destruct: Option<Destruct>,
    pub confirm: Option<Confirm>,
    pub radar: Radar,
    /// The details' and the starmap's labels (the card's icons).
    pub details: String,
    pub starmap: String,
}

/// What stays between frames.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub bar: sm::State,
    /// The build menu's group, `None`: the current kind's.
    tab: Option<usize>,
}

/// The colony's own icons.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Glyph {
    Repair,
    Demolish,
    Power,
    Destruct,
    Cursor,
    Build,
    /// A war button by its hot-spot.
    War(u32),
}

fn glyph(p: &egui::Painter, r: Rect, g: Glyph, c: Color32) {
    use std::f32::consts::PI;
    let s = Stroke::new(1.4f32, c);
    let m = r.center();
    let k = r.height().min(r.width()) * 0.3;
    let at = |x: f32, y: f32| pos2(m.x + x * k, m.y + y * k);
    let line = |pts: &[(f32, f32)]| p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), s));
    match g {
        // A spanner.
        Glyph::Repair => {
            p.line_segment([at(-0.85, 0.85), at(0.15, -0.15)], Stroke::new(2.2f32, c));
            p.add(Shape::line((0..=14).map(|j| {
                let a = PI * (0.95 + 1.6 * j as f32 / 14.0);
                pos2(m.x + 0.45 * k + a.cos() * k * 0.5, m.y - 0.45 * k + a.sin() * k * 0.5)
            }).collect(), Stroke::new(1.8f32, c)));
        }
        // A building's outline crossed out.
        Glyph::Demolish => {
            line(&[(-0.9, 0.9), (-0.9, -0.3), (0.0, -0.9), (0.9, -0.3), (0.9, 0.9), (-0.9, 0.9)]);
            p.line_segment([at(-0.45, -0.05), at(0.45, 0.75)], s);
            p.line_segment([at(0.45, -0.05), at(-0.45, 0.75)], s);
        }
        // The power sign.
        Glyph::Power => {
            p.add(Shape::line((0..=20).map(|j| {
                let a = -PI / 2.0 + PI * 0.22 + PI * 1.56 * j as f32 / 20.0;
                pos2(m.x + a.cos() * k * 0.9, m.y + a.sin() * k * 0.9)
            }).collect(), s));
            p.line_segment([at(0.0, -1.05), at(0.0, -0.1)], Stroke::new(1.6f32, c));
        }
        // A burst.
        Glyph::Destruct => {
            let v: Vec<Pos2> = (0..16).map(|j| {
                let a = PI * 2.0 * j as f32 / 16.0;
                let d = if j % 2 == 0 { 1.05 } else { 0.45 };
                pos2(m.x + a.cos() * k * d, m.y + a.sin() * k * d)
            }).collect();
            p.add(Shape::closed_line(v, s));
        }
        // The build cursor: a corner frame round a dot.
        Glyph::Cursor => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                line(&[(sx * 0.95, sy * 0.35), (sx * 0.95, sy * 0.95), (sx * 0.35, sy * 0.95)]);
            }
            p.circle_filled(m, k * 0.25, c);
        }
        Glyph::Build => {
            p.line_segment([at(-0.8, 0.0), at(0.8, 0.0)], Stroke::new(1.8f32, c));
            p.line_segment([at(0.0, -0.8), at(0.0, 0.8)], Stroke::new(1.8f32, c));
        }
        Glyph::War(id) => match id {
            // Air strike: a delta wing.
            0xb => {
                p.add(Shape::closed_line([(0.0, -1.0), (0.9, 0.7), (0.0, 0.35), (-0.9, 0.7)].iter().map(|&(x, y)| at(x, y)).collect(), s));
            }
            // Laser strike: a beam from above onto a mark.
            0xc => {
                p.line_segment([at(0.0, -1.05), at(0.0, 0.45)], Stroke::new(2.0f32, c));
                p.add(Shape::line((0..=12).map(|j| {
                    let a = PI * 2.0 * j as f32 / 12.0;
                    pos2(m.x + a.cos() * k * 0.8, m.y + 0.75 * k + a.sin() * k * 0.25)
                }).collect(), s));
            }
            // Detonator and devastator: a charge with its fuse (the devastator's bigger).
            0xd | 0xe => {
                let big = if id == 0xe { 1.0 } else { 0.75 };
                p.circle_stroke(at(-0.1, 0.2), k * 0.7 * big, s);
                line(&[(0.35, -0.3), (0.65, -0.75), (0.95, -0.95)]);
                if id == 0xe {
                    p.circle_filled(at(-0.1, 0.2), k * 0.25, c);
                }
            }
            // Mines: a ring with studs.
            0xf => {
                p.circle_stroke(m, k * 0.55, s);
                for j in 0..6 {
                    let a = PI * 2.0 * j as f32 / 6.0;
                    p.line_segment([pos2(m.x + a.cos() * k * 0.55, m.y + a.sin() * k * 0.55), pos2(m.x + a.cos() * k, m.y + a.sin() * k)], s);
                }
            }
            // Retreat: a back arrow.
            0x10 => {
                line(&[(0.9, 0.7), (0.9, -0.1), (-0.8, -0.1)]);
                line(&[(-0.35, -0.6), (-0.85, -0.1), (-0.35, 0.4)]);
            }
            // Cancel order: a stop square.
            _ => {
                p.rect_stroke(Rect::from_center_size(m, vec2(k * 1.5, k * 1.5)), 1.0, s, egui::StrokeKind::Middle);
            }
        },
    }
}

/// A square button with a colony glyph, the HUD's [`icon_button`] look.
fn glyph_button(ui: &mut egui::Ui, ink: &Ink, g: Glyph, on: bool, enabled: bool, size: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), if enabled { Sense::click() } else { Sense::hover() });
    let hot = ui.ctx().animate_bool_with_time(resp.id, enabled && resp.hovered(), 0.12);
    let p = ui.painter();
    if resp.is_pointer_button_down_on() && enabled {
        p.rect_filled(r, 2.0, ink.accent.gamma_multiply(0.35));
    } else if on {
        p.rect_filled(r, 2.0, ink.accent.gamma_multiply(0.16));
    }
    if hot > 0.0 {
        p.rect_filled(r, 2.0, Color32::from_white_alpha((18.0 * hot) as u8));
        p.rect_stroke(r, 2.0, Stroke::new(1.0f32, ink.accent.gamma_multiply(0.7 * hot)), egui::StrokeKind::Inside);
    }
    if on {
        p.rect_filled(Rect::from_min_max(pos2(r.left() + 3.0, r.bottom() - 2.0), pos2(r.right() - 3.0, r.bottom())), 0.0, ink.accent);
    }
    let fg = match () {
        _ if !enabled => ink.dim.gamma_multiply(0.6),
        _ if on => lerp(ink.accent, Color32::WHITE, 0.35),
        _ => lerp(ink.text.gamma_multiply(0.82), ink.text, hot),
    };
    glyph(p, r.shrink(1.0), g, fg);
    resp
}

/// A thin bar: the part done in the accent (red below a quarter, amber below half, as `DrawHpStripe`'s colours).
fn meter(ui: &mut egui::Ui, ink: &Ink, w: f32, part: f32, health: bool) {
    let (r, _) = ui.allocate_exact_size(vec2(w, 4.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(r, 1.0, Color32::from_black_alpha(150));
    let part = part.clamp(0.0, 1.0);
    let c = match () {
        _ if !health => ink.accent,
        _ if part > 0.5 => Color32::from_rgb(110, 205, 120),
        _ if part > 0.25 => Color32::from_rgb(232, 182, 72),
        _ => Color32::from_rgb(235, 96, 84),
    };
    p.rect_filled(Rect::from_min_size(r.min, vec2(r.width() * part, r.height())), 1.0, c);
}

/// A hairline across the plate.
fn hairline(ui: &mut egui::Ui, ink: &Ink, w: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, ink.edge);
}

/// The panels' points: the build menu's and the card's inside widths, the radar's side, the buttons.
const MENU_W: f32 = 262.0;
const CARD_W: f32 = 236.0;
const RADAR_W: f32 = 150.0;
const ACT: f32 = 22.0;
const WAR: f32 = 28.0;

/// One frame of the panels over the colony; the acts in the order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut bar = Vec::new();
    let ink = sm::bar(ctx, &v.bar, &mut st.bar, &mut bar);
    let mut acts: Vec<Act> = bar.into_iter().map(Act::Bar).collect();
    let top = ctx.memory(|m| m.area_rect(egui::Id::new("smh-left"))).map_or(46.0, |r| r.bottom() + 6.0);
    if let Some(b) = &v.build {
        egui::Area::new(egui::Id::new("cmh-build")).fixed_pos(pos2(EDGE, top)).fade_in(false).show(ctx, |ui| {
            plate(ui, &ink, Margin::symmetric(10, 8), |ui| build_menu(ui, b, st, &ink, &mut acts));
        });
    }
    let card = egui::Area::new(egui::Id::new("cmh-card")).anchor(Align2::LEFT_BOTTOM, vec2(EDGE, -EDGE)).fade_in(false).show(ctx, |ui| {
        plate(ui, &ink, Margin::symmetric(10, 8), |ui| card_panel(ui, v, &ink, &mut acts));
    });
    if let Some(c) = &v.confirm {
        let at = card.response.rect.right_bottom() + vec2(6.0, 0.0);
        egui::Area::new(egui::Id::new("cmh-confirm")).pivot(Align2::LEFT_BOTTOM).fixed_pos(at).order(egui::Order::Foreground).fade_in(false).show(ctx, |ui| {
            plate(ui, &ink, Margin::symmetric(12, 9), |ui| confirm(ui, c, &ink, &mut acts));
        });
    }
    if let Some(w) = &v.war {
        egui::Area::new(egui::Id::new("cmh-war")).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -EDGE)).fade_in(false).show(ctx, |ui| {
            plate(ui, &ink, Margin::symmetric(10, 7), |ui| war_strip(ui, w, &ink, &mut acts));
        });
    }
    // The banner, the war's lines and the self-destruct under the top bar, centred.
    if v.banner.is_some() || v.destruct.is_some() || v.war.as_ref().is_some_and(|w| !w.notices.is_empty()) {
        egui::Area::new(egui::Id::new("cmh-notice")).anchor(Align2::CENTER_TOP, vec2(0.0, top)).fade_in(false).show(ctx, |ui| {
            plate(ui, &ink, Margin::symmetric(14, 9), |ui| notices(ui, v, &ink, &mut acts));
        });
    }
    egui::Area::new(egui::Id::new("cmh-radar")).anchor(Align2::RIGHT_BOTTOM, vec2(-EDGE, -EDGE)).fade_in(false).show(ctx, |ui| {
        plate(ui, &ink, Margin::same(7), |ui| radar(ui, &v.radar, &ink, &mut acts));
    });
    acts
}

/// The current kind with its costs and the cursor, the groups as tabs, the group's entries with their count and
/// price; a pick puts the build cursor up.
fn build_menu(ui: &mut egui::Ui, b: &Build, st: &mut State, ink: &Ink, acts: &mut Vec<Act>) {
    ui.set_width(MENU_W);
    ui.spacing_mut().item_spacing.y = 3.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let name = b.current.as_ref().map_or("", |c| c.0.as_str());
        ui.add(egui::Label::new(RichText::new(name).size(15.0).strong().color(Color32::WHITE)).truncate());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if sm::either(&tip(glyph_button(ui, ink, Glyph::Build, false, b.cursor, ACT), &b.build_tip, "", ink)) {
                acts.push(Act::Spot(0x13));
            }
            if sm::either(&tip(glyph_button(ui, ink, Glyph::Cursor, b.cursor, !b.pending, ACT), &b.cursor_tip, "", ink)) {
                acts.push(Act::Pick(-1));
            }
        });
    });
    if let Some((_, price, energy, workers)) = &b.current {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(RichText::new(price).size(12.5).strong().color(ink.accent));
            ui.add_space(8.0);
            cost(ui, ink, sm::Icon::Bolt, *energy);
            ui.add_space(8.0);
            cost_workers(ui, ink, *workers);
        });
    }
    ui.add_space(2.0);
    hairline(ui, ink, MENU_W);
    ui.add_space(2.0);
    let current = b.entries.iter().find(|e| e.current).map(|e| e.column);
    let tab = st.tab.or(current).unwrap_or(0).min(b.headers.len().saturating_sub(1));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let w = (MENU_W - 2.0 * (b.headers.len().max(1) - 1) as f32) / b.headers.len().max(1) as f32;
        for (i, h) in b.headers.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 20.0), Sense::click());
            let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
            let p = ui.painter();
            if hot > 0.0 {
                p.rect_filled(r, 2.0, Color32::from_white_alpha((14.0 * hot) as u8));
            }
            let on = i == tab;
            if on {
                p.rect_filled(Rect::from_min_max(pos2(r.left() + 2.0, r.bottom() - 2.0), pos2(r.right() - 2.0, r.bottom())), 0.0, ink.accent);
            }
            let c = if on { Color32::WHITE } else { lerp(ink.dim, ink.text, hot) };
            let mut g = p.layout_no_wrap(h.to_uppercase(), FontId::proportional(10.5), c);
            if g.size().x > w - 4.0 {
                g = p.layout_no_wrap(h.to_uppercase(), FontId::proportional(10.5 * (w - 4.0) / g.size().x), c);
            }
            p.with_clip_rect(r).galley(r.center() - g.size() / 2.0, g, c);
            if resp.clicked() {
                st.tab = Some(i);
            }
        }
    });
    ui.add_space(2.0);
    egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for e in b.entries.iter().filter(|e| e.column == tab) {
            let ok = e.ok && !b.pending;
            let (r, resp) = ui.allocate_exact_size(vec2(MENU_W, 21.0), if ok { Sense::click() } else { Sense::hover() });
            let hot = ui.ctx().animate_bool_with_time(resp.id, ok && resp.hovered(), 0.1);
            let p = ui.painter();
            if e.current {
                p.rect_filled(r, 2.0, ink.accent.gamma_multiply(0.18));
            }
            if hot > 0.0 {
                p.rect_filled(r, 2.0, Color32::from_white_alpha((16.0 * hot) as u8));
            }
            let fg = if !ok { ink.dim.gamma_multiply(0.7) } else if e.current { Color32::WHITE } else { lerp(ink.text.gamma_multiply(0.88), ink.text, hot) };
            let price = p.layout_no_wrap(e.price.clone(), FontId::proportional(12.0), if ok { ink.accent } else { fg });
            let px = r.right() - 6.0 - price.size().x;
            p.galley(pos2(px, r.center().y - price.size().y / 2.0), price, fg);
            let mut name_r = r.left() + 6.0;
            if e.count > 0 {
                let g = p.layout_no_wrap(e.count.to_string(), FontId::proportional(10.5), ink.text);
                let chip = Rect::from_min_size(pos2(name_r, r.center().y - 7.0), vec2(g.size().x.max(8.0) + 7.0, 14.0));
                p.rect_filled(chip, 2.0, ink.edge.gamma_multiply(0.9));
                p.galley(chip.center() - g.size() / 2.0, g, ink.text);
                name_r = chip.right() + 6.0;
            } else {
                name_r += 22.0;
            }
            let g = p.layout_no_wrap(e.name.clone(), FontId::proportional(12.5), fg);
            p.with_clip_rect(Rect::from_min_max(pos2(name_r, r.top()), pos2(px - 6.0, r.bottom()))).galley(pos2(name_r, r.center().y - g.size().y / 2.0), g, fg);
            if sm::either(&resp) {
                acts.push(Act::Pick(e.tech));
                st.tab = None;
            }
        }
    });
}

/// A cost with its icon: energy (the bolt).
fn cost(ui: &mut egui::Ui, ink: &Ink, i: sm::Icon, n: i32) {
    let (r, _) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
    sm::icon(ui.painter(), r, i, ink.dim);
    ui.label(RichText::new(n.to_string()).size(12.5).color(ink.text));
}

/// The workers' cost: a figure.
fn cost_workers(ui: &mut egui::Ui, ink: &Ink, n: i32) {
    let (r, _) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
    figure(ui.painter(), r, ink.dim);
    ui.label(RichText::new(n.to_string()).size(12.5).color(ink.text));
}

/// The workers' icon: a head and shoulders.
fn figure(p: &egui::Painter, r: Rect, c: Color32) {
    let m = r.center();
    p.circle_stroke(pos2(m.x, m.y - 3.5), 2.6, Stroke::new(1.3f32, c));
    p.add(Shape::line((0..=12).map(|j| {
        let a = std::f32::consts::PI * (1.0 + j as f32 / 12.0);
        pos2(m.x + a.cos() * 5.0, m.y + 6.0 + a.sin() * 5.0)
    }).collect(), Stroke::new(1.3f32, c)));
}

/// A surplus: the icon byte's meaning and the signed number, red below zero.
fn surplus(ui: &mut egui::Ui, ink: &Ink, energy: bool, n: i32) {
    let (r, _) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
    if energy {
        sm::icon(ui.painter(), r, sm::Icon::Bolt, ink.dim);
    } else {
        figure(ui.painter(), r, ink.dim);
    }
    let c = if n < 0 { Color32::from_rgb(235, 96, 84) } else { ink.text };
    ui.label(RichText::new(format!("{n:+}")).size(12.5).strong().color(c));
}

/// The short info and the selected building's buttons; the card is hot-spot 4 (left: the info mode, right: the
/// starmap).
fn card_panel(ui: &mut egui::Ui, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    let c = &v.card;
    ui.set_width(CARD_W);
    let body = ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
            let m = r.center();
            if c.planet {
                ui.painter().circle_filled(m, 5.0, c.colour);
                ui.painter().circle_stroke(m, 7.0, Stroke::new(1.0f32, c.colour.gamma_multiply(0.55)));
            } else {
                ui.painter().rect_filled(Rect::from_center_size(m, vec2(9.0, 9.0)), 1.0, c.colour);
            }
            ui.add(egui::Label::new(RichText::new(&c.title).size(15.0).strong().color(Color32::WHITE)).truncate());
        });
        if let Some((w, e)) = c.surplus {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.spacing_mut().item_spacing.x = 3.0;
                surplus(ui, ink, false, w);
                ui.add_space(10.0);
                surplus(ui, ink, true, e);
            });
        }
        ui.add_space(3.0);
        hairline(ui, ink, CARD_W);
        ui.add_space(3.0);
        for (l, col) in &c.lines {
            ui.add(egui::Label::new(RichText::new(l).size(12.0).color(*col)).truncate());
        }
        if let Some((label, icon, n)) = &c.product {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.label(RichText::new(label).size(12.0).color(ink.text));
                match icon {
                    1 => cost_workers(ui, ink, *n),
                    2 => cost(ui, ink, sm::Icon::Bolt, *n),
                    _ => {
                        ui.label(RichText::new(sm::money(*n as f64)).size(12.5).strong().color(ink.text));
                    }
                }
            });
        }
        for (i, (l, part)) in c.bars.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(l).size(11.0).color(ink.dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| meter(ui, ink, 110.0, *part, i == 0));
            });
        }
    });
    let card = ui.interact(body.response.rect, ui.id().with("cmh-card-body"), Sense::click());
    let card = if c.tip.is_empty() { card } else { card.on_hover_text(&c.tip) };
    if card.clicked() {
        acts.push(Act::Card(false));
    }
    if card.secondary_clicked() {
        acts.push(Act::Card(true));
    }
    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if tip(icon_button(ui, ink, sm::Icon::Details, false, true, ACT), &v.details, "", ink).clicked() {
            acts.push(Act::Card(false));
        }
        if !v.starmap.is_empty() && tip(icon_button(ui, ink, sm::Icon::Screen(2), false, true, ACT), &v.starmap, "", ink).clicked() {
            acts.push(Act::Card(true));
        }
        let a = &v.actions;
        if a.on.iter().any(|&o| o) {
            sm::rule(ui, ink, ACT);
        }
        for (i, (g, id)) in [(Glyph::Repair, 1), (Glyph::Demolish, 2), (Glyph::Power, 0x11), (Glyph::Destruct, 10)].into_iter().enumerate() {
            if !a.on[i] {
                continue;
            }
            let on = id == 0x11 && !a.switched_off;
            // Either button (`AktIkon2`).
            if sm::either(&tip(glyph_button(ui, ink, g, on, true, ACT), &a.tips[i], "", ink)) {
                acts.push(Act::Spot(id));
            }
        }
    });
}

/// The repair or demolish window: the question, the price, the answers.
fn confirm(ui: &mut egui::Ui, c: &Confirm, ink: &Ink, acts: &mut Vec<Act>) {
    ui.spacing_mut().item_spacing.y = 4.0;
    ui.label(RichText::new(&c.title).size(13.5).strong().color(Color32::WHITE));
    if let Some(p) = &c.price {
        ui.label(RichText::new(p).size(12.0).color(ink.text));
    }
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let yes = egui::Button::new(RichText::new(&c.yes).color(Color32::WHITE)).fill(ink.accent.gamma_multiply(0.45)).min_size(vec2(64.0, 22.0));
        if ui.add(yes).clicked() {
            acts.push(Act::Answer(true));
        }
        if ui.add(egui::Button::new(RichText::new(&c.no).color(ink.text)).min_size(vec2(64.0, 22.0))).clicked() {
            acts.push(Act::Answer(false));
        }
    });
}

/// The war's buttons (the charge under the strikes) and the sides on the field.
fn war_strip(ui: &mut egui::Ui, w: &War, ink: &Ink, acts: &mut Vec<Act>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for (id, frame, t) in &w.buttons {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let on = *frame > 1 && matches!(id, 0xb | 0xc | 0x10);
                let r = glyph_button(ui, ink, Glyph::War(*id), on, true, WAR);
                // Either button presses it (`AktIkon2`).
                if sm::either(&tip(r, t, "", ink)) {
                    acts.push(Act::Spot(*id));
                }
                match id {
                    0xb => meter(ui, ink, WAR, w.charge.0, false),
                    0xc => meter(ui, ink, WAR, w.charge.1, false),
                    _ => {
                        ui.allocate_exact_size(vec2(WAR, 4.0), Sense::hover());
                    }
                }
            });
        }
        if !w.buttons.is_empty() && !w.sides.is_empty() {
            sm::rule(ui, ink, WAR + 6.0);
        }
        for (c, name, n) in &w.sides {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
                    ui.painter().rect_filled(Rect::from_center_size(r.center(), vec2(7.0, 7.0)), 1.0, *c);
                    ui.label(RichText::new(n.to_string()).size(15.0).strong().color(Color32::WHITE));
                });
                ui.label(RichText::new(name).size(10.5).color(ink.dim));
            });
            ui.add_space(6.0);
        }
    });
}

/// The attack banner, the retreat and landing lines, the self-destruct's countdown and its cancel.
fn notices(ui: &mut egui::Ui, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    ui.spacing_mut().item_spacing.y = 3.0;
    ui.vertical_centered(|ui| {
        if let Some((t, k)) = &v.banner {
            ui.label(RichText::new(t).size(14.0).strong().color(Color32::WHITE.gamma_multiply(k.clamp(0.0, 1.0))));
        }
        for n in v.war.iter().flat_map(|w| &w.notices) {
            ui.label(RichText::new(n).size(12.5).color(ink.accent));
        }
        if let Some(d) = &v.destruct {
            ui.label(RichText::new(&d.lines[0]).size(12.5).color(ink.text));
            ui.label(RichText::new(&d.lines[1]).size(14.0).strong().color(Color32::WHITE));
            ui.label(RichText::new(d.seconds.to_string()).size(30.0).strong().color(Color32::from_rgb(235, 96, 84)));
            if let Some(c) = &d.cancel
                && sm::either(&ui.add(egui::Button::new(RichText::new(c).color(Color32::WHITE)).fill(ink.accent.gamma_multiply(0.4)).min_size(vec2(90.0, 22.0))))
            {
                acts.push(Act::Spot(8));
            }
        }
    });
}

/// The radar (`DrawRadar`): the buildings, the units (own green, the rest red), the view's outline; a press glides the
/// camera there, a drag follows.
fn radar(ui: &mut egui::Ui, r: &Radar, ink: &Ink, acts: &mut Vec<Act>) {
    let (rect, resp) = ui.allocate_exact_size(vec2(RADAR_W, RADAR_W), Sense::click_and_drag());
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(rect, 0.0, Color32::from_black_alpha(150));
    for j in 1..4 {
        let (x, y) = (rect.left() + rect.width() * j as f32 / 4.0, rect.top() + rect.height() * j as f32 / 4.0);
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0f32, ink.edge.gamma_multiply(0.35)));
        p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.0f32, ink.edge.gamma_multiply(0.35)));
    }
    let at = |u: [f32; 2]| pos2(rect.left() + u[0] * rect.width(), rect.top() + u[1] * rect.height());
    for &(u, c) in &r.buildings {
        p.rect_filled(Rect::from_center_size(at(u), vec2(3.0, 3.0)), 0.0, c);
    }
    for &(u, own) in &r.units {
        p.circle_filled(at(u), 1.5, if own { Color32::from_rgb(90, 230, 110) } else { Color32::from_rgb(240, 80, 70) });
    }
    if let Some(q) = r.view {
        p.add(Shape::closed_line([q[0], q[1], q[3], q[2]].into_iter().map(at).collect(), Stroke::new(1.0f32, ink.accent)));
    }
    p.rect_stroke(rect, 0.0, Stroke::new(1.0f32, ink.edge), egui::StrokeKind::Inside);
    let resp = tip(resp, &r.tip, "", ink);
    let (down, first) = ui.input(|i| (i.pointer.any_down(), i.pointer.any_pressed()));
    if let (true, Some(q)) = (resp.is_pointer_button_down_on() && down, resp.interact_pointer_pos()) {
        acts.push(Act::Radar([(q.x - rect.left()) / rect.width(), (q.y - rect.top()) / rect.height()], first));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        View {
            card: Card { title: "Magellan 10".into(), planet: true, surplus: Some((12, -3)), ..Default::default() },
            actions: Actions { on: [true, true, true, false], tips: ["Repair".into(), "Demolish".into(), "On/off".into(), String::new()], ..Default::default() },
            build: Some(Build {
                headers: ["Civil", "Industry", "Military", "Other"].map(String::from).to_vec(),
                entries: vec![
                    Entry { tech: 0x51, column: 0, name: "Colony hub".into(), price: "$1000".into(), ok: true, current: true, count: 1 },
                    Entry { tech: 0x52, column: 0, name: "Houses".into(), price: "$200".into(), ok: true, ..Default::default() },
                    Entry { tech: 0x61, column: 1, name: "Factory".into(), price: "$900".into(), ok: false, ..Default::default() },
                ],
                current: Some(("Colony hub".into(), "$1000".into(), 5, 30)),
                ..Default::default()
            }),
            details: "Details".into(),
            starmap: "Starmap".into(),
            ..Default::default()
        }
    }

    const SIZE: (u32, u32) = (1920, 1080);

    /// Frames of the panels: three to place the anchored areas, then a click at the point `find` gives (button 0 or 1).
    fn click(v: &View, button: u8, find: impl Fn(&egui::Context) -> Pos2) -> Vec<Act> {
        let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        let style = crate::gui::style(&crate::theme::Theme::modern());
        let mut st = State::default();
        let k = crate::gui::Gui::scale(SIZE.1);
        let mut target = None;
        for _ in 0..3 {
            g.run(SIZE, &style, |ctx| {
                show(ctx, v, &mut st);
                target = Some(find(ctx));
            });
        }
        let p = target.unwrap();
        let mut all = Vec::new();
        for step in 0..3 {
            g.pointer(Some([p.x * k, p.y * k]), k);
            match step {
                1 => g.button(button, true),
                2 => g.button(button, false),
                _ => {}
            }
            g.run(SIZE, &style, |ctx| all.extend(show(ctx, v, &mut st)));
        }
        all.retain(|a| !matches!(a, Act::Bar(sm::Act::Hold(None) | sm::Act::History(false))));
        all
    }

    fn area(ctx: &egui::Context, id: &str) -> Rect {
        ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap()
    }

    /// The card is hot-spot 4: left = the planet info mode, right = the starmap.
    #[test]
    fn card_left_and_right() {
        let at = |ctx: &egui::Context| area(ctx, "cmh-card").left_top() + vec2(60.0, 16.0);
        assert_eq!(click(&view(), 0, at), vec![Act::Card(false)]);
        assert_eq!(click(&view(), 1, at), vec![Act::Card(true)]);
    }

    /// A build menu entry is picked by its tech; the current kind's group is the first tab.
    #[test]
    fn build_entry_picks() {
        // The header (≈20), the costs (≈18), the rule, the tabs (20): the second entry's row.
        let at = |ctx: &egui::Context| area(ctx, "cmh-build").left_top() + vec2(120.0, 8.0 + 22.0 + 19.0 + 9.0 + 23.0 + 2.0 + 42.0 + 10.0);
        assert_eq!(click(&view(), 0, at), vec![Act::Pick(0x52)]);
    }

    /// A building entry that can't be picked (no working hub) gives nothing.
    #[test]
    fn unpickable_entry_is_dead() {
        let mut v = view();
        if let Some(b) = &mut v.build {
            b.entries[1].ok = false;
        }
        let at = |ctx: &egui::Context| area(ctx, "cmh-build").left_top() + vec2(120.0, 8.0 + 22.0 + 19.0 + 9.0 + 23.0 + 2.0 + 42.0 + 10.0);
        assert_eq!(click(&v, 0, at), vec![]);
    }

    /// The card's action strip: after the details and starmap icons and the rule, repair is the first.
    #[test]
    fn repair_button_presses_its_spot() {
        let at = |ctx: &egui::Context| area(ctx, "cmh-card").left_bottom() + vec2(10.0 + 2.0 * (ACT + 2.0) + 9.0 + 2.0 + ACT / 2.0, -8.0 - ACT / 2.0);
        assert_eq!(click(&view(), 0, at), vec![Act::Spot(1)]);
    }

    /// A press on the radar asks for the point across it.
    #[test]
    fn radar_press() {
        let at = |ctx: &egui::Context| area(ctx, "cmh-radar").left_top() + vec2(7.0 + RADAR_W / 2.0, 7.0 + RADAR_W / 4.0);
        let acts = click(&view(), 0, at);
        let Some(Act::Radar(u, true)) = acts.first().copied() else { panic!("{acts:?}") };
        assert!((u[0] - 0.5).abs() < 0.02 && (u[1] - 0.25).abs() < 0.02, "{u:?}");
    }
}
