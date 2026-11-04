//! Ours (ADR 0065): the modern starmap HUD on the egui layer (ADR 0062), the "Modern starmap HUD" option. The same
//! functions as the original's HUD (re/screens/starmap.md §11, re/screens/hud.md §2, re/screens/input.md §6): the top
//! bar's pause and speeds, date, money, message line and history, its screen and quick-research menus; the selection's
//! short info with its tanks and production buttons; the console's camera buttons and the minimap. The window fills
//! a [`View`] from the same state the classic HUD reads, runs [`show`] once a frame and applies the [`Act`]s through
//! the classic hot-spots' own code, so the orders, sounds and tutorial gates are the same. The radial menu, the select
//! menus and the planet's long info are drawn here too ([`Menus`]) on the original's own places, so its hot-spot code
//! still decides what is pressed; the panels stay the original's.
//!
//! The look is a ship's console: small dark plates with cut corners, a hairline edge and corner marks in the player's
//! colour, drawn icons with their keys in the tooltips. It is laid out in points and follows the shared UI scale.

use crate::gui::ds::{self, Font, IconKey, Plate, Text, Weight};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, Sense, Shadow, Shape, Stroke, Vec2};

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
    /// The long-press menus and the long info.
    pub menus: Menus,
    /// The game's turning globe of the selected planet is rendered: the card and the long info show it.
    pub globe: bool,
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
    /// The long info's controls this frame (the pointer's remap).
    placed: Option<Placed>,
    /// The game's turning globe cut into the card's and the long info's planet ([`crate::fleet_modern::pictures`]).
    pub wells: Vec<([f32; 4], Rect, f32)>,
    /// The planet list moved under the production view's title plate: its bounds and each drawn row with the
    /// middle of the original's (virtual), for the pointer's remap.
    list: Option<([f32; 3], Vec<ListRow>)>,
}

/// A drawn row of the moved planet list and the middle of the original's (virtual).
type ListRow = (Rect, [f32; 2]);

/// Where the modern production view put its title plate and the planet on it (`production_modern`, last frame).
pub const PLATE_ID: &str = "pmh-plate";

/// The info box's globe on the original's virtual screen (`render_globe`'s render target), with a margin.
const GLOBE_SRC: [f32; 4] = [6.0, 433.5, 34.0, 34.0];
/// How much of [`GLOBE_SRC`] the globe fills across.
const GLOBE_FILL: f32 = 30.5 / 34.0;

/// The HUD's colours this frame.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ink {
    pub text: Color32,
    /// The player's colour, lifted toward the text so it reads on the dark plates.
    pub accent: Color32,
    pub fill: Color32,
    pub edge: Color32,
}

impl Ink {
    pub(crate) fn new(ctx: &egui::Context, race: Color32) -> Self {
        let v = &ctx.style().visuals;
        let text = v.widgets.inactive.fg_stroke.color;
        let base = if race.a() == 0 || race == Color32::BLACK { v.hyperlink_color } else { race };
        let accent = lerp(base, text, 0.3);
        let [r, g, b, _] = v.window_fill.to_array();
        let fill = Color32::from_rgba_unmultiplied((r as f32 * 0.7) as u8, (g as f32 * 0.7) as u8, (b as f32 * 0.75) as u8, 214);
        Self { text, accent, fill, edge: accent.gamma_multiply(0.38) }
    }
}

pub(crate) fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// The vector icons the HUD's buttons draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Icon {
    RotateLeft,
    RotateRight,
    ZoomIn,
    ZoomOut,
    TiltUp,
    TiltDown,
    TopView,
    SideView,
    Tanks,
    Factory,
    Details,
    Split,
    Mail,
    /// A screen of the screen menu by its number (`AktKep`).
    Screen(i32),
    /// A radial menu icon by its number (`StarmapRopIkonok`).
    Rop(i32),
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

pub(crate) fn icon(p: &egui::Painter, r: Rect, i: Icon, c: Color32) {
    use std::f32::consts::{PI, TAU};
    let s = Stroke::new(1.4f32, c);
    let m = r.center();
    let k = r.height().min(r.width()) * 0.3;
    match i {
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
        Icon::Rop(n) => match n {
            // Redirect: a curved route ending on a ringed target.
            3 => {
                let mut v = arc(pos2(m.x + k * 0.5, m.y + k * 0.9), k * 1.4, k * 1.6, PI * 1.0, PI * 1.38);
                v.push(pos2(m.x + k * 0.25, m.y - k * 0.62));
                p.add(Shape::line(v, s));
                p.circle_stroke(pos2(m.x + k * 0.55, m.y - k * 0.55), k * 0.42, s);
                p.circle_filled(pos2(m.x + k * 0.55, m.y - k * 0.55), k * 0.14, c);
            }
            // Satellite: a body between two panels; the spy satellite adds its lens.
            5 | 6 => {
                p.rect_stroke(Rect::from_center_size(m, vec2(k * 0.6, k * 0.6)), 0.0, s, egui::StrokeKind::Middle);
                for d in [-1.0, 1.0] {
                    p.line_segment([pos2(m.x + d * k * 0.3, m.y), pos2(m.x + d * k * 0.5, m.y)], s);
                    p.rect_stroke(Rect::from_center_size(pos2(m.x + d * k * 0.8, m.y), vec2(k * 0.6, k * 0.95)), 0.0, s, egui::StrokeKind::Middle);
                }
                if n == 6 {
                    p.circle_filled(pos2(m.x, m.y + k * 0.62), k * 0.2, c);
                    p.line_segment([pos2(m.x, m.y + k * 0.3), pos2(m.x, m.y + k * 0.45)], s);
                }
            }
            // Terraform: a globe with its parallels.
            7 => {
                p.circle_stroke(m, k, s);
                p.add(Shape::line(arc(m, k, k * 0.35, 0.0, PI), s));
                p.add(Shape::line(arc(pos2(m.x, m.y - k * 0.45), k * 0.88, k * 0.22, 0.0, PI), Stroke::new(1.0f32, c.gamma_multiply(0.7))));
                p.add(Shape::line(arc(m, k * 0.4, k, -PI * 0.5, PI * 0.5), Stroke::new(1.0f32, c.gamma_multiply(0.7))));
            }
            // Colonise: a flag planted on a world's rim.
            8 => {
                p.add(Shape::line(arc(pos2(m.x, m.y + k * 2.0), k * 1.3, k * 1.05, PI * 1.2, PI * 1.8), s));
                p.line_segment([pos2(m.x - k * 0.3, m.y + k * 0.95), pos2(m.x - k * 0.3, m.y - k * 1.0)], s);
                p.add(Shape::convex_polygon(vec![pos2(m.x - k * 0.3, m.y - k * 1.0), pos2(m.x + k * 0.75, m.y - k * 0.65), pos2(m.x - k * 0.3, m.y - k * 0.3)], c, Stroke::NONE));
            }
            // The vehicles' info: a tank over a list.
            9 => {
                icon(p, Rect::from_center_size(pos2(m.x, m.y - k * 0.35), r.size() * 0.8), Icon::Tanks, c);
                for (j, w) in [1.0, 0.6].iter().enumerate() {
                    let y = m.y + k * (0.55 + 0.38 * j as f32);
                    p.line_segment([pos2(m.x - k, y), pos2(m.x - k + 2.0 * k * w, y)], Stroke::new(1.2f32, c));
                }
            }
            // Join: two routes merging into one.
            0xe => {
                path(p, m, k, &[(-0.8, -0.9), (0.0, 0.0), (0.8, -0.9)], false, s);
                path(p, m, k, &[(0.0, 0.0), (0.0, 1.0)], false, s);
                path(p, m, k, &[(-0.4, 0.6), (0.0, 1.0), (0.4, 0.6)], false, s);
            }
            // Wormhole: a spiral.
            0x10 => {
                let v: Vec<Pos2> = (0..=40).map(|j| {
                    let t = j as f32 / 40.0;
                    let a = t * TAU * 1.6;
                    pos2(m.x + a.cos() * k * (0.15 + 0.85 * t), m.y + a.sin() * k * (0.15 + 0.85 * t))
                }).collect();
                p.add(Shape::line(v, s));
            }
            // Virus alert: a cell with its spikes.
            0x11 => {
                p.circle_stroke(m, k * 0.55, s);
                for j in 0..8 {
                    let a = TAU * j as f32 / 8.0 + PI / 8.0;
                    let (x, y) = (a.cos(), a.sin());
                    p.line_segment([pos2(m.x + x * k * 0.55, m.y + y * k * 0.55), pos2(m.x + x * k * 0.9, m.y + y * k * 0.9)], s);
                    p.circle_filled(pos2(m.x + x * k * 1.0, m.y + y * k * 1.0), k * 0.12, c);
                }
            }
            // The gigabomb: a sphere with a burst of rays.
            0x12 => {
                p.circle_filled(pos2(m.x - k * 0.15, m.y + k * 0.2), k * 0.6, c);
                for j in 0..5 {
                    let a = -PI * 0.15 - PI * 0.175 * j as f32;
                    p.line_segment([pos2(m.x + k * 0.45 + a.cos() * k * 0.2, m.y - k * 0.45 + a.sin() * k * 0.2), pos2(m.x + k * 0.45 + a.cos() * k * 0.6, m.y - k * 0.45 + a.sin() * k * 0.6)], s);
                }
            }
            _ => icon(p, r, Icon::Details, c),
        },
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

/// A tooltip: the name and, when bound, its key in a small key cap.
pub(crate) fn tip(resp: egui::Response, name: &str, key: &str, ink: &Ink) -> egui::Response {
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

/// The boards' gap of the corner panels to the screen's edges (`left: 24px; bottom: 24px`).
const GAP: f32 = 24.0;
/// The top bar's height (`.ig-topbar`: 44, 4 × 12 inside, 10 between its parts).
const BAR_H: f32 = 44.0;
const BAR_PAD: Vec2 = vec2(12.0, 4.0);
const BAR_GAP: f32 = 10.0;
/// A metal key (`.ig-key`) and a key group's width for `n` keys of `w` (2 apart, 2 inside).
const KEY: Vec2 = vec2(34.0, 28.0);
fn group_w(n: usize, w: f32) -> f32 {
    n as f32 * w + (n.max(1) - 1) as f32 * 2.0 + 4.0
}
/// The research button's width (board: 300) and the readouts' and the ticker's height.
const RESEARCH_W: f32 = 300.0;
const LINE_H: f32 = 30.0;
/// The selection card (`.ig-panel.deep`, 380 wide, 14 × 16 inside; the globe's column 96, 16 to the text).
const CARD_W: f32 = 380.0;
const CARD_SIDE: f32 = 96.0;
/// The navigation plate (300 wide, 10 inside): the radar 280 wide in the classic window's ratio, the camera's keys.
const NAV_W: f32 = 280.0;
const CAM_KEY: Vec2 = vec2(32.0, 28.0);

/// The ticker's and the research button's type (`15px/30px`, 0.06 em; `500 14px`, 0.08 em).
const TICKER: Font = Font { size: 15.0, line: 20.0, weight: Weight::Regular, tracking: 0.06, upper: false, lcd: false };
const BUTTON_TEXT: Font = Font { size: 14.0, line: 18.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };
/// The card's name (22) and lines (`.ig-card .line`: 15, 0.12 em, upper case), its side ratios (500 12, 0.08 em).
const CARD_NAME: Font = Font { size: 22.0, line: 26.0, weight: Weight::Regular, tracking: 0.08, upper: false, lcd: false };
const CARD_LINE: Font = Font { size: 15.0, line: 20.0, weight: Weight::Regular, tracking: 0.12, upper: true, lcd: false };
const RATIO: Font = Font { size: 12.0, line: 14.0, weight: Weight::Medium, tracking: 0.08, upper: false, lcd: false };

/// The hand over what a click works.
fn hand(r: egui::Response) -> egui::Response {
    if r.enabled() { r.on_hover_cursor(egui::CursorIcon::PointingHand) } else { r }
}

/// One frame of the HUD over the starmap; the acts in the order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut acts = Vec::new();
    let ink = bar(ctx, v, st, &mut acts);
    st.placed = None;
    st.list = None;
    st.wells.clear();
    // The selection, bottom left (the long info shows the planet itself).
    if let Some(info) = v.info.as_ref().filter(|_| v.menus.info.is_none()) {
        egui::Area::new(egui::Id::new("smh-info")).anchor(Align2::LEFT_BOTTOM, vec2(GAP, -GAP)).fade_in(false).show(ctx, |ui| info_card(ui, info, v, &ink, &mut acts, &mut st.wells));
    }
    // The radar with the camera's keys under it, bottom right.
    if let Some(mini) = &v.mini {
        egui::Area::new(egui::Id::new("smh-mini")).anchor(Align2::RIGHT_BOTTOM, vec2(-GAP, -GAP)).fade_in(false).show(ctx, |ui| {
            let at = ui.painter().add(Shape::Noop);
            let r = egui::Frame::new().inner_margin(10).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                minimap(ui, mini, v, &ink, &mut acts);
                camera_buttons(ui, v, &ink, &mut acts);
            });
            ds::behind(ui, at, |p| metal_plate(p, r.response.rect, 18));
        });
    } else {
        acts.push(Act::Hold(None));
    }
    menus(ctx, v, st, &ink, &mut acts);
    // The pictures are drawn in pixels.
    let ppp = ctx.pixels_per_point();
    for w in &mut st.wells {
        w.1 = Rect::from_min_max((w.1.min.to_vec2() * ppp).to_pos2(), (w.1.max.to_vec2() * ppp).to_pos2());
    }
    acts
}

/// The cast-metal plate (the navigation's: `radial-gradient(ellipse at 50% 0%, metal-light, metal 35 %, metal-dark)`
/// with the relief).
pub(crate) fn metal_plate(p: &egui::Painter, r: Rect, radius: u8) {
    let k = ds::tokens();
    ds::shadow(p, r.translate(vec2(0.0, 3.0)), radius, 0, 8, 0, Color32::from_black_alpha(204));
    ds::radial(p, r, radius, [0.5, 0.0], &[(0.0, k.metal_light), (0.35, k.metal), (1.0, k.metal_dark)]);
    ds::relief(p, r, radius);
}

/// The top bar's area, the boards sharing the bar place themselves under it ([`bar_bottom`]).
pub(crate) const BAR_ID: &str = "smh-left";

/// The top bar's rectangle (none before its first frame).
pub(crate) fn bar_rect(ctx: &egui::Context) -> Option<Rect> {
    ctx.memory(|m| m.area_rect(egui::Id::new(BAR_ID)))
}

/// The top bar's bottom edge (its height before its first frame).
pub(crate) fn bar_bottom(ctx: &egui::Context) -> f32 {
    bar_rect(ctx).map_or(BAR_H, |r| r.bottom())
}

/// The top bar over a board that shares it, its acts as the board's (`wrap`: the board's `Act::Bar`).
pub(crate) fn board_bar<A>(ctx: &egui::Context, v: &View, st: &mut State, wrap: impl Fn(Act) -> A) -> Vec<A> {
    let mut acts = Vec::new();
    bar(ctx, v, st, &mut acts);
    acts.into_iter().map(wrap).collect()
}

/// The top bar (`.ig-topbar` across the screen: the speed keys, money and date readouts, the message ticker, the
/// research under way, the screens' keys), shared with the modern colony, info and research screens (ADR 0070–0072),
/// who place themselves under the [`BAR_ID`] area; the frame's colours.
pub(crate) fn bar(ctx: &egui::Context, v: &View, st: &mut State, acts: &mut Vec<Act>) -> Ink {
    let ink = Ink::new(ctx, v.race);
    let k = ds::tokens();
    ctx.style_mut(|s| {
        s.visuals.window_stroke = Stroke::new(1.0f32, ink.edge);
        s.visuals.window_fill = Color32::from_rgba_unmultiplied(ink.fill.r(), ink.fill.g(), ink.fill.b(), 240);
        s.visuals.widgets.hovered.weak_bg_fill = ink.accent.gamma_multiply(0.22);
        s.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0f32, ink.accent.gamma_multiply(0.7));
        s.visuals.selection.bg_fill = ink.accent.gamma_multiply(0.3);
    });
    let screen = ctx.screen_rect();
    let mut status = Rect::NOTHING;
    egui::Area::new(egui::Id::new(BAR_ID)).fixed_pos(screen.min).fade_in(false).show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(screen.width(), BAR_H), Sense::hover());
        let p = ui.painter();
        ds::shadow(p, rect, 0, 3, 8, 0, Color32::from_black_alpha(204));
        ds::vgrad(p, rect, 0, &[(0.0, k.metal_light), (0.3, k.metal_dark), (1.0, k.metal_shadow)]);
        ds::relief(p, rect, 0);
        let inner = rect.shrink2(BAR_PAD);
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = BAR_GAP;
            // Pause and the speeds: one key block, the running one lit.
            ds::key_group(ui, |ui| {
                let r = ui.add(IconKey::icon(ds::Icon::Pause).on(v.paused));
                if ds::either(&hand(tip(r, &v.labels.pause, &v.keys.pause, &ink))) {
                    acts.push(Act::Pause);
                }
                for (n, i) in [(1, ds::Icon::Play), (2, ds::Icon::Fast), (3, ds::Icon::Fastest)] {
                    let r = ui.add(IconKey::icon(i).on(!v.paused && v.speed + 1 == n));
                    if ds::either(&hand(tip(r, &format!("{} {n}", v.labels.speed), &v.keys.speed[n - 1], &ink))) {
                        acts.push(Act::Speed(n as i32));
                    }
                }
            });
            if v.turbo > 1 {
                Text::new(&format!("×{}", v.turbo), Font::LABEL).color(k.lcd_ink).show(ui, vec2(26.0, LINE_H));
            }
            let money = v.money.replace('\u{2009}', "");
            readout(ui, &money, if money.starts_with('-') { k.enemy } else { k.lcd_ink }, 8);
            readout(ui, &v.date, k.lcd_ink, 12);
            // The right part's room first; the ticker takes what is left between.
            let (main, side): (Vec<_>, Vec<_>) = v.screens.iter().partition(|s| !matches!(s.n, 1 | 10 | 13));
            let keys: f32 = [main.len(), side.len()].iter().filter(|&&n| n > 0).map(|&n| group_w(n, KEY.x) + BAR_GAP).sum();
            let free = ui.available_width() - keys - BAR_GAP;
            // A narrow bar (4:3 at a large UI scale) gives the ticker's room up first, then the research's.
            let research_w = if free >= 2.0 * 170.0 { (free * 0.4).clamp(170.0, RESEARCH_W) } else { free.clamp(0.0, 170.0) };
            ticker(ui, v, &ink, vec2((free - research_w).max(0.0), LINE_H), acts);
            status = research_status(ui, v, st, &ink, research_w);
            for list in [main, side].into_iter().filter(|l| !l.is_empty()) {
                ds::key_group(ui, |ui| {
                    for s in list {
                        let n = s.n;
                        let draw = move |p: &egui::Painter, r: Rect, c: Color32| icon(p, r, Icon::Screen(n), c);
                        let r = ui.add_enabled(s.on, IconKey::new(ds::Glyph::Custom(&draw)).on(s.current));
                        // New mail waiting: a lit dot on the messages key.
                        if n == 10 && v.waiting {
                            let c = r.rect.right_top() + vec2(-6.0, 6.0);
                            ds::shadow(ui.painter(), Rect::from_center_size(c, vec2(7.0, 7.0)), 255, 0, 6, 0, k.lcd_ink.gamma_multiply(0.6));
                            ui.painter().circle_filled(c, 3.5, k.lcd_ink);
                        }
                        if hand(tip(r, &s.name, &s.key, &ink)).clicked() && !s.current {
                            acts.push(Act::Screen(n));
                            st.research_open = false;
                        }
                    }
                });
            }
        });
    });
    // The quick-research menu drops from the research button, sliding and fading in.
    let open = ds::open_t(ctx, egui::Id::new("smh-research"), st.research_open);
    if st.research_open {
        let r = egui::Area::new(egui::Id::new("smh-research")).pivot(Align2::RIGHT_TOP).fixed_pos(pos2(status.right(), BAR_H + 6.0 - 12.0 * (1.0 - open))).order(egui::Order::Foreground).fade_in(false).show(ctx, |ui| {
            ui.set_opacity(open);
            ds::panel(ui, ds::PanelKind::Deep, 0.96, 16.0, |ui| research_menu(ui, v, st, &ink, acts))
        });
        close_outside(ctx, st, &[r.response.rect, status]);
    }
    ink
}

/// An LCD readout (`.ig-lcd`, 22 points) at least `min_chars` wide, so the bar doesn't move as the digits change.
fn readout(ui: &mut egui::Ui, text: &str, colour: Color32, min_chars: usize) {
    let f = Font::READOUT.sized(22.0);
    let digit = Text::new("0", f).layout(ui.ctx(), f32::INFINITY).width + f.spacing();
    let w = Text::new(text, f).layout(ui.ctx(), f32::INFINITY).width.max(digit * min_chars as f32) + 2.0 * ds::SPACE[1] + f.size * 0.3;
    let (rect, _) = ui.allocate_exact_size(vec2(w, LINE_H + 2.0), Sense::hover());
    ds::lcd_window(ui.painter(), rect);
    Text::new(text, f).color(colour).paint(ui, rect.shrink2(vec2(ds::SPACE[1], 0.0)));
}

/// The research button (black, a metal rim; the flask, the project's name, its progress and per cent, or the idle
/// text); a click opens the quick-research menu.
fn research_status(ui: &mut egui::Ui, v: &View, st: &mut State, ink: &Ink, width: f32) -> Rect {
    let k = ds::tokens();
    let r = &v.research;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, LINE_H), Sense::click());
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered()).max(st.research_open as u8 as f32);
    let p = ui.painter();
    p.rect_filled(rect, ds::RADIUS_PLATE, Color32::BLACK);
    ds::rim(p, rect, ds::RADIUS_PLATE, k.metal_light.lerp_to_gamma(k.ink_silver, hot));
    let inner = rect.shrink2(vec2(10.0, 0.0));
    let on = r.current.is_some() || r.researchable;
    icon(p, Rect::from_center_size(pos2(inner.left() + 8.0, rect.center().y), vec2(17.0, 17.0)), Icon::Screen(4), if on { k.ink_silver } else { k.ink_muted });
    let text = Rect::from_min_max(pos2(inner.left() + 26.0, rect.top()), inner.max);
    match &r.current {
        Some((_, name, pct, _)) => {
            let f = Font::READOUT.sized(14.0);
            let num = format!("{pct}%");
            let pw = Text::new("100%", f).layout(ui.ctx(), f32::INFINITY).width;
            let (rest, pct_r) = text.split_left_right_at_x(text.right() - pw);
            Text::new(&num, f).color(k.lcd_ink).right().paint(ui, pct_r);
            // The name as wide as it needs up to 60 % of the room, the progress in the rest (`flex: 1`).
            let room = rest.width() - 10.0;
            let nw = Text::new(name, BUTTON_TEXT).layout(ui.ctx(), f32::INFINITY).width.min(room * 0.6);
            Text::new(name, BUTTON_TEXT).color(k.ink_silver).paint(ui, Rect::from_min_size(rest.min, vec2(nw, rest.height())));
            let track = Rect::from_min_max(pos2(rest.left() + nw + 10.0, rect.center().y - 3.0), pos2(rest.right() - 8.0, rect.center().y + 3.0));
            if track.width() > 8.0 {
                let f = ui.ctx().animate_value_with_time(resp.id.with("pct"), *pct as f32 / 100.0, ds::anim(ds::GLIDE_TIME)).clamp(0.0, 1.0);
                p.rect_filled(track, 3, k.glass_deep);
                if f > 0.0 {
                    p.rect_filled(track.with_max_x(track.left() + (track.width() * f).max(6.0)), 3, k.lcd_ink);
                }
                ds::rim(p, track, 3, k.rim);
            }
        }
        None => _ = Text::new(&v.labels.idle, BUTTON_TEXT).color(if on { k.ink_silver } else { k.ink_muted }).paint(ui, text),
    }
    if hand(tip(resp, &v.labels.research, "", ink)).clicked() {
        st.research_open = !st.research_open;
    }
    rect
}

/// The message ticker (black, a metal rim): the original's scrolling message (or the chat entry, the error, the
/// waiting text pulsing in LCD ink), else "Pause" while paused, else the latest of the history, dim. Left button: the
/// messages screen; right button held: the history.
fn ticker(ui: &mut egui::Ui, v: &View, ink: &Ink, size: Vec2, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let hot = ds::hover_t(ui.ctx(), resp.id, resp.hovered());
    let p = ui.painter();
    p.rect_filled(r, ds::RADIUS_PLATE, Color32::BLACK);
    ds::rim(p, r, ds::RADIUS_PLATE, k.metal_light.lerp_to_gamma(k.ink_silver, hot));
    let (text, colour) = match (v.line.is_empty(), v.history.last()) {
        (false, _) if v.waiting => (v.line.as_str(), k.lcd_ink.gamma_multiply(if v.blink { 1.0 } else { 0.55 })),
        (false, _) => (v.line.as_str(), k.ink),
        (true, _) if v.paused => (v.labels.pause.as_str(), k.lcd_ink),
        (true, Some(h)) => (h.as_str(), k.ink_muted),
        (true, None) => ("", k.ink_muted),
    };
    let upper = text == v.labels.pause && v.paused;
    Text::new(text, TICKER.upper(upper)).color(colour).paint_only(ui, r.shrink2(vec2(ds::SPACE[1], 0.0)));
    let name = v.screens.iter().find(|s| s.n == 10);
    let resp = hand(tip(resp, name.map_or("", |s| s.name.as_str()), name.map_or("", |s| s.key.as_str()), ink));
    let right_held = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
    if right_held != v.history_open {
        acts.push(Act::History(right_held));
    }
    // The left button held over the line asks every frame (`MOUSE_SelectScreens`, `top_bar_frame`).
    if resp.contains_pointer() && ui.input(|i| i.pointer.primary_down()) && resp.is_pointer_button_down_on() {
        acts.push(Act::Messages);
    }
    if v.history_open && !v.history.is_empty() {
        egui::Area::new(egui::Id::new("smh-history")).fixed_pos(pos2(r.left(), r.bottom() + 8.0)).order(egui::Order::Foreground).fade_in(false).show(ui.ctx(), |ui| {
            ds::panel(ui, ds::PanelKind::Deep, 0.96, 12.0, |ui| {
                let w = r.width().max(320.0) - 24.0;
                for h in &v.history {
                    Text::new(h, TICKER).color(k.ink_silver).show(ui, vec2(w, 22.0));
                }
            });
        });
    }
}

/// A click anywhere but the menu and its button closes the menu.
fn close_outside(ctx: &egui::Context, st: &mut State, keep: &[Rect]) {
    let (clicked, at) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    if clicked && at.is_some_and(|p| !keep.iter().any(|r| r.contains(p))) {
        st.research_open = false;
    }
}

/// The quick-research menu: the startable techs by column (their cost as the tooltip), the running project's
/// progress, STOP and the money left.
fn research_menu(ui: &mut egui::Ui, v: &View, st: &mut State, _ink: &Ink, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let r = &v.research;
    let cols = r.headers.len().max(1);
    const COL: f32 = 190.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, 2.0);
        for c in 0..cols {
            ui.vertical(|ui| {
                ui.set_width(COL);
                Text::new(r.headers.get(c).map_or("", |h| h.as_str()), Font::LABEL).color(k.info).show(ui, vec2(COL, 24.0));
                for (t, name, _, ok, short, cost) in r.items.iter().filter(|i| i.2 == c) {
                    let off = *short || r.current.is_some();
                    let resp = ui.add_enabled(*ok, ds::ListRow::new(name).off(off).height(24.0).font(Font::DATA));
                    let resp = if *ok { hand(resp).on_hover_text(format!("{}: {}", r.footer[3], money(*cost as f64))) } else { resp };
                    if resp.clicked() {
                        acts.push(Act::Start(*t));
                        st.research_open = false;
                    }
                }
            });
        }
    });
    if let Some((t, name, pct, left)) = &r.current {
        ds::separator(ui, 6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let w = (ui.available_width() - 240.0).max(120.0);
            Text::new(&format!("{}: {name}", r.footer[0]), Font::DATA).color(k.ink).show(ui, vec2(w, 32.0));
            ds::bar(ui, 90.0, 6.0, *pct as f32 / 100.0, k.lcd_ink);
            Text::new(&format!("{pct}%"), Font::READOUT.sized(15.0)).color(k.lcd_ink).show(ui, vec2(44.0, 32.0));
            if hand(ui.add(Plate::new(&r.footer[1]).height(32.0).width(80.0).font(Plate::FONT.sized(14.0)))).clicked() {
                acts.push(Act::Stop(*t));
                st.research_open = false;
            }
        });
        Text::new(&format!("{}: {}", r.footer[2], money(*left as f64)), Font::LABEL.upper(false)).color(k.ink_muted).show(ui, vec2(ui.available_width(), 20.0));
    }
}

/// A shaded globe (the card's and the long info's planet; no turning model) in `colour`, or a fleet's mark.
fn globe(p: &egui::Painter, r: Rect, colour: Color32, planet: bool) {
    let k = ds::tokens();
    let c = r.center();
    if planet {
        ds::shadow(p, r.shrink(r.width() * 0.08), 255, 0, 14, 0, colour.gamma_multiply(0.35));
        ds::radial(p, r, 255, [0.34, 0.28], &[(0.0, lerp(colour, Color32::WHITE, 0.3).gamma_multiply(0.85)), (0.4, colour.gamma_multiply(0.42)), (0.8, colour.gamma_multiply(0.12)), (1.0, Color32::from_black_alpha(250))]);
        p.circle_stroke(c, r.width() / 2.0 - 0.5, Stroke::new(1.0f32, colour.gamma_multiply(0.5)));
    } else {
        p.circle_filled(c, r.width() / 2.0, k.glass_deep);
        p.circle_stroke(c, r.width() / 2.0 - 0.5, Stroke::new(1.0f32, k.rim_bright));
        let s = r.width() * 0.3;
        p.add(Shape::convex_polygon(vec![pos2(c.x + s, c.y), pos2(c.x - s * 0.85, c.y - s * 0.75), pos2(c.x - s * 0.4, c.y), pos2(c.x - s * 0.85, c.y + s * 0.75)], colour, Stroke::NONE));
    }
}

/// The card's and the long info's planet: the game's turning globe cut into `r` (a shadow under it), else the
/// shaded globe.
fn planet(ui: &egui::Ui, r: Rect, colour: Color32, picture: bool, is_planet: bool, wells: &mut Vec<([f32; 4], Rect, f32)>) {
    if !picture {
        return globe(ui.painter(), r, colour, is_planet);
    }
    ds::shadow(ui.painter(), r.shrink(r.width() * 0.08), 255, 0, 14, 0, Color32::from_black_alpha(200));
    wells.push((GLOBE_SRC, Rect::from_center_size(r.center(), r.size() / GLOBE_FILL), ui.opacity()));
}

/// A side ratio of the card (`.ratio`: an icon and the value, muted): the richness (a crystal) or the habitability
/// (a figure); the label as its tooltip.
fn ratio(ui: &mut egui::Ui, label: &str, value: &str, habitability: bool) {
    let k = ds::tokens();
    let (r, resp) = ui.allocate_exact_size(vec2(CARD_SIDE, 16.0), Sense::hover());
    let w = Text::new(value, RATIO).layout(ui.ctx(), CARD_SIDE).width + 15.0;
    let i = Rect::from_center_size(pos2(r.center().x - w / 2.0 + 6.0, r.center().y), vec2(12.0, 12.0));
    let (p, s, c) = (ui.painter(), Stroke::new(1.3f32, k.ink_muted), i.center());
    if habitability {
        p.circle_stroke(pos2(c.x, c.y - 3.0), 2.6, s);
        p.add(Shape::line(arc(pos2(c.x, c.y + 6.0), 5.0, 5.0, std::f32::consts::PI, std::f32::consts::TAU), s));
    } else {
        p.add(Shape::closed_line(vec![pos2(c.x, c.y - 6.0), pos2(c.x + 5.0, c.y - 1.5), pos2(c.x, c.y + 6.0), pos2(c.x - 5.0, c.y - 1.5)], s));
        p.line_segment([pos2(c.x - 5.0, c.y - 1.5), pos2(c.x + 5.0, c.y - 1.5)], s);
    }
    Text::new(value, RATIO).color(k.ink_muted).paint_only(ui, Rect::from_min_max(pos2(i.right() + 3.0, r.top()), r.max));
    resp.on_hover_text(label);
}

/// The selection card (`.ig-panel.deep` with `.ig-card`): the globe between its ratios, the name, the original's
/// lines, the buttons as plates (the colony or split, the details) and keys (tanks, production).
fn info_card(ui: &mut egui::Ui, info: &Info, v: &View, ink: &Ink, acts: &mut Vec<Act>, wells: &mut Vec<([f32; 4], Rect, f32)>) {
    let k = ds::tokens();
    let at = ui.painter().add(Shape::Noop);
    let frame = egui::Frame::new().inner_margin(Margin { left: 16, right: 16, top: 14, bottom: 14 }).show(ui, |ui| {
        let inner = CARD_W - 32.0;
        ui.set_width(inner);
        let main_w = inner - CARD_SIDE - 16.0;
        let body = ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(16.0, 2.0);
            ui.vertical(|ui| {
                ui.set_width(CARD_SIDE);
                if let Some((l, val)) = info.stats.first() {
                    ratio(ui, l, val, false);
                }
                let (g, _) = ui.allocate_exact_size(vec2(CARD_SIDE, 84.0), Sense::hover());
                planet(ui, Rect::from_center_size(g.center(), vec2(84.0, 84.0)), info.colour, info.planet && v.globe, info.planet, wells);
                if let Some((l, val)) = info.stats.get(1) {
                    ratio(ui, l, val, true);
                }
            });
            ui.vertical(|ui| {
                ui.set_width(main_w);
                ui.spacing_mut().item_spacing.y = 2.0;
                Text::new(&info.title, CARD_NAME).color(k.ink).show(ui, vec2(main_w, 28.0));
                for (l, c) in info.lines.iter().take(6) {
                    Text::new(l, CARD_LINE).color(*c).show(ui, vec2(main_w, 20.0));
                }
                if info.redirect {
                    Text::new(&v.labels.redirect, Font::LABEL).color(k.lcd_ink.gamma_multiply(if v.blink { 1.0 } else { 0.45 })).show(ui, vec2(main_w, 20.0));
                }
                ui.add_space(6.0);
                ui.allocate_exact_size(vec2(main_w, 32.0), Sense::hover()).0
            })
            .inner
        });
        let buttons = body.inner;
        // The card itself is the info box (hot-spot 8): left and right click as the original's.
        let card = ui.interact(body.response.rect, ui.id().with("smh-info-card"), Sense::click());
        let hot = ds::hover_t(ui.ctx(), card.id, card.hovered());
        if hot > 0.0 {
            ds::rim(ui.painter(), body.response.rect.expand(6.0), ds::RADIUS_PLATE, k.rim_bright.gamma_multiply(hot));
        }
        let card = hand(card).on_hover_text(&info.tip);
        if card.clicked() {
            acts.push(Act::Info(false));
        }
        if card.secondary_clicked() {
            acts.push(Act::Info(true));
        }
        // The buttons over the card (they win its click): the plates as wide as their labels, a share of the room at
        // most.
        ui.scope_builder(egui::UiBuilder::new().max_rect(buttons).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            // One type size for both plates (`.ig-plate`, 14 points, the plate's 16 aside), smaller together when
            // the labels and the keys don't fit; a label still too long is shrunk or cut by the plate.
            let keys = info.tanks as usize + info.product as usize;
            let labels: Vec<&str> = info.right.iter().map(|s| s.as_str()).chain([info.left.as_str()]).collect();
            let room = main_w - keys as f32 * (KEY.x + 6.0) - 6.0 * (labels.len() as f32 - 1.0) - 2.0 * ds::SPACE[2] * labels.len() as f32;
            let ctx = ui.ctx().clone();
            let text_w = |s: &str, f: Font| Text::new(s, f).layout(&ctx, f32::INFINITY).width;
            let natural: f32 = labels.iter().map(|s| text_w(s, Plate::FONT.sized(14.0).tracking(0.12))).sum();
            let font = Plate::FONT.sized((14.0 * room / natural.max(1.0)).clamp(11.0, 14.0)).tracking(0.12);
            let share = (room / labels.len() as f32).floor() + 2.0 * ds::SPACE[2];
            let fits = labels.iter().map(|s| text_w(s, font)).sum::<f32>() <= room + 0.5;
            let pw = |s: &str| (text_w(s, font) + 2.0 * ds::SPACE[2]).min(if fits { f32::INFINITY } else { share });
            if let Some(r) = &info.right {
                let pw = pw(r);
                let key = if info.planet { v.screens.iter().find(|s| s.n == 3).map_or("", |s| s.key.as_str()) } else { "" };
                if hand(tip(ui.add_sized(vec2(pw, 32.0), Plate::new(r).height(32.0).width(pw).font(font)), r, key, ink)).clicked() {
                    acts.push(Act::Info(true));
                }
            }
            let lw = pw(&info.left);
            if hand(ui.add_sized(vec2(lw, 32.0), Plate::new(&info.left).height(32.0).width(lw).font(font))).clicked() {
                acts.push(Act::Info(false));
            }
            for (on, i, n, id) in [(info.tanks, Icon::Tanks, 8, 10), (info.product, Icon::Factory, 9, 11)] {
                if on {
                    let draw = move |p: &egui::Painter, r: Rect, c: Color32| icon(p, r, i, c);
                    if hand(tip(ui.add(IconKey::new(ds::Glyph::Custom(&draw)).size(vec2(KEY.x, 32.0))), &v.tips[n], "", ink)).clicked() {
                        acts.push(Act::Panel(id));
                    }
                }
            }
        });
    });
    ds::behind(ui, at, |p| ds::paint_panel(p, frame.response.rect, ds::PanelKind::Deep, 0.88));
}

/// The camera's keys under the radar: rotate, zoom and tilt held (the left button's way, the right button the other
/// way as on the console), the top and side views clicked.
fn camera_buttons(ui: &mut egui::Ui, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    let mut held = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(((NAV_W - group_w(6, CAM_KEY.x) - group_w(2, CAM_KEY.x) - 4.0) / 2.0).max(0.0));
        let none = String::new();
        ds::key_group(ui, |ui| {
            let buttons = [(Icon::RotateLeft, 3, true, &none), (Icon::RotateRight, 2, true, &none), (Icon::ZoomIn, 5, true, &v.keys.zoom[0]), (Icon::ZoomOut, 5, false, &v.keys.zoom[1]), (Icon::TiltUp, 6, true, &none), (Icon::TiltDown, 6, false, &none)];
            for (i, spot, left, key) in buttons {
                let draw = move |p: &egui::Painter, r: Rect, c: Color32| icon(p, r, i, c);
                let r = hand(tip(ui.add(IconKey::new(ds::Glyph::Custom(&draw)).size(CAM_KEY)), &v.tips[spot as usize], key, ink));
                if r.is_pointer_button_down_on() && r.contains_pointer() {
                    let right = ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary) && !i.pointer.primary_down());
                    held = Some((spot, left != right));
                }
            }
        });
        ds::key_group(ui, |ui| {
            for (i, spot) in [(Icon::TopView, 1), (Icon::SideView, 4)] {
                let draw = move |p: &egui::Painter, r: Rect, c: Color32| icon(p, r, i, c);
                if ds::either(&hand(tip(ui.add(IconKey::new(ds::Glyph::Custom(&draw)).size(CAM_KEY)), &v.tips[spot as usize], "", ink))) {
                    acts.push(Act::View(spot));
                }
            }
        });
    });
    acts.push(Act::Hold(held));
}

/// The radar (a black window sunk into the metal): the planets and fleets, the view's outline; press or drag moves
/// the camera there (the target pointer).
fn minimap(ui: &mut egui::Ui, mini: &Mini, v: &View, ink: &Ink, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let size = vec2(NAV_W, (NAV_W * MINI[3] / MINI[2]).round());
    let (r, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let back = ui.painter();
    ds::shadow(back, r, 10, 0, 0, 1, k.metal_shadow);
    back.rect_filled(r, 10, Color32::BLACK);
    ds::vgrad(back, r, 10, &[(0.0, Color32::from_black_alpha(255)), (6.0 / r.height(), Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    let p = ui.painter().with_clip_rect(r.shrink(2.0));
    // A faint grid: the console's screen.
    for j in 1..6 {
        let (x, y) = (r.left() + r.width() * j as f32 / 6.0, r.top() + r.height() * j as f32 / 6.0);
        p.vline(x, r.y_range(), Stroke::new(1.0f32, k.rim_bright.gamma_multiply(0.18)));
        p.hline(r.x_range(), y, Stroke::new(1.0f32, k.rim_bright.gamma_multiply(0.18)));
    }
    let at = |u: [f32; 2]| pos2(r.left() + u[0] * r.width(), r.top() + u[1] * r.height());
    for &(u, c, sel) in &mini.fleets {
        let c = if sel && !v.blink { c.gamma_multiply(0.3) } else { c };
        p.rect_filled(Rect::from_center_size(at(u), vec2(3.4, 1.8)), 0.0, c);
    }
    for &(u, c, sel) in &mini.planets {
        let q = at(u);
        p.circle_filled(q, 2.1, c);
        if sel {
            p.circle_stroke(q, 5.5, Stroke::new(1.2f32, if v.blink { c } else { c.gamma_multiply(0.4) }));
        }
    }
    let outline: Vec<Pos2> = [0, 1, 3, 2].iter().filter_map(|&i| mini.view[i].map(at)).collect();
    if outline.len() == 4 {
        p.add(Shape::convex_polygon(outline.clone(), k.focus.gamma_multiply(0.06), Stroke::NONE));
        p.add(Shape::closed_line(outline, Stroke::new(1.2f32, k.focus)));
    }
    ds::rim(back, r, 10, Color32::from_white_alpha(10));
    let resp = tip(resp, &v.tips[7], "", ink).on_hover_cursor(egui::CursorIcon::Crosshair);
    let (down, first) = ui.input(|i| (i.pointer.any_down(), i.pointer.any_pressed()));
    if let (true, Some(q)) = (resp.is_pointer_button_down_on() && down, resp.interact_pointer_pos()) {
        acts.push(Act::Minimap(mini_virtual([(q.x - r.left()) / r.width(), (q.y - r.top()) / r.height()]), first));
    }
}

/// The starmap's long-press menus and the planet's long info (re/screens/starmap.md §14–§17), laid out on the original's
/// virtual screen so the classic hot-spot code that hit-tests them (`crate::starmap_menus`) answers exactly what is
/// shown. Painted on their own layers: they take no input from egui.
#[derive(Debug, Clone, Default)]
pub struct Menus {
    /// Virtual pixels → points: `(x0 + x·k, y0 + y·k)`.
    pub map: [f32; 3],
    pub radial: Option<Radial>,
    pub select: Option<Select>,
    pub info: Option<LongInfo>,
}

/// The radial icon menu (`StarmapRopIkonMod`), shown once the button has been held 0.3 s.
#[derive(Debug, Clone, Default)]
pub struct Radial {
    /// The press point and the four places' hit centres (`LastMouse + Starmap_RopIkonX/Y`), virtual.
    pub at: [f32; 2],
    pub centres: [[f32; 2]; 4],
    /// The places' icons (0 = none) and names (`StarmapRopIkon_Nev`).
    pub icons: [i32; 4],
    pub names: [String; 4],
    /// `RopIkonFrame`: 30..=50 while it comes in; it answers from 50.
    pub frame: i32,
    /// `AktRopIkon2`.
    pub hover: i32,
}

/// An open select menu: its rows' rectangles (virtual), texts and the one under the pointer (`smSelected`).
#[derive(Debug, Clone, Default)]
pub struct Select {
    pub rows: Vec<SelectRow>,
    /// A settings menu's field (0 tax, 1 autobuild, 2 priority).
    pub field: Option<usize>,
    /// The planet list (`MenuKind::Planets`).
    pub planets: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SelectRow {
    pub rect: [f32; 4],
    pub text: String,
    /// The planet list's production text.
    pub sub: String,
    pub hovered: bool,
}

/// The planet's long info (`StarmapPlanetInfoMod`).
#[derive(Debug, Clone, Default)]
pub struct LongInfo {
    /// How far the camera has flown in (`q`, 0..=1) and the exit button's slide (`StarmapInfoOffHoltart`).
    pub q: f32,
    pub exit_in: f32,
    pub exit: String,
    /// The name (the first line) and the planet owner's colour.
    pub title: String,
    pub colour: Color32,
    pub lines: Vec<InfoRow>,
    /// The PROBLEMS label, its virtual y and each problem's per cent and name.
    pub problems: Option<Problems>,
    /// The player's planet: the TAX, AUTO BUILD, PRIORITY labels and values, and "ALL".
    pub settings: Option<([String; 3], [String; 3], String)>,
    /// The long info's hot-spot under the pointer (`crate::starmap_menus::INFO_SPOTS`, 9 = the exit button) and
    /// whether the left button is held.
    pub hot: Option<u32>,
    pub held: bool,
}

/// The long info's PROBLEMS row: its label, virtual y and each problem's per cent and name.
pub type Problems = (String, f32, Vec<(i32, String)>);

/// A line of the long info: label, value, the shortage colour, the NEEDED label and value, the
/// SPECIAL row.
#[derive(Debug, Clone, Default)]
pub struct InfoRow {
    pub label: String,
    pub value: String,
    pub short: bool,
    pub needed: Option<(String, String)>,
    pub special: bool,
    /// The line's number in `StarmapPlanetInfoMod` (2 the surface, 3 the empire, 13 the special).
    pub n: u16,
}

impl Menus {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.map[0] + x * self.map[2], self.map[1] + y * self.map[2])
    }

    fn rect(&self, r: [f32; 4]) -> Rect {
        Rect::from_min_max(self.at(r[0], r[1]), self.at(r[0] + r[2], r[1] + r[3]))
    }

    /// Virtual pixels as points (sizes, fonts).
    fn k(&self, v: f32) -> f32 {
        v * self.map[2]
    }
}

/// The radial menu's icons come in from frame 30 to 50 (0.3 s to 0.5 s held), easing out of the press point.
pub fn radial_in(frame: i32) -> f32 {
    let t = ((frame - 30) as f32 / 20.0).clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}

/// The rop icon's drawn icon (`Starmap_RopIkonKonvert`'s picture, re/screens/starmap.md §14.5).
fn rop_icon(n: i32) -> Icon {
    match n {
        1 => Icon::Tanks,
        2 => Icon::Factory,
        4 => Icon::Screen(3),
        0xd => Icon::Split,
        0xf => Icon::Details,
        n => Icon::Rop(n),
    }
}

/// A game label in capitals (the screen menu's) as the boards' plates write it: the first letter capital, the rest
/// small; a label with small letters stays.
pub(crate) fn calm(s: &str) -> String {
    if s.chars().any(char::is_lowercase) {
        return s.to_string();
    }
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c.as_str().to_lowercase().chars()).collect())
}

/// The game's text in the panel's font: the font's private marks (the population's and energy's signs) dropped.
fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// The long info's panel (the board's: 460 wide, right, under the bar) and its parts' sizes.
const LI_W: f32 = 500.0;
const LI_PAD: f32 = 18.0;
const LI_HEAD: f32 = 144.0;
const LI_GLOBE: f32 = 112.0;
const LI_LABEL: f32 = 130.0;
/// A settings row: the label's column, the ALL chip, the round − and + (`.ig-round.small` in its ring: 42).
const LI_SET_LABEL: f32 = 120.0;
const LI_ALL: f32 = 64.0;
const LI_ROUND: f32 = 42.0;
const LI_PLATES: f32 = 44.0;
const LI_NAME: Font = Font { size: 26.0, line: 30.0, weight: Weight::SemiBold, tracking: 0.1, upper: false, lcd: false };
const LI_OWNER: Font = Font { size: 13.0, line: 16.0, weight: Weight::SemiBold, tracking: 0.16, upper: true, lcd: false };

/// Where the modern long info put the original's hot-spots this frame (points), so the pointer reaches them there.
#[derive(Debug, Clone)]
pub struct Placed {
    map: [f32; 3],
    panel: Rect,
    spots: Vec<(u32, Rect)>,
    /// An open select menu of the long info: its shift (virtual) and its rows' bounds (points).
    menu: Option<([f32; 2], Rect)>,
}

/// A point inside the original's long info that is none of its controls (hot-spot 100: the panel takes the press).
const NEUTRAL: [f32; 2] = [330.0, 40.0];

/// The middle of the original's long-info hot-spot `id` (virtual).
fn spot_centre(id: u32) -> Option<[f32; 2]> {
    crate::starmap_menus::INFO_SPOTS.iter().find(|s| s.0 == id).map(|s| [(s.1[0] as f32 + s.1[2] as f32 / 2.0), (s.1[1] as f32 + s.1[3] as f32 / 2.0)])
}

impl State {
    /// The moved planet list is open (the pointer goes through [`State::list_pointer`]).
    pub fn list_open(&self) -> bool {
        self.list.is_some()
    }

    /// The pointer `at` (virtual) over the moved planet list: over a drawn row the middle of the original's; elsewhere
    /// unchanged (a press there closes the list, as the original's off its rows).
    pub fn list_pointer(&self, at: [f32; 2]) -> [f32; 2] {
        let Some((map, rows)) = &self.list else { return at };
        let p = pos2(map[0] + at[0] * map[2], map[1] + at[1] * map[2]);
        rows.iter().find(|r| r.0.contains(p)).map_or(at, |r| r.1)
    }

    /// The pointer `at` (virtual) as the original's long-info hot-spots should see it under the modern panel: over one
    /// of its controls, the middle of the original's spot; over the panel (or where the original's controls were),
    /// the original panel's empty part; over an open settings menu, its original place. Elsewhere unchanged.
    pub fn info_pointer(&self, at: [f32; 2]) -> [f32; 2] {
        let Some(pl) = &self.placed else { return at };
        let m = pl.map;
        let p = pos2(m[0] + at[0] * m[2], m[1] + at[1] * m[2]);
        if let Some((shift, r)) = pl.menu {
            // Off the drawn menu nothing of the original's may answer, its rows at their old place included.
            return if r.contains(p) { [at[0] - shift[0], at[1] - shift[1]] } else { NEUTRAL };
        }
        if let Some(c) = pl.spots.iter().find(|s| s.1.contains(p)).and_then(|s| spot_centre(s.0)) {
            return c;
        }
        let classic = crate::starmap_menus::INFO_SPOTS.iter().any(|s| s.0 != 100 && crate::kit::inside(s.1, at)) || crate::starmap_menus::exit_spot(at);
        if pl.panel.contains(p) || classic { NEUTRAL } else { at }
    }
}

fn menus(ctx: &egui::Context, v: &View, st: &mut State, ink: &Ink, acts: &mut Vec<Act>) {
    let m = &v.menus;
    if let Some(i) = &m.info {
        long_info(ctx, m, i, v, st, acts);
    }
    if m.select.is_none() && m.radial.is_none() {
        return;
    }
    egui::Area::new(egui::Id::new("smh-menus")).order(egui::Order::Foreground).interactable(false).fixed_pos(ctx.screen_rect().min).fade_in(false).show(ctx, |ui| {
        // Taken: the production view puts it back each frame it is up.
        let plate = ui.ctx().data_mut(|d| {
            let id = egui::Id::new(PLATE_ID);
            let v = d.get_temp::<(Rect, String)>(id);
            d.remove::<(Rect, String)>(id);
            v
        });
        if let (Some(s), Some((plate, name))) = (m.select.as_ref().filter(|s| s.planets), plate) {
            st.list = Some((m.map, planet_list(ui, s, plate, &name)));
        } else if let Some(s) = &m.select {
            let (shift, field) = select_shift(m, s, st, ui.ctx().screen_rect());
            // A settings menu marks the field's value and is the field's width (ADR 0077).
            let current = field.and_then(|f| m.info.as_ref()?.settings.as_ref().map(|x| x.1[f.0].clone()));
            let bounds = select(ui, m, s, shift, current.as_deref(), field.map(|f| f.1.width()));
            if let Some(pl) = &mut st.placed {
                pl.menu = (shift != [0.0; 2]).then_some((shift, bounds));
            }
        }
        if let Some(r) = &m.radial {
            radial(ui, m, r, ink);
        }
    });
}

/// A settings menu of the long info opens over its modern field, the field's value on the field (ADR 0077; under
/// it when no row is the value, kept on the screen): moved by as much from where the original opened it at the press
/// (virtual); and which field (0 tax, 1 autobuild, 2 priority) with its rect.
fn select_shift(m: &Menus, s: &Select, st: &State, screen: Rect) -> ([f32; 2], Option<(usize, Rect)>) {
    let (Some(pl), Some(n), true) = (&st.placed, s.field, !s.rows.is_empty()) else { return ([0.0; 2], None) };
    let Some(field) = pl.spots.iter().find(|sp| sp.0 == [0x5a, 0x5e, 0x60][n]).map(|sp| sp.1) else { return ([0.0; 2], None) };
    let value = m.info.as_ref().and_then(|i| i.settings.as_ref()).map(|x| x.1[n].clone());
    let on_field = value.and_then(|v| s.rows.iter().position(|r| r.text == v));
    let k = m.map[2];
    let (x0, y0) = s.rows.iter().fold((f32::MAX, f32::MAX), |a, r| (a.0.min(r.rect[0]), a.1.min(r.rect[1])));
    let (x1, y1) = s.rows.iter().fold((f32::MIN, f32::MIN), |a, r| (a.0.max(r.rect[0] + r.rect[2]), a.1.max(r.rect[1] + r.rect[3])));
    let (w, h) = ((x1 - x0) * k, (y1 - y0) * k);
    let left = field.left().min(screen.right() - 16.0 - w);
    let top = match on_field {
        Some(i) => (field.center().y - (i as f32 + 0.5) * h / s.rows.len() as f32).clamp(BAR_H + 8.0, (screen.bottom() - 8.0 - h).max(BAR_H + 8.0)),
        None if field.bottom() + 8.0 + h <= screen.bottom() - 8.0 => field.bottom() + 8.0,
        None => field.top() - 8.0 - h,
    };
    ([(left - m.map[0]) / k - x0, (top - m.map[1]) / k - y0], Some((n, field)))
}

/// The planet list under the production view's title plate (the "Select planet" board): the plate's width and a bit,
/// the board's 52-point rows, the planet on the plate in the list's selected look, the row under the pointer with the
/// bright rim. The original's rows answer the pointer moved onto them ([`State::list_pointer`]).
fn planet_list(ui: &egui::Ui, s: &Select, plate: Rect, current: &str) -> Vec<ListRow> {
    let k = ds::tokens();
    let screen = ui.ctx().screen_rect();
    let n = s.rows.len().max(1) as f32;
    let row_h = 52.0f32.min(((screen.bottom() - plate.bottom() - 32.0) / n).floor()).max(24.0);
    let w = plate.width() + 20.0;
    let panel = Rect::from_min_size(pos2(plate.center().x - w / 2.0, plate.bottom() + 10.0), vec2(w, row_h * n + 12.0));
    let p = ui.painter();
    ds::shadow(p, panel, ds::RADIUS_PANEL, 8, 24, 0, Color32::from_black_alpha(204));
    p.rect_filled(panel, ds::RADIUS_PANEL, k.void);
    ds::paint_panel(p, panel, ds::PanelKind::Deep, 0.97);
    let font = Font::DATA.sized(16.0).tracking(0.08);
    let mut rows = Vec::new();
    for (i, r) in s.rows.iter().enumerate() {
        let row = Rect::from_min_size(panel.min + vec2(6.0, 6.0 + i as f32 * row_h), vec2(w - 12.0, row_h)).shrink2(vec2(0.0, 2.0));
        let hot = ds::hover_t(ui.ctx(), egui::Id::new(("smh-planets", i)), r.hovered);
        if r.text == current {
            ds::hgrad(p, row, ds::RADIUS_PLATE, &[(0.0, Color32::from_white_alpha(46)), (1.0, Color32::from_white_alpha(5))]);
            ds::rim(p, row, ds::RADIUS_PLATE, k.ink_silver);
        }
        if hot > 0.0 {
            ds::rim(p, row.shrink(1.0), ds::RADIUS_PLATE, k.rim_bright.gamma_multiply(hot));
        }
        Text::new(&r.text, font).color(k.ink).shrink(12.0).paint_only(ui, row.shrink2(vec2(16.0, 0.0)));
        // The drawn row reaches its whole height (the margins included).
        rows.push((row.expand2(vec2(6.0, 2.0)), [r.rect[0] + r.rect[2] / 2.0, r.rect[1] + r.rect[3] / 2.0]));
    }
    rows
}

/// The radial menu: a hub at the press point, a hairline to each icon, the icons on metal keys that come out of the
/// hub; armed (from frame 50) they brighten, the one under the pointer lights in LCD ink and shows its name on a
/// plate.
fn radial(ui: &egui::Ui, m: &Menus, r: &Radial, ink: &Ink) {
    let k = ds::tokens();
    let p = ui.painter();
    let t = radial_in(r.frame);
    let armed = r.frame >= 50;
    let hub = m.at(r.at[0], r.at[1]);
    // About the 30-pixel circle each icon answers in.
    let d = m.k(36.0);
    p.circle_stroke(hub, m.k(3.0), Stroke::new(1.5f32, ink.accent.gamma_multiply(t)));
    let mut label = None;
    for n in 0..4 {
        if r.icons[n] <= 0 {
            continue;
        }
        let c = m.at(r.centres[n][0], r.centres[n][1]);
        let c = hub + (c - hub) * t;
        let hot = armed && r.hover == r.icons[n];
        let dir = (c - hub).normalized();
        p.line_segment([hub + dir * m.k(5.0), c - dir * d * 0.6], Stroke::new(1.0f32, ink.accent.gamma_multiply(0.45 * t)));
        let rect = Rect::from_center_size(c, vec2(d, d));
        let mut f = p.clone();
        f.multiply_opacity(t);
        ds::shadow(&f, rect.translate(vec2(0.0, 3.0)), 255, 0, 8, 0, Color32::from_black_alpha(200));
        f.circle_filled(c, d / 2.0 + 1.0, k.metal_shadow);
        ds::radial(&f, rect, 255, [0.4, 0.2], &[(0.0, k.metal_shine), (0.45, k.metal_light), (1.0, k.metal_dark)]);
        ds::relief(&f, rect, 255);
        let face = rect.shrink(d * 0.13);
        f.circle_filled(c, face.width() / 2.0 + 1.0, k.metal_shadow);
        if hot {
            ds::shadow(&f, rect, 255, 0, 14, 0, k.lcd_ink.gamma_multiply(0.5));
            ds::radial(&f, face, 255, [0.5, 0.4], &[(0.0, k.lcd), (1.0, k.metal_shadow)]);
        } else {
            ds::radial(&f, face, 255, [0.45, 0.3], &[(0.0, k.metal_dark), (1.0, k.metal_shadow)]);
        }
        ds::vgrad(&f, face, 255, &[(0.0, Color32::from_black_alpha(160)), (0.25, Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
        let fg = if hot { k.lcd_ink } else if armed { k.ink_silver } else { k.ink_muted };
        icon(&f, face.shrink(d * 0.1), rop_icon(r.icons[n]), fg);
        if hot && !r.names[n].is_empty() {
            label = Some((n, rect));
        }
    }
    // The name out beside the icon under the pointer, away from the hub, over the other icons (`.ig-plate`, 13 / 0.16 em).
    if let Some((n, rect)) = label {
        let font = Font { size: 13.0, line: 16.0, weight: Weight::Medium, tracking: 0.16, upper: true, lcd: false };
        let w = Text::new(&r.names[n], font).layout(ui.ctx(), 260.0).width + 28.0;
        let size = vec2(w, 28.0);
        let gap = 8.0;
        let at = match n {
            0 => pos2(rect.left() - gap - size.x, rect.center().y - size.y / 2.0),
            1 => pos2(rect.center().x - size.x / 2.0, rect.bottom() + gap),
            2 => pos2(rect.right() + gap, rect.center().y - size.y / 2.0),
            _ => pos2(rect.center().x - size.x / 2.0, rect.top() - gap - size.y),
        };
        let at = Rect::from_min_size(at, size);
        ds::plate_fill(p, at, ds::RADIUS_PANEL, false);
        Text::new(&r.names[n], font).center().color(k.ink).paint_only(ui, at.shrink2(vec2(14.0, 0.0)));
    }
}

/// A select menu (moved by `shift`, virtual): a deep glass panel around the rows, the row under the pointer in the
/// list's selected look; `width` a settings field's (the rows stay inside the original's, which the pointer reaches).
/// Its bounds in points.
fn select(ui: &egui::Ui, m: &Menus, s: &Select, shift: [f32; 2], current: Option<&str>, width: Option<f32>) -> Rect {
    let k = ds::tokens();
    let p = ui.painter();
    let rect = |r: [f32; 4]| m.rect([r[0] + shift[0], r[1] + shift[1], r[2], r[3]]);
    let Some(first) = s.rows.first() else { return Rect::NOTHING };
    let mut b = rect(first.rect);
    for r in &s.rows {
        b = b.union(rect(r.rect));
    }
    let row_h = b.height() / s.rows.len().max(1) as f32;
    let font = Font::DATA.sized((row_h * 0.55).clamp(11.0, 15.0)).tracking(0.08);
    let sub = Font::LABEL.upper(false).sized((row_h * 0.45).clamp(10.0, 13.0));
    // Room for the texts (the sale's line is wider than its row).
    let widest = s.rows.iter().map(|r| Text::new(&r.text, font).layout(ui.ctx(), f32::INFINITY).width + if r.sub.is_empty() { 0.0 } else { Text::new(&r.sub, sub).layout(ui.ctx(), f32::INFINITY).width + 10.0 }).fold(0.0, f32::max);
    b.max.x = match width {
        Some(w) => b.max.x.min(b.min.x + (w - 8.0).max(widest + 16.0)),
        None => b.max.x.max(b.min.x + widest + 16.0),
    };
    let panel = b.expand2(vec2(if width.is_some() { 4.0 } else { 8.0 }, 6.0));
    // Opaque: the field and the plates under the list don't show through (the boards' menus).
    p.rect_filled(panel, ds::RADIUS_PANEL, k.void);
    ds::paint_panel(p, panel, ds::PanelKind::Deep, 0.97);
    for r in &s.rows {
        let mut row = rect(r.rect);
        row.max.x = b.max.x;
        let row = row.expand2(vec2(4.0, 0.0));
        // The field's value in the list's selected look, the row under the pointer with the bright rim (the boards'
        // menus); a menu without a value (the planet list) lights the pointer's row.
        let hot = ds::hover_t(ui.ctx(), egui::Id::new(("smh-select", r.text.as_str())), r.hovered);
        if current == Some(r.text.as_str()) || (current.is_none() && r.hovered) {
            ds::hgrad(p, row, ds::RADIUS_PLATE, &[(0.0, Color32::from_white_alpha(46)), (1.0, Color32::from_white_alpha(5))]);
            ds::rim(p, row, ds::RADIUS_PLATE, k.ink_silver);
        }
        if current.is_some() && hot > 0.0 {
            ds::rim(p, row.shrink(1.0), ds::RADIUS_PLATE, k.rim_bright.gamma_multiply(hot));
        }
        let inner = row.shrink2(vec2(6.0, 0.0));
        let base = crate::gui::ds::fit::baseline(inner, font);
        let w = Text::new(&r.text, font).color(k.ink).baseline(base).paint_only(ui, inner).width;
        if !r.sub.is_empty() {
            Text::new(&r.sub, sub).color(k.ink_muted).baseline(base).paint_only(ui, Rect::from_min_max(pos2(inner.left() + w + 10.0, inner.top()), inner.max));
        }
    }
    panel
}

/// A data line of the long info (`.ig-table`: the label muted and upper case in its column, the value in the data
/// style on the same baseline).
fn table_row(ui: &mut egui::Ui, label: &str, value: &str, c: Color32, pitch: f32) {
    let label = label.trim_end_matches(':');
    let (r, _) = ui.allocate_exact_size(vec2(LI_W, pitch), Sense::hover());
    let r = Rect::from_min_size(r.min + vec2(LI_PAD, 0.0), vec2(LI_W - 2.0 * LI_PAD, 20.0));
    let base = crate::gui::ds::fit::baseline(r, Font::DATA);
    let (a, b) = r.split_left_right_at_x(r.left() + LI_LABEL);
    Text::new(label, Font::LABEL).color(ds::tokens().ink_muted).baseline(base).paint(ui, a.with_max_x(a.right() - ds::SPACE[2]));
    Text::new(value, Font::DATA).color(c).baseline(base).paint(ui, b);
}

/// The planet's long info as the board's panel at the screen's right under the bar: the globe, name, owner and
/// ratios on a lit header, the original's lines as a data table, the problems, the settings (a field that opens its
/// menu, ALL, − and +), the production and exit plates. The panel takes no input itself: its controls are the
/// original's hot-spots moved here ([`State::info_pointer`]), lit from the original's hover and held state; the plates
/// are the card's production button and the info box's left click, which leave the long info.
fn long_info(ctx: &egui::Context, m: &Menus, i: &LongInfo, v: &View, st: &mut State, acts: &mut Vec<Act>) {
    let k = ds::tokens();
    let q = i.q.clamp(0.0, 1.0).max(i.exit_in.clamp(0.0, 1.0));
    let ease = 1.0 - (1.0 - q) * (1.0 - q);
    let at = pos2(ctx.screen_rect().right() - GAP - LI_W + 24.0 * (1.0 - ease), BAR_H + 28.0);
    let card = v.info.as_ref().filter(|c| c.planet);
    let mut spots = Vec::new();
    let mut plates = Rect::NOTHING;
    let panel = egui::Area::new(egui::Id::new("smh-long-info")).order(egui::Order::Background).interactable(false).fixed_pos(at).fade_in(false).show(ctx, |ui| {
        ui.set_opacity(ease);
        let back = ui.painter().add(Shape::Noop);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        ui.set_width(LI_W);
        // The header: the globe, the name, the owner glowing in its colour, the surface.
        let (h, _) = ui.allocate_exact_size(vec2(LI_W, LI_HEAD), Sense::hover());
        let p = ui.painter();
        ds::vgrad(p, h, CornerRadius { nw: ds::RADIUS_PANEL, ne: ds::RADIUS_PANEL, sw: 0, se: 0 }, &[(0.0, k.rim_bright.gamma_multiply(0.25)), (1.0, Color32::TRANSPARENT)]);
        planet(ui, Rect::from_min_size(h.min + vec2(LI_PAD, 16.0), vec2(LI_GLOBE, LI_GLOBE)), i.colour, v.globe, true, &mut st.wells);
        let tx = h.left() + LI_PAD + LI_GLOBE + 16.0;
        let tw = h.right() - LI_PAD - tx;
        // The board's header: the empire and the surface lines move up from the table.
        let row = |n: u16| i.lines.iter().find(|l| l.n == n).map(|l| clean(&l.value));
        let owner = row(3).unwrap_or_else(|| card.and_then(|c| c.lines.first()).map_or(String::new(), |l| l.0.clone()));
        let owner = owner.as_str();
        let sub = row(2).unwrap_or_default();
        let lines = [owner.is_empty(), sub.is_empty()].iter().filter(|e| !**e).count() as f32;
        let mut y = h.center().y - (30.0 + 22.0 * lines) / 2.0;
        Text::new(&i.title, LI_NAME).color(k.ink).paint(ui, Rect::from_min_size(pos2(tx, y), vec2(tw, 30.0)));
        y += 34.0;
        if !owner.is_empty() {
            let f = Text::new(owner, LI_OWNER).color(i.colour).layout(ui.ctx(), tw);
            let r = Rect::from_min_size(pos2(tx, y), vec2(tw, 18.0));
            ds::glowing_galley(ui.painter(), pos2(tx, crate::gui::ds::fit::baseline(r, LI_OWNER) - f.baseline()), &f.galley, i.colour);
            y += 22.0;
        }
        if !sub.is_empty() {
            Text::new(&sub, BUTTON_TEXT).color(k.ink_muted).paint(ui, Rect::from_min_size(pos2(tx, y), vec2(tw, 18.0)));
        }
        // The original's lines; the needed amount after the value. The rows close up where the screen is short.
        ui.add_space(4.0);
        // Shown in the header, or empty (the special line of a planet without one).
        let shown = |l: &&InfoRow| !(l.n == 2 || l.n == 3 || (l.special && l.value.trim().is_empty()));
        let rows = i.lines.iter().filter(shown).count() + i.problems.as_ref().map_or(0, |p| p.2.len());
        let rest = 4.0 + 12.0 + if i.settings.is_some() { 14.0 + 3.0 * (LI_ROUND + 8.0) } else { 0.0 } + LI_PLATES + LI_PAD + 4.0;
        let room = ctx.screen_rect().bottom() - GAP - at.y - LI_HEAD - rest;
        let pitch = (room / rows.max(1) as f32).clamp(20.0, 26.0);
        for l in i.lines.iter().filter(shown) {
            let c = if l.short { k.enemy } else if l.special { k.own } else { k.info };
            let value = match &l.needed {
                Some((_, n)) => format!("{} / {}", clean(&l.value), clean(n)),
                None => clean(&l.value),
            };
            table_row(ui, &l.label, &value, c, pitch);
        }
        if let Some((label, _, problems)) = &i.problems {
            for (n, (pct, name)) in problems.iter().enumerate() {
                table_row(ui, if n == 0 { label } else { "" }, &format!("{pct}% {name}"), k.enemy, pitch);
            }
        }
        ui.add_space(12.0);
        // The settings: one row each, the original's controls on the original's hot-spots.
        if let Some((labels, values, all)) = &i.settings {
            let (line, _) = ui.allocate_exact_size(vec2(LI_W, 14.0), Sense::hover());
            ui.painter().hline(line.x_range(), line.top() + 0.5, Stroke::new(1.0f32, k.rim));
            for n in 0..3 {
                let [field, all_id, minus, plus] = [[0x5a, 0x5b, 0x5c, 0x5d], [0x5e, 0x5f, 0, 0], [0x60, 0x61, 0x62, 0x63]][n];
                let (row, _) = ui.allocate_exact_size(vec2(LI_W, LI_ROUND + 8.0), Sense::hover());
                let row = Rect::from_min_size(row.min + vec2(LI_PAD, 0.0), vec2(LI_W - 2.0 * LI_PAD, LI_ROUND));
                let state = |id: u32| (ds::hover_t(ctx, egui::Id::new(("smh-li", id)), i.hot == Some(id)), i.hot == Some(id) && i.held);
                Text::new(labels[n].trim_end_matches(':'), Font::LABEL).color(k.ink_muted).paint(ui, row.with_max_x(row.left() + LI_SET_LABEL));
                let fw = row.width() - LI_SET_LABEL - LI_ALL - 2.0 * LI_ROUND - 4.0 * 8.0;
                let mut x = row.left() + LI_SET_LABEL + 8.0;
                let mut next = |w: f32, h: f32| {
                    let r = Rect::from_min_size(pos2(x, row.center().y - h / 2.0), vec2(w, h));
                    x += w + 8.0;
                    r
                };
                // The value: a field with a chevron that opens its menu.
                let r = next(fw, 36.0);
                let (hot, down) = state(field);
                let p = ui.painter();
                p.rect_filled(r, ds::RADIUS_PLATE, if down { k.plate_low } else { k.glass_deep });
                ds::rim(p, r, ds::RADIUS_PLATE, k.rim_bright.lerp_to_gamma(k.focus, hot));
                Text::new(&values[n], Font::DATA).color(k.ink).paint_only(ui, r.shrink2(vec2(10.0, 0.0)).with_max_x(r.right() - 26.0));
                ds::Icon::ChevronDown.paint(p, Rect::from_center_size(pos2(r.right() - 15.0, r.center().y), vec2(16.0, 16.0)), k.ink_silver.lerp_to_gamma(k.ink, hot));
                spots.push((field, r));
                // ALL: a small plate.
                let r = next(LI_ALL, 36.0);
                let (hot, down) = state(all_id);
                ds::plate_fill(ui.painter(), r, ds::RADIUS_PLATE, down);
                if hot > 0.0 {
                    ds::rim(ui.painter(), r, ds::RADIUS_PLATE, k.rim_bright.gamma_multiply(hot));
                }
                Text::new(&calm(all), Font::LABEL.tracking(0.12).upper(false)).center().shrink(9.0).color(k.ink).paint_only(ui, r.shrink2(vec2(3.0, 0.0)));
                spots.push((all_id, r));
                for (id, glyph) in [(minus, ds::Icon::Minus), (plus, ds::Icon::Plus)] {
                    let r = next(LI_ROUND, LI_ROUND);
                    if id != 0 {
                        let (hot, down) = state(id);
                        ds::forced(hot > 0.5, down, || ui.put(r, |ui: &mut egui::Ui| ds::round_button(ui, ds::Glyph::Icon(glyph), true, false)));
                        spots.push((id, r));
                    }
                }
            }
        }
        // Room for the plates (drawn on their own, they take the pointer).
        let (r, _) = ui.allocate_exact_size(vec2(LI_W, LI_PLATES + LI_PAD + 4.0), Sense::hover());
        plates = Rect::from_min_size(r.min + vec2(LI_PAD, 4.0), vec2(LI_W - 2.0 * LI_PAD, LI_PLATES));
        let rect = ui.min_rect();
        ds::behind(ui, back, |p| ds::paint_panel(p, rect, ds::PanelKind::Glass, 0.94));
    });
    st.placed = Some(Placed { map: m.map, panel: panel.response.rect, spots, menu: None });
    // Production (the card's button, hot-spot 11) and the exit (the info box's left click, hot-spot 8): both leave.
    egui::Area::new(egui::Id::new("smh-long-info-plates")).fixed_pos(plates.min).fade_in(false).show(ctx, |ui| {
        ui.set_opacity(ease);
        ui.spacing_mut().item_spacing.x = 8.0;
        // The board's three plates less the colony one (the original's long info has none): two across the width.
        let w = ((plates.width() - 8.0) / 2.0).floor();
        let font = Plate::FONT.sized(15.0).tracking(0.12);
        ui.horizontal(|ui| {
            // The original's production button (hot-spot 11) leaves the long info whatever the planet.
            if hand(ui.add_sized(vec2(w, LI_PLATES), Plate::new(&calm(&v.tips[9])).width(w).font(font))).clicked() {
                acts.push(Act::Panel(11));
            }
            if hand(ui.add_sized(vec2(w, LI_PLATES), Plate::new(&calm(&i.exit)).width(w).font(font))).clicked() {
                acts.push(Act::Info(false));
            }
        });
    });
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
    fn calm_writes_capitals_small() {
        assert_eq!(calm("KOLÓNIA"), "Kolónia");
        assert_eq!(calm("Részletek"), "Részletek");
        assert_eq!(calm(""), "");
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
        let at = |ctx: &egui::Context| area(ctx, BAR_ID).left_top() + vec2(BAR_PAD.x + 2.0 + KEY.x / 2.0, BAR_H / 2.0);
        let acts = drive(&view(), &[(None, None), (None, Some((0, true))), (None, Some((0, false)))], at);
        assert_eq!(acts, vec![Act::Pause]);
    }

    /// The screens are keys at the bar's right end: a click picks the screen.
    #[test]
    fn screen_icon_click() {
        let at = |ctx: &egui::Context| area(ctx, BAR_ID).right_top() + vec2(-BAR_PAD.x - 2.0 - KEY.x / 2.0, BAR_H / 2.0);
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

    /// The menus and the long info are drawn only: no panel under the pointer over them and no act for a click (the
    /// window's hot-spot code takes the press, as the original's); the radial icons come in from frame 30 to 50.
    #[test]
    fn menus_take_no_input() {
        assert_eq!((radial_in(0), radial_in(30), radial_in(50), radial_in(70)), (0.0, 0.0, 1.0, 1.0));
        assert!(radial_in(40) > 0.5 && radial_in(40) < 1.0);
        let mut v = view();
        let centres = crate::starmap_menus::HIT.map(|c| [(238 + c[0]) as f32, (156 + c[1]) as f32]);
        v.menus = Menus {
            map: [240.0, 0.0, 2.25],
            radial: Some(Radial { at: [320.0, 240.0], centres, icons: [2, 4, 3, 9], frame: 50, hover: 3, ..Default::default() }),
            select: Some(Select { rows: vec![SelectRow { rect: [400.0, 300.0, 121.0, 13.0], text: "Low".into(), hovered: true, ..Default::default() }], field: None, planets: false }),
            info: Some(LongInfo { q: 1.0, exit_in: 1.0, title: "Magellan 10".into(), ..Default::default() }),
        };
        // On the select menu's row, on the long info's panel (at the screen's right), on a radial icon.
        for at in [pos2(240.0 + 450.0 * 2.25, 306.0 * 2.25), pos2(SIZE.0 as f32 - GAP - LI_W / 2.0, BAR_H + 28.0 + 100.0), pos2(240.0 + 370.0 * 2.25, 242.0 * 2.25)] {
            let acts = drive(&v, &[(None, None), (None, Some((0, true))), (None, Some((0, false)))], |_| at);
            assert!(acts.is_empty(), "{acts:?}");
            let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
            let style = crate::gui::style(&crate::theme::Theme::modern());
            let mut st = State::default();
            g.pointer(Some([at.x, at.y]), crate::gui::Gui::scale(SIZE.1));
            for _ in 0..3 {
                g.run(SIZE, &style, |ctx| {
                    show(ctx, &v, &mut st);
                });
            }
            assert!(!g.ctx.is_pointer_over_area());
        }
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
        // The strip's third key (zoom in), under the radar.
        let x0 = 10.0 + ((NAV_W - group_w(6, CAM_KEY.x) - group_w(2, CAM_KEY.x) - 4.0) / 2.0).max(0.0) + 2.0;
        let p = rect.left_bottom() + vec2(x0 + 2.0 * (CAM_KEY.x + 2.0) + CAM_KEY.x / 2.0, -10.0 - 2.0 - CAM_KEY.y / 2.0);
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

    /// Under the modern long info the pointer reaches the original's hot-spots where their controls are drawn: a field
    /// is its spot's middle, the rest of the panel and the original's places are the panel's empty part, a settings
    /// menu is moved back to where it opened; elsewhere the pointer is unchanged.
    #[test]
    fn long_info_pointer_reaches_the_original_spots() {
        let mut v = view();
        let map = [240.0, 0.0, 2.25];
        let settings = (["Tax:".into(), "Auto build:".into(), "Priority:".into()], ["Low".into(), "Civil".into(), "Normal".into()], "ALL".into());
        v.menus = Menus { map, info: Some(LongInfo { q: 1.0, exit_in: 1.0, title: "Magellan 10".into(), settings: Some(settings), ..Default::default() }), ..Default::default() };
        let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        let style = crate::gui::style(&crate::theme::Theme::modern());
        let mut st = State::default();
        for _ in 0..3 {
            g.run(SIZE, &style, |ctx| _ = show(ctx, &v, &mut st));
        }
        let pl = st.placed.clone().expect("placed");
        let virt = |p: Pos2| [(p.x - map[0]) / map[2], (p.y - map[1]) / map[2]];
        // Each control on its spot; the tax field (0x5a: 400, 289, 100 × 16) and the priority's plus (0x63).
        for (id, want) in [(0x5a, [450.0, 297.0]), (0x63, [578.0, 341.0]), (0x5f, [521.0, 319.0])] {
            let r = pl.spots.iter().find(|s| s.0 == id).expect("spot").1;
            assert!(r.left() > SIZE.0 as f32 - GAP - LI_W, "{id:#x} {r:?}");
            assert_eq!(st.info_pointer(virt(r.center())), want, "{id:#x}");
        }
        // The panel's header: the original panel's empty part, no control.
        assert_eq!(st.info_pointer(virt(pl.panel.left_top() + vec2(40.0, 40.0))), NEUTRAL);
        // The original's tax field and exit button, where nothing modern is: no press reaches them.
        assert_eq!(st.info_pointer([450.0, 297.0]), NEUTRAL);
        assert_eq!(st.info_pointer([500.0, 400.0]), NEUTRAL);
        // The open map: unchanged.
        assert_eq!(st.info_pointer([100.0, 200.0]), [100.0, 200.0]);
        // A tax menu opened at the field's spot (its rows from the press point) is drawn at the modern field, and the
        // pointer over a drawn row reaches the row.
        v.menus.select = Some(Select { rows: vec![SelectRow { rect: [450.0, 297.0, 80.0, 13.0], text: "Low".into(), ..Default::default() }, SelectRow { rect: [450.0, 310.0, 80.0, 13.0], text: "High".into(), ..Default::default() }], field: Some(0), planets: false });
        for _ in 0..2 {
            g.run(SIZE, &style, |ctx| _ = show(ctx, &v, &mut st));
        }
        let (shift, bounds) = st.placed.as_ref().and_then(|p| p.menu).expect("menu");
        let field = st.placed.as_ref().unwrap().spots.iter().find(|s| s.0 == 0x5a).unwrap().1;
        assert!((field.left() - (map[0] + (450.0 + shift[0]) * map[2])).abs() < 1.0, "{shift:?}");
        // The field's value ("Low", the first row) lies on the field (ADR 0077).
        assert!((field.center().y - (map[1] + (303.5 + shift[1]) * map[2])).abs() < 1.0, "{shift:?}");
        let row = pos2(map[0] + (470.0 + shift[0]) * map[2], map[1] + (316.0 + shift[1]) * map[2]);
        assert!(bounds.contains(row));
        let back = st.info_pointer(virt(row));
        assert!((back[0] - 470.0).abs() < 0.01 && (back[1] - 316.0).abs() < 0.01, "{back:?}");
        // Off the drawn menu, the original's rows at their old place answer nothing (no unseen pick).
        assert_eq!(st.info_pointer([470.0, 316.0]), NEUTRAL);        // The menu's kind names its field, wherever the press opened it: an autobuild menu opened nearer the tax
        // field still opens on the autobuild one, its value ("Civil") marked there; the list is the field's width.
        v.menus.select = Some(Select { rows: vec![SelectRow { rect: [450.0, 297.0, 121.0, 13.0], text: "None".into(), ..Default::default() }, SelectRow { rect: [450.0, 310.0, 121.0, 13.0], text: "Civil".into(), ..Default::default() }], field: Some(1), planets: false });
        for _ in 0..2 {
            g.run(SIZE, &style, |ctx| _ = show(ctx, &v, &mut st));
        }
        let (shift, bounds) = st.placed.as_ref().and_then(|p| p.menu).expect("menu");
        let field = st.placed.as_ref().unwrap().spots.iter().find(|s| s.0 == 0x5e).unwrap().1;
        assert!((field.center().y - (map[1] + (316.5 + shift[1]) * map[2])).abs() < 1.0, "{shift:?}");
        assert!(bounds.width() < field.width() + 12.0, "{bounds:?} {field:?}");
    }

    /// The planet list opens under the production view's plate at the board's rows, the plate's planet marked, and
    /// the pointer on a drawn row reaches the original's row; without the plate the list stays the original's.
    #[test]
    fn planet_list_opens_under_the_plate() {
        let mut v = view();
        v.menus = Menus { map: [240.0, 0.0, 2.25], ..Default::default() };
        let row = |y: f32, t: &str| SelectRow { rect: [300.0, y, 120.0, 13.0], text: t.into(), ..Default::default() };
        v.menus.select = Some(Select { rows: vec![row(90.0, "Tra 5"), row(103.0, "Magellan 10")], field: None, planets: true });
        let plate = Rect::from_min_size(pos2(780.0, 64.0), vec2(360.0, 60.0));
        let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        let style = crate::gui::style(&crate::theme::Theme::modern());
        let mut st = State::default();
        g.run(SIZE, &style, |ctx| {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new(PLATE_ID), (plate, "Magellan 10".to_string())));
            _ = show(ctx, &v, &mut st);
        });
        let (_, rows) = st.list.clone().expect("moved list");
        assert!(rows[0].0.top() > plate.bottom() && (rows[1].0.top() - rows[0].0.top() - 52.0).abs() < 0.5, "{rows:?}");
        let virt = |p: Pos2| [(p.x - 240.0) / 2.25, p.y / 2.25];
        assert_eq!(st.list_pointer(virt(rows[1].0.center())), [360.0, 109.5]);
        assert_eq!(st.list_pointer([10.0, 400.0]), [10.0, 400.0]);
        // The next frame without the production view's plate: the original's list.
        g.run(SIZE, &style, |ctx| _ = show(ctx, &v, &mut st));
        assert!(!st.list_open());
    }

    /// Long texts (a long translation, a long name) never resize the panels: they are cut or shrunk inside them.
    #[test]
    fn long_texts_keep_the_panels_size() {
        let long = "Hosszú szöveg, amely sokkal hosszabb, mint amennyi hely van rá ".repeat(4);
        let mut v = view();
        v.line = long.clone();
        v.date = "2327 : 10 : 23".into();
        v.globe = true;
        v.research.current = Some((1, long.clone(), 64, 100));
        if let Some(i) = &mut v.info {
            i.title = long.clone();
            i.lines = vec![(long.clone(), Color32::WHITE); 3];
            i.left = long.clone();
            i.right = Some(long.clone());
            i.stats = vec![(long.clone(), long.clone()); 2];
        }
        let settings = ([long.clone(), long.clone(), long.clone()], [long.clone(), long.clone(), long.clone()], long.clone());
        let mut rows = vec![InfoRow { label: long.clone(), value: long.clone(), needed: Some((long.clone(), long.clone())), ..Default::default() }; 3];
        // The surface and the empire go up to the header.
        (rows[0].n, rows[1].n) = (2, 3);
        let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        let style = crate::gui::style(&crate::theme::Theme::modern());
        let mut st = State::default();
        let mut rects = Vec::new();
        for info in [None, Some(LongInfo { q: 1.0, exit_in: 1.0, title: long.clone(), lines: rows, settings: Some(settings), exit: long.clone(), ..Default::default() })] {
            v.menus.info = info;
            for _ in 0..3 {
                g.run(SIZE, &style, |ctx| {
                    show(ctx, &v, &mut st);
                    rects = ["smh-left", "smh-info", "smh-mini", "smh-long-info"].iter().map(|id| ctx.memory(|m| m.area_rect(egui::Id::new(*id)))).collect();
                });
            }
            let [bar, card, nav, li] = [rects[0], rects[1], rects[2], rects[3]];
            assert_eq!(bar.map(|r| r.size()), Some(vec2(SIZE.0 as f32, BAR_H)));
            assert_eq!(nav.map(|r| r.width()), Some(NAV_W + 20.0));
            // One planet picture: the card's, or the long info's (the card is hidden under it).
            assert_eq!(st.wells.len(), 1);
            if v.menus.info.is_none() {
                assert_eq!(card.map(|r| r.width()), Some(CARD_W));
            } else {
                assert_eq!(li.map(|r| r.width()), Some(LI_W));
            }
        }
    }
}
