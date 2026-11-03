//! Ours (ADR 0062): the egui layer for the remake's own designed pages. A page runs once a frame through [`Gui::run`]
//! in the band's pixels, then [`Gui::paint`] draws it over our 3D and 2D and under the game's pointer. The pointer's
//! events come in band pixels ([`Gui::pointer`], [`Gui::button`], [`Gui::wheel`]): the window's, a script's and a
//! test's go the same way. The keyboard stays with the page's own navigation. The UI theme (ADR 0056) is the style
//! ([`style`]); the shared widgets for the pages are below it.

pub mod ds;

use crate::theme::Theme;
use egui::{Color32, CornerRadius, Margin, Shadow, Stroke, Vec2};

/// The page's height in points at 100 % UI scale and any window size (a 1080-line window draws a point as a pixel),
/// so a page scales with the window as every screen does (and stays sharp: egui draws at the window's
/// pixels). The window's DPI factor plays no part: the band is already in physical pixels.
pub const POINTS_HIGH: f32 = 1080.0;

/// The "UI scale" option in per cent (ADR 0062; the settings file's `UiScale`), read by every page's next frame.
static UI_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);

pub fn set_ui_scale(percent: u32) {
    UI_SCALE.store(percent.clamp(50, 200), std::sync::atomic::Ordering::Relaxed);
}

/// The egui context, its renderer and the frame's painted output.
pub struct Gui {
    pub ctx: egui::Context,
    /// Made at the first paint (a page run without a GPU, in a test, needs none).
    renderer: Option<egui_wgpu::Renderer>,
    format: wgpu::TextureFormat,
    events: Vec<egui::Event>,
    at: Option<egui::Pos2>,
    start: web_time::Instant,
    /// The last run's output: tessellated, waiting for [`Gui::paint`].
    painted: Option<(Vec<egui::ClippedPrimitive>, egui::TexturesDelta, egui_wgpu::ScreenDescriptor)>,
    /// The pointer is over something clickable (the game pointer's hand).
    pub hand: bool,
    /// The page's pointer (egui's cursor icon, ADR 0073 §5).
    pub cursor: egui::CursorIcon,
}

impl Gui {
    /// A layer drawing into targets of `format`.
    pub fn new(format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        ctx.options_mut(|o| o.input_options.line_scroll_speed = 60.0);
        let mut f = egui::FontDefinitions::default();
        ds::add_fonts(&mut f);
        ctx.set_fonts(f);
        Self { ctx, renderer: None, format, events: Vec::new(), at: None, start: web_time::Instant::now(), painted: None, hand: false, cursor: egui::CursorIcon::Default }
    }

    /// The design's faces (ADR 0073: Oxanium, Share Tech Mono, in the client) in front of the remaster's sans
    /// (`fonts/jurabook.ttf`, in the install) where the install has it, then egui's bundled fonts, for the letters the
    /// ones before lack.
    pub fn install_fonts(self, install: &ig2_formats::Install) -> Self {
        // An `ecGs` container on disk; a file that doesn't parse would panic in egui, so it is checked first.
        if let Some(bytes) = install.read("fonts/jurabook.ttf").ok().filter(|b| ab_glyph::FontRef::try_from_slice(b).is_ok()) {
            let mut f = egui::FontDefinitions::default();
            f.font_data.insert("jura".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            f.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "jura".into());
            ds::add_fonts(&mut f);
            self.ctx.set_fonts(f);
        }
        self
    }

    /// The page's pointer as the overlay's pointer type ([`ds::cursor::Cursor::kind`]).
    pub fn pointer_kind(&self) -> i32 {
        ds::cursor::Cursor::from_icon(self.cursor).kind()
    }

    /// Band pixels per point for a band `h` pixels high at the current UI scale.
    pub fn scale(h: u32) -> f32 {
        Self::scale_at(h, UI_SCALE.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// Band pixels per point for a band `h` pixels high at `percent` UI scale.
    pub fn scale_at(h: u32, percent: u32) -> f32 {
        (h as f32 / POINTS_HIGH * percent as f32 / 100.0).max(0.25)
    }

    /// The pointer moved to `px` (band pixels), or left the window (`None`).
    pub fn pointer(&mut self, px: Option<[f32; 2]>, scale: f32) {
        self.at = px.map(|p| egui::pos2(p[0] / scale, p[1] / scale));
        self.events.push(match self.at {
            Some(p) => egui::Event::PointerMoved(p),
            None => egui::Event::PointerGone,
        });
    }

    /// A mouse button (0 left, 1 right, 2 middle) went down or up where the pointer is.
    pub fn button(&mut self, button: u8, down: bool) {
        let (Some(pos), Some(button)) = (self.at, [egui::PointerButton::Primary, egui::PointerButton::Secondary, egui::PointerButton::Middle].get(button as usize)) else { return };
        self.events.push(egui::Event::PointerButton { pos, button: *button, pressed: down, modifiers: Default::default() });
    }

    /// The wheel turned `lines` lines (positive: up).
    pub fn wheel(&mut self, lines: f32) {
        self.events.push(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, lines), modifiers: Default::default() });
    }

    /// One frame of `page` over a band of `size` pixels.
    pub fn run(&mut self, size: (u32, u32), style: &egui::Style, page: impl FnMut(&egui::Context)) {
        let ppp = Self::scale(size.1);
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size.0 as f32, size.1 as f32) / ppp)),
            time: Some(self.start.elapsed().as_secs_f64()),
            events: std::mem::take(&mut self.events),
            focused: true,
            ..Default::default()
        };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
        self.ctx.set_style(style.clone());
        let out = self.ctx.run(input, page);
        self.cursor = out.platform_output.cursor_icon;
        self.hand = self.cursor == egui::CursorIcon::PointingHand;
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let mut textures = out.textures_delta;
        // A frame not painted (no surface texture) still uploads its textures with the next one.
        if let Some((_, old, _)) = self.painted.take() {
            textures.set.splice(0..0, old.set);
            textures.free.splice(0..0, old.free);
        }
        self.painted = Some((prims, textures, egui_wgpu::ScreenDescriptor { size_in_pixels: [size.0, size.1], pixels_per_point: out.pixels_per_point }));
    }

    /// Draw the last run's page over `target` (nothing when no page ran since the last paint).
    pub fn paint(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, target: &wgpu::Texture) {
        let Some((prims, textures, screen)) = self.painted.take() else { return };
        let renderer = self.renderer.get_or_insert_with(|| egui_wgpu::Renderer::new(device, self.format, None, 1, true));
        for (id, delta) in &textures.set {
            renderer.update_texture(device, queue, *id, delta);
        }
        let mut enc = device.create_command_encoder(&Default::default());
        let cmds = renderer.update_buffers(device, queue, &mut enc, &prims, &screen);
        let view = target.create_view(&Default::default());
        {
            let mut pass = enc
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            renderer.render(&mut pass, &prims, &screen);
        }
        queue.submit(cmds.into_iter().chain([enc.finish()]));
        for id in &textures.free {
            renderer.free_texture(id);
        }
    }
}

/// A text or quad colour (0..1).
pub fn ink(c: [f32; 3]) -> Color32 {
    Color32::from_rgb((c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8)
}

/// The theme as egui's style in the design kit's look (ADR 0073): deep glass windows with a rim, plate-blue buttons,
/// the lit plate on what is on or chosen, the bright rim under the pointer, 4-point corners, the design's faces at the
/// pages' type sizes. Sets the kit's tokens ([`ds::tokens`]) from the theme too.
pub fn style(t: &Theme) -> egui::Style {
    use egui::{FontFamily::Proportional, FontId, TextStyle};
    let k = ds::Tokens::from_theme(t);
    ds::set_tokens(k);
    let mut s = egui::Style {
        text_styles: [
            (TextStyle::Heading, ds::Font::HEADING.sized(26.0).id()),
            (TextStyle::Body, FontId::new(15.0, Proportional)),
            (TextStyle::Button, ds::Font::DATA.id()),
            (TextStyle::Small, FontId::new(12.5, Proportional)),
            (TextStyle::Monospace, FontId::new(14.0, egui::FontFamily::Monospace)),
        ]
        .into(),
        ..Default::default()
    };
    let sp = &mut s.spacing;
    (sp.item_spacing, sp.button_padding, sp.interact_size.y) = (egui::vec2(8.0, 6.0), egui::vec2(14.0, 6.0), 28.0);
    (sp.slider_width, sp.combo_width, sp.window_margin) = (190.0, 200.0, Margin::same(20));
    sp.scroll.bar_width = 8.0;
    s.interaction.selectable_labels = false;
    let v = &mut s.visuals;
    v.dark_mode = true;
    v.override_text_color = None;
    v.window_fill = k.glass_deep.gamma_multiply(0.88);
    v.panel_fill = v.window_fill;
    v.window_stroke = Stroke::new(1.0f32, k.rim);
    v.window_corner_radius = CornerRadius::same(ds::RADIUS_PANEL);
    v.window_shadow = Shadow { offset: [0, 0], blur: 18, spread: 0, color: Color32::from_black_alpha(153) };
    v.popup_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(140) };
    v.menu_corner_radius = CornerRadius::same(ds::RADIUS_PLATE);
    v.extreme_bg_color = k.glass_deep;
    v.faint_bg_color = k.glass.gamma_multiply(0.72);
    v.hyperlink_color = k.info;
    v.selection.bg_fill = k.plate_high;
    v.selection.stroke = Stroke::new(1.5f32, k.ink);
    v.text_cursor.stroke = Stroke::new(2.0f32, k.focus);
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.6 };
    v.weak_text_color = Some(k.ink_muted);
    let w = &mut v.widgets;
    let r = CornerRadius::same(ds::RADIUS_PLATE);
    let look = |bg_fill, weak_bg_fill, edge, fg| egui::style::WidgetVisuals { bg_fill, weak_bg_fill, bg_stroke: Stroke::new(1.0f32, edge), corner_radius: r, fg_stroke: Stroke::new(1.0f32, fg), expansion: 0.0 };
    w.noninteractive = look(k.glass, k.glass, k.rim, k.ink);
    w.inactive = look(k.plate, k.plate, k.rim, k.ink);
    w.hovered = look(k.plate.lerp_to_gamma(k.plate_high, 0.5), k.plate, k.rim_bright, k.ink);
    w.active = look(k.plate_high, k.plate_high, k.rim_bright, k.ink);
    w.open = w.active;
    s
}

/// The page's dimmed backdrop over the screen under it, and its centred deep glass panel `size` points big (fading
/// and sliding in as it opens).
pub fn page(ctx: &egui::Context, id: &str, size: Vec2, add: impl FnOnce(&mut egui::Ui)) {
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, Color32::from_black_alpha(128));
    let size = size.min(screen.size() - egui::vec2(16.0, 16.0));
    let t = ds::open_t(ctx, egui::Id::new(id), true);
    egui::Area::new(egui::Id::new(id)).fixed_pos(screen.center() - size / 2.0 + egui::vec2(0.0, (1.0 - t) * 12.0)).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(t);
        let at = ui.painter().add(egui::Shape::Noop);
        let r = egui::Frame::new().inner_margin(Margin::same(20)).show(ui, |ui| {
            let inner = size - egui::vec2(40.0, 40.0);
            ui.set_min_size(inner);
            ui.set_max_size(inner);
            add(ui);
        });
        let fill = ui.visuals().window_fill;
        ds::behind(ui, at, |p| ds::glass(p, r.response.rect, fill, true));
    });
}

/// A board's page (ADR 0078): the screen under it dimmed by `dim`, a deep glass panel `size` points (at most the
/// screen less 16) centred over it with 28 × 32 points inside, fading and sliding in as it opens.
pub fn board<R>(ctx: &egui::Context, id: &str, size: Vec2, dim: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, Color32::from_black_alpha((255.0 * dim) as u8));
    let size = size.min(screen.size() - egui::vec2(16.0, 16.0));
    let t = ds::open_t(ctx, egui::Id::new(id), true);
    egui::Area::new(egui::Id::new(id)).fixed_pos(screen.center() - size / 2.0 + egui::vec2(0.0, (1.0 - t) * 12.0)).fade_in(false).show(ctx, |ui| {
        ui.multiply_opacity(t);
        let at = ui.painter().add(egui::Shape::Noop);
        let r = egui::Frame::new().inner_margin(Margin { left: 32, right: 32, top: 28, bottom: 28 }).show(ui, |ui| {
            let inner = size - egui::vec2(64.0, 56.0);
            ui.set_min_size(inner);
            ui.set_max_size(inner);
            add(ui)
        });
        ds::behind(ui, at, |p| ds::paint_panel(p, r.response.rect, ds::PanelKind::Deep, 0.88));
        r.inner
    })
    .inner
}

/// A titled card: a lighter glass box grouping settings, its heading in the label style ([`ds::card`]).
pub fn card<R>(ui: &mut egui::Ui, title: Option<&str>, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ds::card(ui, title, add)
}

/// An on/off switch: a recessed well, the lit plate gliding to the right when on.
pub fn switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let size = egui::vec2(2.0, 1.1) * ui.spacing().interact_size.y * 0.8;
    let (rect, mut resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, ""));
    if ui.is_rect_visible(rect) {
        let k = ds::tokens();
        let x = ui.ctx().animate_bool_with_time(resp.id, *on, ds::anim(ds::GLIDE_TIME));
        let hover = ds::hover_t(ui.ctx(), resp.id, resp.hovered() && ui.is_enabled());
        let p = ui.painter();
        p.rect_filled(rect, ds::RADIUS_PLATE, k.glass_deep);
        ds::rim(p, rect, ds::RADIUS_PLATE, k.rim.lerp_to_gamma(k.rim_bright, hover));
        let half = egui::vec2(rect.width() / 2.0 - 3.0, rect.height() - 6.0);
        let knob = egui::Rect::from_min_size(egui::pos2(egui::lerp((rect.left() + 3.0)..=(rect.center().x), x), rect.top() + 3.0), half);
        let fill = if !ui.is_enabled() { k.plate } else { k.plate_low.lerp_to_gamma(k.plate_high, x) };
        p.rect_filled(knob, 3, fill);
        ds::edge_line(p, knob, 3.0, true, k.ink_silver.gamma_multiply(0.25 + 0.35 * x));
    }
    resp
}

/// The lit plate of a page's main action.
pub fn primary(ui: &mut egui::Ui, enabled: bool, text: &str) -> egui::Response {
    ui.add_enabled(enabled, ds::Plate::new(text).primary(enabled).width(120.0).height(32.0).font(ds::Plate::FONT.sized(15.0).tracking(0.12)))
}

/// A plain plate of a page's footer.
pub fn secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(ds::Plate::new(text).width(120.0).height(32.0).font(ds::Plate::FONT.sized(15.0).tracking(0.12)))
}

/// The keyboard's focus mark: the design's focus outline two points around `rect`.
pub fn focus_ring(ui: &egui::Ui, rect: egui::Rect) {
    ds::focus_mark(ui.painter(), rect, ds::RADIUS_PLATE as f32, 2.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 100 % draws a point as a pixel at 1080p and grows with the window; the option multiplies it (200 % is the
    /// pages' size before the option, 540 points high).
    #[test]
    fn scale_follows_the_window_and_the_option() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-5;
        assert!(near(Gui::scale_at(1080, 100), 1.0));
        assert!(near(Gui::scale_at(1440, 100), 4.0 / 3.0));
        assert!(near(Gui::scale_at(2160, 100), 2.0));
        assert!(near(Gui::scale_at(1080, 150), 1.5));
        assert!(near(Gui::scale_at(1080, 200), 2.0));
        assert!(near(Gui::scale_at(1080, 50), 0.5));
        assert_eq!(Gui::scale_at(240, 50), 0.25);
    }

    /// The bundled font has the languages' letters (Hungarian's ő and ű, the others' accents), and both themes map.
    #[test]
    fn font_has_the_letters_and_themes_map() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |_| {});
        let id = egui::FontId::proportional(15.0);
        assert!(ctx.fonts(|f| f.has_glyphs(&id, "őűŐŰáéíóöúüÁÉäßàçèñ¿¡ìò")));
        for t in [Theme::modern(), Theme::classic(|_, _| [1.0; 3])] {
            let s = style(&t);
            assert_eq!(s.visuals.window_corner_radius, CornerRadius::same(8));
            assert_eq!(s.visuals.hyperlink_color, ink(t.accent));
        }
    }

    /// A press and release over a button arrive as a click, in band pixels at any scale.
    #[test]
    fn pointer_events_click() {
        let mut g = Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb);
        let st = style(&Theme::modern());
        let mut clicks = 0;
        let size = (1920, 1080);
        let k = Gui::scale(size.1);
        let mut at = None;
        for step in 0..4 {
            g.run(size, &st, |ctx| {
                egui::Area::new(egui::Id::new("t")).fixed_pos(egui::pos2(100.0, 100.0)).show(ctx, |ui| {
                    let r = ui.button("Apply");
                    at = Some(r.rect.center());
                    clicks += r.clicked() as u32;
                });
            });
            let p = at.unwrap();
            match step {
                0 => g.pointer(Some([p.x * k, p.y * k]), k),
                1 => g.button(0, true),
                2 => g.button(0, false),
                _ => {}
            }
        }
        assert_eq!(clicks, 1);
        assert!((k - 1.0).abs() < 1e-5);
    }
}
