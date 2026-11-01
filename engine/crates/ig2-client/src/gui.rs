//! Ours (ADR 0062): the egui layer for the remake's own designed pages. A page runs once a frame through [`Gui::run`]
//! in the band's pixels, then [`Gui::paint`] draws it over our 3D and 2D and under the game's pointer. The pointer's
//! events come in band pixels ([`Gui::pointer`], [`Gui::button`], [`Gui::wheel`]): the window's, a script's and a
//! test's go the same way. The keyboard stays with the page's own navigation. The UI theme (ADR 0056) is the style
//! ([`style`]); the shared widgets for the pages are below it.

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
}

impl Gui {
    /// A layer drawing into targets of `format`.
    pub fn new(format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        ctx.options_mut(|o| o.input_options.line_scroll_speed = 60.0);
        Self { ctx, renderer: None, format, events: Vec::new(), at: None, start: web_time::Instant::now(), painted: None, hand: false }
    }

    /// The remaster's sans (`fonts/jurabook.ttf`, in the install) for the pages' text where the install has it; egui's
    /// bundled font stays behind it for the letters it lacks, and is the font without it.
    pub fn install_fonts(self, install: &ig2_formats::Install) -> Self {
        // An `ecGs` container on disk; a file that doesn't parse would panic in egui, so it is checked first.
        if let Some(bytes) = install.read("fonts/jurabook.ttf").ok().filter(|b| ab_glyph::FontRef::try_from_slice(b).is_ok()) {
            let mut f = egui::FontDefinitions::default();
            f.font_data.insert("jura".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            f.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "jura".into());
            self.ctx.set_fonts(f);
        }
        self
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
        self.hand = out.platform_output.cursor_icon == egui::CursorIcon::PointingHand;
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

/// A box colour (0..255) at `alpha`.
fn boxed(c: [f32; 3], alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0] as u8, c[1] as u8, c[2] as u8, alpha)
}

/// A text or quad colour (0..1).
pub fn ink(c: [f32; 3]) -> Color32 {
    Color32::from_rgb((c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8)
}

/// The theme as egui's style: dark glass panels in the theme's panel colour, its accent on what is on or chosen, the
/// hover colour on what is under the pointer, rounded corners, a clear type scale.
pub fn style(t: &Theme) -> egui::Style {
    use egui::{FontFamily::Proportional, FontId, TextStyle};
    let mut s = egui::Style {
        text_styles: [
            (TextStyle::Heading, FontId::new(26.0, Proportional)),
            (TextStyle::Body, FontId::new(15.0, Proportional)),
            (TextStyle::Button, FontId::new(15.0, Proportional)),
            (TextStyle::Small, FontId::new(12.5, Proportional)),
            (TextStyle::Monospace, FontId::new(14.0, egui::FontFamily::Monospace)),
        ]
        .into(),
        ..Default::default()
    };
    let sp = &mut s.spacing;
    (sp.item_spacing, sp.button_padding, sp.interact_size.y) = (egui::vec2(8.0, 6.0), egui::vec2(14.0, 6.0), 28.0);
    (sp.slider_width, sp.combo_width, sp.window_margin) = (190.0, 200.0, Margin::same(20));
    s.interaction.selectable_labels = false;
    let v = &mut s.visuals;
    let (text, accent, hot) = (ink(t.text), ink(t.accent), ink(t.hot));
    v.dark_mode = true;
    v.override_text_color = None;
    v.window_fill = boxed(t.panel, 236);
    v.panel_fill = v.window_fill;
    v.window_stroke = Stroke::new(1.0f32, boxed(t.panel_edge, 200));
    v.window_corner_radius = CornerRadius::same(16);
    v.window_shadow = Shadow { offset: [0, 12], blur: 36, spread: 0, color: Color32::from_black_alpha(150) };
    v.popup_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(140) };
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = ink(t.off_track);
    v.faint_bg_color = boxed(t.tab, 160);
    v.hyperlink_color = accent;
    v.selection.bg_fill = ink(t.on_track);
    v.selection.stroke = Stroke::new(1.5f32, text);
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.weak_text_color = Some(ink(t.dim));
    let w = &mut v.widgets;
    let r = CornerRadius::same(8);
    w.noninteractive.fg_stroke = Stroke::new(1.0f32, text);
    w.noninteractive.bg_stroke = Stroke::new(1.0f32, boxed(t.panel_edge, 110));
    w.noninteractive.bg_fill = boxed(t.panel, 255);
    w.noninteractive.corner_radius = r;
    w.inactive = egui::style::WidgetVisuals { bg_fill: ink(t.off_track), weak_bg_fill: boxed(t.button, 255), bg_stroke: Stroke::new(1.0f32, boxed(t.button_edge, 255)), corner_radius: r, fg_stroke: Stroke::new(1.0f32, text), expansion: 0.0 };
    w.hovered = egui::style::WidgetVisuals { bg_fill: ink(t.off_track), weak_bg_fill: boxed(t.tab_on, 255), bg_stroke: Stroke::new(1.0f32, hot), corner_radius: r, fg_stroke: Stroke::new(1.5f32, hot), expansion: 1.0 };
    w.active = egui::style::WidgetVisuals { bg_fill: ink(t.on_track), weak_bg_fill: ink(t.on_track), bg_stroke: Stroke::new(1.0f32, accent), corner_radius: r, fg_stroke: Stroke::new(2.0f32, text), expansion: 1.0 };
    w.open = w.active;
    s
}

/// The page's dimmed backdrop over the screen under it, and its centred glass panel `size` points big.
pub fn page(ctx: &egui::Context, id: &str, size: Vec2, add: impl FnOnce(&mut egui::Ui)) {
    let screen = ctx.screen_rect();
    ctx.layer_painter(egui::LayerId::background()).rect_filled(screen, 0.0, Color32::from_black_alpha(150));
    let size = size.min(screen.size() - egui::vec2(16.0, 16.0));
    egui::Area::new(egui::Id::new(id)).fixed_pos(screen.center() - size / 2.0).fade_in(false).show(ctx, |ui| {
        egui::Frame::window(ui.style()).show(ui, |ui| {
            let inner = size - egui::vec2(40.0, 40.0);
            ui.set_min_size(inner);
            ui.set_max_size(inner);
            add(ui);
        });
    });
}

/// A titled card: a slightly lighter rounded box grouping settings.
pub fn card<R>(ui: &mut egui::Ui, title: Option<&str>, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let v = ui.visuals().clone();
    egui::Frame::new()
        .fill(v.faint_bg_color)
        .stroke(v.widgets.noninteractive.bg_stroke)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(t) = title {
                ui.label(egui::RichText::new(t.to_uppercase()).small().strong().color(v.hyperlink_color));
                ui.add_space(2.0);
            }
            add(ui)
        })
        .inner
}

/// An on/off switch (the track fills with the accent when on, the knob slides).
pub fn switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let size = egui::vec2(2.0, 1.1) * ui.spacing().interact_size.y * 0.8;
    let (rect, mut resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *on, ""));
    if ui.is_rect_visible(rect) {
        let k = ui.ctx().animate_bool_responsive(resp.id, *on);
        let w = ui.style().interact_selectable(&resp, *on);
        let r = rect.height() / 2.0;
        let track = if *on { ui.visuals().selection.bg_fill } else { ui.visuals().extreme_bg_color };
        ui.painter().rect(rect, r, track, w.bg_stroke, egui::StrokeKind::Inside);
        let x = egui::lerp((rect.left() + r)..=(rect.right() - r), k);
        let knob = if ui.is_enabled() { ui.visuals().widgets.active.fg_stroke.color } else { ui.visuals().weak_text_color() };
        ui.painter().circle(egui::pos2(x, rect.center().y), r * 0.78, knob, Stroke::NONE);
    }
    resp
}

/// The accented button of a page's main action.
pub fn primary(ui: &mut egui::Ui, enabled: bool, text: &str) -> egui::Response {
    let fill = if enabled { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.inactive.weak_bg_fill };
    ui.add_enabled(enabled, egui::Button::new(egui::RichText::new(text).strong()).fill(fill).min_size(egui::vec2(120.0, 32.0)))
}

/// A plain button of a page's footer.
pub fn secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(text).min_size(egui::vec2(120.0, 32.0)))
}

/// The keyboard's focus mark: an accent outline around `rect`.
pub fn focus_ring(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().rect_stroke(rect.expand(2.0), 9.0, Stroke::new(1.5f32, ui.visuals().hyperlink_color), egui::StrokeKind::Outside);
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
            assert_eq!(s.visuals.window_corner_radius, CornerRadius::same(16));
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
