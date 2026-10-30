//! The play-through harness (`TASKS.md` → Play-through harness, `docs/BUILDING.md`): a whole game headless at the
//! fastest speed, on the player's own path ([`World::tick`], commands through [`World::send`]/[`World::pump`]), to
//! victory, defeat or a tick budget. The human's empire is played by the sim's own AI ([`Game::autopilot`]); the
//! harness does what only the player's screens would: it reads the inbox (the messages screen's orders, the first
//! answer it may take), leaves a finished real-time battle, takes the pause off, and saves and loads on the way
//! (our `.ig2s` through [`World::load_bytes`]). It reports panics, hangs, stories left running, broken invariants
//! and what the game asked of the client (videos, battles, the end).
//!
//! `ig2-client --playthrough <run>[,<run>…] [--ticks N] [--seed S] [--save-every M] [--stall M] [--human ai|idle] [--compare-every N]`, runs
//! `campaign:C[:D]`, `scenario:<file|all>`, `skirmish[:RACE[:AIS]]`, `mp[:AIS[:PLANETS]]` ([`mp`]), or `all`.

mod mp;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ig2_formats::Install;
use ig2_net::Command;
use ig2_sim::game::{choice_count, choice_disabled, Game, GameOver, MessageOrder, RealTimeEvent};
use ig2_sim::save::va;
use ig2_sim::time::Speed;

use crate::world::World;

/// One game to play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    /// `CampaignType` 1..=3 and `Nehezseg` 1/2/4.
    Campaign { campaign: i32, difficulty: i32 },
    /// A `scenario/<name>.scn` of the scenario menu (the `dataraw/` relative path).
    Scenario(String),
    /// The skirmish menu's defaults with the human's race and the computer count.
    Skirmish { race: i32, ais: i32 },
    /// A 2-machine loopback multiplayer galaxy with `ais` computers and `planets` planets ([`mp`]).
    Mp { ais: u8, planets: u8 },
}

impl Run {
    pub fn name(&self) -> String {
        match self {
            Run::Campaign { campaign, difficulty } => format!("campaign:{campaign}:{difficulty}"),
            Run::Scenario(f) => format!("scenario:{f}"),
            Run::Skirmish { race, ais } => format!("skirmish:{race}:{ais}"),
            Run::Mp { ais, planets } => format!("mp:{ais}:{planets}"),
        }
    }
}

/// The limits of a run.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Sim ticks (50 ms each) before the run stops as [`End::Budget`].
    pub ticks: u64,
    /// `RandomInit`'s seed and `SDL_GetTicks()` of the start.
    pub seed: u32,
    /// Save and load every this many game months (0: never).
    pub save_every: i32,
    /// No planet changed hands, no empire died and no story moved for this many game months: [`End::Stalled`].
    pub stall: i32,
    /// The AI plays the human's empire ([`Game::autopilot`]); off: the human stays human and idle (every battle
    /// it is in runs in real time, the diplomacy questions come to its inbox).
    pub autopilot: bool,
    /// Multiplayer: compare both machines' states at the same turn every this many ticks ([`mp`]).
    pub compare_every: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { ticks: 400_000, seed: 4242, save_every: 6, stall: 36, autopilot: true, compare_every: 200 }
    }
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum End {
    Over(GameOver),
    Budget,
    /// Nothing moved for [`Limits::stall`] months.
    Stalled,
    /// The calendar stood still for this many ticks while the game was neither paused nor on a battle screen.
    ClockStuck(u64),
    Panic(String),
    /// The start failed.
    Error(String),
}

/// What a run saw.
#[derive(Debug, Default)]
pub struct Report {
    pub name: String,
    pub ticks: u64,
    pub date: (i32, i32, i32),
    pub end: Option<End>,
    /// Problems, each with the tick it was first seen and how often (an invariant broken, a save that changed the
    /// state, a pause the harness had to take off).
    pub problems: BTreeMap<String, (u64, u64)>,
    /// Observations that need a look but aren't failures (a story's lookup that failed, a pause taken off).
    pub notes: BTreeMap<String, (u64, u64)>,
    /// What the client is asked to show (videos, real-time battles) and how often.
    pub client: BTreeMap<String, u64>,
    /// Story ids seen running, and those finished (`HistoryVodor`).
    pub stories_seen: BTreeSet<i32>,
    pub stories_done: BTreeSet<i32>,
    /// The stories still running at the end, with the date they were first seen.
    pub stories_left: Vec<(i32, (i32, i32, i32))>,
    pub saves: u32,
    pub answers: u32,
    /// The living empires and their planets at the end.
    pub empires: Vec<(usize, usize)>,
    pub human: usize,
}

impl Report {
    fn problem(&mut self, tick: u64, what: impl Into<String>) {
        self.problems.entry(what.into()).or_insert((tick, 0)).1 += 1;
    }

    fn note(&mut self, tick: u64, what: impl Into<String>) {
        self.notes.entry(what.into()).or_insert((tick, 0)).1 += 1;
    }

    fn client(&mut self, what: impl Into<String>) {
        *self.client.entry(what.into()).or_insert(0) += 1;
    }

    /// The run failed: a panic, a stuck clock or a broken invariant (a stall is the players' stalemate, not a hang).
    pub fn failed(&self) -> bool {
        matches!(self.end, Some(End::Panic(_) | End::Error(_) | End::ClockStuck(_))) || !self.problems.is_empty()
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        let (y, m, d) = self.date;
        let _ = writeln!(s, "== {}: {:?} after {} ticks, {y}-{m:02}-{d:02} (human {}, {} saves, {} answers)", self.name, self.end, self.ticks, self.human, self.saves, self.answers);
        let _ = writeln!(s, "   empires (empire, planets): {:?}", self.empires);
        for (what, (tick, n)) in &self.problems {
            let _ = writeln!(s, "   PROBLEM {what} (first at tick {tick}, {n}x)");
        }
        for (what, (tick, n)) in &self.notes {
            let _ = writeln!(s, "   note: {what} (first at tick {tick}, {n}x)");
        }
        for (what, n) in &self.client {
            let _ = writeln!(s, "   client: {what} ({n}x)");
        }
        let _ = writeln!(s, "   stories: {} seen, {} finished; left running: {:?}", self.stories_seen.len(), self.stories_done.len(), self.stories_left);
        s
    }
}

/// What the runs share: the install and its rules.
pub struct Harness {
    pub install: Install,
    pub rules: ig2_sim::game::GameRules,
    pub tech_defaults: Vec<u8>,
    pub volatile: Vec<(u32, Vec<u8>)>,
    empire_names: Vec<Vec<u8>>,
}

impl Harness {
    pub fn new(install: Install) -> Result<Self, String> {
        let (rules, tech_defaults) = crate::rules::load(&install)?;
        let volatile = crate::rules::scenario_globals(&install)?;
        let empire_names = crate::ui::empire_names(&crate::ui::names_txt17(&install, false));
        Ok(Self { install, rules, tech_defaults, volatile, empire_names })
    }

    /// The scenario menu's scenario files.
    pub fn scenarios(&self) -> Vec<String> {
        let texts = crate::ui::UiTexts::load(&self.install);
        crate::mainscr_scenario::Assets::load(&self.install, &texts).entries.into_iter().filter(|e| !e.skirmish()).map(|e| e.file).collect()
    }

    fn start(&self, run: &Run, seed: u32) -> Result<World<'_>, String> {
        let rules = &self.rules;
        let game = match run {
            Run::Campaign { campaign, difficulty } => {
                let cfg = ig2_sim::game::new_game::NewCampaign { campaign: *campaign, difficulty: *difficulty, seed, ticks: seed, storyline: 0, storymap: 0, last_storyline: None };
                crate::new_game::campaign(&self.install, &cfg, rules)?
            }
            Run::Scenario(file) => crate::new_game::scenario(&self.install, file, seed, rules)?,
            Run::Skirmish { race, ais } => {
                let exe = ig2_rules::PeImage::parse(&self.install.read_raw("ig2.exe").map_err(|e| e.to_string())?).map(|pe| crate::mainscr_scenario::Tables::read(&pe)).map_err(|e| e.to_string())?;
                let k = crate::mainscr_scenario::Skirmish::new(*race);
                let cfg = ig2_sim::game::new_game::NewSkirmish {
                    en: k.race,
                    ais: *ais,
                    random_id: 1,
                    tech: k.technology,
                    planets: exe.planets[k.planets.clamp(0, 4) as usize],
                    money: exe.money[k.money.clamp(0, 5) as usize],
                    difficulty: k.difficulty,
                    population: k.population,
                    seed,
                    ticks: seed,
                };
                crate::new_game::skirmish(&self.install, &cfg, rules)?
            }
            Run::Mp { .. } => return Err("a multiplayer run starts its own machines".into()),
        };
        let mut w = World::new_game(game);
        w.scenario = matches!(run, Run::Scenario(_));
        w.game.empire_names = self.empire_names.clone();
        if matches!(run, Run::Campaign { .. }) {
            w.campaign_intro();
        }
        Ok(w)
    }

    /// Play `run` to its end; a panic ends it as [`End::Panic`].
    pub fn play(&self, run: &Run, limits: Limits) -> Report {
        let mut report = Report { name: run.name(), ..Default::default() };
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.play_inner(run, limits, &mut report)));
        report.end = Some(match r {
            Ok(end) if limits.autopilot => end,
            Ok(end) => {
                report.name += " (idle human)";
                end
            }
            Err(p) => End::Panic(p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()),
        });
        report
    }

    fn play_inner(&self, run: &Run, limits: Limits, report: &mut Report) -> End {
        if let Run::Mp { ais, planets } = *run {
            return mp::play(self, ais, planets, limits, report);
        }
        let mut w = match self.start(run, limits.seed) {
            Ok(w) => w,
            Err(e) => return End::Error(e),
        };
        let en = w.game.state.mem.i32(va::EN).clamp(1, 18) as usize;
        report.human = en;
        if limits.autopilot {
            w.game.autopilot(en);
        }
        w.set_speed(Speed::Fastest);
        let mut since: BTreeMap<i32, (i32, i32, i32)> = BTreeMap::new();
        let mut progress = (signature(&w.game), months(&w.game));
        let mut clock = (w.game.state.mem.i32(va::IDO), 0u64);
        let mut next_save = months(&w.game) + limits.save_every;
        let mut battle: Option<usize> = None;
        let mut ever_alive = w.game.state.alive();
        let mut end = End::Budget;
        for tick in 0..limits.ticks {
            report.ticks = tick;
            if tick % 200 == 0 && battle.is_none() {
                report.answers += read_inbox(&mut w);
            }
            if w.paused() && battle.is_none() {
                report.note(tick, "the game paused itself (the harness took the pause off)");
                w.game.set_pause(false, false);
                w.set_speed(Speed::Fastest);
            }
            // The player presses the fastest speed again after a slow-down (`SetIdoLelassit`).
            if tick % 20 == 0 && battle.is_none() && w.game.state.mem.i32(va::IDO_SPEED) < 16 {
                if w.game.state.mem.i32(va::SPEED_LOCKED) != 0 {
                    report.note(tick, "the speed is locked below the fastest");
                }
                w.set_speed(Speed::Fastest);
            }
            w.tick();
            // The tick's battles (the world takes the events at the next tick).
            for e in &w.game.events {
                if let ig2_sim::game::GameEvent::Battle(b) = e {
                    let k = format!("{b:?}");
                    report.client(format!("battle {}", k.split([' ', '(', '{']).next().unwrap_or("")));
                }
            }
            for v in std::mem::take(&mut w.videos) {
                report.client(format!("video {v:?}"));
            }
            for u in std::mem::take(&mut w.story_ui) {
                report.client(format!("story ui {u:?}"));
            }
            for r in std::mem::take(&mut w.realtime) {
                match r {
                    RealTimeEvent::SpaceStarted { slot } => {
                        report.client("real-time space battle");
                        battle = Some(slot);
                        w.game.sp_battles.on_screen = true;
                        w.game.sp_battles.fade = 1.0;
                        w.game.set_pause(false, false);
                    }
                    RealTimeEvent::SpaceOver { .. } => {
                        w.send(Command::SpaceBattleLeft);
                        w.pump();
                        w.game.sp_battles.on_screen = false;
                        battle = None;
                        w.set_speed(Speed::Fastest);
                    }
                    RealTimeEvent::GroundStarted { .. } => report.client("real-time ground battle"),
                    _ => {}
                }
            }
            while w.pop_message().is_some() {}
            w.local_lines.clear();
            if let Some(o) = w.game.over {
                end = End::Over(o);
                break;
            }
            // The calendar.
            let ido = w.game.state.mem.i32(va::IDO);
            if ido != clock.0 || battle.is_some() || w.paused() {
                clock = (ido, tick);
            } else if tick - clock.1 > 4000 {
                end = End::ClockStuck(tick - clock.1);
                break;
            }
            if tick % 100 != 0 {
                continue;
            }
            for (e, a) in w.game.state.alive().iter().enumerate() {
                ever_alive[e] |= *a;
            }
            check(&w.game, tick, &ever_alive, report);
            if tick % 2000 == 0 && std::env::var_os("IG2_PLAYTHROUGH_DEBUG").is_some() {
                let t = &w.game.state.treasuries()[en];
                let mine = w.game.state.colony().planets.iter().filter(|p| p.owner as usize == en).count();
                let fleets = w.game.state.starmap().fleets.iter().filter(|f| f.owner as usize == en).count();
                eprintln!("{:?} EN money {:.0} pools {:?} planets {mine} fleets {fleets} ki {} ai {}", date(&w.game), t.total, t.pools, w.game.state.mem.u8(va::PLAYER_KI + en as u32), w.game.state.mem.u8(va::AI_EMPIRE + en as u32));
            }
            let story = w.game.state.story();
            let date = date(&w.game);
            for b in &story.buckets {
                for &(id, _) in &b.entries {
                    let id = id.abs();
                    report.stories_seen.insert(id);
                    since.entry(id).or_insert(date);
                }
            }
            for &(id, _) in &story.history.entries {
                report.stories_done.insert(id.abs());
            }
            let sig = signature(&w.game);
            if sig != progress.0 {
                progress = (sig, months(&w.game));
            } else if months(&w.game) - progress.1 >= limits.stall {
                end = End::Stalled;
                break;
            }
            if limits.save_every > 0 && months(&w.game) >= next_save && battle.is_none() {
                next_save = months(&w.game) + limits.save_every;
                match self.save_and_load(w, tick, report) {
                    Ok(n) => w = n,
                    Err(e) => return End::Error(e),
                }
            }
        }
        report.date = date(&w.game);
        let story = w.game.state.story();
        report.stories_left = story.buckets.iter().flat_map(|b| b.entries.iter().map(|&(id, _)| id.abs())).collect::<BTreeSet<_>>().into_iter().map(|id| (id, since.get(&id).copied().unwrap_or_default())).collect();
        let colony = w.game.state.colony();
        let alive = w.game.state.alive();
        report.empires = (1..alive.len()).filter(|&e| alive[e]).map(|e| (e, colony.planets.iter().skip(1).filter(|p| p.owner as usize == e).count())).collect();
        end
    }

    /// The Options screen's save and load: our save file of the running game, loaded back as a slot is.
    fn save_and_load<'a>(&'a self, w: World<'a>, tick: u64, report: &mut Report) -> Result<World<'a>, String> {
        let before = ig2_net::snapshot::state_hash(&w.game, &[]);
        let bytes = w.save_file();
        let mut n = World::load_bytes(&bytes, &self.rules, &self.tech_defaults, &self.volatile)?;
        report.saves += 1;
        if ig2_net::snapshot::state_hash(&n.game, &[]) != before {
            report.problem(tick, "a save and load changed the state hash");
        }
        if !n.game.sp_battles.live {
            report.problem(tick, "a loaded save shows no real-time battles (sp_battles.live off)");
            n.game.sp_battles.live = true;
        }
        // The window's `start_world` puts the install's design names back.
        if n.game.design_texts.is_empty() {
            n.game.design_texts = crate::new_game::design_texts(&self.install);
        }
        if n.game.design_texts != w.game.design_texts {
            report.problem(tick, "a loaded save names the designs differently (design_texts)");
        }
        if n.game.empire_names != w.game.empire_names {
            report.problem(tick, "a loaded save lost the empire names");
        }
        n.scenario = w.scenario;
        n.game.state.mem.set_i32(va::IDO_SPEED, w.game.state.mem.i32(va::IDO_SPEED));
        Ok(n)
    }
}

/// The messages screen as a player clicks through it: `Messages_Belep`, then each dialogue typed out with the first
/// answer each block allows, the automatic messages, and every unread entry opened in turn; the answers taken.
fn read_inbox(w: &mut World) -> u32 {
    use ig2_sim::game::dialogue_in_progress;
    let unread = |w: &World, tried: &[usize]| {
        let b = w.game.state.inbox();
        (0..b.count.max(0) as usize).find(|&i| b.messages.get(i * ig2_sim::messages::MESSAGE + 4) == Some(&0) && !tried.contains(&i))
    };
    let mut answers = 0;
    let mut tried = Vec::new();
    w.send(MessageOrder::Enter.into());
    w.pump();
    for _ in 0..200 {
        if dialogue_in_progress(&w.game.state).is_some() {
            let before = ig2_net::snapshot::state_hash(&w.game, &[]);
            w.send(MessageOrder::TypedOut.into());
            w.pump();
            let n = choice_count(&w.game.state);
            if let Some(k) = (0..n).find(|&k| !choice_disabled(&w.game.state, k)) {
                w.send(MessageOrder::Answer { k: k as i32 }.into());
                w.pump();
                answers += 1;
            } else if ig2_net::snapshot::state_hash(&w.game, &[]) == before {
                break;
            }
            continue;
        }
        w.send(MessageOrder::Auto.into());
        w.pump();
        if dialogue_in_progress(&w.game.state).is_some() {
            continue;
        }
        let Some(i) = unread(w, &tried) else { break };
        tried.push(i);
        w.send(MessageOrder::Scroll { pointer: i as i32, row: 0 }.into());
        w.send(MessageOrder::Open { row: 0 }.into());
        w.pump();
    }
    if std::env::var_os("IG2_PLAYTHROUGH_DEBUG").is_some() {
        let left = unread(w, &[]);
        eprintln!("inbox: {} messages, {answers} answers, dialogue {:?}, first unread {left:?}", w.game.state.inbox().count, dialogue_in_progress(&w.game.state));
    }
    answers
}

fn date(g: &Game) -> (i32, i32, i32) {
    let m = &g.state.mem;
    (m.i32(va::EV), m.i32(va::HONAP), m.i32(va::NAP))
}

fn months(g: &Game) -> i32 {
    let (y, m, _) = date(g);
    y * 12 + m
}

/// What counts as progress: the planets' owners, the living empires and the stories' history.
fn signature(g: &Game) -> (Vec<u8>, [bool; 19], usize, usize) {
    let colony = g.state.colony();
    let story = g.state.story();
    (colony.planets.iter().map(|p| p.owner).collect(), g.state.alive(), story.history.entries.len(), story.buckets.iter().map(|b| b.entries.len()).sum())
}

/// The invariants of a running game.
fn check(g: &Game, tick: u64, ever_alive: &[bool; 19], report: &mut Report) {
    use ig2_sim::fleet::{flag, DELETED};
    let alive = g.state.alive();
    let live = |e: u8| (1..19).contains(&(e as usize)) && alive[e as usize];
    // An empire that lived in this run and died (the campaigns' minor empires own planets without ever living).
    let dead = |e: u8| (1..19).contains(&(e as usize)) && ever_alive[e as usize] && !alive[e as usize];
    for (e, t) in g.state.treasuries().iter().enumerate().skip(1) {
        if !alive[e] {
            continue;
        }
        if !t.total.is_finite() {
            report.problem(tick, format!("empire {e}'s money is not finite"));
        } else if t.total < 0.0 {
            report.problem(tick, format!("empire {e}'s money is negative"));
        }
    }
    let map = g.state.starmap();
    for f in map.fleets.iter().skip(1) {
        if f.owner == DELETED || f.flags & flag::WORMHOLE != 0 {
            continue;
        }
        if !f.x.is_finite() || !f.y.is_finite() {
            report.problem(tick, format!("a fleet of empire {} has no position", f.owner));
        }
        if dead(f.owner) && f.story_id == 0 {
            report.problem(tick, format!("a fleet of dead empire {} is on the map", f.owner));
        }
        if f.ship_count <= 0 && live(f.owner) && f.story_id == 0 {
            report.problem(tick, format!("an empty fleet of empire {} is on the map", f.owner));
        }
    }
    let colony = g.state.colony();
    for (p, pl) in colony.planets.iter().enumerate().skip(1) {
        if dead(pl.owner) {
            report.problem(tick, format!("planet {p} belongs to dead empire {}", pl.owner));
        }
    }
    let story = g.state.story();
    if story.error != 0 {
        report.note(tick, format!("StoryError {}", story.error));
    }
}

/// `--playthrough`'s run list: `campaign:C[:D]`, `scenario:<file|all>`, `skirmish[:RACE[:AIS]]` or `all`.
pub fn parse_runs(spec: &str, scenarios: impl Fn() -> Vec<String>) -> Result<Vec<Run>, String> {
    let mut runs = Vec::new();
    for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let parts: Vec<&str> = item.split(':').collect();
        let num = |i: usize, default: i32| parts.get(i).map_or(Ok(default), |s| s.parse::<i32>().map_err(|_| format!("--playthrough {item}: a number")));
        match parts[0] {
            "all" => {
                runs.extend((1..=3).map(|c| Run::Campaign { campaign: c, difficulty: 2 }));
                runs.extend(scenarios().into_iter().map(Run::Scenario));
                runs.push(Run::Skirmish { race: 1, ais: 3 });
            }
            "campaign" => runs.push(Run::Campaign { campaign: num(1, 1)?, difficulty: num(2, 2)? }),
            "scenario" => match parts.get(1).copied() {
                None | Some("all") => runs.extend(scenarios().into_iter().map(Run::Scenario)),
                Some(f) if f.contains('/') => runs.push(Run::Scenario(f.to_string())),
                Some(f) => runs.push(Run::Scenario(format!("scenario/{f}.scn"))),
            },
            "skirmish" => runs.push(Run::Skirmish { race: num(1, 1)?, ais: num(2, 3)? }),
            "mp" => runs.push(Run::Mp { ais: num(1, 2)?.clamp(0, 6) as u8, planets: num(2, 40)?.clamp(0, 255) as u8 }),
            _ => return Err(format!("--playthrough {item}: campaign:C[:D], scenario:<file|all>, skirmish[:RACE[:AIS]], mp[:AIS[:PLANETS]] or all")),
        }
    }
    Ok(runs)
}

/// `ig2-client --playthrough <runs> [--ticks N] [--seed S] [--save-every M] [--stall M] [--human ai|idle] [--compare-every N]`: every run's report; an
/// error when one failed.
pub fn main(args: &[String]) -> Result<(), String> {
    let spec = args.first().ok_or("usage: --playthrough <runs> [--ticks N] [--seed S] [--save-every M] [--stall M] [--human ai|idle] [--compare-every N]")?;
    let mut limits = Limits::default();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        let v = it.next().ok_or_else(|| format!("{a}: a value"))?;
        let n = |v: &str| v.parse::<u64>().map_err(|_| format!("{a} {v}: a number"));
        match a.as_str() {
            "--ticks" => limits.ticks = n(v)?,
            "--seed" => limits.seed = n(v)? as u32,
            "--save-every" => limits.save_every = n(v)? as i32,
            "--stall" => limits.stall = n(v)? as i32,
            "--compare-every" => limits.compare_every = n(v)?,
            "--human" => limits.autopilot = match v.as_str() { "ai" => true, "idle" => false, _ => return Err(format!("--human {v}: ai or idle")) },
            _ => return Err(format!("{a}: unknown")),
        }
    }
    let install = Install::from_env(ig2_formats::text::Lang::En).ok_or("set IG2_INSTALL_DIR")?.map_err(|e| e.to_string())?;
    let h = Harness::new(install)?;
    let runs = parse_runs(spec, || h.scenarios())?;
    let mut failed = 0;
    for run in &runs {
        let t = std::time::Instant::now();
        let r = h.play(run, limits);
        print!("{}", r.text());
        println!("   ({:.1} s)", t.elapsed().as_secs_f64());
        failed += r.failed() as usize;
    }
    if failed > 0 {
        return Err(format!("{failed} of {} runs failed", runs.len()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every mode played to its end (`cargo test --release -p ig2-client playthrough -- --ignored`): long, so off by
    /// default.
    #[test]
    #[ignore]
    fn playthrough_all_modes() {
        let Some(Ok(install)) = Install::from_env(ig2_formats::text::Lang::En) else {
            eprintln!("IG2_INSTALL_DIR not set; skipping");
            return;
        };
        let h = Harness::new(install).unwrap();
        let ticks = std::env::var("IG2_PLAYTHROUGH_TICKS").ok().and_then(|v| v.parse().ok()).unwrap_or(Limits::default().ticks);
        let runs = parse_runs(&std::env::var("IG2_PLAYTHROUGH").unwrap_or_else(|_| "all".into()), || h.scenarios()).unwrap();
        let reports: Vec<Report> = runs.iter().map(|r| h.play(r, Limits { ticks, ..Limits::default() })).collect();
        for r in &reports {
            eprint!("{}", r.text());
        }
        let failed: Vec<&str> = reports.iter().filter(|r| r.failed()).map(|r| r.name.as_str()).collect();
        assert!(failed.is_empty(), "failed runs: {failed:?}");
    }

    #[test]
    fn runs_parse() {
        let s = || vec!["scenario/a.scn".to_string()];
        assert_eq!(parse_runs("campaign:2,skirmish", s).unwrap(), [Run::Campaign { campaign: 2, difficulty: 2 }, Run::Skirmish { race: 1, ais: 3 }]);
        assert_eq!(parse_runs("scenario:all,scenario:b", s).unwrap(), [Run::Scenario("scenario/a.scn".into()), Run::Scenario("scenario/b.scn".into())]);
        assert_eq!(parse_runs("mp,mp:1:20", s).unwrap(), [Run::Mp { ais: 2, planets: 40 }, Run::Mp { ais: 1, planets: 20 }]);
        assert!(parse_runs("bogus", s).is_err());
    }
}
