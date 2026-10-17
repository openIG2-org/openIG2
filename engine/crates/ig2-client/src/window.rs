//! The window and frame loop (ADR 0006 §1–§2): winit + a wgpu surface, the sim ticked by the original's
//! `TickClock` (20 Hz, ≤ 500 ms catch-up) and every frame drawn at the display's refresh rate, interpolated.
//!
//! Starmap: wheel = zoom, right drag = pan, Q/E = turn, PgUp/PgDn = tilt (the original's keyboard steps), left click =
//! select (again: the next unit under the cursor), right click = the selected fleet's order (Shift: waypoint, Ctrl:
//! attack, Alt: a point), a minimap click moves the view, R = reveal every fleet. C = the colony screen of the selected
//! planet (else the human's first one) and back.
//! Colony: wheel = zoom, left drag = pan, right drag = turn and tilt (the original's both-buttons drag); the building
//! screen's hot-spots (`crate::colony_ui`: select, repair/demolish windows, the build menu's pick list held on the
//! planet panel, the build cursor, the radar), E = a building's priority switch, Shift+Delete = demolish, PgUp/PgDn =
//! the previous/next own colony.
//! I = the info screen (the mouse over a race column picks the empire, the chart buttons pick the chart, Tab = the
//! next chart), M = the messages screen (a click on an inbox row opens the message, on an answer answers it; a click
//! in the text types it out; a click elsewhere leaves), S = the spy screen (left button on its buttons, lists and
//! spies; ←/→ = the next/previous spy; re/screens/spy.md).
//! D = the diplomacy screen (re/screens/diplomacy.md: a click on a head picks the empire, its menus by mouse).
//! K or the top bar's research button = the research screen (re/screens/research.md: its buttons, lists and models
//! by mouse; arrows = fields, PgUp/PgDn = categories, Enter = start/stop, D = urgent).
//! The info box's buttons open the production view and the transfer panel (re/screens/starmap.md §12–§13); a right
//! click on the box splits the selected fleet (the transfer between the two opens) or shows the planet's colony; T =
//! the transfer between the selected fleet and the player's nearest one (the radial menu's icon, not drawn yet).
//! Always: the top bar's speed buttons or 1/2/3 = speed, space = pause (lockstep commands, re/screens/starmap.md §10),
//! V = vsync on/off, Esc = back to the starmap / quit.

use std::sync::Arc;
use std::time::Instant;

use ig2_net::{Command, StarmapCmd};
use ig2_sim::game::{LocalView, OrderEvent, StarmapOrder};
use ig2_sim::time::{TickClock, SIM_TICK_MS};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::orders::{self, Keys, Selection};
use crate::render::Renderer;
use crate::starmap::{Assets, Pick, View};
use crate::starmap_panels::{Action, PanelModels, Panels};
use crate::world::World;

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    assets: Assets,
}

/// The open colony screen: its scene on the GPU, its view, the input state and the panel's turning building.
struct Colony {
    scene: crate::colony::ColonyScene,
    view: crate::colony::View,
    ui: crate::colony_ui::ColonyUi,
    panel: crate::colony_ui::PanelBuilding,
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
    /// The spy screen, if it is up.
    spy: Option<crate::spy_screen::SpyView>,
    /// The message line's 10 ms UI frames done so far.
    ui_frames: u64,
    /// The diplomacy screen while it is up, and this frame's pointer edges on it.
    diplomacy: Option<crate::diplomacy::Open>,
    dip_input: crate::diplomacy::Input,
    gpu: Option<Gpu>,
    view: Option<View>,
    clock: TickClock,
    start: Instant,
    vsync: bool,
    cursor: [f32; 2],
    drag: Option<[f32; 2]>,
    /// Where the left and right buttons went down on the starmap (a click is a press and release in place).
    left_down: Option<[f32; 2]>,
    right_down: Option<[f32; 2]>,
    modifiers: ModifiersState,
    title_at: Instant,
    frames: u32,
    /// The production view and transfer panel, their models, and a button held on them (left?).
    panels: Panels,
    panel_models: Option<PanelModels>,
    panel_held: Option<bool>,
    /// The info and messages screens' 3D frames.
    frame_models: Option<crate::screen_frames::FrameModels>,
    /// `TargetXStart/YStart/Zoom` with the frames left (`Mozgasd`), and the view `StarmapBeZoomol` remembered.
    cam_tween: Option<([f32; 3], i32)>,
    cam_saved: Option<[f32; 3]>,
    /// The research screen (re/screens/research.md): its state, models and the techs' info texts.
    research: Option<crate::research::Research>,
    research_models: Option<crate::research::ResearchModels>,
    tech_infos: Vec<[Vec<u8>; 3]>,
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
        spy: None,
        ui_frames: 0,
        diplomacy: None,
        dip_input: Default::default(),
        gpu: None,
        view: None,
        clock: TickClock::new(SIM_TICK_MS, 0),
        start,
        vsync: true,
        cursor: [0.0; 2],
        drag: None,
        left_down: None,
        right_down: None,
        modifiers: ModifiersState::empty(),
        title_at: start,
        frames: 0,
        panels: Panels::default(),
        panel_models: None,
        panel_held: None,
        frame_models: None,
        cam_tween: None,
        cam_saved: None,
        research: None,
        research_models: None,
        tech_infos: Vec::new(),
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
        self.panel_models = Some(PanelModels::new(&mut renderer));
        self.frame_models = Some(crate::screen_frames::FrameModels::new(&mut renderer));
        let scale = window.scale_factor() as f32;
        self.view = Some(View::new(&self.world.curr, &self.world.view, (config.width, config.height), scale));
        let sel = self.selection();
        if sel.mode == 1 || sel.mode == 2 {
            self.select(sel);
        }
        let hud = crate::ui::Hud::load(&mut renderer, &self.install, self.tables);
        // The messages quote the empires by `BirodalomNev` (the HUD's table).
        self.world.game.empire_names = hud.empire_names.clone();
        self.hud = Some(hud);
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
        self.open_colony(p);
    }

    /// Planet `p`'s colony screen (`Colony_Belep`).
    fn open_colony(&mut self, p: usize) {
        let Some(g) = &mut self.gpu else { return };
        self.world.show_colony(Some(p));
        let Some(snap) = self.world.colony_curr.clone() else { return };
        match crate::colony::ColonyScene::load(&mut g.renderer, &self.install, self.tables, self.maps, &snap) {
            Ok(scene) => {
                let view = crate::colony::View::new(&snap, (g.config.width, g.config.height), g.window.scale_factor() as f32, &scene);
                let own = snap.owner == self.world.curr.human && !snap.at_war;
                let ui = crate::colony_ui::ColonyUi::enter(&snap, own, &self.world.game.state);
                self.colony = Some(Colony { scene, view, ui, panel: Default::default() });
            }
            Err(e) => {
                eprintln!("ig2-client: colony {p}: {e}");
                self.world.show_colony(None);
            }
        }
    }

    /// The colony screen's input with its context; the outcome carried out.
    fn colony_input(&mut self, f: impl FnOnce(&mut crate::colony_ui::ColonyUi, &crate::colony_ui::Ctx, [f32; 2], u32) -> crate::colony_ui::Outcome) {
        let now = self.now_ms();
        let (Some(c), Some(h), Some(snap)) = (&mut self.colony, &self.hud, &self.world.colony_curr) else { return };
        let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
        let ctx = crate::colony_ui::Ctx { state: &self.world.game.state, snap, rules: self.tables, sim: self.world.game.rules, texts: &h.texts, tech_name: &|t| h.tech_name(t) };
        let out = f(&mut c.ui, &ctx, at, now);
        if out.pan {
            self.left_drag = Some(self.cursor);
        }
        for cmd in out.commands {
            self.world.send(cmd);
        }
        if let Some(e) = out.error {
            self.world.local_lines.push(e);
        }
        if out.starmap {
            self.toggle_colony();
        } else if let Some(p) = out.colony {
            self.colony = None;
            self.open_colony(p);
        }
    }

    /// Open the spy screen (`AktKepKell = 7`) or close it.
    fn toggle_spy(&mut self) {
        if self.spy.take().is_some() {
            return;
        }
        let (Some(g), Some(h)) = (&mut self.gpu, &self.hud) else { return };
        let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
        match crate::spy_screen::SpyView::open(&mut g.renderer, &self.install, h, &d, true) {
            Ok(v) => self.spy = Some(v),
            Err(e) => eprintln!("ig2-client: spy screen: {e}"),
        }
    }

    /// The pointer on the spy screen's 640 × 480 virtual screen.
    fn spy_virtual(&self, p: [f32; 2]) -> Option<[f32; 2]> {
        let g = self.gpu.as_ref()?;
        self.spy.as_ref()?;
        let v = crate::font::Virtual::new([g.config.width as f32, g.config.height as f32]);
        Some([(p[0] - v.x0) / v.k, p[1] / v.k])
    }

    /// A starmap order or setting of the local player (`Multi_Send(.., EN, ..)`), applied in the next frame.
    /// Open or close a 2D screen (`I` info, `M` messages): `Info_Belep` starts on the human's empire and chart 0.
    fn toggle_screen(&mut self, s: crate::ui::Screen) {
        self.overlay = if self.overlay == Some(s) { None } else { Some(s) };
        if let (Some(crate::ui::Screen::Info), Some(h)) = (self.overlay, &mut self.hud) {
            h.info = crate::ui::InfoScreen::enter(self.world.ui.human);
        }
    }

    /// A left press on the info or messages screen (`Info_Main`, `Messages_Main`).
    fn screen_press(&mut self, screen: crate::ui::Screen) {
        let at = self.virtual_cursor();
        let Some(h) = &mut self.hud else { return };
        match screen {
            crate::ui::Screen::Info => {
                h.info.press(crate::info_screen::spot_at(&self.tables.race_name_x, at));
            }
            crate::ui::Screen::Messages => match h.messages.press(at, self.world.ui.inbox.len()) {
                crate::messages_screen::Press::Open(i) => h.open_message(&self.install, &self.world.ui, i),
                crate::messages_screen::Press::Leave => self.overlay = None,
                crate::messages_screen::Press::None => {}
            },
            _ => {}
        }
    }

    fn send(&mut self, o: StarmapOrder) {
        self.world.send(Command::Starmap { order: StarmapCmd::from(o) });
    }

    /// The top bar's pause button (hot-spot 2, `IG2_gameUpdate`): SP toggles `IdoPause` unless the speed is locked.
    fn toggle_pause(&mut self) {
        if !self.speed_locked() {
            let paused = self.world.paused();
            self.send(StarmapOrder::Pause { paused: !paused });
        }
    }

    /// A speed button (hot-spots 3..=5): `SetIdoSpeed(n)` and unpause, when paused or at another speed.
    fn set_speed(&mut self, level: i32) {
        if !self.speed_locked() && (self.world.paused() || self.world.game.speed_level() != level) {
            self.send(StarmapOrder::Speed { level });
            self.send(StarmapOrder::Pause { paused: false });
        }
    }

    /// The top bar's research button (1005) or K: the research screen opens (`AktKepKell = 4`, `Research_Belep`)
    /// or, when up, closes.
    fn toggle_research(&mut self) {
        if self.overlay == Some(crate::ui::Screen::Research) {
            self.overlay = None;
            self.research = None;
            return;
        }
        if self.tech_infos.is_empty() {
            self.tech_infos = crate::research::tech_infos(&self.install);
        }
        self.research = Some(crate::research::Research::open(&self.world.game.state, self.tables));
        self.overlay = Some(crate::ui::Screen::Research);
    }

    /// The pointer on the top bar's research button (`HACK_SetBrushok`'s 1005 at (600, 0), 40 × 20).
    fn top_bar_research(&self) -> bool {
        let at = self.virtual_cursor();
        at[1] < 20.0 && (600.0..640.0).contains(&at[0])
    }

    /// A press on the research screen (`Research_Main`, the lists, the models).
    fn research_press(&mut self) {
        let at = self.virtual_cursor();
        let (Some(r), Some(m)) = (&mut self.research, &self.research_models) else { return };
        for a in r.press(&self.world.game.state, self.tables, &m.mask, at, &m.centres) {
            let crate::research::Act::Send(c) = a;
            self.world.send(c);
        }
    }

    fn research_key(&mut self, k: crate::research::Key) {
        let Some(r) = &mut self.research else { return };
        for a in r.key(&self.world.game.state, k) {
            let crate::research::Act::Send(c) = a;
            self.world.send(c);
        }
    }

    fn speed_locked(&self) -> bool {
        self.world.game.state.mem.i32(ig2_sim::save::va::SPEED_LOCKED) != 0
    }

    /// The local selection (`StarmapMode`, `AktPlanet`, `AktFleet`).
    fn selection(&self) -> Selection {
        let v = self.world.game.local_view();
        Selection { mode: v.starmap_mode, planet: v.planet.max(0) as usize, fleet: v.fleet.max(0) as usize }
    }

    fn select(&mut self, s: Selection) {
        let v = self.world.game.local_view();
        self.world.game.set_view(LocalView { starmap_mode: s.mode, planet: s.planet as i32, fleet: s.fleet as i32, ..v });
        if let Some(view) = &mut self.view {
            view.selected = Some(if s.mode == 2 { Pick::Fleet(s.fleet) } else { Pick::Planet(s.planet) });
        }
    }

    /// Carry out what the panels asked for.
    fn apply(&mut self, actions: Vec<Action>) {
        for a in actions {
            match a {
                Action::Send(o) => self.send(o),
                Action::Select(s) => self.select(s),
                Action::ZoomIn { x, y, zoom } => {
                    if let (Some(v), None) = (&self.view, self.cam_saved) {
                        self.cam_saved = Some([v.camera.x, v.camera.y, v.camera.zoom]);
                    }
                    self.cam_tween = Some(([x, -y, zoom], 8));
                }
                Action::ZoomOut => {
                    if let Some(s) = self.cam_saved.take() {
                        self.cam_tween = Some((s, 8));
                    }
                }
                Action::Centre { x, y } => {
                    if let Some(v) = &self.view {
                        self.cam_tween = Some(([x, -y, v.camera.zoom], 8));
                    }
                }
            }
        }
    }

    /// The pointer in virtual pixels.
    fn virtual_cursor(&self) -> [f32; 2] {
        self.view.as_ref().map_or([0.0; 2], |v| v.camera.virtual_of(self.cursor))
    }

    /// A press on the starmap: the panels' hot-spots take it first.
    fn panel_press(&mut self, left: bool) -> bool {
        let at = self.virtual_cursor();
        let sel = self.selection();
        match self.panels.press(&self.world.game.state, &sel, at, left) {
            Some(actions) => {
                self.apply(actions);
                true
            }
            None => false,
        }
    }

    /// The info box (hot-spot 8) clicked with the right button: the planet's colony screen, or the fleet splits
    /// (`Multi_Send(0x14)`, after closing a transfer panel; a fleet-to-fleet panel closing doesn't split).
    fn info_box_click(&mut self) {
        let sel = self.selection();
        if sel.mode == 1 {
            self.toggle_colony();
            return;
        }
        let state = &self.world.game.state;
        let Some(r) = state.fleets.get(sel.fleet) else { return };
        let f = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let (mine, calm, pos) = (r[0x15] == self.world.curr.human, r[0x52] & 1 == 0, (f(0x30), f(0x34)));
        let closed = self.panels.transfer;
        let acts = self.panels.close_transfer(state, &sel, true);
        self.apply(acts);
        if mine && calm && closed.is_none_or(|t| t.tank_mode) {
            if closed.is_none() {
                self.apply(vec![Action::ZoomIn { x: pos.0, y: pos.1, zoom: 590.0 }]);
            }
            self.send(StarmapOrder::Split { fleet: sel.fleet as i32 });
        }
    }

    /// A click on the starmap screen at physical `p` (left: buttons, minimap, selection; right: an order).
    fn starmap_click(&mut self, p: [f32; 2], right: bool) {
        let Some(v) = &self.view else { return };
        let at = v.camera.virtual_of(p);
        let keys = Keys { shift: self.modifiers.shift_key(), ctrl: self.modifiers.control_key(), alt: self.modifiers.alt_key() };
        let sel = self.selection();
        // The info box (hot-spot 8 at (0, 424, 166, 56)).
        if at[0] >= 0.0 && at[0] < 166.0 && at[1] >= 424.0 {
            if right {
                self.info_box_click();
            }
            return;
        }
        // The transfer panel covers the map (hot-spot 0x58).
        if self.panels.transfer.is_some() && at[1] >= 20.0 {
            return;
        }
        if right {
            if let Some(o) = orders::right_click(&self.world.curr, &v.camera, at, &sel, keys) {
                self.send(o);
            }
            return;
        }
        if at[1] < 20.0 {
            // The top bar's buttons (`HACK_SetBrushok`'s speed records 1012..=1015: pause, ×1, ×2, ×3).
            match at[0] {
                x if (0.0..15.0).contains(&x) => self.toggle_pause(),
                x if (15.0..29.0).contains(&x) => self.set_speed(1),
                x if (29.0..50.0).contains(&x) => self.set_speed(2),
                x if (50.0..73.0).contains(&x) => self.set_speed(3),
                _ => {}
            }
            return;
        }
        let m = orders::MINIMAP_SPOT;
        // The panels turn the minimap off.
        if !self.panels.active() && at[0] >= m[0] && at[0] < m[2] && at[1] >= m[1] && at[1] < m[3] {
            let t = orders::minimap_target(at);
            if let Some(v) = &mut self.view {
                (v.camera.x, v.camera.y) = (t[0], t[1]);
            }
            return;
        }
        if let Some(s) = orders::left_click(&self.world.curr, &v.camera, at, &sel) {
            // The production view picks planets only (`SelectNearestPlanet`).
            if s.mode == 1 || !self.panels.production_open() {
                self.select(s);
            }
        }
    }

    fn frame(&mut self) {
        let now = self.now_ms();
        // The frame's message pass, then the ticks due (none while `IdoPause`).
        self.world.pump();
        if let Some(c) = &mut self.colony {
            let p = self.world.colony.unwrap_or(0);
            c.ui.events(&std::mem::take(&mut self.world.colony_events), self.world.curr.human, p);
            // A building came or went: `Init3dColony`'s building loop again.
            if let (Some(g), Some(s)) = (&mut self.gpu, &self.world.colony_curr) {
                if c.scene.stale(s) {
                    match crate::colony::ColonyScene::load(&mut g.renderer, &self.install, self.tables, self.maps, s) {
                        Ok(scene) => c.scene = scene,
                        Err(e) => eprintln!("ig2-client: colony: {e}"),
                    }
                }
            }
        } else {
            self.world.colony_events.clear();
        }
        for e in std::mem::take(&mut self.world.order_events) {
            if let OrderEvent::Split { empire, fleet, new } = e {
                if empire == self.world.curr.human {
                    // `MULTIGET_Starmap`: the new fleet is selected and the transfer between the two opens, before the
                    // frame's ticks (their 0x1b spares the panel's fleets).
                    self.select(Selection { mode: 2, fleet: new, ..self.selection() });
                    self.panels.open_transfer(fleet as i32, new as i32, false);
                }
            }
        }
        self.world.game.set_transfer(self.panels.transfer.map(|t| (t.f1, t.f2)));
        let due = self.clock.advance(now);
        if !self.world.paused() {
            for _ in 0..due {
                self.world.tick();
            }
        }
        if let Some(s) = self.world.fix_selection() {
            self.select(s);
        }
        let sel = self.selection();
        let acts = self.panels.advance(&self.world.game.state, &sel, now as f32 / 1000.0);
        self.apply(acts);
        // The panel's fleets are spared `FRISSIT_Starmap`'s 0x1b while it is up.
        self.world.game.set_transfer(self.panels.transfer.map(|t| (t.f1, t.f2)));
        // `Mozgasd`: the camera halves its way to the target for 8 frames.
        if let (Some((t, n)), Some(v)) = (self.cam_tween, &mut self.view) {
            let c = &mut v.camera;
            if n > 1 {
                c.x += (t[0] - c.x) / 2.0;
                c.y += (t[1] - c.y) / 2.0;
                c.zoom += (t[2] - c.zoom) / 2.0;
                self.cam_tween = Some((t, n - 1));
            } else {
                (c.x, c.y, c.zoom) = (t[0], t[1], t[2]);
                self.cam_tween = None;
            }
        }
        // The message line runs on the 100 Hz UI clock (`FRISSIT_Scroll2`), also while paused.
        let due_ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50);
        self.ui_frames = now as u64 / 10;
        if let Some(h) = &mut self.hud {
            let world = &mut self.world;
            for _ in 0..due_ui {
                h.scroller.step(&mut || world.pop_message());
                h.info.ui_frame();
                h.messages.ui_frame();
            }
            if let Some(d) = &mut h.messages.open {
                d.advance(due_ui as f32 * 10.0);
            }
        }
        if let (Some(c), Some(s)) = (&mut self.colony, &self.world.colony_curr) {
            for _ in 0..due_ui {
                c.ui.ui_frame(s, &mut c.view.camera);
            }
            c.view.overlay = c.ui.overlay();
        }
        if let Some(r) = &mut self.research {
            for _ in 0..due_ui {
                r.step(&self.world.game.state);
            }
        }
        // A button held on a panel repeats on the 100 Hz UI frames (`Starmap_Main::KezdoIdo`).
        if self.panel_held.is_some() {
            let at = self.virtual_cursor();
            for _ in 0..due_ui {
                let acts = self.panels.hold_frame(&self.world.game.state, &sel, at);
                self.apply(acts);
            }
        }
        let paused = self.world.paused();
        let alpha = if paused { 1.0 } else { self.clock.phase(now) };
        let (Some(g), Some(v)) = (&mut self.gpu, &mut self.view) else { return };
        let t = now as f32 / 1000.0;
        let size = [g.config.width as f32, g.config.height as f32];
        let panel_tex = match (&mut self.panel_models, self.colony.is_none() && self.overlay.is_none() && self.panels.active()) {
            (Some(m), true) => m.render(&mut g.renderer, &self.install, self.tables, &self.panels, &self.world.game.state, &sel, size),
            _ => None,
        };
        if let (Some(spy), Some(h)) = (&mut self.spy, &mut self.hud) {
            let en = self.world.curr.human as usize;
            let d = crate::spy_screen::SpyData::read(&self.world.game.state, en, self.world.game.rules, self.tables);
            spy.advance(&mut g.renderer, &self.install, &d, due_ui, t);
            h.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
            h.paused = paused;
            let mut frame = spy.build(h, &d, size, self.install.lang() == ig2_formats::text::Lang::Hu);
            h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Spy, t);
            if let Ok(out) = g.surface.get_current_texture() {
                let view = out.texture.create_view(&Default::default());
                g.renderer.draw(&frame, &view, (g.config.width, g.config.height), t);
                g.window.pre_present_notify();
                out.present();
            }
            return;
        }
        if let (Some(s), Some(h), Some(m)) = (self.overlay, &mut self.hud, &mut self.frame_models) {
            h.frame_tex = match s {
                crate::ui::Screen::Info => m.render_info(&mut g.renderer, &self.install, &h.info.frame_view(&self.world.ui), size),
                crate::ui::Screen::Messages => m.render_messages(&mut g.renderer, &self.install, if h.messages.open.is_some() { 30 } else { 15 }, size),
                _ => None,
            };
        }
        let mut colony_panel = None;
        // The diplomacy screen: its UI ticks, the frame's input (its commands go to the queue), then its frame.
        if let (Some(d), Some(h), Some(crate::ui::Screen::Diplomacy)) = (&mut self.diplomacy, &self.hud, self.overlay) {
            for _ in 0..due_ui {
                d.screen.ui_tick();
            }
            let vv = crate::font::Virtual::new(size);
            self.dip_input.at = [(self.cursor[0] - vv.x0) / vv.k, self.cursor[1] / vv.k];
            for c in d.input(&self.world.game.state, h, self.tables, &self.dip_input) {
                self.world.send(c);
            }
            self.dip_input.pressed = false;
            self.dip_input.released = false;
        }
        let research = match (&self.research, self.overlay) {
            (Some(r), Some(crate::ui::Screen::Research)) => {
                let m = self.research_models.get_or_insert_with(|| crate::research::ResearchModels::new(&mut g.renderer, &self.install));
                Some(m.render(&mut g.renderer, &self.install, self.tables, r, &self.world.game.state, size))
            }
            _ => None,
        };
        let (mut frame, screen) = match (self.overlay, &mut self.colony, &self.world.colony_prev, &self.world.colony_curr) {
            (Some(crate::ui::Screen::Diplomacy), ..) => {
                let mut f = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
                if let (Some(d), Some(h)) = (&mut self.diplomacy, &self.hud) {
                    d.draw(&mut f, &mut g.renderer, &self.install, self.tables, h, &self.world.game.state, size, t);
                }
                (f, crate::ui::Screen::Diplomacy)
            }
            (Some(s), ..) => (crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]), s),
            (None, Some(c), Some(prev), Some(curr)) => {
                colony_panel = c.panel.render(&mut g.renderer, &self.install, self.tables, c.ui.kind, c.ui.spin(), size);
                (c.view.build(&mut g.renderer, &self.install, self.tables, self.maps, &mut c.scene, prev, curr, alpha, t), crate::ui::Screen::Colony)
            }
            _ => {
                v.hover = v.pick(&self.world.prev, &self.world.curr, alpha, self.cursor);
                (v.build(&g.assets, &self.world.prev, &self.world.curr, alpha, t), crate::ui::Screen::Starmap)
            }
        };
        if let Some(h) = &mut self.hud {
            h.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
            h.paused = paused;
            match (screen, &self.colony, &self.world.colony_curr) {
                (crate::ui::Screen::Colony, Some(c), Some(curr)) => {
                    let cursor = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
                    let input = crate::ui::ColonyInput { ui: &c.ui, sim: self.world.game.rules, panel: colony_panel, cursor, now_ms: now, time: t };
                    h.draw_colony(&mut frame, &self.world.game.state, self.tables, self.maps, &c.view, &c.scene, curr, &input);
                }
                (crate::ui::Screen::Starmap, ..) => {
                    h.draw_starmap_names(&mut frame, v, &g.assets, &self.world.prev, &self.world.curr, alpha, t);
                    let lv = self.world.game.local_view();
                    let sel = Selection { mode: lv.starmap_mode, planet: lv.planet.max(0) as usize, fleet: lv.fleet.max(0) as usize };
                    h.draw_starmap_hud(&mut frame, &self.world.game.state, self.tables, v, &self.world.prev, &self.world.curr, &sel, alpha, t);
                    h.draw_starmap_panels(&mut frame, &self.panels, &self.world.game.state, &sel, panel_tex, size, t);
                }
                (crate::ui::Screen::Research, ..) => {
                    if let (Some(r), Some((tex, lines))) = (&self.research, research) {
                        h.draw_research(&mut frame, r, &self.tech_infos, &self.world.game.state, self.tables, tex, lines, size, t);
                    }
                }
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
            let state = if paused { "paused".to_string() } else { format!("×{speed}") };
            g.window.set_title(&format!("Imperium Galactica II — {y}-{m:02}-{d:02} {state} — {fps:.0} fps"));
            self.title_at = Instant::now();
            self.frames = 0;
        }
    }
}

/// `ResearchKeyboardHandler`'s keys.
fn research_key(k: NamedKey) -> Option<crate::research::Key> {
    use crate::research::Key as K;
    Some(match k {
        NamedKey::ArrowDown => K::Down,
        NamedKey::ArrowUp => K::Up,
        NamedKey::ArrowRight => K::Right,
        NamedKey::ArrowLeft => K::Left,
        NamedKey::Enter => K::Enter,
        NamedKey::PageUp => K::PageUp,
        NamedKey::PageDown => K::PageDown,
        _ => return None,
    })
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
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let p = [position.x as f32, position.y as f32];
                if let Some(at) = self.spy_virtual(p) {
                    let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
                    if let Some(spy) = &mut self.spy {
                        spy.hover(&d, at);
                    }
                }
                if let Some(v) = &self.view {
                    let at = v.camera.virtual_of(p);
                    self.panels.hover(at);
                    if let Some(h) = &mut self.hud {
                        match self.overlay {
                            Some(crate::ui::Screen::Info) => {
                                h.info.hover(crate::info_screen::spot_at(&self.tables.race_name_x, at));
                            }
                            Some(crate::ui::Screen::Messages) => h.messages.hover(at),
                            _ => {}
                        }
                    }
                }
                if self.colony.is_some() {
                    self.cursor = p;
                    let gm_cell = self.colony.as_ref().zip(self.world.colony_curr.as_ref()).and_then(|(c, s)| self.maps.get(s.map).and_then(|gm| crate::colony_ui::cell_under(&c.view, gm, p)));
                    self.colony_input(|ui, ctx, at, _| {
                        ui.drag(ctx, at);
                        ui.hover(ctx, gm_cell);
                        Default::default()
                    });
                }
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
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.spy.is_some() => {
                let Some(at) = self.spy_virtual(self.cursor) else { return };
                let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
                let Some(spy) = &mut self.spy else { return };
                let orders = if state == ElementState::Pressed { spy.press(&d, at) } else { spy.release(&d) };
                for c in orders {
                    self.world.send(c);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let on_starmap = self.colony.is_none() && self.overlay.is_none();
                let moved = |d: Option<[f32; 2]>, p: [f32; 2]| d.is_none_or(|d| (d[0] - p[0]).abs() + (d[1] - p[1]).abs() > 4.0);
                match (button, state) {
                    (MouseButton::Left, ElementState::Pressed) if self.top_bar_research() => self.toggle_research(),
                    (MouseButton::Left, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Research) => self.research_press(),
                    (MouseButton::Left, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Diplomacy) => {
                        self.dip_input.pressed = true;
                        self.dip_input.held = true;
                    }
                    (MouseButton::Left, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Diplomacy) => {
                        self.dip_input.released = true;
                        self.dip_input.held = false;
                    }
                    (MouseButton::Left, ElementState::Pressed) if self.overlay.is_some() => {
                        let s = self.overlay.unwrap();
                        self.screen_press(s);
                    }
                    (MouseButton::Left, ElementState::Pressed) if self.colony.is_some() => self.colony_input(|ui, ctx, at, now| ui.press(ctx, at, true, now)),
                    (MouseButton::Left, ElementState::Released) if self.colony.is_some() => {
                        self.left_drag = None;
                        self.colony_input(|ui, ctx, at, now| ui.release(ctx, at, true, now));
                    }
                    (MouseButton::Left, ElementState::Pressed) => {
                        if on_starmap && self.panel_press(true) {
                            self.panel_held = Some(true);
                        } else {
                            self.left_down = Some(self.cursor);
                        }
                    }
                    (MouseButton::Left, ElementState::Released) if self.panel_held.is_some() => {
                        self.panel_held = None;
                        let sel = self.selection();
                        let acts = self.panels.release(&self.world.game.state, &sel);
                        self.apply(acts);
                    }
                    (MouseButton::Left, ElementState::Released) => {
                        self.left_drag = None;
                        let down = self.left_down.take();
                        if on_starmap && !moved(down, self.cursor) {
                            self.starmap_click(self.cursor, false);
                        }
                    }
                    (MouseButton::Right, ElementState::Pressed) if on_starmap && self.panel_press(false) => self.panel_held = Some(false),
                    (MouseButton::Right, ElementState::Pressed) => {
                        self.drag = Some(self.cursor);
                        self.right_down = Some(self.cursor);
                        if self.colony.is_some() {
                            self.colony_input(|ui, ctx, at, now| ui.press(ctx, at, false, now));
                        }
                    }
                    (MouseButton::Right, ElementState::Released) if self.panel_held.is_some() => {
                        self.panel_held = None;
                        let sel = self.selection();
                        let acts = self.panels.release(&self.world.game.state, &sel);
                        self.apply(acts);
                    }
                    (MouseButton::Right, ElementState::Released) => {
                        self.drag = None;
                        let down = self.right_down.take();
                        if self.colony.is_some() && !moved(down, self.cursor) {
                            self.colony_input(|ui, ctx, at, now| ui.release(ctx, at, false, now));
                        }
                        if on_starmap && !moved(down, self.cursor) {
                            self.starmap_click(self.cursor, true);
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => match event.logical_key.as_ref() {
                Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Research) => self.toggle_research(),
                Key::Named(NamedKey::Escape) if self.spy.is_some() => self.spy = None,
                Key::Named(NamedKey::Escape) if self.overlay.is_some() => self.overlay = None,
                Key::Character("s" | "S") => self.toggle_spy(),
                Key::Named(dir @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if self.spy.is_some() => {
                    let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
                    if let Some(spy) = &mut self.spy {
                        spy.key(&d, dir == NamedKey::ArrowLeft);
                    }
                }
                Key::Named(k) if self.overlay == Some(crate::ui::Screen::Research) && research_key(k).is_some() => {
                    if let Some(k) = research_key(k) {
                        self.research_key(k);
                    }
                }
                Key::Character("d" | "D") if self.overlay == Some(crate::ui::Screen::Research) => self.research_key(crate::research::Key::Double),
                Key::Character("k" | "K") => self.toggle_research(),
                Key::Named(NamedKey::Escape) if self.colony.is_some() => self.toggle_colony(),
                Key::Character("e" | "E") if self.colony.is_some() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Priority)),
                Key::Named(NamedKey::Delete) if self.colony.is_some() && self.modifiers.shift_key() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Demolish)),
                Key::Named(NamedKey::PageUp) if self.colony.is_some() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Prev)),
                Key::Named(NamedKey::PageDown) if self.colony.is_some() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Next)),
                Key::Character("i" | "I") => self.toggle_screen(crate::ui::Screen::Info),
                Key::Character("m" | "M") => self.toggle_screen(crate::ui::Screen::Messages),
                Key::Character("d" | "D") => {
                    if self.overlay == Some(crate::ui::Screen::Diplomacy) {
                        self.overlay = None;
                    } else if let (Some(g), Some(h)) = (&mut self.gpu, &self.hud) {
                        if self.diplomacy.is_none() {
                            self.diplomacy = Some(crate::diplomacy::Open::new(&mut g.renderer, &self.install, self.tables, &h.texts, &self.world.game.state));
                        }
                        self.overlay = Some(crate::ui::Screen::Diplomacy);
                    }
                }
                Key::Named(NamedKey::Tab) => {
                    if let Some(h) = &mut self.hud {
                        h.info.chart = (h.info.chart + 1) % 10;
                    }
                }
                Key::Named(NamedKey::Escape) => el.exit(),
                Key::Character("c" | "C") => self.toggle_colony(),
                Key::Character("t" | "T") if self.colony.is_none() => {
                    let sel = self.selection();
                    let acts = self.panels.fleet_transfer(&self.world.game.state, &sel);
                    self.apply(acts);
                }
                Key::Named(NamedKey::Space) => self.toggle_pause(),
                Key::Character("1") => self.set_speed(1),
                Key::Character("2") => self.set_speed(2),
                Key::Character("3") => self.set_speed(3),
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
