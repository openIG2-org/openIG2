//! Ours (ADR 0070, ADR 0074): the modern colony screen on the egui layer (ADR 0062), the "Modern colony screen"
//! option, drawn with the design kit (ADR 0073, `gui::ds`) after the colony boards. The same functions as the
//! original's 2D parts (re/screens/colony.md §6, §10, §11, §12): the top bar (the modern starmap HUD's,
//! [`crate::starmap_modern::bar`]), the build menu with its costs and the build cursor, the short info (the planet, the
//! selected building or units) with the selected building's buttons, the radar, the ground war's buttons with their
//! charge, the battle's sides, the attack banner, the repair, demolish and self-destruct windows, and the planet info
//! mode's long info with its settings. The 3D view, its keys and its clicks stay the original's. The window fills a
//! [`View`] from the state the classic panels read, runs [`show`] once a frame and hands the [`Act`]s to
//! [`crate::colony_ui::ColonyUi::modern_act`], which runs the classic hot-spots' own code; the turning building and the
//! short info's model are the classic render targets, drawn into the wells the panels leave ([`pictures`]).

use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, Ui};

use crate::gui::ds::{self, Font, Glyph, IconKey, PanelKind, Text, Weight};
use crate::starmap_modern::{self as sm};

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
    /// The radar's zoom keys held: `Some(true)` in (numpad +), `Some(false)` out, `None` let go.
    Zoom(Option<bool>),
    /// The planet info mode's button pressed (spec §12.2): "ALL" 0x16/0x1a/0x1c, tax −/+ 0x17/0x18, priority −/+
    /// 0x1d/0x1e, 0x20 the mode off.
    Info(u32),
    /// The info mode's menu 0x15 (tax), 0x19 (auto-build) or 0x1b (priority) answered with an item.
    InfoPick(u32, i32),
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
    /// The "Build" button's label (its tooltip) and the cursor's.
    pub build_tip: String,
    pub cursor_tip: String,
}

/// A data line of the short info: label, value, the value's colour, a bar (0..1) instead of the value, the product's
/// icon byte (1 workers, 2 energy, 3 money).
#[derive(Debug, Clone, Default)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub colour: Color32,
    pub bar: Option<f32>,
    pub icon: u8,
}

/// The short info: the planet's, the selected building's or the selected units'.
#[derive(Debug, Clone, Default)]
pub struct Card {
    pub title: String,
    pub colour: Color32,
    /// The planet's and the units' lines (text, colour).
    pub lines: Vec<(String, Color32)>,
    /// The building's data lines.
    pub rows: Vec<Row>,
    /// Hot-spot 4's tooltip.
    pub tip: String,
    /// Workers' and energy's surplus (EN's planet at peace).
    pub surplus: Option<(i32, i32)>,
    /// The planet (not a building or units): the globe mark.
    pub planet: bool,
    /// The planet's surface richness and the race's affinity, per cent (the board's ratios beside the globe).
    pub ratios: Option<(i32, i32)>,
}

/// The selected building's buttons (on when their hot-spot is): repair, demolish, on/off, self-destruct.
#[derive(Debug, Clone, Default)]
pub struct Actions {
    pub on: [bool; 4],
    pub switched_off: bool,
    pub tips: [String; 4],
}

/// One side on the field: colour, empire, units, its role (attacker or defender).
#[derive(Debug, Clone, Default)]
pub struct Side {
    pub colour: Color32,
    pub name: String,
    pub units: i32,
    pub role: String,
}

/// The ground war's strip.
#[derive(Debug, Clone, Default)]
pub struct War {
    /// Shown buttons: id, the brush's frame (2 = on), the tooltip.
    pub buttons: Vec<(u32, usize, String)>,
    /// The fighters' and big ships' charge, 0..1.
    pub charge: (f32, f32),
    /// The sides on the field, the player's first.
    pub sides: Vec<Side>,
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
    /// The zoom keys' and the starmap key's tooltips.
    pub keys: [String; 3],
    /// The terrain's picture (`radar_picture`) lies under the screen: drawn by [`radar_under`].
    pub picture: bool,
}

/// A NEEDED block of the long info: label, the NEEDED label and value, the value, the part covered (0..1), short.
#[derive(Debug, Clone, Default)]
pub struct Need {
    pub label: String,
    pub needed: String,
    pub value: String,
    pub part: f32,
    pub short: bool,
}

/// A settings row of the info mode: label, value, its menu's hot-spot and items (value, text), the −/+ spots.
#[derive(Debug, Clone, Default)]
pub struct Setting {
    pub label: String,
    pub value: String,
    pub menu: u32,
    pub items: Vec<(i32, String)>,
    pub all: u32,
    pub steps: Option<(u32, u32)>,
}

/// The planet info mode's long info (`StarmapHosszuInfoKiir` on the colony's planet, spec §12).
#[derive(Debug, Clone, Default)]
pub struct Info {
    pub title: String,
    pub owner: Option<(String, Color32)>,
    /// The table's lines: label, value, short (the shortage colour).
    pub rows: Vec<(String, String, bool)>,
    pub needs: Vec<Need>,
    /// The PROBLEMS label and each problem's per cent and name.
    pub problems: Option<(String, Vec<(i32, String)>)>,
    /// EN's planet: tax, auto-build, priority; and the "ALL" label.
    pub settings: Vec<Setting>,
    pub all: String,
    /// The back-to-the-colony and the starmap plates.
    pub back: String,
    pub starmap: String,
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
    /// The planet info mode is on: its long info.
    pub info: Option<Info>,
    /// The details key's label.
    pub details: String,
}

/// A picture well: the classic render target it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pic {
    /// The planet panel's turning building (`buildshown`, `crate::colony_ui::PanelBuilding::render`).
    Building,
    /// The short info's turning model (`HandleKisInfoObject`: the building, the unit or the globe).
    Info,
    /// The radar's terrain picture, drawn under the panels ([`radar_under`]).
    Radar,
}

/// What stays between frames.
#[derive(Debug, Clone, Default)]
pub struct State {
    pub bar: sm::State,
    /// The build menu's group, `None`: the current kind's.
    tab: Option<usize>,
    /// The zoom key held last frame.
    pub zoom: Option<bool>,
    /// The info mode's open menu (its hot-spot).
    menu: Option<u32>,
    /// This frame's picture wells in pixels and their opacity ([`pictures`]).
    pub wells: Vec<(Pic, Rect, f32)>,
}

/// The boards' measures in points.
const EDGE: f32 = 24.0;
const BUILD_W: f32 = 430.0;
const CARD_W: f32 = 440.0;
const WAR_CARD_W: f32 = 400.0;
const RADAR_W: f32 = 300.0;
const ROUND: f32 = 60.0;
const ROW_H: f32 = 34.0;
const INFO_W: f32 = 760.0;

// ---- Icons ----

/// The colony's line icons (the boards' lucide strokes in a 24-unit box, 2 wide).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Ico {
    Coins,
    Bolt,
    User,
    Info,
    Power,
    Wrench,
    Trash,
    Undo,
    ZoomIn,
    ZoomOut,
    Target,
    Burst,
    /// A war button by its hot-spot.
    War(u32),
}

fn ico(p: &egui::Painter, rect: Rect, i: Ico, c: Color32) {
    let s = rect.width().min(rect.height()) / 24.0;
    let at = |x: f32, y: f32| rect.center() + vec2(x - 12.0, y - 12.0) * s;
    let st = Stroke::new(2.0 * s.max(0.75), c);
    let poly = |pts: &[(f32, f32)]| p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), st));
    let closed = |pts: &[(f32, f32)]| p.add(Shape::closed_line(pts.iter().map(|&(x, y)| at(x, y)).collect(), st));
    let circle = |x: f32, y: f32, r: f32| p.circle_stroke(at(x, y), r * s, st);
    // An arc about (x, y) from `a` to `b` degrees (y down: 90 is below).
    let arc = |x: f32, y: f32, r: f32, a: f32, b: f32| -> Vec<(f32, f32)> {
        (0..=24).map(|j| (a + (b - a) * j as f32 / 24.0).to_radians()).map(|t| (x + r * t.cos(), y + r * t.sin())).collect()
    };
    match i {
        Ico::Coins => {
            circle(8.0, 8.0, 6.0);
            poly(&arc(16.0, 16.0, 6.0, -69.6, 160.5));
            poly(&[(7.0, 6.0), (8.0, 6.0), (8.0, 10.0)]);
            poly(&[(16.71, 13.88), (17.41, 14.59), (14.59, 17.41)]);
        }
        Ico::Bolt => _ = closed(&[(13.0, 2.0), (3.0, 14.0), (12.0, 14.0), (11.0, 22.0), (21.0, 10.0), (12.0, 10.0)]),
        Ico::User => {
            circle(12.0, 7.0, 4.0);
            let mut v = vec![(19.0, 21.0)];
            v.extend(arc(15.0, 19.0, 4.0, 0.0, -90.0));
            v.extend(arc(9.0, 19.0, 4.0, -90.0, -180.0));
            v.push((5.0, 21.0));
            poly(&v);
        }
        Ico::Info => {
            circle(12.0, 12.0, 10.0);
            poly(&[(12.0, 16.0), (12.0, 12.0)]);
            p.circle_filled(at(12.0, 8.0), 1.3 * s, c);
        }
        Ico::Power => {
            poly(&[(12.0, 2.0), (12.0, 12.0)]);
            poly(&arc(12.0, 12.0, 9.0, -40.0, 220.0));
        }
        Ico::Wrench => {
            poly(&arc(15.5, 8.5, 5.5, 10.0, 280.0));
            poly(&[(11.6, 12.4), (4.0, 20.0)]);
            poly(&[(4.0, 20.0), (5.0, 21.0), (12.6, 13.4)]);
        }
        Ico::Trash => {
            poly(&[(3.0, 6.0), (21.0, 6.0)]);
            poly(&[(19.0, 6.0), (19.0, 20.0), (17.0, 22.0), (7.0, 22.0), (5.0, 20.0), (5.0, 6.0)]);
            poly(&[(8.0, 6.0), (8.0, 4.0), (10.0, 2.0), (14.0, 2.0), (16.0, 4.0), (16.0, 6.0)]);
            poly(&[(10.0, 11.0), (10.0, 17.0)]);
            poly(&[(14.0, 11.0), (14.0, 17.0)]);
        }
        Ico::Undo | Ico::War(0x10) => {
            poly(&[(9.0, 14.0), (4.0, 9.0), (9.0, 4.0)]);
            let mut v = vec![(4.0, 9.0)];
            v.extend(arc(14.5, 14.5, 5.5, -90.0, 90.0));
            v.push((11.0, 20.0));
            poly(&v);
        }
        Ico::ZoomIn | Ico::ZoomOut => {
            circle(11.0, 11.0, 8.0);
            poly(&[(21.0, 21.0), (16.65, 16.65)]);
            poly(&[(8.0, 11.0), (14.0, 11.0)]);
            if i == Ico::ZoomIn {
                poly(&[(11.0, 8.0), (11.0, 14.0)]);
            }
        }
        Ico::Target => {
            circle(12.0, 12.0, 10.0);
            for l in [[(22.0, 12.0), (18.0, 12.0)], [(6.0, 12.0), (2.0, 12.0)], [(12.0, 6.0), (12.0, 2.0)], [(12.0, 22.0), (12.0, 18.0)]] {
                poly(&l);
            }
        }
        // The self-destruct: a burst.
        Ico::Burst => {
            let v: Vec<(f32, f32)> = (0..16).map(|j| {
                let (a, d) = (std::f32::consts::TAU * j as f32 / 16.0, if j % 2 == 0 { 10.0 } else { 4.5 });
                (12.0 + a.cos() * d, 12.0 + a.sin() * d)
            }).collect();
            closed(&v);
        }
        Ico::War(id) => match id {
            // Air strike: a jet.
            0xb => _ = closed(&[(12.0, 2.0), (14.0, 9.0), (21.0, 13.0), (21.0, 15.0), (14.0, 13.5), (13.5, 19.0), (16.0, 21.0), (16.0, 22.0), (12.0, 21.0), (8.0, 22.0), (8.0, 21.0), (10.5, 19.0), (10.0, 13.5), (3.0, 15.0), (3.0, 13.0), (10.0, 9.0)]),
            // Laser strike: a beam from above onto a ring.
            0xc => {
                poly(&[(12.0, 2.0), (12.0, 15.0)]);
                let v: Vec<(f32, f32)> = (0..=24).map(|j| std::f32::consts::TAU * j as f32 / 24.0).map(|t| (12.0 + 7.0 * t.cos(), 18.5 + 2.5 * t.sin())).collect();
                poly(&v);
                poly(&[(7.0, 8.0), (9.0, 10.0)]);
                poly(&[(17.0, 8.0), (15.0, 10.0)]);
            }
            // Detonator and devastator: a charge with its fuse (the devastator's with a core).
            0xd | 0xe => {
                circle(10.0, 14.0, 7.0);
                poly(&[(15.0, 9.0), (18.0, 6.0)]);
                p.circle_filled(at(19.5, 4.5), 1.4 * s, c);
                if id == 0xe {
                    p.circle_filled(at(10.0, 14.0), 3.0 * s, c);
                    poly(&[(21.0, 2.0), (22.0, 1.0)]);
                }
            }
            // Mines: a ring with studs.
            0xf => {
                circle(12.0, 12.0, 5.0);
                for j in 0..8 {
                    let a = std::f32::consts::TAU * j as f32 / 8.0;
                    poly(&[(12.0 + 5.0 * a.cos(), 12.0 + 5.0 * a.sin()), (12.0 + 9.5 * a.cos(), 12.0 + 9.5 * a.sin())]);
                }
            }
            // Cancel the order: a stop sign.
            _ => {
                let v: Vec<(f32, f32)> = (0..8).map(|j| std::f32::consts::TAU * (j as f32 + 0.5) / 8.0).map(|t| (12.0 + 10.0 * t.cos(), 12.0 + 10.0 * t.sin())).collect();
                closed(&v);
                poly(&[(9.0, 9.0), (15.0, 15.0)]);
                poly(&[(15.0, 9.0), (9.0, 15.0)]);
            }
        },
    }
}

/// A metal key with a colony icon.
fn key(ui: &mut Ui, i: Ico, on: bool, size: egui::Vec2, tip: &str) -> egui::Response {
    let g = move |p: &egui::Painter, r: Rect, c: Color32| ico(p, r, i, c);
    let r = ui.add(IconKey::new(Glyph::Custom(&g)).on(on).size(size));
    ds::tip(r, tip)
}

/// A round action button (60 across) with its label on a black tag under it (`active` lit); the response is the
/// button's.
fn round(ui: &mut Ui, i: Ico, active: bool, label: &str, tip: &str) -> egui::Response {
    let k = ds::tokens();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let g = move |p: &egui::Painter, r: Rect, c: Color32| ico(p, r, i, c);
        let r = ds::round_button_sized(ui, Glyph::Custom(&g), ROUND, active);
        // Room for the game's longer labels ("Visszavonulás") before they shrink.
        let w = r.rect.width() + 100.0;
        let f = Font { size: 11.0, line: 14.0, weight: Weight::SemiBold, tracking: 0.14, upper: true, lcd: false };
        let fit = Text::new(label, f).layout(ui.ctx(), w - 16.0);
        let tag_w = (fit.width + 16.0).min(w);
        let (row, _) = ui.allocate_exact_size(vec2(r.rect.width(), 18.0), Sense::hover());
        let tag = Rect::from_center_size(row.center(), vec2(tag_w, 18.0));
        ui.painter().rect_filled(tag, ds::RADIUS_PLATE, Color32::BLACK);
        Text::new(label, f).center().shrink(9.0).color(if active { k.ink } else { k.ink_silver }).paint(ui, tag.shrink2(vec2(8.0, 0.0)));
        if tip == label { r } else { ds::tip(r, tip) }
    })
    .inner
}

/// The top of the panels under the bar.
fn top(ctx: &egui::Context) -> f32 {
    sm::bar_bottom(ctx) + 20.0
}

/// One frame of the panels over the colony; the acts in the order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut acts = sm::board_bar(ctx, &v.bar, &mut st.bar, Act::Bar);
    st.wells.clear();
    let ppp = ctx.pixels_per_point();
    let screen = ctx.screen_rect();
    let top = top(ctx);
    if let Some(b) = &v.build {
        let card_top = screen.bottom() - EDGE - 190.0;
        egui::Area::new(egui::Id::new("cmh-build")).fixed_pos(pos2(EDGE, top)).fade_in(false).show(ctx, |ui| build_menu(ui, b, st, (card_top - top - 16.0).max(200.0), &mut acts));
    }
    let war = v.war.is_some();
    let card = egui::Area::new(egui::Id::new("cmh-card")).anchor(Align2::LEFT_BOTTOM, vec2(EDGE, -EDGE)).fade_in(false).show(ctx, |ui| card_panel(ui, v, st, if war { WAR_CARD_W } else { CARD_W }, &mut acts));
    let card = card.response.rect;
    // The selected building's round buttons right of the card, the window over them.
    let a = &v.actions;
    let mut rounds = None;
    if [0, 1, 3].iter().any(|&i| a.on[i]) {
        let r = egui::Area::new(egui::Id::new("cmh-actions")).pivot(Align2::LEFT_BOTTOM).fixed_pos(pos2(card.right() + 24.0, card.bottom())).fade_in(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 22.0;
                for (i, ic, id) in [(0usize, Ico::Wrench, 1u32), (1, Ico::Trash, 2), (3, Ico::Burst, 10)] {
                    if a.on[i] {
                        let open = v.confirm.is_some() && ((id == 1) == v.confirm.as_ref().is_some_and(|c| c.price.is_some()));
                        let label = a.tips[i].split(['\n', '.']).next().unwrap_or("");
                        if ds::either(&round(ui, ic, open, label, &a.tips[i])) {
                            acts.push(Act::Spot(id));
                        }
                    }
                }
            });
        });
        rounds = Some(r.response.rect);
    }
    if let Some(c) = &v.confirm {
        let at = rounds.map_or(pos2(card.right() + 24.0, card.bottom()), |r| pos2(r.left() + 32.0, r.top() - 16.0));
        let t = ds::open_t(ctx, egui::Id::new("cmh-confirm"), true);
        egui::Area::new(egui::Id::new("cmh-confirm")).pivot(Align2::LEFT_BOTTOM).fixed_pos(at + vec2(0.0, (1.0 - t) * 12.0)).order(egui::Order::Foreground).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(t);
            confirm(ui, c, &v.card.title, &mut acts);
        });
    } else {
        ds::open_t(ctx, egui::Id::new("cmh-confirm"), false);
    }
    if let Some(w) = &v.war {
        let width = (screen.width() - 2.0 * (EDGE + WAR_CARD_W + 16.0)).clamp(480.0, INFO_W);
        egui::Area::new(egui::Id::new("cmh-war")).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -EDGE)).fade_in(false).show(ctx, |ui| war_strip(ui, w, width, &mut acts));
    }
    // The banner, the war's lines and the self-destruct under the top bar, centred.
    if v.banner.is_some() || v.destruct.is_some() || v.war.as_ref().is_some_and(|w| !w.notices.is_empty()) {
        egui::Area::new(egui::Id::new("cmh-notice")).anchor(Align2::CENTER_TOP, vec2(0.0, top)).fade_in(false).show(ctx, |ui| notices(ui, v, &mut acts));
    }
    if let Some(i) = &v.info {
        let x = (screen.right() - 150.0 - INFO_W).max(EDGE);
        let t = ds::open_t(ctx, egui::Id::new("cmh-info"), true);
        egui::Area::new(egui::Id::new("cmh-info")).fixed_pos(pos2(x, top + 20.0 + (1.0 - t) * 12.0)).fade_in(false).show(ctx, |ui| {
            ui.multiply_opacity(t);
            info_panel(ui, i, st, &mut acts);
        });
    } else {
        ds::open_t(ctx, egui::Id::new("cmh-info"), false);
        st.menu = None;
    }
    egui::Area::new(egui::Id::new("cmh-radar")).anchor(Align2::RIGHT_BOTTOM, vec2(-EDGE, -EDGE)).fade_in(false).show(ctx, |ui| radar(ui, &v.radar, v.info.is_none(), st, &mut acts));
    for w in &mut st.wells {
        w.1 = Rect::from_min_max((w.1.min.to_vec2() * ppp).to_pos2(), (w.1.max.to_vec2() * ppp).to_pos2());
    }
    acts
}

/// A recessed picture well (`glass-deep` with a rim) that [`pictures`] fills with `pic`.
fn well(ui: &mut Ui, size: egui::Vec2, pic: Pic, st: &mut State) -> egui::Response {
    let k = ds::tokens();
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    ui.painter().rect_filled(r, ds::RADIUS_PLATE, k.glass_deep);
    ds::radial(ui.painter(), r, ds::RADIUS_PLATE, [0.5, 0.6], &[(0.0, k.rim_bright.gamma_multiply(0.3)), (0.7, Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    ds::rim(ui.painter(), r, ds::RADIUS_PLATE, k.rim);
    st.wells.push((pic, r, ui.opacity()));
    resp
}

/// The current kind with its picture, costs and "Build", the groups as tabs, the group's entries with their count
/// and price; a pick puts the build cursor up, a click on the picture toggles it.
fn build_menu(ui: &mut Ui, b: &Build, st: &mut State, room: f32, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let at = ui.painter().add(Shape::Noop);
    ui.set_width(BUILD_W);
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    // The header: the picture and the current kind (`grid 150px 1fr`, 16 inside).
    let head = ui.allocate_ui(vec2(BUILD_W, 208.0), |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(BUILD_W, 208.0), Sense::hover());
        ds::vgrad(ui.painter(), Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom())), egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, &[(0.0, k.rim_bright.gamma_multiply(0.25)), (1.0, Color32::TRANSPARENT)]);
        let mut pic = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(rect.min + vec2(16.0, 16.0), vec2(150.0, 176.0))));
        let w = well(&mut pic, vec2(150.0, 176.0), Pic::Building, st);
        let hot = ds::hover_t(ui.ctx(), w.id, w.hovered() && !b.pending);
        if b.cursor || hot > 0.0 {
            // The build cursor's mark in the well's corner: lit while it is up.
            let m = Rect::from_min_size(w.rect.right_top() + vec2(-30.0, 6.0), vec2(24.0, 24.0));
            let c = if b.cursor { k.lcd_ink } else { k.ink_silver.gamma_multiply(hot) };
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let p = |x: f32, y: f32| m.center() + vec2(sx * x, sy * y) * 9.0;
                ui.painter().add(Shape::line(vec![p(1.0, 0.35), p(1.0, 1.0), p(0.35, 1.0)], Stroke::new(2.0f32, c)));
            }
            ui.painter().circle_filled(m.center(), 2.5, c);
        }
        if b.cursor {
            ds::rim(ui.painter(), w.rect, ds::RADIUS_PLATE, k.lcd_ink.gamma_multiply(0.7));
        } else if hot > 0.0 {
            ds::rim(ui.painter(), w.rect, ds::RADIUS_PLATE, k.rim_bright.gamma_multiply(hot));
        }
        let w = ds::tip(w, &b.cursor_tip);
        if !b.pending && ds::either(&w) {
            acts.push(Act::Pick(-1));
        }
        let col = Rect::from_min_max(rect.min + vec2(16.0 + 150.0 + 16.0, 16.0), rect.max - vec2(16.0, 16.0));
        let (name, price, energy, workers) = b.current.clone().unwrap_or_default();
        let title = Font::HEADING.sized(24.0).tracking(0.08);
        Text::new(&name, title).color(k.ink).shrink(16.0).paint(ui, Rect::from_min_size(col.min, vec2(col.width(), 28.0)));
        let data = Font::DATA.sized(17.0);
        for (j, (i, s)) in [(Ico::Coins, price), (Ico::Bolt, energy.to_string()), (Ico::User, workers.to_string())].into_iter().enumerate() {
            let y = col.top() + 38.0 + 26.0 * j as f32;
            let line = Rect::from_min_size(pos2(col.left(), y), vec2(col.width(), 20.0));
            ico(ui.painter(), Rect::from_min_size(line.min + vec2(0.0, 2.0), vec2(16.0, 16.0)), i, k.info);
            Text::new(&s, data).color(k.info).paint(ui, line.with_min_x(line.left() + 24.0));
        }
        let plate = Rect::from_min_max(pos2(col.left(), col.bottom() - 40.0), col.max);
        let f = ds::Plate::FONT.sized(15.0).tracking(0.14);
        let label = if b.build_tip.is_empty() { "Build" } else { b.build_tip.as_str() };
        let r = ui.put(plate, |ui: &mut Ui| ui.add_enabled(b.cursor, ds::Plate::new(label).primary(b.cursor).font(f).height(40.0).width(plate.width())));
        if b.cursor {
            // The armed plate breathes (the original's "BUILD" sparkles).
            let t = ui.input(|i| i.time) as f32;
            ds::shadow(ui.painter(), r.rect, 8, 0, 10, 0, k.focus.gamma_multiply(0.18 + 0.12 * (t * 3.0).sin()));
            ui.ctx().request_repaint();
        }
        if r.clicked() {
            acts.push(Act::Spot(0x13));
        }
    });
    let _ = head;
    // The groups (4 plates, the current group pressed).
    let current = b.entries.iter().find(|e| e.current).map(|e| e.column);
    let tab = st.tab.or(current).unwrap_or(0).min(b.headers.len().saturating_sub(1));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.spacing_mut().item_spacing.x = 6.0;
        let n = b.headers.len().max(1) as f32;
        let w = (BUILD_W - 32.0 - 6.0 * (n - 1.0)) / n;
        let f = ds::Plate::FONT.sized(12.0).tracking(0.10);
        for (i, h) in b.headers.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 36.0), Sense::click());
            let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
            let on = i == tab;
            ds::plate_fill(ui.painter(), r, ds::RADIUS_PANEL, on || resp.is_pointer_button_down_on());
            if hot > 0.0 && !on {
                ds::rim(ui.painter(), r, ds::RADIUS_PANEL, k.rim_bright.gamma_multiply(hot));
            }
            let cut = Text::new(h, f).center().shrink(9.0).color(if on { k.ink } else { k.ink_silver.lerp_to_gamma(k.ink, hot) }).paint(ui, r.shrink2(vec2(4.0, 0.0))).cut;
            let resp = if cut { resp.on_hover_text(h) } else { resp };
            if resp.clicked() {
                st.tab = Some(i);
            }
        }
    });
    ui.add_space(12.0);
    // The group's entries in a recessed well (name, count, price).
    let rows = b.entries.iter().filter(|e| e.column == tab).count();
    let list_h = (rows.max(1) as f32 * ROW_H).min((room - 208.0 - 52.0 - 28.0).max(3.0 * ROW_H));
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        let (outer, _) = ui.allocate_exact_size(vec2(BUILD_W - 32.0, list_h + 12.0), Sense::hover());
        ui.painter().rect_filled(outer, ds::RADIUS_PLATE, k.glass_deep);
        ds::rim(ui.painter(), outer, ds::RADIUS_PLATE, k.rim);
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(outer.shrink(6.0)).layout(egui::Layout::top_down(egui::Align::Min)));
        egui::ScrollArea::vertical().id_salt("cmh-build-list").max_height(list_h).auto_shrink([false, true]).show(&mut inner, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let f = Font::BODY.sized(16.0);
            for e in b.entries.iter().filter(|e| e.column == tab) {
                let ok = e.ok && !b.pending;
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), if ok { Sense::click() } else { Sense::hover() });
                let hot = ds::hover_t(ui.ctx(), resp.id, ok && resp.hovered());
                ds::row_back(ui, r, hot, e.current);
                let row = r.shrink2(vec2(8.0, 0.0));
                let base = ds::fit::baseline(r, f);
                let ink = if ok { k.ink } else { k.ink_muted };
                let (rest, price) = row.split_left_right_at_x(row.right() - 80.0);
                let (name, count) = rest.split_left_right_at_x(rest.right() - 36.0);
                Text::new(&e.price, Font::DATA.sized(16.0)).right().color(if ok { k.info } else { k.ink_muted }).baseline(base).paint(ui, price);
                if e.count > 0 {
                    Text::new(&e.count.to_string(), Font::DATA.sized(16.0)).right().color(if ok { k.own } else { k.ink_muted }).baseline(base).paint(ui, count.with_max_x(count.right() - 8.0));
                }
                let cut = Text::new(&e.name, f).color(ink).baseline(base).paint(ui, name.with_max_x(name.right() - 8.0)).cut;
                let resp = if cut { resp.on_hover_text(&e.name) } else { resp };
                if ds::either(&resp) {
                    acts.push(Act::Pick(e.tech));
                    st.tab = None;
                }
            }
        });
    });
    ui.add_space(16.0);
    let rect = ui.min_rect();
    ds::behind(ui, at, |p| ds::paint_panel(p, rect, PanelKind::Glass, 0.88));
}

/// The short info: the picture well, the title with the details and on/off keys, the lines or data rows; above it
/// the workers' and energy's surplus. The card is hot-spot 4 (left: the info mode, right: the starmap).
fn card_panel(ui: &mut Ui, v: &View, st: &mut State, width: f32, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let c = &v.card;
    ui.set_width(width);
    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
    if let Some((w, e)) = c.surplus.filter(|_| v.info.is_none()) {
        ui.horizontal(|ui| {
            let cw = (width - 8.0) / 2.0;
            for (i, n) in [(Ico::User, w), (Ico::Bolt, e)] {
                let (r, _) = ui.allocate_exact_size(vec2(cw, 38.0), Sense::hover());
                ds::paint_panel(ui.painter(), r, PanelKind::Deep, 0.88);
                ico(ui.painter(), Rect::from_min_size(r.min + vec2(14.0, 10.0), vec2(18.0, 18.0)), i, k.ink);
                Text::new(&format!("{n:+}"), Font::DATA.sized(18.0)).color(if n < 0 { k.enemy } else { k.own }).paint(ui, r.with_min_x(r.left() + 42.0).shrink2(vec2(0.0, 4.0)));
            }
        });
    }
    let at = ui.painter().add(Shape::Noop);
    let well_w = if c.planet { 80.0 } else if v.war.is_some() { 112.0 } else { 84.0 };
    let inner = egui::Frame::new().inner_margin(14).show(ui, |ui| {
        ui.set_width(width - 28.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            if c.planet {
                // The globe between the planet's ratios (`.ig-card .side`).
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let ratio = |ui: &mut Ui, n: Option<i32>| _ = Text::new(&n.map(|n| format!("{n}%")).unwrap_or_default(), Font::DATA.sized(12.0).tracking(0.08)).center().color(k.ink_muted).show(ui, vec2(well_w, 14.0));
                    ratio(ui, c.ratios.map(|r| r.0));
                    let (r, _) = ui.allocate_exact_size(vec2(well_w, 72.0), Sense::hover());
                    st.wells.push((Pic::Info, Rect::from_center_size(r.center(), vec2(72.0, 72.0)), ui.opacity()));
                    ratio(ui, c.ratios.map(|r| r.1));
                });
            } else {
                well(ui, vec2(well_w, 96.0), Pic::Info, st);
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                let w = ui.available_width();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let keys = if c.planet || !v.actions.on[2] { 1.0 } else { 2.0 };
                    let title = if c.planet { Font::HEADING.sized(20.0).weight(Weight::Medium) } else { Font::HEADING };
                    Text::new(&c.title, title).color(k.ink).shrink(14.0).show(ui, vec2(w - keys * 42.0, 32.0));
                    if ds::tip(ui.add(IconKey::new(Glyph::Custom(&|p: &egui::Painter, r: Rect, col: Color32| ico(p, r, Ico::Info, col))).size(vec2(36.0, 32.0))), &v.details).clicked() {
                        acts.push(Act::Card(false));
                    }
                    if !c.planet && v.actions.on[2] && ds::either(&key(ui, Ico::Power, v.actions.switched_off, vec2(36.0, 32.0), &v.actions.tips[2])) {
                        acts.push(Act::Spot(0x11));
                    }
                });
                for (i, (l, col)) in c.lines.iter().enumerate() {
                    // The units' empire as the board's empire label; the planet's lines upper case (`.ig-card .line`).
                    let f = if c.planet { Font::DATA.tracking(0.12).upper(true) } else if i == 0 { Font::LABEL.tracking(0.16) } else { Font::DATA };
                    Text::new(l, f).shrink(11.0).color(*col).show(ui, vec2(w, 20.0));
                }
                for r in &c.rows {
                    let (rect, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
                    let base = ds::fit::baseline(rect, Font::DATA);
                    let (a, b) = rect.split_left_right_at_x(rect.left() + 110.0);
                    Text::new(&r.label, Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, a.with_max_x(a.right() - 8.0));
                    if let Some(part) = r.bar {
                        let bar = Rect::from_min_size(pos2(b.left(), rect.center().y - 3.0), vec2(b.width(), 6.0));
                        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar));
                        let fill = if part > 0.5 { k.own } else if part > 0.25 { k.lcd_ink } else { k.enemy };
                        ds::bar(&mut child, bar.width(), 6.0, part, if r.icon == 0 { fill } else { k.info });
                    } else {
                        let mut x = b.left();
                        if let Some(i) = [None, Some(Ico::User), Some(Ico::Bolt), Some(Ico::Coins)].get(r.icon as usize).copied().flatten() {
                            ico(ui.painter(), Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(16.0, 16.0)), i, r.colour);
                            x += 22.0;
                        }
                        Text::new(&r.value, Font::DATA).color(r.colour).baseline(base).paint(ui, b.with_min_x(x));
                    }
                }
            });
        });
    });
    let rect = inner.response.rect;
    ds::behind(ui, at, |p| ds::paint_panel(p, rect, PanelKind::Glass, 0.88));
    let card = ui.interact(rect, ui.id().with("cmh-card-body"), Sense::click());
    let hot = ds::hover_t(ui.ctx(), card.id, card.hovered());
    if hot > 0.0 {
        ds::rim(ui.painter(), rect, ds::RADIUS_PANEL, k.rim_bright.gamma_multiply(hot));
    }
    let card = if c.tip.is_empty() { card } else { card.on_hover_text(&c.tip) };
    if card.clicked() {
        acts.push(Act::Card(false));
    }
    if card.secondary_clicked() {
        acts.push(Act::Card(true));
    }
}

/// The repair or demolish window: the question, the building and the price, the answers.
fn confirm(ui: &mut Ui, c: &Confirm, name: &str, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    ds::panel(ui, PanelKind::Raised, 0.96, 16.0, |ui| {
        ui.set_width(300.0 - 32.0);
        ui.spacing_mut().item_spacing.y = 12.0;
        let w = ui.available_width();
        Text::new(&c.title, Font::BODY.weight(Weight::Medium).sized(17.0)).color(k.ink).lines(2).show(ui, vec2(w, 22.0));
        let sub = c.price.as_deref().unwrap_or(name);
        Text::new(sub, Font::DATA.sized(14.0).tracking(0.08)).color(if c.price.is_some() { k.info } else { k.ink_silver }).show(ui, vec2(w, 18.0));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let f = ds::Plate::FONT.sized(15.0);
            let pw = (w - 8.0) / 2.0;
            if ui.add(ds::Plate::new(&c.yes).font(f).height(40.0).width(pw)).clicked() {
                acts.push(Act::Answer(true));
            }
            if ui.add(ds::Plate::new(&c.no).font(f).height(40.0).width(pw)).clicked() {
                acts.push(Act::Answer(false));
            }
        });
    });
}

/// The war's buttons as round keys (the strikes' charge under them), then the sides on the field with the retreat
/// between them.
fn war_strip(ui: &mut Ui, w: &War, width: f32, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    ui.set_width(width);
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 12.0;
        let others: Vec<_> = w.buttons.iter().filter(|b| b.0 != 0x10).collect();
        if !others.is_empty() {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 18.0;
                let n = others.len() as f32;
                ui.add_space(((width - n * (ROUND + 12.0) - (n - 1.0) * 18.0) / 2.0).max(0.0));
                for (id, frame, t) in others {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let label = t.split(['\n', '.', '(']).next().unwrap_or("").trim();
                        if ds::either(&round(ui, Ico::War(*id), *frame > 1, label, t)) {
                            acts.push(Act::Spot(*id));
                        }
                        let charge = match id {
                            0xb => Some(w.charge.0),
                            0xc => Some(w.charge.1),
                            _ => None,
                        };
                        if let Some(c) = charge {
                            let full = c >= 1.0;
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                let r = ds::bar(ui, ROUND - 8.0, 4.0, c, if full { k.lcd_ink } else { k.info });
                                if full {
                                    let t = ui.input(|i| i.time) as f32;
                                    ds::shadow(ui.painter(), r.rect, 2, 0, 6, 0, k.lcd_ink.gamma_multiply(0.3 + 0.2 * (t * 4.0).sin()));
                                    ui.ctx().request_repaint();
                                }
                            });
                        }
                    });
                }
            });
        }
        if w.sides.is_empty() && !w.buttons.iter().any(|b| b.0 == 0x10) {
            return;
        }
        let most = w.sides.iter().map(|s| s.units).max().unwrap_or(1).max(1) as f32;
        ds::panel(ui, PanelKind::Glass, 0.88, 0.0, |ui| {
            egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 14)).show(ui, |ui| {
                ui.set_width(width - 36.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    let side_w = (width - 36.0 - 96.0 - 32.0) / 2.0;
                    let side = |ui: &mut Ui, s: Option<&Side>, right: bool| {
                        // As tall as the retreat's column, the side's lines centred in it (one row, one baseline).
                        let (r, _) = ui.allocate_exact_size(vec2(side_w, 90.0), Sense::hover());
                        let r = Rect::from_center_size(r.center(), vec2(side_w, 66.0));
                        let Some(s) = s else { return };
                        let (head, rest) = r.split_top_bottom_at_y(r.top() + 30.0);
                        let name_f = Font::LABEL.sized(14.0).tracking(0.16);
                        let num_f = Font::HEADING.sized(28.0).tracking(0.06);
                        let base = head.bottom() - 6.0;
                        let num = s.units.to_string();
                        let nw = Text::new(&num, num_f).layout(ui.ctx(), side_w).width;
                        let (name_r, num_r) = if right { (head.with_min_x(head.left() + nw + 8.0), head.with_max_x(head.left() + nw)) } else { (head.with_max_x(head.right() - nw - 8.0), head.with_min_x(head.right() - nw)) };
                        let t = Text::new(&s.name, name_f).color(s.colour).baseline(base);
                        if right { t.right() } else { t }.paint(ui, name_r);
                        Text::new(&num, num_f).color(s.colour).baseline(base).paint(ui, num_r);
                        let role = Text::new(&s.role, Font::LABEL.sized(12.0)).color(k.ink_muted);
                        if right { role.right() } else { role }.paint(ui, Rect::from_min_size(rest.min + vec2(0.0, 4.0), vec2(rest.width(), 16.0)));
                        let bar = Rect::from_min_size(pos2(rest.left(), rest.bottom() - 8.0), vec2(rest.width(), 8.0));
                        let part = s.units as f32 / most;
                        let p = ui.painter();
                        p.rect_filled(bar, 4, k.glass_deep);
                        let f = ui.ctx().animate_value_with_time(ui.id().with(("side", right)), part, ds::anim(ds::GLIDE_TIME));
                        let fill = if right { bar.with_min_x(bar.right() - bar.width() * f) } else { bar.with_max_x(bar.left() + bar.width() * f) };
                        p.rect_filled(fill, 4, s.colour);
                        ds::rim(p, bar, 4, k.rim);
                    };
                    side(ui, w.sides.first(), false);
                    let retreat = w.buttons.iter().find(|b| b.0 == 0x10);
                    ui.allocate_ui(vec2(96.0, 90.0), |ui| {
                        ui.vertical_centered(|ui| {
                            if let Some((id, frame, t)) = retreat {
                                let label = t.split(['\n', '.', '(']).next().unwrap_or("").trim();
                                if ds::either(&round(ui, Ico::Undo, *frame > 1, label, t)) {
                                    acts.push(Act::Spot(*id));
                                }
                            } else {
                                ui.allocate_exact_size(vec2(96.0, 66.0), Sense::hover());
                            }
                        });
                    });
                    side(ui, w.sides.get(1), true);
                });
                for s in w.sides.iter().skip(2) {
                    Text::new(&format!("{} · {}", s.name, s.units), Font::LABEL).center().color(s.colour).show(ui, vec2(width - 36.0, 18.0));
                }
            });
        });
    });
}

/// The attack banner, the retreat and landing lines, the self-destruct's countdown and its cancel.
fn notices(ui: &mut Ui, v: &View, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    ds::panel(ui, PanelKind::Deep, 0.88, 16.0, |ui| {
        ui.set_width(720.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.vertical_centered(|ui| {
            if let Some((t, a)) = &v.banner {
                Text::new(t, Font::HEADING).center().lines(2).color(k.ink.gamma_multiply(a.clamp(0.0, 1.0))).show(ui, vec2(720.0, 26.0));
            }
            for n in v.war.iter().flat_map(|w| &w.notices) {
                Text::new(n, Font::DATA).center().color(k.lcd_ink).show(ui, vec2(720.0, 20.0));
            }
            if let Some(d) = &v.destruct {
                Text::new(&d.lines[0], Font::LABEL).center().color(k.ink_muted).show(ui, vec2(720.0, 18.0));
                Text::new(&d.lines[1], Font::HEADING).center().color(k.ink).show(ui, vec2(720.0, 26.0));
                // The seconds on an LCD, its red glow pulsing once a second.
                let f = Font::READOUT.sized(44.0);
                let s = d.seconds.to_string();
                let (r, _) = ui.allocate_exact_size(vec2(140.0, 52.0), Sense::hover());
                let t = ui.input(|i| i.time) as f32;
                ds::shadow(ui.painter(), r, ds::RADIUS_PLATE, 0, 14, 0, k.signal_red.gamma_multiply(0.25 + 0.2 * (t * std::f32::consts::TAU).cos()));
                ds::lcd_window(ui.painter(), r);
                Text::new(&s, f).center().color(k.signal_red).paint(ui, r);
                ui.ctx().request_repaint();
                if let Some(c) = &d.cancel
                    && ds::either(&ui.add(ds::Plate::new(c).primary(true).font(ds::Plate::FONT.sized(15.0)).height(40.0).width(200.0)))
                {
                    acts.push(Act::Spot(8));
                }
            }
        });
    });
}

/// The planet info mode's long info (the PlanetInfo board): the name and the owner, the table, the workers' and
/// energy's NEEDED blocks and the problems, the settings with their menus, "ALL" and −/+; the plates back to the
/// colony and to the starmap.
fn info_panel(ui: &mut Ui, i: &Info, st: &mut State, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let at = ui.painter().add(Shape::Noop);
    ui.set_width(INFO_W);
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    // The header strip.
    let (head, _) = ui.allocate_exact_size(vec2(INFO_W, 76.0), Sense::hover());
    ds::vgrad(ui.painter(), head, egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, &[(0.0, k.rim_bright.gamma_multiply(0.25)), (1.0, Color32::TRANSPARENT)]);
    let hr = head.shrink2(vec2(28.0, 0.0));
    let base = hr.top() + 22.0 + 30.0;
    let owner_w = i.owner.as_ref().map_or(0.0, |(n, _)| Text::new(n, Font::LABEL.sized(14.0).tracking(0.16)).layout(ui.ctx(), 300.0).width);
    Text::new(&i.title, Font::HEADING.sized(34.0).tracking(0.1)).color(k.ink).shrink(22.0).baseline(base).paint(ui, hr.with_max_x(hr.right() - owner_w - 16.0));
    if let Some((n, c)) = &i.owner {
        Text::new(n, Font::LABEL.sized(14.0).tracking(0.16)).right().color(*c).baseline(base).paint(ui, hr.with_min_x(hr.right() - owner_w));
    }
    // The table and the blocks, two columns.
    ui.add_space(6.0);
    ui.horizontal_top(|ui| {
        ui.add_space(28.0);
        ui.spacing_mut().item_spacing.x = 28.0;
        let cw = (INFO_W - 56.0 - 28.0) / 2.0;
        ui.allocate_ui_with_layout(vec2(cw, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(cw);
            ui.spacing_mut().item_spacing.y = 10.0;
            for (l, val, short) in &i.rows {
                let (r, _) = ui.allocate_exact_size(vec2(cw, 22.0), Sense::hover());
                let f = Font::DATA.sized(17.0);
                let base = ds::fit::baseline(r, f);
                let (a, b) = r.split_left_right_at_x(r.left() + 130.0);
                Text::new(l, Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, a.with_max_x(a.right() - 8.0));
                Text::new(val, f).shrink(12.0).color(if *short { k.enemy } else { k.info }).baseline(base).paint(ui, b);
            }
        });
        ui.allocate_ui_with_layout(vec2(cw, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(cw);
            ui.spacing_mut().item_spacing.y = 8.0;
            for n in &i.needs {
                let (r, _) = ui.allocate_exact_size(vec2(cw, 18.0), Sense::hover());
                let (a, b) = r.split_left_right_at_x(r.center().x);
                Text::new(&n.label, Font::LABEL).color(k.ink_muted).paint(ui, a);
                Text::new(&n.needed, Font::LABEL).right().color(k.ink_muted).paint(ui, b);
                let c = if n.short { k.enemy } else { k.info };
                Text::new(&n.value, Font::DATA.sized(22.0)).color(c).show(ui, vec2(cw, 26.0));
                ds::bar(ui, cw, 6.0, n.part, c);
                ui.add_space(6.0);
                ds::separator(ui, 0.0);
                ui.add_space(6.0);
            }
            if let Some((label, ps)) = &i.problems {
                ds::heading_label(ui, label);
                ui.spacing_mut().item_spacing.y = 4.0;
                for (pct, name) in ps {
                    let (r, _) = ui.allocate_exact_size(vec2(cw, 22.0), Sense::hover());
                    let f = Font::DATA.sized(17.0).tracking(0.08);
                    let base = ds::fit::baseline(r, f);
                    let p = format!("{pct}%");
                    let pw = Text::new(&p, f).layout(ui.ctx(), cw).width;
                    Text::new(&p, f).color(k.enemy).baseline(base).paint(ui, r);
                    Text::new(name, f).color(k.ink).baseline(base).paint(ui, r.with_min_x(r.left() + pw + 8.0));
                }
            }
        });
    });
    ui.add_space(22.0);
    // The settings (EN's planet).
    if !i.settings.is_empty() {
        let (line, _) = ui.allocate_exact_size(vec2(INFO_W, 1.0), Sense::hover());
        ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0f32, k.rim));
        ui.add_space(20.0);
        for s in &i.settings {
            ui.horizontal(|ui| {
                ui.add_space(28.0);
                ui.spacing_mut().item_spacing.x = 10.0;
                let (l, _) = ui.allocate_exact_size(vec2(160.0, 40.0), Sense::hover());
                Text::new(&s.label, Font::LABEL).color(k.ink_muted).paint(ui, l);
                let field_w = INFO_W - 56.0 - 160.0 - 80.0 - 2.0 * 44.0 - 40.0;
                let open = st.menu == Some(s.menu);
                let (f, resp) = ui.allocate_exact_size(vec2(field_w, 40.0), Sense::click());
                let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
                ui.painter().rect_filled(f, ds::RADIUS_PLATE, k.glass_deep);
                ds::rim(ui.painter(), f, ds::RADIUS_PLATE, if open { k.focus } else { k.rim_bright.lerp_to_gamma(k.focus, hot * 0.6) });
                Text::new(&s.value, Font::DATA.sized(16.0).tracking(0.08)).color(k.ink).shrink(12.0).paint(ui, f.shrink2(vec2(12.0, 0.0)).with_max_x(f.right() - 34.0));
                ds::Icon::ChevronDown.paint(ui.painter(), Rect::from_center_size(pos2(f.right() - 18.0, f.center().y), vec2(16.0, 16.0)), k.ink_silver);
                if resp.clicked() {
                    st.menu = if open { None } else { Some(s.menu) };
                }
                if open {
                    let picked = menu_list(ui, f, s, st);
                    if let Some(item) = picked {
                        acts.push(Act::InfoPick(s.menu, item));
                        st.menu = None;
                    }
                }
                let r = ui.add(ds::Plate::new(&i.all).font(ds::Plate::FONT.sized(13.0)).height(40.0).width(80.0));
                if ds::pressed(&r) {
                    acts.push(Act::Info(s.all));
                }
                if let Some((minus, plus)) = s.steps {
                    for (id, icon) in [(minus, ds::Icon::Minus), (plus, ds::Icon::Plus)] {
                        ui.allocate_ui(vec2(44.0, 40.0), |ui| {
                            if ds::pressed(&ds::round_button(ui, Glyph::Icon(icon), true, false)) {
                                acts.push(Act::Info(id));
                            }
                        });
                    }
                }
            });
            ui.add_space(10.0);
        }
        ui.add_space(10.0);
    }
    let rect = ui.min_rect();
    ds::behind(ui, at, |p| ds::paint_panel(p, rect, PanelKind::Glass, 0.9));
    // The board's plates 12 under the panel.
    ui.add_space(12.0);
    ui.allocate_ui_with_layout(vec2(INFO_W, 44.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        if ui.add(ds::Plate::new(&i.starmap).width(220.0)).clicked() {
            acts.push(Act::Card(true));
        }
        if ds::pressed(&ui.add(ds::Plate::new(&i.back).width(220.0))) {
            acts.push(Act::Info(0x20));
        }
    });
}

/// An info menu's open list under its field `f`: the items, the current one selected; a click picks one, a click
/// elsewhere closes it.
fn menu_list(ui: &Ui, f: Rect, s: &Setting, st: &mut State) -> Option<i32> {
    let mut picked = None;
    let id = egui::Id::new(("cmh-menu", s.menu));
    let area = egui::Area::new(id).fixed_pos(f.left_bottom() + vec2(0.0, 4.0)).order(egui::Order::Foreground).fade_in(false).show(ui.ctx(), |ui| {
        ds::panel(ui, PanelKind::Deep, 0.97, 6.0, |ui| {
            ui.set_width(f.width() - 12.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            for (v, t) in &s.items {
                if ui.add(ds::ListRow::new(t).selected(*t == s.value).height(30.0).font(Font::DATA.sized(16.0))).clicked() {
                    picked = Some(*v);
                }
            }
        });
    });
    if picked.is_none() && ui.input(|i| i.pointer.any_pressed()) && !area.response.rect.contains(ui.input(|i| i.pointer.interact_pos()).unwrap_or(Pos2::ZERO)) && !f.contains(ui.input(|i| i.pointer.interact_pos()).unwrap_or(Pos2::ZERO)) {
        st.menu = None;
    }
    picked
}

/// The radar in its metal frame (`DrawRadar`): the buildings, the units (own green, the rest red), the view's
/// outline; a press glides the camera there, a drag follows. Under it the zoom keys (held: numpad + / −) and the
/// starmap.
fn radar(ui: &mut Ui, r: &Radar, live: bool, st: &mut State, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let (outer, _) = ui.allocate_exact_size(vec2(RADAR_W, RADAR_W + 8.0 + 32.0), Sense::hover());
    let screen = Rect::from_min_size(outer.min + vec2(10.0, 10.0), vec2(RADAR_W - 20.0, RADAR_W - 20.0));
    // Over the terrain's picture the metal is a frame around the screen (the picture lies under the panels).
    let parts = if r.picture {
        let o = outer.expand(12.0);
        vec![o.with_max_y(screen.top()), o.with_min_y(screen.bottom()), Rect::from_x_y_ranges(o.left()..=screen.left(), screen.y_range()), Rect::from_x_y_ranges(screen.right()..=o.right(), screen.y_range())]
    } else {
        vec![ui.clip_rect()]
    };
    for part in parts {
        let p = ui.painter().with_clip_rect(part.intersect(ui.clip_rect()));
        ds::shadow(&p, outer.translate(vec2(0.0, 3.0)), 18, 0, 8, 0, Color32::from_black_alpha(204));
        ds::radial(&p, outer, 18, [0.5, 0.0], &[(0.0, k.metal_light), (0.35, k.metal), (1.0, k.metal_dark)]);
        ds::relief(&p, outer, 18);
    }
    // Hot-spot 5 is off in the planet info mode (spec §12.2).
    let resp = ui.interact(screen, ui.id().with("cmh-radar-screen"), if live { Sense::click_and_drag() } else { Sense::hover() });
    let p = ui.painter().with_clip_rect(screen);
    if r.picture {
        // The board's terrain picture, its edges sunk into the recess.
        st.wells.push((Pic::Radar, screen, ui.opacity()));
        ds::radial(&p, screen, 10, [0.5, 0.5], &[(0.0, Color32::TRANSPARENT), (0.75, Color32::TRANSPARENT), (1.0, Color32::from_black_alpha(140))]);
    } else {
        p.rect_filled(screen, 10, Color32::BLACK);
        ds::radial(&p, screen, 10, [0.5, 0.5], &[(0.0, k.glass.gamma_multiply(0.9)), (1.0, Color32::BLACK)]);
        let grid = Stroke::new(1.0f32, k.rim_bright.gamma_multiply(0.18));
        for j in 1..8 {
            let (x, y) = (screen.left() + screen.width() * j as f32 / 8.0, screen.top() + screen.height() * j as f32 / 8.0);
            p.vline(x, screen.y_range(), grid);
            p.hline(screen.x_range(), y, grid);
        }
    }
    let at = |u: [f32; 2]| pos2(screen.left() + u[0] * screen.width(), screen.top() + u[1] * screen.height());
    for &(u, c) in &r.buildings {
        p.rect_filled(Rect::from_center_size(at(u), vec2(5.0, 5.0)), 1, c);
    }
    for &(u, own) in &r.units {
        let c = if own { k.own } else { k.enemy };
        p.circle_filled(at(u), 4.0, c.gamma_multiply(0.25));
        p.circle_filled(at(u), 2.2, c);
    }
    if let Some(q) = r.view {
        let pts: Vec<Pos2> = [q[0], q[1], q[3], q[2]].into_iter().map(at).collect();
        p.add(Shape::convex_polygon(pts.clone(), k.lcd_ink.gamma_multiply(0.06), Stroke::NONE));
        p.add(Shape::closed_line(pts, Stroke::new(1.5f32, if r.picture { k.ink } else { k.lcd_ink })));
    }
    // The recess's inner shadow and the metal's dark ring.
    ds::vgrad(ui.painter(), screen, 10, &[(0.0, Color32::from_black_alpha(220)), (6.0 / screen.height(), Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    ds::rim(ui.painter(), screen.expand(1.0), 11, k.metal_shadow);
    let resp = if r.tip.is_empty() { resp } else { resp.on_hover_text(&r.tip) };
    if live && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    let (down, first) = ui.input(|i| (i.pointer.any_down(), i.pointer.any_pressed()));
    if let (true, Some(q)) = (live && resp.is_pointer_button_down_on() && down, resp.interact_pointer_pos()) {
        acts.push(Act::Radar([(q.x - screen.left()) / screen.width(), (q.y - screen.top()) / screen.height()], first));
    }
    // The keys.
    let keys = Rect::from_center_size(pos2(outer.center().x, outer.bottom() - 10.0 - 16.0), vec2(3.0 * 34.0 + 2.0 * 2.0 + 4.0, 32.0));
    let mut zoom = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(keys));
    ds::key_group(&mut child, |ui| {
        for (i, (ic, z)) in [(Ico::ZoomOut, Some(false)), (Ico::ZoomIn, Some(true)), (Ico::Target, None)].into_iter().enumerate() {
            let resp = key(ui, ic, false, vec2(34.0, 28.0), &r.keys[i]);
            match z {
                Some(z) if resp.is_pointer_button_down_on() => zoom = Some(z),
                None if resp.clicked() => acts.push(Act::Card(true)),
                _ => {}
            }
        }
    });
    if zoom != st.zoom {
        acts.push(Act::Zoom(zoom));
        st.zoom = zoom;
    } else if zoom.is_some() {
        acts.push(Act::Zoom(zoom));
    }
}

/// The classic render targets in the wells [`show`] left (`wells` in pixels): the panel's turning building
/// (`crate::colony_ui::PANEL_VIEW`'s pass about (98, 88)) and the short info's model (the info box's corner,
/// 0..50 × 400..480), each fitted into its well and drawn over the panels.
pub fn pictures(f: &mut crate::render::Frame, wells: &[(Pic, Rect, f32)], building: Option<crate::render::TexId>, info: Option<crate::render::TexId>, v: &crate::font::Virtual) {
    let wide = crate::kit::Wide::new(v);
    for &(pic, r, alpha) in wells {
        let (tex, src) = match pic {
            // The middle of the panel's picture (the model turns about (98, 88)), larger in the well.
            Pic::Building => (building, [43.0, 38.0, 110.0, 100.0]),
            // The box's model stands about (26, 446): its middle, not the box's empty edges.
            Pic::Info => (info, [4.0, 418.0, 44.0, 56.0]),
            Pic::Radar => continue,
        };
        let Some(tex) = tex else { continue };
        let [mut x0, mut y0, mut w0, mut h0] = src;
        // The building fits its well; the short info's model fills it (the box's corner cut to the well's shape).
        if pic == Pic::Info {
            let aspect = r.width() / r.height().max(1.0);
            let (w1, h1) = if w0 / h0 > aspect { (h0 * aspect, h0) } else { (w0, w0 / aspect) };
            (x0, y0, w0, h0) = (x0 + (w0 - w1) / 2.0, y0 + (h0 - h1) / 2.0, w1, h1);
        }
        let s = (r.width() / (w0 * v.k)).min(r.height() / (h0 * v.k));
        let half = [w0 * v.k * s / 2.0, h0 * v.k * s / 2.0];
        let mut i = crate::render::Instance::new(crate::render::kind::SPRITE, [r.center().x, r.center().y], half, [alpha; 4]);
        i.uv = [wide.u(x0), y0 / 480.0, w0 / (640.0 * wide.sx), h0 / 480.0];
        f.push(tex, crate::render::Blend::GammaPremul, i);
    }
}

/// The radar's terrain picture (`colony::radar_picture`, the classic radar's `CopyToScreen(28)`) into its screen, in
/// the frame under the panels: the radar's dots and outline lie over it.
pub fn radar_under(f: &mut crate::render::Frame, wells: &[(Pic, Rect, f32)], radar: Option<crate::render::TexId>) {
    let Some(tex) = radar else { return };
    for &(_, r, alpha) in wells.iter().filter(|w| w.0 == Pic::Radar) {
        let i = crate::render::Instance::new(crate::render::kind::SPRITE, [r.center().x, r.center().y], [r.width() / 2.0, r.height() / 2.0], [1.0, 1.0, 1.0, alpha]);
        f.push(tex, crate::render::Blend::Alpha, i);
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
        let at = |ctx: &egui::Context| area(ctx, "cmh-card").left_bottom() + vec2(200.0, -30.0);
        assert_eq!(click(&view(), 0, at), vec![Act::Card(false)]);
        assert_eq!(click(&view(), 1, at), vec![Act::Card(true)]);
    }

    /// A build menu entry is picked by its tech; the current kind's group is the first tab.
    #[test]
    fn build_entry_picks() {
        // The header (208), the tabs (4 + 36 + 12), the well's 6: the second entry's row.
        let at = |ctx: &egui::Context| area(ctx, "cmh-build").left_top() + vec2(120.0, 208.0 + 52.0 + 6.0 + ROW_H * 1.5);
        assert_eq!(click(&view(), 0, at), vec![Act::Pick(0x52)]);
    }

    /// A building entry that can't be picked (no working hub) gives nothing.
    #[test]
    fn unpickable_entry_is_dead() {
        let mut v = view();
        if let Some(b) = &mut v.build {
            b.entries[1].ok = false;
        }
        let at = |ctx: &egui::Context| area(ctx, "cmh-build").left_top() + vec2(120.0, 208.0 + 52.0 + 6.0 + ROW_H * 1.5);
        assert_eq!(click(&v, 0, at), vec![]);
    }

    /// Long or translated texts never resize the panels: they are shrunk and cut inside them.
    #[test]
    fn long_texts_keep_the_panels() {
        let long = "Rendkívül hosszú nevű épület, amely sehogy sem fér el a sorban".repeat(3);
        let mut v = view();
        v.card.title = long.clone();
        if let Some(b) = &mut v.build {
            b.entries[0].name = long.clone();
            b.headers[3] = long.clone();
            b.current.as_mut().unwrap().0 = long;
        }
        let widths = |v: &View| {
            let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
            let style = crate::gui::style(&crate::theme::Theme::modern());
            let mut st = State::default();
            let mut w = (0.0, 0.0);
            for _ in 0..3 {
                g.run(SIZE, &style, |ctx| {
                    show(ctx, v, &mut st);
                    w = (area(ctx, "cmh-build").width(), area(ctx, "cmh-card").width());
                });
            }
            w
        };
        assert_eq!(widths(&v), widths(&view()));
    }

    /// The picture well toggles the build cursor (the pick list's short press).
    #[test]
    fn building_well_toggles_the_cursor() {
        let at = |ctx: &egui::Context| area(ctx, "cmh-build").left_top() + vec2(16.0 + 75.0, 16.0 + 88.0);
        assert_eq!(click(&view(), 0, at), vec![Act::Pick(-1)]);
    }

    /// The round buttons right of the card: repair is the first.
    #[test]
    fn repair_button_presses_its_spot() {
        let at = |ctx: &egui::Context| area(ctx, "cmh-actions").left_top() + vec2(36.0, 36.0);
        assert_eq!(click(&view(), 0, at), vec![Act::Spot(1)]);
    }

    /// A press on the radar asks for the point across it.
    #[test]
    fn radar_press() {
        let at = |ctx: &egui::Context| area(ctx, "cmh-radar").left_top() + vec2(10.0 + (RADAR_W - 20.0) / 2.0, 10.0 + (RADAR_W - 20.0) / 4.0);
        let acts = click(&view(), 0, at);
        let Some(Act::Radar(u, true)) = acts.first().copied() else { panic!("{acts:?}") };
        assert!((u[0] - 0.5).abs() < 0.02 && (u[1] - 0.25).abs() < 0.02, "{u:?}");
    }

    /// The info mode's "ALL" and back plates are its hot-spots' acts; a menu item is picked by its value.
    #[test]
    fn info_mode_settings() {
        let mut v = view();
        v.build = None;
        v.info = Some(Info {
            title: "Magellan 10".into(),
            settings: vec![Setting { label: "Tax".into(), value: "Low".into(), menu: 0x15, items: vec![(0, "None".into()), (1, "Low".into())], all: 0x16, steps: Some((0x17, 0x18)) }],
            all: "ALL".into(),
            back: "Colony".into(),
            starmap: "Starmap".into(),
            ..Default::default()
        });
        // The header (76 + 6), the empty columns, 22, the rule and 20: the settings row; "ALL" after 28 + 160 + field.
        let field_w = INFO_W - 56.0 - 160.0 - 80.0 - 88.0 - 40.0;
        let row = |ctx: &egui::Context| area(ctx, "cmh-info").left_top() + vec2(0.0, 76.0 + 6.0 + 22.0 + 1.0 + 20.0 + 20.0);
        assert_eq!(click(&v, 0, |ctx| row(ctx) + vec2(28.0 + 160.0 + 10.0 + field_w + 10.0 + 40.0, 0.0)), vec![Act::Info(0x16)]);
        let back = |ctx: &egui::Context| area(ctx, "cmh-info").right_bottom() + vec2(-220.0 - 12.0 - 110.0, -22.0);
        assert_eq!(click(&v, 0, back), vec![Act::Info(0x20)]);
    }
}
