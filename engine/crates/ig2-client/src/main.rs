//! IG2 game client (ADR 0006): loads an original save into the simulation, runs it at the original's 20 Hz and
//! renders the starmap at the display's refresh rate.
//!
//! Window:   `ig2-client [--save <file.sav>] [--speed 1|2|3]`
//! Headless: `ig2-client --snapshot <out.png> [--save <file.sav>] [--speed 1|2|3] [--ticks N] [--alpha A]
//!            [--size WxH] [--scale S] [--hover X,Y] [--select-fleet N] [--reveal]`
//!           runs N sim ticks (default 0) and renders the frame `A` (0..1) of the way from tick N−1 to N;
//!           `--reveal` draws the fleets the human doesn't see too; `--colony P` draws planet P's colony screen
//!           (`--colony-cam X,Y,ZOOM,PITCH,ROTATE` moves its camera); `--screen info|messages [--chart C]` draws the
//!           info charts or the inbox; `--ui-frames N` runs the message line N frames of 10 ms (default 150).
//! Info:     `ig2-client --info [--save <file.sav>] [--ticks N]` prints the starmap's planets and fleets.
//!
//! The install comes from `IG2_INSTALL_DIR`. The default save is `quick-1.sav` in `IG2_SAVE_DIR` (else the
//! repository's `_local/saves`). Saves, installs and rendered PNGs hold game data: keep them out of the repo.

mod assets;
mod colony;
mod font;
mod hud;
mod render;
mod render3d;
mod rules;
mod scene;
mod starmap;
mod ui;
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
    pub speed: Speed,
    pub ticks: u32,
    pub alpha: f32,
    pub size: (u32, u32),
    pub scale: f32,
    pub hover: Option<(f32, f32)>,
    pub select_fleet: Option<usize>,
    pub reveal: bool,
    /// Open the colony screen of this planet (`--colony P`).
    pub colony: Option<usize>,
    /// Override the colony camera: x, y, zoom, pitch, rotate (`--colony-cam`).
    pub colony_cam: Option<[f32; 5]>,
    /// `--screen info|messages` (with `--chart C`), and how many 10 ms UI frames the message line runs (`--ui-frames`).
    pub screen: Option<String>,
    pub chart: usize,
    pub ui_frames: u32,
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
        speed: Speed::Fastest,
        ticks: 0,
        alpha: 1.0,
        size: (1920, 1080),
        scale: 1.0,
        hover: None,
        select_fleet: None,
        reveal: false,
        colony: None,
        colony_cam: None,
        screen: None,
        chart: 0,
        ui_frames: 150,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{a} needs a value"));
        let num = |v: String| v.parse::<f32>().map_err(|e| format!("{a}: {e}"));
        match a.as_str() {
            "--save" => o.save = PathBuf::from(val()?),
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
            "--colony-cam" => {
                let v: Vec<f32> = val()?.split(',').map(|x| x.trim().parse::<f32>().map_err(|e| format!("--colony-cam: {e}"))).collect::<Result<_, _>>()?;
                o.colony_cam = Some(v.try_into().map_err(|_| "--colony-cam X,Y,ZOOM,PITCH,ROTATE")?);
            }
            "--screen" => o.screen = Some(val()?),
            "--chart" => o.chart = val()?.parse().map_err(|e| format!("--chart: {e}"))?,
            "--ui-frames" => o.ui_frames = val()?.parse().map_err(|e| format!("--ui-frames: {e}"))?,
            "--colony" => o.colony = Some(val()?.parse().map_err(|e| format!("--colony: {e}"))?),
            "--select-fleet" => o.select_fleet = Some(val()?.parse().map_err(|e| format!("--select-fleet: {e}"))?),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    Ok(o)
}

/// Everything a session needs from the user's install and save.
pub struct Loaded {
    pub install: Install,
    pub rules: ig2_sim::game::GameRules,
    pub tech_defaults: Vec<u8>,
    pub volatile: Vec<(u32, Vec<u8>)>,
    pub save: SaveGame,
    /// The exe's tables the screens draw with (models, colours).
    pub tables: ig2_rules::Rules,
}

pub fn load(o: &Options) -> Result<Loaded, String> {
    let install = Install::from_env(Lang::En).ok_or("set IG2_INSTALL_DIR to your IG2 install")?.map_err(|e| e.to_string())?;
    let (rules, tech_defaults) = rules::load(&install)?;
    let volatile = rules::scenario_globals(&install)?;
    let bytes = std::fs::read(&o.save).map_err(|e| format!("{}: {e}", o.save.display()))?;
    let save = SaveGame::parse(&bytes).map_err(|e| format!("{}: {e}", o.save.display()))?;
    let tables = ig2_rules::Rules::from_install(&install)?;
    Ok(Loaded { install, rules, tech_defaults, volatile, save, tables })
}

fn info(o: &Options) -> Result<(), String> {
    let l = load(o)?;
    let mut w = world::World::load(&l.save, &l.rules, &l.tech_defaults, &l.volatile);
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
