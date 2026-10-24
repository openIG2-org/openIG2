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
//! V = vsync on/off, Esc or O = the Options screen (settings, save and load slots, quit; re/screens/options.md), Esc
//! on the messages screen closes it.

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
    /// The scenario list's entries, tables and pictures, read when it first opens.
    scenarios: Option<crate::mainscr_scenario::Assets>,
    /// Our MP screens (ADR 0014), their links, and the session a started room built (the starmap takes it).
    mp: Option<crate::mp_lobby::MpScreen>,
    net: crate::mp_lobby::QuicNet,
    session: Option<crate::mp_lobby::Session<'static>>,
}

impl Front {
    /// `MainScr_Belep`, `MultiplayerNev` from the settings file (`SaveGlobalRegistrySettings`' value; "Player" without one).
    fn new(scene: Option<crate::mainscr_scene::MainScene>, ui_ms: u32, settings: &ig2_formats::options::Options) -> Self {
        let mut s = crate::mainscr::MainScr::enter();
        if let Some(n) = settings.get("MultiplayerNev").filter(|n| !n.is_empty()) {
            s.player = n.as_bytes().to_vec();
        }
        Front { s, scene, ui_ms, at: [320.0, 240.0], quit: false, scenarios: None, mp: None, net: Default::default(), session: None }
    }
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
    /// `FRISSIT_Messages2`'s frame counter (the automatic messages every 11th UI frame).
    messages_auto: u32,
    /// The diplomacy screen while it is up, and this frame's pointer edges on it.
    diplomacy: Option<crate::diplomacy::Open>,
    dip_input: crate::diplomacy::Input,
    /// The screen the diplomacy screen's closing leads to (`Diplomacy_Kilep`; `None` = the starmap).
    dip_next: Option<crate::ui::Screen>,
    gpu: Option<Gpu>,
    view: Option<View>,
    clock: TickClock,
    start: Instant,
    vsync: bool,
    /// `IG2_PROFILE_TOUR`'s keys (crate::prof).
    tour: Option<crate::prof::Tour>,
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
    /// The Options screen while it is up (re/screens/options.md), over the main menu (load only) or the game.
    options: Option<OptionsUp>,
    help: Option<HelpUp>,
    /// `OldTopicSel` and the line table across visits.
    help_memory: crate::help::Memory,
    /// What Options needs to start and to load a slot: its rows (from the user's exe), the settings file, the
    /// save's companions.
    opt_rows: Vec<crate::options::Row>,
    settings: ig2_formats::options::Options,
    rules: &'static ig2_sim::game::GameRules,
    tech_defaults: Vec<u8>,
    volatile: Vec<(u32, Vec<u8>)>,
    /// The pointer in physical window pixels (the battle screen's edge scrolling reads the window, not the frame).
    cursor_px: [i32; 2],
    /// The mouse buttons held (bit 0 left, bit 1 right): `Mouse_Gomb` stops the colony's edge scroll.
    mouse_held: u8,
    /// The pointer is over the window (a `CursorMoved` came, no `CursorLeft` since): the edge scroll reads it.
    pointer_in: bool,
    /// The window's pointer is hidden (the colony screen draws its own).
    cursor_hidden: bool,
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
    let mut world = crate::start_world(&o, &l.install, l.save.as_ref(), rules, &l.tech_defaults, &l.volatile)?;
    world.set_speed(o.speed);
    let exe = l.install.read_raw("ig2.exe").map_err(|e| e.to_string())?;
    let sounds = ig2_rules::sounds::SoundTables::from_exe(&exe).map_err(|e| e.to_string())?;
    let audio_install = ig2_formats::Install::open(l.install.root(), l.install.lang()).map_err(|e| e.to_string())?;
    let mut audio = crate::audio::Audio::new(audio_install, sounds, true);
    audio.set_speech(&ig2_rules::speech::Speech::from_exe(&exe).unwrap_or_default());
    if !audio.has_device() {
        eprintln!("ig2-client: no sound device; running silent");
    }
    // The client starts on the main menu (`MainScr_Belep`'s `PlayMusicMenu(false, true)`), or with `--save` on the
    // starmap (`Starmap_Belep`'s `PlayMusicMain(0, 0)`); nothing plays yet: at once.
    // On the main menu the title plays first (`StartDemoMode` exe 0x4fe3c0: once per run, unless the cut-scenes are
    // off) and the menu's music follows it.
    let title = o.main_menu && crate::video::playback::cutscenes_enabled();
    let music = if o.main_menu { crate::audio::Music::Menu } else { crate::audio::Music::Main };
    // A new campaign enters the starmap after its intro (re/screens/mainscr.md §9.1).
    let start = world.start_voice.take();
    if !title && start.is_none() {
        audio.play_music(music, o.main_menu, human_race(&world.game.state));
    }
    // The option rows as the exe holds them, their values from the settings file (the registry `SaveOptions` fills).
    let mut opt_rows = ig2_rules::PeImage::parse(&exe).map(|img| crate::options::read_rows(&img, &crate::ui::UiTexts::load(&l.install))).unwrap_or_default();
    let mut app = App::new(world, l.install, tables, rules, audio);
    for r in &mut opt_rows {
        if let Some(v) = app.settings.int(&r.key) {
            r.value = v;
        }
    }
    (app.opt_rows, app.tech_defaults, app.volatile) = (opt_rows, l.tech_defaults, l.volatile);
    app.front = o.main_menu.then(|| Front::new(None, 0, &app.settings));
    // `IG2_PROFILE_BATTLE=DUMP`: a battle capture's dump goes live in slot 0 and the screen opens (crate::prof).
    if let Ok(p) = std::env::var("IG2_PROFILE_BATTLE") {
        crate::spacewar_screen::go_live(&mut app.world, crate::spacewar_screen::Dump::load(&p)?.battle);
        app.world.realtime.push(ig2_sim::game::RealTimeEvent::SpaceStarted { slot: 0 });
    }
    if title {
        app.queue_videos(["title.bik".to_string()], false, AfterVideo::MenuMusic);
    }
    if let Some(c) = start {
        app.take_video_requests();
        app.after_video = AfterVideo::StarmapEntry(c);
    }
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
    let mut world = crate::start_world(o, &l.install, l.save.as_ref(), rules, &l.tech_defaults, &l.volatile)?;
    world.set_speed(o.speed);
    let exe = l.install.read_raw("ig2.exe").map_err(|e| e.to_string())?;
    let sounds = ig2_rules::sounds::SoundTables::from_exe(&exe).map_err(|e| e.to_string())?;
    let audio_install = ig2_formats::Install::open(l.install.root(), l.install.lang()).map_err(|e| e.to_string())?;
    let mut audio = crate::audio::Audio::new(audio_install, sounds, false);
    audio.set_speech(&ig2_rules::speech::Speech::from_exe(&exe).unwrap_or_default());
    let mut renderer = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let hud = crate::ui::Hud::load(&mut renderer, &l.install, tables);
    let mut app = App::new(world, l.install, tables, rules, audio);
    app.world.game.empire_names = hud.empire_names.clone();
    app.hud = Some(hud);
    app.view = Some(View::new(&app.world.curr, &app.world.view, o.size, 1.0));
    let sel = app.selection();
    if sel.mode == 1 || sel.mode == 2 {
        app.select(sel);
    }
    app.headless = Some(Headless { renderer, band: o.size, ms: 0 });
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

/// The Options screen over `frame` (after the 3D behind it).
fn draw_options(o: &mut OptionsUp, h: &crate::ui::Hud, frame: &mut crate::render::Frame, size: [f32; 2], now_ms: f64) {
    let Some(font) = &h.font else { return };
    let d = crate::options::Draw { font, white: h.white, glow: o.glow.unwrap_or(h.white), v: crate::font::Virtual::new(size), palette: &h.palette, texts: &h.texts, now_ms };
    o.s.draw(frame, &d);
}

/// The main menu's 2D (`MainScrGfx2D_1/_2`): the scenario list, the name entry.
fn draw_front_2d(fr: &mut Front, h: &crate::ui::Hud, r: &mut crate::render::Renderer, install: &ig2_formats::Install, frame: &mut crate::render::Frame, size: [f32; 2], now_ms: f64) {
    crate::mainscr_scene::draw_2d(&mut fr.s, &mut fr.scenarios, h, r, install, frame, size, now_ms);
    if let (Some(mp), Some(font)) = (&fr.mp, &h.font) {
        let d = crate::options::Draw { font, white: h.white, glow: h.white, v: crate::font::Virtual::new(size), palette: &h.palette, texts: &h.texts, now_ms };
        mp.draw(frame, &d, crate::kit::fader_design::blink(now_ms));
    }
}

/// `PlayerFaj[EN]`, the music's race.
fn human_race(state: &ig2_sim::save::SaveState) -> u8 {
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
            left_drag: None,
            hud: None,
            overlay: None,
            spy: None,
            ui_frames: 0,
            messages_auto: 0,
            diplomacy: None,
            dip_next: None,
            dip_input: Default::default(),
            gpu: None,
            view: None,
            clock: TickClock::new(SIM_TICK_MS, 0),
            start,
            vsync: std::env::var("IG2_VSYNC").map_or(true, |v| v != "0"),
            tour: crate::prof::Tour::from_env(),
            cursor: [0.0; 2],
            drag: None,
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
            sounds: Vec::new(),
            speech: Vec::new(),
            headless: None,
            spacewar: None,
            options: None,
            help: None,
            help_memory: Default::default(),
            opt_rows: Vec::new(),
            settings: ig2_formats::options::Options::load(),
            rules,
            tech_defaults: Vec::new(),
            volatile: Vec::new(),
            video: None,
            video_queue: Default::default(),
            after_video: AfterVideo::Resume,
            anim_names: None,
            head: Default::default(),
            head_armed: false,
            mes_gyorsito: 0,
            clip_lists: None,
            cursor_px: [0; 2],
            mouse_held: 0,
            pointer_in: false,
            cursor_hidden: false,
            tutorial: None,
            tutorial_at: crate::tutorial_ui::PARKED,
            tutorial_buttons: (false, false),
            tutorial_kind: -2,
            star_keys: Default::default(),
            console_held: None,
            star_mask: None,
            dk: Default::default(),
            dk_owner: 0,
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
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        // `UpdateFrame`: the start voice's ducking (the main menu and the start screens are past 0xd).
        self.audio.start_voice_frame(self.front.is_some());
        // `FRISSIT_Scroll2` (exe 0x556210): a message taken onto the line speaks, unless it is one of the four
        // silent ids; the brutal voice for empire 5 (`EN`), else the human one.
        let en = self.world.game.state.mem.i32(ig2_sim::save::va::EN);
        for id in std::mem::take(&mut self.world.popped) {
            if !matches!(id, 1 | 0x14 | 0x15 | 0x1a) {
                let voice = if en == 5 { "brutal" } else { "human" };
                self.audio.play_wav(&format!("sounds/scroll/{voice}/{id:04}.wav"));
            }
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
        if self.overlay != self.sound_overlay {
            if self.sound_overlay == Some(Screen::Messages) {
                // `Messages_Kilep` (exe 0x5083d0) also stops the typer and the speech, and ends the ducking.
                self.audio.stop_sound(SoundId::global(7));
                self.audio.stop_message_speech();
                self.audio.set_duck(0);
                self.audio.message.indult = false;
                self.audio.play(SoundId::new(Table::Mes, 1));
            }
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
        // The starmap's ways to the colony screen (F3 `DoKeyboard` 0x4db3eb, the screen menu `MOUSE_SelectScreens`
        // 0x55555f, the colony icon 0x5b5b51 and the planet picture 0x5b6533 of `Starmap_Main`): while the tutorial
        // awaits 0x1c, the main planet (`Tutorial_MainBolygoNeve`) stays shut and any other raises the event.
        if self.world.game.state.mem.i32(ig2_sim::save::va::TUTORIAL_EVENT) == 0x1c {
            let main = self.tutorial.as_ref().map(|t| t.t.gates.main_planet.clone()).unwrap_or_default();
            let rec = self.world.game.state.mem.read(ig2_sim::save::va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, 0x18);
            if rec.split(|&b| b == 0).next() == Some(main.split(|&b| b == 0).next().unwrap_or_default()) {
                return;
            }
            self.world.game.tutorial_check_event(0x1c);
        }
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

    /// A key of the war (`ColonyKeyboardHandler`): 4..9 the groups, S stop, `a` every own unit; S on every colony
    /// screen (`StopSelected`, exe 0x4628c0), the others while the shown planet is at war.
    fn colony_war_key(&self, ch: &str) -> Option<crate::colony_war::WarKey> {
        use crate::colony_war::WarKey;
        let snap = self.world.colony_curr.as_ref().filter(|_| self.colony.is_some() && !self.under_screen())?;
        match ch {
            "s" | "S" => Some(WarKey::Stop),
            _ if !snap.war_flag => None,
            "4" | "5" | "6" | "7" | "8" | "9" => Some(WarKey::Group(ch.as_bytes()[0] as usize - b'4' as usize)),
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
        // From the diplomacy screen (`Diplomacy_Kilep`): the closing first, except towards the messages screen.
        let closing = self.overlay == Some(crate::ui::Screen::Diplomacy) && !self.leave_diplomacy(Some(s));
        if !closing {
            self.overlay = if self.overlay == Some(s) { None } else { Some(s) };
            if self.overlay == Some(crate::ui::Screen::Messages) {
                // `Messages_Belep`'s saved part (re/rules/info-messages.md §7.3).
                self.world.send(ig2_sim::game::MessageOrder::Enter.into());
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

    /// D or F6: the diplomacy screen opens (`Diplomacy_Belep`) or, when up, leaves for the starmap.
    fn toggle_diplomacy(&mut self) {
        if self.overlay == Some(crate::ui::Screen::Diplomacy) {
            if self.leave_diplomacy(None) {
                self.overlay = None;
            }
        } else if let (Some(g), Some(h)) = (&mut self.gpu, &self.hud) {
            let d = self.diplomacy.get_or_insert_with(|| crate::diplomacy::Open::new(&mut g.renderer, &self.install, self.tables, &h.texts, &self.world.game.state));
            // `Diplomacy_Belep`: no opening (and no sound) back from the messages screen.
            d.enter(self.tables, &h.texts, &self.world.game.state, self.overlay != Some(crate::ui::Screen::Messages));
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
            let state = &mut self.world.game.state;
            let en = state.mem.i32(ig2_sim::save::va::EN).clamp(0, 18) as usize;
            if let Some(r) = state.designs.get_mut(en).and_then(|t| t.get_mut(k)) {
                let n = name.len().min(0x15);
                r[..n].copy_from_slice(&name[..n]);
                r[n] = 0;
            }
        }
    }

    /// F5 (`DoKeyboard`: F1..F10 ask for `AktKep` 1..10): the design screen opens (`Design_Belep`, animated) or, when
    /// up, closes after its closing animation (`Design_Kilep`).
    fn toggle_design(&mut self) {
        if self.overlay == Some(crate::ui::Screen::Design) {
            if self.design.as_mut().is_none_or(|d| d.leave()) {
                self.overlay = None;
                self.drop_design();
            }
            return;
        }
        if let (true, Some(h)) = (self.design_tips.is_empty(), &self.hud) {
            self.design_tips = crate::design::tips(&h.texts);
        }
        self.research = None;
        let mut d = crate::design::Design::open(&self.world.game.state, true);
        if let Some(h) = &self.hud {
            d.names = crate::design_edit::Names::read(&h.texts);
        }
        self.design = Some(d);
        self.overlay = Some(crate::ui::Screen::Design);
    }

    /// F8 (`DoKeyboard`: F1..F10 ask for `AktKep` 1..10): the trade screen opens (`Trade_Belep`, animated) or, when up,
    /// closes after its closing animation (`Trade_Kilep` towards the starmap).
    fn toggle_trade(&mut self) {
        if self.overlay == Some(crate::ui::Screen::Trade) {
            if self.trade.as_mut().is_none_or(|t| t.leave(2)) {
                self.overlay = None;
                self.trade = None;
            }
            return;
        }
        let Some(h) = &self.hud else { return };
        if self.trade_tips.is_empty() {
            self.trade_tips = crate::trade::tips(&h.texts);
        }
        let advance = h.font.as_ref().map_or(6.0, |f| f.advance(2));
        let lines = crate::trade::ticker_lines(&h.texts);
        self.research = None;
        self.drop_design();
        self.trade = Some(crate::trade::Trade::open(&self.world.game.state, true, 2, lines, advance));
        self.overlay = Some(crate::ui::Screen::Trade);
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
    /// `DoKeyboard`'s chat entry in multiplayer (`IOMode` 1, exe 0x4db1a0 cases 0xd and 0x60): Enter or the
    /// backquote opens it on a game screen (`AktKep` < 0xd); while open the keys type into it and Enter sends the line
    /// (`ExecuteString`: the 0x3fc chat; the debug commands aren't ported), Esc drops it; the F-keys still switch
    /// screens, which closes it (`IG2_gameUpdate`). True when the key was taken. Enter that opens it still reaches
    /// the screen (the exe's case 0xd falls into the screen's handler); the backquote doesn't.
    fn chat_key(&mut self, key: &Key<&str>) -> bool {
        use crate::chat::EntryKey as E;
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
                self.world.say(&line);
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
        if at[1] < 20.0 || (at[0] < 166.0 && at[1] >= 424.0) || (self.panels.transfer.is_some() && at[1] >= 20.0) {
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
        if self.panels.minimap_off() || self.menus.info_on() {
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
        }
        self.console_held = Some((s, left));
        true
    }

    /// The button went up over a held console spot: its flags clear; the top view (1) and the side view (4) start
    /// when released over their button (`StarmapSetTopView`, `StarmapSetSideView`, 8 frames).
    fn console_release(&mut self) {
        let Some((s, _)) = self.console_held.take() else { return };
        let k = &mut self.star_keys;
        match s {
            2 | 3 => (k.left, k.right) = (false, false),
            5 => (k.zoom_in, k.zoom_out) = (false, false),
            6 => (k.up, k.down) = (false, false),
            _ => {}
        }
        let at = self.virtual_cursor();
        if !matches!(s, 1 | 4) || self.console_spot(at) != Some(s) {
            return;
        }
        self.sounds.push(crate::audio::CLICK);
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
        let now = self.now_ms();
        let (Some(fr), true) = (&mut self.front, self.gpu.is_some()) else { return };
        let mut due = 0;
        let mut outs = Vec::new();
        while now >= fr.ui_ms + 10 {
            fr.ui_ms += 10;
            fr.s.tick();
            if let Some(mp) = &mut fr.mp {
                outs.extend(mp.tick(&mut fr.net));
            }
            due += 1;
        }
        for o in outs {
            self.front_mp_out(o);
        }
        let (Some(fr), Some(g)) = (&mut self.front, &mut self.gpu) else { return };
        for _ in 0..due {
            self.audio.tick();
        }
        self.audio.frame(human_race(&self.world.game.state));
        fr.s.frame();
        fr.s.hover(fr.at);
        if let Some(mp) = &mut fr.mp {
            mp.hover(fr.at);
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
        if let Some(h) = &self.hud {
            let now = self.start.elapsed().as_secs_f64() * 1000.0;
            draw_front_2d(fr, h, &mut g.renderer, &self.install, &mut frame, size, now);
        }
        if let (Some(o), Some(h)) = (&mut self.options, &self.hud) {
            draw_options(o, h, &mut frame, size, self.start.elapsed().as_secs_f64() * 1000.0);
        }
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
            // `OptionsCsakLoadLehet = 1; AktKep = 0xd`: Options in load-only mode over the main menu's scene.
            Act::LoadGame => self.open_options(true),
            // `Mainscreen_Update3D` exe 0x50147d: `OpenBink("misc/credits.bik", 1, 0, 0, force)`, blocking.
            Act::Credits => self.queue_videos(["misc/credits.bik".to_string()], true, AfterVideo::Resume),
            Act::Click => self.audio.play(crate::audio::CLICK),
            Act::OpenScenarios | Act::Campaign => {
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
                    let mut own = ig2_formats::options::Options::own_path().and_then(|p| std::fs::read(p).ok()).map(|b| ig2_formats::options::Options::parse(&b)).unwrap_or_default();
                    own.set("MultiplayerNev", &name);
                    if let Some(p) = ig2_formats::options::Options::own_path() {
                        if let Err(e) = std::fs::write(&p, own.to_bytes()) {
                            eprintln!("ig2-client: {}: {e}", p.display());
                        }
                    }
                }
            }
            // `MainScr_Main` 0x23 / 0x22: the server path and the remaster's matchmaking; ours: our MP screens
            // (ADR 0014 §4; "inet" is the same screen without matchmaking).
            Act::MultiServer | Act::MultiMatchmaking => {
                let rules = self.install.read_raw("ig2.exe").map(|exe| ig2_net::session::rules_fingerprint(&exe)).unwrap_or_default();
                let Some(f) = &mut self.front else { return };
                let name = String::from_utf8_lossy(&f.s.player).into_owned();
                f.mp = Some(crate::mp_lobby::MpScreen::open(a == Act::MultiMatchmaking, &name, rules));
            }
            Act::Tutorial(_) => {}
        }
    }

    /// What our MP screens ask (ADR 0014): the click, leaving them, or the room's start: the game built here
    /// (`mp_lobby::launch`), then the menu's exit animation into the starmap.
    fn front_mp_out(&mut self, o: crate::mp_lobby::Out) {
        use crate::mp_lobby::Out;
        match o {
            Out::Click => self.audio.play(crate::audio::CLICK),
            Out::Close => {
                if let Some(f) = &mut self.front {
                    f.mp = None;
                    f.s.close_mp();
                }
            }
            Out::Start(b) => {
                let rules = self.rules;
                let ticks = crate::new_game::wall_ticks();
                let built = crate::new_game::StartTexts::read(&self.install).and_then(|t| t.with(&self.install, |data| crate::mp_lobby::launch(b, data, rules, ticks))).and_then(|r| r);
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
                let cfg = ig2_sim::game::new_game::NewCampaign { campaign: *race as i32 + 1, difficulty, seed, ticks: seed, storyline: 0, storymap: 0, last_storyline: None };
                crate::new_game::campaign(&self.install, &cfg, rules)
            }
            Start::Scenario { file, .. } => crate::new_game::scenario(&self.install, file, seed, rules),
            Start::Tutorial(n) => crate::new_game::tutorial(&self.install, *n, seed, rules),
            Start::Multiplayer => unreachable!("handled above"),
            // `SetScirmishGameOptions(race, computers, kind, technology, ScenarioKezdoBolygoSzamok[planets],
            // ScenarioKezdoPenzek[money], difficulty, population)` (spec §2.2 step 3).
            Start::Skirmish { settings: k, kind } => {
                let tables = self.front.as_ref().and_then(|f| f.scenarios.as_ref()).map(|a| (a.tables.planets, a.tables.money));
                match tables {
                    Some((planets, money)) => {
                        let cfg = ig2_sim::game::new_game::NewSkirmish {
                            en: k.race,
                            ais: k.computers,
                            random_id: *kind,
                            tech: k.technology,
                            planets: planets[k.planets.clamp(0, 4) as usize],
                            money: money[k.money.clamp(0, 5) as usize],
                            difficulty: k.difficulty,
                            population: k.population,
                            seed,
                            ticks: seed,
                        };
                        crate::new_game::skirmish(&self.install, &cfg, rules)
                    }
                    None => Err("no scenario list".into()),
                }
            }
        };
        match game {
            Ok(g) => {
                let mut w = World::new_game(g);
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
    fn open_options(&mut self, load_only: bool) {
        if self.options.is_some() {
            return;
        }
        let Some(h) = &self.hud else { return };
        let s = crate::options::Settings { music: self.audio.music_volume, sound: self.audio.sound_volume, ..crate::options::start_settings(&self.settings) };
        let slots = crate::options::read_slots(crate::options::slot_dir().as_deref());
        let screen = crate::options::OptionsScreen::enter(&h.texts, self.opt_rows.clone(), &s, load_only, false, slots);
        let glow = self.gpu.as_mut().and_then(|g| {
            self.install.resolve_texture("textures/starmap/", "laservago.bmp").and_then(|p| crate::assets::Image::load(&self.install, &p).ok()).map(|img| g.renderer.add_texture(&img))
        });
        let was_paused = self.world.paused();
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
        let Some(o) = &mut self.options else { return };
        let act = if o.s.entry.is_some() {
            let key = match k.as_ref() {
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
        if self.options.is_some() {
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
        let now = self.now_ms();
        let due_ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50);
        self.ui_frames = now as u64 / 10;
        for _ in 0..due_ui {
            self.audio.tick();
        }
        self.audio.frame(human_race(&self.world.game.state));
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
        if let Some(font) = &hud.font {
            let d = crate::help::Draw { font, white: hud.white, glow: h.glow.unwrap_or(hud.white), v: crate::font::Virtual::new(size), palette: &hud.palette, texts: &hud.texts, pics: &h.pics, now_ms: self.start.elapsed().as_secs_f64() * 1000.0 };
            h.s.draw(&mut frame, &d, crate::help::char_y(font));
        }
        hud.paused = self.world.paused();
        hud.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Help, t);
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
        }
    }

    /// A game frame while Options is up: its ticks, the sound clock, black behind it (`Copy3hatterkep(0x35)`'s
    /// picture is black in the captures), the screen and the top bar.
    fn options_game_frame(&mut self) {
        self.options_tick();
        if self.options.is_none() || self.front.is_some() {
            return;
        }
        let now = self.now_ms();
        let due_ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50);
        self.ui_frames = now as u64 / 10;
        for _ in 0..due_ui {
            self.audio.tick();
        }
        self.audio.frame(human_race(&self.world.game.state));
        let Some(g) = &mut self.gpu else { return };
        let (bw, bh) = g.band();
        let size = [bw as f32, bh as f32];
        let t = now as f32 / 1000.0;
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        if let (Some(o), Some(h)) = (&mut self.options, &mut self.hud) {
            draw_options(o, h, &mut frame, size, self.start.elapsed().as_secs_f64() * 1000.0);
            h.paused = true;
            h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Options, t);
        }
        let Ok(out) = g.surface.get_current_texture() else {
            self.reconfigure();
            return;
        };
        g.present(frame, &out.texture, t);
        g.window.pre_present_notify();
        out.present();
    }

    /// `ExitToMainScreen` (exe 0x5117f0), then `AktKepKell = 0xF`: `MainScr_Belep` (the rings fly in again, the menu
    /// music). The world stays as it is until the next game replaces it.
    fn tutorial_owns_input(&self) -> bool {
        self.options.is_none() && self.tutorial.as_ref().is_some_and(|t| t.owns_input(akt_kep(if self.help.is_some() { Some(crate::ui::Screen::Help) } else { self.overlay }, self.colony.is_some(), self.spacewar.is_some())))
    }

    fn tutorial_inputs(&self) -> crate::tutorial_ui::Inputs {
        let counter = self.tutorial.as_ref().map_or(0, |t| t.counter);
        let screen = akt_kep(if self.help.is_some() { Some(crate::ui::Screen::Help) } else { self.overlay }, self.colony.is_some(), self.spacewar.is_some());
        // `Atlepes || Belep` (`WAITFORATLEPES`): approximated by the tick the screen changes (the exe holds them through
        // the screens' change animation, `IG2_gameUpdate` exe 0x4e0440; deviation 202).
        let transition = self.tutorial.as_ref().is_some_and(|t| t.screen() != screen);
        crate::tutorial_ui::Inputs { screen, counter, transition, ..Default::default() }.counts(&self.world.game.state)
    }

    /// `TutorialSetCampaignGameOptions` has made the game; `TutorialStart("main", n)` (exe 0x611f80).
    fn start_tutorial(&mut self, chapter: i32) {
        let inputs = self.tutorial_inputs();
        match crate::tutorial_ui::TutorialUi::start(&self.install, chapter, inputs) {
            Ok((mut t, effects)) => {
                t.ai_off = Some(self.world.game.state.mem.i32(ig2_sim::save::va::AI_OFF));
                t.push(&mut self.world.game.state.mem);
                self.tutorial = Some(t);
                (self.tutorial_at, self.tutorial_buttons) = (crate::tutorial_ui::PARKED, (false, false));
                self.tutorial_effects(effects);
            }
            Err(e) => eprintln!("ig2-client: tutorial {chapter}: {e}"),
        }
    }

    /// One UI tick of the tutorial: the recorded input (`Mouse_Tutorial_Kezel`, `ReadNext`) goes to the screens, an
    /// event the game raised resumes the script, then `TutorialFrissit` and its effects.
    fn tutorial_tick(&mut self) {
        let kep = akt_kep(if self.help.is_some() { Some(crate::ui::Screen::Help) } else { self.overlay }, self.colony.is_some(), self.spacewar.is_some());
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
        let Some(t) = &mut self.tutorial else { return };
        t.entered(kep, &mut self.world.game);
        t.pull(&self.world.game.state.mem);
        t.ms = now;
        let effects = t.tick(&self.install, inputs);
        t.push(&mut self.world.game.state.mem);
        let g = &t.t.gates;
        self.panels.locks = crate::starmap_panels::TutorialLocks { production: g.production, exit: g.exit_button, split: g.split };
        if let Some(c) = &mut self.colony {
            c.ui.locked = g.buildings.to_vec();
            (c.ui.no_demolish, c.ui.all_off) = (g.demolish, g.colony);
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
        }
    }

    /// What the script does outside the interpreter (re/rules/tutorial.md §3): the game's in-place writes, the
    /// starmap's selection, the clock, leaving.
    fn tutorial_effects(&mut self, effects: Vec<ig2_sim::tutorial::Effect>) {
        use ig2_sim::tutorial::Effect as E;
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
                // The music ducking isn't ported.
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
                E::Stopped { restore_overrun, .. } => {
                    if let Some(v) = self.tutorial.as_mut().and_then(|t| t.overrun.take()).filter(|_| restore_overrun) {
                        for (i, x) in v.into_iter().enumerate() {
                            self.world.game.state.mem.set_i32(0xB77230 + 4 * i as u32, x);
                        }
                    }
                }
                E::CenterFleet | E::FindFleet(_) | E::FindPlanet(_) | E::CenterPlanet | E::CenterUncolonisedPlanet => self.tutorial_centre(&e),
                E::GameSpeed(0) | E::Pause(true) => {
                    if !self.world.paused() {
                        self.send(StarmapOrder::Pause { paused: true });
                    }
                }
                E::GameSpeed(n) => {
                    self.set_speed(n);
                    if self.world.paused() {
                        self.send(StarmapOrder::Pause { paused: false });
                    }
                }
                E::Pause(false) => {
                    if self.world.paused() {
                        self.send(StarmapOrder::Pause { paused: false });
                    }
                }
                // `GOTO SCREEN n` (`AktKepKell = n`): as the F-key of that screen (F1 = 1 .. F11 = 11) when another is up.
                E::GotoScreen(n) if (1..=11).contains(&n) => {
                    let now = akt_kep(if self.help.is_some() { Some(crate::ui::Screen::Help) } else { self.overlay }, self.colony.is_some(), self.spacewar.is_some());
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
        self.front = Some(Front::new(scene, self.now_ms(), &self.settings));
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
                // The screen before is entered again (`_Belep` with `OldKep` 0xd, re/screens/options.md §3): the spy
                // screen ignores `OldKep` and plays its opening again; the starmap's `Starmap_Belep` re-inits its
                // faders (`fifo`, `starf`).
                if self.spy.is_some() {
                    self.spy = None;
                    self.toggle_spy();
                } else if self.colony.is_none() && self.overlay.is_none() {
                    self.on_starmap = false;
                }
            }
            Act::MainMenu => self.exit_to_main_menu(),
            Act::Load(n) => {
                let path = o.s.slots.get(n).and_then(Option::as_ref).map(|s| s.file.clone());
                match path.map(|p| World::load_slot(&p, self.rules, &self.tech_defaults, &self.volatile)) {
                    Some(Ok(w)) => {
                        self.start_world(w);
                        // `Options_Main`'s load: the last planet whose ground war runs (`GetWarfareStatus == 1`) opens
                        // on its colony screen (`AktKepKell 3`, back to the starmap).
                        let n = self.world.game.state.planet_count();
                        if let Some(p) = (1..=n).filter(|&p| self.world.game.warfare_status(p) == 1).last() {
                            let sel = self.selection();
                            self.select(Selection { mode: 1, planet: p, ..sel });
                            self.open_colony(p);
                        }
                    }
                    Some(Err(e)) => eprintln!("ig2-client: load: {e}"),
                    None => {}
                }
            }
            Act::Save(..) => {}
        }
    }

    /// `SaveGlobalRegistrySettings`: the screen's settings into the remake's own settings file (beside the original's
    /// `options.dat`, which stays untouched), and the rows' values for the next time.
    fn store_settings_from(&mut self, s: &crate::options::OptionsScreen) {
        let mut own = ig2_formats::options::Options::own_path().and_then(|p| std::fs::read(p).ok()).map(|b| ig2_formats::options::Options::parse(&b)).unwrap_or_default();
        s.store(&mut own);
        if let Some(p) = ig2_formats::options::Options::own_path() {
            if let Err(e) = std::fs::write(&p, own.to_bytes()) {
                eprintln!("ig2-client: {}: {e}", p.display());
            }
        }
        s.store(&mut self.settings);
        for r in &mut self.opt_rows {
            if let Some(n) = s.rows.iter().find(|n| n.key == r.key) {
                r.value = n.value;
            }
        }
    }

    /// `Options_EnterPressed`: `LoadSave(slot, name, 2)` as the remake's own save (`save<n>.ig2s`), the slots read
    /// again and back to the screen before; a failed save stays (the exe's "Failed to save game!" line).
    fn options_save(&mut self, slot: usize, name: Vec<u8>) {
        let Some(dir) = crate::options::slot_dir() else { return };
        let (y, m, d) = self.world.curr.date();
        let state = match self.world.mp_save() {
            // `LoadSave(…, 2)` writes the live starmap camera (`StarmapXStart` … `StarmapPlanetKering`, exe 0xB9A968..);
            // a multiplayer save leaves the synced state alone.
            None => {
                if let Some(v) = &self.view {
                    self.world.store_view(&v.camera);
                }
                self.world.save_file()
            }
            Some(Ok(f)) => f,
            Some(Err(e)) => {
                self.world.local_lines.push(e.into_bytes());
                if let Some(o) = &mut self.options {
                    o.s.refresh(crate::options::read_slots(Some(&dir)));
                }
                return;
            }
        };
        let file = crate::options::slot_file(state, &name, [y, m, d]);
        let path = dir.join(format!("save{slot}.ig2s"));
        let ok = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, file));
        let slots = crate::options::read_slots(Some(&dir));
        let Some(o) = &mut self.options else { return };
        match ok {
            Ok(()) => o.s.saved(slots),
            Err(e) => {
                eprintln!("ig2-client: {}: {e}", path.display());
                o.s.refresh(slots);
            }
        }
    }

    /// The screens of the game before close (a load or the main menu starts afresh).
    fn close_screens(&mut self) {
        self.colony = None;
        self.overlay = None;
        self.spy = None;
        self.diplomacy = None;
        self.research = None;
        self.stop_battle_loops();
        self.spacewar = None;
        self.cam_tween = None;
        self.cam_saved = None;
        self.panels = Panels::default();
        self.menus = Menus::default();
    }

    /// A loaded slot (`Options_Main`'s load: `LoadSave(slot, "", 1)`, `SetIdoSpeed(1, 0)`, unpaused, the starmap).
    fn start_world(&mut self, w: World<'static>) {
        self.close_screens();
        self.tutorial = None;
        self.world = w;
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
    }

    /// The battle screen's part of the frame's update (re/screens/spacewar.md): a started battle opens it, an ended one
    /// zooms out, then [`crate::spacewar_live::Live::update`].
    fn spacewar_update(&mut self, now: u32) {
        use ig2_sim::game::RealTimeEvent;
        for e in std::mem::take(&mut self.world.realtime) {
            match e {
                RealTimeEvent::SpaceStarted { slot } if self.spacewar.is_none() => {
                    let Some(cam) = self.view.as_ref().map(|v| v.camera) else { continue };
                    // `SpaceWar_Belep`: always entered from the starmap.
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
                RealTimeEvent::SpaceOver { slot, .. } => {
                    if let Some(sw) = self.spacewar.as_mut().filter(|sw| sw.ui.slot == slot) {
                        sw.over(&self.world, now);
                        self.send(StarmapOrder::Pause { paused: true });
                    }
                }
                _ => {}
            }
        }
        let Some(sw) = &mut self.spacewar else { return };
        sw.pointer = self.cursor_px;
        if let Some(g) = &self.gpu {
            let size = g.window.inner_size();
            sw.window = [size.width as i32, size.height as i32];
        }
        let out = sw.update(&mut self.world, now);
        self.spacewar_out(out);
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
            for c in sw.commands(&o) {
                self.world.send(c);
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
                    self.spacewar = None;
                    if let Some(v) = &mut self.view {
                        crate::spacewar_live::restore_view(v);
                    }
                }
                _ => {}
            }
        }
    }

    /// The battle screen's frame: its input and its picture.
    fn spacewar_frame(&mut self, now: u32, alpha: f32, size: [f32; 2], t: f32) -> Option<crate::render::Frame> {
        let sw = self.spacewar.as_mut()?;
        let (g, v, h) = (self.gpu.as_mut()?, self.view.as_mut()?, self.hud.as_ref()?);
        let mouse = v.camera.virtual_of(self.cursor);
        let (frame, out) = sw.frame(&self.world, &mut g.renderer, &self.install, &g.assets, v, h, self.tables, mouse, self.modifiers.shift_key(), now, alpha, t, size);
        self.spacewar_out(out);
        frame
    }

    /// `SpaceWarKeyboardHandler` (spec §7.1.5), with the shared keys it leaves to `DoKeyboard`: space pauses, Esc
    /// leaves the screen (`SpaceWar_Kilep`'s zoom-out) [u: `DoKeyboard` not read].
    fn spacewar_key(&mut self, event: &winit::event::KeyEvent) {
        use crate::spacewar_ui::Key as K;
        use winit::keyboard::{KeyCode, PhysicalKey};
        let down = event.state == ElementState::Pressed;
        // `DoKeyboard` (re/screens/input.md §1) first: the top row's 1..3 set the speed and unpause (sound 10) and
        // never reach the screen, Space and P pause, O and Esc (`ESC_Pressed`) open the options; the pause and speed
        // keys wait for the opening zoom (`nemlehetidotallitani`).
        let ch = match event.logical_key.as_ref() {
            Key::Character(c) => c.chars().next().map(|c| c.to_ascii_lowercase()),
            _ => None,
        };
        let digit = match event.physical_key {
            PhysicalKey::Code(c) if (KeyCode::Digit1..=KeyCode::Digit9).contains(&c) => Some(c as i32 - KeyCode::Digit0 as i32),
            _ => None,
        };
        if down {
            let free = self.multiplayer() || self.spacewar.as_ref().is_some_and(|sw| sw.clock_free());
            if matches!(event.logical_key.as_ref(), Key::Named(NamedKey::Space)) || ch == Some('p') {
                if free {
                    self.toggle_pause();
                }
                return;
            }
            if let Some(n @ 1..=3) = digit {
                if free && !self.speed_locked() {
                    if !self.multiplayer() {
                        self.sounds.push(crate::audio::SoundId::global(10));
                    }
                    self.send(StarmapOrder::Speed { level: n });
                    self.send(StarmapOrder::Pause { paused: false });
                }
                return;
            }
            if matches!(event.logical_key.as_ref(), Key::Named(NamedKey::Escape)) || ch == Some('o') {
                if !event.repeat && self.spacewar.as_ref().is_some_and(|sw| sw.clock_free()) {
                    self.open_options(false);
                }
                return;
            }
        }
        // `SpaceWarKeyboardHandler`: the letters by the typed character (the char events), the rest by key code.
        let k = match (event.logical_key.as_ref(), ch, digit) {
            (Key::Named(NamedKey::ArrowLeft), ..) => Some(K::Left),
            (Key::Named(NamedKey::ArrowRight), ..) => Some(K::Right),
            (Key::Named(NamedKey::ArrowUp), ..) => Some(K::Up),
            (Key::Named(NamedKey::ArrowDown), ..) => Some(K::Down),
            _ if event.physical_key == PhysicalKey::Code(KeyCode::NumpadAdd) => Some(K::ZoomIn),
            _ if event.physical_key == PhysicalKey::Code(KeyCode::NumpadSubtract) => Some(K::ZoomOut),
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
        if let Some(mp) = &mut f.mp {
            let escape = matches!(k.as_ref(), Key::Named(NamedKey::Escape));
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
        if f.s.name.is_none() {
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
            Key::Character(c) => c.bytes().next().filter(|b| b.is_ascii()).map(EntryKey::Char),
            _ => None,
        };
        if let Some(a) = key.and_then(|key| f.s.name_key(key)) {
            self.front_act(a);
        }
    }

    /// `st_ShowAnim` for each name in turn (`OpenBink(name, 1, 0, 0, force)`, `PlayBink(1, 0, 0)`, `CloseBink`);
    /// `then` once the last ends.
    fn queue_videos(&mut self, names: impl IntoIterator<Item = String>, force: bool, then: AfterVideo) {
        self.video_queue.extend(names.into_iter().map(|n| if force { format!("!{n}") } else { n }));
        self.after_video = then;
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
        match (reqs.iter().any(crate::video::Request::ends_game), names.is_empty()) {
            (true, true) => self.exit_to_main_menu(),
            (true, false) => self.queue_videos(names, false, AfterVideo::MainMenu),
            (false, _) => self.queue_videos(names, false, AfterVideo::Resume),
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
                self.audio.play_video(&v.player, true);
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
        let open = self.overlay == Some(crate::ui::Screen::Messages);
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
        if let Some(p) = &mut self.head.playback {
            p.frame(renderer, &self.install, now as u64, size.1);
            let win = [size.0 as f32, size.1 as f32];
            h.clip = p.quad(win, &crate::font::Virtual::new(win));
        }
    }

    fn frame(&mut self) {
        self.take_video_requests();
        if self.video.is_some() || !self.video_queue.is_empty() {
            self.video_frame();
            if self.video.is_some() {
                return;
            }
        }
        if self.front.is_some() {
            self.front_frame();
            return;
        }
        if self.options.is_some() {
            self.options_game_frame();
            return;
        }
        if self.help.is_some() {
            self.help_game_frame();
            return;
        }
        let now = self.now_ms();
        let sim = crate::prof::scope("sim");
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
        self.world.game.set_transfer(self.panels.transfer.map(|t| (t.f1, t.f2)));
        let due = self.clock.advance(now);
        // Multiplayer: the session is pumped while paused too (its keep-alive and the others' commands).
        if (!self.world.paused() || self.world.net.is_some()) && self.headless.is_none() {
            for _ in 0..due {
                self.world.tick();
                if let Some(t) = &mut self.tutorial {
                    t.counter += 1;
                }
            }
        }
        drop(sim);
        let ui = crate::prof::scope("ui");
        // Multiplayer: the session ended under this machine (`ExitToMainScreen` from `MULTIGET`'s disconnect).
        if self.world.net_over {
            self.world.net_over = false;
            self.exit_to_main_menu();
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
        self.world.game.set_transfer(self.panels.transfer.map(|t| (t.f1, t.f2)));
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
        if let Some(h) = &mut self.hud {
            let world = &mut self.world;
            let on_messages = self.overlay == Some(crate::ui::Screen::Messages);
            let now_ms = now as f64;
            for line in world.chat_lines.drain(..) {
                h.chat.ring.add(line, crate::chat::CHAT_COLOUR, now_ms);
            }
            for _ in 0..due_ui {
                h.scroller.step(&mut || world.pop_message());
                h.info.ui_frame(&world.ui.info, world.ui.human);
                h.messages.ui_frame();
                // `FRISSIT_Messages2`: every 11th UI frame on the screen, the automatic messages.
                self.messages_auto = (self.messages_auto + 1) % 11;
                if on_messages && self.messages_auto == 0 {
                    world.send(ig2_sim::game::MessageOrder::Auto.into());
                }
            }
            if on_messages {
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
                d.advance(due_ui as f32 * 10.0);
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
        // not under the long info, the zoom or a tutorial; the minimap held retargets every frame.
        let scroll = self.colony.is_none() && self.spacewar.is_none() && !self.under_screen() && !self.menus.info_on() && self.cam_saved.is_none() && self.tutorial.is_none();
        let win = self.gpu.as_ref().map_or([640, 480], |g| {
            let s = g.window.inner_size();
            [s.width, s.height]
        });
        if let Some(v) = &mut self.view {
            for _ in 0..due_ui {
                self.star_keys.tick(&mut v.camera);
                // Gated off, the speeds keep their values (the exe skips the whole function). [deviation: the port
                // reads the pointer only while it is over the window; the exe keeps the last physical position]
                if scroll {
                    let held = self.mouse_held != 0 || !self.pointer_in;
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
            if !self.on_starmap {
                self.panels.faders = crate::starmap_panels::StarmapFaders::enter();
            }
            for _ in 0..due_ui {
                v.ui_tick();
                self.panels.ui_tick();
            }
            self.on_starmap = true;
        } else {
            self.on_starmap = false;
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
        let win = self.gpu.as_ref().map_or([640, 480], |g| {
            let s = g.window.inner_size();
            [s.width, s.height]
        });
        // The colony screen draws the game's pointer (`UpdateFrame`): the window's own is hidden there.
        let hide = self.colony.is_some() && self.overlay.is_none() && self.spy.is_none();
        if hide != self.cursor_hidden
            && let Some(g) = &self.gpu
        {
            g.window.set_cursor_visible(!hide);
            self.cursor_hidden = hide;
        }
        if let (Some(c), Some(s)) = (&mut self.colony, &self.world.colony_curr) {
            let shown = self.overlay.is_none() && self.spy.is_none();
            for _ in 0..due_ui {
                // `Frissit2_Colony` runs every UI tick (the camera keys); `Colony_Autoscroll` only on the screen.
                c.ui.cam_keys.tick(&mut c.view.camera);
                // [deviation: the port scrolls only while the pointer is over the window; the exe keeps the last
                // physical position]
                if shown && self.pointer_in {
                    c.ui.cam_keys.autoscroll(&mut c.view.camera, self.cursor_px, win, self.mouse_held != 0);
                } else {
                    c.ui.cam_keys.stop_scroll();
                }
                // `Frissit2_Colony`'s ambients and music only with the screen shown (`AktKep == 3`).
                if shown {
                    c.ui.ui_frame(s, self.world.curr.human as i32, &mut c.view.camera);
                    if let Some(m) = c.ui.war_music(s.at_war) {
                        self.audio.play_music(m, false, human_race(&self.world.game.state));
                    }
                }
            }
            // `TransformRotation`, once a drawn frame in `ShowColony`.
            if shown {
                c.ui.cam_keys.frame(&mut c.view.camera);
            }
            c.view.overlay = c.ui.overlay(s, self.world.curr.human as i32);
        }
        if let Some(r) = &mut self.research {
            if std::mem::take(&mut r.picked_turret) {
                self.world.game.tutorial_check_event(8);
            }
            for _ in 0..due_ui {
                r.step(&self.world.game.state);
            }
            if r.closed() {
                self.research = None;
                self.overlay = None;
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
            }
        }
        if let (Some(r), Some(m)) = (&mut self.research, &self.research_models) {
            r.tip.moved(self.view.as_ref().map_or([0.0; 2], |v| v.camera.virtual_of(self.cursor)));
            r.tooltip(&self.world.game.state, &m.mask, &self.research_tips, now as f64);
        }
        // The sounds queued by the frame's input and by these UI ticks, then the ticks' music steps.
        self.audio_frame(due_ui);
        self.row_frame_phase(due_ui);
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
        drop(ui);
        let (Some(g), Some(v)) = (&mut self.gpu, &mut self.view) else { return };
        let t = now as f32 / 1000.0;
        let size = { let (w, h) = g.band(); [w as f32, h as f32] };
        let offscreen = crate::prof::scope("offscreen");
        let panel_tex = match (&mut self.panel_models, self.colony.is_none() && self.overlay.is_none() && self.panels.active()) {
            (Some(m), true) => m.render(&mut g.renderer, &self.install, self.tables, &self.panels, &self.world.game.state, &sel, size),
            _ => None,
        };
        if self.spacewar.is_some() {
            let Some(mut frame) = self.spacewar_frame(now, alpha, size, t) else { return };
            let (Some(g), Some(h)) = (&mut self.gpu, &mut self.hud) else { return };
            h.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
            h.paused = paused;
            h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Starmap, t);
            if let Ok(out) = g.surface.get_current_texture() {
                g.present(frame, &out.texture, t);
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
        let mut colony_info = None;
        // The diplomacy screen: its UI ticks, the frame's input (its commands go to the queue), then its frame.
        if let (Some(d), Some(h), Some(crate::ui::Screen::Diplomacy)) = (&mut self.diplomacy, &self.hud, self.overlay) {
            for _ in 0..due_ui {
                d.screen.ui_tick();
            }
            if d.screen.closed() {
                self.overlay = self.dip_next.take();
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
                colony_panel = c.panel.render(&mut g.renderer, &self.install, self.tables, c.ui.kind, unit, c.ui.spin(), size);
                let w = crate::colony_war::WarCtx { snap: curr, en: self.world.curr.human as i32, tutorial_paused: false };
                colony_info = c.ui.war.info_model(&w).and_then(|(u, turn)| c.scene.render_info_model(&mut g.renderer, &self.install, &self.tables.unit_models, u, turn, size));
                (c.view.build(&mut g.renderer, &self.install, self.tables, self.maps, &mut c.scene, prev, curr, alpha, t), crate::ui::Screen::Colony)
            }
            _ => {
                v.hover = v.pick(&self.world.prev, &self.world.curr, alpha, self.cursor);
                (v.build(&g.assets, &self.world.prev, &self.world.curr, alpha, t), crate::ui::Screen::Starmap)
            }
        };
        drop(scene);
        let hud = crate::prof::scope("hud");
        if let Some(h) = &mut self.hud {
            h.speed = (self.world.game.speed_level() - 1).clamp(0, 2) as usize;
            h.paused = paused;
            match (screen, &self.colony, &self.world.colony_curr) {
                (crate::ui::Screen::Colony, Some(c), Some(curr)) => {
                    let cursor = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
                    let input = crate::ui::ColonyInput { ui: &c.ui, sim: self.world.game.rules, panel: colony_panel, cursor, pointer: self.pointer_in, info_model: colony_info, now_ms: now, time: t };
                    h.draw_colony(&mut frame, &self.world.game.state, self.tables, self.maps, &c.view, &c.scene, curr, &input);
                }
                (crate::ui::Screen::Starmap, ..) => {
                    h.draw_starmap_names(&mut frame, v, &g.assets, &self.world.prev, &self.world.curr, alpha, t);
                    let lv = self.world.game.local_view();
                    let sel = Selection { mode: lv.starmap_mode, planet: lv.planet.max(0) as usize, fleet: lv.fleet.max(0) as usize };
                    h.draw_starmap_hud(&mut frame, &self.world.game.state, self.tables, v, &self.world.prev, &self.world.curr, &sel, alpha, t, &self.panels.faders.starf, self.menus.info_box(v.camera.zoom));
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
            // `GFX_DateMoneyScrollFade`'s fader pass 3 and `Update3D`'s `TutorialSzovegFrissit`: the window over the
            // screen; the pointer the recording moves (`UpdateFrame`).
            if let Some(tu) = &self.tutorial {
                let kep = akt_kep(if self.help.is_some() { Some(crate::ui::Screen::Help) } else { self.overlay }, self.colony.is_some(), self.spacewar.is_some());
                h.draw_tutorial(&mut frame, tu, size, kep);
                if tu.owns_input(kep) && self.tutorial_kind >= 0 {
                    h.draw_pointer(&mut frame, size, self.tutorial_at, self.tutorial_kind);
                }
            }
        }
        drop(hud);
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
        g.present(frame, &out.texture, t);
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
            g.window.set_title(&format!("Imperium Galactica II — {y}-{m:02}-{d:02} {state} — {fps:.0} fps"));
            self.title_at = Instant::now();
            self.frames = 0;
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

/// A recorded key (`ReadNext`'s slots: kind 0 a character, 1 a key going down, 2 up; Windows key codes) as the
/// client's key: the characters, and the keys that make none.
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
        // `ig2screen::anyEvent` (exe 0x7329d0) passes a button's or a finger's release on to the game's handler
        // (`MainCallBackEventHandler` 0x4e2db0) also while a fullscreen video plays: after the video the game sees the
        // button up (a press that started the video isn't left held).
        if let (Some(v), WindowEvent::MouseInput { state: ElementState::Released, .. }) = (&mut self.video, &event) {
            v.release();
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
            WindowEvent::CursorMoved { position, .. } if self.help.is_some() => {
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let (bw, bh) = self.gpu.as_ref().map_or((640, 480), |g| g.band());
                let at = crate::font::Virtual::new([bw as f32, bh as f32]).virtual_of([position.x as f32, position.y as f32 - top]);
                if let Some(h) = &mut self.help {
                    h.s.hover(at);
                }
            }
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
                    k @ Key::Named(NamedKey::F2 | NamedKey::F3 | NamedKey::F4 | NamedKey::F5 | NamedKey::F6 | NamedKey::F7 | NamedKey::F8
                    | NamedKey::F9 | NamedKey::F10)
                    | k @ Key::Character("o" | "O" | "m" | "M") => {
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
            WindowEvent::KeyboardInput { event, .. } if self.options.is_some() => {
                if event.state == ElementState::Pressed {
                    self.options_key(&event.logical_key);
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
                if event.state == ElementState::Pressed {
                    self.front_key(&event.logical_key);
                }
            }
            // Our MP screens: either button's press.
            WindowEvent::MouseInput { state: ElementState::Pressed, button: button @ (MouseButton::Left | MouseButton::Right), .. } if self.front.as_ref().is_some_and(|f| f.mp.is_some()) => {
                let Some(f) = &mut self.front else { return };
                let at = f.at;
                let Some(mp) = &mut f.mp else { return };
                if let Some(o) = mp.press(at, button == MouseButton::Right, &mut f.net) {
                    self.front_mp_out(o);
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
            // While the script plays (`MainCallBackEventHandler`): the live pointer and buttons are dropped; `t` shows or
            // hides the window, `o` opens the options screen.
            // The OS pointer is still tracked (`MainCallBackEventHandler` stores the raw position first).
            WindowEvent::CursorMoved { position, .. } if self.tutorial_owns_input() => {
                self.cursor_px = [position.x as i32, position.y as i32];
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
                        Key::Character("t" | "T") => {
                            if let Some(t) = &mut self.tutorial {
                                t.t.gates.text_shown ^= true;
                            }
                        }
                        Key::Character("o" | "O") => self.open_options(false),
                        _ => {}
                    }
                }
            }
            // Paused (live input): `t` still shows or hides the window and `o` opens the options screen, then the key goes
            // on to the screen.
            WindowEvent::KeyboardInput { ref event, .. }
                if event.state == ElementState::Pressed && matches!(event.logical_key.as_ref(), Key::Character("t" | "T" | "o" | "O")) && self.tutorial.as_ref().is_some_and(|t| t.running()) && self.options.is_none() =>
            {
                if matches!(event.logical_key.as_ref(), Key::Character("o" | "O")) {
                    self.open_options(false);
                    return;
                }
                if let Some(t) = &mut self.tutorial {
                    t.t.gates.text_shown ^= true;
                }
                if self.spacewar.is_some() {
                    self.spacewar_key(event);
                } else if !self.colony_camera_key(event) && !self.starmap_key(event) && self.key(event.logical_key.as_ref()) {
                    el.exit();
                }
            }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::CursorLeft { .. } => self.pointer_in = false,
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_px = [position.x as i32, position.y as i32];
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
                let mut bar = None;
                let mp = self.multiplayer();
                if let Some(sw) = &mut self.spacewar {
                    let down = state == ElementState::Pressed;
                    match button {
                        MouseButton::Left => sw.left = down,
                        MouseButton::Right => sw.right = down,
                        _ => {}
                    }
                    bar = if down && button == MouseButton::Left { sw.bar_press(at, mp) } else { None };
                }
                match bar {
                    Some(0) => self.toggle_pause(),
                    Some(n) => self.set_speed(n),
                    None => {}
                }
            }
            WindowEvent::KeyboardInput { event, .. } if self.spacewar.is_some() => self.spacewar_key(&event),
            WindowEvent::KeyboardInput { event, .. } if self.colony_camera_key(&event) || self.trade_key(&event) || self.starmap_key(&event) => {}
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
                    Some(crate::ui::Screen::Messages) => h.messages.hover(at),
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
            let gm = self.colony.as_ref().zip(self.world.colony_curr.as_ref()).and_then(|(c, s)| self.maps.get(s.map).map(|gm| (c, gm)));
            let gm_cell = gm.and_then(|(c, gm)| crate::colony_ui::cell_under(&c.view, gm, p));
            let start = gm.and_then(|(c, gm)| crate::colony_ui::cursor_cell_under(&c.view, gm, p));
            self.colony_input(|ui, ctx, at, _| {
                ui.drag(ctx, at);
                ui.hover(ctx, gm_cell, start);
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
            // Not under the long info, the zoom or the fleet's panel (as the exe's camera keys, re/deviations.md #43).
            if !self.menus.info_on() && self.cam_saved.is_none() && !self.panels.fleet_info() {
                v.camera.pan(d, p);
            }
            self.drag = Some(p);
        }
        self.cursor = p;
    }

    /// A 2D screen (`overlay`) or the spy screen is up over the starmap or the colony.
    fn under_screen(&self) -> bool {
        self.overlay.is_some() || self.spy.is_some()
    }

    /// The wheel turned by `steps` notches.
    fn wheel(&mut self, steps: f32) {
        // None of the 2D screens (`Design_Main`, `Trade_Main`, `Research_Main`, …, `Spy_Main`) has a wheel.
        if self.under_screen() {
            return;
        }
        if let Some(c) = &mut self.colony {
            c.view.camera.zoom_steps(steps);
        } else if let (Some(v), false) = (&mut self.view, self.menus.info_on() || self.cam_saved.is_some() || self.panels.fleet_info()) {
            v.camera.zoom_steps(steps);
        }
    }

    /// A mouse button went down or up.
    fn mouse(&mut self, button: MouseButton, pressed: bool) {
        let state = if pressed { ElementState::Pressed } else { ElementState::Released };
        let bit = match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            _ => 0,
        };
        self.mouse_held = if pressed { self.mouse_held | bit } else { self.mouse_held & !bit };
        if button != MouseButton::Left && self.spy.is_some() {
            // `Spy_Main` has no right-button input; nothing reaches the starmap under it.
            self.drag = None;
            self.right_down = None;
            return;
        }
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
        if on_starmap && pressed && self.spacewar.is_none() {
            let at = self.virtual_cursor();
            let on_box = at[0] >= 0.0 && at[0] < 166.0 && at[1] >= 424.0 && !self.menus.info_on();
            // `Starmap_Main`: a press ends the rename, the old name back unless it is on the info box.
            if let Some(r) = self.menus.rename.take() {
                if !on_box {
                    self.menus.rename = Some(r.clone());
                    self.write_rename(&r.backup);
                    self.menus.rename = None;
                }
            }
            if on_box && self.mouse_held == 3 {
                self.left_down = None;
                self.right_down = None;
                self.drag = None;
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
            // The other 2D screens' right button doesn't reach the starmap or colony under them (no camera drag, no
            // order).
            (MouseButton::Right, _) if self.overlay.is_some() => {
                self.drag = None;
                self.right_down = None;
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
            (MouseButton::Left | MouseButton::Right, ElementState::Released) if self.console_held.is_some_and(|(_, l)| l == (button == MouseButton::Left)) => {
                self.menus.held = false;
                self.console_release();
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
            (MouseButton::Right, ElementState::Pressed) if on_starmap && self.console_held.is_none() && self.console_press(false) => {}
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

    /// `ColonyKeyboardHandler`'s held camera keys (spec §11.9), down and up: the arrows turn and tilt, numpad + / −
    /// zoom. True when taken.
    fn colony_camera_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        self.colony_camera_key_by(event.physical_key, event.logical_key.as_ref(), event.state == ElementState::Pressed)
    }

    fn colony_camera_key_by(&mut self, physical: winit::keyboard::PhysicalKey, _logical: Key<&str>, down: bool) -> bool {
        use winit::keyboard::{KeyCode, PhysicalKey};
        if self.overlay.is_some() || self.spy.is_some() {
            return false;
        }
        let Some(c) = &mut self.colony else { return false };
        let k = &mut c.ui.cam_keys;
        let flag = match physical {
            PhysicalKey::Code(KeyCode::ArrowLeft) => &mut k.left,
            PhysicalKey::Code(KeyCode::ArrowRight) => &mut k.right,
            PhysicalKey::Code(KeyCode::ArrowUp) => &mut k.up,
            PhysicalKey::Code(KeyCode::ArrowDown) => &mut k.down,
            PhysicalKey::Code(KeyCode::NumpadAdd) => &mut k.zoom_in,
            PhysicalKey::Code(KeyCode::NumpadSubtract) => &mut k.zoom_out,
            _ => return false,
        };
        *flag = down;
        true
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
        if self.view.is_none() || self.colony.is_some() || self.under_screen() || self.menus.rename.is_some() {
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
        if !down || self.tutorial.as_ref().is_some_and(|t| t.t.gates.starmap) {
            return false;
        }
        match logical.clone() {
            Key::Named(k @ (NamedKey::PageUp | NamedKey::PageDown)) => {
                self.starmap_step(k == NamedKey::PageDown);
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
        self.menus.rename = Some(r);
    }

    /// The rename's text written into the record (`IOString` is the record's name).
    fn write_rename(&mut self, text: &[u8]) {
        let Some(r) = &self.menus.rename else { return };
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
                // 3 for a planet (0x15), then `IOMode = 0`.
                if let Some(r) = self.menus.rename.take() {
                    self.world.game.tutorial_check_event(if r.fleet { 4 } else { 3 });
                }
                return;
            }
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
                match b.first().copied().filter(|&b| b >= 0x20) {
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
        match frame {
            Some(f) if owner == self.dk_owner => {
                let [m, c, n] = f.phase();
                self.dk = crate::kit::DesignFrame::with_phase(m, c, n);
            }
            Some(f) => {
                for _ in 0..due_ui {
                    self.dk.tick();
                }
                let [m, c, n] = self.dk.phase();
                *f = crate::kit::DesignFrame::with_phase(m, c, n);
            }
            None => {
                for _ in 0..due_ui {
                    self.dk.tick();
                }
            }
        }
        self.dk_owner = owner;
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

    /// A key went down; true = quit.
    fn key(&mut self, key: Key<&str>) -> bool {
        if self.chat_key(&key) {
            return false;
        }
        match key {
            // The starmap's rename takes the keys (`IOMode`).
            _ if self.menus.rename.is_some() && self.colony.is_none() && !self.under_screen() => self.rename_key(&key),
            // `DoKeyboard`: F1 asks for `AktKep` 1, the Help screen (re/screens/help.md §1).
            Key::Named(NamedKey::F1) => self.open_help(),
            // `DoKeyboard`: F2 asks for the starmap (the closing runs, `Design_Kilep`); F5 on the screen asks for itself.
            // Esc in the viewing mode goes to the options screen at once (`AktKep` 0xd, no closing).
            Key::Named(NamedKey::F2) if self.overlay == Some(crate::ui::Screen::Design) => self.toggle_design(),
            Key::Named(NamedKey::F5) if self.overlay == Some(crate::ui::Screen::Design) => {}
            // `DesignKeyboardEscPressed`: the name input ends, an edit is cancelled; else the options screen.
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Design) => {
                if self.design_do(|d, c, _| d.escape(c)) != Some(true) {
                    self.open_options(false);
                }
            }
            // The name input (`IOMode`, `IOAzonosito` 0x33; exe `DoKeyboard` 0x4db1a0): Enter ends it
            // (`DesignEntertNyomott`, no `DesignKeyboardHandler` after it); the other keys, Space, the digits and the
            // arrows included, go to the input or nowhere. Only the F-keys (`DoKeyboard`'s block before it) pass.
            _ if self.overlay == Some(crate::ui::Screen::Design)
                && self.design.as_ref().is_some_and(|d| d.entry.is_some())
                && !matches!(key, Key::Named(NamedKey::F1 | NamedKey::F2 | NamedKey::F3 | NamedKey::F4 | NamedKey::F5 | NamedKey::F6
                    | NamedKey::F7 | NamedKey::F8 | NamedKey::F9 | NamedKey::F10 | NamedKey::F11)) =>
            {
                self.design_entry_key(&key)
            }
            Key::Named(NamedKey::Enter) if self.overlay == Some(crate::ui::Screen::Design) => {
                self.design_do(|d, c, _| d.edit_key(c, false));
            }
            Key::Character("r") if self.overlay == Some(crate::ui::Screen::Design) => {
                self.design_do(|d, c, _| d.edit_key(c, true));
            }
            // The trade screen has no keyboard handler: F2 asks for the starmap, Esc for the options screen.
            Key::Named(NamedKey::F2) if self.overlay == Some(crate::ui::Screen::Trade) => self.toggle_trade(),
            // `DoKeyboard`'s F2 (the starmap) from the diplomacy screen: `Diplomacy_Kilep`'s closing first.
            Key::Named(NamedKey::F2) if self.overlay == Some(crate::ui::Screen::Diplomacy) => {
                if self.leave_diplomacy(None) {
                    self.overlay = None;
                }
            }
            Key::Named(NamedKey::F8) if self.overlay == Some(crate::ui::Screen::Trade) => {}
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Trade) => {
                self.trade_under_options = self.trade.take();
                self.overlay = None;
                self.open_options(false);
            }
            // F5 on the trade screen: `Trade_Kilep` towards the design screen (the closing first), then F5 again opens it.
            Key::Named(NamedKey::F5) if self.overlay == Some(crate::ui::Screen::Trade) => {
                if self.trade.as_mut().is_none_or(|t| t.leave(5)) {
                    self.trade = None;
                    self.overlay = None;
                    self.toggle_design();
                }
            }
            // The other screens' keys do nothing here (no `DispatchKeyboard` target); `DoKeyboard`'s speed keys and
            // Space stay.
            Key::Character(c) if self.overlay == Some(crate::ui::Screen::Trade) && !matches!(c, "1" | "2" | "3" | "p" | "P") => {}
            Key::Named(NamedKey::Tab | NamedKey::PageUp | NamedKey::PageDown | NamedKey::Delete) if self.overlay == Some(crate::ui::Screen::Trade) => {}
            Key::Named(NamedKey::F8) => self.toggle_trade(),
            Key::Named(NamedKey::F5) => self.toggle_design(),
            // `DoKeyboard`'s other F-keys (`AktKepKell = key − 0x6f`, nothing on the screen asked for), the way the
            // client's own screen keys go; the design and trade screens keep only theirs (re/deviations.md).
            Key::Named(f @ (NamedKey::F3 | NamedKey::F4 | NamedKey::F6 | NamedKey::F7 | NamedKey::F9 | NamedKey::F10))
                if !matches!(self.overlay, Some(crate::ui::Screen::Design | crate::ui::Screen::Trade)) =>
            {
                use crate::ui::Screen;
                match f {
                    NamedKey::F3 if self.colony.is_none() => self.toggle_colony(),
                    NamedKey::F4 if self.overlay != Some(Screen::Research) => self.toggle_research(),
                    NamedKey::F6 if self.overlay != Some(Screen::Diplomacy) => self.toggle_diplomacy(),
                    NamedKey::F7 if self.spy.is_none() => self.toggle_spy(),
                    NamedKey::F9 if self.overlay != Some(Screen::Info) => self.toggle_screen(Screen::Info),
                    NamedKey::F10 if self.overlay != Some(Screen::Messages) => self.toggle_screen(Screen::Messages),
                    _ => {}
                }
            }
            // The starmap's and the other screens' own keys do nothing here (no `DispatchKeyboard` target); the speed
            // keys, P and Space are `DoKeyboard`'s and stay.
            Key::Character(c) if self.overlay == Some(crate::ui::Screen::Design) && !matches!(c, "1" | "2" | "3" | "p" | "P") => {}
            Key::Named(NamedKey::Tab | NamedKey::PageUp | NamedKey::PageDown | NamedKey::Delete) if self.overlay == Some(crate::ui::Screen::Design) => {}
            Key::Named(k @ (NamedKey::ArrowUp | NamedKey::ArrowDown)) if self.overlay == Some(crate::ui::Screen::Design) => {
                if let Some(d) = &mut self.design {
                    d.key(if k == NamedKey::ArrowUp { crate::design::Key::Up } else { crate::design::Key::Down });
                }
            }
            // Esc on the messages screen does nothing while answers are pending or the text is typing.
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Messages) => {
                if self.hud.as_ref().is_none_or(|h| h.messages.open.as_ref().is_none_or(|d| d.answers.is_empty() && d.typed_all)) {
                    self.overlay = None;
                }
            }
            // `MessagesKeyboardHandler` (exe 0x507eb0): a..d answer; M asks for the screen it is on (nothing).
            Key::Character(c @ ("a" | "b" | "c" | "d")) if self.overlay == Some(crate::ui::Screen::Messages) => {
                if let Some(h) = &mut self.hud {
                    h.messages.key(c);
                }
            }
            Key::Character("m" | "M") if self.overlay == Some(crate::ui::Screen::Messages) => {}
            // The ground war's keys (`ColonyKeyboardHandler`, colony.md §11.2) while the shown planet is at war.
            Key::Character(ch) if self.colony_war_key(ch).is_some() => {
                if let Some(k) = self.colony_war_key(ch) {
                    self.colony_input(move |ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::War(k)));
                }
            }
            // `ESC_Pressed` (ELF 0x581440) on every other game screen: Options (re/screens/options.md §5.1).
            Key::Named(NamedKey::Escape) | Key::Character("o" | "O") => self.open_options(false),
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
            Key::Character("e" | "E") if self.colony.is_some() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Priority)),
            Key::Named(NamedKey::Delete) if self.colony.is_some() && self.modifiers.shift_key() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Demolish)),
            // `ColonyKeyboardHandler` (exe 0x462caa / 0x462df6): `TutorialCheckEvent(0x1b)` before the step.
            Key::Named(NamedKey::PageUp) if self.colony.is_some() => {
                self.world.game.tutorial_check_event(0x1b);
                self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Prev));
            }
            Key::Named(NamedKey::PageDown) if self.colony.is_some() => {
                self.world.game.tutorial_check_event(0x1b);
                self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Next));
            }
            // `ColonyKeyboardHandler`: I is the planet info mode on the colony screen (colony.md §12.1).
            Key::Character("i" | "I") if self.colony.is_some() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Info)),
            Key::Character("i" | "I") => self.toggle_screen(crate::ui::Screen::Info),
            Key::Character("m" | "M") => self.toggle_screen(crate::ui::Screen::Messages),
            Key::Character("d" | "D") => self.toggle_diplomacy(),
            Key::Named(NamedKey::Tab) => {
                if let Some(h) = &mut self.hud {
                    h.info.chart = (h.info.chart + 1) % 10;
                }
            }
            // `ColonyKeyboardHandler`: `c` (ch 99 only) cycles the camera presets on the colony screen.
            Key::Character("c") if self.colony.is_some() && self.overlay.is_none() && self.spy.is_none() => {
                if let Some(c) = &mut self.colony {
                    c.ui.cam_keys.preset();
                }
            }
            // `ColonyKeyboardHandler` takes only the lower-case c.
            Key::Character("c" | "C") if self.colony.is_none() => self.toggle_colony(),
            // `DoKeyboard`: Space and P pause.
            Key::Named(NamedKey::Space) | Key::Character("p" | "P") => self.toggle_pause(),
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
            // The starmap's own keys (the camera, PgUp/PgDn, `c`) are `starmap_key`'s; R acts only on the starmap itself.
            Key::Character("r" | "R") if self.under_screen() || self.colony.is_some() => {}
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
