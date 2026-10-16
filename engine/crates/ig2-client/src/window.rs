//! The window and frame loop (ADR 0006 §1–§2): winit + a wgpu surface, the sim ticked by the original's
//! `TickClock` (20 Hz, ≤ 500 ms catch-up) and every frame drawn at the display's refresh rate, interpolated.
//!
//! Starmap: wheel = zoom, right drag = pan, Q/E = turn, PgUp/PgDn = tilt (the original's keyboard steps), left click =
//! select, R = reveal every fleet. C = the colony screen of the selected planet (else the human's first one) and back.
//! Colony: wheel = zoom, left drag = pan, right drag = turn and tilt (the original's both-buttons drag).
//! I = the info screen (Tab = next chart), M = the message inbox.
//! Always: 1/2/3 = speed, space = pause, V = vsync on/off, Esc = back to the starmap / quit.

use std::sync::Arc;
use std::time::Instant;

use ig2_sim::time::{Speed, TickClock, SIM_TICK_MS};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::render::Renderer;
use crate::starmap::{Assets, View};
use crate::world::World;

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    assets: Assets,
}

/// The open colony screen: its scene on the GPU and its view.
struct Colony {
    scene: crate::colony::ColonyScene,
    view: crate::colony::View,
}

struct App {
    world: World<'static>,
    install: ig2_formats::Install,
    tables: &'static ig2_rules::Rules,
    maps: &'static [ig2_sim::ground::GroundMap],
    colony: Option<Colony>,
    left_drag: Option<[f32; 2]>,
    hud: Option<crate::ui::Hud>,
    /// The 2D screen over the starmap (info, messages), if one is up.
    overlay: Option<crate::ui::Screen>,
    /// The message line's 10 ms UI frames done so far.
    ui_frames: u64,
    gpu: Option<Gpu>,
    view: Option<View>,
    clock: TickClock,
    start: Instant,
    paused: bool,
    vsync: bool,
    cursor: [f32; 2],
    drag: Option<[f32; 2]>,
    title_at: Instant,
    frames: u32,
}

pub fn run(o: crate::Options) -> Result<(), String> {
    let l = crate::load(&o)?;
    // The rules live for the whole process; `World` borrows them.
    let rules: &'static ig2_sim::game::GameRules = Box::leak(Box::new(l.rules));
    let tables: &'static ig2_rules::Rules = Box::leak(Box::new(l.tables));
    let mut world = World::load(&l.save, rules, &l.tech_defaults, &l.volatile);
    world.set_speed(o.speed);
    let start = Instant::now();
    let mut app = App {
        world,
        install: l.install,
        tables,
        maps: &rules.ground_maps,
        colony: None,
        left_drag: None,
        hud: None,
        overlay: None,
        ui_frames: 0,
        gpu: None,
        view: None,
        clock: TickClock::new(SIM_TICK_MS, 0),
        start,
        paused: false,
        vsync: true,
        cursor: [0.0; 2],
        drag: None,
        title_at: start,
        frames: 0,
    };
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

impl App {
    fn now_ms(&self) -> u32 {
        self.start.elapsed().as_millis() as u32
    }

    fn init_gpu(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let attrs = Window::default_attributes().with_title("Imperium Galactica II").with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0));
        let window = Arc::new(el.create_window(attrs).map_err(|e| e.to_string())?);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .map_err(|e| e.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(wgpu::TextureFormat::is_srgb).unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        let mut renderer = Renderer::new(device, queue, format);
        let assets = Assets::load(&mut renderer, &self.install);
        let scale = window.scale_factor() as f32;
        self.view = Some(View::new(&self.world.curr, &self.world.view, (config.width, config.height), scale));
        self.hud = Some(crate::ui::Hud::load(&mut renderer, &self.install, self.tables));
        self.gpu = Some(Gpu { window, surface, config, renderer, assets });
        Ok(())
    }

    fn reconfigure(&mut self) {
        let Some(g) = &mut self.gpu else { return };
        let size = g.window.inner_size();
        g.config.width = size.width.max(1);
        g.config.height = size.height.max(1);
        g.config.present_mode = if self.vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
        g.surface.configure(&g.renderer.device, &g.config);
        if let Some(v) = &mut self.view {
            v.resize((g.config.width, g.config.height), g.window.scale_factor() as f32);
        }
        if let Some(c) = &mut self.colony {
            c.view.resize((g.config.width, g.config.height), g.window.scale_factor() as f32);
        }
    }

    /// Open the colony screen of the selected planet (else the human's first planet), or close it.
    fn toggle_colony(&mut self) {
        if self.colony.take().is_some() {
            self.world.show_colony(None);
            return;
        }
        let human = self.world.curr.human;
        let selected = match self.view.as_ref().and_then(|v| v.selected) {
            Some(crate::starmap::Pick::Planet(p)) => Some(p),
            _ => None,
        };
        let Some(p) = selected.or_else(|| self.world.curr.planets.iter().find(|p| p.owner == human).map(|p| p.index)) else { return };
        let Some(g) = &mut self.gpu else { return };
        self.world.show_colony(Some(p));
        let Some(snap) = self.world.colony_curr.clone() else { return };
        match crate::colony::ColonyScene::load(&mut g.renderer, &self.install, self.tables, self.maps, &snap) {
            Ok(scene) => {
                let view = crate::colony::View::new(&snap, (g.config.width, g.config.height), g.window.scale_factor() as f32, &scene);
                self.colony = Some(Colony { scene, view });
            }
            Err(e) => {
                eprintln!("ig2-client: colony {p}: {e}");
                self.world.show_colony(None);
            }
        }
    }

    fn frame(&mut self) {
        let now = self.now_ms();
        let due = self.clock.advance(now);
        if !self.paused {
            for _ in 0..due {
                self.world.tick();
            }
        }
        // The message line runs on the 100 Hz UI clock (`FRISSIT_Scroll2`), also while paused.
        let due_ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50);
        self.ui_frames = now as u64 / 10;
        if let Some(h) = &mut self.hud {
            let world = &mut self.world;
            for _ in 0..due_ui {
                h.scroller.step(&mut || world.pop_message());
            }
        }
        let alpha = if self.paused { 1.0 } else { self.clock.phase(now) };
        let (Some(g), Some(v)) = (&mut self.gpu, &mut self.view) else { return };
        let t = now as f32 / 1000.0;
        let size = [g.config.width as f32, g.config.height as f32];
        let (mut frame, screen) = match (self.overlay, &mut self.colony, &self.world.colony_prev, &self.world.colony_curr) {
            (Some(s), ..) => (crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]), s),
            (None, Some(c), Some(prev), Some(curr)) => {
                (c.view.build(&mut g.renderer, &self.install, self.tables, self.maps, &mut c.scene, prev, curr, alpha, t), crate::ui::Screen::Colony)
            }
            _ => {
                v.hover = v.pick(&self.world.prev, &self.world.curr, alpha, self.cursor);
                (v.build(&g.assets, &self.world.prev, &self.world.curr, alpha, t), crate::ui::Screen::Starmap)
            }
        };
        if let Some(h) = &mut self.hud {
            h.speed = crate::hud::speed_index(self.world.game.state.mem.i32(ig2_sim::save::va::IDO_SPEED));
            h.paused = self.paused;
            match (screen, &self.colony, &self.world.colony_curr) {
                (crate::ui::Screen::Colony, Some(c), Some(curr)) => h.draw_colony(&mut frame, &self.world.game.state, self.tables, self.maps, &c.view, &c.scene, curr),
                (crate::ui::Screen::Starmap, ..) => h.draw_starmap_names(&mut frame, v, &g.assets, &self.world.prev, &self.world.curr, alpha, t),
                _ => {}
            }
            h.draw(&mut frame, size, &self.world.ui, self.tables, screen, t);
        }
        let out = match g.surface.get_current_texture() {
            Ok(out) => out,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.reconfigure();
                return;
            }
            Err(e) => {
                eprintln!("ig2-client: {e}");
                return;
            }
        };
        let view = out.texture.create_view(&Default::default());
        g.renderer.draw(&frame, &view, (g.config.width, g.config.height), t);
        g.window.pre_present_notify();
        out.present();
        self.frames += 1;
        if self.title_at.elapsed().as_secs_f32() >= 1.0 {
            let fps = self.frames as f32 / self.title_at.elapsed().as_secs_f32();
            let (y, m, d) = self.world.curr.date();
            let speed = self.world.game.state.mem.i32(ig2_sim::save::va::IDO_SPEED);
            let state = if self.paused { "paused".to_string() } else { format!("×{speed}") };
            g.window.set_title(&format!("Imperium Galactica II — {y}-{m:02}-{d:02} {state} — {fps:.0} fps"));
            self.title_at = Instant::now();
            self.frames = 0;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_none() {
            if let Err(e) = self.init_gpu(el) {
                eprintln!("ig2-client: {e}");
                el.exit();
            }
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(g) = &self.gpu {
            g.window.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.reconfigure(),
            WindowEvent::RedrawRequested => self.frame(),
            WindowEvent::CursorMoved { position, .. } => {
                let p = [position.x as f32, position.y as f32];
                if let Some(c) = &mut self.colony {
                    if let Some(d) = self.drag {
                        c.view.camera.turn(p[0] - d[0], p[1] - d[1]);
                        self.drag = Some(p);
                    }
                    if let Some(d) = self.left_drag {
                        c.view.camera.pan(d[0] - p[0], d[1] - p[1]);
                        self.left_drag = Some(p);
                    }
                } else if let (Some(d), Some(v)) = (self.drag, &mut self.view) {
                    v.camera.pan(d, p);
                    self.drag = Some(p);
                }
                self.cursor = p;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 60.0,
                };
                if let Some(c) = &mut self.colony {
                    c.view.camera.zoom_steps(steps);
                } else if let Some(v) = &mut self.view {
                    v.camera.zoom_steps(steps);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match (button, state) {
                (MouseButton::Left, ElementState::Pressed) if self.colony.is_some() => self.left_drag = Some(self.cursor),
                (MouseButton::Left, ElementState::Released) => self.left_drag = None,
                (MouseButton::Left, ElementState::Pressed) => {
                    if let Some(v) = &mut self.view {
                        v.selected = v.hover;
                    }
                }
                (MouseButton::Right, ElementState::Pressed) => self.drag = Some(self.cursor),
                (MouseButton::Right, ElementState::Released) => self.drag = None,
                _ => {}
            },
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => match event.logical_key.as_ref() {
                Key::Named(NamedKey::Escape) if self.overlay.is_some() => self.overlay = None,
                Key::Named(NamedKey::Escape) if self.colony.is_some() => self.toggle_colony(),
                Key::Character("i" | "I") => {
                    self.overlay = if self.overlay == Some(crate::ui::Screen::Info) { None } else { Some(crate::ui::Screen::Info) };
                }
                Key::Character("m" | "M") => {
                    self.overlay = if self.overlay == Some(crate::ui::Screen::Messages) { None } else { Some(crate::ui::Screen::Messages) };
                }
                Key::Named(NamedKey::Tab) => {
                    if let Some(h) = &mut self.hud {
                        h.chart = (h.chart + 1) % 10;
                    }
                }
                Key::Named(NamedKey::Escape) => el.exit(),
                Key::Character("c" | "C") => self.toggle_colony(),
                Key::Named(NamedKey::Space) => self.paused = !self.paused,
                Key::Character("1") => self.world.set_speed(Speed::Normal),
                Key::Character("2") => self.world.set_speed(Speed::Fast),
                Key::Character("3") => self.world.set_speed(Speed::Fastest),
                Key::Character("q" | "Q" | "e" | "E") => {
                    if let Some(v) = &mut self.view {
                        // Five keyboard frames per key event (the original turns per 10 ms frame while held).
                        v.camera.turn(if matches!(event.logical_key.as_ref(), Key::Character("q" | "Q")) { -5.0 } else { 5.0 });
                    }
                }
                Key::Named(NamedKey::PageUp | NamedKey::PageDown) => {
                    if let Some(v) = &mut self.view {
                        v.camera.tilt(if event.logical_key == Key::Named(NamedKey::PageUp) { 5.0 } else { -5.0 });
                    }
                }
                Key::Character("r" | "R") => {
                    if let Some(v) = &mut self.view {
                        v.reveal = !v.reveal;
                    }
                }
                Key::Character("v" | "V") => {
                    self.vsync = !self.vsync;
                    self.reconfigure();
                }
                _ => {}
            },
            _ => {}
        }
    }
}
