//! Ours (ADR 0073): the design kit of the remake's egui pages, the openIG2 Design System in Rust. Everything a page
//! draws comes from here: the colour tokens (mapped from the UI theme, ADR 0056, so a mod's theme recolours the kit),
//! the type scale with its letter spacing ([`Font`]), text that fits a fixed box ([`fit`]), the widgets with all
//! their states ([`widgets`]), the painter helpers for a page's own components (gradients, rims, glows), the
//! modern pointer set ([`cursor`]) and the gallery that shows them all ([`gallery`]).
//!
//! One design pixel is one point: at 100 % UI scale a 1080-line window draws a board 1:1 (`gui::POINTS_HIGH`).

pub mod cursor;
pub mod fit;
pub mod gallery;
pub mod widgets;

pub use fit::{baseline, text, Text};
pub use widgets::*;

use crate::theme::Theme;
use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};
use std::cell::Cell;
use std::sync::{Arc, OnceLock};

/// The design's colours by role (`tokens.css`). The theme maps onto the main ones ([`Tokens::from_theme`]); the metal,
/// rust, LCD and empire colours are the design's own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    pub void: Color32,
    pub glass_deep: Color32,
    pub glass: Color32,
    pub glass_raised: Color32,
    pub glass_frame: Color32,
    pub rim: Color32,
    pub rim_bright: Color32,
    pub plate: Color32,
    pub plate_low: Color32,
    pub plate_high: Color32,
    pub metal_shadow: Color32,
    pub metal_dark: Color32,
    pub metal: Color32,
    pub metal_light: Color32,
    pub metal_shine: Color32,
    pub rust_dark: Color32,
    pub rust: Color32,
    pub rust_light: Color32,
    pub lcd: Color32,
    pub lcd_ink: Color32,
    pub ink: Color32,
    pub ink_silver: Color32,
    pub ink_muted: Color32,
    /// Text of what can't be used (the theme's `text_off`; the boards' `.off` uses `ink_muted`).
    pub ink_off: Color32,
    pub info: Color32,
    pub own: Color32,
    pub enemy: Color32,
    pub signal_red: Color32,
    pub focus: Color32,
}

const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

impl Tokens {
    /// The deep-space palette (`tokens.css`), the built-in Modern theme's.
    pub const DEEP_SPACE: Tokens = Tokens {
        void: hex(0x000000),
        glass_deep: hex(0x000c14),
        glass: hex(0x091d30),
        glass_raised: hex(0x121e28),
        glass_frame: hex(0x1d2221),
        rim: hex(0x0f2f44),
        rim_bright: hex(0x2b5d82),
        plate: hex(0x1d2d4d),
        plate_low: hex(0x10243c),
        plate_high: hex(0x516294),
        metal_shadow: hex(0x0e121b),
        metal_dark: hex(0x27282d),
        metal: hex(0x4e4659),
        metal_light: hex(0x7a776d),
        metal_shine: hex(0xaa9a92),
        rust_dark: hex(0x412922),
        rust: hex(0x5f483f),
        rust_light: hex(0x836b60),
        lcd: hex(0x21210d),
        lcd_ink: hex(0xd4d41c),
        ink: hex(0xfeffff),
        ink_silver: hex(0xc9ced2),
        ink_muted: hex(0x7e8d98),
        ink_off: hex(0x4d5461),
        info: hex(0x49a7ff),
        own: hex(0x57f836),
        enemy: hex(0xff5983),
        signal_red: hex(0xd8262e),
        focus: hex(0x9fd3ff),
    };

    /// The theme's colours on their roles: panel → glass, panel edge → bright rim, button edge → rim, button / tab /
    /// selected tab → the plate's three shades, text / dim / off → the inks, accent → info, hover → focus. The shades
    /// the theme has no key for (deep and raised glass, silver ink) keep their ratio to the colour they derive from.
    pub fn from_theme(t: &Theme) -> Self {
        let d = Self::DEEP_SPACE;
        let boxed = |c: [f32; 3]| Color32::from_rgb(c[0] as u8, c[1] as u8, c[2] as u8);
        let ink = crate::gui::ink;
        // `to` as `from` relates to `base`, per channel.
        let like = |base: Color32, from: Color32, to: Color32| {
            let ch = |b: u8, f: u8, t: u8| if f == 0 { t } else { (b as f32 * t as f32 / f as f32).round().min(255.0) as u8 };
            Color32::from_rgb(ch(base.r(), from.r(), to.r()), ch(base.g(), from.g(), to.g()), ch(base.b(), from.b(), to.b()))
        };
        let glass = boxed(t.panel);
        let text = ink(t.text);
        Self {
            glass,
            glass_deep: like(glass, d.glass, d.glass_deep),
            glass_raised: like(glass, d.glass, d.glass_raised),
            rim: boxed(t.button_edge),
            rim_bright: boxed(t.panel_edge),
            plate: boxed(t.button),
            plate_low: boxed(t.tab),
            plate_high: boxed(t.tab_on),
            ink: text,
            ink_silver: like(text, d.ink, d.ink_silver),
            ink_muted: ink(t.dim),
            ink_off: ink(t.off),
            info: ink(t.accent),
            focus: ink(t.hot),
            ..d
        }
    }
}

thread_local! {
    /// The tokens of the style set last on this thread ([`crate::gui::style`]): the page's theme.
    static TOKENS: Cell<Tokens> = const { Cell::new(Tokens::DEEP_SPACE) };
}

/// The current page's tokens.
pub fn tokens() -> Tokens {
    TOKENS.with(Cell::get)
}

pub(crate) fn set_tokens(k: Tokens) {
    TOKENS.with(|c| c.set(k));
}

/// Spacing steps (`--space-1` … `--space-5`), corner radii.
pub const SPACE: [f32; 5] = [4.0, 8.0, 16.0, 24.0, 40.0];
/// The boards' gap to the screen's edges: a HUD's panels over a scene (24), a full-screen board's (40).
pub const EDGE: f32 = SPACE[3];
pub const BOARD_EDGE: f32 = SPACE[4];
/// A board's exit plate's width.
pub const EXIT_W: f32 = 220.0;
pub const RADIUS_PANEL: u8 = 8;
pub const RADIUS_PLATE: u8 = 4;
/// State changes: hover and press fades, a selection's glide, a panel's open and close.
pub const HOVER_TIME: f32 = 0.12;
pub const GLIDE_TIME: f32 = 0.16;
pub const OPEN_TIME: f32 = 0.18;

/// How much a window shorter than the boards' 1080 lines squeezes their heights (1 from there up).
pub fn squeeze(height: f32) -> f32 {
    (height / crate::gui::POINTS_HIGH).min(1.0)
}

/// Oxanium's weights (the design's 400, 500, 600; 700 draws as 600).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Medium,
    SemiBold,
}

const FAMILY: [&str; 3] = ["ig2-ui-400", "ig2-ui-500", "ig2-ui-600"];
const LCD: &str = "ig2-lcd";
const OXANIUM: [&[u8]; 3] = [include_bytes!("../../fonts/Oxanium-Regular.ttf"), include_bytes!("../../fonts/Oxanium-Medium.ttf"), include_bytes!("../../fonts/Oxanium-SemiBold.ttf")];
const SHARE_TECH_MONO: &[u8] = include_bytes!("../../fonts/ShareTechMono-Regular.ttf");

/// A text style of the type scale: size and line box in points, weight, letter spacing (em), upper case, the LCD face
/// (Share Tech Mono, drawn slanted as the design's italic).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Font {
    pub size: f32,
    pub line: f32,
    pub weight: Weight,
    pub tracking: f32,
    pub upper: bool,
    pub lcd: bool,
}

impl Font {
    const fn ui(size: f32, line: f32, weight: Weight, tracking: f32, upper: bool) -> Self {
        Self { size, line, weight, tracking, upper, lcd: false }
    }
    pub const TITLE_PLATE: Font = Font::ui(28.0, 32.0, Weight::Medium, 0.12, true);
    pub const HEADING: Font = Font::ui(20.0, 26.0, Weight::SemiBold, 0.08, false);
    pub const BODY: Font = Font::ui(17.0, 26.0, Weight::Regular, 0.06, false);
    pub const LABEL: Font = Font::ui(13.0, 18.0, Weight::SemiBold, 0.14, true);
    pub const DATA: Font = Font::ui(15.0, 20.0, Weight::Medium, 0.10, false);
    pub const READOUT: Font = Font { size: 24.0, line: 28.0, weight: Weight::Regular, tracking: 0.12, upper: false, lcd: true };
    /// A page's big title ("BEÁLLÍTÁSOK": 600 30/36, 0.3 em).
    pub const PAGE_TITLE: Font = Font::ui(30.0, 36.0, Weight::SemiBold, 0.30, true);
    /// A board panel's title (600 20 / 0.16 em, upper case).
    pub const PANEL_TITLE: Font = Font::HEADING.tracking(0.16).upper(true);
    /// A card's name (22) and lines (`.ig-card .line`: 15, 0.12 em, upper case), an empire's name (`.ig-star`: 600 13
    /// / 0.16 em, upper case).
    pub const CARD_NAME: Font = Font::ui(22.0, 26.0, Weight::Regular, 0.08, false);
    pub const CARD_LINE: Font = Font::ui(15.0, 20.0, Weight::Regular, 0.12, true);
    pub const OWNER: Font = Font::ui(13.0, 16.0, Weight::SemiBold, 0.16, true);

    /// The same style at another size, the line box scaled with it.
    pub const fn sized(self, size: f32) -> Self {
        Self { size, line: self.line * size / self.size, ..self }
    }
    pub const fn weight(self, weight: Weight) -> Self {
        Self { weight, ..self }
    }
    pub const fn tracking(self, tracking: f32) -> Self {
        Self { tracking, ..self }
    }
    pub const fn upper(self, upper: bool) -> Self {
        Self { upper, ..self }
    }

    pub fn id(&self) -> egui::FontId {
        let family = if self.lcd { LCD } else { FAMILY[self.weight as usize] };
        egui::FontId::new(self.size, egui::FontFamily::Name(family.into()))
    }

    /// The letter spacing in points.
    pub fn spacing(&self) -> f32 {
        self.tracking * self.size
    }

    /// The capital letters' height in points (the face's `H`), what a line centres on.
    pub fn cap(&self) -> f32 {
        static CAP: OnceLock<[f32; 2]> = OnceLock::new();
        let [ui, lcd] = *CAP.get_or_init(|| {
            use ab_glyph::Font as _;
            let h = |bytes: &[u8]| {
                let f = ab_glyph::FontRef::try_from_slice(bytes).expect("bundled font");
                f.outline(f.glyph_id('H')).map_or(0.7, |o| (o.bounds.max.y - o.bounds.min.y).abs() / f.units_per_em().unwrap_or(1000.0))
            };
            [h(OXANIUM[1]), h(SHARE_TECH_MONO)]
        });
        self.size * if self.lcd { lcd } else { ui }
    }

    /// `text` as this style shows it (upper case where it is).
    pub fn shown<'a>(&self, text: &'a str) -> std::borrow::Cow<'a, str> {
        if self.upper { text.to_uppercase().into() } else { text.into() }
    }

    /// One section of `text` in this style.
    pub fn job(&self, text: &str, color: Color32) -> egui::text::LayoutJob {
        let mut job = egui::text::LayoutJob::default();
        job.append(&self.shown(text), 0.0, egui::TextFormat { font_id: self.id(), extra_letter_spacing: self.spacing(), color, line_height: Some(self.line), ..Default::default() });
        job
    }
}

/// The kit's faces in front of `f`'s: Oxanium (each weight its own family, the regular one also egui's proportional),
/// Share Tech Mono; whatever was behind (the install's sans, egui's bundled fonts) stays behind them for the letters
/// they lack.
pub fn add_fonts(f: &mut egui::FontDefinitions) {
    let behind = f.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    for (i, (name, bytes)) in FAMILY.iter().zip(OXANIUM).enumerate() {
        f.font_data.insert((*name).into(), Arc::new(egui::FontData::from_static(bytes)));
        f.families.insert(egui::FontFamily::Name((*name).into()), std::iter::once(name.to_string()).chain(behind.iter().cloned()).collect());
        if i == 0 {
            f.families.entry(egui::FontFamily::Proportional).or_default().insert(0, name.to_string());
        }
    }
    f.font_data.insert(LCD.into(), Arc::new(egui::FontData::from_static(SHARE_TECH_MONO)));
    let mono = f.families.get(&egui::FontFamily::Monospace).cloned().unwrap_or_default();
    f.families.insert(egui::FontFamily::Name(LCD.into()), std::iter::once(LCD.to_string()).chain(mono).chain(behind).collect());
}

// ---- Painter helpers: the CSS effects as egui shapes ----

/// A gradient's colour stops: (position 0..1, colour).
pub type Stops<'a> = &'a [(f32, Color32)];

/// Which way a gradient runs.
#[derive(Clone, Copy)]
enum Dir {
    Down,
    Right,
    /// From a centre (0..1 of the box) out.
    Radial([f32; 2]),
}

/// A texture of `stops` across 64 texels (radial: 64 × 64), made once per gradient.
fn gradient_texture(ctx: &egui::Context, stops: Stops, dir: Dir) -> egui::TextureId {
    let key = egui::Id::new(("ig2-gradient", match dir { Dir::Down => [0, 0, 0], Dir::Right => [1, 0, 0], Dir::Radial(c) => [2, c[0].to_bits(), c[1].to_bits()] }, stops.iter().map(|(p, c)| (p.to_bits(), c.to_array())).collect::<Vec<_>>()));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(key)) {
        return t.id();
    }
    let at = |p: f32| -> Color32 {
        let p = p.clamp(0.0, 1.0);
        let i = stops.iter().position(|s| s.0 >= p).unwrap_or(stops.len() - 1);
        if i == 0 {
            return stops[0].1;
        }
        let (a, b) = (stops[i - 1], stops[i]);
        a.1.lerp_to_gamma(b.1, (p - a.0) / (b.0 - a.0).max(1e-6))
    };
    const N: usize = 64;
    let image = match dir {
        Dir::Down => egui::ColorImage::new([1, N], (0..N).map(|i| at((i as f32 + 0.5) / N as f32)).collect()),
        Dir::Right => egui::ColorImage::new([N, 1], (0..N).map(|i| at((i as f32 + 0.5) / N as f32)).collect()),
        Dir::Radial(c) => egui::ColorImage::new([N, N], (0..N * N).map(|i| {
            let (x, y) = ((i % N) as f32 + 0.5, (i / N) as f32 + 0.5);
            let (dx, dy) = (x / N as f32 - c[0], y / N as f32 - c[1]);
            // The farthest corner is 1 (CSS `farthest-corner`).
            let far = (c[0].max(1.0 - c[0]).powi(2) + c[1].max(1.0 - c[1]).powi(2)).sqrt();
            at((dx * dx + dy * dy).sqrt() / far)
        }).collect()),
    };
    let t = ctx.load_texture("ig2-gradient", image, egui::TextureOptions::LINEAR);
    let id = t.id();
    ctx.data_mut(|d| d.insert_temp(key, t));
    id
}

fn textured(p: &Painter, rect: Rect, r: impl Into<CornerRadius>, tex: egui::TextureId) {
    p.add(egui::epaint::RectShape::filled(rect, r, Color32::WHITE).with_texture(tex, Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0))));
}

/// `rect` with rounded corners `r` filled top to bottom with `stops` (CSS `linear-gradient(180deg, …)`).
pub fn vgrad(p: &Painter, rect: Rect, r: impl Into<CornerRadius>, stops: Stops) {
    textured(p, rect, r, gradient_texture(p.ctx(), stops, Dir::Down));
}

/// `rect` filled left to right with `stops` (CSS `linear-gradient(90deg, …)`).
pub fn hgrad(p: &Painter, rect: Rect, r: impl Into<CornerRadius>, stops: Stops) {
    textured(p, rect, r, gradient_texture(p.ctx(), stops, Dir::Right));
}

/// `rect` filled with a radial gradient from `centre` (0..1 of the box) out to the farthest corner (CSS
/// `radial-gradient(circle at …)`).
pub fn radial(p: &Painter, rect: Rect, r: impl Into<CornerRadius>, centre: [f32; 2], stops: Stops) {
    textured(p, rect, r, gradient_texture(p.ctx(), stops, Dir::Radial(centre)));
}

/// A one-point rim inside `rect` (CSS `inset 0 0 0 1px`).
pub fn rim(p: &Painter, rect: Rect, r: impl Into<CornerRadius>, c: Color32) {
    p.rect_stroke(rect, r, Stroke::new(1.0f32, c), StrokeKind::Inside);
}

/// A lit (or dark) one-point line along the inside of `rect`'s top (`top`) or bottom edge (CSS `inset 0 ±1px 0 0`),
/// between the corners.
pub fn edge_line(p: &Painter, rect: Rect, r: f32, top: bool, c: Color32) {
    let y = if top { rect.top() + 0.5 } else { rect.bottom() - 0.5 };
    let inset = (r * 0.6).max(1.0);
    p.line_segment([egui::pos2(rect.left() + inset, y), egui::pos2(rect.right() - inset, y)], Stroke::new(1.0f32, c));
}

/// A soft shadow (or glow) of `rect` (CSS `0 dy blur spread colour`), drawn under what comes next.
pub fn shadow(p: &Painter, rect: Rect, r: impl Into<CornerRadius>, dy: i8, blur: u8, spread: u8, c: Color32) {
    p.add(egui::Shadow { offset: [0, dy], blur, spread, color: c }.as_shape(rect, r));
}

/// The design's soft inner shading of a raised part (`--shadow-relief`'s insets): a lit band under the top edge, a
/// dark one over the bottom.
pub fn relief(p: &Painter, rect: Rect, r: impl Into<CornerRadius> + Copy) {
    let k = tokens();
    let h = rect.height().max(1.0);
    let shine = k.metal_shine.gamma_multiply(0.55);
    let dark = k.metal_shadow.gamma_multiply(0.9);
    vgrad(p, rect, r, &[(0.0, shine), (2.0 / h, Color32::TRANSPARENT), (1.0 - 4.0 / h, Color32::TRANSPARENT), (1.0, dark)]);
}

/// The plate's gradient (plate → low at 55 % → plate) with `--shadow-plate`; `high` lit (pressed, the primary).
pub fn plate_fill(p: &Painter, rect: Rect, r: u8, high: bool) {
    let k = tokens();
    shadow(p, rect.translate(Vec2::new(0.0, 2.0)), r, 0, 6, 0, Color32::from_black_alpha(178));
    if high {
        p.rect_filled(rect, r, k.plate_high);
    } else {
        vgrad(p, rect, r, &[(0.0, k.plate), (0.55, k.plate_low), (1.0, k.plate)]);
    }
    edge_line(p, rect, r as f32, true, k.plate_high);
    edge_line(p, rect, r as f32, false, hex(0x061722));
}

/// The glass panel (`.ig-panel`): its fill at `alpha`, the faint 22-point circuit grid, the rim and lit top edge,
/// the soft outer shadow.
pub fn glass(p: &Painter, rect: Rect, fill: Color32, grid: bool) {
    let k = tokens();
    let r = RADIUS_PANEL;
    shadow(p, rect, r, 0, 18, 0, Color32::from_black_alpha(153));
    p.rect_filled(rect, r, fill);
    if grid {
        let clip = p.with_clip_rect(rect.shrink(1.0).intersect(p.clip_rect()));
        let (v, h) = (k.rim_bright.gamma_multiply(0.05), k.rim_bright.gamma_multiply(0.04));
        let mut x = rect.left();
        while x < rect.right() {
            clip.vline(x + 0.5, rect.y_range(), Stroke::new(1.0f32, v));
            x += 22.0;
        }
        let mut y = rect.top();
        while y < rect.bottom() {
            clip.hline(rect.x_range(), y + 0.5, Stroke::new(1.0f32, h));
            y += 22.0;
        }
    }
    rim(p, rect, r, k.rim);
    edge_line(p, rect, r as f32, true, k.rim_bright);
}

/// The LCD window (`.ig-lcd`): black to olive, a metal rim, the recess's inner shadow.
pub fn lcd_window(p: &Painter, rect: Rect) {
    let k = tokens();
    let h = rect.height().max(1.0);
    vgrad(p, rect, RADIUS_PLATE, &[(0.0, Color32::BLACK), (1.0, k.lcd)]);
    vgrad(p, rect, RADIUS_PLATE, &[(0.0, Color32::from_black_alpha(230)), (4.0 / h, Color32::TRANSPARENT), (1.0, Color32::TRANSPARENT)]);
    rim(p, rect, RADIUS_PLATE, k.metal_dark);
}

/// The keyboard focus mark (`:focus-visible`): a two-point outline `gap` points outside `rect`.
pub fn focus_mark(p: &Painter, rect: Rect, r: f32, gap: f32) {
    p.rect_stroke(rect.expand(gap), r + gap, Stroke::new(2.0f32, tokens().focus), StrokeKind::Outside);
}

/// `galley` drawn at `pos` slanted forward by `shear` (x per y: the LCD's italic), in its own colours.
pub fn sheared_galley(p: &Painter, pos: Pos2, galley: &Arc<egui::Galley>, shear: f32) {
    let ctx = p.ctx();
    let (size, discs) = ctx.fonts(|f| (f.font_image_size(), f.texture_atlas().lock().prepared_discs()));
    let mut t = egui::epaint::Tessellator::new(ctx.pixels_per_point(), Default::default(), size, discs);
    let mut mesh = egui::Mesh::default();
    t.tessellate_text(&egui::epaint::TextShape::new(pos, galley.clone(), Color32::WHITE), &mut mesh);
    let base = pos.y + galley.rect.height();
    for v in &mut mesh.vertices {
        v.pos.x += (base - v.pos.y) * shear;
    }
    p.add(Shape::mesh(mesh));
}

/// `galley` at `pos` with a soft glow of `c` around it (CSS `text-shadow: 0 0 6px`).
pub fn glowing_galley(p: &Painter, pos: Pos2, galley: &Arc<egui::Galley>, c: Color32) {
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        p.galley_with_override_text_color(pos + Vec2::angled(a) * 1.6, galley.clone(), c.gamma_multiply(0.22));
    }
    p.galley(pos, galley.clone(), Color32::WHITE);
}

/// The wells a page leaves for the renderer's pictures (what, where, the opacity) from points into pixels.
pub fn wells_in_pixels<T>(wells: &mut [(T, Rect, f32)], ppp: f32) {
    for w in wells {
        w.1 = Rect::from_min_max((w.1.min.to_vec2() * ppp).to_pos2(), (w.1.max.to_vec2() * ppp).to_pos2());
    }
}

/// The kit's animations run (off: every state change is instant, a snapshot's still or a reduced-motion setting).
static MOTION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_motion(on: bool) {
    MOTION.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// An animation's length: `t` seconds, or none without motion.
pub fn anim(t: f32) -> f32 {
    if MOTION.load(std::sync::atomic::Ordering::Relaxed) { t } else { 0.0 }
}

/// How far `id`'s hover is in (0..1), fading over [`HOVER_TIME`].
pub fn hover_t(ctx: &egui::Context, id: egui::Id, hovered: bool) -> f32 {
    ctx.animate_bool_with_time(id.with("hover"), hovered, anim(HOVER_TIME))
}

/// How far a panel `id` is open (0..1) over [`OPEN_TIME`], eased; a page fades and slides it by this.
pub fn open_t(ctx: &egui::Context, id: egui::Id, open: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(id.with("open"), open, anim(OPEN_TIME), egui::emath::easing::cubic_out)
}

/// A planet's globe: its surface map `tex` (equirectangular, the texture repeating across) on a sphere of `radius` at
/// `centre`, turned by `spin` (0..1 a full turn), lit from the upper left with a soft dark limb and a faint glow of
/// the atmosphere `air`; `alpha` fades it.
pub fn globe(p: &Painter, centre: Pos2, radius: f32, tex: egui::TextureId, spin: f32, air: Color32, alpha: f32) {
    use std::f32::consts::PI;
    const N: u32 = 24;
    let r = Rect::from_center_size(centre, Vec2::splat(2.0 * radius));
    shadow(p, r, 255, 0, (radius * 0.35) as u8, 0, air.gamma_multiply(0.35 * alpha));
    let light = [-0.55f32, -0.6, 0.58];
    let mut mesh = egui::Mesh::with_texture(tex);
    for j in 0..=N {
        for i in 0..=N {
            // A grid over the disc: the square's points pulled onto the circle (rows at fixed latitude).
            let lat = PI * (j as f32 / N as f32 - 0.5);
            let lon = PI * (i as f32 / N as f32 - 0.5);
            let (x, y, z) = (lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos());
            let lit = (x * light[0] + y * light[1] + z * light[2]).max(0.0);
            let shade = (0.22 + 0.9 * lit) * (0.55 + 0.45 * z.sqrt());
            let c = Color32::from_gray((shade.min(1.0) * 255.0) as u8).gamma_multiply(alpha);
            let uv = Pos2::new(lon / (2.0 * PI) + 0.25 + spin, j as f32 / N as f32);
            mesh.vertices.push(egui::epaint::Vertex { pos: centre + Vec2::new(x, y) * radius, uv, color: c });
        }
    }
    for j in 0..N {
        for i in 0..N {
            let a = j * (N + 1) + i;
            mesh.add_triangle(a, a + 1, a + N + 2);
            mesh.add_triangle(a, a + N + 2, a + N + 1);
        }
    }
    p.add(Shape::mesh(mesh));
    p.circle_stroke(centre, radius - 0.5, Stroke::new(1.0f32, air.gamma_multiply(0.45 * alpha)));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Modern's theme file is the deep-space palette, every mapped role exactly; another theme moves the roles it maps
    /// and keeps the design's own colours.
    #[test]
    fn modern_theme_is_the_deep_space_palette() {
        assert_eq!(Tokens::from_theme(&Theme::modern()), Tokens::DEEP_SPACE);
        let mut t = Theme::modern();
        t.panel = [18.0, 58.0, 96.0];
        t.accent = [1.0, 0.0, 0.0];
        let k = Tokens::from_theme(&t);
        assert_eq!(k.glass, Color32::from_rgb(18, 58, 96));
        assert_eq!(k.glass_deep, Color32::from_rgb(0, 24, 40));
        assert_eq!(k.info, Color32::RED);
        assert_eq!((k.lcd_ink, k.metal, k.own), (Tokens::DEEP_SPACE.lcd_ink, Tokens::DEEP_SPACE.metal, Tokens::DEEP_SPACE.own));
        crate::gui::style(&t);
        assert_eq!(tokens(), k);
    }

    /// The type scale is `tokens.css`'s; the faces have the languages' letters (Hungarian's ő and ű) and a capital
    /// height a line can centre on.
    #[test]
    fn type_scale_and_letters() {
        assert_eq!((Font::BODY.size, Font::BODY.line, Font::LABEL.tracking, Font::READOUT.lcd), (17.0, 26.0, 0.14, true));
        assert!((Font::LABEL.spacing() - 1.82).abs() < 1e-4);
        assert_eq!(Font::LABEL.shown("Képernyő"), "KÉPERNYŐ");
        assert_eq!(Font::DATA.sized(30.0).line, 40.0);
        let cap = Font::BODY.cap();
        assert!(cap > 0.6 * 17.0 && cap < 0.8 * 17.0, "{cap}");
        let ctx = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb).ctx;
        let _ = ctx.run(Default::default(), |_| {});
        for f in [Font::BODY, Font::LABEL, Font::HEADING, Font::READOUT] {
            assert!(ctx.fonts(|x| x.has_glyphs(&f.id(), "őűŐŰáéíóöúüÁÉäßàçèñ¿¡ìò€…")), "{f:?}");
        }
    }
}
