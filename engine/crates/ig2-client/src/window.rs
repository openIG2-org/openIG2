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

mod do_keyboard;
mod pointer;
mod top_bar;
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
    /// side strips black under the "letterbox" option. Gives the frame back (the overlays' `SaveScreen`).
    fn present(&mut self, frame: crate::render::Frame, out: &wgpu::Texture, t: f32) -> crate::render::Frame {
        self.present_over(frame, None, out, t)
    }

    /// [`Gpu::present`], with this frame's egui page (ADR 0062) over the band and `top` (the pointer) over it.
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
}

impl Front {
    /// `MainScr_Belep`, `MultiplayerNev` from the settings file (`SaveGlobalRegistrySettings`' value; "Player" without one).
    fn new(scene: Option<crate::mainscr_scene::MainScene>, ui_ms: u32, settings: &ig2_formats::options::Options) -> Self {
        let mut s = crate::mainscr::MainScr::enter();
        s.settings = settings.int(crate::display::MENU_SETTINGS) != Some(0);
        s.flow = settings.int(crate::menu_flow::KEY) == Some(1);
        s.history = settings.int(crate::display::STATISTICS) != Some(0);
        s.can_continue = crate::options::slot_dir().is_some_and(|d| !crate::menu_flow::saves_newest_first(&d).is_empty());
        if let Some(n) = settings.get("MultiplayerNev").filter(|n| !n.is_empty()) {
            s.player = n.as_bytes().to_vec();
        }
        Front { s, scene, ui_ms, at: [320.0, 240.0], quit: false, scenarios: None, mp: None, net: Default::default(), session: None, stats: None, replays: None, mods: None, opening: false }
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
}

/// The Options screen up: its state, its 10 ms UI clock, the lasers' glow, and `OptionsOldIdoPause`.
/// The Help screen up (`AktKep` 1, re/screens/help.md) over whatever screen F1 came from, which comes back after.
struct HelpUp {
    s: crate::help::HelpScreen,
    ui_ms: u32,
    glow: Option<crate::render::TexId>,
    pics: crate::help::Pictures,
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
        audio.play_music(music, o.main_menu, human_race(&world.game.state));
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
        st.draw(frame, &d);
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

/// `PlayerFaj[EN]`, the music's race.
pub(crate) fn human_race(state: &ig2_sim::save::SaveState) -> u8 {
    let en = state.mem.i32(ig2_sim::save::va::EN).max(0) as u32;
    state.mem.u8(ig2_sim::save::va::PLAYER_FAJ + en)
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
        let unit = crate::colony::landing_unit(&self.world.game.state, curr);
        let panel = c.panel.render(r, &self.install, self.tables, c.ui.kind, unit, c.ui.spin(), light, size, c.scene.sky.as_deref());
        let info_model = crate::colony_ui::render_kis_info(r, &self.install, self.tables, &c.ui, &mut c.scene, &mut c.panel, curr, self.world.curr.human as i32, light, size);
        let mut frame = c.view.build(r, &self.install, self.tables, self.maps, &mut c.scene, prev, curr, 1.0, t);
        let cursor = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
        let input = crate::ui::ColonyInput { ui: &c.ui, sim: self.world.game.rules, panel, cursor, pointer: false, info_model, now_ms: h.ms, time: t, names: true, rename: None };
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

    fn audio_frame(&mut self, due_ui: u64) {
        use crate::audio::SoundId;
        use crate::ui::Screen;
        use ig2_rules::sounds::Table;
        // `Colony_Kilep` (exe 0x466390): `DeInitSounds` of `Colony_Villamok`, `Colony_Hangok` and `Gr_Hangok` stops
        // whatever of them still plays (the wind, the thunder, the war).
        if self.sound_colony && self.colony.is_none() {
            for t in [Table::Villam, Table::Colony, Table::Gr] {
                self.audio.deinit(t);
            }
        }
        self.sound_colony = self.colony.is_some();
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        // `UpdateFrame`: the start voice's ducking (the main menu and the start screens are past 0xd).
        self.audio.start_voice_frame(self.front.is_some());
        // `FRISSIT_Scroll2` (exe 0x556210): a message taken onto the line speaks, unless it is one of the four
        // silent ids; the brutal voice for empire 5 (`EN`), else the human one.
        let en = self.world.game.state.mem.i32(ig2_sim::save::va::EN);
        for (id, text) in std::mem::take(&mut self.world.popped) {
            if !matches!(id, 1 | 0x14 | 0x15 | 0x1a) {
                let voice = if en == 5 { "brutal" } else { "human" };
                let ch = self.audio.play_wav(&format!("sounds/scroll/{voice}/{id:04}.wav"));
                if let Some(ch) = ch.filter(|_| self.display.subtitles) {
                    self.subtitle = Some((ch, text));
                }
            }
        }
        if self.subtitle.as_ref().is_some_and(|(ch, _)| !self.display.subtitles || !self.audio.playing(*ch)) {
            self.subtitle = None;
        }
        if let Some(h) = &mut self.hud {
            h.subtitle = self.subtitle.as_ref().map(|(_, t)| t.clone());
        }
        // `WF::PlaySpeechGrWar` (exe 0x48c670) and the retreat 0x12f: only while the colony screen shows the war's
        // planet; the line and kind for this player's empire (ground-combat.md §11.12).
        let shown = self.world.colony.filter(|_| self.on_colony());
        for (p, v) in std::mem::take(&mut self.world.ground_voices) {
            if shown == Some(p) {
                let (line, kind) = v.line(en);
                self.speech.push(crate::audio::speech::Speak { who: crate::audio::speech::Who::Colony, line, kind });
            }
        }
        // `Gr_Hangok` and the flashes of the shown war (the sim gates them by the screen; another planet's are dropped).
        for (p, e) in std::mem::take(&mut self.world.ground_fx) {
            let Some(c) = self.colony.as_mut().filter(|_| shown == Some(p)) else { continue };
            match e {
                ig2_sim::ground::GroundEvent::Sound(i) => c.ui.sounds.push(crate::audio::SoundId::new(ig2_rules::sounds::Table::Gr, i as u16)),
                e => {
                    let counter = c.ui.war.counter();
                    c.ui.fx.event(&e, counter);
                }
            }
        }
        if let Some(c) = &mut self.colony {
            c.scene.take_fx(&mut c.ui.fx);
            c.ui.fx.step_layers(c.ui.war.counter());
        }
        if let Some(c) = &mut self.colony {
            for id in c.ui.sounds.drain(..) {
                self.audio.play(id);
            }
            for (line, kind) in c.ui.voices.drain(..) {
                self.speech.push(crate::audio::speech::Speak { who: crate::audio::speech::Who::Colony, line, kind });
            }
        }
        if let Some(sw) = &mut self.spacewar {
            for (line, kind) in sw.ui.voices.drain(..) {
                self.speech.push(crate::audio::speech::Speak { who: crate::audio::speech::Who::SpaceWar, line, kind });
            }
        }
        for s in std::mem::take(&mut self.speech) {
            self.audio.speak(s, en);
        }
        let on_starmap = self.on_starmap();
        let on_battle = self.spacewar.is_some();
        for _ in 0..due_ui {
            self.audio.speakers.tick(on_starmap);
            if on_battle {
                self.audio.speakers.tick_spacewar();
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
        if let Some(d) = &mut self.design {
            for id in d.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        if let Some(t) = &mut self.trade {
            for id in t.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        // `Messages_Belep`/`_Kilep` exe 0x507f90/0x5083d0 (the diplomacy screen queues its own: `Screen::enter`/`leave`).
        // `Messages_Kilep` (exe 0x5083d0) as it begins (before its closing): the typer and the speech stop, the ducking
        // ends, `scrn23ki.wav`; a screen gone some other way makes them as it goes.
        let kilep = self.hud.as_mut().is_some_and(|h| std::mem::take(&mut h.messages.kilep));
        let gone = self.overlay != self.sound_overlay && self.sound_overlay == Some(Screen::Messages);
        if kilep || (gone && !self.hud.as_ref().is_some_and(|h| h.messages.left)) {
            self.audio.stop_sound(SoundId::global(7));
            self.audio.stop_message_speech();
            self.audio.set_duck(0);
            self.audio.message.indult = false;
            self.audio.play(SoundId::new(Table::Mes, 1));
        }
        if self.overlay != self.sound_overlay {
            if self.overlay == Some(Screen::Messages) {
                self.audio.play(SoundId::new(Table::Mes, 0));
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
        // The top bar's speed sound (`top_bar`) over this screen.
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        self.audio.frame(human_race(&self.world.game.state));
    }

    fn init_gpu(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        let attrs = {
            let d = self.display;
            let attrs = Window::default_attributes().with_title(format!("Imperium Galactica II — openIG2 {}", crate::version()));
            let attrs = match d.size {
                Some((w, h)) if d.mode == crate::display::Mode::Window => attrs.with_inner_size(winit::dpi::PhysicalSize::new(w, h)),
                _ => attrs.with_inner_size(winit::dpi::LogicalSize::new(1600.0, 900.0)),
            };
            with_icon(attrs.with_fullscreen(d.fullscreen(el.primary_monitor())))
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

    fn finish_gpu(&mut self, parts: GpuParts) -> Result<(), String> {
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
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: Renderer::view_formats(format),
        };
        surface.configure(&device, &config);
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
        self.gpu = Some(Gpu { window, surface, config, renderer, assets, band_target: None, gui: crate::gui::Gui::new(format).install_fonts(&self.install) });
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

    /// Open the colony screen of `AktPlanet`, or close it.
    fn toggle_colony(&mut self) {
        if self.colony.take().is_some() {
            self.world.show_colony(None);
            return;
        }
        if !self.colony_tutorial_gate() {
            return;
        }
        if let Some(p) = self.colony_target() {
            self.open_colony(p);
        }
    }

    /// `TutorialActive` (exe 0xf6915d): the script plays (its recording drives the input), not waiting for the player.
    fn tutorial_playing(&self) -> bool {
        self.tutorial.as_ref().is_some_and(|t| t.t.active)
    }

    /// `KepernyokLetiltva[n]` (exe 0xf66f2c + n): the tutorial has turned screen `n` off.
    fn tutorial_screen_off(&self, n: usize) -> bool {
        self.tutorial.as_ref().is_some_and(|t| t.t.gates.screens.get(n).copied().unwrap_or(false))
    }

    /// The planet the colony screen opens on: `AktPlanet` ([`Self::akt_planet`]; `Colony_Belep` exe 0x465b40 reads
    /// 0xb9933c from 0x465bb9 on, so after a fleet pick the planet picked or shown before it; re/deviations.md row
    /// 342).
    fn colony_target(&mut self) -> Option<usize> {
        let p = self.akt_planet();
        self.world.curr.planets.iter().any(|q| q.index == p).then_some(p)
    }

    /// `AktPlanet` (exe 0xb9933c): the colony shown, else the planet last picked on the starmap (`Starmap_Main` exe
    /// 0x5b2b70 sets it on a planet pick, a fleet pick leaves it) or shown, else the saved one.
    fn akt_planet(&mut self) -> usize {
        let selected = match self.view.as_ref().and_then(|v| v.selected) {
            Some(crate::starmap::Pick::Planet(p)) => Some(p),
            _ => None,
        };
        if let Some(p) = self.world.colony.or(selected) {
            self.akt_planet = Some(p);
        }
        self.akt_planet.unwrap_or_else(|| self.world.game.state.mem.i32(ig2_sim::save::va::AKT_PLANET).max(0) as usize)
    }

    /// The starmap's ways to the colony screen (F3 `DoKeyboard` 0x4db3eb, the screen menu `MOUSE_SelectScreens`
    /// 0x55555f, the colony icon 0x5b5b51 and the planet picture 0x5b6533 of `Starmap_Main`): while the tutorial awaits
    /// 0x1c, the main planet (`Tutorial_MainBolygoNeve`) stays shut (false) and any other raises the event. The test
    /// reads `AktPlanet` ([`Self::akt_planet`]), whatever the screen.
    fn colony_tutorial_gate(&mut self) -> bool {
        let p = self.akt_planet();
        if self.world.game.state.mem.i32(ig2_sim::save::va::TUTORIAL_EVENT) == 0x1c {
            let main = self.tutorial.as_ref().map(|t| t.t.gates.main_planet.clone()).unwrap_or_default();
            let rec = self.world.game.state.mem.read(ig2_sim::save::va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, 0x18);
            if rec.split(|&b| b == 0).next() == Some(main.split(|&b| b == 0).next().unwrap_or_default()) {
                return false;
            }
            self.world.game.tutorial_check_event(0x1c);
        }
        true
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
                let mut ui = crate::colony_ui::ColonyUi::enter(&snap, own, &self.world.game.state);
                ui.cam_keys = self.cam_keep.reentered();
                if let Some(field) = self.world.game.state.ground_field(p) {
                    ui.war.seed_groups(field.units.iter().enumerate().map(|(slot, u)| (slot, u.tail.f13c)));
                }
                ui.war.banner = std::mem::take(&mut self.attack_banner);
                self.colony = Some(Colony { scene, view, ui, panel: Default::default() });
                self.send_watching(p);
            }
            Err(e) => {
                eprintln!("ig2-client: colony {p}: {e}");
                self.world.show_colony(None);
            }
        }
    }

    /// `Colony_Belep` (exe 0x465b40) over the colony that is up (back from the options screen; it reads no `OldKep`):
    /// the screen's state starts over as on any entry (the selection rule, the war groups from the units' bytes, the
    /// planet's ambience again); the camera is the one `Colony_Kilep` wrote to the planet, the view's, so it stays.
    fn reenter_colony(&mut self) {
        let (Some(c), Some(snap)) = (&mut self.colony, self.world.colony_curr.as_ref()) else { return };
        let own = snap.owner == self.world.curr.human && !snap.at_war;
        let mut ui = crate::colony_ui::ColonyUi::enter(snap, own, &self.world.game.state);
        ui.cam_keys = c.ui.cam_keys.reentered();
        if let Some(field) = self.world.game.state.ground_field(snap.planet) {
            ui.war.seed_groups(field.units.iter().enumerate().map(|(slot, u)| (slot, u.tail.f13c)));
        }
        ui.war.banner = std::mem::take(&mut c.ui.war.banner);
        c.ui = ui;
        let p = snap.planet;
        self.send_watching(p);
    }

    /// `Colony_Belep`'s `Multi_Send(0x26)`: MP tells the others which planet this machine's colony screen shows
    /// (`KiMelyikBolygonVan`, re/screens/colony.md §9.1); nothing clears it on leaving.
    fn send_watching(&mut self, p: usize) {
        if self.multiplayer() {
            self.world.send(ig2_sim::game::ColonyOrder::Watch { planet: p as i32 }.into());
        }
    }

    /// A key of the war (`ColonyKeyboardHandler` exe 0x4628c0) by its character, at war or not: S stops
    /// (`StopSelected`), `a` (not under the tutorial's colony lock 0xf66ff9) selects every own unit. The groups go by
    /// virtual key (`colony_camera_key_by`).
    fn colony_war_key(&self, ch: &str) -> Option<crate::colony_war::WarKey> {
        use crate::colony_war::WarKey;
        let c = self.colony.as_ref().filter(|_| self.world.colony_curr.is_some() && !self.under_screen())?;
        match ch {
            "s" | "S" => Some(WarKey::Stop),
            "a" if !c.ui.all_off => Some(WarKey::All),
            _ => None,
        }
    }

    /// The colony screen's input with its context; the outcome carried out.
    fn colony_input(&mut self, f: impl FnOnce(&mut crate::colony_ui::ColonyUi, &crate::colony_ui::Ctx, [f32; 2], u32) -> crate::colony_ui::Outcome) {
        let now = self.now_ms();
        self.colony_input_inner(f, now);
        // `WF::SaveSelection` writes the groups into the units' bytes `+0x13c + n`, which the save keeps
        // (re/screens/colony.md §11.2); in multiplayer each peer keeps its own (ADR 0007: no per-peer state in the sim).
        let mp = self.multiplayer();
        let Some((p, groups)) = self.colony.as_mut().zip(self.world.colony).and_then(|(c, p)| Some((p, c.ui.war.take_groups()?.clone()))) else { return };
        if mp {
            return;
        }
        if let Some(mut field) = self.world.game.state.ground_field(p) {
            for (slot, u) in field.units.iter_mut().enumerate() {
                for (n, g) in groups.iter().enumerate() {
                    u.tail.f13c[n] = g.contains(&slot) as u8;
                }
            }
            self.world.game.state.store_ground_field(p, field);
        }
    }

    fn colony_input_inner(&mut self, f: impl FnOnce(&mut crate::colony_ui::ColonyUi, &crate::colony_ui::Ctx, [f32; 2], u32) -> crate::colony_ui::Outcome, now: u32) {
        let (Some(c), Some(h), Some(snap)) = (&mut self.colony, &self.hud, &self.world.colony_curr) else { return };
        let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
        if let Some(gm) = self.maps.get(snap.map) {
            c.ui.war.project(&c.view, gm, snap, &c.scene.unit_centres);
        }
        c.ui.war.mods = crate::colony_war::Mods { shift: self.modifiers.shift_key(), ctrl: self.modifiers.control_key(), alt: self.modifiers.alt_key() };
        let ctx = crate::colony_ui::Ctx { state: &self.world.game.state, snap, rules: self.tables, sim: self.world.game.rules, texts: &h.texts, tech_name: &|t| h.tech_name(t) };
        let out = f(&mut c.ui, &ctx, at, now);
        if out.click {
            self.audio.play(crate::audio::SoundId::global(0));
        }
        for cmd in out.commands {
            self.world.send(cmd);
        }
        if let Some(e) = out.error {
            self.scroll_error(e);
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
        match crate::spy_screen::SpyView::open(&mut g.renderer, &self.install, h, &d, true, &mut self.spy_scene) {
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
        // From the diplomacy screen (`Diplomacy_Kilep`): the closing first, except towards the messages screen.
        let closing = self.overlay == Some(crate::ui::Screen::Diplomacy) && !self.leave_diplomacy(Some(s));
        if !closing {
            self.overlay = if self.overlay == Some(s) { None } else { Some(s) };
            if self.overlay == Some(crate::ui::Screen::Messages) {
                // `Messages_Belep`'s saved part (re/rules/info-messages.md §7.3); `VoltUjValasz = −1`.
                self.world.send(ig2_sim::game::MessageOrder::Enter.into());
                if let Some(h) = &mut self.hud {
                    h.messages.pending = None;
                    h.messages.enter(true);
                }
            }
        }
        let next = if closing { Some(s) } else { self.overlay };
        if let (Some(crate::ui::Screen::Info), Some(h)) = (next, &mut self.hud) {
            // `InfoTriX/Y` keep the knob where it was left (`Info_Belep` only moves the model there).
            let tri = h.info.tri;
            h.info = crate::ui::InfoScreen::enter(self.world.ui.human);
            h.info.tri = tri;
        }
    }

    /// The diplomacy screen enters (`Diplomacy_Belep`; `animated` = with its opening, not back from the messages
    /// screen).
    fn enter_diplomacy(&mut self, animated: bool) {
        if let (Some(g), Some(h)) = (&mut self.gpu, &self.hud) {
            let d = self.diplomacy.get_or_insert_with(|| crate::diplomacy::Open::new(&mut g.renderer, &self.install, self.tables, &h.texts, &self.world.game.state));
            d.enter(self.tables, &h.texts, &self.world.game.state, animated);
            self.overlay = Some(crate::ui::Screen::Diplomacy);
        }
    }

    /// Leave the diplomacy screen for `next` (`None` = the starmap): true = gone now (towards the messages screen, or
    /// already closed); else its closing runs and the frame loop goes on to `next` when it ends.
    fn leave_diplomacy(&mut self, next: Option<crate::ui::Screen>) -> bool {
        let quiet = next == Some(crate::ui::Screen::Messages);
        let Some(d) = &mut self.diplomacy else { return true };
        if d.screen.leave(quiet) {
            return true;
        }
        self.dip_next = next;
        false
    }

    /// A left press on the info or messages screen (`Info_Main`, `Messages_Main`).
    fn screen_press(&mut self, screen: crate::ui::Screen) {
        let at = self.virtual_cursor();
        let Some(h) = &mut self.hud else { return };
        let mut leave = false;
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
            // ADR 0040: the event log's label and, while it is up, its panel take the presses.
            crate::ui::Screen::Messages if self.display.event_log && h.messages.list_up() && (h.messages.log.open || crate::event_log::Panel::over_label(at)) => {
                if crate::event_log::Panel::over_label(at) {
                    h.messages.log.toggle();
                    self.sounds.push(crate::audio::CLICK);
                } else if let Some(t) = h.messages.log.press(at, &self.world.log, h.texts.lang) {
                    self.log_jump(t);
                }
            }
            crate::ui::Screen::Messages => match h.messages.press(at, self.world.ui.inbox.len(), ig2_sim::game::inbox_arrows_allowed(&self.world.game.state)) {
                // The sim refuses the row while a dialogue or an automatic message holds the screen.
                crate::messages_screen::Press::Open(i) if ig2_sim::game::inbox_open_allowed(&self.world.game.state) => h.open_message(&self.install, &self.world.ui, i),
                crate::messages_screen::Press::Open(_) => {}
                crate::messages_screen::Press::Leave => leave = true,
                crate::messages_screen::Press::None => {}
            },
            _ => {}
        }
        if leave {
            self.leave_messages();
        }
    }

    /// Back to `MessagesElottiScreen` (hot-spots 7..=9, Esc): `Messages_Kilep`'s closing runs first.
    fn leave_messages(&mut self) {
        if self.kep_kell.is_none() {
            self.request_screen(akt_kep(None, self.colony.is_some(), false));
        }
    }

    /// A right press on the info screen's buttons: `Info_Main` (exe 0x4e81c0) takes `Mouse_Lenyom`, either button's, on
    /// the rows 9..=12, the mode and chart buttons 14..=25, the arrows 30/31 and 35/36 and the back button 37 (FX3).
    fn info_right_press(&mut self) {
        let at = self.virtual_cursor();
        let Some(h) = &mut self.hud else { return };
        let id = h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui);
        if matches!(id, Some(9..=12 | 14..=25 | 30 | 31 | 35 | 36 | 37)) {
            h.info.press(id, &self.world.ui.info, self.world.ui.human, at[1]);
        }
    }

    /// A right press, or a release, on the info screen (`Info_Main`'s triangle reset, drags, menus).
    fn info_button(&mut self, press: bool, right: bool) {
        let at = self.virtual_cursor();
        let Some(h) = &mut self.hud else { return };
        let id = h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui);
        self.info_down = press;
        if !press {
            h.info.release(id, &self.world.ui.info, self.world.ui.human);
        }
        let b = crate::info_screen::Buttons { press, release: !press, down: press, left: !right, right };
        for c in h.info.orders(id, at, b, &self.world.ui.info, self.world.ui.human) {
            self.world.send(c);
        }
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

    /// `Research_Belep` from screen `from` (`OldKep`): animated (`KELLKINYITASDEFAULT`) unless from the messages, help
    /// or options screen.
    fn enter_research(&mut self, from: i32) {
        if self.tech_infos.is_empty() {
            self.tech_infos = crate::research::tech_infos(&self.install);
        }
        if let (true, Some(h)) = (self.research_tips.is_empty(), &self.hud) {
            self.research_tips = crate::research::tips(&h.texts);
        }
        let place = self.research_place.unwrap_or_else(|| crate::research::Place::read(&self.world.game.state));
        self.research = Some(crate::research::Research::open_at(&self.world.game.state, self.tables, opens_animated(from), place));
        self.overlay = Some(crate::ui::Screen::Research);
    }

    /// The research screen goes; its place stays for the next visit (`Research_Kilep` leaves the globals as they are).
    fn drop_research(&mut self) {
        if let Some(r) = self.research.take() {
            self.research_place = Some(r.place());
        }
    }

    /// Run `f` on the design screen with what it reads ([`crate::design_edit::Ctx`]) and its hot-spot mask, then send
    /// the orders it made.
    fn design_do<R>(&mut self, f: impl FnOnce(&mut crate::design::Design, &crate::design_edit::Ctx, &[u8]) -> R) -> Option<R> {
        let state = &self.world.game.state;
        let techs = state.tech();
        let d = self.design.as_mut()?;
        let c = crate::design_edit::Ctx {
            install: &self.install,
            state,
            techs: &techs,
            rules: self.tables,
            sim: &self.world.game.rules.ai,
            debug: state.mem.i32(ig2_sim::save::va::DESIGN_DEBUG) != 0,
            texts: self.hud.as_ref().map(|h| &h.texts),
        };
        let mask = self.design_scene.as_ref().map_or(&[][..], |s| &s.mask[..]);
        // `Design_Main`'s delete (spot 4) waits for `!TutorialPaused`.
        d.tutorial_paused = state.mem.u8(ig2_sim::save::va::TUTORIAL_PAUSED) != 0;
        let r = f(d, &c, mask);
        let events = std::mem::take(&mut d.tutorial);
        for o in std::mem::take(&mut d.orders) {
            self.world.send(Command::from(o));
        }
        for id in events {
            self.world.game.tutorial_check_event(id);
        }
        Some(r)
    }

    /// The design screen goes; the orders it sent that are still on their way are delivered.
    fn drop_design(&mut self) {
        if let Some(mut d) = self.design.take() {
            for o in d.flush_orders() {
                self.world.send(Command::from(o));
            }
        }
    }

    /// A key in the design screen's name input; the viewing mode's input writes the stored design's name in place
    /// (re/deviations.md).
    fn design_entry_key(&mut self, k: &Key<&str>) {
        use crate::design_edit::EntryKey;
        let key = match k {
            Key::Named(NamedKey::Enter) => {
                if let Some(d) = &mut self.design {
                    d.entry = None;
                }
                return;
            }
            Key::Named(NamedKey::Backspace) => EntryKey::Back,
            Key::Named(NamedKey::Delete) => EntryKey::Delete,
            Key::Named(NamedKey::Home) => EntryKey::Home,
            Key::Named(NamedKey::End) => EntryKey::End,
            Key::Named(NamedKey::ArrowLeft) => EntryKey::Left,
            Key::Named(NamedKey::ArrowRight) => EntryKey::Right,
            Key::Named(NamedKey::Space) => EntryKey::Char(b' '),
            Key::Character(c) => {
                let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                match b.first().copied().filter(|&b| b >= 0x20 && b != 0x81) {
                    Some(b) => EntryKey::Char(b),
                    None => return,
                }
            }
            _ => return,
        };
        let Some(d) = &mut self.design else { return };
        if let Some((k, name)) = d.entry_key(key) {
            // Multiplayer: the shared state changes only through the lockstep (deviation 96).
            if self.world.net.is_some() {
                let n = name.iter().take_while(|&&c| c != 0).take(0x15).copied().collect();
                self.world.send(Command::RenameDesign { index: k as u32, name: n });
                return;
            }
            let state = &mut self.world.game.state;
            let en = state.mem.i32(ig2_sim::save::va::EN).clamp(0, 18) as usize;
            if let Some(r) = state.designs.get_mut(en).and_then(|t| t.get_mut(k)) {
                let n = name.len().min(0x15);
                r[..n].copy_from_slice(&name[..n]);
                r[n] = 0;
            }
        }
    }

    /// `Design_Belep` from screen `from` (`OldKep`; `desKELLKINYITAS`, re/screens/design.md §3).
    fn enter_design(&mut self, from: i32) {
        if let (true, Some(h)) = (self.design_tips.is_empty(), &self.hud) {
            self.design_tips = crate::design::tips(&h.texts);
        }
        self.drop_research();
        let mut d = crate::design::Design::open(&self.world.game.state, opens_animated(from));
        if let Some(h) = &self.hud {
            d.names = crate::design_edit::Names::read(&h.texts);
        }
        self.design = Some(d);
        self.overlay = Some(crate::ui::Screen::Design);
    }

    /// `Trade_Belep` from screen `from` (`TradeElottiKep = OldKep`; `traKELLKINYITAS`, re/screens/trade.md §5).
    fn enter_trade(&mut self, from: i32) {
        let Some(h) = &self.hud else { return };
        if self.trade_tips.is_empty() {
            self.trade_tips = crate::trade::tips(&h.texts);
        }
        let advance = h.font.as_ref().map_or(6.0, |f| f.advance(2));
        let lines = crate::trade::ticker_lines(&h.texts);
        self.drop_research();
        self.drop_design();
        self.trade = Some(crate::trade::Trade::open(&self.world.game.state, opens_animated(from), from, lines, advance));
        self.overlay = Some(crate::ui::Screen::Trade);
    }

    /// What the top bar's drop-downs show now (re/screens/hud.md §2.1).
    fn top_menus_data(&self) -> Option<crate::topbar_menus::Data> {
        let h = self.hud.as_ref()?;
        let screen = akt_kep(if self.help.is_some() { Some(crate::ui::Screen::Help) } else if self.options.is_some() { Some(crate::ui::Screen::Options) } else { self.overlay }, self.colony.is_some(), self.spacewar.is_some());
        let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
        Some(crate::topbar_menus::Data::read(&self.world.game.state, self.tables, &h.texts, screen, gates, self.spacewar.is_some()))
    }

    /// Every frame a drop-down is open: its data again and `Selected` under the pointer (`MOUSE_SelectScreens` runs
    /// every held frame).
    fn top_menus_frame(&mut self) {
        self.akt_planet();
        self.top_bar_frame();
        if self.hud.as_ref().is_none_or(|h| h.top.open == crate::topbar_menus::Open::None) {
            return;
        }
        // `MOUSE_SelectScreens`' tail (exe 0x5557ce..): the research menu (`e40c5d`) shuts on any frame with no button
        // down (`Mouse_Gomb` 0xdb4650), a release lost outside the window included; the screen menu waits for one.
        if self.mouse_held == 0 {
            if let Some(h) = self.hud.as_mut().filter(|h| h.top.open == crate::topbar_menus::Open::Research) {
                h.top.open = crate::topbar_menus::Open::None;
                return;
            }
        }
        let at = self.virtual_cursor().map(|c| c as i32);
        let d = self.top_menus_data();
        if let (Some(h), Some(d)) = (&mut self.hud, d) {
            h.top.moved(at, &d);
            h.top_data = d;
        }
    }

    /// A button on the top bar's drop-downs (hot-spots 6 and 8, `MOUSE_SelectScreens`): true = taken. The pick asks
    /// for a screen the way its key does, or sends the research order.
    fn top_menus_button(&mut self, button: MouseButton, pressed: bool) -> bool {
        use crate::topbar_menus::{Open, Pick};
        if self.front.is_some() || self.hud.is_none() || !matches!(button, MouseButton::Left | MouseButton::Right) {
            return false;
        }
        let at = self.virtual_cursor().map(|c| c as i32);
        let open = self.hud.as_ref().is_some_and(|h| h.top.open != Open::None);
        if !open && (!pressed || crate::topbar_menus::spot(at).is_none()) {
            return false;
        }
        let Some(d) = self.top_menus_data() else { return false };
        let Some(h) = &mut self.hud else { return false };
        let left = button == MouseButton::Left || self.mouse_held & 1 != 0;
        let pick = if pressed { h.top.press(at, left, &d) } else { h.top.release(at, &d) };
        h.top_data = d;
        match pick {
            Pick::Pass => return false,
            Pick::Taken => {}
            // The arrow's right press asks for the starmap with no tutorial gate; the menu's pick has one.
            Pick::Screen(n) if pressed => self.request_screen(n),
            Pick::Screen(n) => self.top_menu_screen(n),
            Pick::Start(t) => self.quick_research(t),
            Pick::Stop(t) => self.world.send(Command::Research { order: ig2_net::ResearchCmd::Stop { tech: t } }),
        }
        true
    }

    /// The quick-research menu's start: `ResearchAtvaltTalalmanyra(t)`; on the research screen its start button goes
    /// in, with the screen down the place it opens at next moves (exe 0x521f20).
    fn quick_research(&mut self, t: i32) {
        if let Some(r) = &mut self.research {
            r.quick_start(t);
        } else {
            let state = &self.world.game.state;
            self.research_place.get_or_insert_with(|| crate::research::Place::read(state)).jump_to(t, self.tables);
        }
        self.world.send(Command::Research { order: ig2_net::ResearchCmd::Start { tech: t } });
    }

    /// Ours (ADR 0065): the modern starmap HUD is up (its option on, the starmap shown with nothing over it).
    fn modern_up(&self) -> bool {
        self.display.modern_starmap && self.front.is_none() && self.colony.is_none() && self.overlay.is_none() && self.spacewar.is_none() && self.spy.is_none() && self.help.is_none() && self.options.is_none() && self.view.is_some()
    }

    /// The pointer is over one of the modern HUD's panels (its last frame's).
    fn over_gui(&self) -> bool {
        self.modern_up() && self.gpu.as_ref().is_some_and(|g| g.gui.ctx.is_pointer_over_area())
    }

    /// The pointer is over the modern HUD's info card.
    fn over_info_card(&self) -> bool {
        self.gpu.as_ref().is_some_and(|g| {
            let at = g.gui.ctx.input(|i| i.pointer.latest_pos());
            let card = g.gui.ctx.memory(|m| m.area_rect(egui::Id::new("smh-info")));
            at.zip(card).is_some_and(|(p, r)| r.contains(p))
        })
    }

    /// The modern HUD's acts of its last frame, through the original's hot-spots' code.
    fn modern_apply(&mut self) {
        use crate::starmap_modern::Act;
        if !self.modern_up() {
            self.modern_acts.clear();
            if let Some((s, _)) = self.modern_hold.take() {
                self.console_let_go(s);
            }
            return;
        }
        for a in std::mem::take(&mut self.modern_acts) {
            let time = self.time_keys_allowed() || self.multiplayer();
            let locked = self.starmap_locked();
            match a {
                Act::Pause if time => self.toggle_pause(),
                Act::Speed(n) if time => self.set_speed(n),
                Act::Messages => self.messages_from_bar(),
                Act::History(on) => {
                    if let Some(h) = &mut self.hud {
                        h.history_open = on;
                    }
                }
                Act::Screen(n) => self.top_menu_screen(n),
                Act::Start(t) => self.quick_research(t),
                Act::Stop(t) => self.world.send(Command::Research { order: ig2_net::ResearchCmd::Stop { tech: t } }),
                Act::Hold(h) if h != self.modern_hold => {
                    if let Some((s, _)) = self.modern_hold.take() {
                        self.console_let_go(s);
                    }
                    if let Some((s, left)) = h {
                        self.console_act(s, left, [0.0; 2]);
                    }
                    self.modern_hold = h;
                }
                Act::View(s) => {
                    self.star_buttons.press(s);
                    self.console_view(s);
                }
                Act::Minimap(at, first) => {
                    if first {
                        self.console_act(7, true, at);
                        self.star_buttons.release();
                    } else {
                        let t = orders::minimap_target(at);
                        let zoom = self.camera3()[2];
                        self.tween_to([t[0], t[1], zoom], 4);
                    }
                }
                Act::Info(right) if !locked && !self.menus.info_on() && self.menus.rename.is_none() && !self.wait_release => {
                    if right {
                        self.info_box_click();
                    } else {
                        self.info_box_left();
                    }
                }
                Act::Panel(id) if !locked => {
                    let sel = self.selection();
                    if let Some(acts) = self.panels.press_id(&self.world.game.state, &sel, id, [0.0; 2], true) {
                        self.apply(acts);
                        let acts = self.panels.release(&self.world.game.state, &sel);
                        self.apply(acts);
                    }
                }
                _ => {}
            }
        }
    }

    /// The screen menu's pick and the arrow's right press (`AktKepKell = n`, [`Self::request_screen`]). While the
    /// tutorial awaits event 0x1c the pick is refused on the main planet and raises the event elsewhere, whatever the
    /// screen (`MOUSE_SelectScreens` exe 0x55555f).
    fn top_menu_screen(&mut self, n: i32) {
        if self.world.game.state.mem.i32(ig2_sim::save::va::TUTORIAL_EVENT) == 0x1c && !self.colony_tutorial_gate() {
            return;
        }
        self.request_screen(n);
    }

    /// A press on the research screen (`Research_Main`, the lists, the models).
    /// A press on the research screen (`left`: `Mouse_Bal`); the right button also grips the ball.
    fn research_press(&mut self, left: bool) {
        let at = self.virtual_cursor();
        let (Some(r), Some(m)) = (&mut self.research, &self.research_models) else { return };
        if left {
            r.held = true;
        }
        r.buttons |= if left { 1 } else { 2 };
        // `Mouse_Bal`: the left button held, whichever went down.
        let bal = left || self.mouse_held & 1 != 0;
        for a in r.press(&self.world.game.state, self.tables, &m.mask, at, &m.centres, bal) {
            let crate::research::Act::Send(c) = a;
            self.world.send(c);
        }
    }

    /// `ResearchKeyboardHandler`'s `oldEnterlenyomva` follows every event the handler sees: every key up, and the key
    /// downs and characters the exe queues that aren't the screen's own keys (those go through `research_key`).
    fn research_key_seen(&mut self, e: &winit::event::KeyEvent) {
        let Some(r) = self.research.as_mut().filter(|_| self.overlay == Some(crate::ui::Screen::Research)) else { return };
        let down = e.state == ElementState::Pressed;
        if down && (!do_keyboard::exe_queues_key_down(e) || matches!(e.logical_key.as_ref(), Key::Named(n) if research_key(n).is_some())) {
            return;
        }
        r.key_seen(down);
    }

    fn research_key(&mut self, k: crate::research::Key) {
        let Some(r) = &mut self.research else { return };
        for a in r.key(&self.world.game.state, k) {
            let crate::research::Act::Send(c) = a;
            self.world.send(c);
        }
    }

    /// `MultiPlayerMode`.
    /// `DoKeyboard`'s chat entry in multiplayer (`IOMode` 1, exe 0x4db1a0 cases 0xd and 0x60): Enter or the
    /// backquote opens it on a game screen (`AktKep` < 0xd); while open the keys type into it and Enter sends the line
    /// (`ExecuteString`, `crate::console`: the host's `/kick`, else the 0x3fc chat), Esc drops it; the F-keys still switch
    /// screens, which closes it (`IG2_gameUpdate`). True when the key was taken. Enter that opens it still reaches
    /// the screen (the exe's case 0xd falls into the screen's handler); the backquote doesn't.
    fn chat_key(&mut self, key: &Key<&str>) -> bool {
        use crate::chat::EntryKey as E;
        let changing = self.screen_changing();
        let Some(h) = &mut self.hud else { return false };
        let Some(e) = &mut h.chat.entry else {
            let opens = matches!(key, Key::Named(NamedKey::Enter) | Key::Character("`"));
            if opens && self.world.net.is_some() && self.options.is_none() && self.front.is_none() {
                h.chat.entry = Some(Default::default());
                return matches!(key, Key::Character("`"));
            }
            return false;
        };
        let k = match key {
            Key::Character(s) => s.bytes().next().map(E::Char),
            Key::Named(NamedKey::Space) => Some(E::Char(b' ')),
            Key::Named(NamedKey::Backspace) => Some(E::Backspace),
            Key::Named(NamedKey::Delete) => Some(E::Delete),
            Key::Named(NamedKey::Home) => Some(E::Home),
            Key::Named(NamedKey::End) => Some(E::End),
            Key::Named(NamedKey::ArrowLeft) => Some(E::Left),
            Key::Named(NamedKey::ArrowRight) => Some(E::Right),
            Key::Named(NamedKey::Enter) => Some(E::Enter),
            // `DoKeyboard` case 0x1b (exe 0x4db1a0): nothing while a screen change is pending.
            Key::Named(NamedKey::Escape) if changing => None,
            Key::Named(NamedKey::Escape) => Some(E::Escape),
            Key::Named(NamedKey::F1 | NamedKey::F2 | NamedKey::F3 | NamedKey::F4 | NamedKey::F5 | NamedKey::F6 | NamedKey::F7 | NamedKey::F8 | NamedKey::F9 | NamedKey::F10 | NamedKey::F11) => {
                h.chat.entry = None;
                return false;
            }
            _ => None,
        };
        if let Some(line) = k.and_then(|k| e.key(k)) {
            h.chat.entry = None;
            if !line.is_empty() {
                let races: Vec<Vec<u8>> = (0..20).map(|r| h.texts.get(0x11, 1, r + 5)).collect();
                self.world.say(&line, &races);
            }
        }
        true
    }

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
                Action::Tutorial(id) => self.world.game.tutorial_check_event(id),
                Action::ZoomIn { x, y, zoom } => {
                    if let (Some(v), None) = (&self.view, self.cam_saved) {
                        // `StarmapBeZoomol`: the zoom sound when not zoomed in yet (`StarmapZoomMode`).
                        self.sounds.push(crate::audio::SoundId::global(5));
                        self.cam_saved = Some([v.camera.x, v.camera.y, v.camera.zoom]);
                    }
                    self.tween_to([x, -y, zoom], 8);
                }
                Action::ZoomOut => {
                    if let Some(s) = self.cam_saved.take() {
                        // `StarmapKiZoomol`'s sound.
                        self.sounds.push(crate::audio::SoundId::global(5));
                        self.tween_to(s, 8);
                    }
                }
                Action::Design(o) => self.world.send(Command::from(o)),
                // The exe calls `Story_Special` itself, in single player only (no message).
                Action::HeroBack { fleet, design } => {
                    if !self.multiplayer() {
                        self.world.send(Command::HeroSentBack { fleet, design });
                    }
                }
                Action::StoryIcon { icon, target, click } => {
                    if icon == 0x12 || !self.multiplayer() {
                        if click {
                            self.sounds.push(crate::audio::CLICK);
                        }
                        self.world.send(Command::StoryIcon { icon, target });
                    }
                }
                Action::Centre { x, y } => {
                    let zoom = self.camera3()[2];
                    self.tween_to([x, -y, zoom], 8);
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
                // The colony icon (`AktRopIkon == 4`), not while `KepernyokLetiltva[3]` (exe 0x5b5af4).
                Do::Colony => {
                    if self.colony.is_none() && !self.tutorial_screen_off(3) {
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

    /// `TargetXStart/YStart/Zoom` = `t` over `frames` frames (`Mozgasd`), `TargetRotate/Pitch` the current turn and
    /// tilt (as every caller but the console's views sets them).
    fn tween_to(&mut self, t: [f32; 3], frames: i32) {
        if let Some(v) = &self.view {
            self.cam_tween = Some(([t[0], t[1], t[2], v.camera.rotate, v.camera.pitch], frames));
        }
    }

    /// The camera as `[XStart, YStart, Zoom]`.
    fn camera3(&self) -> [f32; 3] {
        self.view.as_ref().map_or([0.0; 3], |v| [v.camera.x, v.camera.y, v.camera.zoom])
    }

    /// `ScrollHibaUzenet` (exe 0x556b20): `PlaySoundGlobal(6)`, the error text `HibaUzenetSzoveg` = `text`, its counter 0.
    fn scroll_error(&mut self, text: Vec<u8>) {
        self.audio.play(crate::audio::SoundId::global(6));
        if let Some(h) = &mut self.hud {
            h.error = Some((text, 0));
        }
    }

    /// The sale menus' texts (`DesignTorlesValasztasNev`).
    fn sale_names(&self) -> crate::starmap_info::SaleNames {
        let messages = &self.world.game.rules.messages;
        self.hud.as_ref().map(|h| crate::starmap_info::SaleNames::load(&h.texts, |n| messages.story_text(n).to_vec())).unwrap_or_default()
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
        // The tutorial's DISABLE STARMAP: `Starmap_MouseInit2` (exe 0x5b898f) switches every hot-spot off and only
        // 1..=7 back on (the console and the minimap; not under the long info): no picks, panels or info box.
        if self.starmap_locked() {
            return self.console_press(true) || at[1] >= 20.0;
        }
        if self.menus.info_on() {
            // Hot-spots 8 (left), 9 and 11 leave; the panel's buttons and menus; the rest of the screen is caught.
            let cam = self.camera3();
            // Leaving raises the tutorial's 0x16 (exe 0x5b3035), then plays the zoom sound (exe 0x5b303c).
            if (at[0] < 166.0 && at[1] >= 424.0) || crate::starmap_menus::exit_spot(at) {
                self.world.game.tutorial_check_event(0x16);
                self.sounds.push(crate::audio::SoundId::global(5));
                self.menus.leave_info(cam, false);
            } else if (422.0..466.0).contains(&at[0]) && (432.0..476.0).contains(&at[1]) {
                self.world.game.tutorial_check_event(0x16);
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
        // Hot-spots 1..=7 are the lowest ids: they win over the panels' and the map's (`Mouse_Darabol`).
        if self.console_press(true) {
            return true;
        }
        if self.panel_press(true) {
            self.panel_held = Some(true);
            return true;
        }
        // The rest of the top bar, the info box and the minimap act on the release (not under the modern HUD, ADR 0065).
        let classic = !self.modern_up();
        if (classic && (at[1] < 20.0 || (at[0] < 166.0 && at[1] >= 424.0))) || (self.panels.transfer.is_some() && at[1] >= 20.0) {
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
            // `Starmap_Main` (ELF 0x6f8e3b): a fleet of the player's picked speaks its line's select.
            if s.mode == 2 && self.world.game.state.fleets.get(s.fleet).is_some_and(|f| f[0x15] == self.world.curr.human) {
                self.speech.push(crate::audio::speech::Speak { who: crate::audio::speech::Who::Starmap, line: s.fleet as i32 % 3 + 1, kind: crate::audio::speech::starmap::SELECT });
            }
            let blink = (self.now_ms() / 100) % 2 == 1;
            let radar = self.world.game.en_radar_range();
            let places = crate::starmap_menus::rop_places(&self.world.game.state, &s, &self.panels, self.menus.redirect, (self.menus.sat_sent, self.menus.col_sent), blink, radar);
            self.menus.open_rop(at, places);
        }
        true
    }

    /// The left button went up after [`Self::starmap_press`]: an open select menu sends, the radial menu fires its
    /// icon.
    fn starmap_release(&mut self) {
        // Also under DISABLE STARMAP (hot-spots off, exe 0x5b898f): an open menu holds `Mouse_Megfogva`, so
        // `IG2_gameUpdate` keeps the pressed hot-spot as `AktIkon` (exe 0x4e0440, no `Mouse_Darabol` while held) and
        // `Starmap_Main`'s release arm still runs.
        let sel = self.selection();
        if self.menus.menu.is_some() {
            let dos = self.menus.release_menu(&self.world.game.state, &sel);
            self.apply_do(dos);
        }
        if self.menus.rop.is_some() {
            let mut icon = self.menus.release_rop();
            // `Starmap_Main`'s production (2) and split (0xd) arms wait for the tutorial's gates.
            let locks = self.panels.locks;
            if (icon == 2 && locks.production) || (icon == 0xd && locks.split) {
                icon = 0;
            }
            let dos = self.menus.fire(icon, &self.world.game.state, &sel);
            self.apply_do(dos);
        }
    }

    /// The pointer in virtual pixels.
    fn virtual_cursor(&self) -> [f32; 2] {
        self.view.as_ref().map_or([0.0; 2], |v| v.camera.virtual_of(self.cursor))
    }

    /// `TutorialStarmapMindenLetiltvaKiveveAKurzorMozgatoBillentyuket` (exe 0xf66ff8, DISABLE STARMAP): read only by
    /// `Starmap_MouseInit2` (exe 0x5b898f) and `StarmapKeyboardHandler` (exe 0x5aed29).
    fn starmap_locked(&self) -> bool {
        self.tutorial.as_ref().is_some_and(|t| t.t.gates.starmap)
    }

    /// A press on the starmap: the panels' hot-spots take it first (none under the tutorial's starmap lock).
    fn panel_press(&mut self, left: bool) -> bool {
        if self.starmap_locked() {
            return false;
        }
        let at = self.virtual_cursor();
        let sel = self.selection();
        // Under the modern HUD the info box's spots (8, the tanks' 10, production's 11) are its card's (ADR 0065).
        if self.modern_up() && self.panels.spot(&self.world.game.state, &sel, at).is_some_and(|id| matches!(id, 8 | 10 | 11)) {
            return false;
        }
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
            // Not while `KepernyokLetiltva[3]` (exe 0x5b64da).
            if !self.tutorial_screen_off(3) {
                self.toggle_colony();
            }
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
        // `TutorialSplitDisabled`'s second site (exe 0x5b6d90).
        if mine && calm && closed.is_none_or(|t| t.tank_mode) && !self.panels.locks.split {
            if closed.is_none() && !had_info {
                self.apply(vec![Action::ZoomIn { x: pos.0, y: pos.1, zoom: 590.0 }]);
            }
            self.send(StarmapOrder::Split { fleet: sel.fleet as i32 });
        }
    }

    /// The info box clicked with the left button: the fleet's info panel, or in planet mode the long info.
    fn info_box_left(&mut self) {
        let sel = self.selection();
        if sel.mode == 2 {
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
    }

    /// A click on the starmap screen at physical `p` (left: buttons, minimap, selection; right: an order).
    /// The starmap's info box (hot-spot 8 at (0, 424, 166, 56)) under physical `p` (none under the modern HUD).
    fn on_info_box(&self, p: [f32; 2]) -> bool {
        !self.modern_up() && self.view.as_ref().is_some_and(|v| {
            let at = v.camera.virtual_of(p);
            at[0] >= 0.0 && at[0] < 166.0 && at[1] >= 424.0
        })
    }

    fn starmap_click(&mut self, p: [f32; 2], right: bool) {
        if self.starmap_locked() {
            return;
        }
        let Some(v) = &self.view else { return };
        let at = v.camera.virtual_of(p);
        let keys = Keys { shift: self.modifiers.shift_key(), ctrl: self.modifiers.control_key(), alt: self.modifiers.alt_key() };
        let sel = self.selection();
        if self.menus.info_on() {
            return;
        }
        // The info box (hot-spot 8 at (0, 424, 166, 56)): right = colony / split, left in planet mode = the long info.
        if at[0] >= 0.0 && at[0] < 166.0 && at[1] >= 424.0 && !self.modern_up() {
            if right {
                self.info_box_click();
            } else {
                self.info_box_left();
            }
            return;
        }
        // The transfer panel covers the map (hot-spot 0x58); under an info panel the map takes no orders.
        if (self.panels.transfer.is_some() || self.panels.info.is_some()) && at[1] >= 20.0 {
            return;
        }
        if right {
            // In planet mode on the player's planet the right button picks the rally planet (`0x400`), redirect mode or
            // not; in fleet mode the redirect mode gives way to the fleet's orders (`Starmap_Main` exe 0x5b53e2).
            let own = sel.mode == 1 && self.world.curr.planets.iter().any(|p| p.index == sel.planet && p.owner == self.world.curr.human);
            if own {
                // The rally pick clicks before it picks (exe 0x5b5406).
                self.sounds.push(crate::audio::CLICK);
                let target = orders::select_nearest_planet(&self.world.curr, &v.camera, at, 1.0);
                let dos = self.menus.redirect_to(&self.world.game.state, &sel, target);
                self.apply_do(dos);
            } else if let Some(o) = orders::right_click(&self.world.curr, &v.camera, at, &sel, keys) {
                // `Starmap_Main` (exe 0x5b4ca6 / 0x5b4d11): an enemy planet or fleet picked for the attack (no Ctrl, no
                // Alt) raises the tutorial's 0x11, then 0x1f.
                if matches!(o, StarmapOrder::Move { order: 2, kind: 1 | 2, .. }) && !keys.ctrl {
                    self.world.game.tutorial_check_event(0x11);
                    self.world.game.tutorial_check_event(0x1f);
                }
                // An own fleet's order (`StarmapVillogasMode` 1, 0xd, 0xe): a click with Shift (exe 0x5b51b3), else the
                // fleet's move line; an attack (2): its attack line (exe 0x5b5231).
                let own_fleet = sel.mode == 2 && self.world.game.state.fleets.get(sel.fleet).is_some_and(|f| f[0x15] == self.world.curr.human);
                if let (true, StarmapOrder::Move { order, .. }) = (own_fleet, &o) {
                    use crate::audio::speech::{starmap, Speak, Who};
                    let line = sel.fleet as i32 % 3 + 1;
                    match *order {
                        1 | 0xd | 0xe if keys.shift => self.sounds.push(crate::audio::CLICK),
                        1 | 0xd | 0xe => self.speech.push(Speak { who: Who::Starmap, line, kind: starmap::MOVE }),
                        2 => self.speech.push(Speak { who: Who::Starmap, line, kind: starmap::ATTACK }),
                        _ => {}
                    }
                }
                self.send(o);
            }
        }
        // The console and the minimap (hot-spots 1..=7) are `console_press`'s.
    }

    /// The console's and the minimap's hot-spot (1..=7) under virtual `at`, while they are on (not under the long
    /// info, the production view, the transfer or the fleet's panel): `Mouse_Darabol` reads `STAR_M.RAW`'s colour
    /// there, their rectangles ignored (re/screens/starmap.md §10.3).
    fn console_spot(&mut self, at: [f32; 2]) -> Option<u32> {
        if self.panels.minimap_off() || self.menus.info_on() || self.modern_up() {
            return None;
        }
        let install = &self.install;
        let mask = self.star_mask.get_or_insert_with(|| install.read("dataraw/screen/star_m.raw").ok().and_then(|b| b.get(800..800 + 640 * 480).map(|x| x.to_vec())).unwrap_or_default());
        let (x, y) = (at[0].clamp(0.0, 639.0) as usize, at[1].clamp(0.0, 479.0) as usize);
        crate::orders::console_spot(*mask.get(y * 640 + x)?)
    }

    /// A press on the console or the minimap (`Starmap_Main` ELF 0x6f50a0 lines 163-285): the held buttons set the
    /// camera flags (rotate; zoom and pitch in with the left button, out with the right), the minimap retargets
    /// the view; the views (1, 4) act on the release. True when taken.
    fn console_press(&mut self, left: bool) -> bool {
        let at = self.virtual_cursor();
        let Some(s) = self.console_spot(at) else { return false };
        self.console_act(s, left, at);
        self.console_held = Some((s, left));
        true
    }

    /// Console spot `s` pressed (`left`: with the left button; the minimap's at virtual `at`).
    fn console_act(&mut self, s: u32, left: bool, at: [f32; 2]) {
        let k = &mut self.star_keys;
        match s {
            2 => (k.right, k.left) = (true, false),
            3 => (k.left, k.right) = (true, false),
            5 => (k.zoom_in, k.zoom_out) = (left, !left),
            6 => (k.up, k.down) = (left, !left),
            7 => {
                let t = orders::minimap_target(at);
                let zoom = self.camera3()[2];
                self.tween_to([t[0], t[1], zoom], 4);
            }
            _ => {}
        }
        if s != 1 && s != 4 {
            self.sounds.push(crate::audio::CLICK);
            self.star_buttons.press(s);
        }
    }

    /// The button went up over a held console spot: its flags clear; the top view (1) and the side view (4) start
    /// when released over their button (`StarmapSetTopView`, `StarmapSetSideView`, 8 frames).
    fn console_release(&mut self) {
        let Some((s, _)) = self.console_held.take() else { return };
        self.console_let_go(s);
        let at = self.virtual_cursor();
        if !matches!(s, 1 | 4) || self.console_spot(at) != Some(s) {
            return;
        }
        self.console_view(s);
    }

    /// A held console spot let go: its camera flags clear.
    fn console_let_go(&mut self, s: u32) {
        self.star_buttons.release();
        let k = &mut self.star_keys;
        match s {
            2 | 3 => (k.left, k.right) = (false, false),
            5 => (k.zoom_in, k.zoom_out) = (false, false),
            6 => (k.up, k.down) = (false, false),
            _ => {}
        }
    }

    /// The top view (1) or the side view (4) released over its button.
    fn console_view(&mut self, s: u32) {
        self.sounds.push(crate::audio::CLICK);
        // The views' hot-spots act on the release (their `Mouse_Szoveg_Set` flag): the button is set, then let go.
        self.star_buttons.press(s);
        let Some(v) = &self.view else { return };
        let c = v.camera;
        if s == 1 {
            if let (t, true) = c.top_view(&self.world.curr) {
                self.cam_tween = Some((t, 8));
            }
        } else {
            let known = self.world.curr.planets.iter().any(|p| p.known);
            let sv = crate::starmap::Camera::side_view(&self.world.curr, c.size);
            let t = if known { [sv.x, sv.y, sv.zoom] } else { [c.x, c.y, c.zoom] };
            self.cam_tween = Some(([t[0], t[1], t[2], 0.0, -0.95], 8));
        }
    }

    /// A frame of the main menu: its UI ticks due (10 ms each), the frame (`Mainscreen_Update3D`), the hover.
    fn front_frame(&mut self) {
        self.options_tick();
        let ov = self.frame_overlay();
        let now = self.now_ms();
        let (Some(fr), true) = (&mut self.front, self.gpu.is_some()) else { return };
        let mut due = 0;
        let mut outs = Vec::new();
        let mut acts = Vec::new();
        while now >= fr.ui_ms + 10 {
            fr.ui_ms += 10;
            fr.s.tick();
            acts.extend(fr.s.due());
            if let Some(mp) = &mut fr.mp {
                outs.extend(mp.tick(&mut fr.net));
            }
            due += 1;
        }
        for o in outs {
            self.front_mp_out(o);
        }
        for a in acts {
            self.front_act(a);
        }
        let (Some(fr), Some(g)) = (&mut self.front, &mut self.gpu) else { return };
        for _ in 0..due {
            self.audio.tick();
        }
        self.audio.frame(human_race(&self.world.game.state));
        fr.s.frame();
        // The main menu (`AktKep` 15) steps the row frame's globals too; the scenario list's frame carries them.
        let list = fr.s.scenario.as_mut().map(|sc| &mut sc.frame);
        carry_row_frame(&mut self.dk, &mut self.dk_owner, if list.is_some() { 6 } else { 0 }, list, due);
        fr.s.hover(fr.at);
        if let Some(st) = &mut fr.stats {
            st.hover(fr.at);
        }
        if let Some(rl) = &mut fr.replays {
            rl.hover(fr.at);
        }
        if let Some(st) = fr.s.take_started() {
            self.front_start(st);
            return;
        }
        let (bw, bh) = g.band();
        let size = [bw as f32, bh as f32];
        let scene = fr.scene.get_or_insert_with(|| crate::mainscr_scene::MainScene::load(&mut g.renderer, &self.install));
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        fr.s.race_boxes = scene.draw(&mut frame, &mut g.renderer, &self.install, &fr.s, &crate::font::Virtual::new(size));
        // Ours (ADR 0059): the panel stays out of reach under History and Replays.
        fr.s.covered = self.options.is_some() || fr.mods.is_some() || (fr.s.flow && (fr.stats.is_some() || fr.replays.is_some()));
        if let Some(h) = &self.hud {
            let now = self.start.elapsed().as_secs_f64() * 1000.0;
            draw_front_2d(fr, h, &mut g.renderer, &self.install, &mut frame, size, now, self.display.statistics);
        }
        if let (Some(o), Some(h)) = (&mut self.options, &self.hud) {
            draw_options(o, h, &mut frame, size, self.start.elapsed().as_secs_f64() * 1000.0);
        }
        // Ours (ADR 0062): the modern Options screen and the multiplayer pages (ADR 0064) are egui pages, over the 2D
        // and under the pointer.
        let mut top = None;
        let mut mp_outs = Vec::new();
        let mut mods_acts = Vec::new();
        let theme = |h: &crate::ui::Hud| page_theme(h, size);
        if let (Some(o), Some(h)) = (self.options.as_mut().filter(|o| o.s.modern.is_some()), &self.hud) {
            let audio = &mut self.audio;
            let theme = theme(h);
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| {
                // Load only: no save to hand back.
                let _ = o.s.modern_page(ctx, &h.texts, &theme, &mut |id| audio.play(id));
            });
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        } else if let (Some(mods), Some(h)) = (fr.mods.as_mut(), &self.hud) {
            // Ours (ADR 0068): the mod manager.
            let theme = theme(h);
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| mods_acts = mods.show(ctx, &theme));
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        } else if let (Some(mp), Some(h)) = (fr.mp.as_mut(), &self.hud) {
            let theme = theme(h);
            mp.load_heads(&g.gui.ctx, &self.install);
            let mut acts = Vec::new();
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| acts = mp.page(ctx, &h.texts, &h.palette, &theme, fr.s.flow));
            mp_outs = mp.apply(acts, &mut fr.net);
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        } else if let Some(h) = &self.hud {
            ov.draw(&mut frame, h, size);
        }
        let Ok(out) = g.surface.get_current_texture() else {
            self.reconfigure();
            return;
        };
        g.present_over(frame, top, &out.texture, now as f32 / 1000.0);
        g.window.pre_present_notify();
        out.present();
        for o in mp_outs {
            self.front_mp_out(o);
        }
        for a in mods_acts {
            self.mods_act(a);
        }
    }

    /// The mod manager's request (ADR 0068 §5).
    fn mods_act(&mut self, a: crate::mods_page::Act) {
        use crate::mods_page::Act;
        let Some(f) = &mut self.front else { return };
        match a {
            Act::Back => f.mods = None,
            Act::Restart => match crate::mods_page::restart() {
                Ok(()) => f.quit = true,
                Err(e) => eprintln!("ig2-client: restart: {e}"),
            },
            Act::Click => {}
        }
        self.audio.play(crate::audio::CLICK);
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
            // `OptionsCsakLoadLehet = 1; AktKep = 0xd`: Options in load-only mode over the main menu's scene.
            Act::LoadGame => self.open_options(true),
            // Ours (ADR 0031): the same screen's settings without the slot list, Load and Save, drawn and driven as the
            // modern main-menu screen (ADR 0053) with the multiplayer name.
            Act::Settings => {
                self.open_options(true);
                let name = self.front.as_ref().map(|f| f.s.player.clone()).unwrap_or_default();
                if let Some(o) = &mut self.options {
                    o.s.settings_only = true;
                    o.s.slots = vec![None; crate::options::SLOTS + 1];
                    o.s.modern = Some(crate::options::modern::Modern::new(&o.s, name, self.bindings.clone()));
                }
            }
            // `Mainscreen_Update3D` exe 0x50147d: `OpenBink("misc/credits.bik", 1, 0, 0, force)`, blocking.
            Act::Credits => self.queue_videos(["misc/credits.bik".to_string()], true, AfterVideo::Resume),
            Act::Click => self.audio.play(crate::audio::CLICK),
            Act::OpenScenarios | Act::Campaign | Act::QuickGame => {
                let Some(f) = &mut self.front else { return };
                let owned;
                let texts = match self.hud.as_ref() {
                    Some(h) => &h.texts,
                    None => {
                        owned = crate::ui::UiTexts::load(&self.install);
                        &owned
                    }
                };
                crate::mainscr_scene::prepare(&mut f.s, a, &mut f.scenarios, &self.install, texts);
            }
            // `MainScr_IPEntered`: `SaveGlobalRegistrySettings` (the name into the settings file).
            Act::NameEntered => {
                if let Some(f) = &self.front {
                    let name = String::from_utf8_lossy(&f.s.player).into_owned();
                    self.settings.set("MultiplayerNev", &name);
                    let mut own = ig2_formats::options::Options::own_path().and_then(|p| ig2_formats::userfs::read(p).ok()).map(|b| ig2_formats::options::Options::parse(&b)).unwrap_or_default();
                    own.set("MultiplayerNev", &name);
                    if let Some(p) = ig2_formats::options::Options::own_path() {
                        if let Err(e) = ig2_formats::userfs::write(&p, own.to_bytes()) {
                            eprintln!("ig2-client: {}: {e}", p.display());
                        }
                    }
                }
            }
            // `MainScr_Main` 0x23 / 0x22: the server path and the remaster's matchmaking; ours: our MP screens
            // (ADR 0014 §4; "inet" is the same screen without matchmaking).
            Act::MultiServer | Act::MultiMatchmaking => {
                let rules = self.install.read_raw("ig2.exe").map(|exe| ig2_net::session::rules_fingerprint(&exe)).unwrap_or_default();
                let mods = crate::mods_fingerprint(&self.install);
                let rules = ig2_net::session::modded_fingerprint(rules, mods);
                let Some(f) = &mut self.front else { return };
                let name = String::from_utf8_lossy(&f.s.player).into_owned();
                let story = ig2_formats::config::Config::read(self.install.root()).int("mpstory");
                f.mp = Some(crate::mp_lobby::MpScreen::open(a == Act::MultiMatchmaking, &name, rules).with_mods(mods).with_story(story));
            }
            Act::Tutorial(_) => {}
            // Ours (ADR 0059): the newest save that reads, as the Load screen's Load.
            Act::Continue => {
                let saves = crate::options::slot_dir().map(|d| crate::menu_flow::saves_newest_first(&d)).unwrap_or_default();
                let _ = saves.iter().any(|p| self.load_file(p));
            }
            Act::History => self.open_history(),
            Act::Replays => self.open_replays(),
            Act::Mods => {
                if let Some(f) = &mut self.front {
                    f.mods = Some(crate::mods_page::ModsPage::open(&self.install));
                }
                self.audio.play(crate::audio::CLICK);
            }
            // Ours (ADR 0050): `AddHighscore` (`LoadHighscores`, the entry slotted in, `SaveHighscores`) into our
            // settings file (`options.dat` stays untouched), and the list's tables reloaded.
            Act::HighScore => {
                let Some(h) = self.front.as_mut().and_then(|f| f.s.scored.take()) else { return };
                let defaults = crate::mainscr_scenario::default_scores(&self.install);
                let mut reg = ig2_formats::options::Options::load();
                let mut t = ig2_formats::highscore::Table::load(&reg, &h.registry, &defaults);
                t.add(&ig2_formats::text::decode(&h.entry.text, self.install.lang()), h.score);
                t.save(&mut reg, &h.registry);
                let pairs: Vec<(String, String)> = reg.pairs.iter().filter(|(k, _)| k.starts_with(&format!("{}_", h.registry))).cloned().collect();
                let pairs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                if let Err(e) = ig2_formats::options::Options::store_own(&pairs) {
                    eprintln!("ig2-client: high scores: {e}");
                }
                let Some(f) = &mut self.front else { return };
                let texts = crate::ui::UiTexts::load(&self.install);
                let scores = crate::mainscr_scenario::high_scores(&reg, &defaults, &h.registry, &texts);
                let lists = f.scenarios.iter_mut().map(|a| &mut a.entries).chain(f.s.scenario.iter_mut().map(|sc| &mut sc.entries));
                for e in lists.flatten().filter(|e| e.registry.eq_ignore_ascii_case(&h.registry)) {
                    e.scores = scores.clone();
                }
            }
        }
    }

    /// What our MP screens ask (ADR 0014): the click, leaving them, or the room's start: the game built here
    /// (`mp_lobby::launch`), then the menu's exit animation into the starmap.
    fn front_mp_out(&mut self, o: crate::mp_lobby::Out) {
        use crate::mp_lobby::Out;
        match o {
            Out::Click => self.audio.play(crate::audio::CLICK),
            Out::Copy(s) => {
                if let Err(e) = arboard::Clipboard::new().and_then(|mut c| c.set_text(s)) {
                    eprintln!("ig2-client: clipboard: {e}");
                }
                self.audio.play(crate::audio::CLICK);
            }
            Out::Close => {
                if let Some(f) = &mut self.front {
                    f.mp = None;
                    f.s.close_mp();
                }
            }
            Out::Start(b) => {
                let rules = self.rules;
                let ticks = crate::new_game::wall_ticks();
                let built = crate::new_game::StartTexts::read(&self.install).and_then(|t| t.with(&self.install, |data| crate::mp_lobby::launch(b, data, rules, ticks, Box::new(crate::mp_lobby::ClientNet::default())))).and_then(|r| r);
                let Some(f) = &mut self.front else { return };
                match built {
                    Ok(s) => {
                        f.session = Some(s);
                        f.mp = None;
                        f.s.start_mp();
                    }
                    Err(e) => {
                        eprintln!("ig2-client: multiplayer start: {e}");
                        if let Some(mp) = &mut f.mp {
                            mp.line = e.into_bytes();
                        }
                    }
                }
            }
        }
    }

    /// `MainScr_Kilep` step 3 (spec §2.2): the game the menu was left for, built from the install; the starmap
    /// follows (a campaign after its intro).
    fn front_start(&mut self, st: crate::mainscr::Start) {
        use crate::mainscr::Start;
        if let Start::Multiplayer = st {
            match self.front.as_mut().and_then(|f| f.session.take()) {
                Some(s) => self.start_world(World::new_mp(s, self.rules)),
                None => {
                    let scene = self.front.take().and_then(|f| f.scene);
                    self.front = Some(Front::new(scene, self.now_ms(), &self.settings));
                }
            }
            return;
        }
        let seed = crate::new_game::wall_ticks();
        let rules = self.rules;
        let game = match &st {
            Start::Campaign { race, level } => {
                // `Nehezseg` by the ring: easy 1, normal (index 2) 2, hard (index 1) 4.
                let difficulty = [1, 4, 2][(*level as usize).min(2)];
                // `InitStory` (exe 0x5c2940): the config's `storyline` and `storymap` (0 = not set).
                let config = ig2_formats::config::Config::read(self.install.root());
                let (storyline, storymap) = (config.int("storyline"), config.int("storymap"));
                let cfg = ig2_sim::game::new_game::NewCampaign { campaign: *race as i32 + 1, difficulty, seed, ticks: seed, storyline, storymap, last_storyline: self.settings.int(&format!("Fable_{}", *race as i32 + 1)) };
                crate::new_game::campaign(&self.install, &cfg, rules)
            }
            Start::Scenario { file, .. } => crate::new_game::scenario(&self.install, file, seed, rules),
            Start::Tutorial(n) => crate::new_game::tutorial(&self.install, *n, seed, rules),
            Start::Multiplayer => unreachable!("handled above"),
            Start::Skirmish { settings: k, kind } => {
                // Ours (ADR 0039): the quick game's next settings.
                self.settings.set(crate::mainscr_scenario::QUICK_GAME, &k.to_setting());
                if let Err(e) = ig2_formats::options::Options::store_own(&[(crate::mainscr_scenario::QUICK_GAME, &k.to_setting())]) {
                    eprintln!("ig2-client: settings: {e}");
                }
                match self.front.as_ref().and_then(|f| f.scenarios.as_ref()) {
                    Some(a) => crate::new_game::skirmish(&self.install, &k.config(*kind, &a.tables, seed), rules),
                    None => Err("no scenario list".into()),
                }
            }
        };
        match game {
            Ok(g) => {
                let mut w = World::new_game(g);
                // Ours (ADR 0045): the computer empires' ruleset of the new game, from the Options row.
                w.game.enhanced_ai = self.settings.int(crate::display::ENHANCED_AI) == Some(1);
                // Ours (ADR 0052): the campaign story's ruleset, likewise.
                w.game.story_fixes = self.settings.int(crate::display::STORY_FIXES) == Some(1);
                // `MainScr` 0x4fef1b: `StoreScenarioDescription` after `Betolt_Scenario`.
                w.scenario = matches!(st, Start::Scenario { .. });
                w.ui.scenario = w.scenario;
                // `MainScr_Kilep` 0x4fef45..63: a scenario's start time and registry name (saved; only ADR 0050's
                // high-score entry reads them).
                if let Start::Scenario { registry, .. } = &st {
                    let m = &mut w.game.state.mem;
                    m.set_i32(SCENARIO_START, m.i32(ig2_sim::save::va::IDO));
                    let name = registry.as_bytes();
                    for i in 0..0x1e {
                        m.set_u8(SCENARIO_REGISTRY + i as u32, if i < 0x1d { name.get(i).copied().unwrap_or(0) } else { 0 });
                    }
                }
                let campaign = matches!(st, Start::Campaign { .. });
                if campaign {
                    w.campaign_intro();
                }
                let voice = w.start_voice.take();
                self.start_world(w);
                if let Start::Tutorial(n) = st {
                    self.start_tutorial(n as i32);
                }
                if campaign {
                    // The intro first; then `Starmap_Belep`'s music and the start voice.
                    self.take_video_requests();
                    self.after_video = AfterVideo::StarmapEntry(voice.unwrap_or(0));
                } else if let Some(c) = voice {
                    self.audio.play_start_voice(c);
                }
            }
            Err(e) => {
                eprintln!("ig2-client: {st:?}: {e}");
                let scene = self.front.take().and_then(|f| f.scene);
                self.front = Some(Front::new(scene, self.now_ms(), &self.settings));
            }
        }
    }

    /// `Options_Belep` (re/screens/options.md §3): the settings as the audio and the settings file hold them, the
    /// slots read; in a game the clock pauses (`SetIdoPause(1, 0)`) until it is left.
    /// `AktKepKell = 0xd`: from the messages screen `Messages_Kilep`'s closing runs first; from the battle its zoom-out
    /// (the tutorial's O too); else Options at once.
    fn ask_options(&mut self) {
        if self.spacewar.is_some() {
            self.leave_battle(0xd);
        } else if self.overlay == Some(crate::ui::Screen::Messages) {
            self.request_screen(0xd);
        } else {
            self.open_options(false);
        }
    }

    fn open_options(&mut self, load_only: bool) {
        if self.options.is_some() {
            return;
        }
        let Some(h) = &self.hud else { return };
        let s = crate::options::Settings { music: self.audio.music_volume, sound: self.audio.sound_volume, ..crate::options::start_settings(&self.settings) };
        let slots = crate::options::read_slots(crate::options::slot_dir().as_deref());
        let mp = self.multiplayer();
        let mut screen = crate::options::OptionsScreen::enter(&h.texts, self.opt_rows.clone(), &s, load_only, mp, slots);
        screen.lang_at_start = Some(self.lang_at_start);
        if self.lang_forced {
            if let Some(r) = screen.rows.iter_mut().find(|r| r.ours && r.key == crate::display::LANGUAGE) {
                r.locked = true;
            }
            crate::options::check_dependencies(&mut screen.rows, mp);
        }
        screen.tutorial = self.tutorial.as_ref().is_some_and(|t| t.t.active || t.t.paused);
        if screen.tutorial {
            screen.lock_battle_simulate();
        }
        screen.mp_save = mp && self.world.mp_host();
        // `OptionsHarcFolyik`: `SpaceWar_Kilep` sets it towards Options (exe 0x562c0f), `Colony_Kilep` when the planet's
        // ground war runs (exe 0x466631, `GetWarfareStatus` inlined); `Options_Belep` keeps it only from 3 or 0xb.
        let planet = self.world.colony_curr.as_ref().map(|c| c.planet).filter(|_| self.colony.is_some() && self.overlay.is_none());
        let war = planet.is_some_and(|p| self.world.game.warfare_status(p) == 1);
        screen.from_battle = !load_only && (self.spacewar.is_some() || self.spacewar_parked.is_some());
        screen.battle = !load_only && (screen.from_battle || war);
        screen.debug = self.world.game.state.mem.u8(ig2_sim::save::va::DEBUGX) != 0;
        // Ours (ADR 0067): the in-game menu (and the main menu's Load game) as the modern page.
        if self.display.modern_menu {
            let name = self.settings.get("MultiplayerNev").filter(|n| !n.is_empty()).map_or_else(|| b"Player".to_vec(), |n| n.as_bytes().to_vec());
            screen.modern = Some(crate::options::modern::Modern::in_game(&screen, name, self.bindings.clone()));
        }
        let glow = self.gpu.as_mut().and_then(|g| {
            self.install.resolve_texture("textures/starmap/", "laservago.bmp").and_then(|p| crate::assets::Image::load(&self.install, &p).ok()).map(|img| g.renderer.add_texture(&img))
        });
        // `SetIdoPause` (exe 0x541300) changes nothing in MP: neither the pause here nor `Options_Kilep`'s restore.
        let was_paused = self.world.paused() || mp;
        if !load_only && !was_paused {
            self.send(StarmapOrder::Pause { paused: true });
        }
        // `Options_Belep`: from the messages screen `OptionsElottiKep = MessagesElottiScreen` (Messages_Kilep runs: back
        // goes to the screen under it, re/screens/options.md §3).
        if self.overlay == Some(crate::ui::Screen::Messages) {
            self.overlay = None;
        }
        self.options = Some(OptionsUp { s: screen, ui_ms: self.now_ms(), glow, was_paused });
    }

    /// The Options screen's UI ticks due and its live settings (`Gfx2D_2` sets the volumes every frame), then what
    /// its fade-out ends in.
    fn options_tick(&mut self) {
        let now = self.now_ms();
        let Some(o) = &mut self.options else { return };
        while now >= o.ui_ms + 10 {
            o.ui_ms += 10;
            o.s.tick();
        }
        let s = o.s.settings();
        (self.audio.music_volume, self.audio.sound_volume) = (s.music, s.sound);
        // ADR 0058: the voice volume heard as it is set, too.
        if let Some(r) = o.s.rows.iter().find(|r| r.ours && r.key == crate::display::VOICE_VOLUME) {
            self.audio.voice_volume = (r.value > 0).then(|| (r.value - 1) * 10);
        }
        if std::mem::take(&mut o.s.dirty) {
            let snap = o.s.clone();
            self.store_settings_from(&snap);
        }
        let act = self.options.as_mut().and_then(|o| o.s.done());
        if let Some(a) = act {
            self.options_act(a);
        }
    }

    /// A key while Options is up: the name entry's keys (`DoKeyboard` with `IOMode`), else Esc or O back
    /// (`ESC_Pressed`).
    fn options_key(&mut self, k: &Key) {
        use crate::options::EntryKey;
        let changing = self.screen_changing();
        let Some(o) = &mut self.options else { return };
        if o.s.modern.is_some() {
            use crate::options::modern::Nav;
            // ADR 0067: the save name's Home, End and Delete, as on the original's entry.
            let edit = match k.as_ref() {
                Key::Named(NamedKey::Home) => Some(EntryKey::Home),
                Key::Named(NamedKey::End) => Some(EntryKey::End),
                Key::Named(NamedKey::Delete) => Some(EntryKey::Delete),
                _ => None,
            };
            if let Some(e) = edit.filter(|_| o.s.entry.is_some()) {
                o.s.entry_key(e);
                return;
            }
            let typing = o.s.typing();
            let shift = self.modifiers.shift_key();
            let nav = match k.as_ref() {
                Key::Named(NamedKey::Escape) => Some(Nav::Back),
                Key::Named(NamedKey::ArrowUp) => Some(Nav::Up),
                Key::Named(NamedKey::ArrowDown) => Some(Nav::Down),
                Key::Named(NamedKey::ArrowLeft) => Some(Nav::Left),
                Key::Named(NamedKey::ArrowRight) => Some(Nav::Right),
                Key::Named(NamedKey::Tab) => Some(if shift { Nav::PrevTab } else { Nav::NextTab }),
                Key::Named(NamedKey::PageDown) => Some(Nav::NextTab),
                Key::Named(NamedKey::PageUp) => Some(Nav::PrevTab),
                Key::Named(NamedKey::Enter) => Some(Nav::Activate),
                Key::Named(NamedKey::Space) if !typing => Some(Nav::Activate),
                Key::Named(NamedKey::Space) => Some(Nav::Char(b' ')),
                Key::Named(NamedKey::Backspace) => Some(Nav::Erase),
                Key::Character(c) if typing && !self.modifiers.control_key() && !self.modifiers.alt_key() => self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c)).first().map(|&b| Nav::Char(b)),
                Key::Character("o" | "O") => Some(Nav::Back),
                _ => None,
            };
            let audio = &mut self.audio;
            let act = match (nav, &mut self.options) {
                (Some(n), Some(o)) => o.s.modern_key(n, &mut |id| audio.play(id)),
                _ => None,
            };
            // ADR 0067: the save name's Enter.
            if let Some(crate::options::Act::Save(slot, name)) = act {
                self.options_save(slot, name);
            }
            return;
        }
        let act = if o.s.entry.is_some() {
            let key = match k.as_ref() {
                // `DoKeyboard` case 0x1b (exe 0x4db1a0): nothing while a screen change is pending.
                Key::Named(NamedKey::Escape) if changing => None,
                Key::Named(NamedKey::Escape) => {
                    o.s.escape();
                    None
                }
                Key::Named(NamedKey::Enter) => Some(EntryKey::Enter),
                Key::Named(NamedKey::Backspace) => Some(EntryKey::Back),
                Key::Named(NamedKey::Delete) => Some(EntryKey::Delete),
                Key::Named(NamedKey::Home) => Some(EntryKey::Home),
                Key::Named(NamedKey::End) => Some(EntryKey::End),
                Key::Named(NamedKey::ArrowLeft) => Some(EntryKey::Left),
                Key::Named(NamedKey::ArrowRight) => Some(EntryKey::Right),
                Key::Named(NamedKey::Space) => Some(EntryKey::Char(b' ')),
                Key::Character(c) => {
                    // The text in the font's code page (`UiTexts::encode`), one byte a character.
                    let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                    b.first().copied().filter(|&b| b >= 0x20).map(EntryKey::Char)
                }
                _ => None,
            };
            key.and_then(|key| o.s.entry_key(key))
        } else {
            if matches!(k.as_ref(), Key::Named(NamedKey::Escape) | Key::Character("o" | "O")) {
                o.s.escape();
            }
            None
        };
        if let Some(crate::options::Act::Save(slot, name)) = act {
            self.options_save(slot, name);
        }
    }

    /// F1: `Help_Belep` over the screen that is up (`HelpElottiScreen`), opening animated; the game time stands until
    /// it is gone (`IdoPauseMertAtlepesAlattVan`). `DoKeyboard`'s F-key gate (exe 0x4db1a0): not on the main menu,
    /// the space battle or a colony at war without `DEBUGX`, Options only when it isn't load-only or in a battle
    /// (`OptionsCsakLoadLehet`, `OptionsHarcFolyik`), Messages only with no answers left and the text typed out, and
    /// not while the tutorial turns the screen off (`KepernyokLetiltva[1]`). From Options and Messages the screen
    /// goes (their `_Kilep`) and Help returns to the one before it (`OptionsElottiKep`, `MessagesElottiScreen`).
    fn open_help(&mut self) {
        let debug = self.world.game.state.mem.u8(ig2_sim::save::va::DEBUGX) != 0;
        let war = self.colony.is_some() && self.overlay.is_none() && self.world.colony_curr.as_ref().is_some_and(|c| c.war_flag);
        let messages_busy = self.overlay == Some(crate::ui::Screen::Messages)
            && self.hud.as_ref().is_some_and(|h| h.messages.open.as_ref().is_some_and(|d| !d.answers.is_empty() || !d.typed_all));
        let options_locked = self.options.as_ref().is_some_and(|o| o.s.load_only || (o.s.battle && !debug));
        let off = self.tutorial.as_ref().is_some_and(|t| t.t.gates.screens[1]) && !debug;
        if self.help.is_some() || self.front.is_some() || ((self.spacewar.is_some() || war) && !debug) || options_locked || messages_busy || off {
            return;
        }
        // Over a parked battle (`DEBUGX`): `Options_Kilep` without re-entering it; `Help_Belep` takes
        // `OptionsElottiKep` (0xb) as `HelpElottiScreen`, so Help's Back goes to the starmap and on to the battle.
        if self.options.is_some() && self.spacewar_parked.is_some() {
            self.leave_options();
        } else if self.options.is_some() {
            self.options_act(crate::options::Act::Back);
        }
        if self.overlay == Some(crate::ui::Screen::Messages) {
            self.overlay = None;
        }
        let Some(font) = self.hud.as_ref().and_then(|h| h.font.as_ref()) else { return };
        let (index, read) = crate::help::install_files(&self.install);
        let s = crate::help::HelpScreen::enter(&index, read, &self.help_memory, true, crate::help::char_y(font));
        let glow = self.gpu.as_mut().and_then(|g| {
            self.install.resolve_texture("textures/starmap/", "laservago.bmp").and_then(|p| crate::assets::Image::load(&self.install, &p).ok()).map(|img| g.renderer.add_texture(&img))
        });
        self.help = Some(HelpUp { s, ui_ms: self.now_ms(), glow, pics: Default::default() });
    }

    /// Help goes at once (its screen changes under a key): the help state is kept, the game clock restarts.
    fn leave_help(&mut self) {
        // Gone some other way than its closing: no Options after it.
        self.help_then_options = false;
        if let Some(h) = self.help.take() {
            self.help_memory = h.s.memory();
            self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
        }
    }

    /// The F-key asks for the screen Help is over (`HelpElottiScreen`): leaving Help is all it does.
    fn help_under_is(&self, k: &Key<&str>) -> bool {
        use crate::ui::Screen;
        match k {
            Key::Named(NamedKey::F2) => self.overlay.is_none() && self.colony.is_none() && self.spy.is_none(),
            Key::Named(NamedKey::F3) => self.overlay.is_none() && self.colony.is_some(),
            Key::Named(NamedKey::F4) => self.overlay == Some(Screen::Research),
            Key::Named(NamedKey::F5) => self.overlay == Some(Screen::Design),
            Key::Named(NamedKey::F6) => self.overlay == Some(Screen::Diplomacy),
            Key::Named(NamedKey::F7) => self.spy.is_some(),
            Key::Named(NamedKey::F8) => self.overlay == Some(Screen::Trade),
            Key::Named(NamedKey::F9) => self.overlay == Some(Screen::Info),
            Key::Named(NamedKey::F10) => self.overlay == Some(Screen::Messages),
            _ => false,
        }
    }

    /// A game frame while Help is up: its UI ticks (no game ticks), black behind it (`Copy3hatterkep(0x35)`), the
    /// screen and the top bar; once it has closed, back to the screen before with the game clock restarted.
    fn help_game_frame(&mut self) {
        let ov = self.frame_overlay();
        let now = self.now_ms();
        let due_ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50);
        self.ui_frames = now as u64 / 10;
        for _ in 0..due_ui {
            self.audio.tick();
        }
        // The top bar's speed sound (`top_bar`) over this screen.
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        self.audio.frame(human_race(&self.world.game.state));
        self.backdrop_switch();
        let backdrop = self.backdrop();
        let Some(h) = &mut self.help else { return };
        while now >= h.ui_ms + 10 {
            h.ui_ms += 10;
            h.s.tick();
        }
        let (Some(g), Some(hud)) = (&mut self.gpu, &mut self.hud) else { return };
        h.pics.load(&mut g.renderer, &self.install, &h.s.text.pics);
        let (bw, bh) = g.band();
        let size = [bw as f32, bh as f32];
        let t = now as f32 / 1000.0;
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        frame.backdrop = backdrop;
        if let Some(font) = &hud.font {
            let d = crate::help::Draw { font, white: hud.white, glow: h.glow.unwrap_or(hud.white), v: crate::font::Virtual::new(size), palette: &hud.palette, texts: &hud.texts, pics: &h.pics, now_ms: self.start.elapsed().as_secs_f64() * 1000.0 };
            h.s.draw(&mut frame, &d, crate::help::char_y(font));
        }
        hud.paused = self.world.paused();
        hud.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Help, t);
        ov.draw(&mut frame, hud, size);
        let done = h.s.done().is_some();
        if let Ok(out) = g.surface.get_current_texture() {
            g.present(frame, &out.texture, t);
            g.window.pre_present_notify();
            out.present();
        } else {
            self.reconfigure();
        }
        if done {
            if let Some(h) = self.help.take() {
                self.help_memory = h.s.memory();
            }
            self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
            if std::mem::take(&mut self.help_then_options) {
                self.open_options(false);
            }
        }
    }

    /// A game frame while Options is up: its ticks, the sound clock, black behind it (`Copy3hatterkep(0x35)`'s
    /// picture is black in the captures), the screen and the top bar.
    fn options_game_frame(&mut self) {
        self.options_tick();
        if self.options.is_none() || self.front.is_some() {
            return;
        }
        let ov = self.frame_overlay();
        let now = self.now_ms();
        let due_ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50);
        self.ui_frames = now as u64 / 10;
        for _ in 0..due_ui {
            self.audio.tick();
        }
        // The top bar's speed sound (`top_bar`) over this screen.
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        self.audio.frame(human_race(&self.world.game.state));
        self.backdrop_switch();
        let backdrop = self.backdrop();
        let Some(g) = &mut self.gpu else { return };
        let (bw, bh) = g.band();
        let size = [bw as f32, bh as f32];
        let t = now as f32 / 1000.0;
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        frame.backdrop = backdrop;
        let (mut top, mut act) = (None, None);
        if let (Some(o), Some(h)) = (&mut self.options, &mut self.hud) {
            draw_options(o, h, &mut frame, size, self.start.elapsed().as_secs_f64() * 1000.0);
            if o.s.modern.is_some() {
                // ADR 0067: the egui page over the black, the pointer over it (the top bar is under the page).
                let theme = page_theme(h, size);
                let audio = &mut self.audio;
                g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| act = o.s.modern_page(ctx, &h.texts, &theme, &mut |id| audio.play(id)));
                let mut f = crate::render::Frame::new([0.0; 4]);
                ov.draw(&mut f, h, size);
                top = Some(f);
            } else {
                h.paused = true;
                h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Options, t);
                ov.draw(&mut frame, h, size);
            }
        }
        let Ok(out) = g.surface.get_current_texture() else {
            self.reconfigure();
            return;
        };
        g.present_over(frame, top, &out.texture, t);
        g.window.pre_present_notify();
        out.present();
        if let Some(crate::options::Act::Save(slot, name)) = act {
            self.options_save(slot, name);
        }
    }

    /// `ExitToMainScreen` (exe 0x5117f0), then `AktKepKell = 0xF`: `MainScr_Belep` (the rings fly in again, the menu
    /// music). The world stays as it is until the next game replaces it.
    /// `AktKep` as the tutorial reads it: the help screen 1, the spy screen 7 (up over the screen it came from), else
    /// the overlay's or the base screen's.
    fn tutorial_kep(&self) -> i32 {
        if self.spy.is_some() && self.help.is_none() && self.spacewar.is_none() {
            return 7;
        }
        akt_kep(if self.help.is_some() { Some(crate::ui::Screen::Help) } else { self.overlay }, self.colony.is_some(), self.spacewar.is_some())
    }

    fn tutorial_owns_input(&self) -> bool {
        self.options.is_none() && self.tutorial.as_ref().is_some_and(|t| t.owns_input(self.tutorial_kep()))
    }

    fn tutorial_inputs(&self) -> crate::tutorial_ui::Inputs {
        let counter = self.tutorial.as_ref().map_or(0, |t| t.counter);
        let screen = self.tutorial_kep();
        // `Atlepes || Belep` (`WAITFORATLEPES`): the whole change runs inside one `IG2_gameUpdate` (exe 0x4e0440), so the
        // ticks that see them are the old screen's closing loop (`*_Kilep`: research, design, trade, spy, diplomacy,
        // messages; 84 ticks in the chapter 3/5 traces) and the tick the screen changes.
        let transition = self.tutorial.as_ref().is_some_and(|t| t.screen() != screen) || self.screen_changing();
        crate::tutorial_ui::Inputs { screen, counter, transition, ..Default::default() }.counts(&self.world.game.state)
    }

    /// `TutorialSetCampaignGameOptions` has made the game; `TutorialStart("main", n)` (exe 0x611f80).
    fn start_tutorial(&mut self, chapter: i32) {
        let inputs = self.tutorial_inputs();
        match crate::tutorial_ui::TutorialUi::start(&self.install, chapter, inputs, Some(&self.world.game.state)) {
            Ok((mut t, effects)) => {
                t.ai_off = Some(self.world.game.state.mem.i32(ig2_sim::save::va::AI_OFF));
                t.push(&mut self.world.game.state.mem);
                self.tutorial = Some(t);
                self.world.drop_replay();
                (self.tutorial_at, self.tutorial_buttons) = (crate::tutorial_ui::PARKED, (false, false));
                self.tutorial_effects(effects);
                // `GlobalMusicVolumeCsokkento` (after `TutorialStart`'s own `TutorialStop(true)`): the music ducked while
                // the tutorial runs. `ExitToMainScreen` clears Active/Paused without `TutorialStop`, so the duck stays.
                self.audio.set_duck(crate::audio::tutorial_duck(self.audio.music_volume));
            }
            Err(e) => eprintln!("ig2-client: tutorial {chapter}: {e}"),
        }
    }

    /// One UI tick of the tutorial: the recorded input (`Mouse_Tutorial_Kezel`, `ReadNext`) goes to the screens, an
    /// event the game raised resumes the script, then `TutorialFrissit` and its effects.
    fn tutorial_tick(&mut self) {
        let kep = self.tutorial_kep();
        let replay = if self.options.is_some() { None } else { self.tutorial.as_mut().and_then(|t| t.replay_step(kep)) };
        match replay {
            Some(r) => self.tutorial_replay(r),
            // The live input takes over (`Mouse_Lekerdez` reads the live buttons again): a recorded button held is let go.
            None => {
                let (left, right) = std::mem::take(&mut self.tutorial_buttons);
                if left {
                    self.mouse(MouseButton::Left, false);
                }
                if right {
                    self.mouse(MouseButton::Right, false);
                }
            }
        }
        let inputs = self.tutorial_inputs();
        let now = self.now_ms();
        let routed = std::mem::take(&mut self.world.ground_war_routed);
        let Some(t) = &mut self.tutorial else { return };
        // `StartGrWar` (exe 0x4a4a70) clears `TutorialWaitForBattle` first thing; single player's real-time ground
        // war has no `StartGrWar` of its own here, so the routing's real-time branch stands for it.
        if routed {
            t.t.battle_started();
        }
        t.entered(kep, &mut self.world.game);
        t.pull(&self.world.game.state.mem);
        t.ms = now;
        let effects = t.tick(&self.install, inputs, Some(&self.world.game.state));
        t.push(&mut self.world.game.state.mem);
        let g = &t.t.gates;
        self.panels.locks = crate::starmap_panels::TutorialLocks { production: g.production, exit: g.exit_button, split: g.split };
        if let Some(c) = &mut self.colony {
            c.ui.locked = g.buildings.to_vec();
            (c.ui.no_demolish, c.ui.all_off, c.ui.starmap_off) = (g.demolish, g.colony, g.screens[2]);
            c.ui.war.locked = g.colony;
        }
        self.tutorial_effects(effects);
    }

    /// The recording's pointer, buttons and keys as live input.
    fn tutorial_replay(&mut self, r: crate::tutorial_ui::Replay) {
        self.tutorial_kind = if r.parked { -2 } else { 0 };
        if r.at != self.tutorial_at {
            self.tutorial_at = r.at;
            let (bw, bh) = self.gpu.as_ref().map_or((640, 480), |g| g.band());
            let p = crate::font::Virtual::new([bw as f32, bh as f32]).px(r.at[0] as f32, r.at[1] as f32);
            self.cursor_moved(p);
        }
        // `KeyAlt`, `KeyShift`, `KeyCtrl` from the recording (1, 2, 4).
        let mut m = winit::keyboard::ModifiersState::empty();
        m.set(winit::keyboard::ModifiersState::ALT, r.modifiers & 1 != 0);
        m.set(winit::keyboard::ModifiersState::SHIFT, r.modifiers & 2 != 0);
        m.set(winit::keyboard::ModifiersState::CONTROL, r.modifiers & 4 != 0);
        self.modifiers = m;
        let (left, right) = self.tutorial_buttons;
        self.tutorial_buttons = (r.left, r.right);
        if r.left != left {
            self.mouse(MouseButton::Left, r.left);
        }
        if r.right != right {
            self.mouse(MouseButton::Right, r.right);
        }
        for (kind, code) in r.keys {
            // On the battle screen the recorded keys go to its handler as live ones do (`SpaceWarKeyboardHandler`
            // behind `DoKeyboard`): Space by its code, the top row's digits and the numpad's + / − by their keys.
            if self.spacewar.is_some() {
                use winit::keyboard::{KeyCode, NativeKeyCode, PhysicalKey};
                let physical = match (kind, code) {
                    (0, d @ 0x31..=0x39) => PhysicalKey::Code([KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9][(d - 0x31) as usize]),
                    (1 | 2, 0x6b) => PhysicalKey::Code(KeyCode::NumpadAdd),
                    (1 | 2, 0x6d) => PhysicalKey::Code(KeyCode::NumpadSubtract),
                    _ => PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
                };
                let logical = match (kind, code) {
                    (0, 0x20) => Some(Key::Named(NamedKey::Space)),
                    (1 | 2, 0x6b) => Some(Key::Character("+".into())),
                    (1 | 2, 0x6d) => Some(Key::Character("-".into())),
                    (2, c) => recorded_key(1, c),
                    _ => recorded_key(kind, code),
                };
                if let Some(k) = logical {
                    self.spacewar_key_by(k.as_ref(), physical, kind != 2);
                }
                continue;
            }
            // The held keys (arrows, numpad + / −) go down and up through the screens' held-key handlers, as live
            // ones do; the other key-ups have no reader.
            if matches!(kind, 1 | 2) {
                use winit::keyboard::{KeyCode, PhysicalKey};
                let held = match code {
                    0x25 => Some((KeyCode::ArrowLeft, Key::Named(NamedKey::ArrowLeft))),
                    0x26 => Some((KeyCode::ArrowUp, Key::Named(NamedKey::ArrowUp))),
                    0x27 => Some((KeyCode::ArrowRight, Key::Named(NamedKey::ArrowRight))),
                    0x28 => Some((KeyCode::ArrowDown, Key::Named(NamedKey::ArrowDown))),
                    0x6b => Some((KeyCode::NumpadAdd, Key::Character("+".into()))),
                    0x6d => Some((KeyCode::NumpadSubtract, Key::Character("-".into()))),
                    _ => None,
                };
                if let Some((pk, logical)) = held {
                    let (pk, down) = (PhysicalKey::Code(pk), kind == 1);
                    if self.colony_camera_key_by(pk, logical.as_ref(), down) || self.trade_key_by(pk, logical.as_ref(), down) || self.starmap_key_by(pk, logical.as_ref(), down) {
                        continue;
                    }
                }
            }
            if let Some(k) = recorded_key(kind, code) {
                self.key(k.as_ref());
            }
            // `oldEnterlenyomva` (`research_key_seen`): a key up, or a down the research screen has no key for.
            if let Some(r) = self.research.as_mut().filter(|_| self.overlay == Some(crate::ui::Screen::Research)) {
                if kind == 2 || !matches!(recorded_key(kind, code), Some(Key::Named(n)) if research_key(n).is_some()) {
                    r.key_seen(kind != 2);
                }
            }
        }
    }

    /// What the script does outside the interpreter (re/rules/tutorial.md §3): the game's in-place writes, the
    /// starmap's selection, the clock, leaving.
    fn tutorial_effects(&mut self, effects: Vec<ig2_sim::tutorial::Effect>) {
        use ig2_sim::tutorial::Effect as E;
        // `IdoPause` as the tick's commands leave it: the pause orders reach the game at the next pump, so a
        // `GAMESPEED 1` then `PAUSE` in one tick (chapter 4's battle) must see the first one's unpause.
        let mut paused = self.world.paused();
        for e in effects {
            if self.world.game.tutorial_effect(&e) {
                // The new fleet of EN is selected and centred (`AktFleet`, `StarmapMode = 2`, `KellSelect`).
                let en = self.world.game.state.mem.i32(ig2_sim::save::va::EN);
                match e {
                    E::KillerFleet(x) if x == en => self.tutorial_centre(&E::FindFleet(b"Terminator".to_vec())),
                    E::ColonizationFleet => self.tutorial_centre(&E::FindFleet(b"Colonization".to_vec())),
                    _ => {}
                }
                continue;
            }
            match e {
                E::ReturnToMainMenu => self.exit_to_main_menu(),
                E::Quit => {
                    self.exit_to_main_menu();
                    if let Some(f) = &mut self.front {
                        f.quit = true;
                    }
                }
                E::End => self.world.local_lines.push(b"Tutorial END.".to_vec()),
                // `DISABLE OVERRUN` (re/rules/tutorial.md §3 12): the four battle debug flags saved (a second one saves the
                // changed values, as in the exe), then 0, 0, 1, 1; `TutorialStop` puts them back.
                E::DisableOverrun => {
                    let m = &mut self.world.game.state.mem;
                    let saved = std::array::from_fn(|i| m.i32(0xB77230 + 4 * i as u32));
                    for (i, v) in [0, 0, 1, 1].into_iter().enumerate() {
                        m.set_i32(0xB77230 + 4 * i as u32, v);
                    }
                    if let Some(t) = &mut self.tutorial {
                        t.overrun = Some(saved);
                    }
                }
                E::Stopped { restore_overrun, restore_music } => {
                    if restore_music {
                        self.audio.set_duck(0);
                    }
                    if let Some(v) = self.tutorial.as_mut().and_then(|t| t.overrun.take()).filter(|_| restore_overrun) {
                        for (i, x) in v.into_iter().enumerate() {
                            self.world.game.state.mem.set_i32(0xB77230 + 4 * i as u32, x);
                        }
                    }
                }
                // `DOUBLEGOMB ON`: `ResearchDoubleGomb()` from any screen (the click and message 0x2b; the button's look
                // only shows on the research screen, which reads `Double_Lenyomva` as it opens).
                E::DoubleButton => match &mut self.research {
                    Some(r) => {
                        for a in r.double() {
                            let crate::research::Act::Send(c) = a;
                            self.world.send(c);
                        }
                    }
                    None => {
                        self.audio.play(crate::audio::CLICK);
                        self.world.send(Command::Research { order: ig2_net::ResearchCmd::Urgent });
                    }
                },
                // `KURVAANYAD`: the colony's planet info (chapter 2 runs it on the colony screen).
                E::PlanetInfo => self.colony_input(|ui, c, _, _| ui.planet_info(c)),
                E::CenterFleet | E::FindFleet(_) | E::FindPlanet(_) | E::CenterPlanet | E::CenterUncolonisedPlanet => self.tutorial_centre(&e),
                E::GameSpeed(0) | E::Pause(true) => {
                    if !paused {
                        self.send(StarmapOrder::Pause { paused: true });
                        paused = true;
                    }
                }
                // `SetIdoSpeed(n, false)` (skipped while `nemlehetidotallitani`, no sound), then `SetIdoPause(false)`.
                E::GameSpeed(n) => {
                    if !self.speed_locked() && self.world.game.speed_level() != n {
                        self.send(StarmapOrder::Speed { level: n });
                    }
                    if paused {
                        self.send(StarmapOrder::Pause { paused: false });
                        paused = false;
                    }
                }
                E::Pause(false) => {
                    if paused {
                        self.send(StarmapOrder::Pause { paused: false });
                        paused = false;
                    }
                }
                // `GOTO SCREEN n` (`AktKepKell = n`): as the F-key of that screen (F1 = 1 .. F11 = 11) when another is up.
                E::GotoScreen(n) if (1..=11).contains(&n) => {
                    let now = self.tutorial_kep();
                    const F: [NamedKey; 11] = [
                        NamedKey::F1, NamedKey::F2, NamedKey::F3, NamedKey::F4, NamedKey::F5, NamedKey::F6, NamedKey::F7, NamedKey::F8,
                        NamedKey::F9, NamedKey::F10, NamedKey::F11,
                    ];
                    if now != n {
                        self.key(Key::Named(F[n as usize - 1]));
                    }
                }
                e => eprintln!("ig2-client: tutorial: {e:?} isn't carried out yet"),
            }
        }
    }

    /// `CENTERFLEET`, `FIND FLEET`, `FIND PLANET`, `CENTERPLANET`, `CENTERUNCOLONISEDPLANET`: select the first match
    /// and centre it (`StarmapKozepreAllit`).
    fn tutorial_centre(&mut self, e: &ig2_sim::tutorial::Effect) {
        use ig2_sim::tutorial::Effect as E;
        let state = &self.world.game.state;
        let en = state.mem.i32(ig2_sim::save::va::EN);
        let name = |b: &[u8]| b.split(|&c| c == 0).next().unwrap_or_default().to_vec();
        let f32_at = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let fleet = |want: Option<&[u8]>| {
            state.fleets.iter().enumerate().skip(1).find(|(_, f)| f[0x15] as i32 == en && want.is_none_or(|w| name(&f[..0x15]) == w)).map(|(i, f)| (i, f32_at(f, 0x30), f32_at(f, 0x34)))
        };
        let planet = |test: &dyn Fn(&[u8]) -> bool| {
            (1..=state.planet_count()).find_map(|p| {
                let r = state.mem.read(ig2_sim::save::va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, ig2_sim::save::PLANET_SIZE);
                test(&r).then(|| (p, f32_at(&r, 0x18), f32_at(&r, 0x1c)))
            })
        };
        let k = 0xd8 + 2 * en.clamp(0, 18) as usize;
        let explored = |r: &[u8]| i16::from_le_bytes([r[k], r[k + 1]]) == 1;
        let cur = self.selection();
        let hit = match e {
            E::CenterFleet => fleet(None).map(|(i, x, y)| (Selection { mode: 2, fleet: i, ..cur }, x, y)),
            E::FindFleet(n) => fleet(Some(n)).map(|(i, x, y)| (Selection { mode: 2, fleet: i, ..cur }, x, y)),
            E::FindPlanet(n) => planet(&|r| r[0x20] as i32 == en && name(&r[..0x18]) == *n).map(|(p, x, y)| (Selection { mode: 1, planet: p, ..cur }, x, y)),
            E::CenterPlanet => planet(&|r| r[0x20] as i32 == en).map(|(p, x, y)| (Selection { mode: 1, planet: p, ..cur }, x, y)),
            E::CenterUncolonisedPlanet => planet(&|r| r[0x20] == 0 && explored(r)).map(|(p, x, y)| (Selection { mode: 1, planet: p, ..cur }, x, y)),
            _ => None,
        };
        if let Some((s, x, y)) = hit {
            self.apply(vec![Action::Select(s), Action::Centre { x, y }]);
        }
    }

    /// Ours (ADR 0050, re/rules/game-end.md §6): a single-player scenario this machine won with the high-score entry
    /// on: its registry name and score, `Ido − scenario_kezdesi_ido` (the exe has no formula: `scenario_score` is never
    /// written; its dialog shows `score / 50` days, so the score is the days to win in `Ido` units).
    fn won_scenario(&self) -> Option<(String, i32)> {
        if !self.display.high_scores || self.world.net.is_some() || self.world.game.over != Some(ig2_sim::game::GameOver::Won) {
            return None;
        }
        let m = &self.world.game.state.mem;
        let name: Vec<u8> = (0..0x1e).map(|i| m.u8(SCENARIO_REGISTRY + i)).take_while(|&b| b != 0).collect();
        let score = m.i32(ig2_sim::save::va::IDO) - m.i32(SCENARIO_START);
        (!name.is_empty() && score > 0).then(|| (String::from_utf8_lossy(&name).into_owned(), score))
    }

    fn exit_to_main_menu(&mut self) {
        // `ExitToMainScreen`: `TutorialStop` (the overrun flags back); in multiplayer the session's teardown
        // (`MultiCloseClient`/`Server`).
        if let Some(t) = self.tutorial.take() {
            let m = &mut self.world.game.state.mem;
            for (i, x) in t.overrun.into_iter().flatten().enumerate() {
                m.set_i32(0xB77230 + 4 * i as u32, x);
            }
            if let Some(a) = t.ai_off {
                m.set_i32(ig2_sim::save::va::AI_OFF, a);
            }
        }
        if let Some((net, _)) = self.world.net.take() {
            net.leave();
        }
        let scene = self.front.take().and_then(|f| f.scene);
        let mut front = Front::new(scene, self.now_ms(), &self.settings);
        // Ours (ADR 0050): back from a won scenario, its list under the high-score entry (`MainScr_Belep` step 7).
        if let Some((registry, score)) = self.won_scenario() {
            let a = front.scenarios.get_or_insert_with(|| crate::mainscr_scenario::Assets::load(&self.install, &crate::ui::UiTexts::load(&self.install)));
            front.s.open_high_score(a.entries.clone(), &registry, score);
        }
        // Ours (ADR 0034): a finished game's statistics, after its end videos.
        front.stats = self.stats_end.take().map(crate::stats_screen::StatsScreen::end);
        self.front = Some(front);
        self.close_screens();
        self.audio.play_music(crate::audio::Music::Menu, false, human_race(&self.world.game.state));
    }

    /// What the screen asks once it has faded out (or, for a save, at once).
    fn options_act(&mut self, a: crate::options::Act) {
        use crate::options::Act;
        let Some(o) = self.options.take() else { return };
        self.store_settings_from(&o.s);
        if a != Act::Back {
            self.trade_under_options = None;
        }
        match a {
            Act::Back => {
                // `SetIdoPause(OptionsOldIdoPause)`.
                if !o.was_paused {
                    self.send(StarmapOrder::Pause { paused: false });
                }
                self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
                // Back to the trade screen it was opened from (`OldKep` 8), entered without its opening.
                if let Some(old) = self.trade_under_options.take() {
                    self.trade = Some(crate::trade::Trade::back(&old, &self.world.game.state));
                    self.overlay = Some(crate::ui::Screen::Trade);
                }
                // The screen before is entered again (`_Belep` with `OldKep` 0xd, re/screens/options.md §3.1): the spy
                // screen ignores `OldKep` and plays its opening again; research, diplomacy and info start over without
                // their opening (selection, menus, counters); the colony's `Colony_Belep` reads no `OldKep` (its
                // ambience again); the starmap's `Starmap_Belep` re-inits its faders (`fifo`, `starf`); design keeps
                // its state.
                use crate::ui::Screen;
                if let Some(mut sw) = self.spacewar_parked.take() {
                    sw.reenter(&mut self.world);
                    self.spacewar = Some(sw);
                } else if self.spy.is_some() {
                    self.spy = None;
                    self.toggle_spy();
                } else {
                    match self.overlay {
                        // `Research_Belep` again, unanimated: the state rebuilt, the field's `Res_Hangok[1]` once the
                        // models load (exe 0x523540, 0x51ee14); its place as the screen left it.
                        Some(Screen::Research) => {
                            self.drop_research();
                            self.enter_research(0xd);
                        }
                        Some(Screen::Diplomacy) => self.enter_diplomacy(false),
                        Some(Screen::Info) => {
                            if let Some(h) = &mut self.hud {
                                let tri = h.info.tri;
                                h.info = crate::ui::InfoScreen::enter(self.world.ui.human);
                                h.info.tri = tri;
                            }
                        }
                        None if self.colony.is_some() => self.reenter_colony(),
                        None => self.on_starmap = false,
                        _ => {}
                    }
                }
            }
            Act::MainMenu => {
                // `Options_Kilep` under the tutorial: `TutorialStop(1)` (the music's duck back) before `ExitToMainScreen`.
                if o.s.tutorial {
                    self.audio.set_duck(0);
                }
                self.exit_to_main_menu();
            }
            Act::Load(n) => {
                if let Some(path) = o.s.slots.get(n).and_then(Option::as_ref).map(|s| s.file.clone()) {
                    self.load_file(&path);
                }
            }
            Act::Save(..) => {}
            // Ours (ADR 0067): Exit game, as the window's close (in multiplayer the session is left first).
            Act::Exit => {
                self.exit_to_main_menu();
                if let Some(f) = &mut self.front {
                    f.quit = true;
                }
            }
        }
    }

    /// `SaveGlobalRegistrySettings`: the screen's settings into the remake's own settings file (beside the original's
    /// `options.dat`, which stays untouched), and the rows' values for the next time.
    fn store_settings_from(&mut self, s: &crate::options::OptionsScreen) {
        let mut own = ig2_formats::options::Options::own_path().and_then(|p| ig2_formats::userfs::read(p).ok()).map(|b| ig2_formats::options::Options::parse(&b)).unwrap_or_default();
        s.store(&mut own);
        if let Some(p) = ig2_formats::options::Options::own_path() {
            if let Err(e) = ig2_formats::userfs::write(&p, own.to_bytes()) {
                eprintln!("ig2-client: {}: {e}", p.display());
            }
        }
        s.store(&mut self.settings);
        if let (Some(m), Some(f)) = (&s.modern, &mut self.front) {
            f.s.player = m.name.clone();
        }
        for r in &mut self.opt_rows {
            if let Some(n) = s.rows.iter().find(|n| n.key == r.key) {
                r.value = n.value;
            }
        }
        self.apply_battle_rows();
        self.apply_display();
    }

    /// `SaveOptions` (exe 0x51b640): the battle rows into the battle flags the sim reads (exe 0xB77230..0xB7723C), as
    /// a row is set and as a game starts or loads (the flags aren't in a save). Not in multiplayer: there they are
    /// lockstep state (ADR 0011). Under the tutorial's `DISABLE OVERRUN` they go into the values `TutorialStop` puts
    /// back, so its override holds while it runs.
    fn apply_battle_rows(&mut self) {
        let Some(v) = crate::options::battle_flags(&self.opt_rows) else { return };
        if self.multiplayer() {
            return;
        }
        if let Some(saved) = self.tutorial.as_mut().and_then(|t| t.overrun.as_mut()) {
            *saved = v;
            return;
        }
        for (i, x) in v.into_iter().enumerate() {
            self.world.game.state.mem.set_i32(0xB77230 + 4 * i as u32, x);
        }
    }

    /// The display rows' settings (crate::display) applied where they changed: the window's mode and size, vsync,
    /// the letterbox, the live background and the names' language.
    fn apply_display(&mut self) {
        let d = crate::display::Display::from_settings(&self.settings);
        let old = std::mem::replace(&mut self.display, d);
        if let Some(g) = &self.gpu {
            if (d.mode, d.size) != (old.mode, old.size) {
                g.window.set_fullscreen(d.fullscreen(g.window.current_monitor()));
                if let (crate::display::Mode::Window, Some((w, h))) = (d.mode, d.size) {
                    let _ = g.window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
                }
            }
        }
        if (d.vsync, d.letterbox) != (old.vsync, old.letterbox) {
            self.vsync = d.vsync;
            crate::kit::layout::set_letterbox(d.letterbox);
            self.reconfigure();
        }
        crate::kit::layout::set_box_models(d.box_models);
        crate::kit::layout::set_slider_under(d.slider_under);
        crate::diplomacy::set_articles(d.articles);
        crate::kit::layout::set_centred_exit(d.centred_exit);
        crate::render::set_sharp_ui(d.sharp_ui);
        crate::render::set_sharp_text(d.sharp_text);
        crate::theme::set(self.settings.get(crate::display::UI_THEME).unwrap_or(""));
        crate::upscale::set_factor(d.upscale);
        crate::render3d::set_lighting(d.lighting);
        crate::battle_fx::set_enhanced(d.battle_effects);
        crate::render::set_smooth_glows(d.smooth_glows);
        crate::gui::set_ui_scale(crate::display::ui_scale(&self.settings));
        self.bindings = crate::bindings::Bindings::from_settings(&self.settings);
        self.audio.voice_volume = crate::display::voice_volume(&self.settings);
        if d.colour_blind != old.colour_blind {
            crate::starmap::set_colour_blind(d.colour_blind);
            self.reload_palettes();
        }
        if d.live_background != old.live_background {
            self.backdrop.live = d.live_background;
        }
        if let Some(f) = &mut self.front {
            f.s.settings = self.settings.int(crate::display::MENU_SETTINGS) != Some(0);
            f.s.flow = self.settings.int(crate::menu_flow::KEY) == Some(1);
            f.s.history = d.statistics;
        }
        if d.one_language != old.one_language {
            self.one_language = d.one_language;
            if let Some(h) = &mut self.hud {
                h.set_one_language(&self.install, d.one_language);
            }
        }
    }

    /// ADR 0036: the palettes read again after the colour-blind option changed.
    fn reload_palettes(&mut self) {
        let Ok(p) = crate::starmap::read_palette(&self.install) else { return };
        if let Some(h) = &mut self.hud {
            h.palette = p.clone();
        }
        if let Some(g) = &mut self.gpu {
            g.assets.set_palette(p);
        }
    }

    /// `Options_EnterPressed`: `LoadSave(slot, name, 2)` as the remake's own save (`save<n>.ig2s`), the slots read
    /// again and back to the screen before; a failed save stays (the exe's "Failed to save game!" line).
    fn options_save(&mut self, slot: usize, name: Vec<u8>) {
        let Some(dir) = crate::options::slot_dir() else { return };
        let ok = self.write_save(&dir.join(format!("save{slot}.ig2s")), &name);
        let slots = crate::options::read_slots(Some(&dir));
        let Some(o) = &mut self.options else { return };
        match ok {
            Ok(()) => o.s.saved(slots),
            Err(e) => {
                self.world.local_lines.push(e.into_bytes());
                o.s.refresh(slots);
            }
        }
    }

    /// `LoadSave(…, 2)`: the game as a slot file (the state, then the title `name` and today's date) at `path`; a
    /// multiplayer game saves the session's state, on the host only.
    fn write_save(&mut self, path: &std::path::Path, name: &[u8]) -> Result<(), String> {
        let (y, m, d) = self.world.curr.date();
        let state = match self.world.mp_save() {
            // `LoadSave(…, 2)` writes the live starmap camera (`StarmapXStart` … `StarmapPlanetKering`, exe 0xB9A968..);
            // a multiplayer save leaves the synced state alone.
            None => {
                if let Some(v) = &self.view {
                    self.world.store_view(&v.camera);
                }
                // The research screen's place (one peer's UI) as the exe's globals hold it when saving.
                if let Some(p) = self.research.as_ref().map(|r| r.place()).or(self.research_place) {
                    p.write(&mut self.world.game.state);
                }
                self.world.save_file()
            }
            Some(r) => r?,
        };
        let file = crate::options::slot_file(state, name, [y, m, d]);
        let groups = self.fleet_groups;
        path.parent().map_or(Ok(()), ig2_formats::userfs::create_dir_all).and_then(|_| ig2_formats::userfs::write(path, file)).and_then(|_| groups.store(path)).map_err(|e| {
            eprintln!("ig2-client: {}: {e}", path.display());
            String::from_utf8_lossy(&crate::tr::fill(crate::tr::game(), "Cannot save: {}", &[e.to_string().as_bytes()])).into_owned()
        })
    }

    /// The Options screen's save gates (`Options_Main`'s save button) without the screen: a game runs, not under the
    /// tutorial, not in a battle (the space battle or a ground war) unless `DEBUGX`.
    fn can_save(&mut self) -> bool {
        let debug = self.world.game.state.mem.u8(ig2_sim::save::va::DEBUGX) != 0;
        let planet = self.world.colony_curr.as_ref().map(|c| c.planet).filter(|_| self.colony.is_some() && self.overlay.is_none());
        let war = planet.is_some_and(|p| self.world.game.warfare_status(p) == 1);
        let battle = self.spacewar.is_some() || self.spacewar_parked.is_some() || war;
        let tutorial = self.tutorial.as_ref().is_some_and(|t| t.t.active || t.t.paused);
        self.front.is_none() && self.options.is_none() && self.help.is_none() && !tutorial && (!battle || debug)
    }

    /// ADR 0032: F12 saves into the quick save beside the slots.
    fn quick_save(&mut self) {
        let Some(dir) = crate::options::slot_dir() else { return };
        if !self.can_save() {
            return;
        }
        let (y, m, d) = self.world.curr.date();
        let line = match self.write_save(&dir.join(crate::options::QUICKSAVE), &crate::tr::fill(crate::tr::game(), "Quicksave {}", &[format!("{y} {m} {d}").as_bytes()])) {
            Ok(()) => crate::tr::get(crate::tr::game(), "Quick saved."),
            Err(e) => e.into_bytes(),
        };
        self.world.local_lines.push(line);
    }

    /// The main menu's "History" (ours, ADR 0034): the match history over the menu.
    fn open_history(&mut self) {
        let (list, error) = match crate::stats_screen::history_dir().map(|d| crate::stats_screen::load(&d)) {
            Some(Ok(l)) => (l, None),
            Some(Err(e)) => (Vec::new(), Some(e)),
            None => (Vec::new(), None),
        };
        if let Some(f) = &mut self.front {
            f.stats = Some(crate::stats_screen::StatsScreen::history(list, error));
            f.opening = true;
        }
        self.audio.play(crate::audio::CLICK);
    }

    /// The main menu's "Replays" (ours, ADR 0038 section 6): the replays list over the menu.
    fn open_replays(&mut self) {
        if let Some(f) = &mut self.front {
            f.replays = Some(crate::replay_viewer::ReplayList::open(None));
            f.opening = true;
        }
        self.audio.play(crate::audio::CLICK);
    }

    /// ADR 0032: Shift+F12 loads the quick save (single player only), as the Options screen's load does.
    fn quick_load(&mut self) {
        let Some(path) = crate::options::slot_dir().map(|d| d.join(crate::options::QUICKSAVE)) else { return };
        let tutorial = self.tutorial.as_ref().is_some_and(|t| t.t.active || t.t.paused);
        if self.multiplayer() || self.front.is_some() || self.options.is_some() || self.help.is_some() || tutorial || !path.exists() {
            return;
        }
        if self.load_file(&path) {
            self.world.local_lines.push(crate::tr::get(crate::tr::game(), "Quick loaded."));
        }
    }

    /// `LoadSave(slot, "", 1)`, then the starmap; true when it loaded.
    fn load_file(&mut self, path: &std::path::Path) -> bool {
        match World::load_slot(path, self.rules, &self.tech_defaults, &self.volatile) {
            Ok(w) => {
                self.start_world(w);
                self.fleet_groups = crate::fleet_groups::Groups::load(path);
                // `Options_Main`'s load: the last planet whose ground war runs (`GetWarfareStatus == 1`) opens on its
                // colony screen (`AktKepKell 3`, back to the starmap).
                let n = self.world.game.state.planet_count();
                if let Some(p) = (1..=n).filter(|&p| self.world.game.warfare_status(p) == 1).last() {
                    let sel = self.selection();
                    self.select(Selection { mode: 1, planet: p, ..sel });
                    self.open_colony(p);
                }
                true
            }
            Err(e) => {
                eprintln!("ig2-client: load: {e}");
                false
            }
        }
    }

    /// `IdoNapValtozott` (exe 0x620720) after a tick: on a new day the autosave (ADR 0032), into slot 24 at the
    /// original's point, or every N days with the last one moved to slot 23. Single player not under the tutorial,
    /// with the "Auto Save" row on; in multiplayer only with an N-day cadence and on the host.
    fn autosave_tick(&mut self) {
        if self.world.viewer.is_some() {
            return;
        }
        let today = self.world.curr.date();
        if today == self.autosave.0 {
            return;
        }
        self.autosave = (today, self.autosave.1 + 1);
        let row = |key: &str| self.opt_rows.iter().find(|r| r.key == key).map(|r| r.value);
        let every = crate::options::AUTOSAVE_DAYS[row(crate::display::AUTOSAVE).unwrap_or(0).clamp(0, 3) as usize];
        let on = row("OptionsKellAutoSave") != Some(0);
        let tutorial = self.tutorial.as_ref().is_some_and(|t| t.t.active || t.t.paused);
        let mp = self.world.net.is_some();
        let (y, m, d) = today;
        if !on || tutorial || (mp && (every == 0 || !self.world.mp_host())) || !crate::options::autosave_due(every, self.autosave.1, m, d) {
            return;
        }
        let Some(dir) = crate::options::slot_dir() else { return };
        self.autosave.1 = 0;
        if every > 0 {
            if let Err(e) = crate::options::rotate_autosaves(&dir) {
                eprintln!("ig2-client: autosave: {e}");
            }
        }
        if let Err(e) = self.write_save(&dir.join("save24.ig2s"), &crate::tr::fill(crate::tr::game(), "Autosave {}", &[format!("{y} {m} {d}").as_bytes()])) {
            self.world.local_lines.push(e.into_bytes());
        }
    }

    /// The screens of the game before close (a load or the main menu starts afresh).
    fn close_screens(&mut self) {
        self.colony = None;
        self.overlay = None;
        self.spy = None;
        self.diplomacy = None;
        self.drop_research();
        self.stop_battle_loops();
        self.spacewar = None;
        self.spacewar_parked = None;
        self.cam_tween = None;
        self.cam_saved = None;
        self.panels = Panels::default();
        self.menus = Menus::default();
    }

    /// A loaded slot (`Options_Main`'s load: `LoadSave(slot, "", 1)`, `SetIdoSpeed(1, 0)`, unpaused, the starmap).
    fn start_world(&mut self, w: World<'static>) {
        self.close_screens();
        self.tutorial = None;
        self.research_place = None;
        self.akt_planet = None;
        self.fleet_groups = Default::default();
        self.volt_harc = false;
        self.world.end_replay();
        self.world = w;
        self.autosave = (self.world.curr.date(), 0);
        // A replay's battle flags are its start's.
        if self.world.viewer.is_none() {
            self.apply_battle_rows();
        }
        // `_s(0x3b, n)` is the install's table, not the save's: a loaded game names the AI's designs too.
        if self.world.game.design_texts.is_empty() {
            self.world.game.design_texts = crate::new_game::design_texts(&self.install);
        }
        self.world.set_speed(ig2_sim::time::Speed::Normal);
        if self.world.paused() {
            self.send(StarmapOrder::Pause { paused: false });
        }
        if let Some(g) = &self.gpu {
            let [_, _, bw, bh] = g.layout().band();
            self.view = Some(View::new(&self.world.curr, &self.world.view, (bw as u32, bh as u32), g.window.scale_factor() as f32));
        }
        if let Some(h) = &self.hud {
            self.world.game.empire_names = h.empire_names.clone();
        }
        self.front = None;
        // A campaign's intro comes first; the music follows it (`AfterVideo::StarmapEntry`).
        if self.world.videos.is_empty() {
            self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game.state));
        }
        self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
        if self.world.viewer.is_none() {
            self.start_replay();
        }
    }

    /// Ours (ADR 0038 §6): a replay from the list opens read-only; one that doesn't read stays in the list with why.
    fn watch_replay(&mut self, path: &std::path::Path) {
        let w = ig2_formats::userfs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| ig2_net::replay::Replay::read(&b).map_err(|e| e.to_string()))
            .and_then(|r| {
                if r.meta.build != crate::version() {
                    eprintln!("ig2-client: replay recorded by {}, played by {}", r.meta.build, crate::version());
                }
                World::watch(r, self.rules)
            });
        match w {
            Ok(w) => self.start_world(w),
            Err(e) => {
                if let Some(f) = &mut self.front {
                    f.replays = Some(crate::replay_viewer::ReplayList::open(Some(format!("{}: {e}", path.display()))));
                }
            }
        }
    }

    /// The replay viewer's bar is up: a replay on the game screens, nothing over them.
    fn replay_bar_up(&self) -> bool {
        self.world.viewer.is_some() && self.front.is_none() && self.options.is_none() && self.help.is_none() && self.video.is_none()
    }

    /// The bar's or a key's act; Exit, or the replay's end, goes back to the replays list.
    fn replay_act(&mut self, a: crate::replay_viewer::Act) {
        use crate::replay_viewer::{Act, End};
        let Some(v) = &mut self.world.viewer else { return };
        v.act(a);
        let end = v.end;
        if a != Act::Exit && end.is_none() {
            return;
        }
        self.exit_to_main_menu();
        // A replay that drifted from its recording says where (ADR 0038 §3).
        let error = match end {
            Some(End::Diverged(d)) => Some(format!("The replay stopped: {d}.")),
            _ => None,
        };
        if let Some(f) = &mut self.front {
            f.replays = Some(crate::replay_viewer::ReplayList::open(error));
        }
    }

    /// ADR 0038: the game's replay into `replays/` beside the slots, unless the "Record replays" row is off.
    fn start_replay(&mut self) {
        let on = self.settings.int(crate::display::REPLAYS) != Some(0);
        let Some(dir) = crate::options::slot_dir().map(|d| d.join("replays")).filter(|_| on) else { return };
        let mp = self.world.net.is_some();
        let r = ig2_formats::userfs::create_dir_all(&dir).and_then(|()| {
            crate::options::prune_replays(&dir, crate::options::KEPT_REPLAYS - 1)?;
            self.world.record_replay(dir.join(crate::options::replay_name(mp)))
        });
        if let Err(e) = r {
            eprintln!("ig2-client: replay: {e}");
        }
    }

    /// The battle screen's part of the frame's update (re/screens/spacewar.md): a started battle opens it, an ended one
    /// zooms out, then [`crate::spacewar_live::Live::update`].
    fn spacewar_update(&mut self, now: u32) {
        use ig2_sim::game::RealTimeEvent;
        for e in std::mem::take(&mut self.world.realtime) {
            match e {
                RealTimeEvent::SpaceStarted { slot } if self.spacewar.is_none() && self.spacewar_parked.is_none() => {
                    self.enter_space_battle(slot, now);
                }
                // `StartGrWar` (exe 0x4a4a70): 900 when the field is contested (`WF::GetStatus` −1 or −10), any planet;
                // it also ends the tutorial's `WAITFORBATTLE` (re/rules/tutorial.md §1).
                RealTimeEvent::GroundStarted { planet, holder } => {
                    if let Some(t) = &mut self.tutorial {
                        t.t.battle_started();
                    }
                    if holder.is_none() {
                        match &mut self.colony {
                            Some(c) => c.ui.war.banner = 900,
                            None => self.attack_banner = 900,
                        }
                        // SP, the human takes part (the sim paused it at speed 1), or MP (every machine, ELF 0x4a4f80):
                        // the war's colony screen; the old screen and planet are kept.
                        if self.multiplayer() || self.world.game.sp_battles.old_pause.is_some() {
                            self.grwar_old = Some((self.shown_kep(), self.akt_planet()));
                            self.show_colony_screen(planet);
                        }
                    }
                }
                // `GroundWarFinished(p, s, false)`: on the war's colony screen, back to the old screen (the battle
                // screen's 0xB becomes the starmap) and its pause.
                RealTimeEvent::GroundEnded { planet, .. } => {
                    let shown = self.shown_kep() == 3 && self.world.colony == Some(planet);
                    if let Some((kep, p)) = self.grwar_old.take().filter(|_| shown) {
                        let kep = if kep == 0xb { 2 } else { kep };
                        if kep == 3 {
                            self.show_colony_screen(p);
                        } else {
                            self.request_screen(kep);
                        }
                    }
                }
                RealTimeEvent::SpaceOver { slot, .. } => {
                    // Under the options screen (MP: the time runs on): the battle ends without its screen.
                    if self.spacewar_parked.as_ref().is_some_and(|sw| sw.ui.slot == slot) {
                        self.spacewar_parked = None;
                        self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game.state));
                    }
                    if let Some(sw) = self.spacewar.as_mut().filter(|sw| sw.ui.slot == slot) {
                        sw.over(&self.world, now);
                        // MP: `SetIdoPause` changes nothing (`leave_battle`); sent, it would pause every machine.
                        if !self.multiplayer() {
                            self.send(StarmapOrder::Pause { paused: true });
                        }
                    }
                }
                _ => {}
            }
        }
        self.mp_battle_check(now);
        self.resume_parked_battle();
        let Some(sw) = &mut self.spacewar else { return };
        sw.pointer = self.cursor_px;
        if let Some(g) = &self.gpu {
            let size = g.window.inner_size();
            sw.window = [size.width as i32, size.height as i32];
        }
        let out = sw.update(&mut self.world, now);
        self.spacewar_out(out);
    }

    /// `SpaceWar_Belep`: the battle screen for `slot`, always entered from the starmap.
    fn enter_space_battle(&mut self, slot: usize, now: u32) {
        let Some(cam) = self.view.as_ref().map(|v| v.camera) else { return };
        self.colony = None;
        self.world.colony = None;
        self.overlay = None;
        self.spy = None;
        if let Some(g) = &mut self.gpu {
            g.assets.planet_meshes.load_battle(&mut g.renderer, &self.install);
        }
        // `SpaceWar_Belep` exe 0x5628ac: `PlayMusicSpWar(1, 0)` (crossfade) when no battle ran yet.
        self.audio.play_music(crate::audio::Music::SpWar, false, human_race(&self.world.game.state));
        self.spacewar = crate::spacewar_live::Live::enter(&mut self.world, slot, &cam, &self.install, now);
    }

    /// Multiplayer: every machine's strategic clock waits until each human has left the running battle's screen
    /// (0x44d). A machine without that screen up (a rejoin, a resumed save, a missed start, a battle decided under
    /// Options) opens it, or leaves at once when the battle is decided or the screen can't open.
    fn mp_battle_check(&mut self, now: u32) {
        let waits = self.world.game.space_battle_waits_on(self.world.game.local_view().empire);
        if waits.is_none() {
            self.mp_left_sent = false;
        }
        let Some(slot) = waits.filter(|_| self.multiplayer() && !self.mp_left_sent) else { return };
        if self.spacewar.is_some() || self.spacewar_parked.is_some() || self.view.is_none() {
            return;
        }
        if self.world.game.realtime.slots.get(slot).is_some_and(|b| b.active && !b.ended) {
            self.enter_space_battle(slot, now);
        }
        if self.spacewar.is_none() {
            self.world.send(Command::SpaceBattleLeft);
            self.mp_left_sent = true;
        }
    }

    /// `AktKepKell = to` on the battle screen, once its opening zoom is over: `SpaceWar_Kilep`'s phase 1 (exe 0x562b50),
    /// the zoom-out to the starmap (five times faster towards Options), the tutorial's 0x20 (not towards Options, Help
    /// or the messages screen), `nemlehetidotallitani = 1; SetIdoPause(1, 0)` (ignored in MP).
    fn leave_battle(&mut self, to: i32) {
        if !self.spacewar.as_ref().is_some_and(|sw| sw.clock_free()) {
            return;
        }
        if !matches!(to, 0xd | 1 | 10) {
            self.world.game.tutorial_check_event(0x20);
        }
        let now = self.now_ms();
        if let Some(sw) = &mut self.spacewar {
            if to == 0xd {
                sw.leave_for_options(&self.world, now);
            } else {
                sw.leave_for_screen(&self.world, now, to);
            }
        }
        if !self.multiplayer() {
            self.send(StarmapOrder::Pause { paused: true });
        }
    }

    /// `FUN_00594a40` (exe, from `Fake_FRISSITES` 0x4de788, only while `AktKep == 2`): a battle left running
    /// (`spacewarbanvanbarkacs` 0xf176ee) is asked for again at once (`AktKepKell = 0xb`), whatever screen was asked
    /// for before; `SpaceWar_Belep` resumes it with its opening zoom.
    fn resume_parked_battle(&mut self) {
        if self.spacewar_parked.is_none() || self.kep_kell.is_some() || self.screen_changing() || self.akt_kep_now() != 2 {
            return;
        }
        if let Some(mut sw) = self.spacewar_parked.take() {
            sw.reenter(&mut self.world);
            self.spacewar = Some(sw);
        }
    }

    /// The battle screen's weapon loops stop with it (`lovedeksoundend`, `unloadgfx`'s `lovedekfreesound`).
    fn stop_battle_loops(&mut self) {
        if let Some(sw) = &mut self.spacewar {
            for (_, ch) in sw.loops.drain() {
                self.audio.stop(ch);
            }
        }
    }

    /// What the battle screen asked for.
    fn spacewar_out(&mut self, out: Vec<crate::spacewar_ui::Out>) {
        use crate::spacewar_ui::Out;
        for o in out {
            let Some(sw) = &self.spacewar else { return };
            // Towards Options a running battle stays: `SpaceWar_Kilep` (exe 0x562b50) returns before `SpaceWarVege` /
            // `Multi_Send(0x44d)`.
            if !(o == Out::Left && (sw.to_options || sw.to_screen.is_some()) && !sw.ended) {
                for c in sw.commands(&o) {
                    self.world.send(c);
                }
                self.mp_left_sent |= o == Out::Left;
            }
            match o {
                Out::Click => self.sounds.push(crate::audio::CLICK),
                Out::Sound(s) => {
                    self.audio.set_record_volume(s.id, s.volume);
                    self.sounds.push(s.id);
                }
                Out::Loop(l) => {
                    use crate::spacewar_fx::Loop;
                    let Some(sw) = &mut self.spacewar else { return };
                    match l {
                        Loop::Start { key, id } => {
                            if let Some(ch) = self.audio.start_loop(id) {
                                sw.loops.insert(key, ch);
                            }
                        }
                        Loop::Volume { key, volume } => {
                            if let Some(&ch) = sw.loops.get(&key) {
                                self.audio.set_channel_volume(ch, volume);
                            }
                        }
                        Loop::End { key } => {
                            if let Some(ch) = sw.loops.remove(&key) {
                                self.audio.stop(ch);
                            }
                        }
                    }
                }
                Out::Left if self.spacewar.as_ref().is_some_and(|sw| sw.to_screen.is_some() && !sw.ended) => {
                    // `SpaceWar_Kilep`'s phase 2 towards another screen (exe 0x562b50): the tutorial's 0x20 again (not
                    // towards Help or the messages screen), `starmaphovaleptovabb = target`, the battle parked (no
                    // `SpaceWarVege`, `SetIdoPause(1, 0)`) and the starmap entered, which asks for the battle again
                    // at once (`resume_parked_battle`): the target is dropped.
                    if self.spacewar.as_ref().and_then(|sw| sw.to_screen).is_some_and(|to| !matches!(to, 1 | 10)) {
                        self.world.game.tutorial_check_event(0x20);
                    }
                    self.world.game.sp_battles.on_screen = false;
                    self.stop_battle_loops();
                    self.spacewar_parked = self.spacewar.take();
                    if let Some(v) = &mut self.view {
                        crate::spacewar_live::restore_view(v);
                    }
                }
                Out::Left if self.spacewar.as_ref().is_some_and(|sw| sw.to_options && !sw.ended) => {
                    // `SpaceWar_Kilep`'s phase 2 towards Options (exe 0x562c0f): `GFX_Starmap(); SaveScreen(2);
                    // OptionsHarcFolyik = 1`; no tutorial event 0x20 (not towards 0xd), the battle stays.
                    self.world.game.sp_battles.on_screen = false;
                    self.stop_battle_loops();
                    self.spacewar_parked = self.spacewar.take();
                    if let Some(v) = &mut self.view {
                        crate::spacewar_live::restore_view(v);
                    }
                    self.open_options(false);
                }
                Out::Left => {
                    // `SpaceWar_Kilep`'s first statement (exe 0x562b68): the tutorial's 0x20 (not towards the options,
                    // Help or the trade screen).
                    self.world.game.tutorial_check_event(0x20);
                    // `SpaceWar_Kilep`'s second phase: a finished battle ends (`SpaceWarVege`); one still running stays.
                    self.world.game.sp_battles.on_screen = false;
                    self.stop_battle_loops();
                    // `Starmap_Belep` exe 0x5b1d74: `PlayMusicMain(0, 0)` unless a battle still runs.
                    if self.spacewar.as_ref().is_some_and(|sw| sw.ended) {
                        self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game.state));
                    }
                    // A battle over during the zoom-out towards Options (MP: the time runs on): phase 2 still takes
                    // the 0xd branch (`OptionsHarcFolyik = 1`) before its end, so Options opens.
                    let to_options = self.spacewar.as_ref().is_some_and(|sw| sw.to_options);
                    self.spacewar = None;
                    if let Some(v) = &mut self.view {
                        crate::spacewar_live::restore_view(v);
                    }
                    if to_options {
                        self.open_options(false);
                        if let Some(o) = &mut self.options {
                            o.s.from_battle = true;
                            o.s.battle = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// The battle screen's frame: its input and its picture.
    fn spacewar_frame(&mut self, now: u32, alpha: f32, size: [f32; 2], t: f32, globe: Option<crate::render::TexId>) -> Option<crate::render::Frame> {
        let sw = self.spacewar.as_mut()?;
        let (g, v, h) = (self.gpu.as_mut()?, self.view.as_mut()?, self.hud.as_ref()?);
        let mouse = v.camera.virtual_of(self.cursor);
        let (frame, out) = sw.frame(&self.world, &mut g.renderer, &self.install, &g.assets, v, h, self.tables, mouse, self.modifiers.shift_key(), now, alpha, t, size, globe);
        self.spacewar_out(out);
        frame
    }

    /// `SpaceWarKeyboardHandler` (spec §7.1.5), with the shared keys it leaves to `DoKeyboard`: space pauses, Esc
    /// leaves the screen (`SpaceWar_Kilep`'s zoom-out) [u: `DoKeyboard` not read].
    fn spacewar_key(&mut self, event: &winit::event::KeyEvent) {
        self.spacewar_key_by(event.logical_key.as_ref(), event.physical_key, event.state == ElementState::Pressed);
    }

    /// [`Self::spacewar_key`] by its parts: a live key, or one the tutorial's recording plays (`ReadNext`).
    fn spacewar_key_by(&mut self, logical: Key<&str>, physical: winit::keyboard::PhysicalKey, down: bool) {
        use crate::spacewar_ui::Key as K;
        use winit::keyboard::{KeyCode, PhysicalKey};
        // MP: `DoKeyboard`'s chat entry (input.md §5; `AktKep` 0xb < 0xd) opens and takes the keys here too.
        if down && self.chat_key(&logical) {
            return;
        }
        // `DoKeyboard` (re/screens/input.md §1) first: the top row's 1..3 set the speed and unpause (sound 10) and
        // never reach the screen, Space and P pause, O and Esc (`ESC_Pressed`) open the options; the pause and speed
        // keys wait for the opening zoom (`nemlehetidotallitani`).
        let ch = match logical {
            Key::Character(c) => c.chars().next().map(|c| c.to_ascii_lowercase()),
            _ => None,
        };
        let digit = match physical {
            PhysicalKey::Code(c) if (KeyCode::Digit1..=KeyCode::Digit9).contains(&c) => Some(c as i32 - KeyCode::Digit0 as i32),
            _ => None,
        };
        if down {
            let free = self.multiplayer() || self.spacewar.as_ref().is_some_and(|sw| sw.clock_free());
            if matches!(logical, Key::Named(NamedKey::Space)) || ch == Some('p') {
                if free {
                    self.toggle_pause();
                }
                return;
            }
            if let Some(n @ 1..=3) = digit {
                if self.multiplayer() {
                    self.set_speed(n);
                } else if free && !self.speed_locked() {
                    self.sounds.push(crate::audio::SoundId::global(10));
                    self.send(StarmapOrder::Speed { level: n });
                    self.send(StarmapOrder::Pause { paused: false });
                }
                return;
            }
            // Every key repeats while held (`MainCallBackEventHandler` exe 0x4e2db0 pushes the OS repeats into the
            // key ring: no repeat test); the zoom-out holds the repeats off (`nemlehetidotallitani`).
            if matches!(logical, Key::Named(NamedKey::Escape)) || ch == Some('o') {
                self.leave_battle(0xd);
                return;
            }
        }
        // `SpaceWarKeyboardHandler`: the letters by the typed character (the char events), the rest by key code.
        let k = match (logical, ch, digit) {
            (Key::Named(NamedKey::ArrowLeft), ..) => Some(K::Left),
            (Key::Named(NamedKey::ArrowRight), ..) => Some(K::Right),
            (Key::Named(NamedKey::ArrowUp), ..) => Some(K::Up),
            (Key::Named(NamedKey::ArrowDown), ..) => Some(K::Down),
            _ if physical == PhysicalKey::Code(KeyCode::NumpadAdd) => Some(K::ZoomIn),
            _ if physical == PhysicalKey::Code(KeyCode::NumpadSubtract) => Some(K::ZoomOut),
            (_, Some('a'), _) => Some(K::All),
            (_, Some('c'), _) => Some(K::Camera),
            (_, Some('r'), _) => Some(K::Retreat),
            (_, _, Some(n)) => Some(K::Group(n - 1)),
            (Key::Named(NamedKey::Backspace), ..) => Some(K::Explode),
            _ => None,
        };
        if let Some(k) = k {
            let debug = self.world.game.state.mem.u8(ig2_sim::save::va::DEBUGX) != 0;
            let ctrl = self.modifiers.control_key();
            let mut out = Vec::new();
            if let Some(sw) = &mut self.spacewar {
                if let Some(b) = self.world.game.realtime.slots.get(sw.ui.slot) {
                    let tp = self.world.game.state.mem.u8(ig2_sim::save::va::TUTORIAL_PAUSED) != 0;
                    sw.ui.key(b, k, down, ctrl, debug, tp, &mut out);
                }
            }
            self.spacewar_out(out);
        }
    }

    /// A key on the main menu: Esc (`MainScr_ESCPressedWhenMultiplayerselect`), or the name entry's keys; our MP
    /// screens take every key while they are up.
    fn front_key(&mut self, k: &Key) {
        use crate::options::EntryKey;
        let Some(f) = &mut self.front else { return };
        if let Some(st) = &mut f.stats {
            match k.as_ref() {
                Key::Named(NamedKey::Escape | NamedKey::Enter) => {
                    if st.back().is_some() {
                        f.stats = None;
                    }
                }
                Key::Named(NamedKey::ArrowLeft) => st.step_metric(false),
                Key::Named(NamedKey::ArrowRight) => st.step_metric(true),
                _ => {}
            }
            return;
        }
        if f.replays.is_some() {
            if matches!(k.as_ref(), Key::Named(NamedKey::Escape | NamedKey::Enter)) {
                f.replays = None;
            }
            return;
        }
        if let Some(m) = &mut f.mods {
            if let Some(a) = m.key(k, self.modifiers.shift_key()) {
                self.mods_act(a);
            }
            return;
        }
        if let Some(mp) = &mut f.mp {
            // Ctrl+V pastes (a join code), Ctrl+C copies the host's join code.
            if self.modifiers.control_key() {
                match k.as_ref() {
                    Key::Character(c) if c.eq_ignore_ascii_case("v") => match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                        Ok(text) => mp.paste(&text),
                        Err(e) => eprintln!("ig2-client: clipboard: {e}"),
                    },
                    Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                        if let Some(o) = mp.copy() {
                            self.front_mp_out(o);
                        }
                    }
                    _ => {}
                }
                return;
            }
            // Ours (ADR 0064): Tab moves between the connection page's fields, the arrows through its list.
            match k.as_ref() {
                Key::Named(NamedKey::Tab) => return mp.next_field(),
                Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) => return mp.step_list(matches!(k.as_ref(), Key::Named(NamedKey::ArrowDown))),
                _ => {}
            }
            let escape = matches!(k.as_ref(), Key::Named(NamedKey::Escape));
            // An open drop-down takes Esc (it closes) and the keys meant for it.
            if let Some(g) = self.gpu.as_ref().filter(|g| egui::Popup::is_any_open(&g.gui.ctx)) {
                if escape {
                    egui::Popup::close_all(&g.gui.ctx);
                }
                return;
            }
            let key = match k.as_ref() {
                Key::Named(NamedKey::Enter) => Some(EntryKey::Enter),
                Key::Named(NamedKey::Backspace) => Some(EntryKey::Back),
                Key::Named(NamedKey::Delete) => Some(EntryKey::Delete),
                Key::Named(NamedKey::Home) => Some(EntryKey::Home),
                Key::Named(NamedKey::End) => Some(EntryKey::End),
                Key::Named(NamedKey::ArrowLeft) => Some(EntryKey::Left),
                Key::Named(NamedKey::ArrowRight) => Some(EntryKey::Right),
                Key::Named(NamedKey::Space) => Some(EntryKey::Char(b' ')),
                Key::Character(c) => {
                    let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                    b.first().copied().filter(|&b| b >= 0x20).map(EntryKey::Char)
                }
                _ => None,
            };
            if let Some(o) = mp.key(key, escape, &mut f.net) {
                self.front_mp_out(o);
            }
            return;
        }
        if matches!(k.as_ref(), Key::Named(NamedKey::Escape)) {
            f.s.escape();
            return;
        }
        if f.s.name.is_none() && f.s.high_score.is_none() {
            if let (Key::Character("o" | "O"), Some(a)) = (k.as_ref(), f.s.settings_key()) {
                self.front_act(a);
            }
            return;
        }
        let key = match k.as_ref() {
            Key::Named(NamedKey::Enter) => Some(EntryKey::Enter),
            Key::Named(NamedKey::Backspace) => Some(EntryKey::Back),
            Key::Named(NamedKey::Delete) => Some(EntryKey::Delete),
            Key::Named(NamedKey::Home) => Some(EntryKey::Home),
            Key::Named(NamedKey::End) => Some(EntryKey::End),
            Key::Named(NamedKey::ArrowLeft) => Some(EntryKey::Left),
            Key::Named(NamedKey::ArrowRight) => Some(EntryKey::Right),
            Key::Named(NamedKey::Space) => Some(EntryKey::Char(b' ')),
            // The high-score name takes the code page's letters (`HighScoreNev` goes through `KiirStr`).
            Key::Character(c) if f.s.high_score.is_some() => {
                let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                b.first().copied().map(EntryKey::Char)
            }
            Key::Character(c) => c.bytes().next().filter(|b| b.is_ascii()).map(EntryKey::Char),
            _ => None,
        };
        let act = match f.s.high_score.is_some() {
            true => key.and_then(|key| f.s.high_score_key(key)),
            false => key.and_then(|key| f.s.name_key(key)),
        };
        if let Some(a) = act {
            self.front_act(a);
        }
    }

    /// `st_ShowAnim` for each name in turn (`OpenBink(name, 1, 0, 0, force)`, `PlayBink(1, 0, 0)`, `CloseBink`);
    /// `then` once the last ends.
    fn queue_videos(&mut self, names: impl IntoIterator<Item = String>, force: bool, then: AfterVideo) {
        self.video_queue.extend(names.into_iter().map(|n| if force { format!("!{n}") } else { n }));
        self.after_video = then;
    }

    /// The multiplayer wait overlay's state this frame (`MOUSE_DrawOver`'s open and hover).
    fn update_mp_wait(&mut self) {
        let Some(crate::world::WaitView { turn, paused, reconnecting, host, late }) = self.world.wait_view() else {
            self.mp_wait = Default::default();
            return;
        };
        let races = |e: u8| {
            let race = self.world.game.state.mem.u8(ig2_sim::save::va::PLAYER_FAJ + e as u32);
            self.hud.as_ref().map_or_else(Vec::new, |h| h.texts.get(0x11, 1, race as u16 + 5))
        };
        let late = late.into_iter().map(|(e, name)| crate::mp_wait::Late { empire: e, name: name.into_bytes(), text: races(e) }).collect();
        let now = Instant::now();
        self.mp_wait.update(now, turn, paused, reconnecting, host, late);
        let at = self.virtual_cursor();
        self.mp_wait.hover(at, now);
        // `MOUSE_DrawOver` tests `Mouse_Gomb` (0xdb4650: left or right held) every pass, a level: a button held as the
        // items arm acts too.
        if self.mp_wait.is_open() && self.mouse_held & 3 != 0 {
            self.mp_wait_press();
        }
    }

    /// A press on the wait overlay (`MOUSE_DrawOver`): the host's "Close connection", "EXIT GAME".
    fn mp_wait_press(&mut self) {
        let at = self.virtual_cursor();
        match self.mp_wait.press(at, Instant::now()) {
            Some(crate::mp_wait::Act::Close(e)) => self.world.drop_player(e),
            Some(crate::mp_wait::Act::Exit) => self.exit_to_main_menu(),
            None => {}
        }
    }

    /// The story's UI effects ([`crate::world::StoryUi`]): the storyline into the registry (written to
    /// `options.dat` at once; the exe writes it with the next `SaveGlobalRegistrySettings`), the colony screen asked
    /// for, the earthquake, the riot's pause, an automatic message's messages screen.
    fn take_story_ui(&mut self) {
        use crate::world::StoryUi;
        for e in std::mem::take(&mut self.world.story_ui) {
            let p = match e {
                StoryUi::Storyline { campaign, value } => {
                    let key = format!("Fable_{campaign}");
                    self.settings.set(&key, &value.to_string());
                    let mut own = ig2_formats::options::Options::own_path().and_then(|p| ig2_formats::userfs::read(p).ok()).map(|b| ig2_formats::options::Options::parse(&b)).unwrap_or_default();
                    own.set(&key, &value.to_string());
                    if let Some(path) = ig2_formats::options::Options::own_path() {
                        if let Err(e) = ig2_formats::userfs::write(&path, own.to_bytes()) {
                            eprintln!("ig2-client: {}: {e}", path.display());
                        }
                    }
                    continue;
                }
                StoryUi::Earthquake(p) => {
                    self.quake = Some((p, None));
                    p
                }
                StoryUi::Hold(p) => {
                    self.send(StarmapOrder::Pause { paused: true });
                    p
                }
                StoryUi::Messages => {
                    // The exe sets `AktKepKell` over any pending change and from any screen; the port keeps a change
                    // under way and the battle screens (their leave paths aren't a plain request).
                    let s = self.akt_kep_now();
                    if s != 10 && s < 0xe && s != 11 && !(s == 3 && self.colony_war_up()) && self.kep_kell.is_none() {
                        self.request_screen(10);
                    }
                    continue;
                }
            };
            self.show_colony_screen(p);
        }
    }

    /// `AktKepKell := 3`, `AktPlanetKell := p`: the colony screen of planet `p`.
    fn show_colony_screen(&mut self, p: usize) {
        match self.colony {
            Some(_) if self.world.colony == Some(p) => {}
            Some(_) => {
                self.colony = None;
                self.open_colony(p);
            }
            None => {
                self.planet_kell = Some(p);
                self.request_screen(3);
            }
        }
    }

    /// The sim's video requests (`st_ShowAnim`, `MEGHALAS`, `MEGNYERES`) become the queue.
    /// A game end (`MEGHALAS`, `MEGNYERES`) plays its videos (none for a race of 9+ or `MEGNYERES(false)`), then
    /// leaves for the main menu (re/rules/game-end.md §2).
    fn take_video_requests(&mut self) {
        let reqs = std::mem::take(&mut self.world.videos);
        // A headless script run has no screen to play them on.
        if reqs.is_empty() || self.headless.is_some() {
            return;
        }
        let anims = self.anim_names.get_or_insert_with(|| crate::video::anim_names(&self.install));
        let race = human_race(&self.world.game.state) as usize;
        let campaign = self.world.game.state.mem.i32(ig2_sim::save::va::CAMPAIGN_TYPE) > 0;
        let names: Vec<String> = reqs.iter().flat_map(|r| r.names(anims, &self.tables.videos, race, campaign)).collect();
        if reqs.iter().any(crate::video::Request::ends_game) {
            self.record_match();
        }
        // A spectator's game goes on after its defeat (ADR 0026).
        let ends = reqs.iter().any(crate::video::Request::ends_game) && !self.world.game.local_view().spectating;
        match (ends, names.is_empty()) {
            (true, true) => self.exit_to_main_menu(),
            (true, false) => self.queue_videos(names, false, AfterVideo::MainMenu),
            (false, _) => self.queue_videos(names, false, AfterVideo::Resume),
        }
    }

    /// Ours (ADR 0034, re/rules/game-end.md §11): the game ended for this machine: its record is added to the match
    /// history and, unless it watches on as a spectator, shown once the end videos are over.
    fn record_match(&mut self) {
        let Some(o) = self.world.game.over.filter(|_| self.display.statistics) else { return };
        let view = self.world.game.local_view();
        let (y, m, d) = self.world.curr.date();
        let when = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |t| t.as_secs());
        let rec = crate::stats_screen::Match::record(&self.world.game, o, view.empire, self.world.net.is_some(), [y, m, d], when);
        if let Some(dir) = crate::stats_screen::history_dir() {
            if let Err(e) = crate::stats_screen::append(&dir, &rec) {
                eprintln!("ig2-client: match history: {e}");
            }
        }
        if !view.spectating {
            self.stats_end = Some(rec);
        }
    }

    /// A frame while fullscreen videos play: the next one opens when one ends; the game and the menu wait. When the
    /// last ends their clocks go on from now (`FRISSITES_IGAZIT`: no catching up).
    fn video_frame(&mut self) {
        let now = self.now_ms();
        if self.video.as_ref().is_some_and(|v| v.done()) {
            self.video = None;
            self.audio.stop_video();
        }
        while self.video.is_none() {
            let Some(name) = self.video_queue.pop_front() else { break };
            let (name, force) = match name.strip_prefix('!') {
                Some(n) => (n.to_string(), true),
                None => (name, false),
            };
            if let Some(v) = crate::video::playback::Playback::open(&self.install, &name, crate::video::playback::Place::Full, false, force) {
                // Ours (ADR 0051): the title's music only with the intro.
                let gate = (self.display.title_music && name.eq_ignore_ascii_case("title.bik")).then_some(crate::video::TITLE_INTRO_MS);
                self.audio.play_video(&v.player, true, gate);
                self.video = Some(v);
            }
        }
        let Some(v) = &mut self.video else {
            self.clock = TickClock::new(SIM_TICK_MS, now);
            if let Some(f) = &mut self.front {
                f.ui_ms = now;
            }
            match std::mem::take(&mut self.after_video) {
                AfterVideo::MenuMusic => self.audio.play_music(crate::audio::Music::Menu, true, human_race(&self.world.game.state)),
                AfterVideo::StarmapEntry(c) => {
                    self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game.state));
                    self.audio.play_start_voice(c);
                }
                AfterVideo::MainMenu => self.exit_to_main_menu(),
                AfterVideo::Resume => {}
            }
            return;
        };
        let Some(g) = &mut self.gpu else { return };
        v.frame(&mut g.renderer, &self.install, now as u64, g.config.height);
        let win = [g.config.width as f32, g.config.height as f32];
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        v.draw(&mut frame, win, &crate::font::Virtual::new(win));
        let Ok(out) = g.surface.get_current_texture() else {
            self.reconfigure();
            return;
        };
        g.renderer.draw(&frame, &out.texture, (g.config.width, g.config.height), now as f32 / 1000.0);
        g.window.pre_present_notify();
        out.present();
    }

    /// The messages screen's talking-head clip (re/screens/video.md §5.3) and its speech (re/screens/audio.md §5): a
    /// `TE` part read sets the clips and arms `StartAnim`, which fires when the typer's front reaches the other side's
    /// colour (`MessagesGfx2D_2`) and starts the speech and the alien voice; the speech's end (`FRISSIT_Messages2`,
    /// every 11th UI tick) or the typer's end moves to the wait clip, and a plain message or a dialogue's last text to
    /// the end clip (`messages-end.wav`, the music back). Leaving the screen closes it (`Messages_Kilep`). Drawn by
    /// the HUD at (201, 204) with the interlace over it.
    fn head_clip_frame(&mut self, now: u32, due_ui: u64) {
        use crate::audio::SoundId;
        use crate::messages_screen::ClipStep;
        use ig2_rules::sounds::Table;
        let Some(h) = &mut self.hud else { return };
        // `MessagesAnimFrissit` (exe 0x506f60) and `StartAnim`'s test: the clip only at `mesframe` 30, not closing.
        let open = self.overlay == Some(crate::ui::Screen::Messages) && h.messages.text_up();
        let Some(d) = h.messages.open.as_mut().filter(|_| open) else {
            self.head.close();
            self.head_armed = false;
            h.clip = None;
            return;
        };
        let state = &self.world.game.state;
        let en = state.mem.i32(ig2_sim::save::va::EN);
        let lists = self.clip_lists.get_or_insert_with(|| crate::video::message::load_lists(&self.install, en));
        let k = state.mem.i32(ig2_sim::save::va::INBOX_RANDOM + 4 * d.index as u32);
        let race = human_race(state);
        let table = &self.tables.videos.message_clips;
        let steps = std::mem::take(&mut d.clip);
        let audio = &mut self.audio;
        // `StopMessagesHang`, `StopHalkAlienHang`, `MesHangIndult = 0` (`Messages_Main`'s presses, a new message).
        if steps.contains(&ClipStep::Hush) {
            audio.stop_message_speech();
            audio.stop_alien();
            audio.message.indult = false;
        }
        // `StartAnim_EndOfMessage`'s sounds when its clip starts.
        let end_sounds = |audio: &mut crate::audio::Audio| {
            audio.stop_alien();
            audio.play(SoundId::new(Table::Mes, 2));
            audio.set_duck(0);
        };
        for s in &steps {
            if let ClipStep::PartRead(part) = *s {
                self.head.part_read(lists, d.id.max(0) as u32, part);
                self.head_armed = true;
            }
        }
        if self.head_armed && d.front_colour() == 2 {
            // `StartAnim` exe 0x5095b0: the typer muted, the clip, the speech, the alien voice.
            audio.mute(SoundId::global(7), true);
            self.head.start(&self.install, table, k);
            audio.play_message_speech(en, &self.head.sound);
            audio.stop_alien();
            let idx = crate::video::message::alien_index(&self.head.anim, en, k, self.head.face);
            if let Some(rec) = crate::video::message::alien_record(idx) {
                audio.play_alien(rec);
            }
            audio.message.indult = audio.sound;
            self.head_armed = false;
        }
        // `FRISSIT_Messages2` exe 0x505180: the speech over → the wait clip (a conversation) or the end clip.
        for _ in 0..due_ui {
            self.mes_gyorsito += 1;
            if self.mes_gyorsito > 10 {
                self.mes_gyorsito = 0;
                if audio.sound && audio.message.indult && self.head.akt == 0 && self.head.sound.contains("ogg") && !audio.message_speech_playing() {
                    let ended = if d.conversation {
                        audio.message.indult = audio.sound;
                        self.head.wait(&self.install, table, k, race)
                    } else {
                        self.head.end(&self.install, race)
                    };
                    if ended {
                        end_sounds(audio);
                    }
                }
            }
        }
        for s in &steps {
            if let ClipStep::TypedOut { plain, ends } = *s {
                let mut ended = false;
                if self.head.akt == 0 {
                    audio.message.indult = audio.sound;
                    ended = self.head.wait(&self.install, table, k, race);
                }
                if plain || ends {
                    ended |= self.head.end(&self.install, race);
                }
                self.head.fresh = true;
                if ended {
                    end_sounds(audio);
                }
            }
        }
        let Some((renderer, size, _)) = gfx(&mut self.gpu, &mut self.headless) else { return };
        if h.interlace.is_none() {
            h.interlace = crate::assets::Image::load(&self.install, "dataraw/textures/messages/finalinter_bmp.pvr").ok().map(|i| renderer.add_texture(&i));
        }
        h.clip = None;
        let fresh = std::mem::take(&mut self.head.fresh);
        if let Some(p) = self.head.playback.as_mut().filter(|_| !fresh) {
            p.frame(renderer, &self.install, now as u64, size.1);
            let win = [size.0 as f32, size.1 as f32];
            h.clip = p.quad(win, &crate::font::Virtual::new(win));
        }
    }

    /// The transfer panel's fleets to the sim (classic: the state's globals); multiplayer sends each change as a
    /// command, the panel counting only on the starmap (`AktKep == 2`, re/screens/starmap.md §10.2).
    fn sync_transfer(&mut self) {
        let fleets = self.panels.transfer.map(|t| (t.f1, t.f2));
        // Only a change: a write of the same value still adds the byte to the state a replay's hash checks (ADR 0038).
        let m = &self.world.game.state.mem;
        let now = (m.u8(ig2_sim::save::va::STARMAP_TRANSFER_MODE) != 0).then(|| (m.i32(ig2_sim::save::va::STARMAP_TRANSFER_FLEETS.0), m.i32(ig2_sim::save::va::STARMAP_TRANSFER_FLEETS.1)));
        if now != fleets {
            self.world.game.set_transfer(fleets);
        }
        let fleets = fleets.filter(|_| self.shown_kep() == 2);
        if self.world.net.is_some() && fleets != self.sent_transfer {
            self.sent_transfer = fleets;
            self.world.send(Command::TransferPanel { fleets });
        }
    }

    /// Multiplayer: an ended rename's name (and a fleet's custom-name bit) as a command (deviation 395); the screen
    /// shows it until the lockstep has carried it.
    fn send_renames(&mut self) {
        if self.world.net.is_none() || self.menus.rename.is_some() {
            return;
        }
        let mut out = Vec::new();
        for n in self.world.game.local_names.iter_mut().filter(|n| !n.sent) {
            n.sent = true;
            let name = n.shown.iter().take_while(|&&c| c != 0).copied().collect();
            out.push(Command::Rename { fleet: n.fleet, index: n.index as u32, name, custom: n.custom });
        }
        for c in out {
            self.world.send(c);
        }
    }

    /// Multiplayer: the session runs on under the screens that hold a single-player game
    /// ([`crate::world::World::run_on`]).
    fn mp_run_on(&mut self) {
        let now = self.now_ms();
        self.world.run_on(&mut self.clock, now);
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
        let at = self.virtual_cursor();
        let on_info = self.overlay == Some(crate::ui::Screen::Info) && self.pointer_in;
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
                    self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game.state));
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
                        self.audio.play_music(m, false, human_race(&self.world.game.state));
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
        if self.panel_held.is_some() {
            let at = self.virtual_cursor();
            for _ in 0..due_ui {
                let acts = self.panels.hold_frame(&self.world.game.state, &sel, at);
                self.apply(acts);
            }
        }
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
        // Ours (ADR 0065): the modern HUD's bar, and its card, console and minimap (the long info keeps the original's).
        let modern_bar = self.modern_up();
        let modern_hud = modern_bar && !self.menus.info_on();
        let mut mv = None;
        let (Some(g), Some(v)) = (&mut self.gpu, &mut self.view) else { return };
        let t = now as f32 / 1000.0;
        let size = { let (w, h) = g.band(); [w as f32, h as f32] };
        let cam = v.camera.matrix();
        let offscreen = crate::prof::scope("offscreen");
        let panel_tex = match (&mut self.panel_models, self.colony.is_none() && self.overlay.is_none() && self.panels.active()) {
            (Some(m), true) => m.render(&mut g.renderer, &self.install, self.tables, &self.panels, &self.world.game.state, &sel, &cam, size),
            _ => None,
        };
        // `StarmapFrissitBolygoUrhajok` turns the globe once a drawn frame; one a UI tick here.
        let globe = match (&mut self.panel_models, self.colony.is_none() && self.overlay.is_none()) {
            (Some(m), true) => m.render_globe(&mut g.renderer, &self.install, &self.world.game.state, &sel, &cam, size, self.ui_frames as f32),
            _ => None,
        };
        if self.spacewar.is_some() {
            let Some(mut frame) = self.spacewar_frame(now, alpha, size, t, globe) else { return };
            let (Some(g), Some(h)) = (&mut self.gpu, &mut self.hud) else { return };
            h.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
            h.turbo = self.turbo;
            h.paused = paused;
            h.modern_bar = false;
            h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Starmap, t);
            // The tutorial's window over the battle too (`GFX_DateMoneyScrollFade` pass 3, every `AktKep` but 0xd).
            if let Some(tu) = &self.tutorial {
                h.draw_tutorial(&mut frame, tu, size, 11);
            }
            if let Some(top) = self.spacewar.as_mut().and_then(|s| s.over_bar.take()) {
                frame.append(top);
            }
            ov.draw(&mut frame, h, size);
            if let Ok(out) = g.surface.get_current_texture() {
                self.backdrop.last = Some((g.present(frame, &out.texture, t), t));
                g.window.pre_present_notify();
                out.present();
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
            let mut frame = spy.build(h, &d, size, self.install.lang() == ig2_formats::text::Lang::Hu);
            frame.backdrop = backdrop;
            h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Spy, t);
            if let Some(tu) = &self.tutorial {
                h.draw_tutorial(&mut frame, tu, size, 7);
            }
            ov.draw(&mut frame, h, size);
            if let Ok(out) = g.surface.get_current_texture() {
                self.backdrop.last = Some((g.present(frame, &out.texture, t), t));
                g.window.pre_present_notify();
                out.present();
            }
            return;
        }
        if let (Some(s), Some(h), Some(m)) = (self.overlay, &mut self.hud, &mut self.frame_models) {
            h.frame_tex = match s {
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
                let unit = crate::colony::landing_unit(&self.world.game.state, curr);
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
            h.modern_bar = modern_bar && screen == crate::ui::Screen::Starmap;
            match (screen, &self.colony, &self.world.colony_curr) {
                (crate::ui::Screen::Colony, Some(c), Some(curr)) => {
                    let cursor = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
                    let input = crate::ui::ColonyInput { ui: &c.ui, sim: self.world.game.rules, panel: colony_panel, cursor, pointer: false, info_model: colony_info, now_ms: now, time: t, names, rename: self.menus.rename.as_ref() };
                    h.draw_colony(&mut frame, &self.world.game.state, self.tables, self.maps, &c.view, &c.scene, curr, &input);
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
                        if !modern_hud {
                            (m.info, m.mini) = (None, None);
                        }
                        mv = Some(m);
                    }
                    h.draw_starmap_panels(&mut frame, &self.panels, &self.world.game.state, self.tables, &sel, panel_tex, size, t);
                    h.draw_starmap_menus(&mut frame, &self.menus, &self.panels, &self.world.game.state, self.tables, size, v.camera.zoom, t);
                }
                (crate::ui::Screen::Design, ..) => {
                    if let Some(d) = &mut self.design {
                        let state = &self.world.game.state;
                        let techs = state.tech();
                        let en = state.mem.i32(ig2_sim::save::va::EN).clamp(0, 18) as usize;
                        let sc = self.design_scene.get_or_insert_with(|| crate::design_scene::DesignScene::new(&mut g.renderer, &self.install));
                        sc.draw(&mut frame, &mut g.renderer, &self.install, self.tables, d, state, &techs, en, size);
                        h.draw_design(&mut frame, d, state, &techs, self.tables, &self.design_tips, size, t);
                    }
                }
                (crate::ui::Screen::Trade, ..) => {
                    if let Some(tr) = &self.trade {
                        let sc = self.trade_scene.get_or_insert_with(|| crate::trade_scene::TradeScene::new(&mut g.renderer));
                        sc.draw(&mut frame, &mut g.renderer, &self.install, self.tables, tr, size);
                        h.draw_trade(&mut frame, tr, &self.world.game.state, self.tables, &self.trade_tips, size, t);
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
            if mv.is_none() {
                ov.draw(&mut frame, h, size);
            }
        }
        drop(hud);
        // The modern HUD over the frame, the pointer over it (its acts run after the frame, `modern_apply`).
        let mut top = None;
        if let (Some(mv), Some(h)) = (&mv, &self.hud) {
            let style = crate::starmap_modern::style(h, size);
            let (st, acts, band) = (&mut self.modern, &mut self.modern_acts, g.band());
            g.gui.run(band, &style, |ctx| acts.extend(crate::starmap_modern::show(ctx, mv, st)));
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        }
        let acquire = crate::prof::scope("acquire");
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
        drop(acquire);
        let draw = crate::prof::scope("draw");
        self.backdrop.last = Some((g.present_over(frame, top, &out.texture, t), t));
        drop(draw);
        let present = crate::prof::scope("present");
        g.window.pre_present_notify();
        out.present();
        drop(present);
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
    }
}

/// The game behind the overlay screens (crate::backdrop, re/screens/overlay-background.md).
impl App {
    /// `AktKep` of what the window shows, Options and Help over the rest.
    fn shown_kep(&self) -> i32 {
        if self.options.is_some() {
            ig2_sim::tutorial::SCREEN_OPTIONS
        } else if self.help.is_some() {
            1
        } else if self.spy.is_some() && self.spacewar.is_none() {
            7
        } else {
            akt_kep(self.overlay, self.colony.is_some(), self.spacewar.is_some())
        }
    }

    /// The starmap's or the colony's scene as `GFX_Starmap` / `GFX_Colony` draw it with `KilepesElott` set (no top
    /// bar, no panels); none during a battle.
    fn game_scene(&mut self) -> Option<crate::render::Frame> {
        if self.spacewar.is_some() {
            return None;
        }
        let now = self.now_ms();
        let alpha = self.world.draw_phase(self.clock.phase(now));
        let t = now as f32 / 1000.0;
        let g = self.gpu.as_mut()?;
        match (&mut self.colony, &self.world.colony_prev, &self.world.colony_curr) {
            (Some(c), Some(prev), Some(curr)) => Some(c.view.build(&mut g.renderer, &self.install, self.tables, self.maps, &mut c.scene, prev, curr, alpha, t)),
            _ => self.view.as_mut().map(|v| v.build(&g.assets, &self.world.prev, &self.world.curr, alpha, t)),
        }
    }

    /// The screen shown changed: the pictures the screen left takes (`Starmap_Kilep` exe 0x5b2200, `Colony_Kilep`
    /// 0x466390, the overlays' `_Kilep`, `SpaceWar_Kilep` 0x562b50, `Messages_Belep` 0x507f90).
    fn backdrop_switch(&mut self) {
        use crate::backdrop::{Pic, Source};
        let kep = self.shown_kep();
        let old = std::mem::replace(&mut self.backdrop.kep, kep);
        if old == kep || old < 0 {
            return;
        }
        let Some(size) = self.gpu.as_ref().map(|g| g.band()) else { return };
        let t = self.now_ms() as f32 / 1000.0;
        // Messages, help and options draw the screen before (`SaveScreen(2)`), the others the starmap (`SaveScreen(1)`).
        let before = matches!(kep, 1 | 10 | 0xd);
        match old {
            2 if kep != 11 => {
                if before {
                    let g = self.gpu.as_mut().unwrap();
                    self.backdrop.save_last(&mut g.renderer, Pic::Before, Source::Game, size);
                }
                if let Some(f) = self.game_scene() {
                    let g = self.gpu.as_mut().unwrap();
                    self.backdrop.save(&mut g.renderer, Pic::Starmap, Source::Game, &f, size, t);
                }
            }
            3 if before => {
                if let Some(f) = self.game_scene() {
                    let g = self.gpu.as_mut().unwrap();
                    self.backdrop.save(&mut g.renderer, Pic::Before, Source::Game, &f, size, t);
                }
            }
            4..=9 if before => {
                let g = self.gpu.as_mut().unwrap();
                self.backdrop.save_last(&mut g.renderer, Pic::Before, Source::Screen, size);
            }
            11 if kep == 0xd => {
                let g = self.gpu.as_mut().unwrap();
                self.backdrop.save_last(&mut g.renderer, Pic::Before, Source::Screen, size);
            }
            _ => {}
        }
    }

    /// `Copy3hatterkep`'s grey on the screen `kep` (crate::backdrop).
    fn backdrop_grey(&self, kep: i32) -> f32 {
        use crate::backdrop as b;
        let h = self.hud.as_ref();
        match kep {
            1 => self.help.as_ref().map_or(1.0, |h| b::help(h.s.frame)),
            4 => self.research.as_ref().map_or(1.0, |r| b::research(r.frame)),
            5 => self.design.as_ref().map_or(1.0, |d| b::design(d.frame)),
            6 => self.diplomacy.as_ref().map_or(1.0, |d| b::diplomacy(d.screen.dipframe)),
            7 => self.spy.as_ref().map_or(1.0, |s| b::spy(s.screen.frame)),
            8 => self.trade.as_ref().map_or(1.0, |t| b::trade(t.frame)),
            9 => h.map_or(1.0, |h| b::info(h.info.frame)),
            10 => h.map_or(1.0, |h| b::messages(h.messages.frame)),
            0xd => self.options.as_ref().map_or(1.0, |o| b::options(o.s.fade_level())),
            _ => 1.0,
        }
    }

    /// What goes under the screen shown (`Copy3hatterkep`): its picture, or in live mode (ADR 0015) the starmap or
    /// colony drawn anew, unless the picture is of another overlay screen.
    fn backdrop(&mut self) -> Option<(crate::render::TexId, f32)> {
        let kep = self.shown_kep();
        let pic = crate::backdrop::pic_of(kep)?;
        let grey = self.backdrop_grey(kep);
        let taken = self.backdrop.pic(pic);
        if self.backdrop.live && taken.is_none_or(|(_, s)| s == crate::backdrop::Source::Game) {
            if let Some(f) = self.game_scene() {
                let t = self.now_ms() as f32 / 1000.0;
                let g = self.gpu.as_mut()?;
                let size = g.band();
                return Some((self.backdrop.draw_live(&mut g.renderer, &f, size, t), grey));
            }
        }
        taken.map(|(tex, _)| (tex, grey))
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

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        if self.gpu_wait.is_some() {
            return;
        }
        if self.gpu.is_none() {
            if let Err(e) = self.init_gpu(el) {
                eprintln!("ig2-client: {e}");
                el.exit();
            }
        }
    }

    /// The browser keeps the page after the game quits: it goes back to its start (web/main.js).
    #[cfg(target_arch = "wasm32")]
    fn exiting(&mut self, _: &ActiveEventLoop) {
        crate::web::exited();
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        if let Some(parts) = self.gpu_wait.as_ref().and_then(|w| w.borrow_mut().take()) {
            self.gpu_wait = None;
            if let Err(e) = parts.and_then(|p| self.finish_gpu(p)) {
                crate::web::failed(&e);
                _el.exit();
            }
        }
        if let Some(g) = &self.gpu {
            g.window.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        // `ig2screen::anyEvent` (exe 0x7329d0) passes a button's or a finger's release on to the game's handler
        // (`MainCallBackEventHandler` 0x4e2db0) also while a fullscreen video plays: after the video the game sees the
        // button up (a press that started the video isn't left held).
        if let (Some(v), WindowEvent::MouseInput { state: ElementState::Released, .. }) = (&mut self.video, &event) {
            v.release();
        }
        // ADR 0058: a bound key arrives as its action's default key (the screens match the original's keys).
        let mut event = event;
        if let WindowEvent::KeyboardInput { event: k, .. } = &mut event {
            if !self.rebind(k) {
                return;
            }
        }
        // The pointer is in the window on every screen (the main menu's, help's and options' arms below don't reach the
        // game's own; `frame_overlay` draws none while it is out).
        if let WindowEvent::CursorMoved { .. } | WindowEvent::CursorEntered { .. } = event {
            self.pointer_in = true;
        }
        // The help and options screens track their own pointer; the top bar's drop-downs read the band's one.
        if let (WindowEvent::CursorMoved { position, .. }, true) = (&event, self.help.is_some() || self.options.is_some()) {
            let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
            self.cursor = [position.x as f32, position.y as f32 - top];
        }
        // Ours (ADR 0062): an egui page up (the modern Options screen) takes the pointer; the keys stay the screen's.
        // The modern starmap HUD (ADR 0065) sees the pointer too; `mouse` gives it the buttons over its panels.
        // The multiplayer pages (ADR 0066) take it the same way.
        let mp_page = self.options.is_none() && self.front.as_ref().is_some_and(|f| f.mp.is_some());
        let mods_page = self.options.is_none() && self.front.as_ref().is_some_and(|f| f.mods.is_some());
        let page = mp_page || mods_page || self.options.as_ref().is_some_and(|o| o.s.modern.is_some());
        if !page && (self.modern_up() || self.gui_held != 0) && let Some(g) = &mut self.gpu {
            let k = crate::gui::Gui::scale(g.band().1);
            let top = g.layout().frame[1] as f32;
            let over = g.gui.ctx.is_pointer_over_area();
            match &event {
                WindowEvent::CursorMoved { position, .. } => g.gui.pointer(Some([position.x as f32, position.y as f32 - top]), k),
                WindowEvent::CursorLeft { .. } => g.gui.pointer(None, k),
                WindowEvent::MouseInput { state, button, .. } => {
                    let b = match button {
                        MouseButton::Left => 0,
                        MouseButton::Right => 1,
                        _ => 2,
                    };
                    // A release always reaches it, a press only over its panels.
                    if over || *state == ElementState::Released {
                        g.gui.button(b, *state == ElementState::Pressed);
                    }
                }
                WindowEvent::MouseWheel { delta, .. } if over => {
                    g.gui.wheel(match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                    });
                    return;
                }
                _ => {}
            }
        }
        if page && let Some(g) = &mut self.gpu {
            let k = crate::gui::Gui::scale(g.band().1);
            let top = g.layout().frame[1] as f32;
            match &event {
                WindowEvent::CursorMoved { position, .. } => g.gui.pointer(Some([position.x as f32, position.y as f32 - top]), k),
                WindowEvent::CursorLeft { .. } => g.gui.pointer(None, k),
                WindowEvent::MouseInput { state, button, .. } => {
                    let b = match button {
                        MouseButton::Left => 0,
                        MouseButton::Right => 1,
                        _ => 2,
                    };
                    g.gui.button(b, *state == ElementState::Pressed);
                    // The right button stays the menu's Back (ADR 0059); the multiplayer page acts on it itself, the
                    // Mods page is left by its release. A left release still reaches the menu under the multiplayer
                    // page (as before the page, which took presses only).
                    if mods_page && *button == MouseButton::Right {
                        if *state == ElementState::Released {
                            self.mods_act(crate::mods_page::Act::Back);
                        }
                        return;
                    }
                    if (*button == MouseButton::Left && !(mp_page && *state == ElementState::Released)) || (mp_page && *button != MouseButton::Left) {
                        return;
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    g.gui.wheel(match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                    });
                    return;
                }
                _ => {}
            }
        }
        if let WindowEvent::KeyboardInput { event: ref k, is_synthetic, .. } = event {
            self.research_key_seen(k);
            // ADR 0030: a W/A/S/D up ends its pan whichever screen has the keyboard (help, a video, a battle…).
            if let (ElementState::Released, Some(i)) = (k.state, do_keyboard::wasd_key(k.physical_key)) {
                self.star_keys.pan[i] = false;
                if let Some(c) = &mut self.colony {
                    c.ui.cam_keys.pan[i] = false;
                }
            }
            if !is_synthetic {
                self.modifier_key(k);
            }
        }
        if let WindowEvent::MouseInput { device_id, state, button } = event {
            if self.middle_clears(el, device_id, state, button) {
                return;
            }
        }
        match event {
            // SDL_QUIT (`MainCallBackEventHandler`): `CloseBink` if a video plays, then the game quits.
            WindowEvent::CloseRequested => {
                self.video = None;
                el.exit();
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.reconfigure(),
            WindowEvent::RedrawRequested => {
                if let Some(due) = self.tour.as_mut().and_then(|t| t.due()) {
                    match due {
                        Some(k) => self.tour_key(&k),
                        None => el.exit(),
                    }
                }
                self.frame();
                crate::prof::frame(self.profile_screen());
                if self.front.as_ref().is_some_and(|f| f.quit) {
                    el.exit();
                }
            }
            // A fullscreen video takes the keys and buttons (`ig2screen::anyEvent`: a press arms it, the release
            // after it skips); the game under it waits.
            WindowEvent::KeyboardInput { event, .. } if self.video.is_some() => {
                if let Some(v) = &mut self.video {
                    if event.state == ElementState::Pressed {
                        v.press();
                    } else {
                        v.release();
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } if self.video.is_some() => {
                if let Some(v) = &mut self.video {
                    v.press();
                }
            }
            // The wait overlay's own frame loop (`MOUSE_DrawOver`): nothing under it takes the buttons (its items take a
            // held button each frame, `update_mp_wait`); the keys stay live (its `Fake_FRISSITES` runs `DoKeyboard`).
            WindowEvent::MouseInput { state, button, .. } if self.mp_wait.is_open() => {
                let bit = match button {
                    MouseButton::Left => 1,
                    MouseButton::Right => 2,
                    _ => 0,
                };
                self.mouse_held = if state == ElementState::Pressed { self.mouse_held | bit } else { self.mouse_held & !bit };
            }
            // The top bar's drop-downs take the buttons first on every screen with the bar (`MOUSE_SelectScreens`).
            WindowEvent::MouseInput { state, button, .. }
                if (self.help.is_some() || self.options.is_some() || self.spacewar.is_some()) && !self.tutorial_owns_input() && self.top_menus_button(button, state == ElementState::Pressed) => {}
            WindowEvent::CursorMoved { position, .. } if self.help.is_some() => {
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let (bw, bh) = self.gpu.as_ref().map_or((640, 480), |g| g.band());
                let at = crate::font::Virtual::new([bw as f32, bh as f32]).virtual_of([position.x as f32, position.y as f32 - top]);
                if let Some(h) = &mut self.help {
                    h.s.hover(at);
                }
            }
            // The top bar stays live over Help (its spots are registered once, help.md §5).
            WindowEvent::MouseInput { state, button, .. } if self.help.is_some() && { let at = self.help.as_ref().map_or([0.0; 2], |h| h.s.at); self.top_bar_mouse(button, state == ElementState::Pressed, at) } => {}
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.help.is_some() => {
                if let Some(h) = &mut self.help {
                    if state == ElementState::Pressed {
                        let at = h.s.at;
                        h.s.press(at);
                    } else {
                        h.s.release();
                    }
                }
            }
            WindowEvent::MouseWheel { .. } | WindowEvent::MouseInput { .. } if self.help.is_some() => {}
            // `remapKeyCodes` (exe 0x4e4670): the key downs the exe never queues reach nothing (the key ups go on); a
            // numpad digit's character still types into an entry up (its SDL_TEXTINPUT). Ours: the main menu's modern
            // Options screen (ADR 0053), the multiplayer pages (ADR 0066) and the Mods page (ADR 0068) take them all (Tab,
            // Delete, Home, End).
            WindowEvent::KeyboardInput { ref event, .. } if event.state == ElementState::Pressed && !do_keyboard::exe_queues_key_down(event)
                && !(do_keyboard::numpad_digit(event) && self.entry_up()) && !self.options.as_ref().is_some_and(|o| o.s.modern.is_some())
                && !(self.options.is_none() && self.front.as_ref().is_some_and(|f| f.mp.is_some() || f.mods.is_some())) => {}
            // Esc is the exit hot-spot (`MainCallBackEventHandler` on `AktKep` 1); F1 does nothing there. Help has no
            // keyboard handler of its own, but `DoKeyboard` stays live (exe 0x4db1a0): F2..F10, O and M leave it for
            // their screen (`AktKep` changes, `Help_Kilep`), Space, P and 1/2/3 pause and set the speed.
            WindowEvent::KeyboardInput { event, .. } if self.help.is_some() => {
                if event.state != ElementState::Pressed {
                    return;
                }
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        if let Some(h) = &mut self.help {
                            h.s.escape();
                        }
                    }
                    // F-keys: `DoKeyboard`'s screen change (Help goes at once, `Help_Kilep`).
                    k @ Key::Named(_) if do_keyboard::f_key_screen(&k).is_some() => {
                        if self.key(k) {
                            el.exit();
                        }
                    }
                    // O: `Help_Kilep` closes the screen first when it opened animated, then Options enters over the
                    // screen before Help (`OptionsElottiKep = HelpElottiScreen`).
                    Key::Character("o" | "O") if self.o_key_allowed() => {
                        if let Some(h) = &mut self.help {
                            h.s.escape();
                            self.help_then_options = true;
                        }
                    }
                    Key::Character("o" | "O") => {}
                    k @ Key::Character("m" | "M") => {
                        let again = self.help_under_is(&k);
                        self.leave_help();
                        if !again && self.key(k) {
                            el.exit();
                        }
                    }
                    k @ (Key::Named(NamedKey::Space) | Key::Character("p" | "P" | "1" | "2" | "3")) => {
                        if self.key(k) {
                            el.exit();
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } if self.options.is_some() => {
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let (bw, bh) = self.gpu.as_ref().map_or((640, 480), |g| g.band());
                let at = crate::font::Virtual::new([bw as f32, bh as f32]).virtual_of([position.x as f32, position.y as f32 - top]);
                if let Some(f) = &mut self.front {
                    f.at = at;
                }
                if let Some(o) = &mut self.options {
                    o.s.hover(at);
                }
            }
            // Ours (ADR 0059): over the main menu the right button is Esc (Load, Settings).
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } if self.options.is_some() && self.front.as_ref().is_some_and(|f| f.s.flow) => {
                let audio = &mut self.audio;
                if let Some(o) = &mut self.options {
                    if o.s.modern.is_some() {
                        o.s.modern_key(crate::options::modern::Nav::Back, &mut |id| audio.play(id));
                    } else if o.s.entry.is_none() {
                        o.s.escape();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } if self.options.as_ref().is_some_and(|o| o.s.modern.is_none()) && { let at = self.options.as_ref().map_or([0.0; 2], |o| o.s.at); self.top_bar_mouse(button, state == ElementState::Pressed, at) } => {}
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.options.is_some() => {
                let audio = &mut self.audio;
                if let Some(o) = &mut self.options {
                    if state == ElementState::Pressed {
                        let at = o.s.at;
                        o.s.press(at, &mut |id| audio.play(id));
                    } else {
                        o.s.release();
                    }
                }
            }
            WindowEvent::MouseWheel { .. } | WindowEvent::MouseInput { .. } if self.options.is_some() => {}
            // `DoKeyboard` on the options screen: the F-keys (also in the name entry) and M ask for their screen.
            WindowEvent::KeyboardInput { event, .. } if self.options.is_some() => {
                // Ours (ADR 0053): a held Esc, Enter, Space or O acts once on the modern screen.
                let once = event.repeat && self.options.as_ref().is_some_and(|o| o.s.modern.is_some())
                    && matches!(event.logical_key.as_ref(), Key::Named(NamedKey::Escape | NamedKey::Enter | NamedKey::Space) | Key::Character("o" | "O"));
                // ADR 0058: a key pressed for an action waiting for one is its new key (Esc gives up).
                let esc = matches!(event.logical_key.as_ref(), Key::Named(NamedKey::Escape));
                if let (ElementState::Pressed, false, Some(o), winit::keyboard::PhysicalKey::Code(code)) = (event.state, esc, &mut self.options, event.physical_key) {
                    if o.s.capturing() {
                        let audio = &mut self.audio;
                        o.s.modern_bind(code, &mut |id| audio.play(id));
                        return;
                    }
                }
                if event.state == ElementState::Pressed && !once {
                    let k = event.logical_key.as_ref();
                    let entry = self.options.as_ref().is_some_and(|o| o.s.typing());
                    if let Some(n) = do_keyboard::f_key_screen(&k) {
                        self.f_key(n);
                    } else if !(matches!(k, Key::Character("m" | "M")) && !entry && self.m_key()) {
                        self.options_key(&event.logical_key);
                    }
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
                // A held Esc's repeats would close the statistics screen and go on to the menu's own Esc; ours: a held Esc
                // or Enter that closed the Options screen (ADR 0053) goes no further.
                let closer = matches!(event.logical_key.as_ref(), Key::Named(NamedKey::Escape | NamedKey::Enter));
                if event.state == ElementState::Pressed && !(event.repeat && (closer || self.front.as_ref().is_some_and(|f| f.stats.is_some() || f.replays.is_some() || f.mods.is_some()))) {
                    self.front_key(&event.logical_key);
                }
            }
            // Ours (ADR 0034): the statistics screen takes either button, acting on the release so the menu under it
            // gets neither half of the click.
            WindowEvent::MouseInput { state, button: button @ (MouseButton::Left | MouseButton::Right), .. } if self.front.as_ref().is_some_and(|f| f.stats.is_some()) => {
                let Some(f) = &mut self.front else { return };
                if state == ElementState::Released {
                    if std::mem::take(&mut f.opening) {
                        return;
                    }
                    let at = f.at;
                    // Ours (ADR 0059): the right button on empty space is Esc.
                    if button == MouseButton::Right && f.s.flow && f.stats.as_ref().is_some_and(|st| !st.on_spot(at)) {
                        if f.stats.as_mut().and_then(|st| st.back()).is_some() {
                            f.stats = None;
                        }
                        return;
                    }
                    if f.stats.as_mut().and_then(|st| st.press(at)).is_some() {
                        f.stats = None;
                        self.audio.play(crate::audio::CLICK);
                    }
                }
            }
            // Ours (ADR 0038 §6): the replays list, as the statistics screen (either button, on the release).
            WindowEvent::MouseInput { state, button: button @ (MouseButton::Left | MouseButton::Right), .. } if self.front.as_ref().is_some_and(|f| f.replays.is_some()) => {
                let Some(f) = &mut self.front else { return };
                if state == ElementState::Released {
                    if std::mem::take(&mut f.opening) {
                        return;
                    }
                    let at = f.at;
                    if button == MouseButton::Right && f.s.flow && f.replays.as_ref().is_some_and(|rl| !rl.on_spot(at)) {
                        f.replays = None;
                        return;
                    }
                    match f.replays.as_mut().and_then(|rl| rl.press(at)) {
                        Some(crate::replay_viewer::ListAct::Close) => f.replays = None,
                        Some(crate::replay_viewer::ListAct::Open(path)) => self.watch_replay(&path),
                        None => return,
                    }
                    self.audio.play(crate::audio::CLICK);
                }
            }
            // The main menu's "Replays" item (ours, ADR 0038 §6).
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.front.as_ref().is_some_and(|f| front_history_shown(f) && crate::kit::inside_f(crate::replay_viewer::REPLAYS_ITEM, f.at)) =>
            {
                self.open_replays();
            }
            // The replay viewer's bar takes the presses on it (ours, ADR 0038 §6).
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.replay_bar_up() && crate::replay_viewer::Bar::covers(self.virtual_cursor()) => {
                if state == ElementState::Pressed {
                    if let Some(a) = crate::replay_viewer::Bar::press(self.virtual_cursor()) {
                        self.audio.play(crate::audio::CLICK);
                        self.replay_act(a);
                    }
                }
            }
            // The main menu's "History" item (ours, ADR 0034).
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.display.statistics && self.front.as_ref().is_some_and(|f| front_history_shown(f) && crate::kit::inside_f(crate::stats_screen::HISTORY_ITEM, f.at)) =>
            {
                self.open_history();
            }
            // The main menu's boxes, race spheres and difficulty rings take either button (`Mouse_Lenyom`, FX3).
            WindowEvent::MouseInput { state, button: MouseButton::Right, .. } if self.front.is_some() => {
                let Some(f) = &mut self.front else { return };
                let at = f.at;
                if state == ElementState::Pressed {
                    if let Some(a) = f.s.press_right(at) {
                        self.front_act(a);
                    }
                } else {
                    f.s.release_right();
                }
            }
            // The wheel is no event of `MainCallBackEventHandler`'s (exe 0x4e2db0); the middle button drops the held ones.
            WindowEvent::MouseWheel { .. } if self.front.is_some() => {}
            WindowEvent::MouseInput { button: MouseButton::Middle, .. } if self.front.as_ref().is_some_and(|f| f.stats.is_none()) => {
                if let Some(f) = &mut self.front {
                    f.s.middle();
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.front.is_some() => {
                let Some(f) = &mut self.front else { return };
                let at = f.at;
                let act = if state == ElementState::Pressed { f.s.press(at) } else { f.s.release(at) };
                if let Some(a) = act {
                    self.front_act(a);
                }
            }
            // While the script plays (`MainCallBackEventHandler`): the live pointer and buttons are dropped; `t` shows or
            // hides the window, `o` opens the options screen.
            // The OS pointer is still tracked (`MainCallBackEventHandler` stores the raw position first).
            WindowEvent::CursorMoved { position, .. } if self.tutorial_owns_input() => {
                self.cursor_px = [position.x as i32, position.y as i32];
                self.pointer_seen = true;
                self.pointer_in = true;
            }
            WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } | WindowEvent::ModifiersChanged(_) if self.tutorial_owns_input() => {}
            WindowEvent::KeyboardInput { event, .. } if self.tutorial_owns_input() => {
                if event.state == ElementState::Pressed {
                    match event.logical_key.as_ref() {
                        // Esc keeps its live path (not tutorial-gated in the exe).
                        Key::Named(NamedKey::Escape) => {
                            if self.key(event.logical_key.as_ref()) {
                                el.exit();
                            }
                        }
                        Key::Character("t" | "T") if !self.entry_up() => {
                            if let Some(t) = &mut self.tutorial {
                                t.t.gates.text_shown ^= true;
                            }
                        }
                        Key::Character("o" | "O") if self.tutorial_o_allowed() => self.ask_options(),
                        _ => {}
                    }
                }
            }
            // Paused (live input): `t` still shows or hides the window and `o` opens the options screen, then the key goes
            // on to the screen.
            WindowEvent::KeyboardInput { ref event, .. }
                if event.state == ElementState::Pressed && matches!(event.logical_key.as_ref(), Key::Character("t" | "T" | "o" | "O")) && self.tutorial.as_ref().is_some_and(|t| t.running()) && self.options.is_none() && !self.entry_up() =>
            {
                if matches!(event.logical_key.as_ref(), Key::Character("o" | "O")) {
                    if self.tutorial_o_allowed() {
                        self.ask_options();
                    }
                    return;
                }
                if let Some(t) = &mut self.tutorial {
                    t.t.gates.text_shown ^= true;
                }
                if self.spacewar.is_some() {
                    self.spacewar_key(event);
                } else if !self.wasd_key(event) && !self.colony_camera_key(event) && !self.starmap_key(event) && self.key(event.logical_key.as_ref()) {
                    el.exit();
                }
            }
            // A modifier is set by its own key's down (`modifier_key`), as the exe's are; the OS state only clears.
            WindowEvent::ModifiersChanged(m) => self.modifiers &= m.state(),
            WindowEvent::CursorLeft { .. } => self.pointer_in = false,
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_px = [position.x as i32, position.y as i32];
                self.pointer_seen = true;
                self.pointer_in = true;
                // The band's pixels: the mouse goes through the frame (`IG2_VIRTUAL_POSY`, re/screens/widescreen.md §4).
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let p = [position.x as f32, position.y as f32 - top];
                self.cursor_moved(p);
            }
            WindowEvent::MouseWheel { .. } if self.spacewar.is_some() => {}
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 60.0,
                };
                self.wheel(steps);
            }
            WindowEvent::MouseInput { state, button, .. } if self.spacewar.is_some() => {
                let at = self.virtual_cursor();
                let down = state == ElementState::Pressed;
                let mut first = false;
                if let Some(sw) = &mut self.spacewar {
                    // `IG2_gameUpdate` (exe 0x4e0440) acts on the press edge of "any button held" (`Mouse_Valtozas`
                    // 0xDB4653 with 0xDB4650): a second button pressed while one is held changes nothing.
                    first = !(sw.left || sw.right);
                    match button {
                        MouseButton::Left => sw.left = down,
                        MouseButton::Right => sw.right = down,
                        _ => {}
                    }
                }
                // The top bar's plain spots as on every screen: pause and the speeds on either button's press, the
                // message line (7) with its battle gate (`DEBUGX`) and the right button's history.
                if !down || first {
                    self.top_bar_mouse(button, down, at);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if self.spacewar.is_some() => self.spacewar_key(&event),
            WindowEvent::KeyboardInput { event, .. } if self.fleet_group_key(&event) || self.wasd_key(&event) || self.colony_camera_key(&event) || self.trade_key(&event) || self.starmap_key(&event) => {}
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
    /// The screen up, for the `IG2_PROFILE` log.
    fn profile_screen(&self) -> &'static str {
        use crate::ui::Screen as S;
        match self.overlay {
            _ if self.video.is_some() => "video",
            _ if self.front.is_some() => "mainmenu",
            _ if self.options.is_some() => "options",
            _ if self.help.is_some() => "help",
            _ if self.spacewar.is_some() => "spacewar",
            _ if self.spy.is_some() => "spy",
            Some(S::Info) => "info",
            Some(S::Messages) => "messages",
            Some(S::Research) => "research",
            Some(S::Design) => "design",
            Some(S::Trade) => "trade",
            Some(S::Diplomacy) => "diplomacy",
            _ if self.colony.is_some() && self.world.colony_curr.as_ref().is_some_and(|s| s.at_war) => "groundwar",
            _ if self.colony.is_some() => "colony",
            _ => "starmap",
        }
    }

    /// One of `IG2_PROFILE_TOUR`'s keys, as if pressed.
    fn tour_key(&mut self, k: &str) {
        eprintln!("[prof] tour key {k}");
        if let Some(v) = &mut self.video {
            v.press();
            v.release();
            return;
        }
        let named = match k {
            "vsync" => {
                self.vsync = !self.vsync;
                self.reconfigure();
                return;
            }
            "Esc" => NamedKey::Escape,
            "Enter" => NamedKey::Enter,
            "Tab" => NamedKey::Tab,
            "Space" => NamedKey::Space,
            "F1" => NamedKey::F1,
            "F2" => NamedKey::F2,
            "F3" => NamedKey::F3,
            "F4" => NamedKey::F4,
            "F5" => NamedKey::F5,
            "F6" => NamedKey::F6,
            "F7" => NamedKey::F7,
            "F8" => NamedKey::F8,
            "F9" => NamedKey::F9,
            "F10" => NamedKey::F10,
            c => {
                self.key(Key::Character(c));
                return;
            }
        };
        self.key(Key::Named(named));
    }

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
                    Some(crate::ui::Screen::Messages) => {
                        h.messages.hover(at);
                        h.messages.log.hover_at(at, &self.world.log);
                    }
                    _ => {}
                }
            }
            if let Some(d) = &mut self.design {
                d.moved(at);
            }
            if let Some(tr) = &mut self.trade {
                tr.moved(at);
            }
        }
        if self.colony.is_some() {
            self.cursor = p;
            self.colony_pointer(true);
        }
        if let Some(c) = self.colony.as_mut().filter(|_| self.overlay.is_none() && self.spy.is_none()) {
            // `ShowColony`: both buttons held turn and tilt the camera (colony.md §2.1).
            if self.mouse_held == 3 && !c.ui.picking() {
                let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(p);
                c.ui.cam_keys.mouse_turn(&mut c.view.camera, at);
            }
        }
        self.cursor = p;
        self.top_menus_frame();
    }

    /// `ShowColony`: the cell and the building under the pointer (`AktBGridX/Y`, the model pick), on every pointer move
    /// and again every drawn frame (the camera or the buildings may have moved under a still pointer); the hold's
    /// `Colony_Main` pass (`drag`) on a move and every frame a button is held. `Mouse_Jobb` is read live.
    fn colony_pointer(&mut self, moved: bool) {
        let p = self.cursor;
        let (held, right) = (moved || self.mouse_held != 0, self.mouse_held & 2 != 0);
        let gm = self.colony.as_ref().zip(self.world.colony_curr.as_ref()).and_then(|(c, s)| self.maps.get(s.map).map(|gm| (c, gm)));
        let gm_cell = gm.and_then(|(c, gm)| crate::colony_ui::cell_under(&c.view, gm, p));
        let start = gm.and_then(|(c, gm)| crate::colony_ui::cursor_cell_under(&c.view, gm, p));
        let visible = self.world.colony_curr.as_ref().is_some_and(|s| s.visible);
        let under = gm.and_then(|(c, gm)| c.view.building_at(gm, &c.scene, visible, p));
        // The pointer's cell and building first: the hold's building rule reads them.
        self.colony_input(|ui, ctx, at, _| {
            ui.war.under = under;
            ui.war.right_held = right;
            ui.hover(ctx, gm_cell, start);
            if held {
                ui.drag(ctx, at);
            }
            Default::default()
        });
    }

    /// A 2D screen (`overlay`) or the spy screen is up over the starmap or the colony.
    fn under_screen(&self) -> bool {
        self.overlay.is_some() || self.spy.is_some()
    }

    /// The wheel turned by `steps` notches.
    fn wheel(&mut self, steps: f32) {
        if let Some(h) = self.hud.as_mut().filter(|h| self.overlay == Some(crate::ui::Screen::Messages) && h.messages.log.open) {
            h.messages.log.scroll_by((-steps * 3.0).round() as i32, &self.world.log);
            return;
        }
        // None of the 2D screens (`Design_Main`, `Trade_Main`, `Research_Main`, …, `Spy_Main`) has a wheel.
        if self.under_screen() {
            return;
        }
        if let Some(c) = &mut self.colony {
            c.view.camera.zoom_steps(steps);
        } else if let (Some(v), false) = (&mut self.view, !self.display.starmap_mouse || self.menus.info_on() || self.cam_saved.is_some() || self.panels.fleet_info()) {
            v.camera.zoom_steps(steps);
        }
    }

    /// A mouse button went down or up.
    /// `KeyShift`/`KeyCtrl`/`KeyAlt` (0xbae295/0xbae27f/0xbae294) are set on every KEYDOWN of their key, an OS
    /// repeat too, and cleared on its KEYUP, either side's (`MainCallBackEventHandler` exe 0x4e2db0 reads no mod state
    /// and filters no repeats; `remapKeyCodes` 0x4e4670 maps both keys of a pair to one VK; not while the tutorial
    /// holds the input, unless `AktKep` 0xd): after a middle click dropped them, the next repeat of a held modifier
    /// brings it back. AltGr (SDL's RALT) is Alt; the focus gain's synthetic presses aren't key-downs.
    fn modifier_key(&mut self, k: &winit::event::KeyEvent) {
        if self.tutorial_owns_input() {
            return;
        }
        let m = match k.logical_key.as_ref() {
            Key::Named(NamedKey::Shift) => ModifiersState::SHIFT,
            Key::Named(NamedKey::Control) => ModifiersState::CONTROL,
            Key::Named(NamedKey::Alt | NamedKey::AltGraph) => ModifiersState::ALT,
            _ => return,
        };
        self.modifiers.set(m, k.state == ElementState::Pressed);
    }

    /// Row 379: `MainCallBackEventHandler` sends a press or release of any button but 1 and 3 to the block at exe
    /// 0x4e345a, which with `IsActive` (0x912a38) set zeroes `KeyShift`/`KeyCtrl`/`KeyAlt` and the held bytes
    /// 0xdb467d..f: the next `Mouse_Lekerdez` (exe 0x5105d0) sees every held button released (`Mouse_Felenged` and the
    /// per-button edges), and the real release later is no edge. So: the modifiers drop, each held button gets its
    /// release now, and its real release is swallowed. True: the event is swallowed.
    fn middle_clears(&mut self, el: &ActiveEventLoop, device_id: winit::event::DeviceId, state: ElementState, button: MouseButton) -> bool {
        let bit = match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            _ => 0,
        };
        if bit != 0 {
            if state == ElementState::Pressed {
                self.buttons_down |= bit;
                self.released_early &= !bit;
                return false;
            }
            self.buttons_down &= !bit;
            let early = self.released_early & bit != 0;
            self.released_early &= !bit;
            return early;
        }
        if !self.gpu.as_ref().is_some_and(|g| g.window.has_focus()) {
            return false;
        }
        self.modifiers = ModifiersState::empty();
        for (b, button) in [(1, MouseButton::Left), (2, MouseButton::Right)] {
            if self.buttons_down & b != 0 {
                self.window_event(el, WindowId::dummy(), WindowEvent::MouseInput { device_id, state: ElementState::Released, button });
                self.released_early |= b;
            }
        }
        false
    }

    fn mouse(&mut self, button: MouseButton, pressed: bool) {
        let state = if pressed { ElementState::Pressed } else { ElementState::Released };
        let bit = match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            _ => 0,
        };
        self.mouse_held = if pressed { self.mouse_held | bit } else { self.mouse_held & !bit };
        if let Some(c) = &mut self.colony {
            c.ui.war.right_held = self.mouse_held & 2 != 0;
        }
        // `Mouse_FelengedesMajdMegvar`: `IG2_gameUpdate` (exe 0x4e0440) runs no screen until every button is up, so
        // nothing (the top bar included) takes the presses and releases meanwhile.
        if self.wait_release {
            if self.mouse_held == 0 {
                self.wait_release = false;
                if let Some(c) = &mut self.colony {
                    c.ui.cam_keys.mouse_release();
                }
            }
            return;
        }
        // Ours (ADR 0065): a press over the modern HUD's panels is its own, and so is that button's release; both
        // buttons there over the info card start the rename, as on the original's info box.
        if bit != 0 && (self.gui_held & bit != 0 || (pressed && self.over_gui())) {
            if pressed {
                self.gui_held |= bit;
                if self.mouse_held == 3 && self.over_info_card() && !self.starmap_locked() {
                    self.wait_release = true;
                    self.gui_held = 0;
                    self.start_rename();
                }
            } else {
                self.gui_held &= !bit;
            }
            return;
        }
        let modern = self.modern_up();
        if !modern && self.top_menus_button(button, pressed) {
            return;
        }
        // The top bar's spots win over every screen's (`top_bar`).
        let at = self.virtual_cursor();
        if !modern && self.top_bar_mouse(button, pressed, at) {
            return;
        }
        if self.spy.is_some() && (bit == 0 || self.mouse_held & !bit != 0) {
            // `Spy_Main` (exe 0x58d410) tests only `Mouse_Lenyom`/`Felenged`, either button's edges: the first button
            // down presses, the last one up releases (`Mouse_Lekerdez` exe 0x510694..0x510717 takes both edges from
            // `Mouse_Gomb`, any button: a second button's edge while one is held is none); nothing reaches the
            // starmap under it.
            self.right_down = None;
            return;
        }
        if self.spy.is_some() {
            let Some(at) = self.spy_virtual(self.cursor) else { return };
            let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
            let Some(spy) = &mut self.spy else { return };
            let orders = if state == ElementState::Pressed { spy.press(&d, at) } else { spy.release(&d) };
            let errors = spy.take_errors();
            for c in orders {
                self.world.send(c);
            }
            if let Some(e) = errors.into_iter().last() {
                self.scroll_error(e);
            }
            return;
        }
        let on_starmap = self.colony.is_none() && self.overlay.is_none();
        // `ShowColony`'s both-button turn: the first press keeps the pointer and the camera, any release lets go.
        if let Some(c) = self.colony.as_mut().filter(|_| self.overlay.is_none() && self.spy.is_none() && bit != 0) {
            if !pressed {
                c.ui.cam_keys.mouse_release();
            } else if !c.ui.picking() {
                let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
                c.ui.cam_keys.mouse_press(&c.view.camera, at);
            }
        }
        // `Colony_Main` hot-spot 4 (exe 0x4667fb): both buttons held over the short info with `ColonyInfoMod` 0 start
        // the planet's name entry (`IOAzonosito` 0x1f; any planet, no sound) and wait for the release.
        if self.colony.is_some() && !self.under_screen() && pressed && self.mouse_held == 3 && self.colony_rename_spot() {
            self.wait_release = true;
            self.start_colony_rename();
            return;
        }
        let moved = |d: Option<[f32; 2]>, p: [f32; 2]| d.is_none_or(|d| (d[0] - p[0]).abs() + (d[1] - p[1]).abs() > 4.0);
        if on_starmap && pressed && self.spacewar.is_none() {
            let at = self.virtual_cursor();
            let on_box = at[0] >= 0.0 && at[0] < 166.0 && at[1] >= 424.0 && !self.menus.info_on() && !modern;
            // `Starmap_Main`: a press ends the rename, the old name back unless it is on the info box.
            if let Some(r) = self.menus.rename.take() {
                if !on_box {
                    self.menus.rename = Some(r.clone());
                    self.write_rename(&r.backup);
                    self.menus.rename = None;
                }
            }
            if on_box && self.mouse_held == 3 {
                // `Mouse_FelengedesMajdMegvar`: the releases don't reach the info box (the long info, the colony).
                self.left_down = None;
                self.right_down = None;
                self.wait_release = true;
                self.start_rename();
                return;
            }
        }
        match (button, state) {
            (MouseButton::Left | MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Design) => {
                let at = self.virtual_cursor();
                if button == MouseButton::Left {
                    self.design_do(|d, c, mask| d.press(c, mask, at));
                } else if let Some(d) = &mut self.design {
                    d.press_right(at);
                }
            }
            (MouseButton::Left | MouseButton::Right, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Design) => {
                if button == MouseButton::Left {
                    self.design_do(|d, c, _| d.release(c));
                } else if let Some(d) = &mut self.design {
                    d.release_right();
                }
            }
            (MouseButton::Left | MouseButton::Right, _) if self.overlay == Some(crate::ui::Screen::Trade) => {
                let at = self.virtual_cursor();
                if let Some(tr) = &mut self.trade {
                    if pressed {
                        tr.press(button == MouseButton::Left, at);
                    } else {
                        tr.release(button == MouseButton::Left, at);
                    }
                }
            }
            // `Research_Main` (exe 0x523c10) acts on either button's press (re/screens/input.md §7).
            (MouseButton::Left | MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Research) => self.research_press(button == MouseButton::Left),
            // The buttons for the drags and the faders (`Mouse_Lekerdez`).
            (MouseButton::Left | MouseButton::Right, _) if self.overlay == Some(crate::ui::Screen::Research) => {
                if let Some(r) = &mut self.research {
                    let bit = if button == MouseButton::Left { 1 } else { 2 };
                    r.buttons = if pressed { r.buttons | bit } else { r.buttons & !bit };
                    if button == MouseButton::Left {
                        r.held = pressed;
                    }
                }
            }
            (MouseButton::Left, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Diplomacy) => {
                self.dip_input.pressed = true;
                self.dip_input.held = true;
            }
            (MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Diplomacy) => self.dip_input.right_pressed = true,
            (MouseButton::Left, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Diplomacy) => {
                self.dip_input.released = true;
                self.dip_input.held = false;
            }
            (MouseButton::Left | MouseButton::Right, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Info) => self.info_button(false, button == MouseButton::Right),
            (MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Info) => {
                self.info_right_press();
                self.info_button(true, true);
            }
            // `Messages_Main`: `Mouse_Lenyom` is either button's (FX3). The right one also sets `SkipAll` (0xd90dc4), which only
            // `MessagesGfx2D_2`'s `PlayBack` pause reads, and `PlayBack` is never set: nothing more to do (audio.md §5).
            (MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Messages) => self.screen_press(crate::ui::Screen::Messages),
            (MouseButton::Left, ElementState::Pressed) if self.overlay.is_some() => {
                let s = self.overlay.unwrap();
                self.screen_press(s);
            }
            // The other 2D screens' right button doesn't reach the starmap or colony under them (no order).
            (MouseButton::Right, _) if self.overlay.is_some() => self.right_down = None,
            (MouseButton::Left, ElementState::Pressed) if self.colony.is_some() => self.colony_input(|ui, ctx, at, now| ui.press(ctx, at, true, now)),
            (MouseButton::Left, ElementState::Released) if self.colony.is_some() => {
                self.colony_input(|ui, ctx, at, now| ui.release(ctx, at, true, now));
            }
            // A second button's press is no new press on the starmap (`Mouse_Lenyom` is the first button down only,
            // `Mouse_Lekerdez` exe 0x5105d0): over a held console spot it is the spot's; after a press on the
            // map, which latches hot-spot 0x35 (`Mouse_Megfogva`, `Starmap_Main` exe 0x5b54a2), no console spot, pick or
            // order starts.
            (MouseButton::Left | MouseButton::Right, ElementState::Pressed) if on_starmap && (self.console_held.is_some() || self.mouse_held & !bit & 3 != 0) => {}
            (MouseButton::Left, ElementState::Pressed) => {
                self.menus.held = true;
                if !(on_starmap && self.starmap_press()) {
                    self.left_down = Some(self.cursor);
                }
            }
            // A held console spot stays latched (`Mouse_Megfogva`) until every button is up (`Mouse_Felenged`,
            // `Mouse_Lekerdez` exe 0x5105d0); a second button's press there is taken by it.
            (MouseButton::Left | MouseButton::Right, ElementState::Released) if self.console_held.is_some() => {
                if self.mouse_held & 3 == 0 {
                    self.menus.held = false;
                    self.console_release();
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
                let down = self.left_down.take();
                if on_starmap && !moved(down, self.cursor) {
                    self.starmap_click(self.cursor, false);
                }
            }
            (MouseButton::Right, ElementState::Pressed) if on_starmap && self.console_held.is_none() && self.console_press(false) => {}
            (MouseButton::Right, ElementState::Pressed) if on_starmap && self.panel_press(false) => self.panel_held = Some(false),
            (MouseButton::Right, ElementState::Pressed) if self.colony.is_some() => self.colony_input(|ui, ctx, at, now| ui.press(ctx, at, false, now)),
            // `Starmap_Main` gives the map's right-button orders (0x15, the rally 0x400) on the press, not under the
            // radial menu (ELF 0x6f50a0 lines 731-732; a press with the left button held is no press, above); the right
            // button never drags the camera (ADR 0049 keeps it so: a pan there moved the map under rapid orders). The
            // info box's right action (colony / split) is the release's.
            (MouseButton::Right, ElementState::Pressed) => {
                self.right_down = Some(self.cursor);
                let gated = self.menus.rop.is_some();
                if on_starmap && !gated && !self.on_info_box(self.cursor) {
                    self.starmap_click(self.cursor, true);
                }
            }
            (MouseButton::Right, ElementState::Released) if self.panel_held.is_some() => {
                self.panel_held = None;
                let sel = self.selection();
                let acts = self.panels.release(&self.world.game.state, &sel);
                self.apply(acts);
            }
            // `Colony_Main` takes any right release (`Mouse_Felenged`), moved or not.
            (MouseButton::Right, ElementState::Released) if self.colony.is_some() => self.colony_input(|ui, ctx, at, now| ui.release(ctx, at, false, now)),
            // The exe tests only the pointer at the release (`AktIkon2 == 8`, lines 1309, 1455), wherever the press was.
            (MouseButton::Right, ElementState::Released) => {
                self.right_down = None;
                if on_starmap && self.on_info_box(self.cursor) {
                    self.starmap_click(self.cursor, true);
                }
            }
            _ => {}
        }
    }

    /// ADR 0030: W/A/S/D held pan the starmap's camera (and with the option's third state the colony's) through the
    /// edge scroll; a key down taken here never reaches the screen (the colony's `a` and `s`). The key up is handled
    /// before the screens (`window_event`). True when taken.
    fn wasd_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        let Some(i) = do_keyboard::wasd_key(event.physical_key) else { return false };
        if event.state != ElementState::Pressed {
            return false;
        }
        let shown = self.spacewar.is_none() && !self.under_screen() && self.options.is_none();
        let starmap = shown && self.view.is_some() && self.colony.is_none();
        let colony = shown && self.colony.is_some();
        if !do_keyboard::wasd_pans(self.display.wasd, starmap, colony, self.entry_up()) {
            return false;
        }
        match &mut self.colony {
            Some(c) => c.ui.cam_keys.pan[i] = true,
            None => self.star_keys.pan[i] = true,
        }
        true
    }

    /// ADR 0035: on the starmap Ctrl+1..9 binds the selected own fleet to the group, Alt+1..9 selects the group's fleet
    /// and, when it is already selected, centres the camera on it. Taken (never reaching `DoKeyboard`'s speed keys) only
    /// with the option on and the starmap free, as PgUp/PgDn's step (re/screens/starmap.md §10.6). True when taken.
    fn fleet_group_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        use crate::fleet_groups::GroupKey;
        use crate::starmap_menus::InfoPhase;
        let m = self.modifiers;
        let Some(k) = crate::fleet_groups::group_key(event.physical_key, m.control_key(), m.alt_key()) else { return false };
        if !self.display.fleet_groups || self.view.is_none() || self.colony.is_some() || self.spacewar.is_some() || self.under_screen()
            || self.options.is_some() || self.entry_up() || self.starmap_locked()
        {
            return false;
        }
        if event.state != ElementState::Pressed {
            return true;
        }
        let blocked = self.menus.menu.is_some() || self.menus.info.is_some_and(|i| i.phase != InfoPhase::Up) || self.panels.fleet_info()
            || self.panels.transfer.is_some();
        if blocked {
            return true;
        }
        let human = self.world.curr.human;
        let sel = self.selection();
        match k {
            GroupKey::Store(g) => {
                if sel.mode == 2 && self.world.game.state.fleets.get(sel.fleet).is_some_and(|f| f[0x15] == human) {
                    self.fleet_groups.0[g] = sel.fleet;
                    self.sounds.push(crate::audio::CLICK);
                }
            }
            GroupKey::Recall(g) => {
                let f = self.fleet_groups.0[g];
                // shortcut: a freed slot reused by a new fleet of the human's recalls that fleet; a fleet id + birth
                // tick would tell them apart if players notice.
                let Some(r) = self.world.game.state.fleets.get(f).filter(|r| f > 0 && r[0x15] == human) else { return true };
                let at = [f32::from_le_bytes(r[0x30..0x34].try_into().unwrap()), f32::from_le_bytes(r[0x34..0x38].try_into().unwrap())];
                if sel.mode == 2 && sel.fleet == f {
                    self.apply(vec![Action::Centre { x: at[0], y: at[1] }]);
                } else {
                    self.sounds.push(crate::audio::CLICK);
                    self.select(Selection { mode: 2, fleet: f, ..sel });
                }
            }
        }
        true
    }

    /// `ColonyKeyboardHandler`'s held camera keys (spec §11.9), down and up: the arrows turn and tilt, numpad + / −
    /// zoom. True when taken.
    fn colony_camera_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        self.colony_camera_key_by(event.physical_key, event.logical_key.as_ref(), event.state == ElementState::Pressed)
    }

    fn colony_camera_key_by(&mut self, physical: winit::keyboard::PhysicalKey, logical: Key<&str>, down: bool) -> bool {
        use winit::keyboard::{KeyCode, PhysicalKey};
        // `DoKeyboard` (exe 0x4db1a0 line 186) hands every key up to the screen's handler, a text entry up or not;
        // a key down goes to the entry.
        let entry = self.menus.rename.is_some() || self.hud.as_ref().is_some_and(|h| h.chat.entry.is_some());
        if self.overlay.is_some() || self.spy.is_some() || self.colony.is_none() || (entry && down) {
            return false;
        }
        // `ColonyKeyboardHandler` (exe 0x4628c0 line 26): the top row's 4..9 (vk 0x34..=0x39, not the numpad) call
        // a group back, with Ctrl keep it, in a war or not.
        let group = match physical {
            PhysicalKey::Code(KeyCode::Digit4) => Some(0),
            PhysicalKey::Code(KeyCode::Digit5) => Some(1),
            PhysicalKey::Code(KeyCode::Digit6) => Some(2),
            PhysicalKey::Code(KeyCode::Digit7) => Some(3),
            PhysicalKey::Code(KeyCode::Digit8) => Some(4),
            PhysicalKey::Code(KeyCode::Digit9) => Some(5),
            _ => None,
        };
        if let Some(n) = group {
            if down {
                self.colony_input(move |ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::War(crate::colony_war::WarKey::Group(n))));
            }
            return true;
        }
        let Some(c) = &mut self.colony else { return false };
        let k = &mut c.ui.cam_keys;
        // VK 0x25..=0x28 by the logical key (a NumLock-off numpad sends them too), numpad + / − by
        // the physical one.
        let Some(i) = do_keyboard::colony_camera_key(physical, logical) else { return false };
        let flag = match i {
            0 => &mut k.left,
            1 => &mut k.right,
            2 => &mut k.up,
            3 => &mut k.down,
            4 => &mut k.zoom_in,
            _ => &mut k.zoom_out,
        };
        *flag = down;
        true
    }

    /// The building names' option row (`KellEpuletnevKiiras` exe 0x996468, id 202): `ShowBuildings` (exe 0x4c9220)
    /// writes them only while it is on. The handler's End toggle (exe 0x4628c0 line 284) never runs in the GOG exe:
    /// `remapKeyCodes` (exe 0x4e4670) has no case for SDLK_END, so its key down is never queued.
    fn building_names(&self) -> bool {
        self.opt_rows.iter().find(|r| r.key == NAMES_KEY).is_none_or(|r| r.value != 0)
    }

    /// The trade screen's keyboard handler (exe 0x609ae0): numpad + / − and the arrows, by virtual key, down only.
    /// True when taken.
    fn trade_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        self.trade_key_by(event.physical_key, event.logical_key.as_ref(), event.state == ElementState::Pressed)
    }

    fn trade_key_by(&mut self, physical: winit::keyboard::PhysicalKey, _logical: Key<&str>, down: bool) -> bool {
        use crate::trade::Key as T;
        use winit::keyboard::{KeyCode, PhysicalKey};
        if self.overlay != Some(crate::ui::Screen::Trade) {
            return false;
        }
        let k = match physical {
            PhysicalKey::Code(KeyCode::NumpadAdd) => T::More,
            PhysicalKey::Code(KeyCode::NumpadSubtract) => T::Less,
            PhysicalKey::Code(KeyCode::ArrowUp) => T::Up,
            PhysicalKey::Code(KeyCode::ArrowDown) => T::Down,
            PhysicalKey::Code(KeyCode::ArrowLeft) => T::Left,
            PhysicalKey::Code(KeyCode::ArrowRight) => T::Right,
            _ => return false,
        };
        if let (true, Some(tr)) = (down, &mut self.trade) {
            tr.key(&self.world.game.state, k);
        }
        true
    }

    /// `StarmapKeyboardHandler` (ELF 0x704580, exe 0x5aeab0; re/screens/starmap.md §10.6) on the starmap itself, down
    /// and up: the held camera keys (not under the long info, the zoom or the fleet's panel), the fleet panel's arrows,
    /// PgUp/PgDn and `c` (not while the tutorial locks the starmap). True when taken.
    fn starmap_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        self.starmap_key_by(event.physical_key, event.logical_key.as_ref(), event.state == ElementState::Pressed)
    }

    fn starmap_key_by(&mut self, physical: winit::keyboard::PhysicalKey, logical: Key<&str>, down: bool) -> bool {
        use crate::starmap::CamKey as K;
        use winit::keyboard::{KeyCode, PhysicalKey};
        // A key up reaches the handler with the rename's entry up too (`DoKeyboard` line 186).
        if self.view.is_none() || self.colony.is_some() || self.under_screen() || (self.menus.rename.is_some() && down) {
            return false;
        }
        // VK 0x25..=0x28 by the logical.clone() key (a NumLock-off numpad sends them too), numpad + / − by the physical one.
        let vk = match (physical, logical.clone()) {
            (PhysicalKey::Code(KeyCode::NumpadAdd), _) => Some(K::ZoomIn),
            (PhysicalKey::Code(KeyCode::NumpadSubtract), _) => Some(K::ZoomOut),
            (_, Key::Named(NamedKey::ArrowLeft)) => Some(K::Left),
            (_, Key::Named(NamedKey::ArrowUp)) => Some(K::Up),
            (_, Key::Named(NamedKey::ArrowRight)) => Some(K::Right),
            (_, Key::Named(NamedKey::ArrowDown)) => Some(K::Down),
            _ => None,
        };
        // The characters `=` and `-` (a numpad − gives its `-` after the key's own flag, which then ignores it).
        let ch = match (logical.clone(), down) {
            (Key::Character("="), true) => Some(K::StepIn),
            (Key::Character("-"), true) if vk.is_none() => Some(K::StepOut),
            _ => None,
        };
        let fleet_panel = self.panels.fleet_info();
        let gate = !self.menus.info_on() && self.cam_saved.is_none() && !fleet_panel;
        if let Some(k) = vk.or(ch) {
            if gate {
                self.star_keys.key(k, down);
            }
            if fleet_panel && down {
                let step = match k {
                    K::Up => -2,
                    K::Down => 2,
                    K::Left => -1,
                    K::Right => 1,
                    _ => 0,
                };
                let sel = self.selection();
                if let (true, Some(i)) = (step != 0, &mut self.panels.info) {
                    i.key(&self.world.game.state, &sel, step);
                }
            }
            return true;
        }
        if !down || self.starmap_locked() {
            return false;
        }
        match logical.clone() {
            Key::Named(k @ (NamedKey::PageUp | NamedKey::PageDown)) => {
                // `remapKeyCodes` swaps them: PgUp comes as vk 0x22, the step on (FX5).
                self.starmap_step(k == NamedKey::PageUp);
                true
            }
            Key::Character("c") => {
                self.starmap_colonise();
                true
            }
            _ => false,
        }
    }

    /// PgUp / PgDn on the starmap: the previous / next own planet or fleet, centred; not with a select menu, the
    /// long info flying, the fleet's panel or a transfer up (re/screens/starmap.md §10.6).
    fn starmap_step(&mut self, next: bool) {
        use crate::starmap_menus::InfoPhase;
        let blocked = self.menus.menu.as_ref().is_some_and(|m| matches!(m.kind, MenuKind::Tax | MenuKind::AutoBuild | MenuKind::Priority | MenuKind::Sale(_)))
            || self.menus.info.is_some_and(|i| i.phase != InfoPhase::Up)
            || self.panels.fleet_info()
            || self.panels.transfer.is_some();
        let sel = self.selection();
        if blocked {
            return;
        }
        let Some((s, at)) = crate::starmap_panels::step_selection(&self.world.game.state, &sel, next, self.modifiers.shift_key()) else { return };
        // The long info follows the planet (its `StarSize` swap).
        if let (1, Some(i)) = (s.mode, &mut self.menus.info) {
            i.planet = s.planet;
        }
        self.select(s);
        self.apply(vec![Action::Centre { x: at[0], y: at[1] }]);
        if s.mode == 1 {
            if let Some(i) = self.panels.info.as_mut().filter(|i| i.kind == crate::starmap_info::Kind::Tanks) {
                (i.first, i.selected) = (0, 0);
            }
        }
        // `DeleteMenus`.
        self.menus.menu = None;
        if let Some(i) = &mut self.panels.info {
            i.sale = None;
        }
    }

    /// Both buttons down over the info box (hot-spot 8): the rename of the shown planet (any) or the human's fleet (not
    /// `fSpec`) starts (`Starmap_Main`, re/screens/starmap.md §10.7; a planet's zoom sound, a fleet's click).
    fn start_rename(&mut self) {
        let sel = self.selection();
        let state = &self.world.game.state;
        let human = self.world.curr.human;
        let r = match sel.mode {
            1 if sel.planet >= 1 && sel.planet <= state.planet_count() => {
                self.sounds.push(crate::audio::SoundId::global(5));
                let r = state.mem.read(ig2_sim::save::va::PLANETS + (sel.planet * ig2_sim::save::PLANET_SIZE) as u32, 0x18);
                crate::starmap_menus::Rename::new(false, sel.planet, &r[..r.iter().position(|&c| c == 0).unwrap_or(r.len())])
            }
            2 => {
                let Some(f) = state.fleets.get(sel.fleet).filter(|f| f[0x15] == human && u16::from_le_bytes([f[0x52], f[0x53]]) & 0x4400 == 0) else { return };
                self.sounds.push(crate::audio::CLICK);
                crate::starmap_menus::Rename::new(true, sel.fleet, &f[..f.iter().take(0x14).position(|&c| c == 0).unwrap_or(0x14)])
            }
            _ => return,
        };
        // A fleet's rename sets its custom-name bit for good (`+0x52 |= 0x800`, `Starmap_Main` exe 0x5b62be / 0x5b6cf4;
        // Esc or a press elsewhere keeps it): `Show_Flottak2d` draws its name on the map from then on. Only that draw
        // and the Elem story read it, so the local edit (no message, as the name's) can't desync.
        if r.fleet {
            if self.world.net.is_some() {
                // Multiplayer: shown here at once, the bit and the name go through the lockstep when the rename ends
                // (deviation 395).
                let name = r.backup.clone();
                self.world.game.edit_local_name(true, r.index, &name, true);
            } else if let Some(f) = self.world.game.state.fleets.get_mut(r.index) {
                f[0x53] |= 0x08;
            }
        }
        self.menus.rename = Some(r);
    }

    /// The pointer on the colony's short info (hot-spot 4, (0, 424) 166 × 56) with no unit or building selected
    /// (`ColonyInfoMod` 0).
    fn colony_rename_spot(&self) -> bool {
        let Some(c) = &self.colony else { return false };
        let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
        (0.0..166.0).contains(&at[0]) && (424.0..480.0).contains(&at[1]) && c.ui.selected == 0 && c.ui.war.selected.is_empty()
    }

    /// The colony's name entry (`Colony_Main`, `IOAzonosito` 0x1f): `IOString` = the shown planet's name, 15
    /// characters, at (50, 427) in colour 0x22, the first key replacing it.
    fn start_colony_rename(&mut self) {
        let Some(p) = self.world.colony_curr.as_ref().map(|s| s.planet) else { return };
        let r = self.world.game.state.mem.read(ig2_sim::save::va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, 0x18);
        self.menus.rename = Some(crate::starmap_menus::Rename::new(false, p, &r[..r.iter().position(|&c| c == 0).unwrap_or(r.len())]));
    }

    /// The rename's text written into the record (`IOString` is the record's name).
    fn write_rename(&mut self, text: &[u8]) {
        let Some(r) = &self.menus.rename else { return };
        // Multiplayer: the shared state isn't written outside the lockstep; the screen shows the typed name and the
        // rename's end sends it (`send_renames`, deviation 395).
        if self.world.net.is_some() {
            let (fleet, index) = (r.fleet, r.index);
            self.world.game.edit_local_name(fleet, index, text, fleet);
            return;
        }
        let state = &mut self.world.game.state;
        let mut name = text.to_vec();
        name.push(0);
        if r.fleet {
            if let Some(f) = state.fleets.get_mut(r.index) {
                f[..name.len()].copy_from_slice(&name);
            }
        } else {
            state.mem.write(ig2_sim::save::va::PLANETS + (r.index * ig2_sim::save::PLANET_SIZE) as u32, &name);
        }
    }

    /// A key while the rename is up (`DoKeyboard`'s editing branch): Enter ends it (the tutorial's events 4 / 3),
    /// Esc restores the old name.
    fn rename_key(&mut self, k: &Key<&str>) {
        use crate::starmap_menus::RenameKey as R;
        let key = match k {
            Key::Named(NamedKey::Enter) => {
                // `DoKeyboard` (exe 0x4db9a7 / 0x4db9c0, `AktKep` 2): the tutorial's 4 for a fleet (`IOAzonosito` 0x16),
                // 3 for a planet (0x15), then `IOMode = 0`; on the colony screen (`AktKep` 3, 0x1f) only the end.
                if let Some(r) = self.menus.rename.take().filter(|_| self.colony.is_none()) {
                    self.world.game.tutorial_check_event(if r.fleet { 4 } else { 3 });
                }
                return;
            }
            // `DoKeyboard` case 0x1b (exe 0x4db1a0): nothing while a screen change is pending (`Belep`, `Atlepes`,
            // `AktKepKell`).
            Key::Named(NamedKey::Escape) if self.screen_changing() => return,
            Key::Named(NamedKey::Escape) => {
                if let Some(b) = self.menus.rename.as_ref().map(|r| r.backup.clone()) {
                    self.write_rename(&b);
                }
                self.menus.rename = None;
                return;
            }
            Key::Named(NamedKey::Backspace) => R::Back,
            Key::Named(NamedKey::Delete) => R::Delete,
            Key::Named(NamedKey::Home) => R::Home,
            Key::Named(NamedKey::End) => R::End,
            Key::Named(NamedKey::ArrowLeft) => R::Left,
            Key::Named(NamedKey::ArrowRight) => R::Right,
            Key::Named(NamedKey::Space) => R::Char(b' '),
            Key::Character(c) => {
                let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                match b.first().copied().filter(|&b| b >= 0x20 && b != 0x81) {
                    Some(b) => R::Char(b),
                    None => return,
                }
            }
            _ => return,
        };
        let Some(r) = &mut self.menus.rename else { return };
        r.key(key);
        let text = r.text.clone();
        self.write_rename(&text);
    }

    /// `DesignKeret1`'s phase is one set of globals (`dkm` 0xB34C0C, `dkmcel` 0x903548, `GfxNovelo` 0xB34BAC) that
    /// `Frissit2_Grafika` steps every UI frame and no screen resets; only the frame's places reset on a screen change.
    /// The shown screen's frame carries it; a newly shown one starts from it.
    fn row_frame_phase(&mut self, due_ui: u64) {
        if self.front.is_some() {
            return;
        }
        use crate::ui::Screen;
        let (owner, frame) = if let Some(o) = &mut self.options {
            (1, Some(&mut o.s.design))
        } else if let (Some(Screen::Design), Some(d)) = (self.overlay, &mut self.design) {
            (2, Some(&mut d.list_frame))
        } else if let (Some(Screen::Trade), Some(t)) = (self.overlay, &mut self.trade) {
            (3, Some(&mut t.list_frame))
        } else if let (Some(Screen::Research), Some(r)) = (self.overlay, &mut self.research) {
            (4, Some(&mut r.design))
        } else if let (None, None, Some(i)) = (self.overlay, &self.colony, &mut self.panels.info) {
            (5, Some(&mut i.frame))
        } else {
            (0, None)
        };
        carry_row_frame(&mut self.dk, &mut self.dk_owner, owner, frame, due_ui);
    }

    /// `c` on the starmap (character 99): a free planet, a colony ship in the pool, none sent this frame or on the
    /// way, and no -0x2094 info: the colonise order 0x16 (kind 1).
    fn starmap_colonise(&mut self) {
        let sel = self.selection();
        let state = &self.world.game.state;
        if sel.mode != 1 || self.menus.col_sent || !crate::starmap_menus::can_send_colony_ship(state, sel.planet) {
            return;
        }
        self.send(StarmapOrder::Colonise { planet: sel.planet as i32, kind: 1 });
        self.menus.col_sent = true;
    }

    /// ADR 0040: on the messages screen `L` opens the event log; while it is up every key but the F-keys is its
    /// (the search, Esc, the arrows and PgUp/PgDn scroll). True when taken.
    /// ADR 0058: `k` as the key of the action the player bound it to, at the screen up; false = the default key of an
    /// action bound elsewhere, which does nothing. Not in the menus, a video, a text entry or the open event log.
    fn rebind(&self, k: &mut winit::event::KeyEvent) -> bool {
        use crate::bindings::Remap;
        let winit::keyboard::PhysicalKey::Code(code) = k.physical_key else { return true };
        let log_open = self.hud.as_ref().is_some_and(|h| h.messages.log.open);
        let modern = self.options.as_ref().is_some_and(|o| o.s.modern.is_some());
        if self.bindings.is_default() || self.front.is_some() || self.video.is_some() || modern || self.entry_up() || log_open {
            return true;
        }
        match self.bindings.remap(crate::bindings::Ctx::of_screen(self.akt_kep_now()), code) {
            Remap::Keep => true,
            Remap::Drop => false,
            Remap::To(to) => {
                let upper = matches!(k.logical_key.as_ref(), Key::Character(c) if c.chars().any(char::is_uppercase));
                let (logical, text) = crate::bindings::logical(to, upper);
                k.physical_key = winit::keyboard::PhysicalKey::Code(to);
                k.logical_key = logical;
                k.text = if k.state == ElementState::Pressed { text } else { None };
                true
            }
        }
    }

    fn event_log_key(&mut self, key: &Key<&str>) -> bool {
        use crate::chat::EntryKey as E;
        let on = self.display.event_log && self.overlay == Some(crate::ui::Screen::Messages);
        let Some(h) = self.hud.as_mut().filter(|h| on && h.messages.list_up()) else { return false };
        let log = &mut h.messages.log;
        if !log.open {
            if matches!(key, Key::Character("l" | "L")) {
                log.toggle();
                return true;
            }
            return false;
        }
        let k = match key {
            Key::Character(s) => s.bytes().next().map(E::Char),
            Key::Named(NamedKey::Space) => Some(E::Char(b' ')),
            Key::Named(NamedKey::Backspace) => Some(E::Backspace),
            Key::Named(NamedKey::Delete) => Some(E::Delete),
            Key::Named(NamedKey::Home) => Some(E::Home),
            Key::Named(NamedKey::End) => Some(E::End),
            Key::Named(NamedKey::ArrowLeft) => Some(E::Left),
            Key::Named(NamedKey::ArrowRight) => Some(E::Right),
            Key::Named(NamedKey::Escape) => Some(E::Escape),
            Key::Named(n @ (NamedKey::ArrowUp | NamedKey::ArrowDown | NamedKey::PageUp | NamedKey::PageDown)) => {
                let lines = match n {
                    NamedKey::ArrowUp => -1,
                    NamedKey::ArrowDown => 1,
                    NamedKey::PageUp => -30,
                    _ => 30,
                };
                log.scroll_by(lines, &self.world.log);
                None
            }
            _ => None,
        };
        if let Some(k) = k {
            log.key(k);
        }
        true
    }

    /// ADR 0040: a log line's planet or fleet selected and centred on the starmap (the messages screen closes first).
    fn log_jump(&mut self, t: crate::event_log::Target) {
        use crate::event_log::Target;
        let sel = self.selection();
        let curr = &self.world.curr;
        let hit = match t {
            Target::Planet(p) => curr.planets.iter().find(|v| v.index == p && v.on_map()).map(|v| (Selection { mode: 1, planet: p, ..sel }, v.pos)),
            Target::Fleet(f) => curr.fleets.iter().find(|v| v.slot == f && v.visible).map(|v| (Selection { mode: 2, fleet: f, ..sel }, v.pos)),
        };
        let Some((s, at)) = hit else { return };
        self.sounds.push(crate::audio::CLICK);
        self.request_screen(2);
        self.apply(vec![Action::Select(s), Action::Centre { x: at[0], y: at[1] }]);
    }

    /// A key went down; true = quit.
    fn key(&mut self, key: Key<&str>) -> bool {
        if self.chat_key(&key) {
            return false;
        }
        // `DoKeyboard`'s F-keys (exe 0x4db1a0): before the text entries, which don't take them.
        if let Some(n) = do_keyboard::f_key_screen(&key) {
            self.f_key(n);
            return false;
        }
        // ADR 0032: F12 (no screen of the original's takes it) quick-saves, Shift+F12 quick-loads.
        if key == Key::Named(NamedKey::F12) {
            if self.modifiers.shift_key() {
                self.quick_load();
            } else {
                self.quick_save();
            }
            return false;
        }
        if self.event_log_key(&key) {
            return false;
        }
        // The replay viewer's keys (ours, ADR 0038 §6).
        if let (true, Key::Character(c)) = (self.replay_bar_up(), &key) {
            if let Some(a) = crate::replay_viewer::key_act(c) {
                self.replay_act(a);
                return false;
            }
        }
        match key {
            // The starmap's rename takes the keys (`IOMode`).
            _ if self.menus.rename.is_some() && !self.under_screen() => self.rename_key(&key),
            // `DoKeyboard` case 0x1b (exe 0x4db1a0): the name input ignores Esc while a screen change is pending.
            Key::Named(NamedKey::Escape)
                if self.overlay == Some(crate::ui::Screen::Design) && self.design.as_ref().is_some_and(|d| d.entry.is_some()) && self.screen_changing() => {}
            // `DesignKeyboardEscPressed`: the name input ends, an edit is cancelled; else the options screen.
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Design) => {
                if self.design_do(|d, c, _| d.escape(c)) != Some(true) && self.o_key_allowed() {
                    self.open_options(false);
                }
            }
            // The name input (`IOMode`, `IOAzonosito` 0x33; exe `DoKeyboard` 0x4db1a0): Enter ends it
            // (`DesignEntertNyomott`, no `DesignKeyboardHandler` after it); the other keys, Space, the digits and the
            // arrows included, go to the input or nowhere.
            _ if self.overlay == Some(crate::ui::Screen::Design) && self.design.as_ref().is_some_and(|d| d.entry.is_some()) => {
                self.design_entry_key(&key)
            }
            Key::Named(NamedKey::Enter) if self.overlay == Some(crate::ui::Screen::Design) => {
                self.design_do(|d, c, _| d.edit_key(c, false));
            }
            Key::Character("r") if self.overlay == Some(crate::ui::Screen::Design) => {
                self.design_do(|d, c, _| d.edit_key(c, true));
            }
            // The trade screen has no keyboard handler but its arrows and ± (`trade_key`): Esc asks for the options
            // screen at once.
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Trade) => {
                if self.o_key_allowed() {
                    self.trade_under_options = self.trade.take();
                    self.overlay = None;
                    self.open_options(false);
                }
            }
            Key::Character(c) if self.overlay == Some(crate::ui::Screen::Trade) && !matches!(c, "1" | "2" | "3" | "p" | "P" | "m" | "M" | "o" | "O") => {}
            Key::Named(NamedKey::Tab | NamedKey::PageUp | NamedKey::PageDown | NamedKey::Delete) if self.overlay == Some(crate::ui::Screen::Trade) => {}
            // The design screen's own keys; the other screens' do nothing here (no `DispatchKeyboard` target).
            Key::Character(c) if self.overlay == Some(crate::ui::Screen::Design) && !matches!(c, "1" | "2" | "3" | "p" | "P" | "m" | "M" | "o" | "O") => {}
            Key::Named(NamedKey::Tab | NamedKey::PageUp | NamedKey::PageDown | NamedKey::Delete) if self.overlay == Some(crate::ui::Screen::Design) => {}
            Key::Named(k @ (NamedKey::ArrowUp | NamedKey::ArrowDown)) if self.overlay == Some(crate::ui::Screen::Design) => {
                if let Some(d) = &mut self.design {
                    d.key(if k == NamedKey::ArrowUp { crate::design::Key::Up } else { crate::design::Key::Down });
                }
            }
            // Esc on the messages screen (`MainCallBackEventHandler` exe 0x4e2db0): when idle `AktIkonKell = 0x25`, a press
            // on hot-spot 7 (`IG2_gameUpdate` 0x4e0440), which leaves for `MessagesElottiScreen` as a click there does;
            // nothing while answers wait or the text types.
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Messages) => {
                if self.messages_idle() && !self.screen_changing() {
                    self.leave_messages();
                }
            }
            // `MessagesKeyboardHandler` (exe 0x507eb0): a..d answer.
            Key::Character(c @ ("a" | "b" | "c" | "d")) if self.overlay == Some(crate::ui::Screen::Messages) => {
                if let Some(h) = &mut self.hud {
                    h.messages.key(c);
                }
            }
            // The ground war's keys (`ColonyKeyboardHandler`, colony.md §11.2) while the shown planet is at war.
            Key::Character(ch) if self.colony_war_key(ch).is_some() => {
                if let Some(k) = self.colony_war_key(ch) {
                    self.colony_input(move |ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::War(k)));
                }
            }
            // Esc (`MainCallBackEventHandler` exe 0x4e2db0): on the starmap with a panel up the panel's exit (hot-spot
            // 9); on the colony in its planet info mode hot-spot 0x1f (`AktIkonKell` 0x3d), the panel's dead spot:
            // nothing; else `ESC_Pressed` (ELF 0x581440): Options (re/screens/options.md §5.1).
            Key::Named(NamedKey::Escape) if self.starmap_escape() => {}
            Key::Named(NamedKey::Escape) if self.colony.is_some() && self.overlay.is_none() && self.colony.as_ref().is_some_and(|c| c.ui.info.is_some()) => {}
            Key::Named(NamedKey::Escape) | Key::Character("o" | "O") => {
                if self.o_key_allowed() {
                    self.ask_options();
                }
            }
            // `DoKeyboard`'s M (exe 0x4db1a0 lines 92-114); refused, the character goes on to the screen (none uses it).
            Key::Character("m" | "M") => {
                self.m_key();
            }
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
            Key::Character("e" | "E") if self.colony.is_some() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Priority)),
            // Shift+Delete (vk 0x2e, `ColonyKeyboardHandler` exe 0x462a9c) never arrives: `remapKeyCodes` (exe 0x4e4670)
            // queues no key down for SDLK_DELETE. PageUp and PageDown come swapped out of it (SDLK_PAGEUP → vk 0x22,
            // SDLK_PAGEDOWN → 0x21): PageUp is the next own planet, PageDown the previous [obs: quick-1.sav planet 42,
            // PageUp → 43 → 46, PageDown → 43]; `TutorialCheckEvent(0x1b)` before the step (exe 0x462caa / 0x462df6).
            Key::Named(NamedKey::PageUp) if self.colony.is_some() => {
                self.world.game.tutorial_check_event(0x1b);
                self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Next));
            }
            Key::Named(NamedKey::PageDown) if self.colony.is_some() => {
                self.world.game.tutorial_check_event(0x1b);
                self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Prev));
            }
            // `ColonyKeyboardHandler`: I is the planet info mode on the colony screen (colony.md §12.1).
            Key::Character("i" | "I") if self.colony.is_some() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Info)),
            // `ColonyKeyboardHandler`: `c` (ch 99 only) cycles the camera presets on the colony screen, not under the
            // tutorial's colony lock (0xf66ff9).
            Key::Character("c") if self.colony.is_some() && self.overlay.is_none() && self.spy.is_none() => {
                if let Some(c) = self.colony.as_mut().filter(|c| !c.ui.all_off) {
                    c.ui.cam_keys.preset();
                }
            }
            // `DoKeyboard`: Space and P pause.
            // `DoKeyboard`: Space and P pause; MP on any screen (input.md §5.2).
            Key::Named(NamedKey::Space) | Key::Character("p" | "P") => {
                if self.time_keys_allowed() || self.multiplayer() {
                    self.toggle_pause();
                }
            }
            Key::Character(c @ ("1" | "2" | "3")) if self.multiplayer() => self.set_speed(c.parse().unwrap_or(1)),
            Key::Character(c @ ("1" | "2" | "3")) if self.time_keys_allowed() => {
                // `DoKeyboard` (ELF 0x5800d0): the keys sound whatever the speed was, single player only.
                self.sounds.push(crate::audio::SoundId::global(10));
                let level = c.parse().unwrap_or(1);
                if !self.turbo_step(level) && (self.world.paused() || self.world.game.speed_level() != level) {
                    self.send(StarmapOrder::Speed { level });
                    self.send(StarmapOrder::Pause { paused: false });
                }
            }
            _ => {}
        }
        false
    }
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
