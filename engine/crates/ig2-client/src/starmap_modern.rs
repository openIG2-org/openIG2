//! Ours (ADR 0065): the modern starmap HUD on the egui layer (ADR 0062), the "Modern starmap HUD" option. The same
//! functions as the original's HUD (re/screens/starmap.md §11, re/screens/hud.md §2, re/screens/input.md §6): the top
//! bar's pause and speeds, date, money, message line and history, its screen and quick-research menus; the selection's
//! short info with its tanks and production buttons; the console's camera buttons and the minimap. The window fills
//! a [`View`] from the same state the classic HUD reads, runs [`show`] once a frame and applies the [`Act`]s through
//! the classic hot-spots' own code, so the orders, sounds and tutorial gates are the same. The radial menu, the select
//! menus and the panels stay the original's.
//!
//! The look is a ship's console: small dark plates with cut corners, a hairline edge and corner marks in the player's
//! colour, drawn icons with their keys in the tooltips. It is laid out in points and follows the shared UI scale.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, Sense, Shadow, Shape, Stroke};

/// The classic minimap's inside (`GFX_StarmapRadar`'s `HatarX1..Y2`, crate::starmap_hud::MINIMAP): its corner and size.
const MINI: [f32; 4] = [274.0, 388.0, 92.0, 65.0];

/// A galaxy point on the minimap, 0..1 across its window (the classic window's mapping, so a click goes back through
/// `orders::minimap_target` unchanged).
pub fn mini_unit(p: [f32; 2]) -> [f32; 2] {
    let q = crate::starmap_hud::minimap_point(p);
    [(q[0] - MINI[0]) / MINI[2], (q[1] - MINI[1]) / MINI[3]]
}

/// A point 0..1 across the minimap as the classic window's virtual point (hot-spot 7's).
pub fn mini_virtual(u: [f32; 2]) -> [f32; 2] {
    [MINI[0] + u[0].clamp(0.0, 1.0) * MINI[2], MINI[1] + u[1].clamp(0.0, 1.0) * MINI[3]]
}

/// The UI theme (ADR 0056) as the HUD's egui style: its colours, the HUD's smaller type and tighter spacing, square
/// tooltips.
pub fn style(h: &crate::ui::Hud, size: [f32; 2]) -> egui::Style {
    use egui::{FontFamily::Proportional, TextStyle};
    let theme = h.font.as_ref().map_or_else(crate::theme::Theme::modern, |font| {
        crate::theme::look(&crate::options::Draw { font, white: h.white, glow: h.white, v: crate::font::Virtual::new(size), palette: &h.palette, texts: &h.texts, now_ms: 0.0 })
    });
    let mut s = crate::gui::style(&theme);
    s.text_styles = [
        (TextStyle::Heading, FontId::new(15.0, Proportional)),
        (TextStyle::Body, FontId::new(12.5, Proportional)),
        (TextStyle::Button, FontId::new(12.5, Proportional)),
        (TextStyle::Small, FontId::new(11.0, Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, egui::FontFamily::Monospace)),
    ]
    .into();
    let sp = &mut s.spacing;
    (sp.item_spacing, sp.button_padding, sp.interact_size.y, sp.menu_margin) = (vec2(4.0, 2.0), vec2(6.0, 2.0), 20.0, Margin::symmetric(8, 5));
    let v = &mut s.visuals;
    (v.window_corner_radius, v.menu_corner_radius) = (CornerRadius::same(2), CornerRadius::same(2));
    v.popup_shadow = Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(130) };
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open, &mut v.widgets.noninteractive] {
        w.corner_radius = CornerRadius::same(2);
        w.expansion = 0.0;
    }
    s
}

/// The money with its thousands apart (thin spaces).
pub fn money(m: f64) -> String {
    let digits = (m as i64).unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('\u{2009}');
        }
        out.push(c);
    }
    format!("{}${out}", if m < 0.0 { "-" } else { "" })
}

/// What the HUD asks for; the window runs the classic hot-spot's code for each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// Hot-spot 2.
    Pause,
    /// Hot-spots 3..=5: speed 1..=3.
    Speed(i32),
    /// The message line's left button (hot-spot 7).
    Messages,
    /// The message line's right button held: the history (`GFX_ScrollHistory`).
    History(bool),
    /// The screen menu's pick (`AktKepKell`).
    Screen(i32),
    /// The quick-research menu: start a tech, stop the running one.
    Start(i32),
    Stop(i32),
    /// A console button held this frame (hot-spot 2, 3, 5 or 6; `true` = as the left button), or none.
    Hold(Option<(u32, bool)>),
    /// The top view (1) or the side view (4).
    View(u32),
    /// The minimap pressed (`true`) or dragged at the classic window's virtual point (hot-spot 7).
    Minimap([f32; 2], bool),
    /// The info box (hot-spot 8) with the left button (`false`) or the right one.
    Info(bool),
    /// A panel hot-spot: 10 tanks, 11 production.
    Panel(u32),
}

/// One row of the screen menu.
#[derive(Debug, Clone, Default)]
pub struct ScreenItem {
    pub n: i32,
    pub name: String,
    pub key: String,
    pub on: bool,
    pub current: bool,
}

/// The quick-research menu.
#[derive(Debug, Clone, Default)]
pub struct Research {
    pub headers: Vec<String>,
    /// Tech, name, column, startable, short of labs, cost.
    pub items: Vec<(i32, String, usize, bool, bool, i32)>,
    /// The running project: tech, name, per cent, money left.
    pub current: Option<(i32, String, i32, i32)>,
    pub researchable: bool,
    /// `GetString(0x12, 3, 1..=4)`: Researching, STOP, Money left, Cost.
    pub footer: [String; 4],
}

/// The selection's short info.
#[derive(Debug, Clone, Default)]
pub struct Info {
    pub title: String,
    pub colour: Color32,
    pub lines: Vec<(String, Color32)>,
    /// Richness and habitability (planet mode): label, value.
    pub stats: Vec<(String, String)>,
    pub planet: bool,
    pub tanks: bool,
    pub product: bool,
    pub redirect: bool,
    /// The info box's tooltip (`Starmap_MouseInit2`'s text for hot-spot 8).
    pub tip: String,
    /// The left and the right button's labels.
    pub left: String,
    pub right: Option<String>,
}

/// The minimap's points, 0..1 across.
#[derive(Debug, Clone, Default)]
pub struct Mini {
    pub planets: Vec<([f32; 2], Color32, bool)>,
    pub fleets: Vec<([f32; 2], Color32, bool)>,
    pub view: [Option<[f32; 2]>; 4],
}

/// The remake's own labels (ADR 0042), in the game's language.
#[derive(Debug, Clone, Default)]
pub struct Labels {
    pub pause: String,
    pub speed: String,
    pub research: String,
    pub idle: String,
    pub redirect: String,
}

/// The keys bound to the HUD's buttons (ADR 0058), for their tooltips.
#[derive(Debug, Clone, Default)]
pub struct Keys {
    pub pause: String,
    pub speed: [String; 3],
    /// Zoom in, zoom out.
    pub zoom: [String; 2],
}

/// What the HUD shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub date: String,
    pub money: String,
    pub speed: usize,
    pub paused: bool,
    pub turbo: u32,
    pub line: String,
    /// The line is the pulsing "message waiting" (or the story's) text.
    pub waiting: bool,
    pub history: Vec<String>,
    pub history_open: bool,
    pub screens: Vec<ScreenItem>,
    pub research: Research,
    pub info: Option<Info>,
    /// The minimap and the console (off under the production view, the transfer and the fleet's panel).
    pub mini: Option<Mini>,
    /// The console's tooltips by hot-spot (1..=7), the tanks' (0xa) and production's (0xb) at 8 and 9.
    pub tips: [String; 10],
    pub labels: Labels,
    pub keys: Keys,
    /// The blink's phase (the selected dots, the redirect mark).
    pub blink: bool,
    /// The player's empire colour: the plates' edges and what is on.
    pub race: Color32,
}

impl View {
    /// The tooltips' keys from the key bindings: the screens', the pause's, the speeds' and the zoom's.
    pub fn bind_keys(&mut self, b: &crate::bindings::Bindings) {
        let key = |id: &str| crate::bindings::ACTIONS.iter().position(|a| a.id == id).and_then(|i| b.0.get(i)).and_then(|k| crate::bindings::key_name(*k)).unwrap_or_default().to_string();
        self.keys = Keys { pause: key("Pause"), speed: [key("Speed1"), key("Speed2"), key("Speed3")], zoom: [key("StarZoomIn"), key("StarZoomOut")] };
        for s in &mut self.screens {
            let id = match s.n {
                1 => "Help",
                2 => "Starmap",
                3 => "Colony",
                4 => "Research",
                5 => "Design",
                6 => "Diplomacy",
                7 => "Spy",
                8 => "Trade",
                9 => "Info",
                10 => "MessagesF",
                13 => "Options",
                _ => continue,
            };
            s.key = key(id);
        }
    }
}

/// What stays between frames.
#[derive(Debug, Clone, Default)]
pub struct State {
    research_open: bool,
}

/// The HUD's colours this frame.
#[derive(Debug, Clone, Copy)]
struct Ink {
    text: Color32,
    dim: Color32,
    /// The player's colour, lifted toward the text so it reads on the dark plates.
    accent: Color32,
    fill: Color32,
    edge: Color32,
}

impl Ink {
    fn new(ctx: &egui::Context, race: Color32) -> Self {
        let v = &ctx.style().visuals;
        let text = v.widgets.inactive.fg_stroke.color;
        let base = if race.a() == 0 || race == Color32::BLACK { v.hyperlink_color } else { race };
        let accent = lerp(base, text, 0.3);
        let [r, g, b, _] = v.window_fill.to_array();
        let fill = Color32::from_rgba_unmultiplied((r as f32 * 0.7) as u8, (g as f32 * 0.7) as u8, (b as f32 * 0.75) as u8, 214);
        Self { text, dim: v.weak_text_color(), accent, fill, edge: accent.gamma_multiply(0.38) }
    }
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// The vector icons the HUD's buttons draw.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Icon {
    Pause,
    Play(u8),
    RotateLeft,
    RotateRight,
    ZoomIn,
    ZoomOut,
    TiltUp,
    TiltDown,
    TopView,
    SideView,
    Bolt,
    Tanks,
    Factory,
    Details,
    Split,
    Mail,
    /// A screen of the screen menu by its number (`AktKep`).
    Screen(i32),
}

/// `pts` (in `k` units around `m`) as one line, closed or not.
fn path(p: &egui::Painter, m: Pos2, k: f32, pts: &[(f32, f32)], closed: bool, s: Stroke) {
    let v: Vec<Pos2> = pts.iter().map(|&(x, y)| pos2(m.x + x * k, m.y + y * k)).collect();
    p.add(if closed { Shape::closed_line(v, s) } else { Shape::line(v, s) });
}

/// An arc around `c` from `a0` to `a1` (radians, y down), `rx` by `ry`.
fn arc(c: Pos2, rx: f32, ry: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    (0..=20).map(|j| {
        let a = a0 + (a1 - a0) * j as f32 / 20.0;
        pos2(c.x + a.cos() * rx, c.y + a.sin() * ry)
    }).collect()
}

fn icon(p: &egui::Painter, r: Rect, i: Icon, c: Color32) {
    use std::f32::consts::{PI, TAU};
    let s = Stroke::new(1.4f32, c);
    let m = r.center();
    let k = r.height().min(r.width()) * 0.3;
    let tri = |at: Pos2, w: f32| vec![pos2(at.x - w * 0.5, at.y - k), pos2(at.x + w * 0.5, at.y), pos2(at.x - w * 0.5, at.y + k)];
    match i {
        Icon::Pause => {
            for dx in [-0.45, 0.45] {
                p.rect_filled(Rect::from_center_size(pos2(m.x + dx * k, m.y), vec2(k * 0.45, k * 1.9)), 0.0, c);
            }
        }
        Icon::Play(n) => {
            let w = k * 1.15;
            let n = n.max(1) as f32;
            let step = w * 0.62;
            let x0 = m.x - (n - 1.0) * step / 2.0;
            for j in 0..n as usize {
                p.add(Shape::convex_polygon(tri(pos2(x0 + j as f32 * step, m.y), w), c, Stroke::NONE));
            }
        }
        Icon::RotateLeft | Icon::RotateRight => {
            let dir = if i == Icon::RotateLeft { -1.0 } else { 1.0 };
            let pts: Vec<Pos2> = (0..=16).map(|j| {
                let a = PI * (0.15 + 1.2 * j as f32 / 16.0);
                pos2(m.x + dir * a.cos() * k, m.y - a.sin() * k * 0.9 + k * 0.25)
            }).collect();
            let end = pts[0];
            p.add(Shape::line(pts, s));
            p.add(Shape::convex_polygon(vec![pos2(end.x - dir * k * 0.45, end.y - k * 0.1), pos2(end.x + dir * k * 0.25, end.y - k * 0.15), pos2(end.x - dir * k * 0.05, end.y + k * 0.5)], c, Stroke::NONE));
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            let o = pos2(m.x - k * 0.2, m.y - k * 0.2);
            p.circle_stroke(o, k * 0.75, s);
            p.line_segment([pos2(o.x + k * 0.55, o.y + k * 0.55), pos2(m.x + k, m.y + k)], Stroke::new(2.0f32, c));
            p.line_segment([pos2(o.x - k * 0.4, o.y), pos2(o.x + k * 0.4, o.y)], s);
            if i == Icon::ZoomIn {
                p.line_segment([pos2(o.x, o.y - k * 0.4), pos2(o.x, o.y + k * 0.4)], s);
            }
        }
        Icon::TiltUp | Icon::TiltDown => {
            let d = if i == Icon::TiltUp { -1.0 } else { 1.0 };
            p.add(Shape::convex_polygon(vec![pos2(m.x, m.y + d * k), pos2(m.x - k, m.y - d * k * 0.2), pos2(m.x + k, m.y - d * k * 0.2)], Color32::TRANSPARENT, s));
            p.line_segment([pos2(m.x - k * 1.1, m.y - d * k * 0.75), pos2(m.x + k * 1.1, m.y - d * k * 0.75)], s);
        }
        Icon::TopView => {
            p.circle_stroke(m, k, s);
            p.circle_filled(m, k * 0.3, c);
        }
        Icon::SideView => {
            p.add(Shape::closed_line(arc(m, k * 1.1, k * 0.35, 0.0, TAU), s));
            p.circle_filled(pos2(m.x, m.y - k * 0.1), k * 0.32, c);
        }
        Icon::Bolt => path(p, m, k, &[(0.2, -1.0), (-0.55, 0.15), (0.0, 0.15), (-0.2, 1.0), (0.55, -0.15), (0.0, -0.15)], true, s),
        Icon::Tanks => {
            p.rect_stroke(Rect::from_center_size(pos2(m.x, m.y + k * 0.35), vec2(k * 2.0, k * 0.8)), k * 0.4, s, egui::StrokeKind::Middle);
            p.rect_filled(Rect::from_center_size(pos2(m.x - k * 0.1, m.y - k * 0.3), vec2(k * 0.9, k * 0.55)), 1.0, c);
            p.line_segment([pos2(m.x + k * 0.3, m.y - k * 0.35), pos2(m.x + k * 1.15, m.y - k * 0.55)], s);
        }
        Icon::Factory => path(p, m, k, &[(-1.0, 1.0), (-1.0, -0.2), (-0.35, 0.2), (-0.35, -0.2), (0.3, 0.2), (0.3, -1.0), (0.75, -1.0), (0.75, 1.0)], true, s),
        Icon::Details => {
            p.circle_stroke(m, k, s);
            p.circle_filled(pos2(m.x, m.y - k * 0.5), k * 0.13, c);
            p.line_segment([pos2(m.x, m.y - k * 0.15), pos2(m.x, m.y + k * 0.55)], Stroke::new(1.6f32, c));
        }
        Icon::Split => {
            path(p, m, k, &[(0.0, 1.0), (0.0, 0.1), (-0.75, -0.8)], false, s);
            p.line_segment([pos2(m.x, m.y + k * 0.1), pos2(m.x + k * 0.75, m.y - k * 0.8)], s);
            path(p, m, k, &[(-0.75, -0.35), (-0.75, -0.8), (-0.3, -0.8)], false, s);
            path(p, m, k, &[(0.75, -0.35), (0.75, -0.8), (0.3, -0.8)], false, s);
        }
        Icon::Mail => {
            p.rect_stroke(Rect::from_center_size(m, vec2(k * 2.0, k * 1.4)), 0.0, s, egui::StrokeKind::Middle);
            path(p, m, k, &[(-1.0, -0.7), (0.0, 0.1), (1.0, -0.7)], false, s);
        }
        Icon::Screen(n) => match n {
            // Help: a question mark in a ring.
            1 => {
                p.circle_stroke(m, k * 1.05, s);
                let mut v = arc(pos2(m.x, m.y - k * 0.3), k * 0.38, k * 0.35, PI * 1.05, PI * 2.35);
                v.push(pos2(m.x, m.y + k * 0.25));
                p.add(Shape::line(v, s));
                p.circle_filled(pos2(m.x, m.y + k * 0.6), k * 0.12, c);
            }
            // The starmap: stars joined by routes.
            2 => {
                let stars = [(-0.95, 0.55), (-0.25, -0.35), (0.45, 0.35), (0.95, -0.8)];
                path(p, m, k, &stars, false, Stroke::new(1.0f32, c.gamma_multiply(0.7)));
                for (j, (x, y)) in stars.into_iter().enumerate() {
                    p.circle_filled(pos2(m.x + x * k, m.y + y * k), k * if j == 1 { 0.3 } else { 0.22 }, c);
                }
            }
            // A colony: a dome with its mast.
            3 => {
                let base = m.y + k * 0.7;
                p.add(Shape::line(arc(pos2(m.x, base), k * 0.95, k * 1.0, PI, TAU), s));
                p.line_segment([pos2(m.x - k * 0.55, base - k * 0.45), pos2(m.x + k * 0.55, base - k * 0.45)], Stroke::new(1.0f32, c.gamma_multiply(0.7)));
                p.line_segment([pos2(m.x - k * 1.1, base), pos2(m.x + k * 1.1, base)], s);
                p.line_segment([pos2(m.x, base - k * 1.0), pos2(m.x, base - k * 1.6)], s);
            }
            // Research: a flask.
            4 => {
                path(p, m, k, &[(-0.3, -1.0), (-0.3, -0.3), (-0.95, 0.85), (0.95, 0.85), (0.3, -0.3), (0.3, -1.0)], false, s);
                p.line_segment([pos2(m.x - k * 0.5, m.y - k), pos2(m.x + k * 0.5, m.y - k)], s);
                p.line_segment([pos2(m.x - k * 0.62, m.y + k * 0.25), pos2(m.x + k * 0.62, m.y + k * 0.25)], s);
            }
            // Design: a ship's outline.
            5 => path(p, m, k, &[(0.0, -1.05), (0.35, -0.2), (0.95, 0.5), (0.35, 0.45), (0.25, 0.95), (-0.25, 0.95), (-0.35, 0.45), (-0.95, 0.5), (-0.35, -0.2)], true, s),
            // Diplomacy: two rings joined.
            6 => {
                p.circle_stroke(pos2(m.x - k * 0.4, m.y), k * 0.65, s);
                p.circle_stroke(pos2(m.x + k * 0.4, m.y), k * 0.65, s);
            }
            // Spies: an eye.
            7 => {
                let mut v = arc(pos2(m.x, m.y + k * 0.9), k * 1.35, k * 1.5, PI * 1.22, PI * 1.78);
                v.extend(arc(pos2(m.x, m.y - k * 0.9), k * 1.35, k * 1.5, PI * 0.22, PI * 0.78));
                p.add(Shape::closed_line(v, s));
                p.circle_filled(m, k * 0.32, c);
            }
            // Trade: two arrows both ways.
            8 => {
                path(p, m, k, &[(-1.0, -0.4), (0.95, -0.4)], false, s);
                path(p, m, k, &[(0.5, -0.85), (0.95, -0.4), (0.5, 0.05)], false, s);
                path(p, m, k, &[(1.0, 0.45), (-0.95, 0.45)], false, s);
                path(p, m, k, &[(-0.5, 0.0), (-0.95, 0.45), (-0.5, 0.9)], false, s);
            }
            // Information: a bar chart.
            9 => {
                for (j, h) in [0.7, 1.3, 1.9].iter().enumerate() {
                    let x = m.x - k * 0.75 + j as f32 * k * 0.75;
                    p.rect_filled(Rect::from_min_max(pos2(x - k * 0.22, m.y + k * 0.95 - h * k), pos2(x + k * 0.22, m.y + k * 0.95)), 0.0, c);
                }
            }
            10 => icon(p, r, Icon::Mail, c),
            // Options: a gear.
            _ => {
                p.circle_stroke(m, k * 0.55, s);
                for j in 0..8 {
                    let a = TAU * j as f32 / 8.0;
                    let (x, y) = (a.cos(), a.sin());
                    p.line_segment([pos2(m.x + x * k * 0.62, m.y + y * k * 0.62), pos2(m.x + x * k * 1.05, m.y + y * k * 1.05)], Stroke::new(2.0f32, c));
                }
            }
        },
    }
}

/// A plate's outline: `rect` with its top-left and bottom-right corners cut by `cut`.
fn cut_corners(r: Rect, cut: f32) -> Vec<Pos2> {
    vec![pos2(r.left() + cut, r.top()), pos2(r.right(), r.top()), pos2(r.right(), r.bottom() - cut), pos2(r.right() - cut, r.bottom()), pos2(r.left(), r.bottom()), pos2(r.left(), r.top() + cut)]
}

/// The console plate under `r`: a soft shadow, the dark fill, a hairline edge and a top bevel, the cut corners marked
/// in the accent.
fn plate_shape(r: Rect, ink: &Ink) -> Shape {
    let cut = 7.0f32.min(r.height() * 0.3);
    let mark = Stroke::new(1.5f32, ink.accent);
    let tl = (pos2(r.left(), r.top() + cut), pos2(r.left() + cut, r.top()));
    let br = (pos2(r.right(), r.bottom() - cut), pos2(r.right() - cut, r.bottom()));
    Shape::Vec(vec![
        Shadow { offset: [0, 3], blur: 12, spread: 0, color: Color32::from_black_alpha(110) }.as_shape(r, CornerRadius::same(2)).into(),
        Shape::convex_polygon(cut_corners(r, cut), ink.fill, Stroke::new(1.0f32, ink.edge)),
        Shape::line_segment([pos2(r.left() + cut + 1.0, r.top() + 1.5), pos2(r.right() - 1.5, r.top() + 1.5)], Stroke::new(1.0f32, Color32::from_white_alpha(16))),
        Shape::line(vec![pos2(tl.0.x, tl.0.y + 8.0), tl.0, tl.1, pos2(tl.1.x + 14.0, tl.1.y)], mark),
        Shape::line(vec![pos2(br.0.x, br.0.y - 8.0), br.0, br.1, pos2(br.1.x - 14.0, br.1.y)], mark),
    ])
}

/// `add` on a console plate with `margin` inside.
fn plate<R>(ui: &mut egui::Ui, ink: &Ink, margin: Margin, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let under = ui.painter().add(Shape::Noop);
    let r = egui::Frame::new().inner_margin(margin).show(ui, add);
    ui.painter().set(under, plate_shape(r.response.rect, ink));
    r
}

/// A tooltip: the name and, when bound, its key in a small key cap.
fn tip(resp: egui::Response, name: &str, key: &str, ink: &Ink) -> egui::Response {
    if name.is_empty() {
        return resp;
    }
    resp.on_hover_ui(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(name).color(ink.text));
            if !key.is_empty() {
                ui.add_space(6.0);
                let g = ui.painter().layout_no_wrap(key.to_string(), FontId::proportional(11.0), ink.accent);
                let (r, _) = ui.allocate_exact_size(g.size() + vec2(8.0, 2.0), Sense::hover());
                ui.painter().rect_stroke(r, 2.0, Stroke::new(1.0f32, ink.edge), egui::StrokeKind::Inside);
                ui.painter().galley(r.center() - g.size() / 2.0, g, ink.accent);
            }
        });
    })
}

/// A square icon button: hover fades in a lit fill and edge, `on` marks the chosen one with an accent bar below.
fn icon_button(ui: &mut egui::Ui, ink: &Ink, i: Icon, on: bool, enabled: bool, size: f32) -> egui::Response {
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
    icon(p, r.shrink(1.0), i, fg);
    resp
}

/// Clicked with the left or the right button.
fn either(r: &egui::Response) -> bool {
    r.clicked() || r.secondary_clicked()
}

/// A thin upright rule between groups on a plate.
fn rule(ui: &mut egui::Ui, ink: &Ink, h: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(9.0, h), Sense::hover());
    ui.painter().line_segment([pos2(r.center().x, r.top() + 2.0), pos2(r.center().x, r.bottom() - 2.0)], Stroke::new(1.0f32, ink.edge));
}

/// The HUD's points: the gap to the screen's edges, the top plates' buttons and inside margin.
const EDGE: f32 = 8.0;
const BUTTON: f32 = 20.0;
const TOP: Margin = Margin { left: 9, right: 9, top: 5, bottom: 5 };
/// The minimap's width (its height keeps the classic window's ratio), the camera buttons under it.
const MAP_W: f32 = 168.0;
const CAM: f32 = 19.0;
const CAM_GAP: f32 = 2.0;
const NAV: Margin = Margin::same(7);
/// The message line's widest.
const LINE_W: f32 = 520.0;
/// The selection panel's inside width.
const INFO_W: f32 = 228.0;

/// One frame of the HUD over the starmap; the acts in the order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut acts = Vec::new();
    let ink = Ink::new(ctx, v.race);
    ctx.style_mut(|s| {
        s.visuals.window_stroke = Stroke::new(1.0f32, ink.edge);
        s.visuals.window_fill = Color32::from_rgba_unmultiplied(ink.fill.r(), ink.fill.g(), ink.fill.b(), 240);
        s.visuals.widgets.hovered.weak_bg_fill = ink.accent.gamma_multiply(0.22);
        s.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0f32, ink.accent.gamma_multiply(0.7));
        s.visuals.selection.bg_fill = ink.accent.gamma_multiply(0.3);
    });
    // Top left: pause and the speeds, the date, the money.
    let left = egui::Area::new(egui::Id::new("smh-left")).fixed_pos(pos2(EDGE, EDGE)).fade_in(false).show(ctx, |ui| {
        plate(ui, &ink, TOP, |ui| {
            ui.horizontal_centered(|ui| {
                ui.set_height(BUTTON);
                ui.spacing_mut().item_spacing.x = 1.0;
                let r = icon_button(ui, &ink, Icon::Pause, v.paused, true, BUTTON);
                if either(&tip(r, &v.labels.pause, &v.keys.pause, &ink)) {
                    acts.push(Act::Pause);
                }
                for n in 1..=3u8 {
                    let on = !v.paused && v.speed + 1 == n as usize;
                    let r = icon_button(ui, &ink, Icon::Play(n), on, true, BUTTON);
                    if either(&tip(r, &format!("{} {n}", v.labels.speed), &v.keys.speed[n as usize - 1], &ink)) {
                        acts.push(Act::Speed(n as i32));
                    }
                }
                if v.turbo > 1 {
                    ui.add_space(3.0);
                    ui.label(egui::RichText::new(format!("×{}", v.turbo)).size(11.0).strong().color(ink.accent));
                }
                ui.spacing_mut().item_spacing.x = 4.0;
                rule(ui, &ink, BUTTON);
                ui.label(egui::RichText::new(&v.date).size(12.5).color(ink.text.gamma_multiply(0.85)));
                rule(ui, &ink, BUTTON);
                let broke = v.money.starts_with('-');
                ui.label(egui::RichText::new(&v.money).size(15.0).strong().color(if broke { Color32::from_rgb(235, 96, 84) } else { Color32::WHITE }));
            });
        });
    });
    let left = left.response.rect;
    // Top right: the research under way and the screens.
    let mut status = Rect::NOTHING;
    let right = egui::Area::new(egui::Id::new("smh-right")).anchor(Align2::RIGHT_TOP, vec2(-EDGE, EDGE)).fade_in(false).show(ctx, |ui| {
        plate(ui, &ink, TOP, |ui| {
            ui.horizontal_centered(|ui| {
                ui.set_height(BUTTON);
                status = research_status(ui, v, st, &ink);
                ui.spacing_mut().item_spacing.x = 4.0;
                rule(ui, &ink, BUTTON);
                ui.spacing_mut().item_spacing.x = 1.0;
                for s in &v.screens {
                    let r = icon_button(ui, &ink, Icon::Screen(s.n), s.current, s.on, BUTTON);
                    if tip(r, &s.name, &s.key, &ink).clicked() && !s.current {
                        acts.push(Act::Screen(s.n));
                        st.research_open = false;
                    }
                }
            });
        });
    });
    let right = right.response.rect;
    // The message line beside the top left plate (left: the messages screen, right held: the history).
    // (On the first frame the right plate has no place yet: its usual width.)
    let right_x = if right.is_positive() { right.left() } else { ctx.screen_rect().right() - 480.0 };
    let room = right_x - left.right() - 2.0 * EDGE;
    if room > BUTTON + 20.0 {
        message_line(ctx, v, &ink, pos2(left.right() + EDGE * 0.75, left.top()), room.min(LINE_W), left.height(), &mut acts);
    }
    if st.research_open {
        let r = egui::Area::new(egui::Id::new("smh-research")).pivot(Align2::RIGHT_TOP).fixed_pos(pos2(right.right(), right.bottom() + 4.0)).order(egui::Order::Foreground).fade_in(false).show(ctx, |ui| {
            plate(ui, &ink, Margin::same(10), |ui| research_menu(ui, v, st, &ink, &mut acts));
        });
        close_outside(ctx, st, &[r.response.rect, status]);
    }
    // The selection, bottom left.
    if let Some(info) = &v.info {
        egui::Area::new(egui::Id::new("smh-info")).anchor(Align2::LEFT_BOTTOM, vec2(EDGE, -EDGE)).fade_in(false).show(ctx, |ui| {
            plate(ui, &ink, Margin::symmetric(10, 8), |ui| info_card(ui, info, v, &ink, &mut acts));
        });
    }
    // The minimap with the camera's buttons under it, bottom right.
    if let Some(mini) = &v.mini {
        egui::Area::new(egui::Id::new("smh-mini")).anchor(Align2::RIGHT_BOTTOM, vec2(-EDGE, -EDGE)).fade_in(false).show(ctx, |ui| {
            plate(ui, &ink, NAV, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                minimap(ui, mini, v, &ink, &mut acts);
                camera_buttons(ui, v, &ink, &mut acts);
            });
        });
    } else {
        acts.push(Act::Hold(None));
    }
    acts
}

/// The running research's name, per cent and progress, or the idle text; a click opens the quick-research menu.
fn research_status(ui: &mut egui::Ui, v: &View, st: &mut State, ink: &Ink) -> Rect {
    let r = &v.research;
    let (rect, resp) = ui.allocate_exact_size(vec2(196.0, BUTTON), Sense::click());
    let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let p = ui.painter();
    if st.research_open || hot > 0.0 {
        let k = if st.research_open { 1.0 } else { hot };
        p.rect_filled(rect, 2.0, Color32::from_white_alpha((14.0 * k) as u8));
        p.rect_stroke(rect, 2.0, Stroke::new(1.0f32, ink.accent.gamma_multiply(0.7 * k)), egui::StrokeKind::Inside);
    }
    let colour = if r.current.is_some() { ink.accent } else if r.researchable { ink.text.gamma_multiply(0.85) } else { ink.dim };
    icon(p, Rect::from_min_size(rect.min + vec2(1.0, 2.0), vec2(16.0, 16.0)), Icon::Bolt, colour);
    let (name, pct) = match &r.current {
        Some((_, name, pct, _)) => (name.clone(), Some(*pct)),
        None => (v.labels.idle.clone(), None),
    };
    let text_x = rect.left() + 21.0;
    let mut text_r = rect.right() - 4.0;
    if let Some(pct) = pct {
        let g = p.layout_no_wrap(format!("{pct}%"), FontId::proportional(12.0), ink.text);
        text_r -= g.size().x + 6.0;
        p.galley(pos2(rect.right() - 4.0 - g.size().x, rect.top() + 1.0), g, ink.text);
        // The progress under the name.
        let track = Rect::from_min_max(pos2(text_x, rect.bottom() - 4.0), pos2(rect.right() - 4.0, rect.bottom() - 2.0));
        p.rect_filled(track, 0.0, ink.edge.gamma_multiply(0.6));
        p.rect_filled(Rect::from_min_max(track.min, pos2(track.left() + track.width() * (pct.clamp(0, 100) as f32 / 100.0), track.bottom())), 0.0, ink.accent);
    }
    let g = p.layout_no_wrap(name, FontId::proportional(12.5), colour);
    let y = if pct.is_some() { rect.top() + 1.0 } else { rect.center().y - g.size().y / 2.0 };
    p.with_clip_rect(Rect::from_min_max(pos2(text_x, rect.top()), pos2(text_r, rect.bottom()))).galley(pos2(text_x, y), g, colour);
    if tip(resp, &v.labels.research, "", ink).clicked() {
        st.research_open = !st.research_open;
    }
    rect
}

/// The message line: the original's scrolling message (or the chat entry, the error, the waiting text), else the
/// latest of the history, dim.
fn message_line(ctx: &egui::Context, v: &View, ink: &Ink, at: Pos2, width: f32, height: f32, acts: &mut Vec<Act>) {
    let (text, colour) = match (v.line.is_empty(), v.history.last()) {
        (false, _) if v.waiting => (v.line.as_str(), ink.accent.gamma_multiply(if v.blink { 1.0 } else { 0.55 })),
        (false, _) => (v.line.as_str(), ink.text),
        (true, Some(h)) => (h.as_str(), ink.dim),
        (true, None) => ("", ink.dim),
    };
    // Only the mail icon when there is nothing to show or no room for text.
    let inner = vec2(if text.is_empty() || width < 120.0 { BUTTON } else { width - (TOP.left + TOP.right) as f32 }, height - (TOP.top + TOP.bottom) as f32);
    egui::Area::new(egui::Id::new("smh-line")).fixed_pos(at).fade_in(false).show(ctx, |ui| {
        plate(ui, ink, TOP, |ui| {
            let (r, resp) = ui.allocate_exact_size(inner, Sense::click());
            let hot = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
            let mail = Rect::from_min_size(r.min, vec2(BUTTON, BUTTON));
            icon(ui.painter(), mail.shrink(1.0), Icon::Mail, lerp(if v.waiting { ink.accent } else { ink.text.gamma_multiply(0.8) }, ink.text, hot));
            let g = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(12.5), colour);
            let clip = Rect::from_min_max(pos2(mail.right() + 6.0, r.top()), r.max);
            ui.painter().with_clip_rect(clip).galley(pos2(clip.left(), r.center().y - g.size().y / 2.0), g, colour);
            if hot > 0.0 {
                ui.painter().rect_stroke(r.expand2(vec2(4.0, 2.0)), 2.0, Stroke::new(1.0f32, ink.accent.gamma_multiply(0.6 * hot)), egui::StrokeKind::Inside);
            }
            let name = v.screens.iter().find(|s| s.n == 10);
            let resp = tip(resp, name.map_or("", |s| s.name.as_str()), name.map_or("", |s| s.key.as_str()), ink);
            let right_held = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
            if right_held != v.history_open {
                acts.push(Act::History(right_held));
            }
            // The left button held over the line asks every frame (`MOUSE_SelectScreens`, `top_bar_frame`).
            if resp.contains_pointer() && ui.input(|i| i.pointer.primary_down()) && resp.is_pointer_button_down_on() {
                acts.push(Act::Messages);
            }
            if v.history_open && !v.history.is_empty() {
                egui::Area::new(egui::Id::new("smh-history")).fixed_pos(pos2(at.x, at.y + height + 4.0)).order(egui::Order::Foreground).fade_in(false).show(ui.ctx(), |ui| {
                    plate(ui, ink, Margin::symmetric(10, 7), |ui| {
                        ui.set_width(width.max(300.0) - 20.0);
                        ui.spacing_mut().item_spacing.y = 3.0;
                        for h in &v.history {
                            ui.label(egui::RichText::new(h).size(12.5).color(ink.text));
                        }
                    });
                });
            }
        });
    });
}

/// A click anywhere but the menu and its button closes the menu.
fn close_outside(ctx: &egui::Context, st: &mut State, keep: &[Rect]) {
    let (clicked, at) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    if clicked && at.is_some_and(|p| !keep.iter().any(|r| r.contains(p))) {
        st.research_open = false;
    }
}

fn research_menu(ui: &mut egui::Ui, v: &View, st: &mut State, ink: &Ink, acts: &mut Vec<Act>) {
    let r = &v.research;
    let cols = r.headers.len().max(1);
    egui::Grid::new("smh-research-grid").num_columns(cols).spacing(vec2(12.0, 1.0)).show(ui, |ui| {
        for h in &r.headers {
            ui.label(egui::RichText::new(h.to_uppercase()).size(11.0).strong().color(ink.accent));
        }
        ui.end_row();
        let rows = (0..cols).map(|c| r.items.iter().filter(|i| i.2 == c).count()).max().unwrap_or(0);
        for row in 0..rows {
            for c in 0..cols {
                match r.items.iter().filter(|i| i.2 == c).nth(row) {
                    Some((t, name, _, ok, short, cost)) => {
                        let colour = if *short || r.current.is_some() { ink.dim } else { ink.text };
                        let b = egui::Button::new(egui::RichText::new(name).color(colour)).frame_when_inactive(false).min_size(vec2(124.0, 19.0));
                        let resp = ui.add_enabled(*ok, b);
                        let resp = if *ok { resp.on_hover_text(format!("{}: {cost}", r.footer[3])) } else { resp };
                        if resp.clicked() {
                            acts.push(Act::Start(*t));
                            st.research_open = false;
                        }
                    }
                    None => {
                        ui.label("");
                    }
                }
            }
            ui.end_row();
        }
    });
    if let Some((t, name, pct, left)) = &r.current {
        ui.add_space(4.0);
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0.0, ink.edge);
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}: {name}", r.footer[0])).color(ink.text));
            ui.add(egui::ProgressBar::new(*pct as f32 / 100.0).desired_width(90.0).desired_height(12.0).fill(ink.accent.gamma_multiply(0.7)).text(egui::RichText::new(format!("{pct}%")).size(11.0)));
            if ui.button(&r.footer[1]).clicked() {
                acts.push(Act::Stop(*t));
                st.research_open = false;
            }
        });
        ui.label(egui::RichText::new(format!("{}: {left}", r.footer[2])).size(11.0).color(ink.dim));
    }
}

fn info_card(ui: &mut egui::Ui, info: &Info, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    ui.set_width(INFO_W);
    let body = ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
            let c = r.center();
            if info.planet {
                ui.painter().circle_filled(c, 5.0, info.colour);
                ui.painter().circle_stroke(c, 7.0, Stroke::new(1.0f32, info.colour.gamma_multiply(0.55)));
            } else {
                ui.painter().add(Shape::convex_polygon(vec![pos2(c.x + 7.0, c.y), pos2(c.x - 6.0, c.y - 5.5), pos2(c.x - 3.0, c.y), pos2(c.x - 6.0, c.y + 5.5)], info.colour, Stroke::NONE));
            }
            ui.add(egui::Label::new(egui::RichText::new(&info.title).size(15.0).strong().color(Color32::WHITE)).truncate());
        });
        if let Some((l, c)) = info.lines.first() {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.add(egui::Label::new(egui::RichText::new(l).size(11.5).color(*c)).truncate());
            });
        }
        ui.add_space(4.0);
        let (line, _) = ui.allocate_exact_size(vec2(INFO_W, 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0.0, ink.edge);
        ui.add_space(4.0);
        for (l, c) in info.lines.iter().skip(1) {
            ui.add(egui::Label::new(egui::RichText::new(l).size(12.0).color(*c)).truncate());
        }
        if !info.stats.is_empty() {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (k, val) in &info.stats {
                    ui.label(egui::RichText::new(k).size(11.0).color(ink.dim));
                    ui.label(egui::RichText::new(val).size(12.5).strong().color(ink.text));
                    ui.add_space(8.0);
                }
            });
        }
    });
    // The panel itself is the info box (hot-spot 8): left and right click as the original's.
    let card = ui.interact(body.response.rect, ui.id().with("smh-info-card"), Sense::click()).on_hover_text(&info.tip);
    if card.clicked() {
        acts.push(Act::Info(false));
    }
    if card.secondary_clicked() {
        acts.push(Act::Info(true));
    }
    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let size = 22.0;
        if tip(icon_button(ui, ink, Icon::Details, false, true, size), &info.left, "", ink).clicked() {
            acts.push(Act::Info(false));
        }
        if let Some(r) = &info.right {
            let i = if info.planet { Icon::Screen(3) } else { Icon::Split };
            let key = if info.planet { v.screens.iter().find(|s| s.n == 3).map_or("", |s| s.key.as_str()) } else { "" };
            if tip(icon_button(ui, ink, i, false, true, size), r, key, ink).clicked() {
                acts.push(Act::Info(true));
            }
        }
        if info.tanks && tip(icon_button(ui, ink, Icon::Tanks, false, true, size), &v.tips[8], "", ink).clicked() {
            acts.push(Act::Panel(10));
        }
        if info.product && tip(icon_button(ui, ink, Icon::Factory, false, true, size), &v.tips[9], "", ink).clicked() {
            acts.push(Act::Panel(11));
        }
        if info.redirect {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(v.labels.redirect.to_uppercase()).size(11.0).strong().color(ink.accent.gamma_multiply(if v.blink { 1.0 } else { 0.45 })));
            });
        }
    });
}

/// The camera's buttons in a strip under the minimap: rotate, zoom and tilt held (the left button's way, the right
/// button the other way as on the console), the top and side views clicked.
fn camera_buttons(ui: &mut egui::Ui, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    let mut held = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = CAM_GAP;
        ui.add_space((MAP_W - 8.0 * CAM - 7.0 * CAM_GAP) / 2.0);
        let none = String::new();
        let buttons = [(Icon::RotateLeft, 3, true, &none), (Icon::RotateRight, 2, true, &none), (Icon::ZoomIn, 5, true, &v.keys.zoom[0]), (Icon::ZoomOut, 5, false, &v.keys.zoom[1]), (Icon::TiltUp, 6, true, &none), (Icon::TiltDown, 6, false, &none)];
        for (i, spot, left, key) in buttons {
            let r = tip(icon_button(ui, ink, i, false, true, CAM), &v.tips[spot as usize], key, ink);
            if r.is_pointer_button_down_on() && r.contains_pointer() {
                let right = ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary) && !i.pointer.primary_down());
                held = Some((spot, left != right));
            }
        }
        for (i, spot) in [(Icon::TopView, 1), (Icon::SideView, 4)] {
            if either(&tip(icon_button(ui, ink, i, false, true, CAM), &v.tips[spot as usize], "", ink)) {
                acts.push(Act::View(spot));
            }
        }
    });
    acts.push(Act::Hold(held));
}

fn minimap(ui: &mut egui::Ui, mini: &Mini, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    let size = vec2(MAP_W, MAP_W * MINI[3] / MINI[2]);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, Color32::from_black_alpha(150));
    // A faint grid: the console's screen.
    for j in 1..4 {
        let (x, y) = (r.left() + r.width() * j as f32 / 4.0, r.top() + r.height() * j as f32 / 4.0);
        p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(1.0f32, ink.edge.gamma_multiply(0.35)));
        p.line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0f32, ink.edge.gamma_multiply(0.35)));
    }
    let at = |u: [f32; 2]| pos2(r.left() + u[0] * r.width(), r.top() + u[1] * r.height());
    for &(u, c, sel) in &mini.fleets {
        let c = if sel && !v.blink { c.gamma_multiply(0.3) } else { c };
        p.rect_filled(Rect::from_center_size(at(u), vec2(2.6, 1.4)), 0.0, c);
    }
    for &(u, c, sel) in &mini.planets {
        let q = at(u);
        p.circle_filled(q, 1.6, c);
        if sel {
            p.circle_stroke(q, 4.0, Stroke::new(1.0f32, if v.blink { c } else { c.gamma_multiply(0.4) }));
        }
    }
    let outline: Vec<Pos2> = [0, 1, 3, 2].iter().filter_map(|&i| mini.view[i].map(at)).collect();
    if outline.len() == 4 {
        p.add(Shape::closed_line(outline, Stroke::new(1.0f32, ink.accent)));
    }
    p.rect_stroke(r, 0.0, Stroke::new(1.0f32, ink.edge), egui::StrokeKind::Inside);
    let resp = tip(resp, &v.tips[7], "", ink);
    let (down, first) = ui.input(|i| (i.pointer.any_down(), i.pointer.any_pressed()));
    if let (true, Some(q)) = (resp.is_pointer_button_down_on() && down, resp.interact_pointer_pos()) {
        acts.push(Act::Minimap(mini_virtual([(q.x - r.left()) / r.width(), (q.y - r.top()) / r.height()]), first));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The minimap's unit square maps the classic window, so a click lands where hot-spot 7's would.
    #[test]
    fn minimap_maps_the_classic_window() {
        let p = [-150.0, 90.0];
        let (q, w) = (mini_virtual(mini_unit(p)), crate::starmap_hud::minimap_point(p));
        assert!((q[0] - w[0]).abs() < 1e-3 && (q[1] - w[1]).abs() < 1e-3, "{q:?} {w:?}");
        assert_eq!(mini_virtual([0.0, 0.0]), [274.0, 388.0]);
        assert_eq!(mini_virtual([2.0, -1.0]), [366.0, 388.0]);
    }

    #[test]
    fn money_groups_thousands() {
        assert_eq!(money(109090.0), "$109\u{2009}090");
        assert_eq!(money(999.0), "$999");
        assert_eq!(money(-1234567.0), "-$1\u{2009}234\u{2009}567");
    }

    fn view() -> View {
        View {
            date: "2327-10-23".into(),
            money: "$109090".into(),
            line: "hello".into(),
            screens: vec![ScreenItem { n: 3, name: "Colony".into(), key: "F2".into(), on: true, current: false }],
            info: Some(Info { title: "Magellan 10".into(), planet: true, product: true, tanks: true, left: "Details".into(), right: Some("Colony".into()), ..Default::default() }),
            mini: Some(Mini::default()),
            ..Default::default()
        }
    }

    const SIZE: (u32, u32) = (1920, 1080);

    /// Run frames of the HUD with the pointer events of `step`, gathering the acts.
    /// A frame's pointer (none: the found target) and button (index, down).
    type Step = (Option<Pos2>, Option<(u8, bool)>);

    fn drive(v: &View, events: &[Step], find: impl Fn(&egui::Context) -> Pos2) -> Vec<Act> {
        let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        let style = crate::gui::style(&crate::theme::Theme::modern());
        let mut st = State::default();
        let mut all = Vec::new();
        let k = crate::gui::Gui::scale(SIZE.1);
        let mut target = None;
        // Anchored areas are sized on their first frames and placed after.
        for _ in 0..3 {
            g.run(SIZE, &style, |ctx| {
                show(ctx, v, &mut st);
                target = Some(find(ctx));
            });
        }
        for (at, button) in events {
            let p = at.unwrap_or(target.unwrap());
            g.pointer(Some([p.x * k, p.y * k]), k);
            if let Some((b, down)) = button {
                g.button(*b, *down);
            }
            g.run(SIZE, &style, |ctx| all.extend(show(ctx, v, &mut st)));
        }
        all.retain(|a| !matches!(a, Act::Hold(None) | Act::History(false)));
        all
    }

    fn area(ctx: &egui::Context, id: &str) -> Rect {
        ctx.memory(|m| m.area_rect(egui::Id::new(id))).unwrap()
    }

    /// The pause button is the top left plate's first; a click on it asks for the pause, nothing else.
    #[test]
    fn pause_click() {
        let at = |ctx: &egui::Context| area(ctx, "smh-left").left_top() + vec2(TOP.left as f32 + BUTTON / 2.0, TOP.top as f32 + BUTTON / 2.0);
        let acts = drive(&view(), &[(None, None), (None, Some((0, true))), (None, Some((0, false)))], at);
        assert_eq!(acts, vec![Act::Pause]);
    }

    /// The screens are icons on the top right plate: a click picks the screen.
    #[test]
    fn screen_icon_click() {
        let at = |ctx: &egui::Context| area(ctx, "smh-right").right_top() + vec2(-(TOP.right as f32) - BUTTON / 2.0, TOP.top as f32 + BUTTON / 2.0);
        let acts = drive(&view(), &[(None, None), (None, Some((0, true))), (None, Some((0, false)))], at);
        assert_eq!(acts, vec![Act::Screen(3)]);
    }

    /// The selection panel: left click = the info box's left button, right click = its right one.
    #[test]
    fn info_card_buttons() {
        let at = |ctx: &egui::Context| area(ctx, "smh-info").left_top() + vec2(60.0, 16.0);
        let left = drive(&view(), &[(None, None), (None, Some((0, true))), (None, Some((0, false)))], at);
        assert_eq!(left, vec![Act::Info(false)]);
        let right = drive(&view(), &[(None, None), (None, Some((1, true))), (None, Some((1, false)))], at);
        assert_eq!(right, vec![Act::Info(true)]);
    }

    /// A held zoom-in button holds hot-spot 5 as the left button, every frame until the release.
    #[test]
    fn camera_buttons_hold() {
        let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        let style = crate::gui::style(&crate::theme::Theme::modern());
        let mut st = State::default();
        let v = view();
        let k = crate::gui::Gui::scale(SIZE.1);
        let mut rect = Rect::NOTHING;
        for _ in 0..3 {
            g.run(SIZE, &style, |ctx| {
                show(ctx, &v, &mut st);
                rect = area(ctx, "smh-mini");
            });
        }
        // The strip's third button (zoom in), under the minimap.
        let x0 = NAV.left as f32 + (MAP_W - 8.0 * CAM - 7.0 * CAM_GAP) / 2.0;
        let p = rect.left_bottom() + vec2(x0 + 2.0 * (CAM + CAM_GAP) + CAM / 2.0, -(NAV.bottom as f32) - CAM / 2.0);
        let mut held = Vec::new();
        for step in 0..5 {
            g.pointer(Some([p.x * k, p.y * k]), k);
            match step {
                1 => g.button(0, true),
                3 => g.button(0, false),
                _ => {}
            }
            g.run(SIZE, &style, |ctx| held.extend(show(ctx, &v, &mut st).into_iter().filter_map(|a| if let Act::Hold(h) = a { Some(h) } else { None })));
        }
        assert_eq!(held, vec![None, Some((5, true)), Some((5, true)), None, None]);
    }
}
