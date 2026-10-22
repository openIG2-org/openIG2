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
//! click on the box splits the selected fleet (the transfer between the two opens) or shows the planet's colony, a
//! left click on a planet's box its long info with the tax, auto-build and priority menus (§15–§16). A left press on
//! the map selects and opens the radial icon menu; releasing over an icon carries it out (§14).
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
use crate::starmap_menus::{Do, MenuKind, Menus};
use crate::starmap_panels::{Action, PanelModels, Panels};
use crate::world::World;

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    assets: Assets,
    /// Where a window narrower than 4:3 draws its band before it goes on the window.
    band_target: Option<crate::render::Target>,
}

impl Gpu {
    /// `ig2_resize` for the window.
    fn layout(&self) -> crate::kit::Layout {
        crate::kit::Layout::new(self.config.width as i32, self.config.height as i32)
    }

    /// The viewport the screens draw in (the whole window unless it is narrower than 4:3).
    fn band(&self) -> (u32, u32) {
        let [_, _, w, h] = self.layout().band();
        (w as u32, h as u32)
    }

    /// Draw the frame on the window: through the band's target when it is letterboxed top and bottom, with the
    /// side strips black under the "letterbox" option.
    fn present(&mut self, mut frame: crate::render::Frame, out: &wgpu::Texture, t: f32) {
        let l = self.layout();
        crate::kit::layout::letterbox_bars(&mut frame, &l, self.assets.white);
        let size = (self.config.width, self.config.height);
        let [x, y, w, h] = l.band();
        if [w, h] == [size.0 as i32, size.1 as i32] {
            self.renderer.draw(&frame, out, size, t);
            return;
        }
        let px = (w as u32, h as u32);
        let target = match &mut self.band_target {
            Some(tg) => tg,
            None => self.band_target.insert(self.renderer.target(px)),
        };
        self.renderer.draw_into(&frame, target, px, t);
        let mut f = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        let c = [x as f32 + w as f32 / 2.0, y as f32 + h as f32 / 2.0];
        f.push(target.tex, crate::render::Blend::Alpha, crate::render::Instance::new(crate::render::kind::SPRITE, c, [w as f32 / 2.0, h as f32 / 2.0], [1.0; 4]));
        self.renderer.draw(&f, out, size, t);
    }
}

/// The open colony screen: its scene on the GPU, its view, the input state and the panel's turning building.
struct Colony {
    scene: crate::colony::ColonyScene,
    view: crate::colony::View,
    ui: crate::colony_ui::ColonyUi,
    panel: crate::colony_ui::PanelBuilding,
}

/// The main menu while it is up (re/screens/mainscr.md): its state, its scene, the 10 ms UI clock and the pointer.
struct Front {
    s: crate::mainscr::MainScr,
    scene: Option<crate::mainscr_scene::MainScene>,
    ui_ms: u32,
    at: [f32; 2],
    quit: bool,
}

struct App {
    /// The main menu, until a ring starts the game.
    front: Option<Front>,
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
    /// The info screen's buttons held (`Mouse_Gomb`; `Info_Main`'s drags and select menus).
    info_down: bool,
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
    /// The radial menu, the planet's long info and the select menus (re/screens/starmap.md §14–§17).
    menus: Menus,
    /// The research screen (re/screens/research.md): its state, models and the techs' info texts.
    research: Option<crate::research::Research>,
    research_models: Option<crate::research::ResearchModels>,
    tech_infos: Vec<[Vec<u8>; 3]>,
    /// The research screen's tooltip texts by hot-spot (`Research_MouseInit`).
    research_tips: Vec<Vec<u8>>,
    /// Sound and music (re/screens/audio.md), and the screens seen up last frame (their enter/exit sounds).
    audio: crate::audio::Audio,
    sound_overlay: Option<crate::ui::Screen>,
    sound_spy: Option<bool>,
    /// The sounds the window's input code started this frame (re/screens/audio.md §6), played by `audio_frame`.
    sounds: Vec<crate::audio::SoundId>,
    /// A headless script run (`--snapshot --script` on the starmap or colony): its renderer, band and clock.
    headless: Option<Headless>,
}

/// The renderer of a run without a window, its band, and the script's clock (one 10 ms UI tick a frame; the sim
/// held, as the exe's script clock holds it: re/screens/sound.md §3).
struct Headless {
    renderer: Renderer,
    band: (u32, u32),
    ms: u32,
}

/// The renderer, the band and the scale factor, from the window or the headless run.
fn gfx<'a>(gpu: &'a mut Option<Gpu>, headless: &'a mut Option<Headless>) -> Option<(&'a mut Renderer, (u32, u32), f32)> {
    match (gpu, headless) {
        (Some(g), _) => {
            let (band, scale) = (g.band(), g.window.scale_factor() as f32);
            Some((&mut g.renderer, band, scale))
        }
        (None, Some(h)) => Some((&mut h.renderer, h.band, 1.0)),
        _ => None,
    }
}

pub fn run(o: crate::Options) -> Result<(), String> {
    let l = crate::load(&o)?;
    // The rules live for the whole process; `World` borrows them.
    let rules: &'static ig2_sim::game::GameRules = Box::leak(Box::new(l.rules));
    let tables: &'static ig2_rules::Rules = Box::leak(Box::new(l.tables));
    let mut world = World::load(&l.save, rules, &l.tech_defaults, &l.volatile);
    world.set_speed(o.speed);
    let exe = l.install.read_raw("ig2.exe").map_err(|e| e.to_string())?;
    let sounds = ig2_rules::sounds::SoundTables::from_exe(&exe).map_err(|e| e.to_string())?;
    let audio_install = ig2_formats::Install::open(l.install.root(), l.install.lang()).map_err(|e| e.to_string())?;
    let mut audio = crate::audio::Audio::new(audio_install, sounds, true);
    if !audio.has_device() {
        eprintln!("ig2-client: no sound device; running silent");
    }
    // The client starts on the main menu (`MainScr_Belep`'s `PlayMusicMenu(false, true)`), or with `--save` on the
    // starmap (`Starmap_Belep`'s `PlayMusicMain(0, 0)`); nothing plays yet: at once.
    let music = if o.main_menu { crate::audio::Music::Menu } else { crate::audio::Music::Main };
    audio.play_music(music, o.main_menu, human_race(&world.game.state));
    let mut app = App::new(world, l.install, tables, &rules.ground_maps, audio);
    app.front = o.main_menu.then(|| Front { s: crate::mainscr::MainScr::enter(), scene: None, ui_ms: 0, at: [320.0, 240.0], quit: false });
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app).map_err(|e| e.to_string())
}

/// `--snapshot OUT --script S` on the starmap (or with `--colony P` the colony screen): the window's own input and
/// frame code run headless on the script's clock (re/screens/sound.md §3: one 10 ms UI tick a frame, the sim held),
/// the script's input fed in as window events. Writes the sounds as `OUT-sounds.jsonl` (the tracer's `t`: the entry's
/// at 0, an event at t's sounds at t, tick k's own at k − 1); the grabs are not drawn.
pub fn script_run(o: &crate::Options, out: &std::path::Path, script: &crate::script::Script) -> Result<(), String> {
    use crate::script::Input;
    let l = crate::load(o)?;
    let rules: &'static ig2_sim::game::GameRules = Box::leak(Box::new(l.rules));
    let tables: &'static ig2_rules::Rules = Box::leak(Box::new(l.tables));
    let mut world = World::load(&l.save, rules, &l.tech_defaults, &l.volatile);
    world.set_speed(o.speed);
    let exe = l.install.read_raw("ig2.exe").map_err(|e| e.to_string())?;
    let sounds = ig2_rules::sounds::SoundTables::from_exe(&exe).map_err(|e| e.to_string())?;
    let audio_install = ig2_formats::Install::open(l.install.root(), l.install.lang()).map_err(|e| e.to_string())?;
    let audio = crate::audio::Audio::new(audio_install, sounds, false);
    let mut renderer = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let hud = crate::ui::Hud::load(&mut renderer, &l.install, tables);
    let mut app = App::new(world, l.install, tables, &rules.ground_maps, audio);
    app.world.game.empire_names = hud.empire_names.clone();
    app.hud = Some(hud);
    app.view = Some(View::new(&app.world.curr, &app.world.view, o.size, 1.0));
    let sel = app.selection();
    if sel.mode == 1 || sel.mode == 2 {
        app.select(sel);
    }
    app.headless = Some(Headless { renderer, band: o.size, ms: 0 });
    if let Some(p) = o.colony {
        app.open_colony(p);
        // The camera of the exe's state save (`--colony-live`, as `colony_snapshot`) or `--colony-cam`.
        let f = |n: &str| l.save.chunk(n).and_then(|b| b.get(..4)).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()));
        if let Some(c) = &mut app.colony {
            if let Some([x, y, zoom, pitch, rotate]) = o.colony_cam {
                c.view.camera = crate::colony::Camera { x, y, zoom, pitch, rotate, ..c.view.camera };
            } else if o.colony_live {
                c.view.camera = crate::colony::Camera { x: f("ColonyXStart"), y: f("ColonyYStart"), zoom: f("ColonyZoom"), pitch: f("ColonyPitch"), rotate: f("ColonyRotate"), ..c.view.camera };
            }
        }
    }
    // The screen's entry sounds: at t 0, as the tracer logs `Colony_Belep`'s on the script's first frame.
    app.audio_frame(0);
    let v = crate::kit::layout::Virtual::new([o.size.0 as f32, o.size.1 as f32]);
    // A virtual point's pixel: its centre, so the exe's truncating mouse map gives the point back.
    let px = |at: [f32; 2]| v.px(at[0] + 0.5, at[1] + 0.5);
    for tick in 0..=script.end() {
        if tick > 0 {
            if let Some(h) = &mut app.headless {
                h.ms = tick * 10;
            }
            app.frame();
        }
        for e in script.at(tick) {
            match &e.input {
                Input::Move(at) => app.cursor_moved(px(*at)),
                Input::Button { button, down, at } => {
                    app.cursor_moved(px(*at));
                    app.mouse(if *button == 3 { MouseButton::Right } else { MouseButton::Left }, *down);
                }
                Input::Key { name, down: true } => {
                    let key = match name.as_str() {
                        "down" => Key::Named(NamedKey::ArrowDown),
                        "up" => Key::Named(NamedKey::ArrowUp),
                        "right" => Key::Named(NamedKey::ArrowRight),
                        "left" => Key::Named(NamedKey::ArrowLeft),
                        "enter" => Key::Named(NamedKey::Enter),
                        "pageup" => Key::Named(NamedKey::PageUp),
                        "pagedown" => Key::Named(NamedKey::PageDown),
                        "escape" => Key::Named(NamedKey::Escape),
                        "space" => Key::Named(NamedKey::Space),
                        "tab" => Key::Named(NamedKey::Tab),
                        "delete" => Key::Named(NamedKey::Delete),
                        c => Key::Character(c),
                    };
                    app.key(key);
                }
                Input::Key { .. } => {}
            }
        }
    }
    // The last events' sounds.
    app.audio_frame(0);
    let heard: Vec<(Option<u32>, crate::audio::SoundId)> = app.audio.log.iter().map(|p| (Some(p.ui as u32), p.id)).collect();
    if !script.grabs.is_empty() {
        println!("ig2-client: --script on the starmap/colony draws no grabs");
    }
    let path = crate::script::write_sounds(out, &heard)?;
    println!("{}: {} sounds", path.display(), heard.len());
    Ok(())
}

/// `PlayerFaj[EN]`, the music's race.
fn human_race(state: &ig2_sim::save::SaveState) -> u8 {
    let en = state.mem.i32(ig2_sim::save::va::EN).max(0) as u32;
    state.mem.u8(ig2_sim::save::va::PLAYER_FAJ + en)
}

impl App {
    fn new(world: World<'static>, install: ig2_formats::Install, tables: &'static ig2_rules::Rules, maps: &'static [ig2_sim::ground::GroundMap], audio: crate::audio::Audio) -> Self {
        let start = Instant::now();
        App {
            front: None,
            world,
            install,
            tables,
            maps,
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
            info_down: false,
            modifiers: ModifiersState::empty(),
            title_at: start,
            frames: 0,
            panels: Panels::default(),
            panel_models: None,
            panel_held: None,
            frame_models: None,
            cam_tween: None,
            cam_saved: None,
            menus: Menus::default(),
            research: None,
            research_models: None,
            tech_infos: Vec::new(),
            research_tips: Vec::new(),
            audio,
            sound_overlay: None,
            sound_spy: None,
            sounds: Vec::new(),
            headless: None,
        }
    }

    fn now_ms(&self) -> u32 {
        match &self.headless {
            Some(h) => h.ms,
            None => self.start.elapsed().as_millis() as u32,
        }
    }

    /// The frame's sounds: the screens' queued ones, the enter/exit sounds of the screens that came or went, the UI
    /// ticks due (`FRISSIT2_SoundSystem`) and `UpdateFrame`'s music watchdog.
    fn audio_frame(&mut self, due_ui: u64) {
        use crate::audio::SoundId;
        use crate::ui::Screen;
        use ig2_rules::sounds::Table;
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        // `FRISSIT_Scroll2` (exe 0x556210): a message taken onto the line speaks, unless it is one of the four
        // silent ids; the brutal voice for empire 5 (`EN`), else the human one.
        let en = self.world.game.state.mem.i32(ig2_sim::save::va::EN);
        for id in std::mem::take(&mut self.world.popped) {
            if !matches!(id, 1 | 0x14 | 0x15 | 0x1a) {
                let voice = if en == 5 { "brutal" } else { "human" };
                self.audio.play_wav(&format!("sounds/scroll/{voice}/{id:04}.wav"));
            }
        }
        if let Some(c) = &mut self.colony {
            for id in c.ui.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        if let Some(h) = &mut self.hud {
            for id in h.info.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        if let Some(d) = self.hud.as_mut().and_then(|h| h.messages.open.as_mut()) {
            for t in d.typing.drain(..) {
                self.audio.typing(t);
            }
        }
        if let Some(d) = &mut self.diplomacy {
            for id in d.screen.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        if let Some(s) = &mut self.spy {
            for id in s.screen.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        if let Some(r) = &mut self.research {
            for id in r.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        for id in self.panels.sounds.drain(..) {
            self.audio.play(id);
        }
        // `Messages_Belep`/`_Kilep` exe 0x507f90/0x5083d0, `Diplomacy_Belep`/`_Kilep` 0x4bf530/0x4bf810.
        if self.overlay != self.sound_overlay {
            match self.sound_overlay {
                Some(Screen::Messages) => {
                    // `Messages_Kilep` (exe 0x5083d0) also stops the typer.
                    self.audio.stop_sound(SoundId::global(7));
                    self.audio.play(SoundId::new(Table::Mes, 1));
                }
                Some(Screen::Diplomacy) => self.audio.play(SoundId::new(Table::Dip, 1)),
                _ => {}
            }
            match self.overlay {
                Some(Screen::Messages) => self.audio.play(SoundId::new(Table::Mes, 0)),
                Some(Screen::Diplomacy) => self.audio.play(SoundId::new(Table::Dip, 0)),
                _ => {}
            }
            self.sound_overlay = self.overlay;
        }
        // `Spy_Belep` exe 0x58cd50, `Spy_Kilep` 0x58d250 (when the closing starts).
        let spy = self.spy.as_ref().map(|s| s.screen.closing);
        match (self.sound_spy, spy) {
            (None, Some(_)) => self.audio.play(SoundId::new(Table::Spy, 0)),
            (Some(false), Some(true)) => self.audio.play(SoundId::new(Table::Spy, 2)),
            _ => {}
        }
        self.sound_spy = spy;
        for _ in 0..due_ui {
            self.audio.tick();
        }
        self.audio.frame(human_race(&self.world.game.state));
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
            view_formats: Renderer::view_formats(format),
        };
        surface.configure(&device, &config);
        let mut renderer = Renderer::new(device, queue, format);
        let assets = Assets::load(&mut renderer, &self.install);
        self.panel_models = Some(PanelModels::new(&mut renderer));
        self.frame_models = Some(crate::screen_frames::FrameModels::new(&mut renderer));
        let scale = window.scale_factor() as f32;
        let [_, _, bw, bh] = crate::kit::Layout::new(config.width as i32, config.height as i32).band();
        self.view = Some(View::new(&self.world.curr, &self.world.view, (bw as u32, bh as u32), scale));
        let sel = self.selection();
        if sel.mode == 1 || sel.mode == 2 {
            self.select(sel);
        }
        let hud = crate::ui::Hud::load(&mut renderer, &self.install, self.tables);
        // The messages quote the empires by `BirodalomNev` (the HUD's table).
        self.world.game.empire_names = hud.empire_names.clone();
        self.hud = Some(hud);
        self.gpu = Some(Gpu { window, surface, config, renderer, assets, band_target: None });
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
            v.resize(g.band(), g.window.scale_factor() as f32);
        }
        if let Some(c) = &mut self.colony {
            c.view.resize(g.band(), g.window.scale_factor() as f32);
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
        let Some((renderer, band, scale)) = gfx(&mut self.gpu, &mut self.headless) else { return };
        self.world.show_colony(Some(p));
        let Some(snap) = self.world.colony_curr.clone() else { return };
        match crate::colony::ColonyScene::load(renderer, &self.install, self.tables, self.maps, &snap) {
            Ok(scene) => {
                let view = crate::colony::View::new(&snap, band, scale, &scene);
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

    /// A key of the war (`ColonyKeyboardHandler`): 4..9 the groups, S stop, `a` every own unit; only while the shown
    /// planet is at war.
    fn colony_war_key(&self, ch: &str) -> Option<crate::colony_war::WarKey> {
        use crate::colony_war::WarKey;
        self.world.colony_curr.as_ref().filter(|s| self.colony.is_some() && s.war_flag)?;
        match ch {
            "4" | "5" | "6" | "7" | "8" | "9" => Some(WarKey::Group(ch.as_bytes()[0] as usize - b'4' as usize)),
            "s" | "S" => Some(WarKey::Stop),
            "a" => Some(WarKey::All),
            _ => None,
        }
    }

    /// The colony screen's input with its context; the outcome carried out.
    fn colony_input(&mut self, f: impl FnOnce(&mut crate::colony_ui::ColonyUi, &crate::colony_ui::Ctx, [f32; 2], u32) -> crate::colony_ui::Outcome) {
        let now = self.now_ms();
        let (Some(c), Some(h), Some(snap)) = (&mut self.colony, &self.hud, &self.world.colony_curr) else { return };
        let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
        if let Some(gm) = self.maps.get(snap.map) {
            c.ui.war.project(&c.view, gm, snap);
        }
        c.ui.war.mods = crate::colony_war::Mods { shift: self.modifiers.shift_key(), ctrl: self.modifiers.control_key(), alt: self.modifiers.alt_key() };
        let ctx = crate::colony_ui::Ctx { state: &self.world.game.state, snap, rules: self.tables, sim: self.world.game.rules, texts: &h.texts, tech_name: &|t| h.tech_name(t) };
        let out = f(&mut c.ui, &ctx, at, now);
        if out.click {
            self.audio.play(crate::audio::SoundId::global(0));
        }
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

    /// Open the spy screen (`AktKepKell = 7`) or close it (`Spy_Kilep`: after the closing animation, which the
    /// redraw runs out).
    fn toggle_spy(&mut self) {
        if let Some(s) = &mut self.spy {
            if s.leave() {
                self.spy = None;
            }
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
        let (w, h) = g.band();
        Some(crate::font::Virtual::new([w as f32, h as f32]).virtual_of(p))
    }

    /// A starmap order or setting of the local player (`Multi_Send(.., EN, ..)`), applied in the next frame.
    /// Open or close a 2D screen (`I` info, `M` messages): `Info_Belep` starts on the human's empire and chart 0.
    fn toggle_screen(&mut self, s: crate::ui::Screen) {
        self.overlay = if self.overlay == Some(s) { None } else { Some(s) };
        if let (Some(crate::ui::Screen::Info), Some(h)) = (self.overlay, &mut self.hud) {
            // `InfoTriX/Y` keep the knob where it was left (`Info_Belep` only moves the model there).
            let tri = h.info.tri;
            h.info = crate::ui::InfoScreen::enter(self.world.ui.human);
            h.info.tri = tri;
        }
    }

    /// A left press on the info or messages screen (`Info_Main`, `Messages_Main`).
    fn screen_press(&mut self, screen: crate::ui::Screen) {
        let at = self.virtual_cursor();
        let Some(h) = &mut self.hud else { return };
        match screen {
            crate::ui::Screen::Info => {
                let id = h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui);
                h.info.press(id, &self.world.ui.info, self.world.ui.human, at[1]);
                self.info_down = true;
                let b = crate::info_screen::Buttons { press: true, down: true, left: true, ..Default::default() };
                for c in h.info.orders(id, at, b, &self.world.ui.info, self.world.ui.human) {
                    self.world.send(c);
                }
            }
            crate::ui::Screen::Messages => match h.messages.press(at, self.world.ui.inbox.len()) {
                crate::messages_screen::Press::Open(i) => h.open_message(&self.install, &self.world.ui, i),
                crate::messages_screen::Press::Leave => self.overlay = None,
                crate::messages_screen::Press::None => {}
            },
            _ => {}
        }
    }

    /// A right press, or a release, on the info screen (`Info_Main`'s triangle reset, drags, menus).
    fn info_button(&mut self, press: bool, right: bool) {
        let at = self.virtual_cursor();
        let Some(h) = &mut self.hud else { return };
        let id = h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui);
        self.info_down = press;
        let b = crate::info_screen::Buttons { press, release: !press, down: press, left: !right, right };
        for c in h.info.orders(id, at, b, &self.world.ui.info, self.world.ui.human) {
            self.world.send(c);
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
            // `IG2_gameUpdate` (exe 0x4e08c6): the speed buttons' sound, single player only.
            if !self.multiplayer() {
                self.sounds.push(crate::audio::SoundId::global(10));
            }
            self.send(StarmapOrder::Speed { level });
            self.send(StarmapOrder::Pause { paused: false });
        }
    }

    /// The top bar's research button (1005) or K: the research screen opens (`AktKepKell = 4`, `Research_Belep`)
    /// or, when up, closes.
    fn toggle_research(&mut self) {
        if self.overlay == Some(crate::ui::Screen::Research) {
            // `Research_Kilep`: after the closing animation, which the redraw runs out.
            if self.research.as_mut().is_none_or(|r| r.leave()) {
                self.overlay = None;
                self.research = None;
            }
            return;
        }
        if self.tech_infos.is_empty() {
            self.tech_infos = crate::research::tech_infos(&self.install);
        }
        if let (true, Some(h)) = (self.research_tips.is_empty(), &self.hud) {
            self.research_tips = crate::research::tips(&h.texts);
        }
        // `KELLKINYITASDEFAULT` (1): coming from the starmap, the screen opens animated.
        self.research = Some(crate::research::Research::open(&self.world.game.state, self.tables, true));
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
        r.held = true;
        if r.closing {
            return;
        }
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

    /// `MultiPlayerMode`.
    fn multiplayer(&self) -> bool {
        self.world.game.state.mem.u8(ig2_sim::save::va::MULTIPLAYER) != 0
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
                Action::Sound(id) => self.sounds.push(id),
                Action::ZoomIn { x, y, zoom } => {
                    if let (Some(v), None) = (&self.view, self.cam_saved) {
                        // `StarmapBeZoomol`: the zoom sound when not zoomed in yet (`StarmapZoomMode`).
                        self.sounds.push(crate::audio::SoundId::global(5));
                        self.cam_saved = Some([v.camera.x, v.camera.y, v.camera.zoom]);
                    }
                    self.cam_tween = Some(([x, -y, zoom], 8));
                }
                Action::ZoomOut => {
                    if let Some(s) = self.cam_saved.take() {
                        // `StarmapKiZoomol`'s sound.
                        self.sounds.push(crate::audio::SoundId::global(5));
                        self.cam_tween = Some((s, 8));
                    }
                }
                Action::Design(o) => {
                    // `MULTIGET_Design` 0x34 / 0x35 aren't in the sim yet (re/screens/starmap.md §19.7).
                    eprintln!("ig2-client: {o:?} not carried out (MULTIGET_Design isn't ported)");
                }
                Action::Centre { x, y } => {
                    if let Some(v) = &self.view {
                        self.cam_tween = Some(([x, -y, v.camera.zoom], 8));
                    }
                }
            }
        }
    }

    /// Carry out what the menus asked for.
    fn apply_do(&mut self, dos: Vec<Do>) {
        let sel = self.selection();
        for d in dos {
            let acts = match d {
                Do::Act(a) => vec![a],
                Do::Colony => {
                    if self.colony.is_none() {
                        self.toggle_colony();
                    }
                    Vec::new()
                }
                Do::Production => self.panels.toggle_production(&self.world.game.state, &sel),
                Do::Tanks => self.panels.tanks_button(&self.world.game.state, &sel),
                Do::Join => self.panels.fleet_transfer(&self.world.game.state, &sel),
                Do::TanksInfo => {
                    self.panels.open_tanks_info(self.sale_names());
                    Vec::new()
                }
                Do::FleetInfo => self.fleet_info(),
            };
            self.apply(acts);
        }
    }

    /// The camera as `[XStart, YStart, Zoom]`.
    fn camera3(&self) -> [f32; 3] {
        self.view.as_ref().map_or([0.0; 3], |v| [v.camera.x, v.camera.y, v.camera.zoom])
    }

    /// The sale menus' texts (`DesignTorlesValasztasNev`).
    fn sale_names(&self) -> crate::starmap_info::SaleNames {
        self.hud.as_ref().map(|h| crate::starmap_info::SaleNames::load(&h.texts)).unwrap_or_default()
    }

    /// The info box's left click in fleet mode (`AktRopIkon` 0xf): the fleet's info panel toggles (§19.1).
    fn fleet_info(&mut self) -> Vec<Action> {
        let sel = self.selection();
        self.cam_tween = None;
        let from = self.cam_saved.map_or(self.camera3()[2], |s| s[2]);
        let names = self.sale_names();
        self.panels.info_box_fleet(&self.world.game.state, &sel, names, from)
    }

    /// A left press on the starmap (`Starmap_Main`): the long info's hot-spots, the panels', the production view's
    /// planet list, then the map: the redirect pick, or a pick that becomes the selection and opens the radial menu.
    fn starmap_press(&mut self) -> bool {
        let at = self.virtual_cursor();
        let sel = self.selection();
        if self.menus.info_on() {
            // Hot-spots 8 (left), 9 and 11 leave; the panel's buttons and menus; the rest of the screen is caught.
            let cam = self.camera3();
            // Leaving plays the zoom sound (exe 0x5b303c).
            if (at[0] < 166.0 && at[1] >= 424.0) || crate::starmap_menus::exit_spot(at) {
                self.sounds.push(crate::audio::SoundId::global(5));
                self.menus.leave_info(cam, false);
            } else if (422.0..466.0).contains(&at[0]) && (432.0..476.0).contains(&at[1]) {
                self.sounds.push(crate::audio::SoundId::global(5));
                self.menus.leave_info(cam, true);
            } else if let Some(dos) = self.menus.press_info(&self.world.game.state, at) {
                self.apply_do(dos);
            }
            return true;
        }
        if self.panels.product && (276.0..363.0).contains(&at[0]) && (32.0..79.0).contains(&at[1]) {
            let dos = self.menus.open_menu(&self.world.game.state, MenuKind::Planets, at);
            self.apply_do(dos);
            return true;
        }
        if self.panel_press(true) {
            self.panel_held = Some(true);
            return true;
        }
        // The top bar's buttons (`HACK_SetBrushok`'s speed records 1012..=1015: pause, ×1, ×2, ×3) act on the press
        // (`IG2_gameUpdate`, exe 0x4e08c6; observed: the speed sound on the press, sound-id trace 2026-10-22).
        if at[1] < 20.0 && at[0] < 73.0 && !self.panels.fleet_info() {
            match at[0] {
                x if x < 15.0 => self.toggle_pause(),
                x if x < 29.0 => self.set_speed(1),
                x if x < 50.0 => self.set_speed(2),
                _ => self.set_speed(3),
            }
            return true;
        }
        // The rest of the top bar, the info box and the minimap act on the release.
        let m = orders::MINIMAP_SPOT;
        let on_minimap = !self.panels.minimap_off() && at[0] >= m[0] && at[0] < m[2] && at[1] >= m[1] && at[1] < m[3];
        // The minimap (hot-spot 7) clicks on the press (exe 0x5b2e84).
        if on_minimap {
            self.sounds.push(crate::audio::CLICK);
        }
        if at[1] < 20.0 || (at[0] < 166.0 && at[1] >= 424.0) || on_minimap || (self.panels.transfer.is_some() && at[1] >= 20.0) {
            return false;
        }
        // With an info panel up the map takes no picks (hot-spot 0x35 is off).
        if self.panels.info.is_some() {
            return true;
        }
        let Some(v) = &self.view else { return false };
        if self.menus.redirect {
            self.sounds.push(crate::audio::CLICK);
            let target = orders::select_nearest_planet(&self.world.curr, &v.camera, at, 1.0);
            let dos = self.menus.redirect_to(&self.world.game.state, &sel, target);
            self.apply_do(dos);
            return true;
        }
        let pick = if self.panels.production_open() {
            orders::select_nearest_planet(&self.world.curr, &v.camera, at, 1.0).map(|p| Selection { mode: 1, planet: p, ..sel })
        } else {
            orders::left_click(&self.world.curr, &v.camera, at, &sel)
        };
        if let Some(s) = pick {
            self.select(s);
            let s = self.selection();
            let blink = (self.now_ms() / 100) % 2 == 1;
            let places = crate::starmap_menus::rop_places(&self.world.game.state, &s, &self.panels, self.menus.redirect, (self.menus.sat_sent, self.menus.col_sent), blink);
            self.menus.open_rop(at, places);
        }
        true
    }

    /// The left button went up after [`Self::starmap_press`]: an open select menu sends, the radial menu fires its
    /// icon.
    fn starmap_release(&mut self) {
        let sel = self.selection();
        if self.menus.menu.is_some() {
            let dos = self.menus.release_menu(&self.world.game.state, &sel);
            self.apply_do(dos);
        }
        if self.menus.rop.is_some() {
            let icon = self.menus.release_rop();
            let dos = self.menus.fire(icon, &self.world.game.state, &sel);
            self.apply_do(dos);
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
            // Hot-spot 8 in planet mode: the zoom sound before the branch (exe 0x5b6373).
            self.sounds.push(crate::audio::SoundId::global(5));
            self.toggle_colony();
            return;
        }
        let state = &self.world.game.state;
        let Some(r) = state.fleets.get(sel.fleet) else { return };
        // Not for `fSpec` fleets (flags 0x4400, `Starmap_Main`, re/screens/starmap.md §19.1).
        if u16::from_le_bytes([r[0x52], r[0x53]]) & 0x4400 != 0 {
            return;
        }
        // Fleet mode clicks (exe 0x5b6a07), again when it closes a transfer panel (0x5b6b99).
        self.sounds.push(crate::audio::CLICK);
        if self.panels.transfer.is_some() {
            self.sounds.push(crate::audio::CLICK);
        }
        let f = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let (mine, calm, pos) = (r[0x15] == self.world.curr.human, r[0x52] & 1 == 0, (f(0x30), f(0x34)));
        // The fleet's info panel goes without the zoom-out; the split then keeps the zoom (§19.1).
        let had_info = self.panels.info.take().is_some();
        let closed = self.panels.transfer;
        let acts = self.panels.close_transfer(state, &sel, true);
        self.apply(acts);
        if mine && calm && closed.is_none_or(|t| t.tank_mode) {
            if closed.is_none() && !had_info {
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
        if self.menus.info_on() {
            return;
        }
        // The info box (hot-spot 8 at (0, 424, 166, 56)): right = colony / split, left in planet mode = the long info.
        if at[0] >= 0.0 && at[0] < 166.0 && at[1] >= 424.0 {
            if right {
                self.info_box_click();
            } else if sel.mode == 2 {
                let acts = self.fleet_info();
                self.apply(acts);
            } else if sel.mode == 1 && sel.planet > 0 {
                self.sounds.push(crate::audio::SoundId::global(5));
                self.panels.info = None;
                self.panels.close_production();
                let acts = self.panels.close_transfer(&self.world.game.state, &sel, false);
                self.apply(acts);
                self.cam_tween = None;
                let cam = self.camera3();
                self.menus.enter_info(sel.planet, cam);
            }
            return;
        }
        // The transfer panel covers the map (hot-spot 0x58); under an info panel the map takes no orders.
        if (self.panels.transfer.is_some() || self.panels.info.is_some()) && at[1] >= 20.0 {
            return;
        }
        if right {
            // In planet mode on the player's planet (and in the redirect mode) the right button picks the rally
            // planet (`0x400`).
            let own = sel.mode == 1 && self.world.curr.planets.iter().any(|p| p.index == sel.planet && p.owner == self.world.curr.human);
            if self.menus.redirect || own {
                // The rally pick clicks before it picks (exe 0x5b5406).
                self.sounds.push(crate::audio::CLICK);
                let target = orders::select_nearest_planet(&self.world.curr, &v.camera, at, 1.0);
                let dos = self.menus.redirect_to(&self.world.game.state, &sel, target);
                self.apply_do(dos);
            } else if let Some(o) = orders::right_click(&self.world.curr, &v.camera, at, &sel, keys) {
                // An own fleet's order: a click with Shift (exe 0x5b51b3); without it the fleet's voice
                // [`StarmapSpeaker`, not ported].
                let own_fleet = sel.mode == 2 && self.world.game.state.fleets.get(sel.fleet).is_some_and(|f| f[0x15] == self.world.curr.human);
                if own_fleet && keys.shift {
                    self.sounds.push(crate::audio::CLICK);
                }
                self.send(o);
            }
            return;
        }
        // The top bar's and the console's hot-spots (1..=7) are off while the fleet's info panel is zoomed in.
        if at[1] < 20.0 && self.panels.fleet_info() {
            return;
        }
        if at[1] < 20.0 {
            return;
        }
        let m = orders::MINIMAP_SPOT;
        // The panels turn the minimap off (not the vehicle panel).
        if !self.panels.minimap_off() && at[0] >= m[0] && at[0] < m[2] && at[1] >= m[1] && at[1] < m[3] {
            let t = orders::minimap_target(at);
            if let Some(v) = &mut self.view {
                (v.camera.x, v.camera.y) = (t[0], t[1]);
            }
        }
    }

    /// A frame of the main menu: its UI ticks due (10 ms each), the frame (`Mainscreen_Update3D`), the hover.
    fn front_frame(&mut self) {
        let now = self.now_ms();
        let (Some(fr), Some(g)) = (&mut self.front, &mut self.gpu) else { return };
        let mut due = 0;
        while now >= fr.ui_ms + 10 {
            fr.ui_ms += 10;
            fr.s.tick();
            due += 1;
        }
        for _ in 0..due {
            self.audio.tick();
        }
        self.audio.frame(human_race(&self.world.game.state));
        fr.s.frame();
        fr.s.hover(fr.at);
        let (bw, bh) = g.band();
        let size = [bw as f32, bh as f32];
        let scene = fr.scene.get_or_insert_with(|| crate::mainscr_scene::MainScene::load(&mut g.renderer, &self.install));
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        scene.draw(&mut frame, &mut g.renderer, &self.install, &fr.s, &crate::font::Virtual::new(size));
        let Ok(out) = g.surface.get_current_texture() else {
            self.reconfigure();
            return;
        };
        g.present(frame, &out.texture, now as f32 / 1000.0);
        g.window.pre_present_notify();
        out.present();
    }

    /// A ring's request (`crate::mainscr::Act`): load game starts the game from the save (the Options screen's load
    /// slots aren't ported yet), quit leaves; the rest wait for their screens.
    fn front_act(&mut self, a: crate::mainscr::Act) {
        use crate::mainscr::Act;
        match a {
            Act::Quit => {
                if let Some(f) = &mut self.front {
                    f.quit = true;
                }
            }
            Act::LoadGame => {
                self.front = None;
                self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game.state));
                self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
            }
            other => eprintln!("ig2-client: {other:?}: not ported yet (re/screens/mainscr.md §14)"),
        }
    }

    fn frame(&mut self) {
        if self.front.is_some() {
            self.front_frame();
            return;
        }
        let now = self.now_ms();
        // The frame's message pass, then the ticks due (none while `IdoPause`).
        self.world.pump();
        self.menus.sent_done();
        if let Some(c) = &mut self.colony {
            let p = self.world.colony.unwrap_or(0);
            c.ui.events(&std::mem::take(&mut self.world.colony_events), self.world.curr.human, p);
            // A building came or went: `Init3dColony`'s building loop again.
            if let (Some((renderer, ..)), Some(s)) = (gfx(&mut self.gpu, &mut self.headless), &self.world.colony_curr) {
                if c.scene.stale(s) {
                    match crate::colony::ColonyScene::load(renderer, &self.install, self.tables, self.maps, s) {
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
        if !self.world.paused() && self.headless.is_none() {
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
        if let Some(i) = &mut self.panels.info {
            let zoom = self.view.as_ref().map_or(0.0, |v| v.camera.zoom);
            let ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50) as u32;
            i.advance(&self.world.game.state, &sel, now as f32 / 1000.0, zoom, ui);
        }
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
                h.info.ui_frame(&world.ui.info, world.ui.human);
                h.messages.ui_frame();
            }
            if let Some(d) = &mut h.messages.open {
                d.advance(due_ui as f32 * 10.0);
            }
        }
        // `FRISSIT_Starmap2` runs on the starmap only.
        if let (Some(v), None, None) = (&mut self.view, &self.colony, self.overlay) {
            for _ in 0..due_ui {
                v.ui_tick();
            }
        }
        // The radial menu's frame (`FRISSIT_Starmap2`).
        for _ in 0..due_ui {
            self.menus.ui_frame();
        }
        // The long info's fly in and out and its pin to the planet (`Starmap_Main`, `GFX_Starmap`).
        let mut production = false;
        if let Some(v) = &mut self.view {
            let c = &v.camera;
            let (curr, prev) = (&self.world.curr, &self.world.prev);
            let clock = prev.orbit + (curr.orbit - prev.orbit) * self.clock.phase(now).clamp(0.0, 1.0);
            let orbit = |p: usize| curr.planets.iter().find(|q| q.index == p).map_or([0.0; 2], |q| crate::starmap::orbit_position(q, clock));
            let planet = self.menus.info.map(|i| i.planet);
            let (cam, star, out) = self.menus.advance_info([c.x, c.y, c.zoom, c.rotate], orbit, now as f32 / 1000.0);
            if let Some([x, y, z]) = cam {
                (v.camera.x, v.camera.y, v.camera.zoom) = (x, y, z);
            }
            v.star_scale = planet.filter(|_| self.menus.info_on()).map(|p| (p, star));
            // `KellSelect` 0: no selection ring while the long info is up.
            if self.menus.info_on() {
                v.selected = None;
            } else if v.selected.is_none() {
                let s = self.world.game.local_view();
                v.selected = match s.starmap_mode {
                    1 => Some(Pick::Planet(s.planet.max(0) as usize)),
                    2 => Some(Pick::Fleet(s.fleet.max(0) as usize)),
                    _ => None,
                };
            }
            production = out == Some(true);
        }
        if production {
            let acts = self.panels.toggle_production(&self.world.game.state, &sel);
            self.apply(acts);
        }
        if let (Some(c), Some(s)) = (&mut self.colony, &self.world.colony_curr) {
            for _ in 0..due_ui {
                // `Frissit2_Colony`'s ambients and music only with the screen shown (`AktKep == 3`).
                if self.overlay.is_none() && self.spy.is_none() {
                    c.ui.ui_frame(s, self.world.curr.human as i32, &mut c.view.camera);
                    if let Some(m) = c.ui.war_music(s.at_war) {
                        self.audio.play_music(m, false, human_race(&self.world.game.state));
                    }
                }
            }
            c.view.overlay = c.ui.overlay(s, self.world.curr.human as i32);
        }
        if let Some(r) = &mut self.research {
            for _ in 0..due_ui {
                r.step(&self.world.game.state);
            }
            if r.closed() {
                self.research = None;
                self.overlay = None;
            }
        }
        if let (Some(r), Some(m)) = (&mut self.research, &self.research_models) {
            r.tip.moved(self.view.as_ref().map_or([0.0; 2], |v| v.camera.virtual_of(self.cursor)));
            r.tooltip(&self.world.game.state, &m.mask, &self.research_tips, now as f64);
        }
        // The sounds queued by the frame's input and by these UI ticks, then the ticks' music steps.
        self.audio_frame(due_ui);
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
        let size = { let (w, h) = g.band(); [w as f32, h as f32] };
        let panel_tex = match (&mut self.panel_models, self.colony.is_none() && self.overlay.is_none() && self.panels.active()) {
            (Some(m), true) => m.render(&mut g.renderer, &self.install, self.tables, &self.panels, &self.world.game.state, &sel, size),
            _ => None,
        };
        if let (Some(spy), Some(h)) = (&mut self.spy, &mut self.hud) {
            let en = self.world.curr.human as usize;
            let d = crate::spy_screen::SpyData::read(&self.world.game.state, en, self.world.game.rules, self.tables);
            spy.advance(&mut g.renderer, &self.install, &d, due_ui, t);
            if spy.closed() {
                self.spy = None;
                g.window.request_redraw();
                return;
            }
            h.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
            h.paused = paused;
            let mut frame = spy.build(h, &d, size, self.install.lang() == ig2_formats::text::Lang::Hu);
            h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Spy, t);
            if let Ok(out) = g.surface.get_current_texture() {
                g.present(frame, &out.texture, t);
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
            self.dip_input.at = vv.virtual_of(self.cursor);
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
                    h.draw_starmap_panels(&mut frame, &self.panels, &self.world.game.state, self.tables, &sel, panel_tex, size, t);
                    h.draw_starmap_menus(&mut frame, &self.menus, &self.world.game.state, self.tables, size, v.camera.zoom, t);
                }
                (crate::ui::Screen::Research, ..) => {
                    if let (Some(r), Some((tex, lines))) = (&self.research, research) {
                        let glow = self.research_models.as_ref().and_then(|m| m.glow);
                        h.draw_research(&mut frame, r, &self.tech_infos, &self.research_tips, &self.world.game.state, self.tables, (tex, glow), lines, size, t);
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
        g.present(frame, &out.texture, t);
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
            WindowEvent::RedrawRequested => {
                self.frame();
                if self.front.as_ref().is_some_and(|f| f.quit) {
                    el.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } if self.front.is_some() => {
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let (bw, bh) = self.gpu.as_ref().map_or((640, 480), |g| g.band());
                let at = crate::font::Virtual::new([bw as f32, bh as f32]).virtual_of([position.x as f32, position.y as f32 - top]);
                if let Some(f) = &mut self.front {
                    f.at = at;
                }
            }
            WindowEvent::KeyboardInput { event, .. } if self.front.is_some() => {
                if event.state == ElementState::Pressed && event.logical_key == Key::Named(NamedKey::Escape) {
                    if let Some(f) = &mut self.front {
                        f.s.escape();
                    }
                }
            }
            WindowEvent::MouseWheel { .. } | WindowEvent::MouseInput { button: MouseButton::Right | MouseButton::Middle, .. } if self.front.is_some() => {}
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.front.is_some() => {
                let Some(f) = &mut self.front else { return };
                let at = f.at;
                let act = if state == ElementState::Pressed { f.s.press(at) } else { f.s.release(at) };
                if let Some(a) = act {
                    self.front_act(a);
                }
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                // The band's pixels: the mouse goes through the frame (`IG2_VIRTUAL_POSY`, re/screens/widescreen.md §4).
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let p = [position.x as f32, position.y as f32 - top];
                self.cursor_moved(p);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 60.0,
                };
                self.wheel(steps);
            }
            WindowEvent::MouseInput { state, button, .. } => self.mouse(button, state == ElementState::Pressed),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let quit = self.key(event.logical_key.as_ref());
                if quit {
                    el.exit();
                }
            }
            _ => {}
        }
    }
}

impl App {
    /// The pointer moved to `p` (the band's pixels).
    fn cursor_moved(&mut self, p: [f32; 2]) {
        if let Some(at) = self.spy_virtual(p) {
            let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
            if let Some(spy) = &mut self.spy {
                spy.hover(&d, at);
            }
        }
        if let Some(v) = &self.view {
            let at = v.camera.virtual_of(p);
            self.panels.hover(at);
            self.menus.hover(at);
            if let Some(h) = &mut self.hud {
                match self.overlay {
                    Some(crate::ui::Screen::Info) => {
                        let id = h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui);
                        h.info.hover(id);
                        let b = crate::info_screen::Buttons { down: self.info_down, left: true, ..Default::default() };
                        for c in h.info.orders(id, at, b, &self.world.ui.info, self.world.ui.human) {
                            self.world.send(c);
                        }
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

    /// The wheel turned by `steps` notches.
    fn wheel(&mut self, steps: f32) {
        if let Some(c) = &mut self.colony {
            c.view.camera.zoom_steps(steps);
        } else if let (Some(v), false) = (&mut self.view, self.menus.info_on()) {
            v.camera.zoom_steps(steps);
        }
    }

    /// A mouse button went down or up.
    fn mouse(&mut self, button: MouseButton, pressed: bool) {
        let state = if pressed { ElementState::Pressed } else { ElementState::Released };
        if button == MouseButton::Left && self.spy.is_some() {
            let Some(at) = self.spy_virtual(self.cursor) else { return };
            let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
            let Some(spy) = &mut self.spy else { return };
            let orders = if state == ElementState::Pressed { spy.press(&d, at) } else { spy.release(&d) };
            let errors = spy.take_errors();
            for c in orders {
                self.world.send(c);
            }
            self.world.local_lines.extend(errors);
            return;
        }
        let on_starmap = self.colony.is_none() && self.overlay.is_none();
        let moved = |d: Option<[f32; 2]>, p: [f32; 2]| d.is_none_or(|d| (d[0] - p[0]).abs() + (d[1] - p[1]).abs() > 4.0);
        match (button, state) {
            (MouseButton::Left, ElementState::Pressed) if self.top_bar_research() => self.toggle_research(),
            (MouseButton::Left, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Research) => self.research_press(),
            (MouseButton::Left, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Research) => {
                if let Some(r) = &mut self.research {
                    r.held = false;
                }
            }
            (MouseButton::Left, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Diplomacy) => {
                self.dip_input.pressed = true;
                self.dip_input.held = true;
            }
            (MouseButton::Left, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Diplomacy) => {
                self.dip_input.released = true;
                self.dip_input.held = false;
            }
            (MouseButton::Left | MouseButton::Right, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Info) => self.info_button(false, button == MouseButton::Right),
            (MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Info) => self.info_button(true, true),
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
                self.menus.held = true;
                if !(on_starmap && self.starmap_press()) {
                    self.left_down = Some(self.cursor);
                }
            }
            (MouseButton::Left, ElementState::Released) if self.menus.menu.is_some() || self.menus.rop.is_some() => {
                self.menus.held = false;
                self.starmap_release();
            }
            (MouseButton::Left, ElementState::Released) if self.panel_held.is_some() => {
                self.menus.held = false;
                self.panel_held = None;
                let sel = self.selection();
                let acts = self.panels.release(&self.world.game.state, &sel);
                self.apply(acts);
            }
            (MouseButton::Left, ElementState::Released) => {
                self.menus.held = false;
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

    /// A key went down; true = quit.
    fn key(&mut self, key: Key<&str>) -> bool {
        match key {
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Research) => self.toggle_research(),
            Key::Named(NamedKey::Escape) if self.spy.is_some() => self.toggle_spy(),
            Key::Named(NamedKey::Escape) if self.overlay.is_some() => self.overlay = None,
            // The ground war's keys (`ColonyKeyboardHandler`, colony.md §11.2) while the shown planet is at war.
            Key::Character(ch) if self.colony_war_key(ch).is_some() => {
                if let Some(k) = self.colony_war_key(ch) {
                    self.colony_input(move |ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::War(k)));
                }
            }
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
            Key::Named(NamedKey::Escape) => return true,
            Key::Character("c" | "C") => self.toggle_colony(),
            Key::Named(NamedKey::Space) => self.toggle_pause(),
            Key::Character(c @ ("1" | "2" | "3")) => {
                // `DoKeyboard` (ELF 0x5800d0): the keys sound whatever the speed was, single player only.
                if !self.multiplayer() && !self.speed_locked() {
                    self.sounds.push(crate::audio::SoundId::global(10));
                }
                let level = c.parse().unwrap_or(1);
                if !self.speed_locked() && (self.world.paused() || self.world.game.speed_level() != level) {
                    self.send(StarmapOrder::Speed { level });
                    self.send(StarmapOrder::Pause { paused: false });
                }
            }
            Key::Character("q" | "Q" | "e" | "E") => {
                if let Some(v) = &mut self.view {
                    // Five keyboard frames per key event (the original turns per 10 ms frame while held).
                    v.camera.turn(if matches!(key, Key::Character("q" | "Q")) { -5.0 } else { 5.0 });
                }
            }
            Key::Named(NamedKey::PageUp | NamedKey::PageDown) => {
                if let Some(v) = &mut self.view {
                    v.camera.tilt(if key == Key::Named(NamedKey::PageUp) { 5.0 } else { -5.0 });
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
        }
        false
    }
}
