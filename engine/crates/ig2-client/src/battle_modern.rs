//! Ours (ADR 0077): the space battle's HUD (re/screens/spacewar.md §5) as the design boards' panels on the design kit
//! (ADR 0073), over the original's 3D battle: the weapon rail at the right edge (the original's turning weapon icons
//! with their reload clocks and "off" marks, cut out of its render target, a charge bar under each), the ship info
//! key and the retreat key, the selection card at the bottom left (the small info box: the selected units' or the
//! battle's overview with the fighters' stance), the big info box as a panel, the navigation plate with the radar and
//! the camera keys at the bottom right, the retreat question as a modal. The panels take no input themselves: the
//! battle screen asks [`State::spot`] which of the original's hot-spots the pointer is on (and where on it), so the
//! weapon groups, the formations, the radar's pan, the camera buttons with both mouse buttons, the stance slider, the
//! info toggle, the retreat and the orders stay the original's.

use egui::{pos2, vec2, Color32, Pos2, Rect, Stroke};

use crate::gui::ds::{self, Font, Text, Weight};

/// A key of the weapon rail (hot-spot 0x21 + i): the group's (or formation's) name, its reload, lit (armed or the
/// formation picked), all its mounts off, how far it has slid in (0..1).
#[derive(Debug, Clone, Default)]
pub struct RailKey {
    pub name: String,
    pub charge: Option<f32>,
    pub lit: bool,
    pub off: bool,
    pub shown: f32,
}

/// The selection card (`kisinfokiir`): a title, an owner, lines, the hull bar; or the battle overview.
#[derive(Debug, Clone, Default)]
pub struct Card {
    pub title: String,
    pub sub: String,
    pub owner: String,
    pub colour: Color32,
    pub lines: Vec<String>,
    /// The hull's label and the part left.
    pub hull: Option<(String, f32)>,
    /// The overview: per row the label and the own, enemy and other sides' values (the others' only when present).
    pub overview: Vec<(String, [String; 3])>,
    pub others: bool,
    /// The fighters' stance slider (0..100) and its label.
    pub stance: Option<(i32, String)>,
}

/// The big info box (`nagyinfokiir`): its slide, the data lines, the weapons.
#[derive(Debug, Clone, Default)]
pub struct Big {
    pub f: f32,
    pub title: String,
    pub owner: String,
    pub colour: Color32,
    pub count: String,
    pub rows: Vec<(String, String)>,
    pub hull: Option<f32>,
    pub weapons: Vec<(String, String)>,
}

/// The radar: the units (0..1 across, own) and the view's outline.
#[derive(Debug, Clone, Default)]
pub struct Radar {
    pub dots: Vec<([f32; 2], bool)>,
    pub view: [Option<[f32; 2]>; 4],
}

/// What the HUD shows this frame.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub rail: [Option<RailKey>; 10],
    /// The info key (the small box's hot-spot 0x42): its label, whether the big box is open.
    pub info: (String, bool),
    pub card: Card,
    pub big: Option<Big>,
    pub radar: Radar,
    /// The camera keys' tooltips: top view, yaw left, yaw right, angled view, zoom, pitch (`GetString(0xb, 2, 6..=11)`).
    pub keys: [String; 6],
    /// The retreat question: the prompt, yes, no.
    pub retreat: Option<[String; 3]>,
    /// `spacewarelfade`: the HUD fades in with the opening zoom.
    pub fade: f32,
    pub radar_tip: String,
}

/// Where the HUD put the original's controls this frame (points), the wells and the pictures' sources.
#[derive(Debug, Clone, Default)]
pub struct State {
    map: [f32; 3],
    panels: Vec<Rect>,
    /// A control's rect, its hot-spot and how a point on it maps to the original's (`None`: the spot's own middle).
    spots: Vec<(Rect, u32, Option<Map>)>,
    yes: Option<Rect>,
    /// The big info panel (cut at the rail): over the radar, the keys or the card it is no control of theirs.
    big: Option<Rect>,
    /// The rail's left edge (virtual; 0 before the first frame).
    rail: f32,
    /// The info model's well and the rail's icon wells (slot) in pixels, with their opacity.
    pub wells: Vec<(Well, Rect, f32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Map {
    /// The radar: the unit square onto the original's window (274, 388, 94 × 67).
    Radar,
    /// The stance slider: across onto 50..163.
    Slider,
}

/// A picture well's source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Well {
    Info,
    Weapon(usize),
}

/// A point that is none of the original's controls: the panels take the press (hot-spot 0x2b answers nothing).
const DEAD: u32 = 0x2b;

impl State {
    /// The hot-spot (`AktIkon`) under the pointer `at` (virtual) and the point the battle's code should see, when the
    /// pointer is on the HUD; `None` elsewhere (the battlefield, the top bar).
    pub fn spot(&self, at: [f32; 2]) -> Option<(u32, [f32; 2])> {
        let m = self.map;
        if m[2] == 0.0 {
            return None;
        }
        let p = pos2(m[0] + at[0] * m[2], m[1] + at[1] * m[2]);
        if self.big.is_some_and(|b| b.contains(p)) {
            return Some((DEAD, at));
        }
        if let Some((r, id, map)) = self.spots.iter().find(|s| s.0.contains(p)) {
            let u = [((p.x - r.left()) / r.width()).clamp(0.0, 1.0), ((p.y - r.top()) / r.height()).clamp(0.0, 1.0)];
            let to = match map {
                Some(Map::Radar) => [274.0 + u[0] * 94.0, 388.0 + u[1] * 67.0],
                Some(Map::Slider) => [50.0 + u[0] * 113.0, 465.0],
                None => at,
            };
            return Some((*id, to));
        }
        self.panels.iter().any(|r| r.contains(p)).then_some((DEAD, at))
    }

    /// The pointer `at` as control `id`'s mapping sees it, clamped onto the control (a latched drag off it).
    pub fn map_to(&self, id: u32, at: [f32; 2]) -> Option<[f32; 2]> {
        let m = self.map;
        let (r, _, map) = self.spots.iter().find(|s| s.1 == id && s.2.is_some())?;
        let p = pos2(m[0] + at[0] * m[2], m[1] + at[1] * m[2]);
        let u = [((p.x - r.left()) / r.width()).clamp(0.0, 1.0), ((p.y - r.top()) / r.height()).clamp(0.0, 1.0)];
        Some(match map {
            Some(Map::Radar) => [274.0 + u[0] * 94.0, 388.0 + u[1] * 67.0],
            _ => [50.0 + u[0] * 113.0, 465.0],
        })
    }

    /// The rail's left edge on the virtual screen (640 or beyond on a wide one; 640 before the HUD's first frame).
    pub fn rail_left(&self) -> f32 {
        if self.map[2] == 0.0 { 640.0 } else { self.rail }
    }

    /// The pointer is on the retreat question's Yes.
    pub fn yes(&self, at: [f32; 2]) -> bool {
        let m = self.map;
        self.yes.is_some_and(|r| r.contains(pos2(m[0] + at[0] * m[2], m[1] + at[1] * m[2])))
    }
}

/// The original's controls the HUD replaces: off the HUD they are the battlefield's.
pub fn replaced(id: u32) -> bool {
    matches!(id, 0x1f | 0x21..=0x2a | 0x37 | 0x42 | 0x46..=0x4b)
}

const EDGE: f32 = 24.0;
const RAIL_W: f32 = 96.0;
const CARD_W: f32 = 380.0;
const NAV_W: f32 = 300.0;
const RAIL: Font = Font { size: 11.0, line: 14.0, weight: Weight::SemiBold, tracking: 0.1, upper: true, lcd: false };
const NAME: Font = Font { size: 22.0, line: 26.0, weight: Weight::Regular, tracking: 0.08, upper: false, lcd: false };
const LINE: Font = Font { size: 15.0, line: 20.0, weight: Weight::Regular, tracking: 0.12, upper: true, lcd: false };
const BIG_TITLE: Font = Font { size: 20.0, line: 26.0, weight: Weight::SemiBold, tracking: 0.1, upper: false, lcd: false };
const OWNER: Font = Font { size: 13.0, line: 16.0, weight: Weight::SemiBold, tracking: 0.16, upper: true, lcd: false };

fn pointer(ui: &egui::Ui) -> Option<Pos2> {
    ui.ctx().input(|i| i.pointer.hover_pos())
}

fn down(ui: &egui::Ui) -> bool {
    ui.ctx().input(|i| i.pointer.primary_down() || i.pointer.secondary_down())
}

/// One frame of the HUD over the battle (after the top bar).
pub fn show(ctx: &egui::Context, v: &View, st: &mut State) {
    let ppp = ctx.pixels_per_point();
    let screen = ctx.screen_rect();
    let virt = crate::font::Virtual::new([screen.width() * ppp, screen.height() * ppp]);
    *st = State { map: [virt.x0 / ppp, virt.y0 / ppp, virt.k / ppp], ..Default::default() };
    let bar = ctx.memory(|m| m.area_rect(egui::Id::new("smh-left")));
    let top = bar.map_or(44.0, |r| r.bottom()) + 16.0;
    // The top bar takes its own presses (egui): none reaches the battlefield.
    st.panels.extend(bar);
    egui::Area::new(egui::Id::new("bmh")).order(egui::Order::Background).interactable(false).fixed_pos(screen.min).fade_in(false).show(ctx, |ui| {
        ui.set_opacity(v.fade.clamp(0.0, 1.0));
        let nav = nav(ui, v, st, screen);
        rail(ui, v, st, screen, top, nav.top() - 16.0);
        card(ui, v, st, screen);
        if let Some(b) = &v.big {
            // It slides out from under the rail: cut at the rail's left edge.
            let clip = Rect::from_min_max(screen.min, pos2(screen.right() - RAIL_W - 4.0, screen.bottom()));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(screen));
            child.set_clip_rect(clip);
            big(&mut child, b, st, screen, top, clip);
        }
    });
    if let Some(r) = &v.retreat {
        let key = st.spots.iter().find(|s| s.1 == 0x2a).map(|s| s.0);
        egui::Area::new(egui::Id::new("bmh-retreat")).order(egui::Order::Foreground).interactable(false).fixed_pos(screen.min).fade_in(false).show(ctx, |ui| retreat(ui, r, st, screen, key));
    }
    tooltip(ctx, v, st);
    for w in &mut st.wells {
        w.1 = Rect::from_min_max((w.1.min.to_vec2() * ppp).to_pos2(), (w.1.max.to_vec2() * ppp).to_pos2());
    }
}

/// The weapon rail: a key per shown group (the original's icon in a well, its name, the charge bar), the info key
/// first, the retreat key last, at the screen's right edge (the board's `nav`, 96 wide, from 60 down to the plate).
fn rail(ui: &mut egui::Ui, v: &View, st: &mut State, screen: Rect, top: f32, bottom: f32) {
    let x = screen.right() - RAIL_W;
    st.rail = (x - st.map[0]) / st.map[2];
    let keys: Vec<usize> = (0..9).filter(|&i| v.rail[i].as_ref().is_some_and(|k| k.shown > 0.0)).collect();
    let retreat = v.rail[9].as_ref().filter(|k| k.shown > 0.0);
    let n = keys.len() + 1 + retreat.is_some() as usize;
    let h = ((bottom - top - 8.0 - 8.0 * n as f32) / n as f32).clamp(44.0, 84.0);
    let mut y = top;
    let info = Rect::from_min_size(pos2(x, y), vec2(RAIL_W, h));
    rail_key(ui, info, &v.info.0, None, v.info.1, false, 1.0, None);
    st.spots.push((info, 0x42, None));
    y += h + 16.0;
    for i in keys {
        let k = v.rail[i].as_ref().unwrap();
        let r = Rect::from_min_size(pos2(x + (1.0 - k.shown) * RAIL_W, y), vec2(RAIL_W, h));
        rail_key(ui, r, &k.name, k.charge, k.lit, k.off, k.shown, Some((Well::Weapon(i), st)));
        st.spots.push((r, 0x21 + i as u32, None));
        y += h + 8.0;
    }
    if let Some(k) = retreat {
        let r = Rect::from_min_size(pos2(x + (1.0 - k.shown) * RAIL_W, bottom - h), vec2(RAIL_W, h));
        rail_key(ui, r, &k.name, None, k.lit, false, k.shown, Some((Well::Weapon(9), st)));
        st.spots.push((r, 0x2a, None));
    }
}

/// A rail key (the board's: dark glass at 0.8 with the panel's shadow, the plate's gradient and silver rim when lit,
/// rounded on the left; the icon, the name upper case, the charge bar in LCD ink).
#[allow(clippy::too_many_arguments)]
fn rail_key(ui: &egui::Ui, r: Rect, name: &str, charge: Option<f32>, lit: bool, off: bool, shown: f32, well: Option<(Well, &mut State)>) {
    let k = ds::tokens();
    let p = ui.painter();
    let radius = egui::CornerRadius { nw: ds::RADIUS_PANEL, sw: ds::RADIUS_PANEL, ne: 0, se: 0 };
    let hot = ds::hover_t(ui.ctx(), egui::Id::new(("bmh-key", name)), pointer(ui).is_some_and(|q| r.contains(q)));
    ds::shadow(p, r, radius, 4, 16, 0, Color32::from_black_alpha(160));
    if lit || (hot > 0.5 && down(ui)) {
        ds::vgrad(p, r, radius, &[(0.0, k.plate_high), (1.0, k.plate)]);
        ds::rim(p, r, radius, k.ink_silver);
    } else {
        p.rect_filled(r, radius, Color32::from_rgba_unmultiplied(0, 12, 20, 204));
        if hot > 0.0 {
            ds::rim(p, r, radius, k.rim_bright.gamma_multiply(hot));
        }
    }
    let bar = charge.map(|_| 10.0).unwrap_or(0.0);
    let two = name.contains(' ') && Text::new(name, RAIL).layout(ui.ctx(), f32::INFINITY).width > r.width() - 8.0;
    let label_h = if two { 28.0 } else { 14.0 };
    let icon_h = (r.height() - label_h - 6.0 - 12.0 - bar).max(16.0);
    let icon = Rect::from_center_size(pos2(r.center().x, r.top() + 10.0 + icon_h / 2.0), vec2(icon_h.min(56.0) * 1.2, icon_h));
    match well {
        Some((w, st)) => st.wells.push((w, icon, ui.opacity() * shown)),
        // The info key's glyph: a ship's outline.
        None => {
            let c = if lit { k.ink } else { k.ink_silver };
            let s = icon.height().min(28.0) / 24.0;
            let at = |x: f32, y: f32| icon.center() + vec2(x - 12.0, y - 12.0) * s;
            let pts = [at(2.0, 12.0), at(9.0, 7.0), at(20.0, 9.0), at(22.0, 12.0), at(20.0, 15.0), at(9.0, 17.0)];
            p.add(egui::Shape::closed_line(pts.to_vec(), Stroke::new(1.8 * s, c)));
            p.circle_stroke(at(15.0, 12.0), 2.0 * s, Stroke::new(1.5 * s, c));
        }
    }
    let label = Rect::from_min_size(pos2(r.left() + 4.0, icon.bottom() + 6.0), vec2(r.width() - 8.0, label_h));
    Text::new(name, RAIL).center().lines(if two { 2 } else { 1 }).shrink(8.5).color(if off { k.ink_muted } else { k.ink }).paint(ui, label);
    if let Some(c) = charge {
        let b = Rect::from_center_size(pos2(r.center().x, label.bottom() + 8.0), vec2(60.0, 4.0));
        p.rect_filled(b, 2, k.lcd);
        let c = ui.ctx().animate_value_with_time(egui::Id::new(("bmh-charge", name)), c.clamp(0.0, 1.0), ds::anim(ds::GLIDE_TIME));
        if c > 0.0 {
            p.rect_filled(b.with_max_x(b.left() + 60.0 * c), 2, k.lcd_ink);
        }
    }
}

/// The selection card (the small info box, hot-spot 0x42 opens the ship info): the info model in its well, the
/// title, the owner, the lines and the hull bar; with nothing selected the overview table and the stance slider.
fn card(ui: &mut egui::Ui, v: &View, st: &mut State, screen: Rect) {
    let k = ds::tokens();
    let c = &v.card;
    let rows = if c.overview.is_empty() { c.lines.len() as f32 * 22.0 + if c.hull.is_some() { 26.0 } else { 0.0 } } else { c.overview.len() as f32 * 22.0 + 36.0 };
    let h = (26.0 + 22.0 + rows + 28.0).max(124.0);
    let r = Rect::from_min_size(pos2(screen.left() + EDGE, screen.bottom() - EDGE - h), vec2(CARD_W, h));
    let p = ui.painter().clone();
    let hot = ds::hover_t(ui.ctx(), egui::Id::new("bmh-card"), pointer(ui).is_some_and(|q| r.contains(q)));
    ds::paint_panel(&p, r, ds::PanelKind::Deep, 0.88);
    if hot > 0.0 {
        ds::rim(&p, r, ds::RADIUS_PANEL, k.rim_bright.gamma_multiply(hot));
    }
    st.panels.push(r);
    let inner = r.shrink2(vec2(16.0, 14.0));
    let side = Rect::from_min_size(inner.min, vec2(96.0, 96.0));
    p.rect_filled(side, ds::RADIUS_PLATE, k.void);
    ds::rim(&p, side, ds::RADIUS_PLATE, k.rim);
    st.wells.push((Well::Info, side.shrink(3.0), ui.opacity()));
    let x = side.right() + 16.0;
    let w = inner.right() - x;
    let mut y = inner.top();
    Text::new(&c.title, NAME).color(k.ink).shrink(15.0).paint(ui, Rect::from_min_size(pos2(x, y), vec2(w, 26.0)));
    y += 28.0;
    if !c.sub.is_empty() {
        Text::new(&c.sub, LINE).color(k.info).paint(ui, Rect::from_min_size(pos2(x, y), vec2(w, 20.0)));
        y += 22.0;
    }
    if !c.owner.is_empty() {
        Text::new(&c.owner, LINE).color(c.colour).paint(ui, Rect::from_min_size(pos2(x, y), vec2(w, 20.0)));
        y += 22.0;
    }
    for l in &c.lines {
        Text::new(l, LINE).color(k.info).paint(ui, Rect::from_min_size(pos2(x, y), vec2(w, 20.0)));
        y += 22.0;
    }
    if let Some((label, f)) = &c.hull {
        let row = Rect::from_min_size(pos2(x, y + 4.0), vec2(w, 20.0));
        let lw = Text::new(label, LINE).color(k.info).paint(ui, row.with_max_x(row.left() + w * 0.45)).width;
        let bar = Rect::from_min_size(pos2(row.left() + lw + 10.0, row.center().y - 3.0), vec2((w - lw - 10.0).max(20.0), 6.0));
        hull_bar(&p, bar, *f);
    }
    if !c.overview.is_empty() {
        let cols = [k.own, k.enemy, k.ink_silver];
        let vw = 52.0;
        for (label, vals) in &c.overview {
            let row = Rect::from_min_size(pos2(x, y), vec2(w, 20.0));
            let base = ds::fit::baseline(row, Font::DATA);
            let n = if c.others { 3 } else { 2 };
            Text::new(label.trim().trim_end_matches(':'), Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, row.with_max_x(row.right() - n as f32 * vw - 8.0));
            for (j, val) in vals.iter().take(n).enumerate() {
                let cell = Rect::from_min_size(pos2(row.right() - (n - j) as f32 * vw, row.top()), vec2(vw - 4.0, 20.0));
                Text::new(val, Font::DATA).right().color(cols[j]).baseline(base).paint(ui, cell);
            }
            y += 22.0;
        }
        if let Some((s, label)) = &c.stance {
            // The fighters' stance: defend (green) to attack (red), a marker on it; the original's hot-spot 0x37.
            let lr = Rect::from_min_size(pos2(x, y + 2.0), vec2(w, 16.0));
            Text::new(label, Font::LABEL.upper(false)).color(k.ink_muted).paint(ui, lr);
            let bar = Rect::from_min_size(pos2(x, lr.bottom() + 6.0), vec2(w, 6.0));
            ds::hgrad(&p, bar, 3, &[(0.0, k.own), (1.0, k.enemy)]);
            ds::rim(&p, bar, 3, k.rim);
            let mx = bar.left() + bar.width() * *s as f32 / 100.0;
            let m = Rect::from_center_size(pos2(mx, bar.center().y), vec2(6.0, 14.0));
            p.rect_filled(m, 2, k.ink);
            ds::rim(&p, m, 2, k.metal_shadow);
            st.spots.push((bar.expand2(vec2(4.0, 6.0)), 0x37, Some(Map::Slider)));
        }
    }
    // The rest of the card is the small box's hot-spot (the info toggle).
    st.spots.push((r, 0x42, None));
}

fn hull_bar(p: &egui::Painter, bar: Rect, f: f32) {
    let k = ds::tokens();
    p.rect_filled(bar, 3, k.glass_deep);
    let f = f.clamp(0.0, 1.0);
    if f > 0.0 {
        let c = if f > 0.5 { k.own } else if f > 0.25 { k.lcd_ink } else { k.enemy };
        p.rect_filled(bar.with_max_x(bar.left() + (bar.width() * f).max(6.0)), 3, c);
    }
    ds::rim(p, bar, 3, k.rim);
}

/// The navigation plate: the radar sunk into cast metal (press or drag pans, hot-spot 0x1f), the camera keys under
/// it (the original's cluster: top view, yaw left, yaw right, angled view, zoom, pitch; both mouse buttons as there).
fn nav(ui: &mut egui::Ui, v: &View, st: &mut State, screen: Rect) -> Rect {
    use crate::starmap_modern::Icon;
    let k = ds::tokens();
    let radar_h = (280.0 * 67.0 / 94.0f32).round();
    let key = vec2(34.0, 28.0);
    let h = 10.0 + radar_h + 8.0 + key.y + 4.0 + 10.0;
    let r = Rect::from_min_size(pos2(screen.right() - EDGE - NAV_W, screen.bottom() - EDGE - h), vec2(NAV_W, h));
    let p = ui.painter().clone();
    crate::starmap_modern::metal_plate(&p, r, 18);
    st.panels.push(r);
    let well = Rect::from_min_size(r.min + vec2(10.0, 10.0), vec2(280.0, radar_h));
    ds::shadow(&p, well, 10, 0, 0, 1, k.metal_shadow);
    p.rect_filled(well, 10, Color32::BLACK);
    ds::vgrad(&p, well, 10, &[(0.0, Color32::from_black_alpha(255)), (6.0 / well.height(), Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    let clip = p.with_clip_rect(well.shrink(2.0));
    for j in 1..6 {
        let (x, y) = (well.left() + well.width() * j as f32 / 6.0, well.top() + well.height() * j as f32 / 6.0);
        clip.vline(x, well.y_range(), Stroke::new(1.0f32, k.rim_bright.gamma_multiply(0.18)));
        clip.hline(well.x_range(), y, Stroke::new(1.0f32, k.rim_bright.gamma_multiply(0.18)));
    }
    let at = |u: [f32; 2]| pos2(well.left() + u[0] * well.width(), well.top() + u[1] * well.height());
    // The view's outline: the edges whose both corners meet the grid plane (`RadarFrissit`'s four lines).
    for (a, c) in [(0, 1), (0, 2), (2, 3), (1, 3)] {
        if let (Some(a), Some(c)) = (v.radar.view[a], v.radar.view[c]) {
            clip.line_segment([at(a), at(c)], Stroke::new(1.2f32, k.focus));
        }
    }
    for &(u, own) in &v.radar.dots {
        let c = if own { k.own } else { k.enemy };
        clip.circle_filled(at(u), 3.0, c.gamma_multiply(0.35));
        clip.circle_filled(at(u), 1.6, c);
    }
    ds::rim(&p, well, 10, Color32::from_white_alpha(10));
    st.spots.push((well, 0x1f, Some(Map::Radar)));
    let icons = [(Icon::TopView, 0x46), (Icon::RotateLeft, 0x47), (Icon::RotateRight, 0x48), (Icon::SideView, 0x49), (Icon::ZoomIn, 0x4a), (Icon::TiltUp, 0x4b)];
    let total = 6.0 * key.x + 5.0 * 2.0 + 2.0 * 4.0;
    let mut x = r.center().x - total / 2.0;
    let y = well.bottom() + 8.0;
    let group = Rect::from_min_size(pos2(x, y), vec2(total, key.y + 4.0));
    p.rect_filled(group, ds::RADIUS_PLATE, k.metal_shadow);
    x += 4.0;
    for (n, (icon, id)) in icons.into_iter().enumerate() {
        let kr = Rect::from_min_size(pos2(x, y + 2.0), key);
        let hot = pointer(ui).is_some_and(|q| kr.contains(q));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(kr));
        let draw = move |p: &egui::Painter, r: Rect, c: Color32| crate::starmap_modern::icon(p, r, icon, c);
        ds::forced(hot, hot && down(ui), || child.add(ds::IconKey::new(ds::Glyph::Custom(&draw)).size(key)));
        st.spots.push((kr, id, None));
        let _ = n;
        x += key.x + 2.0;
    }
    r
}

/// The big info box as the board's panel left of the rail: the title and the owner, the selected unit's model under
/// it (the original's info model in its well) with the count, the data table with the hull bar, the weapons.
fn big(ui: &mut egui::Ui, b: &Big, st: &mut State, screen: Rect, top: f32, clip: Rect) {
    let k = ds::tokens();
    let w = 440.0;
    let rows = b.rows.len() + b.weapons.len();
    let h = 52.0 + 200.0 + 18.0 + rows as f32 * 30.0 + 18.0 + if b.weapons.is_empty() { 0.0 } else { 12.0 };
    let f = b.f.clamp(0.0, 1.0);
    let x = screen.right() - RAIL_W - 8.0 - w + (1.0 - f) * (w + RAIL_W);
    let r = Rect::from_min_size(pos2(x, top), vec2(w, h));
    let p = ui.painter().clone();
    ds::paint_panel(&p, r, ds::PanelKind::Glass, 0.92);
    let head = Rect::from_min_size(r.min, vec2(w, 52.0));
    ds::header_strip(&p, head);
    st.panels.push(r.intersect(clip));
    st.big = Some(r.intersect(clip));
    let inner = head.shrink2(vec2(18.0, 0.0));
    let base = ds::fit::baseline(inner, BIG_TITLE);
    let tw = Text::new(&b.title, BIG_TITLE).color(k.ink).baseline(base).paint(ui, inner.with_max_x(inner.left() + inner.width() * 0.55)).width;
    if !b.owner.is_empty() {
        let g = Text::new(&b.owner, OWNER).color(b.colour).layout(ui.ctx(), inner.right() - inner.left() - tw - 16.0);
        ds::glowing_galley(&p, pos2(inner.left() + tw + 16.0, base - g.baseline()), &g.galley, b.colour);
    }
    // The selection's model on the void (the original's info model), the count in an LCD window.
    let pic = Rect::from_min_size(head.left_bottom(), vec2(w, 200.0));
    p.rect_filled(pic, 0, k.void);
    p.hline(pic.x_range(), pic.bottom() - 0.5, Stroke::new(1.0f32, k.rim));
    let well = Rect::from_center_size(pic.center(), vec2(190.0, 190.0));
    // A picture can't be cut: the model shows once its well is clear of the rail.
    if clip.contains_rect(well) {
        st.wells.push((Well::Info, well, ui.opacity()));
    }
    let lw = 14.0 + 12.0 * b.count.len() as f32;
    let lcd = Rect::from_min_size(pic.right_bottom() - vec2(14.0 + lw, 14.0 + 26.0), vec2(lw, 26.0));
    ds::lcd_window(&p, lcd);
    Text::new(&b.count, Font::READOUT.sized(20.0)).center().color(k.lcd_ink).paint_only(ui, lcd);
    let mut y = pic.bottom() + 16.0;
    for (n, (label, value)) in b.rows.iter().enumerate() {
        let row = Rect::from_min_size(pos2(r.left() + 18.0, y), vec2(w - 36.0, 20.0));
        let base = ds::fit::baseline(row, Font::DATA);
        let (a, c) = row.split_left_right_at_x(row.left() + 120.0);
        Text::new(label.trim().trim_end_matches(':'), Font::LABEL).color(k.ink_muted).baseline(base).paint(ui, a.with_max_x(a.right() - 8.0));
        // The hull's value leaves the row's right half to its bar.
        let vc = if b.hull.is_some() && n == 0 { c.with_max_x(c.left() + c.width() * 0.5) } else { c };
        let vw = Text::new(value, Font::DATA).color(k.info).baseline(base).paint(ui, vc).width;
        if let (Some(hf), 0) = (b.hull, n) {
            hull_bar(&p, Rect::from_min_size(pos2(c.left() + vw + 14.0, row.center().y - 3.0), vec2((c.width() - vw - 14.0).max(30.0), 6.0)), hf);
        }
        y += 30.0;
    }
    if !b.weapons.is_empty() {
        y += 6.0;
        p.hline(r.x_range().shrink(18.0), y - 4.0, Stroke::new(1.0f32, k.rim));
        for (name, count) in &b.weapons {
            let row = Rect::from_min_size(pos2(r.left() + 18.0, y + 4.0), vec2(w - 36.0, 20.0));
            let base = ds::fit::baseline(row, Font::DATA);
            let (a, c) = row.split_left_right_at_x(row.right() - 48.0);
            Text::new(name, Font::DATA).color(k.ink).baseline(base).paint(ui, a);
            Text::new(count, Font::DATA).right().color(k.info).baseline(base).paint(ui, c);
            y += 30.0;
        }
    }
}

/// The retreat question as a kit popup beside the retreat key (the original's: it opens on the key's press and the
/// release answers it, Yes or anywhere else): the prompt, Yes (the original's first row) and No.
fn retreat(ui: &mut egui::Ui, t: &[String; 3], st: &mut State, screen: Rect, key: Option<Rect>) {
    let k = ds::tokens();
    let w = 420.0;
    let r = match key {
        Some(key) => Rect::from_min_size(pos2(key.left() - 12.0 - w, (key.bottom() - 150.0).max(screen.top())), vec2(w, 150.0)),
        None => Rect::from_center_size(screen.center(), vec2(w, 150.0)),
    };
    let p = ui.painter().clone();
    ds::shadow(&p, r, ds::RADIUS_PANEL, 8, 24, 0, Color32::from_black_alpha(204));
    ds::paint_panel(&p, r, ds::PanelKind::Raised, 0.97);
    Text::new(&t[0], Font::BODY).center().lines(2).color(k.ink).paint(ui, Rect::from_min_size(r.min + vec2(24.0, 20.0), vec2(w - 48.0, 52.0)));
    let bw = (w - 48.0 - 16.0) / 2.0;
    for (n, label) in [&t[1], &t[2]].into_iter().enumerate() {
        let br = Rect::from_min_size(pos2(r.left() + 24.0 + n as f32 * (bw + 16.0), r.bottom() - 24.0 - 44.0), vec2(bw, 44.0));
        let hot = pointer(ui).is_some_and(|q| br.contains(q));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(br));
        ds::forced(hot, false, || child.add_sized(br.size(), ds::Plate::new(label).width(bw).primary(n == 0)));
        if n == 0 {
            st.yes = Some(br);
        }
    }
    st.panels.push(r);
}

/// The tooltip of the HUD control under the pointer (the original's texts), after the kit's delay.
fn tooltip(ctx: &egui::Context, v: &View, st: &State) {
    let Some(p) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let Some((_, id, _)) = st.spots.iter().find(|s| s.0.contains(p)) else { return };
    let text = match *id {
        0x1f => v.radar_tip.as_str(),
        0x21..=0x2a => v.rail[(*id - 0x21) as usize].as_ref().map_or("", |k| k.name.as_str()),
        0x46..=0x4b => v.keys[(*id - 0x46) as usize].as_str(),
        _ => "",
    };
    let still = ctx.input(|i| i.pointer.time_since_last_movement()) > 0.5;
    if text.is_empty() || !still {
        return;
    }
    ds::tip_at(ctx, "bmh-tip", p, text);
}

/// A line of the original's with its padded fields ("TE:    0") as one space each.
fn spaced(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The original's render targets in the wells [`show`] left: the info model (its corner of the screen, 0..60 ×
/// 420..480) and each weapon key's icon (the column's target about the slot's place, 48 across).
pub fn pictures(f: &mut crate::render::Frame, wells: &[(Well, Rect, f32)], info: Option<crate::render::TexId>, column: Option<crate::render::TexId>, places: &[[f32; 2]; 10], v: &crate::font::Virtual) {
    for &(w, r, alpha) in wells {
        // Each target covers its virtual rectangle (`fixed_texture`): the info model's 0..60 × 420..480, the
        // column's 560..640 × 20..480.
        let (tex, area, src) = match w {
            Well::Info => (info, [0.0, 420.0, 60.0, 60.0], [7.5, 427.5, 45.0, 45.0]),
            Well::Weapon(i) => (column, [560.0, 20.0, 80.0, 460.0], [places[i][0] - 16.0, places[i][1] - 16.0, 32.0, 32.0]),
        };
        let Some(tex) = tex else { continue };
        let s = (r.width() / (src[2] * v.k)).min(r.height() / (src[3] * v.k));
        let half = [src[2] * v.k * s / 2.0, src[3] * v.k * s / 2.0];
        let mut i = crate::render::Instance::new(crate::render::kind::SPRITE, [r.center().x, r.center().y], half, [alpha; 4]);
        i.uv = [(src[0] - area[0]) / area[2], (src[1] - area[1]) / area[3], src[2] / area[2], src[3] / area[3]];
        f.push(tex, crate::render::Blend::Alpha, i);
    }
}

/// The rail's icons' places on the column's target (virtual: the hot-spots' middles, `Slot::place` + (320, 250)).
pub fn places(ui: &crate::spacewar_ui::SpaceWarUi) -> [[f32; 2]; 10] {
    std::array::from_fn(|i| {
        let p = crate::spacewar_screen::Slot { counter: ui.column[i], state: 0, reload: 0 }.place(i, ui.xcenter);
        [p[0] + 320.0, p[1] + 250.0]
    })
}

impl crate::ui::Hud {
    /// What the modern battle HUD shows, from what the original's HUD draws (`fegyverpanelfrissit`, `kisinfokiir`,
    /// `nagyinfokiir`, `RadarFrissit`, `WriteRetreatWindow`, re/screens/spacewar.md §5).
    pub fn battle_modern_view(&self, ui: &crate::spacewar_ui::SpaceWarUi, b: &ig2_sim::spacebattle::Battle) -> View {
        use crate::spacewar_hud::{format, rank};
        let lang = self.texts.lang;
        let dec = |t: &[u8]| ig2_formats::text::decode(t.split(|&c| c == 0).next().unwrap_or(&[]), lang).trim().to_string();
        let s = |k: u16| self.texts.get(0xb, 1, k);
        let col = |i: usize| crate::gui::ink(self.palette.get(i).copied().unwrap_or([1.0; 3]));
        let empire = |o: i32| (self.empire_names.get(o.max(0) as usize).map(|n| dec(n)).unwrap_or_default(), col(if o <= 0 { 12 } else { o as usize + 15 }));
        // A label with the original's trailing value field ("Hull: %d") cut off.
        let label = |t: Vec<u8>| dec(&t).trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == ':' || c == '%').to_string();
        let formation = ui.formation(b);
        let rail = std::array::from_fn(|i| {
            (ui.column[i] > 0).then(|| RailKey {
                name: dec(&self.texts.get(0xb, 3, if formation { i as u16 + 11 } else { i as u16 + 1 })),
                // The reload clock runs 0..0xaa (`ora.xvb`'s frames); empty is ready.
                charge: (i < 9 && !formation).then(|| 1.0 - ui.group_reload[i].clamp(0, 0xaa) as f32 / 170.0),
                lit: if formation { ui.formation_pick == i as i32 } else { ui.armed == i as i32 },
                off: !formation && ui.group_state[i] == 6,
                shown: ui.column[i] as f32 / 50.0,
            })
        });
        let st = ui.stats;
        let hull = || (st.max_hp - st.damage) as f32 / st.max_hp as f32;
        let mut card = Card::default();
        if st.total != 0 && st.sel_ships != 0 {
            card.title = if st.total == 1 { dec(&ui.title) } else { dec(&format(&s(5), &[(st.sel_ships + st.sel_guns) as i64], &[])) };
            if st.total == 1 && ui.title_tech >= 0 && st.last >= 0 {
                card.sub = dec(&self.tech_name(ui.title_tech as usize));
            }
            let owner = match st.last_kind {
                2 => b.planet_owner,
                1 => b.units.get(st.last.max(0) as usize).map_or(1, |u| u.owner),
                _ => 1,
            };
            (card.owner, card.colour) = empire(owner);
            if st.ships != 0 && st.max_hp != 0 {
                let r = if st.own == 0 { Vec::new() } else if (st.rank_sum as f32 / st.ships as f32) < 0.0 { b"-".to_vec() } else { rank(self, st.rank_sum / st.ships) };
                card.lines.push(spaced(&dec(&format(&s(0xb), &[st.value as i64], &[&r]))));
                card.hull = Some((label(format(&s(0xc), &[0], &[])), hull()));
            }
        } else if st.total != 0 && st.sel_guns != 0 && st.last_kind == 2 && st.last != -1 {
            card.title = if st.total == 1 && ui.title_tech >= 0 { dec(&self.tech_name(ui.title_tech as usize)) } else { dec(&format(&s(0xd), &[st.total as i64], &[])) };
            (card.owner, card.colour) = empire(b.planet_owner);
            if st.ships != 0 && st.max_hp != 0 {
                card.lines.push(spaced(&dec(&format(&s(0xe), &[st.value as i64], &[]))));
                card.hull = Some((label(format(&s(0xf), &[0], &[])), hull()));
            }
        } else if st.total == 0 {
            // The battle overview: firepower (K, M above 9999 and 999000), ships, fighters of the viewer's side, its
            // enemies and the others; the fighters' stance.
            let [ships, fighters, fire] = st.sides;
            card.others = fire[2] != 0 || ships[2] != 0 || fighters[2] != 0;
            let top = fire.iter().copied().max().unwrap_or(0);
            let (div, unit) = if top > 0xf3e58 { (1_000_000, "M") } else if top > 9999 { (1000, "K") } else { (1, "") };
            card.title = dec(&s(0x10));
            card.overview = vec![
                (label(format(&s(0x11), &[0], &[])), fire.map(|x| format!("{}{unit}", x / div))),
                (dec(&s(0x12)), ships.map(|x| x.to_string())),
                (dec(&s(0x13)), fighters.map(|x| x.to_string())),
            ];
            card.stance = Some((ui.stance.clamp(0, 100), dec(&s(0x14))));
        }
        let big = (ui.big_info != 0 && st.total != 0).then(|| {
            let mut big = Big { f: ui.big.f, title: card.title.clone(), owner: card.owner.clone(), colour: card.colour, count: st.total.to_string(), ..Default::default() };
            let mut rows = Vec::new();
            if st.max_hp != 0 {
                rows.push((dec(&s(0x19)), format!("{}%, {}hp", st.damage * 100 / st.max_hp, (st.max_hp - st.damage) / 15)));
                big.hull = Some(hull());
            }
            if let Some(ship) = &ui.big.ship {
                rows.push((dec(&s(0x15)), dec(&ship.plan)));
                rows.push((dec(&s(0x16)), dec(&self.tech_name(ship.tech.max(0) as usize))));
                (big.owner, big.colour) = empire(ship.owner);
                rows.push((dec(&s(0x17)), big.owner.clone()));
                rows.push((dec(&s(0x18)), ship.fleet.as_ref().map(|n| dec(n)).unwrap_or_default()));
            }
            if st.ships != 0 {
                rows.push((dec(&s(0x1a)), if st.shield == 0 { "-".into() } else { format!("{}%", 100 - st.shield_used * 100 / st.shield) }));
            }
            rows.push((dec(&s(0x1b)), st.value.to_string()));
            // The crew's label always, its rank for the viewer's own ships (`nagyinfokiir`).
            rows.push((dec(&s(0x1c)), if st.own != 0 && st.ships != 0 { dec(&rank(self, st.rank_sum / st.ships)) } else { String::new() }));
            big.rows = rows;
            big.weapons = ui.big.weapons.iter().take(10).map(|(t, n)| (dec(&self.tech_name(*t as usize)), n.to_string())).collect();
            big
        });
        // The radar (`RadarFrissit`, fully open): the units turned by the snapped yaw, the view's outline on the grid
        // plane, 0..1 across the original's window.
        let (sn, cs) = ((ui.snap_yaw as f64).sin() as f32, (ui.snap_yaw as f64).cos() as f32);
        let dots = b
            .units
            .iter()
            .filter(|u| u.alive && (u.owner == b.human || !u.cloaked))
            .map(|u| ([0.5 + (-cs * u.pos.y + sn * u.pos.z) / 384.0, 0.5 + (cs * u.pos.z + sn * u.pos.y) / 384.0], u.owner == b.human))
            .filter(|(q, _)| (0.0..=1.0).contains(&q[0]) && (0.0..=1.0).contains(&q[1]))
            .collect();
        let (eye, target) = ui.eye_target();
        let m = crate::spacewar_screen::look_matrix(eye, target);
        let corner = |sx: f32, sy: f32| -> Option<[f32; 2]> {
            let e = [(sx - 320.0) / 600.0, (sy - 250.0) / 600.0, -1.0];
            let dir: [f32; 3] = std::array::from_fn(|j| m[0][j] * e[0] + m[1][j] * e[1] + m[2][j] * e[2]);
            let t = Some((18.0 - eye[0]) / dir[0]).filter(|t| dir[0].abs() > 1e-9 && *t > 0.0)?;
            let (a, c) = (-(eye[1] + t * dir[1]), eye[2] + t * dir[2]);
            // The outline maps 94 pixels across both ways in the original's 94 × 67 window.
            Some([0.5 + (cs * a + sn * c) / 384.0, 0.5 + (cs * c - sn * a) * 94.0 / 67.0 / 384.0])
        };
        let t2 = |k: u16| dec(&self.texts.get(0xb, 2, k));
        View {
            rail,
            info: (dec(&self.texts.ours("Ship info")), ui.big_info != 0),
            card,
            big,
            radar: Radar { dots, view: [corner(0.0, 0.0), corner(640.0, 0.0), corner(0.0, 480.0), corner(640.0, 480.0)] },
            keys: std::array::from_fn(|i| t2(6 + i as u16)),
            retreat: ui.retreat_window.then(|| [dec(&s(1)), dec(&self.texts.get(3, 4, 2)), dec(&self.texts.get(3, 4, 3))]),
            fade: ui.fade,
            radar_tip: t2(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The HUD's controls are the original's hot-spots: the radar maps onto its window (a latched drag clamped to
    /// it), a panel's empty part is no control, the open battlefield is the original's.
    #[test]
    fn controls_reach_the_original_spots() {
        let ctx = crate::gui::Gui::new(wgpu::TextureFormat::Rgba8UnormSrgb).ctx;
        let mut st = State::default();
        let v = View { fade: 1.0, rail: std::array::from_fn(|i| (i == 0).then(|| RailKey { name: "Lasers".into(), shown: 1.0, ..Default::default() })), ..Default::default() };
        for _ in 0..2 {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0))), ..Default::default() };
            let _ = ctx.run(input, |ctx| show(ctx, &v, &mut st));
        }
        let virt = |p: Pos2| [(p.x - st.map[0]) / st.map[2], (p.y - st.map[1]) / st.map[2]];
        let radar = st.spots.iter().find(|s| s.1 == 0x1f).unwrap().0;
        assert_eq!(st.spot(virt(radar.center())), Some((0x1f, [321.0, 421.5])));
        assert_eq!(st.map_to(0x1f, virt(radar.left_top() - vec2(50.0, 50.0))), Some([274.0, 388.0]));
        let laser = st.spots.iter().find(|s| s.1 == 0x21).unwrap().0;
        assert_eq!(st.spot(virt(laser.center())).map(|s| s.0), Some(0x21));
        let card = st.panels[1];
        assert_eq!(st.spot(virt(card.center())).map(|s| s.0), Some(0x42));
        assert_eq!(st.spot([320.0, 240.0]), None);
        assert!(replaced(0x1f) && replaced(0x2a) && !replaced(0x4c));
    }
}
