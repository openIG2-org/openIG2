//! The window's GPU: the surface, its configuration and the renderer's setup.

use super::*;

impl App {
    pub(super) fn init_gpu(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        let attrs = {
            let d = self.display;
            let attrs = Window::default_attributes().with_title(format!("Imperium Galactica II — openIG2 {}", crate::version()));
            let attrs = match d.size {
                Some((w, h)) if d.mode == crate::display::Mode::Window => attrs.with_inner_size(winit::dpi::PhysicalSize::new(w, h)),
                _ => attrs.with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0)),
            };
            // Hidden until its first (black) frame: shown at once, the system paints it white while the assets load.
            with_icon(attrs.with_fullscreen(d.fullscreen(el.primary_monitor())).with_visible(false))
        };
        // A canvas added to the page, sized by the page's stylesheet (web/index.html).
        #[cfg(target_arch = "wasm32")]
        let attrs = winit::platform::web::WindowAttributesExtWebSys::with_append(Window::default_attributes(), true);
        let window = Arc::new(el.create_window(attrs).map_err(|e| e.to_string())?);
        #[cfg(not(target_arch = "wasm32"))]
        return self.finish_gpu(pollster::block_on(GpuParts::request(window))?);
        // The page's adapter and device come asynchronously; `about_to_wait` finishes the start (ADR 0047 §4).
        #[cfg(target_arch = "wasm32")]
        {
            let wait = std::rc::Rc::new(std::cell::RefCell::new(None));
            let to = wait.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let w = window.clone();
                *to.borrow_mut() = Some(GpuParts::request(window).await);
                w.request_redraw();
            });
            self.gpu_wait = Some(wait);
            Ok(())
        }
    }

    pub(super) fn finish_gpu(&mut self, parts: GpuParts) -> Result<(), String> {
        let GpuParts { window, surface, adapter, device, queue } = parts;
        // The Options screen's display rows, offering the monitor's sizes.
        let now = window.inner_size();
        self.opt_rows.retain(|r| !r.ours);
        self.opt_rows.extend(crate::display::rows(&self.settings, &crate::display::sizes(window.current_monitor().as_ref()), (now.width, now.height)));
        let caps = surface.get_capabilities(&adapter);
        // An sRGB frame with raw views for the gamma passes; without such views (WebGL2) a raw frame, the shaders
        // doing the sRGB transfer (ADR 0047 §4).
        let views = adapter.get_downlevel_capabilities().flags.contains(wgpu::DownlevelFlags::VIEW_FORMATS);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb() == views).unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: if self.vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync },
            desired_maximum_frame_latency: 1,
            alpha_mode: caps.alpha_modes[0],
            view_formats: Renderer::view_formats(format),
        };
        surface.configure(&device, &config);
        present_black(&surface, &device, &queue);
        window.set_visible(true);
        let mut renderer = Renderer::new(device, queue, format);
        crate::prof::mark("gpu");
        let assets = Assets::load(&mut renderer, &self.install);
        crate::prof::mark("assets");
        self.panel_models = Some(PanelModels::new(&mut renderer));
        self.frame_models = Some(crate::screen_frames::FrameModels::new(&mut renderer));
        let scale = window.scale_factor() as f32;
        let [_, _, bw, bh] = crate::kit::Layout::new(config.width as i32, config.height as i32).band();
        self.view = Some(View::new(&self.world.curr, &self.world.view, (bw as u32, bh as u32), scale));
        let sel = self.selection();
        if sel.mode == 1 || sel.mode == 2 {
            self.select(sel);
        }
        let mut hud = crate::ui::Hud::load(&mut renderer, &self.install, self.tables);
        crate::prof::mark("hud");
        // The messages quote the empires by `BirodalomNev`: the exe's table, the same on every peer (ADR 0017).
        self.world.game.empire_names = hud.empire_names.clone();
        // A game started from the command line records from here, as one from the menu does once its names are set
        // (ADR 0038 §4; they are in the state).
        if self.front.is_none() && self.world.viewer.is_none() && self.world.replay.is_none() {
            self.start_replay();
        }
        hud.set_one_language(&self.install, self.one_language);
        self.hud = Some(hud);
        // Loaded with the rest at start: at its first opening it would hold that frame for tens of milliseconds.
        self.spy_scene = crate::spy_scene::SpyScene::load(&mut renderer, &self.install).ok().map(std::rc::Rc::new);
        crate::prof::mark("spy scene");
        self.gpu = Some(Gpu { window, surface, adapter, config, renderer, assets, band_target: None, gui: crate::gui::Gui::new(format).install_fonts(&self.install) });
        Ok(())
    }

    /// A frame the surface couldn't take: a lost or outdated surface is configured again (the next frame draws),
    /// anything else is logged.
    pub(super) fn surface_failed(&mut self, e: wgpu::SurfaceError) {
        match e {
            wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => self.reconfigure(),
            e => eprintln!("ig2-client: {e}"),
        }
    }

    pub(super) fn reconfigure(&mut self) {
        let Some(g) = &mut self.gpu else { return };
        let size = g.window.inner_size();
        g.config.width = size.width.max(1);
        g.config.height = size.height.max(1);
        g.config.present_mode = if self.vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
        g.surface.configure(&g.renderer.device, &g.config);
        g.renderer.forget_sizes();
        if let Some(v) = &mut self.view {
            v.resize(g.band(), g.window.scale_factor() as f32);
        }
        if let Some(c) = &mut self.colony {
            c.view.resize(g.band(), g.window.scale_factor() as f32);
        }
    }
}

/// One black frame, before the assets load (a frame the surface can't give is skipped: the window then shows
/// whatever the system draws until the first real frame).
fn present_black(surface: &wgpu::Surface, device: &wgpu::Device, queue: &wgpu::Queue) {
    let Ok(frame) = surface.get_current_texture() else { return };
    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
        })],
        ..Default::default()
    });
    queue.submit([enc.finish()]);
    frame.present();
}
