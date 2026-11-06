//! The window and frame loop (ADR 0006 §1–§2): winit + a wgpu surface, the sim ticked by the original's
//! `TickClock` (20 Hz, ≤ 500 ms catch-up) and every frame drawn at the display's refresh rate, interpolated.
//!
//! Screens (`DoKeyboard`, re/screens/input.md §1; `do_keyboard`): F1 help, F2 starmap, F3 colony (the selected planet,
//! else the human's first), F4 research, F5 design, F6 diplomacy, F7 spy, F8 trade, F9 info, F10 messages, M the
//! messages screen, Esc or O the Options screen (settings, save and load slots, quit; re/screens/options.md), each
//! with the original's gates; a screen up leaves through its closing first.
//! Starmap: the screen edges, the minimap and the arrows and numpad +/- = the camera; ours (ADR 0049) also wheel =
//! zoom (the right button never pans: it gives orders); PgUp/PgDn = the previous/next own planet or fleet, left click = select (again: the next unit
//! under the cursor), the right press = the selected fleet's order (Shift: waypoint, Ctrl: attack, Alt: a point), `c`
//! colonises.
//! Colony: wheel = zoom, left drag = pan, right drag = turn and tilt (the original's both-buttons drag); the building
//! screen's hot-spots (`crate::colony_ui`: select, repair/demolish windows, the build menu's pick list held on the
//! planet panel, the build cursor, the radar), E = a building's priority switch, Shift+Delete = demolish, PgUp/PgDn =
//! the previous/next own colony, I = the planet info mode, `c` = the camera presets.
//! Info: the mouse over a race column picks the empire, the chart buttons pick the chart. Messages: a click on an inbox
//! row opens the message, on an answer (or a..d) answers it; a click in the text types it out; a click elsewhere
//! leaves. Spy: its buttons, lists and spies by mouse; ←/→ = the next/previous spy (re/screens/spy.md). Diplomacy: a
//! click on a head picks the empire, its menus by mouse (re/screens/diplomacy.md). Research: its buttons, lists and
//! models by mouse; arrows = fields, PgUp/PgDn = categories, Enter = start/stop, D = urgent (re/screens/research.md).
//! The info box's buttons open the production view and the transfer panel (re/screens/starmap.md §12–§13); a right
//! click on the box splits the selected fleet (the transfer between the two opens) or shows the planet's colony, a
//! left click on a planet's box its long info with the tax, auto-build and priority menus (§15–§16). A left press on
//! the map selects and opens the radial icon menu; releasing over an icon carries it out (§14). Esc with a panel up
//! closes it (hot-spot 9).
//! Always: the top bar's speed buttons or 1/2/3 = speed, space or P = pause (lockstep commands, re/screens/starmap.md
//! §10). `IG2_VSYNC=0` starts with vsync off.

use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use crate::web::arboard;
use web_time::Instant;

use ig2_net::{Command, StarmapCmd};
use ig2_sim::game::{LocalView, OrderEvent, StarmapOrder};
use ig2_sim::time::{TickClock, SIM_TICK_MS};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::orders::{self, Keys, Selection};

mod backdrop;
mod battle;
mod colony;
mod do_keyboard;
mod events;
mod front;
mod gpu;
mod input;
mod media;
mod menus;
mod modern;
mod pointer;
mod presence;
mod saves;
mod screens;
mod session;
mod starmap;
mod top_bar;
mod tutorial;
use crate::render::Renderer;
use crate::starmap::{Assets, Pick, View};
use crate::starmap_menus::{Do, MenuKind, Menus};
use crate::starmap_panels::{Action, PanelModels, Panels};
use crate::world::World;

/// The page's answer to come: [`GpuParts::request`]'s result.
#[cfg(target_arch = "wasm32")]
type GpuWait = std::rc::Rc<std::cell::RefCell<Option<Result<GpuParts, String>>>>;

/// The window's surface and the device it was asked for.
struct GpuParts {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuParts {
    async fn request(window: Arc<Window>) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        // The browser: WebGPU where it works, else WebGL2 (ADR 0047 §4); the page's `?webgl` asks for WebGL2.
        #[cfg(target_arch = "wasm32")]
        let instance = {
            let mut desc = wgpu::InstanceDescriptor::default();
            if crate::web::webgl_only() {
                desc.backends.remove(wgpu::Backends::BROWSER_WEBGPU);
            }
            wgpu::util::new_instance_with_webgpu_detection(&desc).await
        };
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| if cfg!(target_arch = "wasm32") { format!("this browser offers neither WebGPU nor WebGL2 ({e})") } else { e.to_string() })?;
        // WebGL2's limits are below wgpu's defaults: the browser asks for what the adapter has.
        let required_limits = if cfg!(target_arch = "wasm32") { adapter.limits() } else { Default::default() };
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor { required_limits, ..Default::default() }).await.map_err(|e| e.to_string())?;
        Ok(Self { window, surface, adapter, device, queue })
    }
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    assets: Assets,
    /// Where a window narrower than 4:3 draws its band before it goes on the window.
    band_target: Option<crate::render::Target>,
    /// Ours (ADR 0062): the designed pages' egui layer.
    gui: crate::gui::Gui,
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
    /// side strips black under the "letterbox" option, this frame's egui page (ADR 0062) over the band and `top` (the
    /// pointer) over it. Gives the frame back (the overlays' `SaveScreen`).
    fn present_over(&mut self, mut frame: crate::render::Frame, top: Option<crate::render::Frame>, out: &wgpu::Texture, t: f32) -> crate::render::Frame {
        let l = self.layout();
        crate::kit::layout::letterbox_bars(&mut frame, &l, self.assets.white);
        let size = (self.config.width, self.config.height);
        let [x, y, w, h] = l.band();
        let over = |r: &mut Renderer, gui: &mut crate::gui::Gui, tex: &wgpu::Texture, px: (u32, u32)| {
            gui.paint(&r.device, &r.queue, tex);
            if let Some(mut top) = top {
                top.keep = true;
                r.draw(&top, tex, px, t);
            }
        };
        if [w, h] == [size.0 as i32, size.1 as i32] {
            self.renderer.draw(&frame, out, size, t);
            over(&mut self.renderer, &mut self.gui, out, size);
            return frame;
        }
        let px = (w as u32, h as u32);
        let target = match &mut self.band_target {
            Some(tg) => tg,
            None => self.band_target.insert(self.renderer.target(px)),
        };
        self.renderer.draw_into(&frame, target, px, t);
        over(&mut self.renderer, &mut self.gui, target.texture(), px);
        let mut f = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        let c = [x as f32 + w as f32 / 2.0, y as f32 + h as f32 / 2.0];
        f.push(target.tex, crate::render::Blend::Alpha, crate::render::Instance::new(crate::render::kind::SPRITE, c, [w as f32 / 2.0, h as f32 / 2.0], [1.0; 4]));
        self.renderer.draw(&f, out, size, t);
        frame
    }

    /// One window frame: acquire the surface's texture, draw `frame` with `top` over it ([`Self::present_over`]) and
    /// present it. Gives the frame back; on an error nothing is drawn (see [`App::surface_failed`]).
    fn present(&mut self, frame: crate::render::Frame, top: Option<crate::render::Frame>, t: f32) -> Result<crate::render::Frame, wgpu::SurfaceError> {
        let acquire = crate::prof::scope("acquire");
        let out = self.surface.get_current_texture()?;
        drop(acquire);
        let draw = crate::prof::scope("draw");
        let frame = self.present_over(frame, top, &out.texture, t);
        drop(draw);
        let _present = crate::prof::scope("present");
        self.window.pre_present_notify();
        out.present();
        Ok(frame)
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
    /// The scenario list's entries, tables and pictures, read when it first opens.
    scenarios: Option<crate::mainscr_scenario::Assets>,
    /// Our MP screens (ADR 0014), their links, and the session a started room built (the starmap takes it).
    mp: Option<crate::mp_lobby::MpScreen>,
    net: crate::mp_lobby::ClientNet,
    session: Option<crate::mp_lobby::Session<'static>>,
    /// Ours (ADR 0034): the statistics screen over the menu (a game's end, or the history).
    stats: Option<crate::stats_screen::StatsScreen>,
    /// Ours (ADR 0038 §6): the replays list over the menu.
    replays: Option<crate::replay_viewer::ReplayList>,
    /// Ours (ADR 0068): the mod manager over the menu.
    mods: Option<crate::mods_page::ModsPage>,
    /// The press that opened the statistics or replays screen: its release isn't theirs (ADR 0059: the panel's
    /// entries lie over their rows).
    opening: bool,
    /// Ours (ADR 0078): the modern new game page's race and level, and the races' portraits (read when it first opens).
    ng: crate::newgame_modern::State,
    ng_heads: Option<[Option<egui::TextureHandle>; 3]>,
    /// The modern statistics page's race heads.
    stats_heads: Option<Vec<Option<egui::TextureHandle>>>,
}

impl Front {
    /// `MainScr_Belep`, `MultiplayerNev` from the settings file (`SaveGlobalRegistrySettings`' value; "Player" without one).
    fn new(scene: Option<crate::mainscr_scene::MainScene>, ui_ms: u32, settings: &ig2_formats::options::Options) -> Self {
        let mut s = crate::mainscr::MainScr::enter();
        s.settings = settings.int(crate::display::MENU_SETTINGS) != Some(0);
        s.flow = settings.int(crate::menu_flow::KEY) == Some(1);
        s.history = settings.int(crate::display::STATISTICS) != Some(0);
        s.can_continue = crate::options::slot_dir().is_some_and(|d| !crate::menu_flow::saves_newest_first(&d).is_empty());
        // Ours (ADR 0078): the modern main menu brings the grouped panel with it.
        s.modern = settings.int(crate::display::MODERN_MAIN_MENU) != Some(0);
        s.flow |= s.modern;
        s.continue_line = s.modern.then(crate::mainscr_modern::continue_line).flatten();
        if let Some(n) = settings.get("MultiplayerNev").filter(|n| !n.is_empty()) {
            s.player = n.as_bytes().to_vec();
        }
        Front { s, scene, ui_ms, at: [320.0, 240.0], quit: false, scenarios: None, mp: None, net: Default::default(), session: None, stats: None, replays: None, mods: None, opening: false, ng: Default::default(), ng_heads: None, stats_heads: None }
    }
}

/// `scenario_kezdesi_ido` (exe 0xBAD108) and `scenario_registry_name` (0xBAD0E8, 0x1e bytes).
const SCENARIO_START: u32 = 0xBAD108;
const SCENARIO_REGISTRY: u32 = 0xBAD0E8;

/// How long a story's earthquake shakes the colony camera (`FRISSIT_Story`: `Colony_foldrenges_ido + 10000 < AktIdo`).
const QUAKE: std::time::Duration = std::time::Duration::from_millis(10000);

struct App {
    /// The main menu, until a ring starts the game.
    front: Option<Front>,
    world: World<'static>,
    install: ig2_formats::Install,
    tables: &'static ig2_rules::Rules,
    maps: &'static [ig2_sim::ground::GroundMap],
    colony: Option<Colony>,
    hud: Option<crate::ui::Hud>,
    /// The 2D screen over the starmap (info, messages), if one is up.
    overlay: Option<crate::ui::Screen>,
    /// The spy screen, if it is up.
    spy: Option<crate::spy_screen::SpyView>,
    /// The spy screen's 3D scene, loaded at its first opening and kept for the next ones.
    spy_scene: Option<std::rc::Rc<crate::spy_scene::SpyScene>>,
    /// The message line's 10 ms UI frames done so far.
    ui_frames: u64,
    /// Multiplayer: the transfer panel as last sent (`Command::TransferPanel`).
    sent_transfer: Option<(i32, i32)>,
    /// `FRISSIT_Messages2`'s frame counter (the automatic messages every 11th UI frame).
    messages_auto: u32,
    /// The diplomacy screen while it is up, and this frame's pointer edges on it.
    diplomacy: Option<crate::diplomacy::Open>,
    dip_input: crate::diplomacy::Input,
    /// The screen the diplomacy screen's closing leads to (`Diplomacy_Kilep`; `None` = the starmap).
    dip_next: Option<crate::ui::Screen>,
    /// The screen change asked for (`AktKepKell`), until the screen up has left (`do_keyboard`).
    kep_kell: Option<do_keyboard::KepKell>,
    /// `AktPlanetKell` with a colony screen asked for by the story (`StoryUi`): the planet it opens on.
    planet_kell: Option<usize>,
    /// `Colony_foldrenges` (exe 0x9ABB70): the planet of the story's earthquake, and when its colony screen first drew
    /// after it (`Colony_foldrenges_ido` 0x9ABB74); `quake_rand` = the jolt's libc `rand()` (cosmetic).
    quake: Option<(usize, Option<Instant>)>,
    quake_rand: crate::audio::Rand,
    /// The multiplayer wait overlay (`crate::mp_wait`).
    mp_wait: crate::mp_wait::Wait,
    /// The button whose press the top bar took (`top_bar`): its release is the bar's too.
    bar_held: Option<MouseButton>,
    /// The left button went down on the message line (spot 7) and is still held.
    bar_line_held: bool,
    /// The left button went down over no hot-spot (`AktIkon` 0, so no screen grabs the pointer) and is still held.
    bar_free_hold: bool,
    /// The top bar's tooltip (`window/pointer.rs`).
    bar_tip: crate::kit::Tooltip,
    gpu: Option<Gpu>,
    /// The browser build's GPU while the page's adapter and device requests run.
    #[cfg(target_arch = "wasm32")]
    gpu_wait: Option<GpuWait>,
    view: Option<View>,
    clock: TickClock,
    start: Instant,
    vsync: bool,
    /// The display settings as last applied (crate::display).
    display: crate::display::Display,
    /// ADR 0058: the keys the player bound, as last applied.
    bindings: crate::bindings::Bindings,
    /// ADR 0058: the extra speeds' sim ticks per tick (1, 2 or 4) at the fastest speed in single player.
    turbo: u32,
    /// ADR 0035: the fleets on Ctrl/Alt+1..9, kept beside the save.
    fleet_groups: crate::fleet_groups::Groups,
    /// Ours (ADR 0065): the modern starmap HUD's menus, the console button it holds, the buttons pressed on it and
    /// the acts of its last frame.
    modern: crate::starmap_modern::State,
    modern_hold: Option<(u32, bool)>,
    gui_held: u8,
    modern_acts: Vec<crate::starmap_modern::Act>,
    /// Ours (ADR 0070): the modern colony screen's state and the acts of its last frame.
    colony_modern: crate::colony_modern::State,
    colony_acts: Vec<crate::colony_modern::Act>,
    info_modern: crate::info_modern::State,
    info_acts: Vec<crate::info_modern::Act>,
    messages_modern: crate::messages_modern::State,
    messages_acts: Vec<crate::messages_modern::Act>,
    diplomacy_modern: crate::diplomacy_modern::State,
    diplomacy_acts: Vec<crate::diplomacy_modern::Act>,
    spy_modern: crate::spy_modern::State,
    spy_acts: Vec<crate::spy_modern::Act>,
    trade_modern: crate::trade_modern::State,
    trade_acts: Vec<crate::trade_modern::Act>,
    /// Ours (ADR 0071): the modern production view's and research screen's state, the acts of their last frame and the
    /// count button held (line, slot, plus).
    production_modern: crate::production_modern::State,
    production_acts: Vec<crate::production_modern::Act>,
    production_hold: Option<(usize, usize, bool)>,
    research_modern: crate::research_modern::State,
    research_acts: Vec<crate::research_modern::Act>,
    /// Ours (ADR 0077): where the modern fleet and transfer panels put the original's controls; the modern battle
    /// HUD's top bar.
    fleet_modern: crate::fleet_modern::State,
    design_modern: crate::design_modern::State,
    design_acts: Vec<crate::design_modern::Act>,
    battle_bar: crate::starmap_modern::State,
    /// `IG2_PROFILE_TOUR`'s keys (crate::prof).
    tour: Option<crate::prof::Tour>,
    cursor: [f32; 2],
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
    /// The starmap was up on the last frame (its next entry is a `Starmap_Belep`).
    on_starmap: bool,
    panel_models: Option<PanelModels>,
    panel_held: Option<bool>,
    /// The info and messages screens' 3D frames.
    frame_models: Option<crate::screen_frames::FrameModels>,
    /// `TargetXStart/YStart/Zoom` with the frames left (`Mozgasd`), and the view `StarmapBeZoomol` remembered.
    cam_tween: Option<([f32; 5], i32)>,
    cam_saved: Option<[f32; 3]>,
    /// The radial menu, the planet's long info and the select menus (re/screens/starmap.md §14–§17).
    menus: Menus,
    /// The research screen (re/screens/research.md): its state, models and the techs' info texts.
    research: Option<crate::research::Research>,
    /// `AktPlanet` (exe 0xb9933c) once a planet was picked on the starmap or shown (none = the saved one).
    akt_planet: Option<usize>,
    /// The research screen's place while it is down (`ResearchTerulet`, `Terulet_Akt`, ..; none = the save's).
    research_place: Option<crate::research::Place>,
    research_models: Option<crate::research::ResearchModels>,
    tech_infos: Vec<[Vec<u8>; 3]>,
    /// The research screen's tooltip texts by hot-spot (`Research_MouseInit`).
    research_tips: Vec<Vec<u8>>,
    /// The design screen (re/screens/design.md), its 3D scene and tooltip texts (`Design_MouseInit`).
    design: Option<crate::design::Design>,
    design_scene: Option<crate::design_scene::DesignScene>,
    design_tips: Vec<Vec<u8>>,
    /// The trade screen (re/screens/trade.md), its 3D scene and tooltip texts (`Trade_MouseInit`).
    trade: Option<crate::trade::Trade>,
    trade_scene: Option<crate::trade_scene::TradeScene>,
    /// The trade screen left for the options screen (Esc): `Trade_Belep` with `OldKep` 0xd follows "back".
    trade_under_options: Option<crate::trade::Trade>,
    trade_tips: Vec<Vec<u8>>,
    /// Sound and music (re/screens/audio.md), and the screens seen up last frame (their enter/exit sounds).
    audio: crate::audio::Audio,
    sound_overlay: Option<crate::ui::Screen>,
    sound_spy: Option<bool>,
    sound_colony: bool,
    /// ADR 0036: the spoken message line's channel and text while its voice plays (the subtitles option).
    subtitle: Option<(usize, Vec<u8>)>,
    /// The sounds the window's input code started this frame (re/screens/audio.md §6), played by `audio_frame`.
    sounds: Vec<crate::audio::SoundId>,
    /// The speakers' lines the window's input code started this frame (re/screens/audio.md §5).
    speech: Vec<crate::audio::speech::Speak>,
    /// A headless script run (`--snapshot --script` on the starmap or colony): its renderer, band and clock.
    headless: Option<Headless>,
    /// The fullscreen video playing (re/screens/video.md): the game waits under it; then the names queued after it
    /// (`MEGHALAS`'s two clips; a leading `!` forces one past the cut-scene option), and what follows the last.
    video: Option<crate::video::playback::Playback>,
    video_queue: std::collections::VecDeque<String>,
    after_video: AfterVideo,
    /// Ours (ADR 0034): the finished game's record the main menu shows once the end videos are over.
    stats_end: Option<crate::stats_screen::Match>,
    /// `--stage-win DAYS` (`crate::Options::stage_win`).
    stage_win: Option<u32>,
    /// `s_anims.txt` (the story videos' names), read on the first request.
    anim_names: Option<ig2_formats::strings::KeyedStrings>,
    /// The messages screen's talking-head clip, `MesKellAnimInditas`, and `uzenetanim_<EN>.txt` once read.
    head: crate::video::message::HeadClip,
    head_armed: bool,
    /// `MessagesGyorsito` (`FRISSIT_Messages2`): the speech's end is looked for every 11th UI tick.
    mes_gyorsito: u32,
    clip_lists: Option<Vec<ig2_formats::dialogue::ClipGroup>>,
    /// The battle screen (`AktKep` 11) while it is up.
    spacewar: Option<crate::spacewar_live::Live>,
    /// The battle screen while the options screen opened from it is up (`OptionsElottiKep` 0xb): the battle goes on
    /// under it, the starmap shows behind Options (`GFX_Starmap(); SaveScreen(2)`).
    spacewar_parked: Option<crate::spacewar_live::Live>,
    /// Multiplayer: this machine's 0x44d for the running battle is on its way (`mp_battle_check`).
    mp_left_sent: bool,
    /// The Options screen while it is up (re/screens/options.md), over the main menu (load only) or the game.
    options: Option<OptionsUp>,
    help: Option<HelpUp>,
    /// `OldTopicSel` and the line table across visits.
    help_memory: crate::help::Memory,
    /// The game behind the overlay screens (crate::backdrop, ADR 0015).
    backdrop: crate::backdrop::Backdrop,
    /// What Options needs to start and to load a slot: its rows (from the user's exe), the settings file, the
    /// save's companions.
    opt_rows: Vec<crate::options::Row>,
    settings: ig2_formats::options::Options,
    /// ADR 0032: the last day the autosave saw and the new days since the last autosave (or the start or a load).
    autosave: ((i32, i32, i32), i32),
    /// ADR 0017: the name tables in the game's language (`OneLanguage`, `--texts`).
    one_language: bool,
    /// Ours (ADR 0031): the Language row's value at start, and `--lang`/`IG2_LANG` set (the row greyed).
    lang_at_start: i32,
    lang_forced: bool,
    rules: &'static ig2_sim::game::GameRules,
    tech_defaults: Vec<u8>,
    volatile: Vec<(u32, Vec<u8>)>,
    /// The pointer in physical window pixels (the battle screen's edge scrolling reads the window, not the frame).
    cursor_px: [i32; 2],
    /// The mouse buttons held (bit 0 left, bit 1 right): `Mouse_Gomb` stops the colony's edge scroll.
    mouse_held: u8,
    /// `MainCallBackEventHandler`'s held bytes (0xdb467e/f: bit 0 left, bit 1 right) over every route, and the buttons
    /// whose release a middle click already gave (their real release is no edge).
    buttons_down: u8,
    released_early: u8,
    /// The pointer is over the window (a `CursorMoved` came, no `CursorLeft` since): the edge scroll reads it.
    pointer_in: bool,
    /// The edge scroll may read `cursor_px`. In play it starts at (0, 0) as `Mouse_Phys_x/y` does (written only by
    /// SDL_MOUSEMOTION, `MainCallBackEventHandler` exe 0x4e34e1; nothing warps the pointer), so the starmap scrolls
    /// up-left until the first move; a headless script run waits for its first move.
    pointer_seen: bool,
    /// The window's pointer is hidden (the colony screen draws its own).
    cursor_hidden: bool,
    /// `Mouse_FelengedesMajdMegvar` (a rename's start on the starmap or the colony screen): the releases wait until
    /// both buttons are up and do nothing.
    wait_release: bool,
    /// `TamadasSzovegCounter` (exe 0x9ABAE0) while no colony screen is up: `StartGrWar` sets 900, the colony screen
    /// takes it on entry (it counts down only there) and `Colony_Kilep` zeroes it.
    attack_banner: i32,
    /// The colony camera's globals as last left (`CameraKeys::reentered`): they outlive the screen.
    cam_keep: crate::colony::CameraKeys,
    /// `GrWarOldKep`, `GrWarOldPlanet`: the screen and planet a single-player real-time ground war took the player from
    /// (`StartGrWar` exe 0x4a4a70); `GroundWarFinished` goes back to them (the sim restores the pause).
    grwar_old: Option<(i32, usize)>,
    /// `VoltHarc` (exe 0x99646a): the shown planet's ground war as `Frissit2_Colony` saw it last (new game: false).
    volt_harc: bool,
    /// A running tutorial (re/rules/tutorial.md), its replayed pointer (virtual pixels) and buttons.
    tutorial: Option<crate::tutorial_ui::TutorialUi>,
    tutorial_at: [i32; 2],
    tutorial_buttons: (bool, bool),
    /// `Mouse_Tipus` of the replayed pointer (−2 while parked: none drawn).
    tutorial_kind: i32,
    /// The starmap's held camera flags and edge-scroll speeds (re/screens/starmap.md §10.6).
    star_keys: crate::starmap::CamKeys,
    /// A console hot-spot (1..=7, `STAR_M.RAW`'s colours) held (`Mouse_Megfogva`), with the left button?
    console_held: Option<(u32, bool)>,
    /// The console buttons' press animation (`MelyikGombot`, `GombFazis`, `KezeloGombFrame`).
    star_buttons: crate::starmap_hud::ConsoleButtons,
    /// O on the help screen: Options opens once Help's closing has run out.
    help_then_options: bool,
    /// `STAR_M.RAW` (kep 0x1f): the console's and the minimap's hot-spot colours, 640 × 480 (loaded on first use).
    star_mask: Option<Vec<u8>>,
    /// `DesignKeret1`'s globals (`dkm`, `dkmcel`, `GfxNovelo`), stepped every UI frame whatever the screen, and the
    /// screen whose row frame carried them last (re/screens/trade.md §13).
    dk: crate::kit::DesignFrame,
    dk_owner: u8,
    /// Ours (ADR 0080): Discord Rich Presence, and the race names its texts use (read on the first report).
    discord: crate::discord::Presence,
    discord_races: Vec<String>,
}

/// The Options screen up: its state, its 10 ms UI clock, the lasers' glow, and `OptionsOldIdoPause`.
/// The Help screen up (`AktKep` 1, re/screens/help.md) over whatever screen F1 came from, which comes back after.
struct HelpUp {
    s: crate::help::HelpScreen,
    ui_ms: u32,
    glow: Option<crate::render::TexId>,
    pics: crate::help::Pictures,
    /// Ours (ADR 0078): the modern help page's state.
    page: crate::help_modern::Page,
}

struct OptionsUp {
    s: crate::options::OptionsScreen,
    ui_ms: u32,
    glow: Option<crate::render::TexId>,
    was_paused: bool,
}

/// What follows the last queued video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AfterVideo {
    /// Back to the game (or the main menu) as it was.
    #[default]
    Resume,
    /// The game ended (`MEGHALAS`, `MEGNYERES`): `AktKepKell = 0xF`, the main menu.
    MainMenu,
    /// The title before the main menu: `PlayMusicMenu(false, true)` once it ends.
    MenuMusic,
    /// A new campaign's intro (`CampaignGameStart`): then `Starmap_Belep`'s `PlayMusicMain(0, 0)` and the start voice
    /// of campaign type N.
    StarmapEntry(i32),
}

/// The renderer of a run without a window, its band, and the script's clock (one 10 ms UI tick a frame; the sim
/// held, as the exe's script clock holds it: re/screens/sound.md §3).
struct Headless {
    renderer: Renderer,
    band: (u32, u32),
    ms: u32,
    /// The sim runs on the script's clock (a ground war's run); else it is held.
    sim: bool,
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

/// Our own icon (`crate::icon`) on the title bar and, on Windows, the taskbar and Alt+Tab.
pub fn with_icon(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    let icon = |n: u32| winit::window::Icon::from_rgba(crate::icon::rgba(n), n, n).ok();
    #[cfg(windows)]
    let attrs = winit::platform::windows::WindowAttributesExtWindows::with_taskbar_icon(attrs, icon(64));
    attrs.with_window_icon(icon(32))
}

pub fn run(o: crate::Options) -> Result<(), String> {
    let l = crate::load(&o)?;
    // The rules live for the whole process; `World` borrows them.
    let rules: &'static ig2_sim::game::GameRules = Box::leak(Box::new(l.rules));
    let tables: &'static ig2_rules::Rules = Box::leak(Box::new(l.tables));
    let mut world = crate::start_world(&o, &l.install, l.save.as_ref(), rules, &l.tech_defaults, &l.volatile)?;
    // As `App::start_world` gives a game loaded from the menu (`_s(0x3b, n)`: the AI's design names).
    if world.game.design_texts.is_empty() {
        world.game.design_texts = crate::new_game::design_texts(&l.install);
    }
    crate::prof::mark("world");
    world.set_speed(o.speed);
    let exe = l.install.read_raw("ig2.exe").map_err(|e| e.to_string())?;
    let sounds = ig2_rules::sounds::SoundTables::from_exe(&exe).map_err(|e| e.to_string())?;
    // The same files: a second `Install::open` walked the whole install again (docs/perf.md).
    let audio_install = l.install.clone();
    let mut audio = crate::audio::Audio::new(audio_install, sounds, true);
    audio.set_speech(&ig2_rules::speech::Speech::from_exe(&exe).unwrap_or_default());
    crate::prof::mark("audio");
    if !audio.has_device() {
        eprintln!("ig2-client: no sound device; running silent");
    }
    // The client starts on the main menu (`MainScr_Belep`'s `PlayMusicMenu(false, true)`), or with `--save` on the
    // starmap (`Starmap_Belep`'s `PlayMusicMain(0, 0)`); nothing plays yet: at once.
    // On the main menu the title plays first (`StartDemoMode` exe 0x4fe3c0: once per run, unless the cut-scenes are
    // off) and the menu's music follows it.
    let title = o.main_menu && o.watch.is_none() && crate::video::playback::cutscenes_enabled();
    let music = if o.main_menu { crate::audio::Music::Menu } else { crate::audio::Music::Main };
    // A new campaign enters the starmap after its intro (re/screens/mainscr.md §9.1).
    let start = world.start_voice.take();
    if !title && start.is_none() {
        audio.play_music(music, o.main_menu, human_race(&world.game));
    }
    // The option rows as the exe holds them, their values from the settings file (the registry `SaveOptions` fills).
    let mut opt_rows = ig2_rules::PeImage::parse(&exe).map(|img| crate::options::read_rows(&img, &crate::ui::UiTexts::load(&l.install))).unwrap_or_default();
    let mut app = App::new(world, l.install, tables, rules, audio);
    crate::prof::mark("app");
    for r in &mut opt_rows {
        if let Some(v) = app.settings.int(&r.key) {
            r.value = v;
        }
    }
    (app.opt_rows, app.tech_defaults, app.volatile) = (opt_rows, l.tech_defaults, l.volatile);
    app.apply_battle_rows();
    app.front = o.main_menu.then(|| Front::new(None, 0, &app.settings));
    app.stage_win = o.stage_win;
    if o.multiplayer && app.front.is_some() {
        app.front_act(crate::mainscr::Act::MultiServer);
    }
    app.backdrop.live = o.background.unwrap_or(app.settings.int("LiveBackground") != Some(0));
    app.one_language = o.texts.unwrap_or(app.settings.int("OneLanguage") != Some(0));
    app.lang_at_start = app.settings.int(crate::display::LANGUAGE).unwrap_or(0);
    app.lang_forced = crate::forced_language(&o).is_some();
    // The display rows' settings (crate::display), `IG2_VSYNC` and `--letterbox` over them.
    app.display = crate::display::Display::from_settings(&app.settings);
    app.bindings = crate::bindings::Bindings::from_settings(&app.settings);
    app.audio.voice_volume = crate::display::voice_volume(&app.settings);
    app.vsync = std::env::var("IG2_VSYNC").map_or(app.display.vsync, |v| v != "0");
    crate::kit::layout::set_letterbox(o.letterbox || app.display.letterbox);
    crate::kit::layout::set_box_models(app.display.box_models);
    crate::kit::layout::set_slider_under(app.display.slider_under);
    crate::diplomacy::set_articles(app.display.articles);
    crate::kit::layout::set_centred_exit(app.display.centred_exit);
    crate::starmap::set_colour_blind(app.display.colour_blind);
    crate::render::set_sharp_ui(o.sharp_ui || app.display.sharp_ui);
    crate::render::set_sharp_text(o.sharp_text || app.display.sharp_text);
    crate::theme::set(o.theme.as_deref().or(app.settings.get(crate::display::UI_THEME)).unwrap_or(""));
    crate::upscale::set_factor(o.upscale.max(app.display.upscale));
    if !o.modern_lighting {
        crate::render3d::set_lighting(app.display.lighting);
    }
    crate::battle_fx::set_enhanced(o.battle_effects || app.display.battle_effects);
    crate::render::set_smooth_glows(o.smooth_glows || app.display.smooth_glows);
    crate::gui::ds::cursor::set_modern(app.display.modern_cursors);
    crate::gui::set_ui_scale(o.ui_scale.unwrap_or(crate::display::ui_scale(&app.settings)));
    // `IG2_PROFILE_BATTLE=DUMP`: a battle capture's dump goes live in slot 0 and the screen opens (crate::prof).
    if let Ok(p) = std::env::var("IG2_PROFILE_BATTLE") {
        crate::spacewar_screen::go_live(&mut app.world, crate::spacewar_screen::Dump::load(&p)?.battle);
        app.world.realtime.push(ig2_sim::game::RealTimeEvent::SpaceStarted { slot: 0 });
    }
    if let Some(n) = o.tutorial.filter(|_| o.main_menu) {
        app.front_start(crate::mainscr::Start::Tutorial(n));
    } else if title {
        app.queue_videos(["title.bik".to_string()], false, AfterVideo::MenuMusic);
    }
    if let Some(p) = o.watch.as_deref().filter(|_| o.main_menu) {
        app.watch_replay(p);
    }
    if let Some(c) = start {
        app.take_video_requests();
        app.after_video = AfterVideo::StarmapEntry(c);
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    crate::prof::mark("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    // The browser owns the loop: it runs the app after this returns.
    #[cfg(target_arch = "wasm32")]
    {
        winit::platform::web::EventLoopExtWebSys::spawn_app(event_loop, app);
        Ok(())
    }
    #[cfg(not(target_arch = "wasm32"))]
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
    let mut world = crate::start_world(o, &l.install, l.save.as_ref(), rules, &l.tech_defaults, &l.volatile)?;
    world.set_speed(o.speed);
    let exe = l.install.read_raw("ig2.exe").map_err(|e| e.to_string())?;
    let sounds = ig2_rules::sounds::SoundTables::from_exe(&exe).map_err(|e| e.to_string())?;
    // The same files: a second `Install::open` walked the whole install again (docs/perf.md).
    let audio_install = l.install.clone();
    let mut audio = crate::audio::Audio::new(audio_install, sounds, false);
    audio.set_speech(&ig2_rules::speech::Speech::from_exe(&exe).unwrap_or_default());
    let mut renderer = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let mut hud = crate::ui::Hud::load(&mut renderer, &l.install, tables);
    let mut app = App::new(world, l.install, tables, rules, audio);
    // The exe's script runs start with the real pointer wherever it is: no edge scroll before the script's first move.
    app.pointer_seen = false;
    // The exe's controls: the scripts' right presses give their orders on the press (ADR 0049 off).
    app.display.starmap_mouse = false;
    // The original's HUD: the scripts click its hot-spots (ADR 0065's panels need the window).
    app.display.modern_starmap = false;
    // The original's Options screen: the scripts click its hot-spots (ADR 0067's page needs a pointer of its own).
    app.display.modern_menu = false;
    // The original's colony panels: the scripts click their hot-spots (ADR 0070's need the window).
    app.display.modern_colony = false;
    // The original's research screen and production view: the scripts click their hot-spots (ADR 0071's need the window).
    app.display.modern_research = false;
    // The original's design screen: the scripts click its hot-spots (ADR 0075's need the window).
    app.display.modern_design = false;
    app.world.game.empire_names = hud.empire_names.clone();
    hud.set_one_language(&app.install, o.texts == Some(true));
    app.hud = Some(hud);
    app.view = Some(View::new(&app.world.curr, &app.world.view, o.size, 1.0));
    let sel = app.selection();
    if sel.mode == 1 || sel.mode == 2 {
        app.select(sel);
    }
    // A ground war (`--colony-ui war`): the sim runs on the script's clock as in the exe's script run, and each grab
    // draws the colony screen (`<out>-t<T>.png`).
    let war = o.colony.is_some() && o.colony_ui.as_deref().is_some_and(|m| m.starts_with("war"));
    app.headless = Some(Headless { renderer, band: o.size, ms: 0, sim: war });
    // `--dkm`: the row frame's global phase at the script's start (the capture's `ui.json` "dkm").
    if let Some([m, c, n]) = o.dkm {
        app.dk = crate::kit::DesignFrame::with_phase(m, c, n);
    }
    if let Some(p) = o.colony {
        app.open_colony(p);
        // The camera of the exe's state save (`--colony-live`, as `colony_snapshot`) or `--colony-cam`.
        let f = |n: &str| crate::save_f32(l.save.as_ref(), n);
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
    // `screen.js` holds the exe's physical pointer at the script's point (its `--mouse` until the first move).
    app.cursor_px = px(o.mouse.unwrap_or([320.0, 240.0])).map(|v| v as i32);
    for tick in 0..=script.end() {
        if tick > 0 {
            if let Some(h) = &mut app.headless {
                h.ms = tick * 10;
            }
            app.frame();
        }
        if war && script.grabs.contains(&tick) {
            app.colony_grab(&crate::script::grab_path(out, tick))?;
        }
        for e in script.at(tick) {
            // The capture writes the script's point to the physical pointer too (`screen.js`), which the edge scroll reads.
            if let Input::Move(at) | Input::Button { at, .. } = &e.input {
                app.cursor_px = px(*at).map(|v| v as i32);
            }
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
                        f if f.len() > 1 && f.starts_with('f') && f[1..].parse::<usize>().is_ok_and(|n| (1..=11).contains(&n)) => {
                            const F: [NamedKey; 11] = [
                                NamedKey::F1, NamedKey::F2, NamedKey::F3, NamedKey::F4, NamedKey::F5, NamedKey::F6, NamedKey::F7, NamedKey::F8,
                                NamedKey::F9, NamedKey::F10, NamedKey::F11,
                            ];
                            Key::Named(F[f[1..].parse::<usize>().unwrap_or(1) - 1])
                        }
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
    if !script.grabs.is_empty() && !war {
        println!("ig2-client: --script on the starmap/colony draws no grabs");
    }
    let path = crate::script::write_sounds(out, &heard)?;
    println!("{}: {} sounds", path.display(), heard.len());
    Ok(())
}

/// The egui pages' theme (ADR 0062): the UI theme's look of the HUD's font and palette.
fn page_theme(h: &crate::ui::Hud, size: [f32; 2]) -> crate::theme::Theme {
    h.font.as_ref().map_or_else(crate::theme::Theme::modern, |font| {
        crate::theme::look(&crate::options::Draw { font, white: h.white, glow: h.white, v: crate::font::Virtual::new(size), palette: &h.palette, texts: &h.texts, now_ms: 0.0 })
    })
}

/// The Options screen over `frame` (after the 3D behind it).
fn draw_options(o: &mut OptionsUp, h: &crate::ui::Hud, frame: &mut crate::render::Frame, size: [f32; 2], now_ms: f64) {
    let Some(font) = &h.font else { return };
    let d = crate::options::Draw { font, white: h.white, glow: o.glow.unwrap_or(h.white), v: crate::font::Virtual::new(size), palette: &h.palette, texts: &h.texts, now_ms };
    o.s.draw(frame, &d);
}

/// The main menu's 2D (`MainScrGfx2D_1/_2`): the scenario list, the name entry.
/// Ours: the statistics screen over it (ADR 0034), and its "History" item while `history` is on.
#[allow(clippy::too_many_arguments)]
fn draw_front_2d(fr: &mut Front, h: &crate::ui::Hud, r: &mut crate::render::Renderer, install: &ig2_formats::Install, frame: &mut crate::render::Frame, size: [f32; 2], now_ms: f64, history: bool) {
    crate::mainscr_scene::draw_2d(&mut fr.s, &mut fr.scenarios, h, r, install, frame, size, now_ms);
    let Some(font) = &h.font else { return };
    let d = crate::options::Draw { font, white: h.white, glow: h.white, v: crate::font::Virtual::new(size), palette: &h.palette, texts: &h.texts, now_ms };
    if let Some(st) = &fr.stats {
        // Ours (ADR 0078): the modern page draws it with the modern main menu.
        if !fr.s.modern {
            st.draw(frame, &d);
        }
    } else if let Some(rl) = &fr.replays {
        rl.draw(frame, &d);
    } else if front_history_shown(fr) {
        if history {
            crate::stats_screen::draw_history_item(frame, &d, crate::kit::inside_f(crate::stats_screen::HISTORY_ITEM, fr.at), fr.s.gt);
        }
        crate::replay_viewer::draw_item(frame, &d, crate::kit::inside_f(crate::replay_viewer::REPLAYS_ITEM, fr.at), fr.s.gt);
    }
}

/// The main menu's "History" and "Replays" items are up (ours, ADR 0034, 0038): the main ring set at rest, as the
/// Settings label.
fn front_history_shown(fr: &Front) -> bool {
    fr.stats.is_none() && fr.replays.is_none() && fr.mods.is_none() && fr.mp.is_none() && !fr.s.flow && fr.s.at_rest()
}

/// `PlayerFaj[EN]`, the music's race: a mod's race plays its original's (ADR 0069 §13).
pub(crate) fn human_race(game: &ig2_sim::game::Game) -> u8 {
    let en = game.state.mem.i32(ig2_sim::save::va::EN).max(0) as u32;
    game.rules.ai.design.race_role(game.state.mem.u8(ig2_sim::save::va::PLAYER_FAJ + en))
}

impl App {
    fn new(world: World<'static>, install: ig2_formats::Install, tables: &'static ig2_rules::Rules, rules: &'static ig2_sim::game::GameRules, audio: crate::audio::Audio) -> Self {
        let start = Instant::now();
        App {
            front: None,
            world,
            install,
            tables,
            maps: &rules.ground_maps,
            colony: None,
            hud: None,
            overlay: None,
            spy: None,
            spy_scene: None,
            ui_frames: 0,
            sent_transfer: None,
            messages_auto: 0,
            diplomacy: None,
            dip_next: None,
            kep_kell: None,
            planet_kell: None,
            quake: None,
            quake_rand: crate::audio::Rand::seeded(),
            mp_wait: Default::default(),
            bar_held: None,
            bar_line_held: false,
            bar_free_hold: false,
            bar_tip: Default::default(),
            dip_input: Default::default(),
            gpu: None,
            #[cfg(target_arch = "wasm32")]
            gpu_wait: None,
            view: None,
            clock: TickClock::new(SIM_TICK_MS, 0),
            start,
            vsync: std::env::var("IG2_VSYNC").map_or(true, |v| v != "0"),
            display: crate::display::Display::from_settings(&ig2_formats::options::Options::default()),
            bindings: Default::default(),
            turbo: 1,
            fleet_groups: Default::default(),
            modern: Default::default(),
            modern_hold: None,
            gui_held: 0,
            modern_acts: Vec::new(),
            colony_modern: Default::default(),
            info_modern: Default::default(),
            info_acts: Vec::new(),
            messages_modern: Default::default(),
            messages_acts: Vec::new(),
            diplomacy_modern: Default::default(),
            diplomacy_acts: Vec::new(),
            spy_modern: Default::default(),
            spy_acts: Vec::new(),
            trade_modern: Default::default(),
            trade_acts: Vec::new(),
            colony_acts: Vec::new(),
            production_modern: Default::default(),
            production_acts: Vec::new(),
            production_hold: None,
            research_modern: Default::default(),
            research_acts: Vec::new(),
            fleet_modern: Default::default(),
            design_modern: Default::default(),
            design_acts: Vec::new(),
            battle_bar: Default::default(),
            tour: crate::prof::Tour::from_env(),
            cursor: [0.0; 2],
            left_down: None,
            right_down: None,
            info_down: false,
            modifiers: ModifiersState::empty(),
            title_at: start,
            frames: 0,
            panels: Panels::default(),
            on_starmap: false,
            panel_models: None,
            panel_held: None,
            frame_models: None,
            cam_tween: None,
            cam_saved: None,
            menus: Menus::default(),
            research: None,
            research_place: None,
            akt_planet: None,
            research_models: None,
            tech_infos: Vec::new(),
            research_tips: Vec::new(),
            design: None,
            design_scene: None,
            design_tips: Vec::new(),
            trade: None,
            trade_scene: None,
            trade_under_options: None,
            trade_tips: Vec::new(),
            audio,
            sound_overlay: None,
            sound_spy: None,
            sound_colony: false,
            subtitle: None,
            sounds: Vec::new(),
            speech: Vec::new(),
            headless: None,
            spacewar: None,
            spacewar_parked: None,
            mp_left_sent: false,
            options: None,
            help: None,
            help_memory: Default::default(),
            backdrop: crate::backdrop::Backdrop::new(true),
            opt_rows: Vec::new(),
            settings: ig2_formats::options::Options::load(),
            autosave: ((0, 0, 0), 0),
            one_language: false,
            lang_at_start: 0,
            lang_forced: false,
            rules,
            tech_defaults: Vec::new(),
            volatile: Vec::new(),
            video: None,
            video_queue: Default::default(),
            after_video: AfterVideo::Resume,
            stats_end: None,
            stage_win: None,
            anim_names: None,
            head: Default::default(),
            head_armed: false,
            mes_gyorsito: 0,
            clip_lists: None,
            cursor_px: [0; 2],
            mouse_held: 0,
            buttons_down: 0,
            released_early: 0,
            pointer_in: false,
            pointer_seen: true,
            cursor_hidden: false,
            wait_release: false,
            attack_banner: 0,
            cam_keep: Default::default(),
            volt_harc: false,
            grwar_old: None,
            tutorial: None,
            tutorial_at: crate::tutorial_ui::PARKED,
            tutorial_buttons: (false, false),
            tutorial_kind: -2,
            star_keys: Default::default(),
            console_held: None,
            star_buttons: Default::default(),
            help_then_options: false,
            star_mask: None,
            // The exe's boot values (`.data`): `dkm` 0, `dkmcel` π/2.
            dk: crate::kit::DesignFrame::with_phase(0.0, std::f32::consts::FRAC_PI_2, 0.0),
            dk_owner: 0,
            discord: Default::default(),
            discord_races: Vec::new(),
        }
    }

    /// A headless war script's grab: the colony screen as the frame draws it.
    fn colony_grab(&mut self, path: &std::path::Path) -> Result<(), String> {
        let (Some(h), Some(hud), Some(c), Some(prev), Some(curr)) = (&mut self.headless, &mut self.hud, &mut self.colony, &self.world.colony_prev, &self.world.colony_curr) else {
            return Ok(());
        };
        hud.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
        hud.turbo = self.turbo;
        hud.paused = self.world.paused();
        let r = &mut h.renderer;
        let size = [h.band.0 as f32, h.band.1 as f32];
        let t = h.ms as f32 / 1000.0;
        let light = c.view.panel_light(self.maps, curr.map);
        let unit = crate::colony::landing_unit(&self.world.game.state, curr, self.tables);
        let panel = c.panel.render(r, &self.install, self.tables, c.ui.kind, unit, c.ui.spin(), light, size, c.scene.sky.as_deref());
        let info_model = crate::colony_ui::render_kis_info(r, &self.install, self.tables, &c.ui, &mut c.scene, &mut c.panel, curr, self.world.curr.human as i32, light, size);
        let mut frame = c.view.build(r, &self.install, self.tables, self.maps, &mut c.scene, prev, curr, 1.0, t);
        let cursor = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
        let input = crate::ui::ColonyInput { ui: &c.ui, sim: self.world.game.rules, panel, cursor, pointer: false, info_model, now_ms: h.ms, time: t, names: true, rename: None, modern: false };
        hud.draw_colony(&mut frame, &self.world.game.state, self.tables, self.maps, &c.view, &c.scene, curr, &input);
        hud.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Colony, t);
        let rgba = r.draw_to_image(&frame, h.band, t);
        crate::assets::write_png(path, h.band.0, h.band.1, &rgba)
    }

    fn now_ms(&self) -> u32 {
        match &self.headless {
            Some(h) => h.ms,
            None => self.start.elapsed().as_millis() as u32,
        }
    }

    /// The frame's sounds: the screens' queued ones, the enter/exit sounds of the screens that came or went, the UI
    /// ticks due (`FRISSIT2_SoundSystem`) and `UpdateFrame`'s music watchdog.
    /// `AktKep == 2`: the starmap is the screen (no other screen or window over it).
    fn on_starmap(&self) -> bool {
        self.front.is_none()
            && self.colony.is_none()
            && self.overlay.is_none()
            && self.spy.is_none()
            && self.diplomacy.is_none()
            && self.research.is_none()
            && self.design.is_none()
            && self.spacewar.is_none()
            && self.options.is_none()
            && self.help.is_none()
    }

    /// The colony screen is up with nothing over it (`AktKep == 3`).
    fn on_colony(&self) -> bool {
        self.colony.is_some()
            && self.front.is_none()
            && self.overlay.is_none()
            && self.spy.is_none()
            && self.diplomacy.is_none()
            && self.research.is_none()
            && self.design.is_none()
            && self.spacewar.is_none()
            && self.options.is_none()
            && self.help.is_none()
    }

    fn send(&mut self, o: StarmapOrder) {
        self.world.send(Command::Starmap { order: StarmapCmd::from(o) });
    }

    /// The top bar's pause button (hot-spot 2, `IG2_gameUpdate`), Space and P: SP toggles `IdoPause` unless the speed is
    /// locked; MP sends 0x407 with the toggled value when this machine may set the time ([`Self::time_mp_allowed`]).
    fn toggle_pause(&mut self) {
        let mp = self.multiplayer();
        if if mp { self.time_mp_allowed() } else { !self.speed_locked() } {
            let paused = self.world.paused();
            self.send(StarmapOrder::Pause { paused: !paused });
        }
    }

    /// A speed button (hot-spots 3..=5) and the keys 1..3: SP `SetIdoSpeed(n)` and unpause, when paused or at another
    /// speed; MP 0x406 then 0x407(0) whatever the speed (`IG2_gameUpdate` exe 0x4e08e9), when it may set the time.
    fn set_speed(&mut self, level: i32) {
        if self.multiplayer() {
            if self.time_mp_allowed() {
                self.send(StarmapOrder::Speed { level });
                self.send(StarmapOrder::Pause { paused: false });
            }
        } else if self.turbo_step(level) {
            self.sounds.push(crate::audio::SoundId::global(10));
        } else if !self.speed_locked() && (self.world.paused() || self.world.game.speed_level() != level) {
            // `IG2_gameUpdate` (exe 0x4e08c6): the speed buttons' sound, single player only.
            self.sounds.push(crate::audio::SoundId::global(10));
            self.send(StarmapOrder::Speed { level });
            self.send(StarmapOrder::Pause { paused: false });
        }
    }

    /// ADR 0058: the fastest speed asked for again while it runs (single player, the Extra game speeds row on) steps
    /// the extra speeds: twice, then four times the sim ticks. Any other speed asked for ends them. True = stepped.
    fn turbo_step(&mut self, level: i32) -> bool {
        let on = self.settings.int(crate::display::EXTRA_SPEEDS) == Some(1);
        if on && level == 3 && !self.world.paused() && self.world.game.speed_level() == 3 && self.spacewar.is_none() {
            self.turbo = (self.turbo * 2).min(4);
            return true;
        }
        self.turbo = 1;
        false
    }

    /// MP pause and speed (input.md §5.2: `DoKeyboard`, `IG2_gameUpdate` exe 0x4e0831/0x4e08e9): the room lets every
    /// player set the time (`OnlyServerCanModifyTime` 0xBAE55C == 0) or this machine is the host (`SuperVisor`); no
    /// screen, `nemlehetidotallitani` or speed test (the receiver's `SetIdoSpeed`/`SetIdoPause` gate on `ProgramSebesseg`).
    fn time_mp_allowed(&self) -> bool {
        const ONLY_SERVER_CAN_MODIFY_TIME: u32 = 0xBAE55C;
        do_keyboard::mp_time_gate(self.world.game.state.mem.i32(ONLY_SERVER_CAN_MODIFY_TIME), self.world.net.as_ref().is_some_and(|(n, _)| n.is_host()))
    }

    fn frame(&mut self) {
        if self.stage_win.is_some_and(|d| self.world.game.stats.days >= d) {
            self.stage_win = None;
            let en = self.world.game.local_view().empire as u32;
            for e in (1..ig2_sim::colony_side::EMPIRES as u32).filter(|&e| e != en) {
                self.world.game.state.mem.set_u8(ig2_sim::save::va::EMPIRE_ALIVE + e, 0);
            }
        }
        self.send_renames();
        self.take_video_requests();
        self.take_story_ui();
        self.update_mp_wait();
        if self.video.is_some() || !self.video_queue.is_empty() {
            self.video_frame();
            if self.video.is_some() {
                self.mp_run_on();
                return;
            }
        }
        if self.front.is_some() {
            self.front_frame();
            return;
        }
        self.top_menus_frame();
        if self.options.is_some() {
            self.options_game_frame();
            self.mp_run_on();
            return;
        }
        if self.help.is_some() {
            self.help_game_frame();
            self.mp_run_on();
            return;
        }
        self.screen_change_step();
        let ov = self.frame_overlay();
        let now = self.now_ms();
        self.frame_sim(now);
        let ui = crate::prof::scope("ui");
        // Multiplayer: the session ended under this machine (`ExitToMainScreen` from `MULTIGET`'s disconnect).
        // A replay at its end (ADR 0038 §6).
        if self.world.viewer.as_ref().is_some_and(|v| v.end.is_some()) {
            self.replay_act(crate::replay_viewer::Act::Exit);
            return;
        }
        if self.world.net_over {
            self.world.net_over = false;
            self.exit_to_main_menu();
            // `FRISSIT_MainScr` (exe 0x4f7300): the drop's `YouAreDisconnectedVolt` box on the main menu; the joiner's
            // reason when the host is gone (`MultiPlayer::ReceiveData` 0x515910: 0xb).
            if let Some(f) = &mut self.front {
                f.s.disconnected = Some(0xb);
            }
            return;
        }
        if let Some(s) = self.world.fix_selection() {
            self.select(s);
        }
        self.spacewar_update(now);
        let sel = self.selection();
        let acts = self.panels.advance(&self.world.game.state, &sel, now as f32 / 1000.0);
        self.apply(acts);
        if let Some(i) = &mut self.panels.info {
            let zoom = self.view.as_ref().map_or(0.0, |v| v.camera.zoom);
            let ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50) as u32;
            i.advance(&self.world.game.state, &sel, now as f32 / 1000.0, zoom, ui);
        }
        // The panel's fleets are spared `FRISSIT_Starmap`'s 0x1b while it is up.
        self.sync_transfer();
        // `Mozgasd`: the camera halves its way to the target for 8 frames.
        if let (Some((t, n)), Some(v)) = (self.cam_tween, &mut self.view) {
            let c = &mut v.camera;
            if n > 1 {
                c.x += (t[0] - c.x) / 2.0;
                c.y += (t[1] - c.y) / 2.0;
                c.zoom += (t[2] - c.zoom) / 2.0;
                c.rotate += (t[3] - c.rotate) / 2.0;
                c.pitch += (t[4] - c.pitch) / 2.0;
                self.cam_tween = Some((t, n - 1));
            } else {
                (c.x, c.y, c.zoom, c.rotate, c.pitch) = (t[0], t[1], t[2], t[3], t[4]);
                self.cam_tween = None;
            }
        }
        // The message line runs on the 100 Hz UI clock (`FRISSIT_Scroll2`), also while paused.
        let due_ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50);
        self.ui_frames = now as u64 / 10;
        // `Frissit2_Colony` (exe 0x4de8e8, before the tutorial's part) on a game screen: the dropships.
        if self.front.is_none() && self.shown_kep() <= 0xd {
            for _ in 0..due_ui {
                self.world.game.colony_ui_tick();
            }
            // Paused (`StartGrWar` pauses single player) no sim tick reads the colony again: the dropships coming down
            // are shown as they move (re/rules/ground-combat.md §11.16).
            if due_ui > 0 && self.world.paused() && self.world.colony.is_some() {
                self.world.show_colony(self.world.colony);
            }
        }
        // `Fake_FRISSITES`' tutorial part per UI tick (re/rules/tutorial.md §2).
        for _ in 0..due_ui {
            if self.tutorial.is_none() || self.front.is_some() {
                break;
            }
            self.tutorial_tick();
        }
        if self.front.is_some() {
            return;
        }
        self.frame_ui_steps(now, due_ui, sel);
        let paused = self.world.paused();
        let alpha = self.world.draw_phase(self.clock.phase(now));
        drop(ui);
        let names = self.building_names();
        // The pictures a screen change takes, and this frame's picture under an overlay screen.
        self.backdrop_switch();
        let backdrop_time = crate::prof::scope("backdrop");
        let backdrop = if self.overlay.is_some() || (self.spy.is_some() && self.spacewar.is_none()) { self.backdrop() } else { None };
        drop(backdrop_time);
        let kep = self.tutorial_kep();
        // Ours (ADR 0065): the modern HUD's bar, card, console and minimap, its menus and long info.
        let modern_bar = self.modern_up();
        let modern_hud = modern_bar;
        let mut mv = None;
        let colony_modern = self.colony_modern_up();
        let mut cmv = None;
        let info_modern = self.info_modern_up();
        let mut imv = None;
        let messages_modern = self.messages_modern_up();
        let mut mmv = None;
        let mut dipv = None;
        let dip_modern = self.diplomacy_modern_up();
        let spy_modern = self.spy_modern_up();
        let trade_modern = self.trade_modern_up();
        let mut tmv = None;
        // Ours (ADR 0071): the production view's and the research screen's modern panels; the original's production
        // view (its 3D buttons, faders and texts) is not drawn under them.
        let production_modern = self.production_modern_up();
        let mut pmv = None;
        let research_modern = self.research_modern_up();
        let mut rmv = None;
        let design_modern = self.design_modern_up();
        let mut dmv = None;
        let classic_panels = !(self.panels.modern && self.panels.transfer.is_none() && self.panels.info.is_none());
        // Ours (ADR 0077): the fleet info and transfer panels' modern ones; the original's models are cut into them.
        let fleet_modern = self.fleet_modern_up();
        let mut fmv = None;
        let (Some(g), Some(v)) = (&mut self.gpu, &mut self.view) else { return };
        let t = now as f32 / 1000.0;
        let size = { let (w, h) = g.band(); [w as f32, h as f32] };
        let cam = v.camera.matrix();
        let offscreen = crate::prof::scope("offscreen");
        let panel_tex = match (&mut self.panel_models, self.colony.is_none() && self.overlay.is_none() && self.panels.active() && classic_panels) {
            (Some(m), true) => m.render(&mut g.renderer, &self.install, self.tables, &self.panels, &self.world.game.state, &sel, &cam, size),
            _ => None,
        };
        // Ours (ADR 0071): the modern production view's slot pictures.
        let thumbs = match (&mut self.panel_models, production_modern && self.colony.is_none() && self.overlay.is_none()) {
            (Some(m), true) => m.render_thumbs(&mut g.renderer, &self.install, self.tables, &self.world.game.state, &sel, &cam, size, self.ui_frames as f32),
            _ => None,
        };
        // `StarmapFrissitBolygoUrhajok` turns the globe once a drawn frame; one a UI tick here.
        let globe = match (&mut self.panel_models, self.colony.is_none() && self.overlay.is_none()) {
            (Some(m), true) => m.render_globe(&mut g.renderer, &self.install, &self.world.game.state, &sel, &cam, size, self.ui_frames as f32),
            _ => None,
        };
        if self.spacewar.is_some() {
            // Ours (ADR 0077): the modern battle HUD with the modern starmap HUD's option.
            let modern = self.display.modern_starmap;
            if let Some(sw) = &mut self.spacewar {
                match modern {
                    true => _ = sw.modern.get_or_insert_with(Default::default),
                    false => sw.modern = None,
                }
            }
            let Some(mut frame) = self.spacewar_frame(now, alpha, size, t, globe) else { return };
            let (Some(g), Some(h)) = (&mut self.gpu, &mut self.hud) else { return };
            h.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
            h.turbo = self.turbo;
            h.paused = paused;
            h.modern_bar = modern;
            h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Starmap, t);
            // The tutorial's window over the battle too (`GFX_DateMoneyScrollFade` pass 3, every `AktKep` but 0xd).
            if let Some(tu) = &self.tutorial {
                h.draw_tutorial(&mut frame, tu, size, 11);
            }
            let mut top = None;
            let mut bar_acts = Vec::new();
            if let (true, Some(sw)) = (modern, &mut self.spacewar) {
                if let Some(b) = self.world.game.realtime.slots.get(sw.ui.slot) {
                    let bv = h.battle_modern_view(&sw.ui, b);
                    let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                    let mut bar = h.bar_view(&self.world.game.state, self.tables, &self.world.ui, self.world.curr.human, gates, t, 0);
                    bar.bind_keys(&self.bindings);
                    let style = crate::starmap_modern::style(h, size);
                    let (st, bs, band) = (sw.modern.get_or_insert_with(Default::default), &mut self.battle_bar, g.band());
                    g.gui.run(band, &style, |ctx| {
                        crate::starmap_modern::bar(ctx, &bar, bs, &mut bar_acts);
                        crate::battle_modern::show(ctx, &bv, st);
                    });
                    let mut f = crate::render::Frame::new([0.0; 4]);
                    if let Some(gfx) = &sw.gfx {
                        crate::battle_modern::pictures(&mut f, &st.wells, gfx.info_tex, gfx.column_tex, &crate::battle_modern::places(&sw.ui), &crate::font::Virtual::new(size));
                    }
                    top = Some(f);
                }
            }
            match &mut top {
                Some(f) => {
                    if let Some(over) = self.spacewar.as_mut().and_then(|s| s.over_bar.take()) {
                        f.append(over);
                    }
                    ov.draw(f, h, size);
                }
                None => {
                    if let Some(over) = self.spacewar.as_mut().and_then(|s| s.over_bar.take()) {
                        frame.append(over);
                    }
                    ov.draw(&mut frame, h, size);
                }
            }
            match g.present(frame, top, t) {
                Ok(shown) => self.backdrop.last = Some((shown, t)),
                Err(e) => self.surface_failed(e),
            }
            // The original's screen menu is off in a battle (`topbar_menus` with the battle flag) but under `DEBUGX`.
            let debug = self.world.game.state.mem.u8(ig2_sim::save::va::DEBUGX) != 0;
            for a in bar_acts {
                if debug || !matches!(a, crate::starmap_modern::Act::Screen(_)) {
                    self.bar_act(a);
                }
            }
            return;
        }
        if let (Some(spy), Some(h)) = (&mut self.spy, &mut self.hud) {
            let en = self.world.curr.human as usize;
            let d = crate::spy_screen::SpyData::read(&self.world.game.state, en, self.world.game.rules, self.tables);
            spy.advance(&mut g.renderer, &self.install, &d, due_ui, t);
            if spy.closed() {
                self.spy = None;
                g.window.request_redraw();
                self.screen_change_step();
                return;
            }
            h.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
            h.turbo = self.turbo;
            h.paused = paused;
            spy.modern = spy_modern;
            h.modern_bar = spy_modern;
            let mut frame = spy.build(h, &d, size, self.install.lang() == ig2_formats::text::Lang::Hu);
            frame.backdrop = backdrop;
            h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Spy, t);
            if let Some(tu) = &self.tutorial {
                h.draw_tutorial(&mut frame, tu, size, 7);
            }
            let mut top = None;
            if spy_modern {
                let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                let mut v = crate::spy_modern::view(h, &self.world.game.state, self.tables, &self.world.ui, spy, &d, size, gates, t);
                v.bar.bind_keys(&self.bindings);
                let style = crate::starmap_modern::style(h, size);
                let (st, acts, band) = (&mut self.spy_modern, &mut self.spy_acts, g.band());
                g.gui.run(band, &style, |ctx| acts.extend(crate::spy_modern::show(ctx, &v, st)));
                let mut f = crate::render::Frame::new([0.0; 4]);
                ov.draw(&mut f, h, size);
                top = Some(f);
            } else {
                ov.draw(&mut frame, h, size);
            }
            match g.present(frame, top, t) {
                Ok(shown) => self.backdrop.last = Some((shown, t)),
                Err(e) => self.surface_failed(e),
            }
            self.spy_modern_apply();
            return;
        }
        if let (Some(s), Some(h), Some(m)) = (self.overlay, &mut self.hud, &mut self.frame_models) {
            h.frame_tex = match s {
                crate::ui::Screen::Info if info_modern => None,
                crate::ui::Screen::Messages if messages_modern => None,
                crate::ui::Screen::Info => m.render_info(&mut g.renderer, &self.install, &h.info.frame_view(&self.world.ui), size),
                crate::ui::Screen::Messages => m.render_messages(&mut g.renderer, &self.install, h.messages.frame, size),
                _ => None,
            };
        }
        let mut colony_panel = None;
        let mut colony_info = None;
        // The diplomacy screen: its UI ticks, the frame's input (its commands go to the queue), then its frame.
        if let (Some(d), Some(h), Some(crate::ui::Screen::Diplomacy)) = (&mut self.diplomacy, &self.hud, self.overlay) {
            for _ in 0..due_ui {
                d.screen.ui_tick();
            }
            // A screen asked for (`kep_kell`) takes over at the next frame's change step.
            if d.screen.closed() && self.kep_kell.is_none() {
                self.overlay = self.dip_next.take();
            }
            let vv = crate::font::Virtual::new(size);
            self.dip_input.at = vv.virtual_of(self.cursor);
            // Ours (ADR 0076): under the modern page no pointer reaches the hidden menus and heads.
            d.scene.modern = dip_modern;
            if dip_modern {
                self.dip_input = crate::diplomacy::Input { at: [-1e4; 2], ..Default::default() };
            }
            for c in d.input(&self.world.game.state, h, self.tables, &self.dip_input) {
                self.world.send(c);
            }
            self.dip_input.pressed = false;
            self.dip_input.right_pressed = false;
            self.dip_input.released = false;
        }
        let research = match (&self.research, self.overlay) {
            (Some(r), Some(crate::ui::Screen::Research)) => {
                let m = self.research_models.get_or_insert_with(|| crate::research::ResearchModels::new(&mut g.renderer, &self.install));
                m.modern = research_modern;
                Some(m.render(&mut g.renderer, &self.install, self.tables, r, &self.world.game.state, size))
            }
            _ => None,
        };
        drop(offscreen);
        let scene = crate::prof::scope("scene");
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
                let unit = crate::colony::landing_unit(&self.world.game.state, curr, self.tables);
                let light = c.view.panel_light(self.maps, curr.map);
                colony_panel = c.panel.render(&mut g.renderer, &self.install, self.tables, c.ui.kind, unit, c.ui.spin(), light, size, c.scene.sky.as_deref());
                colony_info = crate::colony_ui::render_kis_info(&mut g.renderer, &self.install, self.tables, &c.ui, &mut c.scene, &mut c.panel, curr, self.world.curr.human as i32, light, size);
                (c.view.build(&mut g.renderer, &self.install, self.tables, self.maps, &mut c.scene, prev, curr, alpha, t), crate::ui::Screen::Colony)
            }
            _ => {
                v.hover = v.pick(&self.world.prev, &self.world.curr, alpha, self.cursor);
                (v.build(&g.assets, &self.world.prev, &self.world.curr, alpha, t), crate::ui::Screen::Starmap)
            }
        };
        drop(scene);
        if self.overlay.is_some() {
            frame.backdrop = backdrop;
        }
        let hud = crate::prof::scope("hud");
        if let Some(h) = &mut self.hud {
            h.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
            h.turbo = self.turbo;
            h.paused = paused;
            h.modern_bar = (modern_bar && screen == crate::ui::Screen::Starmap) || (colony_modern && screen == crate::ui::Screen::Colony) || (info_modern && screen == crate::ui::Screen::Info) || (messages_modern && screen == crate::ui::Screen::Messages) || (dip_modern && screen == crate::ui::Screen::Diplomacy) || (trade_modern && screen == crate::ui::Screen::Trade) || (research_modern && screen == crate::ui::Screen::Research) || (design_modern && screen == crate::ui::Screen::Design);
            h.modern_info = info_modern && screen == crate::ui::Screen::Info;
            h.modern_messages = messages_modern && screen == crate::ui::Screen::Messages;
            if h.modern_messages {
                let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                let mut m = crate::messages_modern::view(h, &self.world.game.state, self.tables, &self.world.ui, gates, t);
                m.bar.bind_keys(&self.bindings);
                mmv = Some(m);
            }
            if let (true, crate::ui::Screen::Diplomacy, Some(d)) = (dip_modern, screen, &self.diplomacy) {
                let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                let mut m = crate::diplomacy_modern::view(h, &self.world.game.state, self.tables, &self.world.ui, d, gates, t);
                m.bar.bind_keys(&self.bindings);
                dipv = Some(m);
            }
            if h.modern_info {
                let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                let mut m = crate::info_modern::view(h, &self.world.game.state, self.tables, &self.world.ui, gates, t);
                m.bar.bind_keys(&self.bindings);
                imv = Some(m);
            }
            match (screen, &self.colony, &self.world.colony_curr) {
                (crate::ui::Screen::Colony, Some(c), Some(curr)) => {
                    let cursor = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
                    let input = crate::ui::ColonyInput { ui: &c.ui, sim: self.world.game.rules, panel: colony_panel, cursor, pointer: false, info_model: colony_info, now_ms: now, time: t, names, rename: self.menus.rename.as_ref(), modern: colony_modern };
                    h.draw_colony(&mut frame, &self.world.game.state, self.tables, self.maps, &c.view, &c.scene, curr, &input);
                    if colony_modern {
                        let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                        let mut m = h.colony_modern_view(&self.world.game.state, self.tables, &self.world.ui, self.maps, &c.view, curr, &c.ui, self.world.game.rules, gates, t);
                        m.bar.bind_keys(&self.bindings);
                        m.radar.picture = c.scene.radar.is_some();
                        if let Some(r) = &self.menus.rename {
                            m.card.title = format!("{}_", ig2_formats::text::decode(&r.text, h.texts.lang));
                        }
                        cmv = Some(m);
                    }
                }
                (crate::ui::Screen::Starmap, ..) => {
                    h.draw_starmap_names(&mut frame, v, &g.assets, &self.world.prev, &self.world.curr, alpha, t);
                    let lv = self.world.game.local_view();
                    let sel = Selection { mode: lv.starmap_mode, planet: lv.planet.max(0) as usize, fleet: lv.fleet.max(0) as usize };
                    if !modern_hud {
                        h.draw_starmap_hud(&mut frame, &self.world.game.state, self.tables, v, &self.world.prev, &self.world.curr, &sel, alpha, t, &self.panels.faders.starf, self.menus.info_box(v.camera.zoom), globe);
                    }
                    if modern_bar {
                        let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                        let mut m = h.modern_view(&self.world.game.state, self.tables, &self.world.ui, v, &self.world.prev, &self.world.curr, &sel, alpha, t, &self.panels, &self.menus, gates);
                        m.bind_keys(&self.bindings);
                        m.globe = globe.is_some();
                        // Under the long info the console and the minimap are off (hot-spots 1..=7).
                        if self.menus.info_on() {
                            m.mini = None;
                        }
                        mv = Some(m);
                    }
                    if classic_panels && !fleet_modern {
                        h.draw_starmap_panels(&mut frame, &self.panels, &self.world.game.state, self.tables, &sel, panel_tex, size, t);
                    }
                    if fleet_modern {
                        fmv = h.fleet_modern_view(&self.world.game.state, self.tables, &self.panels, &sel, mv.as_ref().and_then(|m| m.info.as_ref()));
                    }
                    if production_modern {
                        pmv = Some(h.production_modern_view(&self.world.game.state, &self.panels, sel.planet));
                    }
                    h.draw_starmap_menus(&mut frame, &self.menus, &self.panels, &self.world.game.state, self.tables, size, v.camera.zoom, t, modern_bar);
                }
                (crate::ui::Screen::Design, ..) => {
                    if let Some(d) = &mut self.design {
                        let state = &self.world.game.state;
                        let techs = state.tech();
                        let en = state.mem.i32(ig2_sim::save::va::EN).clamp(0, 18) as usize;
                        let sc = self.design_scene.get_or_insert_with(|| crate::design_scene::DesignScene::new(&mut g.renderer, &self.install));
                        sc.modern = design_modern;
                        d.modern = design_modern;
                        sc.draw(&mut frame, &mut g.renderer, &self.install, self.tables, d, state, &techs, en, size);
                        if design_modern {
                            let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                            let mut m = h.design_modern_view(state, self.tables, &self.world.ui, d, &self.design_tips, gates, t);
                            m.bar.bind_keys(&self.bindings);
                            dmv = Some(m);
                        } else {
                            h.draw_design(&mut frame, d, state, &techs, self.tables, &self.design_tips, size, t);
                        }
                    }
                }
                (crate::ui::Screen::Trade, ..) => {
                    if let Some(tr) = &self.trade {
                        let sc = self.trade_scene.get_or_insert_with(|| crate::trade_scene::TradeScene::new(&mut g.renderer));
                        sc.set_modern(trade_modern.then_some(crate::trade_modern::WELL_AT));
                        sc.draw(&mut frame, &mut g.renderer, &self.install, self.tables, tr, size);
                        if trade_modern {
                            let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                            let mut m = crate::trade_modern::view(h, &self.world.game.state, self.tables, &self.world.ui, tr, gates, t);
                            m.bar.bind_keys(&self.bindings);
                            tmv = Some(m);
                        } else {
                            h.draw_trade(&mut frame, tr, &self.world.game.state, self.tables, &self.trade_tips, size, t);
                        }
                    }
                }
                (crate::ui::Screen::Research, ..) if research_modern => {
                    if let (Some(r), Some((tex, lines))) = (&self.research, research) {
                        h.draw_research_scene(&mut frame, r, tex, lines, size, t);
                    }
                    if let Some(r) = &self.research {
                        let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
                        let mut m = h.research_modern_view(&self.world.game.state, self.tables, &self.world.ui, r, &self.tech_infos, gates, t);
                        m.bar.bind_keys(&self.bindings);
                        rmv = Some(m);
                    }
                }
                (crate::ui::Screen::Research, ..) => {
                    if let (Some(r), Some((tex, lines))) = (&mut self.research, research) {
                        let (glow, arrow) = self.research_models.as_ref().map_or((None, None), |m| (m.glow, m.arrow));
                        h.draw_research(&mut frame, r, &self.tech_infos, &self.research_tips, &self.world.game.state, self.tables, (tex, glow, arrow), lines, size, t);
                    }
                }
                _ => {}
            }
            h.draw(&mut frame, size, &self.world.ui, self.tables, screen, t);
            if screen == crate::ui::Screen::Messages && self.display.event_log {
                h.draw_event_log(&mut frame, &self.world.log, size, t);
            }
            // `GFX_DateMoneyScrollFade`'s fader pass 3 and `Update3D`'s `TutorialSzovegFrissit`: the window over the
            // screen (the pointer the recording moves is the overlay's).
            if let Some(tu) = &self.tutorial {
                h.draw_tutorial(&mut frame, tu, size, kep);
            }
            if mv.is_none() && cmv.is_none() && imv.is_none() && rmv.is_none() && dmv.is_none() && dipv.is_none() && tmv.is_none() {
                ov.draw(&mut frame, h, size);
            }
        }
        drop(hud);
        // The modern HUD over the frame, the pointer over it (its acts run after the frame, `modern_apply`).
        let mut top = None;
        if let (Some(mv), Some(h)) = (&mv, &self.hud) {
            let style = crate::starmap_modern::style(h, size);
            let (st, acts, band) = (&mut self.modern, &mut self.modern_acts, g.band());
            let (pst, pacts) = (&mut self.production_modern, &mut self.production_acts);
            let fst = &mut self.fleet_modern;
            g.gui.run(band, &style, |ctx| {
                acts.extend(crate::starmap_modern::show(ctx, mv, st));
                if let Some(pv) = &pmv {
                    pacts.extend(crate::production_modern::show(ctx, pv, pst, mv.race));
                }
                match &fmv {
                    Some(fv) => crate::fleet_modern::show(ctx, fv, fst),
                    None => *fst = Default::default(),
                }
            });
            let mut f = crate::render::Frame::new([0.0; 4]);
            crate::fleet_modern::pictures(&mut f, &self.fleet_modern.wells, panel_tex, &crate::font::Virtual::new(size));
            crate::fleet_modern::pictures(&mut f, &self.modern.wells, globe, &crate::font::Virtual::new(size));
            if pmv.is_some() {
                crate::production_modern::pictures(&mut f, &pst.wells, thumbs, &crate::font::Virtual::new(size));
            }
            ov.draw(&mut f, h, size);
            top = Some(f);
        }
        if let (Some(cv), Some(h)) = (&cmv, &self.hud) {
            let style = crate::starmap_modern::style(h, size);
            let (st, acts, band) = (&mut self.colony_modern, &mut self.colony_acts, g.band());
            g.gui.run(band, &style, |ctx| acts.extend(crate::colony_modern::show(ctx, cv, st)));
            let mut f = crate::render::Frame::new([0.0; 4]);
            crate::colony_modern::pictures(&mut f, &st.wells, colony_panel, colony_info, &crate::font::Virtual::new(size));
            crate::colony_modern::radar_under(&mut frame, &st.wells, self.colony.as_ref().and_then(|c| c.scene.radar));
            ov.draw(&mut f, h, size);
            top = Some(f);
        }
        if let (Some(iv), Some(h)) = (&imv, &self.hud) {
            let style = crate::starmap_modern::style(h, size);
            let (st, acts, band, install) = (&mut self.info_modern, &mut self.info_acts, g.band(), &self.install);
            g.gui.run(band, &style, |ctx| {
                st.pics.get_or_insert_with(|| crate::info_modern::Pics::load(ctx, install));
                acts.extend(crate::info_modern::show(ctx, iv, st));
            });
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        }
        if let (Some(tv), Some(h)) = (&tmv, &self.hud) {
            let style = crate::starmap_modern::style(h, size);
            let (st, acts, band) = (&mut self.trade_modern, &mut self.trade_acts, g.band());
            g.gui.run(band, &style, |ctx| acts.extend(crate::trade_modern::show(ctx, tv, st)));
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        }
        if let (Some(dv), Some(h)) = (&dipv, &self.hud) {
            let style = crate::starmap_modern::style(h, size);
            let (st, acts, band, install) = (&mut self.diplomacy_modern, &mut self.diplomacy_acts, g.band(), &self.install);
            g.gui.run(band, &style, |ctx| {
                st.pics.get_or_insert_with(|| crate::info_modern::Pics::load(ctx, install));
                acts.extend(crate::diplomacy_modern::show(ctx, dv, st));
            });
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        }
        if let (Some(mv), Some(h)) = (&mmv, &self.hud) {
            let style = crate::starmap_modern::style(h, size);
            let (st, acts, band) = (&mut self.messages_modern, &mut self.messages_acts, g.band());
            g.gui.run(band, &style, |ctx| acts.extend(crate::messages_modern::show(ctx, mv, st)));
            let mut f = crate::render::Frame::new([0.0; 4]);
            // The talking head's clip (and its interlace) fitted into the well the page left.
            if let (Some((tex, _, _)), Some(w)) = (h.clip, st.well) {
                let (c, half) = ([w.center().x, w.center().y], [w.width() / 2.0, w.height() / 2.0]);
                f.push(tex, crate::render::Blend::GammaAlpha, crate::render::Instance::new(crate::render::kind::SPRITE, c, half, [1.0; 4]));
                if let Some(il) = h.interlace {
                    let [lo, hi] = self.tables.videos.interlace_uv;
                    let mut q = crate::render::Instance::new(crate::render::kind::SPRITE, c, half, [1.0, 1.0, 1.0, 25.0 / 255.0]);
                    q.uv = [lo, lo, hi - lo, hi - lo];
                    f.push(il, crate::render::Blend::GammaAlpha, q);
                }
            }
            ov.draw(&mut f, h, size);
            top = Some(f);
        }
        if let (Some(rv), Some(h)) = (&rmv, &self.hud) {
            let style = crate::starmap_modern::style(h, size);
            let (st, acts, band) = (&mut self.research_modern, &mut self.research_acts, g.band());
            g.gui.run(band, &style, |ctx| acts.extend(crate::research_modern::show(ctx, rv, st)));
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        }
        if let (Some(dv), Some(h)) = (&dmv, &self.hud) {
            let style = crate::starmap_modern::style(h, size);
            let (st, acts, band) = (&mut self.design_modern, &mut self.design_acts, g.band());
            g.gui.run(band, &style, |ctx| acts.extend(crate::design_modern::show(ctx, dv, st)));
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        }
        match g.present(frame, top, t) {
            Ok(shown) => self.backdrop.last = Some((shown, t)),
            Err(e) => return self.surface_failed(e),
        }
        self.frames += 1;
        if self.title_at.elapsed().as_secs_f32() >= 1.0 {
            let fps = self.frames as f32 / self.title_at.elapsed().as_secs_f32();
            let (y, m, d) = self.world.curr.date();
            let speed = self.world.game.state.mem.i32(ig2_sim::save::va::IDO_SPEED);
            let state = if paused { "paused".to_string() } else { format!("×{speed}") };
            g.window.set_title(&format!("Imperium Galactica II — {y}-{m:02}-{d:02} {state} — {fps:.0} fps — openIG2 {}", crate::version()));
            self.title_at = Instant::now();
            self.frames = 0;
        }
        self.modern_apply();
        self.colony_modern_apply();
        self.production_modern_apply();
        self.research_modern_apply();
        self.design_modern_apply();
        self.info_modern_apply();
        self.messages_modern_apply();
        self.diplomacy_modern_apply();
        self.spy_modern_apply();
        self.trade_modern_apply();
    }

    /// The frame's message pass and the sim ticks due (none while `IdoPause`), with what the screens take from the
    /// world first: the colony's events and its scene's reload, the order events, the screen the sim sees.
    fn frame_sim(&mut self, now: u32) {
        let sim = crate::prof::scope("sim");
        // The frame's message pass, then the ticks due (none while `IdoPause`).
        self.world.log_on = self.display.event_log;
        self.world.pump();
        self.menus.sent_done();
        if let Some(c) = &mut self.colony {
            let p = self.world.colony.unwrap_or(0);
            c.ui.events(&std::mem::take(&mut self.world.colony_events), self.world.curr.human, p);
            // A building came or went: `Init3dColony`'s building loop again.
            if let (Some((renderer, ..)), Some(s)) = (gfx(&mut self.gpu, &mut self.headless), &self.world.colony_curr) {
                if c.scene.stale(s) {
                    let _reload = crate::prof::scope("reload");
                    if let Err(e) = c.scene.reload(renderer, &self.install, self.tables, self.maps, s) {
                        eprintln!("ig2-client: colony: {e}");
                    }
                }
            }
        } else {
            self.world.colony_events.clear();
        }
        for e in std::mem::take(&mut self.world.order_events) {
            if let Some(d) = &mut self.design {
                d.event(&e);
            }
            if let OrderEvent::Split { empire, fleet, new } = e {
                if empire == self.world.curr.human {
                    // `MULTIGET_Starmap`: the new fleet is selected and the transfer between the two opens, before the
                    // frame's ticks (their 0x1b spares the panel's fleets).
                    self.select(Selection { mode: 2, fleet: new, ..self.selection() });
                    self.panels.open_transfer(fleet as i32, new as i32, false);
                }
            }
        }
        self.sync_transfer();
        self.world.game.set_screen(self.shown_kep());
        self.world.game.sp_battles.colony = if self.shown_kep() == 3 { self.world.colony } else { None };
        let due = self.clock.advance(now);
        // Multiplayer: the session is pumped while paused too (its keep-alive and the others' commands).
        // A tutorial just started holds the game until its first tick has run (`ticked`).
        let held = self.tutorial.as_ref().is_some_and(|t| !t.ticked);
        if (!self.world.paused() || self.world.net.is_some()) && self.headless.as_ref().is_none_or(|h| h.sim) && !held {
            // ADR 0058: the extra speeds run more ticks; a slow-down (`SetIdoLelassit`) or the row off ends them.
            let fastest = self.world.game.speed_level() == 3 && self.settings.int(crate::display::EXTRA_SPEEDS) == Some(1);
            if !fastest || self.world.net.is_some() || self.world.viewer.is_some() || self.spacewar.is_some() {
                self.turbo = 1;
            }
            for _ in 0..due * self.turbo {
                self.world.tick();
                if let Some(t) = &mut self.tutorial {
                    t.counter += 1;
                }
                let _autosave = crate::prof::scope("autosave");
                self.autosave_tick();
            }
        }
        drop(sim);
    }

    /// The frame's UI steps after the sim: the HUD and its message line, the camera and the starmap's scroll, the
    /// colony, research, design and trade screens' steps, the frame's sounds and the panels' held buttons.
    fn frame_ui_steps(&mut self, now: u32, due_ui: u64, sel: Selection) {
        let at = self.virtual_cursor();
        let on_info = self.overlay == Some(crate::ui::Screen::Info) && self.pointer_in && !self.info_modern_up();
        let info_down = self.info_down;
        let (line_on, waiting, story) = self.message_line_gates();
        // `GFX_AllandoCallBack2`'s story text `_s(5, 0x215d)` on the colony screen.
        let story = story.then(|| self.world.game.rules.messages.story_text(0x215d).to_vec());
        // `FRISSIT_Scroll2`'s head (exe 0x556210): an error text doesn't count in a chat entry (`IOAzonosito` 1) or a
        // screen change.
        let scroll_runs = !self.screen_changing();
        if let Some(h) = &mut self.hud {
            let world = &mut self.world;
            (h.line_held, h.message_waiting, h.story_line) = (!line_on, waiting, story);
            let on_messages = self.overlay == Some(crate::ui::Screen::Messages);
            let now_ms = now as f64;
            for (line, colour) in world.chat_lines.drain(..) {
                h.chat.ring.add(line, colour, now_ms);
            }
            for _ in 0..due_ui {
                if let Some(e) = &mut h.error {
                    // An error text counts up to 300 UI frames instead of the line moving, then goes.
                    if scroll_runs && h.chat.entry.is_none() {
                        e.1 += 1;
                        if e.1 > 300 {
                            h.error = None;
                        }
                    }
                } else if line_on {
                    let hu = h.texts.lang == ig2_formats::text::Lang::Hu;
                    h.scroller.step(&mut || world.pop_message().map(|(t, i)| (crate::diplomacy::articles(&t, hu), i)));
                }
                // `Info_Main`'s per-frame spots see the hot-spot under the pointer (`AktIkon2`).
                let spot = if on_info { h.info.spot_at(&self.tables.race_name_x, at, &world.ui) } else { None };
                h.info.ui_frame(&world.ui.info, world.ui.human, spot, at[1], info_down);
                h.messages.ui_frame();
                if on_messages {
                    h.messages.tick();
                }
                // `FRISSIT_Messages2`: every 11th UI frame on the screen, the automatic messages.
                self.messages_auto = (self.messages_auto + 1) % 11;
                if on_messages && self.messages_auto == 0 {
                    world.send(ig2_sim::game::MessageOrder::Auto.into());
                }
            }
            if on_messages {
                h.messages.sync(ig2_sim::game::inbox_place(&world.game.state));
                // A dialogue the sim opened by itself (an automatic message, one taken up again) is shown.
                if let Some(i) = ig2_sim::game::dialogue_in_progress(&world.game.state) {
                    if h.messages.open.as_ref().is_none_or(|d| d.index != i) {
                        h.open_message(&self.install, &world.ui, i);
                    }
                }
            }
            if let Some(d) = &mut h.messages.open {
                for (k, a) in d.answers.iter_mut().enumerate() {
                    a.disabled = ig2_sim::game::choice_disabled(&world.game.state, k);
                }
            }
            // `Messages_Main`'s last step every frame on the screen: the answer asked for (`ValaszKivalasztva`).
            if on_messages {
                h.messages.answer();
            }
            // `MessagesGfx2D_2` types only at `mesframe` 30.
            let typing = on_messages && h.messages.text_up();
            if let Some(d) = h.messages.open.as_mut().filter(|_| on_messages) {
                if typing { d.advance(due_ui as f32 * 10.0) } else { d.wait(due_ui as f32 * 10.0) }
            }
            let mut orders = std::mem::take(&mut h.messages.orders);
            if let Some(d) = &mut h.messages.open {
                orders.append(&mut d.orders);
            }
            for o in orders {
                world.send(o.into());
            }
        }
        self.head_clip_frame(now, due_ui);
        // `StarmapKeyboardDoModifies` runs every UI frame whatever the screen; `Starmap_Autoscroll` on the starmap only,
        // not under the long info, the zoom or a playing tutorial (`TutorialActive`, ELF `Starmap_Autoscroll`; the player's
        // turns scroll); the minimap held retargets every frame.
        let scroll = self.colony.is_none() && self.spacewar.is_none() && !self.under_screen() && !self.menus.info_on() && self.cam_saved.is_none() && !self.tutorial_playing();
        let win = self.gpu.as_ref().map_or([640, 480], |g| {
            let s = g.window.inner_size();
            [s.width, s.height]
        });
        if let Some(v) = &mut self.view {
            for _ in 0..due_ui {
                self.star_keys.tick(&mut v.camera);
                // Gated off, the speeds keep their values (the exe skips the whole function). The pointer is the last
                // physical one, so the scroll goes on with it outside the window (as the colony's), (0, 0) before the
                // first move (`pointer_seen`).
                if scroll {
                    let held = self.mouse_held != 0 || !self.pointer_seen;
                    self.star_keys.autoscroll(&mut v.camera, self.cursor_px, win, held);
                }
            }
        }
        // `FlottaDelete` / `FlottaOsszevon` end a fleet's rename when that fleet goes.
        if let Some(r) = self.menus.rename.as_ref().filter(|r| r.fleet) {
            if self.world.game.state.fleets.get(r.index).is_none_or(|f| f[0x15] != self.world.curr.human) {
                self.menus.rename = None;
            }
        }
        if due_ui > 0 && self.console_held.is_some_and(|(s, _)| s == 7) {
            let t = orders::minimap_target(self.virtual_cursor());
            let zoom = self.camera3()[2];
            self.tween_to([t[0], t[1], zoom], 4);
        }
        // `FRISSIT_Starmap2` runs on the starmap only.
        if let (Some(v), None, None) = (&mut self.view, &self.colony, self.overlay) {
            // `Starmap_Belep` inits `fifo` and `starf` on every entry.
            // `Starmap_Belep` also lets every console button out (`SetFrame(GetSubMeshid(KezeloId, i), 0)`, i < 6, exe
            // 0x5b19a0); `GombFazis`, `MelyikGombot` and `KezeloGombFrame` keep their values.
            if !self.on_starmap {
                self.panels.faders = crate::starmap_panels::StarmapFaders::enter();
                self.star_buttons.depth = [0.0; 6];
                // `Starmap_Belep` exe 0x5b1d74: `PlayMusicMain(0, 0)` unless a battle still runs (`spacewarbanvanbarkacs`);
                // it does nothing while a main track plays, so a ground war's music ends here (a campaign's intro
                // plays first, `AfterVideo::StarmapEntry`).
                let battle = self.spacewar_parked.is_some() || self.spacewar.as_ref().is_some_and(|sw| !sw.ended);
                if !battle && self.world.videos.is_empty() {
                    self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game));
                }
            }
            for _ in 0..due_ui {
                v.ui_tick();
                self.panels.ui_tick();
                // `Starmap_Main` sets a held rotate, zoom or pitch button again every frame, then `ShowStarmap` steps.
                // Zoom and pitch read the buttons each frame: left in / up, then right out / down (right wins with both).
                if let Some((s @ (2 | 3 | 5 | 6), _)) = self.console_held {
                    self.star_buttons.press(s);
                    let k = &mut self.star_keys;
                    let pair = match s {
                        5 => Some((&mut k.zoom_in, &mut k.zoom_out)),
                        6 => Some((&mut k.up, &mut k.down)),
                        _ => None,
                    };
                    if let Some((inc, dec)) = pair {
                        if self.mouse_held & 1 != 0 {
                            (*inc, *dec) = (true, false);
                        }
                        if self.mouse_held & 2 != 0 {
                            (*inc, *dec) = (false, true);
                        }
                    }
                }
                self.star_buttons.step();
            }
            self.on_starmap = true;
        } else {
            self.on_starmap = false;
        }
        if let (Some(h), Some(g)) = (&mut self.hud, &mut self.gpu)
            && let Some(c) = &mut h.starmap.console
        {
            c.pose(&mut g.renderer, &self.install, &self.star_buttons.depth);
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
            let clock = prev.orbit + (curr.orbit - prev.orbit) * self.world.draw_phase(self.clock.phase(now));
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
        let win = self.gpu.as_ref().map_or([640, 480], |g| {
            let s = g.window.inner_size();
            [s.width, s.height]
        });
        // `ColonyKisInfo` (ELF 0x4edb40): with a unit or a building selected (`ColonyInfoMod` 1, 2) the name entry ends
        // and the old name is back.
        if self.menus.rename.is_some() && self.colony.as_ref().is_some_and(|c| c.ui.selected != 0 || !c.ui.war.selected.is_empty()) {
            if let Some(b) = self.menus.rename.as_ref().map(|r| r.backup.clone()) {
                self.write_rename(&b);
            }
            self.menus.rename = None;
        }
        let tutorial_playing = self.tutorial_playing();
        if let (Some(c), Some(s)) = (&mut self.colony, &self.world.colony_curr) {
            let shown = self.overlay.is_none() && self.spy.is_none();
            for _ in 0..due_ui {
                // `Frissit2_Colony` runs every UI tick (the camera keys); `Colony_Autoscroll` only on the screen.
                c.ui.cam_keys.tick(&mut c.view.camera);
                // The pointer's last window position, also once it has left the window (the exe reads the last
                // physical position, colony.md §11.9).
                // `Colony_Autoscroll` (ELF 0x4e2e40) stops while the tutorial's script plays (`TutorialActive`).
                if shown && !tutorial_playing {
                    c.ui.cam_keys.autoscroll(&mut c.view.camera, self.cursor_px, win, self.mouse_held != 0);
                } else {
                    c.ui.cam_keys.stop_scroll();
                }
                // `Frissit2_Colony`'s ambients and music only with the screen shown (`AktKep == 3`).
                if shown {
                    c.ui.ui_frame(s, self.world.curr.human as i32, &mut c.view.camera);
                    if let Some(m) = crate::colony_ui::ColonyUi::war_music(&mut self.volt_harc, s.at_war) {
                        self.audio.play_music(m, false, human_race(&self.world.game));
                    }
                }
            }
            // `TransformRotation`, once a drawn frame in `ShowColony`.
            if shown {
                c.ui.cam_keys.frame(&mut c.view.camera);
                c.ui.cam_keys.tank(&mut c.view.camera, &s.units, &c.ui.war.selected);
            }
            self.cam_keep.clone_from(&c.ui.cam_keys);
            // The earthquake (`ShowColony`): the clock starts at the first colony draw after it, `FRISSIT_Story` ends it
            // 10000 `AktIdo` (ms) later; the jolt only on the quake's planet.
            c.view.camera.shake = [0.0; 2];
            if let Some((q, since)) = &mut self.quake {
                if since.get_or_insert_with(Instant::now).elapsed() >= QUAKE {
                    self.quake = None;
                } else if Some(*q) == self.world.colony && shown {
                    let x = (self.quake_rand.rand() % 31 - 15) as f32;
                    let y = (self.quake_rand.rand() % 31 - 15) as f32;
                    c.view.camera.shake = [x, y];
                }
            }
            c.ui.building_sounds(s, shown);
            c.view.overlay = c.ui.overlay(s, self.world.curr.human as i32);
            // The lightning lights one drawn frame.
            c.ui.flash = false;
        }
        if self.colony.is_some() && self.overlay.is_none() && self.spy.is_none() {
            self.colony_pointer(false);
        }
        // `Messages_Kilep`'s loop and `Messages3DUpdate`'s flags, before the frame is drawn: the closing run out, the
        // screen asked for takes over.
        if self.overlay == Some(crate::ui::Screen::Messages) && self.hud.as_mut().is_some_and(|h| h.messages.render_step()) {
            if self.kep_kell.is_some() {
                self.screen_change_step();
            } else {
                self.overlay = None;
            }
        }
        if let Some(r) = &mut self.research {
            if std::mem::take(&mut r.picked_turret) {
                self.world.game.tutorial_check_event(8);
            }
            for _ in 0..due_ui {
                r.step(&self.world.game.state);
            }
            if r.closed() {
                self.drop_research();
                self.overlay = None;
                self.screen_change_step();
            }
        }
        let tips = std::mem::take(&mut self.design_tips);
        self.design_do(|d, c, mask| {
            for _ in 0..due_ui {
                d.step(c, mask);
            }
            d.tooltip(c, mask, &tips, now as f64);
        });
        self.design_tips = tips;
        if self.design.as_ref().is_some_and(|d| d.closed()) {
            self.drop_design();
            self.overlay = None;
            self.screen_change_step();
        }
        if let Some(tr) = &mut self.trade {
            for _ in 0..due_ui {
                tr.step(&self.world.game.state);
                for delivery in tr.orders.drain(..) {
                    self.world.send(ig2_net::Command::Buy { delivery });
                }
            }
            tr.tooltip(&self.trade_tips, now as f64);
            if tr.closed() {
                self.trade = None;
                self.overlay = None;
                self.screen_change_step();
            }
        }
        if let (Some(r), Some(m)) = (&mut self.research, &self.research_models) {
            r.tip.moved(self.view.as_ref().map_or([0.0; 2], |v| v.camera.virtual_of(self.cursor)));
            r.set_auto(self.world.game.auto_research[crate::research::en(&self.world.game.state)]);
            if let Some(h) = &self.hud {
                r.tooltip(&self.world.game.state, &m.mask, &self.research_tips, &h.texts, &m.centres, now as f64);
            }
        }
        // The sounds queued by the frame's input and by these UI ticks, then the ticks' music steps.
        self.audio_frame(due_ui);
        self.row_frame_phase(due_ui);
        // A button held on a panel steps on the 100 Hz UI ticks (`Starmap_Main::KezdoIdo` counts 60 a second).
        if self.panel_held.is_some() || self.production_hold.is_some() {
            // The modern view's held count button keeps its half (ADR 0071).
            let at = match self.production_hold {
                Some((l, k, plus)) => crate::production_modern::count_spot(l, k, plus).1,
                // ADR 0077: a held modern scroll bar reads the pointer's height only.
                None if self.fleet_modern_up() && self.panels.scrolling() => self.fleet_modern.scroll_pointer(self.virtual_cursor()),
                None => self.starmap_virtual(self.virtual_cursor()),
            };
            for _ in 0..due_ui {
                let acts = self.panels.hold_frame(&self.world.game.state, &sel, at);
                self.apply(acts);
            }
        }
    }
}

/// `AktKep` of what the client shows (re/screens/README.md).
fn akt_kep(overlay: Option<crate::ui::Screen>, colony: bool, spacewar: bool) -> i32 {
    use crate::ui::Screen as S;
    match overlay {
        _ if spacewar => 11,
        Some(S::Help) => 1,
        Some(S::Research) => 4,
        Some(S::Design) => 5,
        Some(S::Diplomacy) => 6,
        Some(S::Spy) => 7,
        Some(S::Trade) => 8,
        Some(S::Info) => 9,
        Some(S::Messages) => 10,
        Some(S::Options) => ig2_sim::tutorial::SCREEN_OPTIONS,
        Some(S::Colony) => 3,
        Some(S::Starmap) | None if colony => 3,
        Some(S::Starmap) | None => 2,
    }
}

/// `KELLKINYITASDEFAULT & 1` unless `OldKep` is the messages, help or options screen: the screens' `_Belep` open
/// animated (research, design, trade, diplomacy; re/screens/design.md §3, trade.md §5).
fn opens_animated(from: i32) -> bool {
    !matches!(from, 1 | 10 | 0xd)
}

/// A recorded key (`ReadNext`'s slots: kind 0 a character, 1 a key going down, 2 up; Windows key codes) as the
/// client's key: the characters, and the keys that make none.
/// The building names' option row (`Options` row 9, id 202).
const NAMES_KEY: &str = "KellEpuletnevKiiras";

fn recorded_key(kind: u8, code: u32) -> Option<Key> {
    let named = |n| Some(Key::Named(n));
    const F: [NamedKey; 12] = [
        NamedKey::F1, NamedKey::F2, NamedKey::F3, NamedKey::F4, NamedKey::F5, NamedKey::F6, NamedKey::F7, NamedKey::F8, NamedKey::F9,
        NamedKey::F10, NamedKey::F11, NamedKey::F12,
    ];
    match (kind, code) {
        (0, 0x20..=0x7e) => Some(Key::Character(char::from(code as u8).to_string().into())),
        (1, 0x08) => named(NamedKey::Backspace),
        (1, 0x0d) => named(NamedKey::Enter),
        (1, 0x1b) => named(NamedKey::Escape),
        (1, 0x21) => named(NamedKey::PageUp),
        (1, 0x22) => named(NamedKey::PageDown),
        (1, 0x25) => named(NamedKey::ArrowLeft),
        (1, 0x26) => named(NamedKey::ArrowUp),
        (1, 0x27) => named(NamedKey::ArrowRight),
        (1, 0x28) => named(NamedKey::ArrowDown),
        (1, k @ 0x70..=0x7b) => named(F[k as usize - 0x70]),
        _ => None,
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
        // `remapKeyCodes` swaps them: PgUp comes as vk 0x22 (PgDn's), PgDn as 0x21 (FX5).
        NamedKey::PageUp => K::PageDown,
        NamedKey::PageDown => K::PageUp,
        _ => return None,
    })
}

/// The row frame's phase hand-over (`dkm`, `dkmcel`, `GfxNovelo`: one set of globals, `Frissit2_Grafika` exe 0x4d2ed0
/// steps them every 10 ms tick on every screen): the shown screen's frame `owner` steps them itself and the globals copy
/// it; a newly shown one starts from the globals with its places reset; with none shown the globals step on their own.
pub(crate) fn carry_row_frame(dk: &mut crate::kit::DesignFrame, dk_owner: &mut u8, owner: u8, frame: Option<&mut crate::kit::DesignFrame>, due: u64) {
    match frame {
        Some(f) if owner == *dk_owner => {
            let [m, c, n] = f.phase();
            *dk = crate::kit::DesignFrame::with_phase(m, c, n);
        }
        Some(f) => {
            for _ in 0..due {
                dk.tick();
            }
            let [m, c, n] = dk.phase();
            *f = crate::kit::DesignFrame::with_phase(m, c, n);
        }
        None => {
            for _ in 0..due {
                dk.tick();
            }
        }
    }
    *dk_owner = owner;
}
