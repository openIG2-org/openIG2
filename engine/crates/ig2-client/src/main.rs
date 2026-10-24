//! IG2 game client (ADR 0006): loads an original save into the simulation, runs it at the original's 20 Hz and
//! renders the starmap at the display's refresh rate.
//!
//! Window:   `ig2-client [--save <file.sav>] [--speed 1|2|3]`: the main menu (re/screens/mainscr.md; its load ring opens
//!           the Options screen's slots, re/screens/options.md), or the game of `--save` at once.
//! Main menu: `ig2-client --snapshot <out.png> --screen main [--ui-frames N] [--mouse X,Y] [--script S]` (no save).
//! Options:  `ig2-client --snapshot <out.png> [--save <file.sav>] --screen options [--ui-frames N] [--mouse X,Y] [--script S]`.
//! Headless: `ig2-client --snapshot <out.png> [--save <file.sav>] [--speed 1|2|3] [--ticks N] [--alpha A]
//!            [--size WxH] [--letterbox] [--scale S] [--hover X,Y] [--select-fleet N] [--reveal]`
//!           runs N sim ticks (default 0) and renders the frame `A` (0..1) of the way from tick N−1 to N;
//!           `--reveal` draws the fleets the human doesn't see too; `--colony P` draws planet P's colony screen
//!           (`--colony-cam X,Y,ZOOM,PITCH,ROTATE` moves its camera); `--screen info|messages|research [--chart C]
//!           [--open-message I | --show-message ID]` draws the info screen, the messages screen (inbox entry I or
//!           message ID open) or the research screen (after `--ui-frames` frames); `--ui-frames N` runs the message line N frames of 10 ms (default 150); `--paused`
//!           shows the clock paused; `--panel production|transfer:F1,F2[,tank]` opens the starmap's production view
//!           (the selected planet's) or the transfer panel between holders F1 and F2 (a planet is −p);
//!           `--planet-info` shows the selected planet's long info, `--rop-menu X,Y` holds the radial menu open at
//!           the virtual point, `--menu tax|autobuild|priority|sale:N|planets@X,Y` a select menu with its box at
//!           X,Y, `--mouse X,Y` the pointer (the menus' hover, a hot-spot's tooltip); `--screen spy` the spy screen,
//!           `--spy-frame F [--spy-closing]` held at frame F of its opening (closing); the research screen takes the
//!           same as `--anim-frame F [--anim-closing]` and `--script S` (an input script, one `OUT-t<T>.png` per
//!           grab, re/screens/sound.md §3); `--tutorial-window FILE` draws the tutorial's window from a probe
//!           record (re/rules/tutorial.md §2); `--clouds N` drifts the starmap's clouds as after N UI ticks
//!           on the starmap (the exe's `GomolygasCounter`; default 0); `--futofeny F` pins the starmap's UI clock
//!           (star twinkle, pulse and the planets' spin, fleet sprite frames; default ticks × 5); `--acttime S` the
//!           remaster's clock in seconds (the long info planet's cloud drift; default the frame time); `--info-turn K`
//!           the info screen's `infokeringeto` (its side lights and chart sweeps; default 0); `--info-list P,H,C`
//!           its list's first row, problem step and `Counter`, `--info-globes S,P0..P8,F0..F8;…` the list slots'
//!           spins, model rotations and positions (the exe's at a capture, re/screens/info.md §6); `--info-detail
//!           E,S,L,P` the detail mode on entry E (slot S, leaving, design pointer) with `--info-texts H,OC,OP`;
//!           `--info-star F` the star log at `infoframe` F and `--info-log FILE` its records (§6.8);
//!           `--info-select K,X,Y,I` the planet-settings menu of hot-spot K open at (X, Y) on item I (§7.1).
//! Info:     `ig2-client --info [--save <file.sav>] [--ticks N]` prints the starmap's planets and fleets.
//!
//! The install comes from `IG2_INSTALL_DIR`; the language from `--lang en|hu|de|fr|es|it`, else `IG2_LANG`, else the one
//! the GOG ig2.exe would pick on this machine (`ig2_formats::locale`, re/formats/locale.md). The default save is `quick-1.sav` in `IG2_SAVE_DIR` (else the
//! repository's `_local/saves`). Saves, installs and rendered PNGs hold game data: keep them out of the repo.

mod assets;
mod audio;
mod backdrop;
mod chat;
mod colony;
mod colony_ui;
mod colony_war;
mod diplomacy;
mod diplomacy_scene;
mod font;
mod help;
mod hud;
mod info_screen;
mod kit;
mod lighting;
mod mainscr;
mod mainscr_scene;
mod mainscr_race;
mod mainscr_scenario;
mod messages_screen;
mod mp_lobby;
mod new_game;
mod options;
mod orders;
mod prof;
mod render;
mod render3d;
mod research;
mod design;
mod design_edit;
mod design_pods;
mod design_scene;
mod rules;
mod scene;
mod script;
mod screen_frames;
mod starmap;
mod starmap_hud;
mod starmap_info;
mod starmap_menus;
mod spacewar_fx;
mod spacewar_hud;
mod spacewar_live;
mod spacewar_screen;
mod spacewar_ui;
mod spy_scene;
mod spy_screen;
mod starmap_panels;
mod starmap_planet;
mod starmap_radar;
mod starmap_tip;
mod trade;
mod topbar_menus;
mod tutorial_ui;
mod trade_scene;
mod ui;
mod video;
mod window;
mod world;

use std::path::PathBuf;

use ig2_formats::save::SaveGame;
use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::time::Speed;

/// Command-line options.
#[derive(Debug, Clone)]
pub struct Options {
    pub save: PathBuf,
    /// `--new-campaign`: start this campaign instead of loading `save` (re/rules/new-game.md).
    pub new_campaign: Option<ig2_sim::game::new_game::NewCampaign>,
    /// The window starts on the main menu (re/screens/mainscr.md), unless `--save` names the game to run.
    pub main_menu: bool,
    pub speed: Speed,
    pub ticks: u32,
    pub alpha: f32,
    pub size: (u32, u32),
    /// The window (`size` is its band, [`kit::Layout::band`]); `--letterbox`: the 3D stops at the 4:3 frame.
    pub window: (u32, u32),
    pub letterbox: bool,
    /// `--background classic|live`: the game behind the overlay screens (ADR 0015; else the settings file's
    /// `LiveBackground`, live by default).
    pub background: Option<bool>,
    /// `--texts classic|one`: the name tables in the exe's Hungarian or in the game's language (ADR 0017; else the
    /// settings file's `OneLanguage`, one language by default; `--snapshot` classic, as the captures it is compared with).
    pub texts: Option<bool>,
    /// `--backdrop` (with `--snapshot`): the overlay screens over the starmap's picture, as after a starmap exit
    /// (`run_screen.py --via-starmap`); without it they are over black, as the exe's bare screen jumps.
    pub backdrop: bool,
    pub scale: f32,
    pub hover: Option<(f32, f32)>,
    pub select_fleet: Option<usize>,
    pub reveal: bool,
    /// Open the colony screen of this planet (`--colony P`).
    pub colony: Option<usize>,
    /// Override the colony camera: x, y, zoom, pitch, rotate (`--colony-cam`).
    pub colony_cam: Option<[f32; 5]>,
    /// `--colony-live`: the save was written on that colony screen (a capture's state save): the camera is the live
    /// `ColonyXStart…` chunks and the entry's selection stands.
    pub colony_live: bool,
    /// `--colony-ui pick|demolish|build`: the colony screen with the pick list held open, the demolish window up, or
    /// the build cursor out (the input driven through `colony_ui`'s own press/release).
    pub colony_ui: Option<String>,
    /// `--screen info|messages` (with `--chart C`), and how many 10 ms UI frames the message line runs (`--ui-frames`).
    pub screen: Option<String>,
    pub chart: usize,
    /// `--open-message I`: the messages screen with inbox entry `I` open (typed for `--ui-frames` frames).
    pub open_message: Option<usize>,
    /// `--show-message ID`: message `ID` open without an inbox entry (checks the dialogue's drawing).
    pub show_message: Option<i32>,
    pub ui_frames: u32,
    /// `--trade-cheat`: the exe's `TradeCheat` (every own planet counts as a trade port; re/screens/trade.md §5.3).
    pub trade_cheat: bool,
    /// `--trade-eta N`: the unsaved `AktEtaDays` (the trade screen's delivery time) as the exe had it.
    pub trade_eta: Option<i32>,
    /// `--dkm M,CEL,NOVELO`: the row frame's global phase at the script's start (`DesignKeret1`'s `dkm`, `dkmcel`,
    /// `GfxNovelo`, kept across screens by the exe).
    pub dkm: Option<[f32; 3]>,
    /// `--spy-frame F [--spy-closing]`: the spy screen with `spyframe` held at F while opening (or closing)
    /// (re/screens/spy.md §1.1, §9).
    pub spy_frame: Option<f32>,
    /// `--screen video:NAME --video-ms T [--video-at X,Y] [--video-loop]`: the window at time `T` of the `.bik`
    /// name's video, fullscreen or in the screen at `X,Y` of the 640×480 frame, looping like the message clips'
    /// loop and wait clips (re/screens/video.md §6).
    pub video_ms: u64,
    pub video_at: Option<[f32; 2]>,
    pub video_loop: bool,
    pub spy_closing: bool,
    /// `--clouds N`: the starmap clouds' drift clock (re/screens/starmap.md §6.2).
    pub clouds: i32,
    /// `--faders UI_JSON`: the starmap's `fifo` / `starf` fader phases at the exe's grab (re/screens/hud.md §1.2.1).
    pub faders: Option<crate::starmap_panels::StarmapFaders>,
    /// `--tutorial-window FILE`: draw the tutorial's window from a `tutwin` record (`tools/trace/probes/tutorial.js`).
    pub tutorial_window: Option<std::path::PathBuf>,
    /// `--futofeny F`: the starmap's UI clock (re/screens/starmap.md §6.4).
    pub futofeny: Option<f32>,
    /// `--acttime S`: the remaster's clock in seconds, which drifts the long info planet's clouds
    /// (re/screens/starmap.md §3.3).
    pub acttime: Option<f32>,
    /// `--info-alap A`: the fleet info panel's model turn (`InfoAlapRotation`, re/screens/starmap.md §19.3).
    pub info_alap: Option<f32>,
    /// `--info-turn K`: the info screen's `infokeringeto` (re/screens/info.md §1.1).
    pub info_turn: f32,
    /// `--info-list P,H,C`: the list's `BolygoFlottaPointer`, its problem step and `Counter` (re/screens/info.md §6.2).
    pub info_list: Option<(usize, usize, i32)>,
    /// `--info-globes S,P0..P8,F0..F8[,POS,HOME,START,TARGET,STEP];…`: per list slot the spin, the planet's and
    /// fleet's rotations and optionally the five positions (3 each) of the slot (`InfoBF`, re/screens/info.md §6.4,
    /// §6.6).
    pub info_globes: Vec<Vec<f32>>,
    /// `--info-detail E,S,L,P`: the detail mode on list entry E in slot S, leaving (0/1), the design list's pointer
    /// (re/screens/info.md §6.6); `--info-texts H,OC,OP` its info-text statics.
    pub info_detail: Option<(usize, usize, bool, usize)>,
    pub info_texts: Option<(usize, i32, usize)>,
    /// `--info-select K,X,Y,I`: the select menu of hot-spot K (26 tax, 27 auto-build, 28 priority) open at
    /// `infoSelectX/Y` (X, Y) with `infosmSelected` I (re/screens/info.md §7.1).
    pub info_select: Option<[i32; 4]>,
    /// `--info-star F`: star-log mode with `infoframe` F; `--info-log FILE` the star log (a line "pointer filled",
    /// then per record "colour hex-text") (re/screens/info.md §6.8).
    pub info_star: Option<f32>,
    pub info_log: Option<String>,
    /// `--script S`: an input script (re/screens/sound.md §3); the snapshot writes one PNG per grab.
    pub script: Option<String>,
    /// `--screen spacewar --battle FILE`: a battle-screen capture's dump (`run_spwc.py --grab-ticks`,
    /// re/screens/spacewar.md §9) drawn as the exe drew it.
    pub battle: Option<String>,
    /// `--sim-phase P` (with `--battle` and `--script`): the script frames whose sim tick runs are those with
    /// `t % 5 == P`, the exe run's phase of its 50 ms clock against the scripted 10 ms one (run_screendiff.py reads
    /// it from the exe's trace).
    pub sim_phase: u32,
    /// Show the clock as paused (`--paused`; the capture tool pauses the exe).
    pub paused: bool,
    /// `--panel production` or `--panel transfer:F1,F2[,tank]`: the starmap's production view or transfer panel up.
    pub panel: Option<String>,
    /// `--planet-info`, `--rop-menu X,Y`, `--menu KIND@X,Y`, `--mouse X,Y` (re/screens/starmap.md §14–§17).
    pub planet_info: bool,
    pub rop_menu: Option<[f32; 2]>,
    pub menu: Option<String>,
    /// `--top-menu screens:X | research | research+T` and `--top-mouse X,Y`: a top-bar drop-down held open (pressed
    /// at x X; with T started first) and the pointer over it (re/screens/hud.md §2.1).
    pub top_menu: Option<String>,
    pub top_mouse: Option<[f32; 2]>,
    pub mouse: Option<[f32; 2]>,
    /// `--lang CODE`: the texts' language (default: the exe's own choice).
    pub lang: Option<Lang>,
}

fn default_save() -> PathBuf {
    std::env::var_os("IG2_SAVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/saves"))
        .join("quick-1.sav")
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        save: default_save(),
        new_campaign: None,
        main_menu: true,
        speed: Speed::Fastest,
        ticks: 0,
        alpha: 1.0,
        size: (1920, 1080),
        window: (1920, 1080),
        letterbox: false,
        background: None,
        texts: None,
        backdrop: false,
        scale: 1.0,
        hover: None,
        select_fleet: None,
        reveal: false,
        colony: None,
        colony_cam: None,
        colony_live: false,
        colony_ui: None,
        screen: None,
        chart: 0,
        open_message: None,
        show_message: None,
        ui_frames: 150,
        trade_cheat: false,
        trade_eta: None,
        dkm: None,
        spy_frame: None,
        video_ms: 0,
        video_at: None,
        video_loop: false,
        spy_closing: false,
        clouds: 0,
        faders: None,
        tutorial_window: None,
        futofeny: None,
        acttime: None,
        info_alap: None,
        info_turn: 0.0,
        info_list: None,
        info_globes: Vec::new(),
        info_detail: None,
        info_select: None,
        info_texts: None,
        info_star: None,
        info_log: None,
        script: None,
        battle: None,
        sim_phase: 0,
        paused: false,
        panel: None,
        planet_info: false,
        rop_menu: None,
        menu: None,
        top_menu: None,
        top_mouse: None,
        mouse: None,
        lang: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        let num = |v: String| v.parse::<f32>().map_err(|e| format!("{a}: {e}"));
        match a.as_str() {
            "--save" => {
                o.save = PathBuf::from(val()?);
                o.main_menu = false;
            }
            "--new-campaign" => {
                o.new_campaign = Some(new_game::parse_campaign(&val()?)?);
                o.main_menu = false;
            }
            "--speed" => {
                o.speed = match val()?.as_str() {
                    "1" => Speed::Normal,
                    "2" => Speed::Fast,
                    "3" => Speed::Fastest,
                    v => return Err(format!("--speed {v}: 1, 2 or 3")),
                }
            }
            "--ticks" => o.ticks = val()?.parse().map_err(|e| format!("--ticks: {e}"))?,
            "--alpha" => o.alpha = num(val()?)?,
            "--scale" => o.scale = num(val()?)?,
            "--size" => {
                let v = val()?;
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                o.size = (w.parse().map_err(|_| "--size WxH")?, h.parse().map_err(|_| "--size WxH")?);
            }
            "--hover" => {
                let v = val()?;
                let (x, y) = v.split_once(',').ok_or("--hover X,Y")?;
                o.hover = Some((num(x.into())?, num(y.into())?));
            }
            "--reveal" => o.reveal = true,
            "--letterbox" => o.letterbox = true,
            "--backdrop" => o.backdrop = true,
            "--background" => {
                o.background = Some(match val()?.as_str() {
                    "live" => true,
                    "classic" => false,
                    v => return Err(format!("--background: {v} (classic or live)")),
                })
            }
            "--texts" => {
                o.texts = Some(match val()?.as_str() {
                    "one" => true,
                    "classic" => false,
                    v => return Err(format!("--texts: {v} (classic or one)")),
                })
            }
            "--paused" => o.paused = true,
            "--colony-cam" => {
                let v: Vec<f32> = val()?.split(',').map(|x| x.trim().parse::<f32>().map_err(|e| format!("--colony-cam: {e}"))).collect::<Result<_, _>>()?;
                o.colony_cam = Some(v.try_into().map_err(|_| "--colony-cam X,Y,ZOOM,PITCH,ROTATE")?);
            }
            "--colony-live" => o.colony_live = true,
            "--colony-ui" => o.colony_ui = Some(val()?),
            "--screen" => o.screen = Some(val()?),
            "--video-ms" => o.video_ms = val()?.parse().map_err(|e| format!("--video-ms: {e}"))?,
            "--video-loop" => o.video_loop = true,
            "--video-at" => {
                let v = val()?;
                let (x, y) = v.split_once(',').ok_or("--video-at X,Y")?;
                o.video_at = Some([num(x.into())?, num(y.into())?]);
            }
            "--chart" => o.chart = val()?.parse().map_err(|e| format!("--chart: {e}"))?,
            "--open-message" => o.open_message = Some(val()?.parse().map_err(|e| format!("--open-message: {e}"))?),
            "--show-message" => o.show_message = Some(val()?.parse().map_err(|e| format!("--show-message: {e}"))?),
            "--trade-cheat" => o.trade_cheat = true,
            "--dkm" => {
                let v: Vec<f32> = val()?.split(',').map(|x| x.trim().parse().map_err(|e| format!("--dkm: {e}"))).collect::<Result<_, _>>()?;
                o.dkm = Some(v.try_into().map_err(|_| "--dkm: three numbers".to_string())?);
            }
            "--trade-eta" => o.trade_eta = Some(val()?.parse().map_err(|e| format!("--trade-eta: {e}"))?),
            "--ui-frames" => o.ui_frames = val()?.parse().map_err(|e| format!("--ui-frames: {e}"))?,
            "--spy-frame" | "--anim-frame" => o.spy_frame = Some(val()?.parse().map_err(|e| format!("--anim-frame: {e}"))?),
            "--spy-closing" | "--anim-closing" => o.spy_closing = true,
            "--tutorial-window" => o.tutorial_window = Some(val()?.into()),
            "--clouds" => o.clouds = val()?.parse().map_err(|e| format!("--clouds: {e}"))?,
            "--faders" => o.faders = Some(crate::starmap_panels::StarmapFaders::from_ui_json(std::path::Path::new(&val()?))?),
            "--futofeny" => o.futofeny = Some(val()?.parse().map_err(|e| format!("--futofeny: {e}"))?),
            "--info-turn" => o.info_turn = val()?.parse().map_err(|e| format!("--info-turn: {e}"))?,
            "--info-list" => {
                let v: Vec<i64> = val()?.split(',').map(|x| x.trim().parse::<i64>()).collect::<Result<_, _>>().map_err(|e| format!("--info-list: {e}"))?;
                let [p, h, c] = v[..] else { return Err("--info-list P,H,C".into()) };
                o.info_list = Some((p.max(0) as usize, h.max(0) as usize, c as i32));
            }
            "--info-globes" => {
                for g in val()?.split(';') {
                    let v: Vec<f32> = g.split(',').map(|x| x.trim().parse::<f32>()).collect::<Result<_, _>>().map_err(|e| format!("--info-globes: {e}"))?;
                    if v.len() != 19 && v.len() != 34 {
                        return Err("--info-globes: 19 or 34 values a slot".into());
                    }
                    o.info_globes.push(v);
                }
            }
            "--info-detail" => {
                let v: Vec<i64> = val()?.split(',').map(|x| x.trim().parse::<i64>()).collect::<Result<_, _>>().map_err(|e| format!("--info-detail: {e}"))?;
                let [e, s, l, p] = v[..] else { return Err("--info-detail E,S,L,P".into()) };
                o.info_detail = Some((e.max(0) as usize, s.clamp(0, 3) as usize, l != 0, p.max(0) as usize));
            }
            "--info-select" => {
                let v: Vec<i32> = val()?.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>().map_err(|e| format!("--info-select: {e}"))?;
                let [k, x, y, i] = v[..] else { return Err("--info-select K,X,Y,I".into()) };
                o.info_select = Some([k, x, y, i]);
            }
            "--info-texts" => {
                let v: Vec<i64> = val()?.split(',').map(|x| x.trim().parse::<i64>()).collect::<Result<_, _>>().map_err(|e| format!("--info-texts: {e}"))?;
                let [h, c, p] = v[..] else { return Err("--info-texts H,OC,OP".into()) };
                o.info_texts = Some((h.clamp(1, 5) as usize, c as i32, p.max(0) as usize));
            }
            "--info-star" => o.info_star = Some(val()?.parse().map_err(|e| format!("--info-star: {e}"))?),
            "--info-log" => o.info_log = Some(val()?),
            "--script" => o.script = Some(val()?),
            "--battle" => o.battle = Some(val()?),
            "--sim-phase" => o.sim_phase = val()?.parse::<u32>().map_err(|e| format!("--sim-phase: {e}"))? % 5,
            "--acttime" => o.acttime = Some(val()?.parse().map_err(|e| format!("--acttime: {e}"))?),
            "--info-alap" => o.info_alap = Some(val()?.parse().map_err(|e| format!("--info-alap: {e}"))?),
            "--colony" => o.colony = Some(val()?.parse().map_err(|e| format!("--colony: {e}"))?),
            "--panel" => o.panel = Some(val()?),
            "--planet-info" => o.planet_info = true,
            "--menu" => o.menu = Some(val()?),
            "--top-menu" => o.top_menu = Some(val()?),
            "--top-mouse" => {
                let v = val()?;
                let (x, y) = v.split_once(',').ok_or("--top-mouse X,Y")?;
                o.top_mouse = Some([num(x.into())?, num(y.into())?]);
            }
            "--rop-menu" | "--mouse" => {
                let v = val()?;
                let (x, y) = v.split_once(',').ok_or(format!("{a} X,Y"))?;
                let p = Some([num(x.into())?, num(y.into())?]);
                if a == "--mouse" {
                    o.mouse = p;
                } else {
                    o.rop_menu = p;
                }
            }
            "--lang" => {
                let v = val()?;
                o.lang = Some(Lang::from_code(&v).ok_or(format!("--lang {v}: en, hu, de, fr, es or it"))?);
            }
            "--select-fleet" => o.select_fleet = Some(val()?.parse().map_err(|e| format!("--select-fleet: {e}"))?),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    o.window = o.size;
    kit::layout::set_letterbox(o.letterbox);
    Ok(o)
}

/// Everything a session needs from the user's install and save.
pub struct Loaded {
    pub install: Install,
    pub rules: ig2_sim::game::GameRules,
    pub tech_defaults: Vec<u8>,
    pub volatile: Vec<(u32, Vec<u8>)>,
    /// The save of `--save`; `None` with `--new-campaign`.
    pub save: Option<SaveGame>,
    /// The exe's tables the screens draw with (models, colours).
    pub tables: ig2_rules::Rules,
}

pub fn load(o: &Options) -> Result<Loaded, String> {
    let install = open_install(o)?;
    let (rules, tech_defaults) = rules::load(&install)?;
    let volatile = rules::scenario_globals(&install)?;
    let save = match o.new_campaign {
        Some(_) => None,
        None => {
            let bytes = std::fs::read(&o.save).map_err(|e| format!("{}: {e}", o.save.display()))?;
            Some(SaveGame::parse(&bytes).map_err(|e| format!("{}: {e}", o.save.display()))?)
        }
    };
    let tables = ig2_rules::Rules::from_install(&install)?;
    Ok(Loaded { install, rules, tech_defaults, volatile, save, tables })
}

/// The session's game: `--new-campaign`'s start (its intro queued), else the save.
/// Takes [`Loaded`]'s parts: the window keeps the rules for the whole process.
pub fn start_world<'r>(o: &Options, install: &Install, save: Option<&SaveGame>, rules: &'r ig2_sim::game::GameRules, tech_defaults: &[u8], volatile: &[(u32, Vec<u8>)]) -> Result<world::World<'r>, String> {
    Ok(match (&o.new_campaign, save) {
        (Some(c), _) => {
            let mut w = world::World::new_game(new_game::campaign(install, c, rules)?);
            w.campaign_intro();
            w
        }
        (None, Some(s)) => world::World::load(s, rules, tech_defaults, volatile),
        (None, None) => return Err("no game: --save or --new-campaign".into()),
    })
}

/// A float chunk of the session's save (0 without one or for a new game).
pub fn save_f32(save: Option<&SaveGame>, n: &str) -> f32 {
    save.and_then(|s| s.chunk(n)).and_then(|b| b.get(..4)).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()))
}

/// The install from `IG2_INSTALL_DIR` in the texts' language.
pub fn open_install(o: &Options) -> Result<Install, String> {
    let root = std::env::var_os(ig2_formats::vfs::INSTALL_ENV).ok_or("set IG2_INSTALL_DIR to your IG2 install")?;
    Install::open(PathBuf::from(&root), language(o, std::path::Path::new(&root))).map_err(|e| e.to_string())
}

/// The texts' language: `--lang`, else `IG2_LANG`, else what the GOG exe would run in (`ig2.settings`' `language=`,
/// a `locale.txt`, the system locale).
fn language(o: &Options, root: &std::path::Path) -> Lang {
    o.lang.or_else(|| std::env::var("IG2_LANG").ok().and_then(|c| Lang::from_code(&c))).unwrap_or_else(|| ig2_formats::locale::exe_language(root).0)
}

fn info(o: &Options) -> Result<(), String> {
    let l = load(o)?;
    let mut w = start_world(o, &l.install, l.save.as_ref(), &l.rules, &l.tech_defaults, &l.volatile)?;
    w.set_speed(o.speed);
    for _ in 0..o.ticks {
        w.tick();
    }
    let s = &w.curr;
    let (y, m, d) = s.date();
    println!("tick {} date {y}-{m:02}-{d:02} human {} view {:?} orbit {}", s.tick, s.human, w.view, s.orbit);
    for p in &s.planets {
        let l = p.look;
        println!(
            "planet {:3} {:>16} ({:8.2}, {:8.2}) owner {:2} surface {} known {} | color {:08x} size {} star {} spin {} dist {} speed {}",
            p.index, p.name, p.pos[0], p.pos[1], p.owner, p.surface, p.known, l.color, l.size, l.star_size, l.spin, l.dist, l.speed
        );
    }
    if o.colony.is_some() {
        for p in &s.planets {
            if let Some(c) = colony::ColonySnap::read(&w.game.state, p.index) {
                let vehicles = c.units.iter().filter(|u| u.flags & ig2_sim::ground::flag::TURRET == 0).count();
                if !c.buildings.is_empty() || !c.units.is_empty() {
                    println!("colony {:3} owner {:2} map {:2} surface {} | {} buildings, {} units ({vehicles} vehicles), {} shots, war {}", p.index, c.owner, c.map, c.surface, c.buildings.len(), c.units.len(), c.shots.len(), c.at_war);
                }
            }
        }
    }
    for f in &s.fleets {
        println!("fleet {:3} ({:8.3}, {:8.3}) owner {:2} visible {} order {} flags {:04x} target {:?}", f.slot, f.pos[0], f.pos[1], f.owner, f.visible, f.order, f.flags, f.target);
    }
    Ok(())
}

fn dump_pvr(path: &str, out: &std::path::Path) -> Result<(), String> {
    let install = Install::from_env(Lang::En).ok_or("set IG2_INSTALL_DIR")?.map_err(|e| e.to_string())?;
    let img = assets::Image::load(&install, path)?;
    println!("{path}: {}x{}", img.width, img.height);
    assets::write_png(out, img.width, img.height, &img.rgba)
}

fn colony_check(map: &str, surface: &str) -> Result<(), String> {
    let install = Install::from_env(Lang::En).ok_or("set IG2_INSTALL_DIR")?.map_err(|e| e.to_string())?;
    let (rules, _) = rules::load(&install)?;
    let map = map.parse().map_err(|_| "map: a number")?;
    colony::check(&install, &rules, map, surface.parse().map_err(|_| "surface: a number")?)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--snapshot") => match args.get(1) {
            Some(out) => parse(&args[2..]).and_then(|o| render::snapshot(&o, std::path::Path::new(out))),
            None => Err("usage: --snapshot <out.png> [options]".into()),
        },
        Some("--info") => parse(&args[1..]).and_then(|o| info(&o)),
        Some("--colony-check") => match &args[1..] {
            [map, surface] => colony_check(map, surface),
            _ => Err("usage: --colony-check <map> <surface>".into()),
        },
        Some("--dump-pvr") => match &args[1..] {
            [path, out] => dump_pvr(path, std::path::Path::new(out)),
            _ => Err("usage: --dump-pvr <game-path.pvr> <out.png>".into()),
        },
        _ => parse(&args).and_then(window::run),
    };
    if let Err(e) = result {
        eprintln!("ig2-client: {e}");
        std::process::exit(1);
    }
}
