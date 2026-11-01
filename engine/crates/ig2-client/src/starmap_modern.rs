//! Ours (ADR 0065): the modern starmap HUD on the egui layer (ADR 0062), the "Modern starmap HUD" option. The same
//! functions as the original's HUD (re/screens/starmap.md §11, re/screens/hud.md §2, re/screens/input.md §6): the top
//! bar's pause and speeds, date, money, message line and history, its screen and quick-research menus; the selection's
//! short info with its tanks and production buttons; the console's camera buttons and the minimap. The window fills
//! a [`View`] from the same state the classic HUD reads, runs [`show`] once a frame and applies the [`Act`]s through
//! the classic hot-spots' own code, so the orders, sounds and tutorial gates are the same. The radial menu, the select
//! menus and the panels stay the original's.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Margin, Pos2, Rect, Sense, Shadow, Stroke, StrokeKind};

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

/// The UI theme (ADR 0056) as the HUD's egui style.
pub fn style(h: &crate::ui::Hud, size: [f32; 2]) -> egui::Style {
    let theme = h.font.as_ref().map_or_else(crate::theme::Theme::modern, |font| {
        crate::theme::look(&crate::options::Draw { font, white: h.white, glow: h.white, v: crate::font::Virtual::new(size), palette: &h.palette, texts: &h.texts, now_ms: 0.0 })
    });
    crate::gui::style(&theme)
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
    pub screens: String,
    pub research: String,
    pub idle: String,
    pub redirect: String,
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
    /// The blink's phase (the selected dots, the redirect mark).
    pub blink: bool,
}

/// What stays between frames.
#[derive(Debug, Clone, Default)]
pub struct State {
    menu: Menu,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum Menu {
    #[default]
    None,
    Screens,
    Research,
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
    Grid,
    Tanks,
    Factory,
}

fn icon(p: &egui::Painter, r: Rect, i: Icon, c: Color32) {
    let s = Stroke::new(1.8f32, c);
    let m = r.center();
    let k = r.height().min(r.width()) * 0.32;
    let tri = |at: Pos2, w: f32| vec![pos2(at.x - w * 0.5, at.y - k), pos2(at.x + w * 0.5, at.y), pos2(at.x - w * 0.5, at.y + k)];
    match i {
        Icon::Pause => {
            for dx in [-0.45, 0.45] {
                p.rect_filled(Rect::from_center_size(pos2(m.x + dx * k, m.y), vec2(k * 0.5, k * 2.0)), 1.0, c);
            }
        }
        Icon::Play(n) => {
            let w = k * 1.3;
            let n = n.max(1) as f32;
            let step = w * 0.62;
            let x0 = m.x - (n - 1.0) * step / 2.0;
            for j in 0..n as usize {
                p.add(egui::Shape::convex_polygon(tri(pos2(x0 + j as f32 * step, m.y), w), c, Stroke::NONE));
            }
        }
        Icon::RotateLeft | Icon::RotateRight => {
            let dir = if i == Icon::RotateLeft { -1.0 } else { 1.0 };
            let pts: Vec<Pos2> = (0..=16).map(|j| {
                let a = std::f32::consts::PI * (0.15 + 1.2 * j as f32 / 16.0);
                pos2(m.x + dir * a.cos() * k, m.y - a.sin() * k * 0.9 + k * 0.25)
            }).collect();
            let end = pts[0];
            p.add(egui::Shape::line(pts, s));
            p.add(egui::Shape::convex_polygon(vec![pos2(end.x - dir * k * 0.45, end.y - k * 0.1), pos2(end.x + dir * k * 0.25, end.y - k * 0.15), pos2(end.x - dir * k * 0.05, end.y + k * 0.5)], c, Stroke::NONE));
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            let o = pos2(m.x - k * 0.2, m.y - k * 0.2);
            p.circle_stroke(o, k * 0.75, s);
            p.line_segment([pos2(o.x + k * 0.55, o.y + k * 0.55), pos2(m.x + k, m.y + k)], Stroke::new(2.4f32, c));
            p.line_segment([pos2(o.x - k * 0.4, o.y), pos2(o.x + k * 0.4, o.y)], s);
            if i == Icon::ZoomIn {
                p.line_segment([pos2(o.x, o.y - k * 0.4), pos2(o.x, o.y + k * 0.4)], s);
            }
        }
        Icon::TiltUp | Icon::TiltDown => {
            let d = if i == Icon::TiltUp { -1.0 } else { 1.0 };
            p.add(egui::Shape::convex_polygon(vec![pos2(m.x, m.y + d * k), pos2(m.x - k, m.y - d * k * 0.2), pos2(m.x + k, m.y - d * k * 0.2)], Color32::TRANSPARENT, s));
            p.line_segment([pos2(m.x - k * 1.1, m.y - d * k * 0.75), pos2(m.x + k * 1.1, m.y - d * k * 0.75)], s);
        }
        Icon::TopView => {
            p.circle_stroke(m, k, s);
            p.circle_filled(m, k * 0.3, c);
        }
        Icon::SideView => {
            let pts: Vec<Pos2> = (0..=24).map(|j| {
                let a = std::f32::consts::TAU * j as f32 / 24.0;
                pos2(m.x + a.cos() * k * 1.1, m.y + a.sin() * k * 0.4)
            }).collect();
            p.add(egui::Shape::line(pts, s));
            p.circle_filled(m, k * 0.3, c);
        }
        Icon::Bolt => {
            let pts = [(0.2, -1.0), (-0.55, 0.15), (0.0, 0.15), (-0.2, 1.0), (0.55, -0.15), (0.0, -0.15), (0.2, -1.0)];
            p.add(egui::Shape::line(pts.iter().map(|&(x, y)| pos2(m.x + x * k, m.y + y * k)).collect(), s));
        }
        Icon::Grid => {
            for (x, y) in [(-1.0, -1.0), (0.15, -1.0), (-1.0, 0.15), (0.15, 0.15)] {
                p.rect_stroke(Rect::from_min_size(pos2(m.x + x * k, m.y + y * k), vec2(k * 0.85, k * 0.85)), 1.5, s, StrokeKind::Middle);
            }
        }
        Icon::Tanks => {
            p.rect_stroke(Rect::from_center_size(pos2(m.x, m.y + k * 0.35), vec2(k * 2.0, k * 0.8)), k * 0.4, s, StrokeKind::Middle);
            p.rect_filled(Rect::from_center_size(pos2(m.x - k * 0.1, m.y - k * 0.3), vec2(k * 0.9, k * 0.55)), 2.0, c);
            p.line_segment([pos2(m.x + k * 0.3, m.y - k * 0.35), pos2(m.x + k * 1.15, m.y - k * 0.55)], s);
        }
        Icon::Factory => {
            let b = m.y + k;
            let pts = [(-1.0, 1.0), (-1.0, -0.2), (-0.35, 0.2), (-0.35, -0.2), (0.3, 0.2), (0.3, -1.0), (0.75, -1.0), (0.75, 1.0)];
            let mut v: Vec<Pos2> = pts.iter().map(|&(x, y)| pos2(m.x + x * k, m.y + y * k)).collect();
            v.push(pos2(m.x - k, b));
            p.add(egui::Shape::line(v, s));
        }
    }
}

/// A HUD panel's glass: the theme's window fill, a little clearer, its edge and a soft shadow.
fn glass(ui: &egui::Ui) -> egui::Frame {
    let v = ui.visuals();
    let [r, g, b, _] = v.window_fill.to_array();
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(r, g, b, 214))
        .stroke(v.window_stroke)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(10, 6))
        .shadow(Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(110) })
}

/// A square icon button: hover and press states, `on` for the chosen one. Its response (held, clicked).
fn icon_button(ui: &mut egui::Ui, i: Icon, on: bool, size: f32, tip: &str) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let v = ui.visuals();
    let (fill, fg) = if resp.is_pointer_button_down_on() {
        (v.widgets.active.weak_bg_fill, v.widgets.active.fg_stroke.color)
    } else if on {
        (v.selection.bg_fill, v.widgets.active.fg_stroke.color)
    } else if resp.hovered() {
        (v.widgets.hovered.weak_bg_fill, v.widgets.hovered.fg_stroke.color)
    } else {
        (Color32::TRANSPARENT, v.widgets.inactive.fg_stroke.color)
    };
    ui.painter().rect_filled(r, 7.0, fill);
    if resp.hovered() && !on {
        ui.painter().rect_stroke(r, 7.0, v.widgets.hovered.bg_stroke, StrokeKind::Inside);
    }
    icon(ui.painter(), r, i, fg);
    if tip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tip)
    }
}

/// Clicked with the left or the right button.
fn either(r: &egui::Response) -> bool {
    r.clicked() || r.secondary_clicked()
}

fn divider(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(1.0, 22.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
}

/// The HUD's points: the top bar's height and the panels' gap to the edges.
const BAR_H: f32 = 34.0;
const EDGE: f32 = 10.0;

/// One frame of the HUD over the starmap; the acts in the order they happened.
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) -> Vec<Act> {
    let mut acts = Vec::new();
    let text = ctx.style().visuals.widgets.inactive.fg_stroke.color;
    let accent = ctx.style().visuals.hyperlink_color;
    let dim = ctx.style().visuals.weak_text_color();
    let mut anchors: [Rect; 2] = [Rect::NOTHING; 2];
    // The left of the top bar: pause and the speeds, the date, the money.
    let left = egui::Area::new(egui::Id::new("smh-left")).fixed_pos(pos2(EDGE, EDGE * 0.6)).fade_in(false).show(ctx, |ui| {
        glass(ui).show(ui, |ui| {
            ui.set_height(BAR_H - 12.0);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if either(&icon_button(ui, Icon::Pause, v.paused, 24.0, &v.labels.pause)) {
                    acts.push(Act::Pause);
                }
                for n in 1..=3u8 {
                    let on = !v.paused && v.speed + 1 == n as usize;
                    let tip = format!("{} {n}", v.labels.speed);
                    if either(&icon_button(ui, Icon::Play(n), on, 24.0, &tip)) {
                        acts.push(Act::Speed(n as i32));
                    }
                }
                if v.turbo > 1 {
                    ui.label(egui::RichText::new(format!("×{}", v.turbo)).small().strong().color(accent));
                }
                ui.add_space(8.0);
                divider(ui);
                ui.add_space(8.0);
                ui.label(egui::RichText::new(&v.date).size(15.0).color(text).monospace());
                ui.add_space(8.0);
                divider(ui);
                ui.add_space(8.0);
                ui.label(egui::RichText::new(&v.money).size(15.0).strong().color(accent));
                ui.add_space(4.0);
            });
        });
    });
    anchors[0] = left.response.rect;
    // The right of the top bar: the quick research and the screen menu.
    let right = egui::Area::new(egui::Id::new("smh-right")).anchor(Align2::RIGHT_TOP, vec2(-EDGE, EDGE * 0.6)).fade_in(false).show(ctx, |ui| {
        glass(ui).show(ui, |ui| {
            ui.set_height(BAR_H - 12.0);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let r = &v.research;
                let (label, on) = match &r.current {
                    Some((_, name, pct, _)) => (format!("{name}  {pct}%"), true),
                    None => (v.labels.idle.clone(), false),
                };
                let colour = if on { accent } else if r.researchable { text } else { dim };
                let (rect, resp) = ui.allocate_exact_size(vec2(190.0, 24.0), Sense::click());
                let vis = ui.visuals();
                if st.menu == Menu::Research || resp.hovered() {
                    let fill = if st.menu == Menu::Research { vis.selection.bg_fill } else { vis.widgets.hovered.weak_bg_fill };
                    ui.painter().rect_filled(rect, 7.0, fill);
                }
                icon(ui.painter(), Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(20.0, 20.0)), Icon::Bolt, colour);
                let galley = ui.painter().layout_no_wrap(label, FontId::proportional(14.0), colour);
                let at = pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0);
                ui.painter().with_clip_rect(rect.shrink(2.0)).galley(at, galley, colour);
                if resp.on_hover_text(&v.labels.research).clicked() {
                    st.menu = if st.menu == Menu::Research { Menu::None } else { Menu::Research };
                }
                if icon_button(ui, Icon::Grid, st.menu == Menu::Screens, 24.0, &v.labels.screens).clicked() {
                    st.menu = if st.menu == Menu::Screens { Menu::None } else { Menu::Screens };
                }
            });
        });
    });
    anchors[1] = right.response.rect;
    // The middle: the message line (left: the messages screen, right held: the history).
    // (On the first frame the right part has no place yet: its usual width.)
    let right_x = if anchors[1].is_positive() { anchors[1].left() } else { ctx.screen_rect().right() - 260.0 };
    let line_rect = Rect::from_min_max(pos2(anchors[0].right() + EDGE, anchors[0].top()), pos2(right_x - EDGE, anchors[0].bottom()));
    if line_rect.width() > 60.0 {
        egui::Area::new(egui::Id::new("smh-line")).fixed_pos(line_rect.min).fade_in(false).show(ctx, |ui| {
            glass(ui).show(ui, |ui| {
                let inner = vec2(line_rect.width() - 20.0, BAR_H - 12.0);
                let (r, resp) = ui.allocate_exact_size(inner, Sense::click());
                let colour = if v.waiting { accent.gamma_multiply(if v.blink { 1.0 } else { 0.55 }) } else { text };
                let font = FontId::proportional(14.0);
                let galley = ui.painter().layout_no_wrap(v.line.clone(), font, colour);
                let at = pos2(r.center().x - galley.size().x.min(r.width()) / 2.0, r.center().y - galley.size().y / 2.0);
                ui.painter().with_clip_rect(r).galley(at, galley, colour);
                if resp.hovered() {
                    ui.painter().rect_stroke(r.expand2(vec2(6.0, 2.0)), 7.0, ui.visuals().widgets.hovered.bg_stroke, StrokeKind::Inside);
                }
                let right_held = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary));
                if right_held != v.history_open {
                    acts.push(Act::History(right_held));
                }
                // The left button held over the line asks every frame (`MOUSE_SelectScreens`, `top_bar_frame`).
                if resp.contains_pointer() && ui.input(|i| i.pointer.primary_down()) && resp.is_pointer_button_down_on() {
                    acts.push(Act::Messages);
                }
                if v.history_open && !v.history.is_empty() {
                    egui::Area::new(egui::Id::new("smh-history")).fixed_pos(pos2(r.left(), r.bottom() + 12.0)).order(egui::Order::Foreground).fade_in(false).show(ui.ctx(), |ui| {
                        glass(ui).show(ui, |ui| {
                            ui.set_width(r.width());
                            for h in &v.history {
                                ui.label(egui::RichText::new(h).size(13.5).color(text));
                            }
                        });
                    });
                }
            });
        });
    }
    // The drop-downs under the right part.
    let menu_at = pos2(anchors[1].right(), anchors[1].bottom() + 6.0);
    match st.menu {
        Menu::Screens => {
            let r = egui::Area::new(egui::Id::new("smh-screens")).pivot(Align2::RIGHT_TOP).fixed_pos(menu_at).order(egui::Order::Foreground).fade_in(false).show(ctx, |ui| {
                glass(ui).inner_margin(Margin::same(6)).show(ui, |ui| {
                    ui.set_width(210.0);
                    for s in &v.screens {
                        let label = egui::RichText::new(&s.name).color(if s.on { text } else { dim });
                        let b = egui::Button::new(label).right_text(egui::RichText::new(&s.key).small().color(dim)).frame_when_inactive(false).selected(s.current).min_size(vec2(210.0, 24.0));
                        if ui.add_enabled(s.on, b).clicked() && !s.current {
                            acts.push(Act::Screen(s.n));
                            st.menu = Menu::None;
                        }
                    }
                });
            });
            close_outside(ctx, st, &[r.response.rect, anchors[1]]);
        }
        Menu::Research => {
            let r = egui::Area::new(egui::Id::new("smh-research")).pivot(Align2::RIGHT_TOP).fixed_pos(menu_at).order(egui::Order::Foreground).fade_in(false).show(ctx, |ui| {
                glass(ui).inner_margin(Margin::same(10)).show(ui, |ui| research_menu(ui, v, st, &mut acts));
            });
            close_outside(ctx, st, &[r.response.rect, anchors[1]]);
        }
        Menu::None => {}
    }
    // The selection's card, bottom left.
    if let Some(info) = &v.info {
        egui::Area::new(egui::Id::new("smh-info")).anchor(Align2::LEFT_BOTTOM, vec2(EDGE, -EDGE)).fade_in(false).show(ctx, |ui| {
            glass(ui).inner_margin(Margin::same(12)).show(ui, |ui| info_card(ui, info, v, &mut acts));
        });
    }
    // The minimap and the camera buttons, bottom right.
    if let Some(mini) = &v.mini {
        egui::Area::new(egui::Id::new("smh-mini")).anchor(Align2::RIGHT_BOTTOM, vec2(-EDGE, -EDGE)).fade_in(false).show(ctx, |ui| {
            glass(ui).inner_margin(Margin::same(8)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    camera_buttons(ui, v, &mut acts);
                    minimap(ui, mini, v, &mut acts);
                });
            });
        });
    } else {
        acts.push(Act::Hold(None));
    }
    acts
}

/// A click anywhere but the menu and its button closes the menu.
fn close_outside(ctx: &egui::Context, st: &mut State, keep: &[Rect]) {
    let (clicked, at) = ctx.input(|i| (i.pointer.any_pressed(), i.pointer.interact_pos()));
    if clicked && at.is_some_and(|p| !keep.iter().any(|r| r.contains(p))) {
        st.menu = Menu::None;
    }
}

fn research_menu(ui: &mut egui::Ui, v: &View, st: &mut State, acts: &mut Vec<Act>) {
    let r = &v.research;
    let (text, accent, dim) = (ui.visuals().widgets.inactive.fg_stroke.color, ui.visuals().hyperlink_color, ui.visuals().weak_text_color());
    let cols = r.headers.len().max(1);
    egui::Grid::new("smh-research-grid").num_columns(cols).spacing(vec2(14.0, 2.0)).show(ui, |ui| {
        for h in &r.headers {
            ui.label(egui::RichText::new(h.to_uppercase()).small().strong().color(accent));
        }
        ui.end_row();
        let rows = (0..cols).map(|c| r.items.iter().filter(|i| i.2 == c).count()).max().unwrap_or(0);
        for row in 0..rows {
            for c in 0..cols {
                match r.items.iter().filter(|i| i.2 == c).nth(row) {
                    Some((t, name, _, ok, short, cost)) => {
                        let colour = if *short || r.current.is_some() { dim } else { text };
                        let b = egui::Button::new(egui::RichText::new(name).color(colour)).frame_when_inactive(false).min_size(vec2(130.0, 22.0));
                        let resp = ui.add_enabled(*ok, b);
                        let resp = if *ok { resp.on_hover_text(format!("{}: {cost}", r.footer[3])) } else { resp };
                        if resp.clicked() {
                            acts.push(Act::Start(*t));
                            st.menu = Menu::None;
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
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{}: {name}", r.footer[0])).color(text));
            ui.add(egui::ProgressBar::new(*pct as f32 / 100.0).desired_width(90.0).text(format!("{pct}%")));
            if ui.button(&r.footer[1]).clicked() {
                acts.push(Act::Stop(*t));
                st.menu = Menu::None;
            }
        });
        ui.label(egui::RichText::new(format!("{}: {left}", r.footer[2])).small().color(dim));
    }
}

fn info_card(ui: &mut egui::Ui, info: &Info, v: &View, acts: &mut Vec<Act>) {
    let (text, accent, dim) = (ui.visuals().widgets.inactive.fg_stroke.color, ui.visuals().hyperlink_color, ui.visuals().weak_text_color());
    ui.set_width(260.0);
    let body = ui.scope(|ui| {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
            if info.planet {
                ui.painter().circle_filled(r.center(), 10.0, info.colour);
                ui.painter().circle_stroke(r.center(), 12.5, Stroke::new(1.2f32, info.colour.gamma_multiply(0.6)));
            } else {
                let c = r.center();
                ui.painter().add(egui::Shape::convex_polygon(vec![pos2(c.x + 11.0, c.y), pos2(c.x - 9.0, c.y - 8.0), pos2(c.x - 4.0, c.y), pos2(c.x - 9.0, c.y + 8.0)], info.colour, Stroke::NONE));
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add(egui::Label::new(egui::RichText::new(&info.title).size(18.0).strong().color(text)).truncate());
                if let Some((l, c)) = info.lines.first() {
                    ui.add(egui::Label::new(egui::RichText::new(l).size(12.5).color(*c)).truncate());
                }
            });
        });
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 1.0;
        for (l, c) in info.lines.iter().skip(1) {
            ui.add(egui::Label::new(egui::RichText::new(l).size(13.5).color(*c)).truncate());
        }
        if !info.stats.is_empty() {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                for (k, val) in &info.stats {
                    ui.label(egui::RichText::new(k).small().color(dim));
                    ui.label(egui::RichText::new(val).strong().color(accent));
                    ui.add_space(6.0);
                }
            });
        }
    });
    // The card itself is the info box (hot-spot 8): left and right click as the original's.
    let card = ui.interact(body.response.rect, ui.id().with("smh-info-card"), Sense::click()).on_hover_text(&info.tip);
    if card.clicked() {
        acts.push(Act::Info(false));
    }
    if card.secondary_clicked() {
        acts.push(Act::Info(true));
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        if ui.add(egui::Button::new(&info.left).min_size(vec2(0.0, 26.0))).clicked() {
            acts.push(Act::Info(false));
        }
        if let Some(r) = &info.right {
            if ui.add(egui::Button::new(r).min_size(vec2(0.0, 26.0))).clicked() {
                acts.push(Act::Info(true));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if info.product && icon_button(ui, Icon::Factory, false, 26.0, &v.tips[9]).clicked() {
                acts.push(Act::Panel(11));
            }
            if info.tanks && icon_button(ui, Icon::Tanks, false, 26.0, &v.tips[8]).clicked() {
                acts.push(Act::Panel(10));
            }
            if info.redirect {
                ui.label(egui::RichText::new(&v.labels.redirect).small().strong().color(accent.gamma_multiply(if v.blink { 1.0 } else { 0.5 })));
            }
        });
    });
}

/// The console's buttons in a column: rotate, zoom and tilt held (the left button's way, the right button the other
/// way as on the console), the top and side views clicked.
fn camera_buttons(ui: &mut egui::Ui, v: &View, acts: &mut Vec<Act>) {
    let mut held = None;
    egui::Grid::new("smh-camera").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
        let rows = [[(Icon::RotateLeft, 3, true), (Icon::RotateRight, 2, true)], [(Icon::ZoomIn, 5, true), (Icon::ZoomOut, 5, false)], [(Icon::TiltUp, 6, true), (Icon::TiltDown, 6, false)]];
        for row in rows {
            for (i, spot, left) in row {
                let r = icon_button(ui, i, false, 30.0, &v.tips[spot as usize]);
                if r.is_pointer_button_down_on() && r.contains_pointer() {
                    let right = ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary) && !i.pointer.primary_down());
                    held = Some((spot, left != right));
                }
            }
            ui.end_row();
        }
        if either(&icon_button(ui, Icon::TopView, false, 30.0, &v.tips[1])) {
            acts.push(Act::View(1));
        }
        if either(&icon_button(ui, Icon::SideView, false, 30.0, &v.tips[4])) {
            acts.push(Act::View(4));
        }
        ui.end_row();
    });
    acts.push(Act::Hold(held));
}

fn minimap(ui: &mut egui::Ui, mini: &Mini, v: &View, acts: &mut Vec<Act>) {
    let size = vec2(200.0, 200.0 * MINI[3] / MINI[2]);
    let (r, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 8.0, Color32::from_black_alpha(150));
    let at = |u: [f32; 2]| pos2(r.left() + u[0] * r.width(), r.top() + u[1] * r.height());
    for &(u, c, sel) in &mini.fleets {
        let c = if sel && !v.blink { c.gamma_multiply(0.3) } else { c };
        p.rect_filled(Rect::from_center_size(at(u), vec2(3.0, 1.6)), 0.0, c);
    }
    for &(u, c, sel) in &mini.planets {
        let q = at(u);
        p.circle_filled(q, 1.9, c);
        if sel {
            p.circle_stroke(q, 4.5, Stroke::new(1.2f32, if v.blink { c } else { c.gamma_multiply(0.4) }));
        }
    }
    let outline: Vec<Pos2> = [0, 1, 3, 2].iter().filter_map(|&i| mini.view[i].map(at)).collect();
    if outline.len() == 4 {
        p.add(egui::Shape::closed_line(outline, Stroke::new(1.2f32, ui.visuals().hyperlink_color)));
    }
    p.rect_stroke(r, 8.0, ui.visuals().widgets.noninteractive.bg_stroke, StrokeKind::Inside);
    let resp = resp.on_hover_text(&v.tips[7]);
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

    /// Run frames of the HUD with the pointer events of `step`, gathering the acts.
    /// A frame's pointer (none: the found target) and button (index, down).
    type Step = (Option<Pos2>, Option<(u8, bool)>);

    fn drive(v: &View, events: &[Step], find: impl Fn(&egui::Context) -> Pos2) -> Vec<Act> {
        let mut g = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        let style = crate::gui::style(&crate::theme::Theme::modern());
        let mut st = State::default();
        let mut all = Vec::new();
        let size = (1920, 1080);
        let k = crate::gui::Gui::scale(size.1);
        let mut target = None;
        // Anchored areas are sized on their first frame and placed on the second.
        for _ in 0..2 {
            g.run(size, &style, |ctx| {
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
            g.run(size, &style, |ctx| all.extend(show(ctx, v, &mut st)));
        }
        all.retain(|a| !matches!(a, Act::Hold(None) | Act::History(false)));
        all
    }

    /// The pause button sits at the top bar's left; a click on it asks for the pause, nothing else.
    #[test]
    fn pause_click() {
        let acts = drive(&view(), &[(None, None), (None, Some((0, true))), (None, Some((0, false)))], |_| pos2(EDGE + 10.0 + 12.0, EDGE * 0.6 + 6.0 + 11.0));
        assert_eq!(acts, vec![Act::Pause]);
    }

    /// The selection card: left click = the info box's left button, right click = its right one.
    #[test]
    fn info_card_buttons() {
        let at = |ctx: &egui::Context| ctx.memory(|m| m.area_rect(egui::Id::new("smh-info"))).unwrap().left_top() + vec2(60.0, 22.0);
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
        let size = (1920, 1080);
        let k = crate::gui::Gui::scale(size.1);
        let mut rect = Rect::NOTHING;
        for _ in 0..2 {
            g.run(size, &style, |ctx| {
                show(ctx, &v, &mut st);
                rect = ctx.memory(|m| m.area_rect(egui::Id::new("smh-mini"))).unwrap();
            });
        }
        // Second row, first column of the grid: zoom in (8-point margin, 30-point buttons, 2 apart).
        let p = rect.left_top() + vec2(8.0 + 15.0, 8.0 + 32.0 + 15.0);
        let mut held = Vec::new();
        for step in 0..5 {
            g.pointer(Some([p.x * k, p.y * k]), k);
            match step {
                1 => g.button(0, true),
                3 => g.button(0, false),
                _ => {}
            }
            g.run(size, &style, |ctx| held.extend(show(ctx, &v, &mut st).into_iter().filter_map(|a| if let Act::Hold(h) = a { Some(h) } else { None })));
        }
        assert_eq!(held, vec![None, Some((5, true)), Some((5, true)), None, None]);
    }
}
