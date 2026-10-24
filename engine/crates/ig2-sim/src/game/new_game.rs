//! A new campaign (`re/rules/new-game.md`): the race menu's `CampaignGameStart` (exe 0x4f5e90) and
//! `InitNewGame(seed, 0, campaign, difficulty)` (exe 0x4e1130) with the campaign map `InitStory` loads
//! (`LoadScenario(map, 1)`, exe 0x545b40) and the star names (`Generate_StarNames`, exe 0x59b860).
//!
//! The state starts as the exe image's initial bytes for every saved global (the process holds them until the game
//! starts: a new game's save matches the image except where the start writes), then the start writes in the exe's
//! order; the ported systems (designs, spies, research, production, the AI's set-up, the story engine) do their own
//! parts. Everything that varies comes from [`NewCampaign`]: the seed, the wall-clock tick count `NEWGAME_Diplomacy`
//! seeds libc with, and the config and registry values `InitStory` reads. The result's chunk template is the save
//! stream the exe would write at tick 0 ([`SaveState::rebuild_template`]).

// The loops follow the exe's 1-based tables (empires, planets, constellations).
#![allow(clippy::needless_range_loop)]

use ig2_formats::save::SaveGame;

use super::{Game, GameRules, Libc};
use crate::campaign::CampaignTables;
use crate::colony_side::EMPIRES;
use crate::research::{self, ResearchEvent};
use crate::save::{va, Memory, SaveState, BUILDING_SIZE, DESIGN_SIZE, PLANET_SIZE, TECHS, TECH_SIZE};
use crate::story::{self, ScenarioStory, StoryEvent, StoryWorld};
use crate::time::GameTime;

/// What a campaign start depends on besides the install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCampaign {
    /// `CampaignType` 1..=3 (the race menu's pick + 1; humans, politicians, brutals).
    pub campaign: i32,
    /// `Nehezseg`: 1 (easy), 2 (normal) or 4 (hard).
    pub difficulty: i32,
    /// `RandomInit`'s seed (the race menu: `init\init`'s `rnd`, else `SDL_GetTicks()`).
    pub seed: u32,
    /// `SDL_GetTicks()` when `NEWGAME_Diplomacy` reseeds libc (the empires' cats, parrots, anniversaries,
    /// weaknesses and ruler ages).
    pub ticks: u32,
    /// The config's `storyline` and `storymap` (0 = not set).
    pub storyline: i32,
    pub storymap: i32,
    /// The registry's last finished storyline of this campaign (`Fable_<ct>`): `None` when the read fails.
    pub last_storyline: Option<i32>,
}

/// What a scenario start (the scenario menu's `Betolt_Scenario`, exe 0x4f5ba0) depends on besides the install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewScenario {
    /// The `.scn` file's text (decoded).
    pub text: String,
    /// `RandomInit`'s seed (`SDL_GetTicks()`); only `RandSeed[0]` outlives the file's `RandSeed` line.
    pub seed: u32,
    /// `SDL_GetTicks()` when `NEWGAME_Diplomacy` reseeds libc.
    pub ticks: u32,
    /// The config's `Difficulty` (`InitNewGame` is called with 0, so it reads the config; default 2).
    pub difficulty: i32,
}

/// The multiplayer galaxy (`FRISSIT_Start`, exe 0x5bc590 → `InitNewGame(seed, 2, 0, 1)`): the lobby's choices.
/// Races and names the lobby picked are the caller's to write first; everything else here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMpGalaxy {
    /// `RandomSeedNumber` (the host's `rand() % 0x7fff`, sent to every client).
    pub seed: u32,
    /// `SDL_GetTicks()` when `NEWGAME_Diplomacy` reseeds libc (each machine its own: not part of the shared state).
    pub ticks: u32,
    /// `HanyBolygo`: 20, 40, 60 or 80.
    pub planets: i32,
    /// This machine's empire (`EN`).
    pub en: i32,
    /// The human empires (`PlayerKi` 1); the rest are the computer's.
    pub humans: Vec<i32>,
    /// The computer empires among 1..=8 the lobby switched on (`BirodalomEl`).
    pub alive: Vec<i32>,
    /// `ProgramSebesseg` (4: the host controls the speed).
    pub speed: i32,
    /// `ProgramGalaxyShape` (exe 0xBAE554, the room's 1..=2; no reader in the generated galaxy).
    pub shape: i32,
    /// `MultiplayerBattleStatusz` (exe 0xBAE558): 0 space and ground battles in real time, 1 space only, 2 ground
    /// only, 3 neither (re/screens/start.md §1).
    pub battle_status: i32,
    /// `OnlyServerCanModifyTime` (exe 0xBAE55C): 0 or 1.
    pub server_time: i32,
    /// `PlayerFaj` of empires 1..=18 (index 0 unused); empty: `Start_Init`'s (exe 0x5bfbf0) defaults, each empire
    /// its own race, 15..=18 races 1, 3, 1, 1.
    pub races: Vec<u8>,
}

/// A skirmish (the skirmish menu's `SetScirmishGameOptions`, exe 0x503e80 → `InitNewGame(ticks, 3, 0, Nehezseg)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSkirmish {
    /// The human's empire (`EN`).
    pub en: i32,
    /// `HanyGepiJatekos`: the computer opponents, 1..=7 (the start switches off the rest of 1..=8 at random).
    pub ais: i32,
    /// `NewgameRandomID`.
    pub random_id: i32,
    /// `ScrirmishTechicalLevel` (0..=3): the techs every empire starts with (`ScirmishTalalmanyFejlesztes`).
    pub tech: i32,
    /// `ScrirmishPlanetNumber`: `HanyBolygo`.
    pub planets: i32,
    /// `ScrirmishStartingMoney`: every empire's.
    pub money: i32,
    /// The menu's difficulty 1..=3 (3, hard, is `Nehezseg` 4).
    pub difficulty: i32,
    /// `ScrirmishPopulation` (0..=3): the empires' starting colonies and their people.
    pub population: i32,
    /// `RandomInit`'s seed (`SDL_GetTicks()`).
    pub seed: u32,
    /// `SDL_GetTicks()` when `NEWGAME_Diplomacy` reseeds libc.
    pub ticks: u32,
}

/// `InitNewGame`'s arguments (spec §0).
struct Init {
    mode: i32,
    campaign: i32,
    difficulty: i32,
    seed: u32,
    ticks: u32,
}

/// The install's part of a start.
pub struct StartData<'a> {
    /// The exe image's initial bytes at a VA (BSS as zero): `ig2_rules::PeImage::read_loaded`.
    pub image: &'a dyn Fn(u32, usize) -> Vec<u8>,
    /// `scenario/<n>.scn`'s text (decoded).
    pub scenario: &'a dyn Fn(i32) -> Option<String>,
    /// `Faj_Nev[race]` (`GetString(0x11, 1, race + 5)` of the Hungarian `txt17.txt`), by race.
    pub race_names: Vec<Vec<u8>>,
    /// The constellation names `_s(0x1d, k)`, k = 1..=15 (index k − 1).
    pub constellations: Vec<Vec<u8>>,
    /// `StringReadAll`'s Hungarian `txt17` names written into the exe's tables at start-up: the techs'
    /// (`Talalmanyok0[t]`, 17/8..=11, index t; 0 unused) and the design templates' (`Tervek0[k]`, 17/7/k + 1).
    pub tech_names: Vec<Vec<u8>>,
    pub design_names: Vec<Vec<u8>>,
    /// `AlapFlottaNev` (the localised default fleet name).
    pub fleet_name: Vec<u8>,
    /// `_s(0x3b, n)` ([`Game::design_texts`]).
    pub design_texts: std::collections::BTreeMap<i32, Vec<u8>>,
    /// `HanyUzenetFajta`: the message kinds `LoadMessagesFile` read.
    pub message_kinds: i32,
}

// Exe image VAs the start reads or writes that `save::va` doesn't name.
const TALALMANYOK0: u32 = 0x8DE048;
const TERVEK0: u32 = 0x90BF80;
const PGYARTAS0: u32 = 0x90B8A8;
const TRADER_NAMES: u32 = 0x90B638;
const STARLOG: u32 = 0xB82990;
/// `Lang` (the game's language; 1 = Hungarian names, no tech names copied into designs).
const LANG: u32 = 0x912724;
const INFO_CHARTS: u32 = 0xB7A000;
const STORY_ADAT: u32 = 0xF273F8;
/// `EpuletekLeiro` (0x24-byte building kinds: +0 required tech, +0xc cost, +0x14 energy).
const BUILDING_KINDS: u32 = 0x8EBA38;
/// `ScirmishTalalmanyFejlesztes[t]`: the skirmish tech level a tech is given at.
const SKIRMISH_TECHS: u32 = 0x8EC218;
/// `HanyGepiJatekos`, `ScrirmishTechicalLevel`, `ScrirmishPlanetNumber`, `ScrirmishPopulation`, `ScrirmishStartingMoney`.
const SK_AIS: u32 = 0xF21488;
const SK_TECH: u32 = 0xBAD0D8;
const SK_PLANETS: u32 = 0xBAD0DC;
const SK_POPULATION: u32 = 0xBAD0E0;
const SK_MONEY: u32 = 0xBAD0E4;
const DESIGN_TEMPLATES: usize = 0x29;
const DESIGNS: usize = 0x2e;

/// The globals the save carries, as `(va, len)`: what a new game starts from (the image).
fn saved_globals(image: &dyn Fn(u32, usize) -> Vec<u8>) -> Memory {
    let mut m = Memory::default();
    // One pass over a zero-count stream gives every static chunk's VA and length.
    let empty = SaveGame::write_stream(false, &mut |_, len| vec![0; len]).expect("schema");
    let mut seen = std::collections::BTreeMap::<String, u32>::new();
    for c in &empty.chunks {
        let k = seen.entry(c.name.clone()).or_insert(0);
        let idx = *k;
        *k += 1;
        let at = match c.name.as_str() {
            "SzovegSzin" => Some(0xF16EB8 + 4 * idx),
            n => crate::save::static_chunk_va(n).or_else(|| crate::save::wf_chunk_va(n)),
        };
        if let Some(at) = at {
            m.write(at, &image(at, c.range.len()));
        }
    }
    // Arrays whose saved length depends on a count: their full extent.
    for (at, len) in [(va::PLANETS, 201 * PLANET_SIZE), (va::TECHS, TECHS * TECH_SIZE), (va::KEMEK, 9 * 0x3dc), (va::VEHICLES, 0x10 * 8),
        (0xF23A20, 0x968), (0xF24388, 0x968), (0xF24CF0, 0x4b8), (0xF251A8, 0x2000), (va::SCROLL, 0x40), (0xE40F70, 0x40), (0xBAF338, 0x30),
        // Unsaved globals the sim reads (a loaded save takes them from the image too: `BirodalomMIzik`, `BirodalomTamadhat`).
        (va::AI_EMPIRE, 19), (va::MAY_ATTACK, 19), (LANG, 4),
        // Tables the start reads from the image.
        (PGYARTAS0, 0x40)] {
        m.write(at, &image(at, len));
    }
    // The battlefields as their constructor leaves them at start-up (`WF::WF`).
    for i in 0..200u32 {
        let b = va::WARFARE + i * va::WARFARE_STRIDE;
        m.set_i32(b + 0x18, 1);
        m.set_u8(b + 0x184, 1);
        m.set_u8(b + 0x190, 1);
        m.set_i32(b + 0x334, -1);
        m.set_i32(b + 0x148, -1);
        for (o, v) in [(0x1a8, 0xC4016383u32), (0x1ac, 0x43F9EBEB), (0x1b0, 0xC4898000), (0x1b4, 0xBD4CCCCD), (0x1b8, 0x40C4A3D7)] {
            m.set_i32(b + o, v as i32);
        }
        for k in 0..18u32 {
            m.set_i32(b + 0x7fc + 8 * k, 200);
            m.set_i32(b + 0x800 + 8 * k, 300);
        }
    }
    m
}

/// `strncpy(dst, src, n)`: the text up to its NUL, the rest of the `n` bytes zero.
fn strncpy(dst: &mut [u8], src: &[u8], n: usize) {
    let src = crate::messages::cstr(src);
    for (i, d) in dst.iter_mut().take(n).enumerate() {
        *d = src.get(i).copied().unwrap_or(0);
    }
}

fn put_i16(r: &mut [u8], o: usize, v: i16) {
    r[o..o + 2].copy_from_slice(&v.to_le_bytes());
}
fn put_i32(r: &mut [u8], o: usize, v: i32) {
    r[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn rd_f32(r: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(r[o..o + 4].try_into().unwrap())
}
fn rd_i16(r: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([r[o], r[o + 1]])
}

impl<'r> Game<'r> {
    /// A new campaign (`CampaignGameStart` → `InitNewGame(seed, 0, campaign, difficulty)`), as its state right
    /// after the start (tick 0, before the first frame).
    pub fn new_campaign(cfg: &NewCampaign, data: &StartData, rules: &'r GameRules) -> Self {
        let mut g = Self::before_start(data, rules);
        g.start_campaign(cfg, data);
        g
    }

    /// The process before any game starts: the saved globals as the exe image holds them (and the battlefields
    /// as their constructor leaves them). [`Game::start_campaign`] starts a campaign on it; set [`Game::uninit`]
    /// in between to give appended records the original's heap bytes.
    pub fn before_start(data: &StartData, rules: &'r GameRules) -> Self {
        let mut state = SaveState::from_parts(saved_globals(data.image), Vec::new());
        state.story.buckets = vec![story::Queue::default(); story::BUCKETS];
        let mut g = Game::new(state, rules);
        g.fleet_name = Some(data.fleet_name.clone());
        g.design_texts = data.design_texts.clone();
        g
    }

    /// `CampaignGameStart` on [`Game::before_start`]'s state; the chunk template becomes the start's save stream.
    pub fn start_campaign(&mut self, cfg: &NewCampaign, data: &StartData) {
        self.campaign_start(cfg, data);
        self.state.rebuild_template();
    }

    /// A scenario (`Betolt_Scenario(file)` → `InitNewGame(ticks, 1, 0, 0)` and `LoadScenario(file, 0)`), as its
    /// state right after the start.
    pub fn new_scenario(cfg: &NewScenario, data: &StartData, rules: &'r GameRules) -> Self {
        let mut g = Self::before_start(data, rules);
        g.start_scenario(cfg, data);
        g
    }

    /// The multiplayer galaxy on [`Game::before_start`]'s state (or the lobby's, races and names written): the
    /// lobby's switches, `MultiPlayerMode` on, eight players, then `InitNewGame(seed, 2, 0, 1)` (spec §0, §3).
    pub fn start_mp_galaxy(&mut self, cfg: &NewMpGalaxy, data: &StartData) {
        let m = self.m();
        m.set_u8(va::MULTIPLAYER, 1);
        m.set_u8(0xBAE297, 1);
        m.set_i32(va::EN, cfg.en);
        m.set_i32(0xBB6798, cfg.en);
        m.set_i32(va::PROGRAM_PLAYERS, 8);
        m.set_i32(va::PROGRAM_PLAYERS2, 8);
        m.set_i32(va::PLANET_COUNT, cfg.planets);
        m.set_i32(0xBAE54C, cfg.speed);
        m.set_i32(0xBAE554, cfg.shape);
        m.set_i32(0xBAE558, cfg.battle_status);
        m.set_i32(0xBAE55C, cfg.server_time);
        m.set_i32(0xBAE544, 0);
        for i in 1..EMPIRES as i32 {
            let race = cfg.races.get(i as usize).copied().unwrap_or(match i {
                15 | 17 | 18 => 1,
                16 => 3,
                _ => i as u8,
            });
            m.set_u8(va::PLAYER_FAJ + i as u32, race);
            m.set_u8(va::PLAYER_KI + i as u32, if cfg.humans.contains(&i) { 1 } else { 2 });
            m.set_u8(va::EMPIRE_ALIVE + i as u32, (i <= 8 && cfg.alive.contains(&i)) as u8);
        }
        let init = Init { mode: 2, campaign: 0, difficulty: 1, seed: cfg.seed, ticks: cfg.ticks };
        self.init_new_game(&init, None, data, data.image);
        self.state.rebuild_template();
    }

    /// `SetScirmishGameOptions` (exe 0x503e80, spec §6) on [`Game::before_start`]'s state: the common prologue, the
    /// human, the menu's choices, then `InitNewGame(seed, 3, 0, Nehezseg)` (a generated galaxy, spec §0, §3).
    pub fn start_skirmish(&mut self, cfg: &NewSkirmish, data: &StartData) {
        self.prologue(data, 1);
        let m = self.m();
        m.set_i32(va::EN, cfg.en);
        m.set_u8(va::PLAYER_KI + cfg.en as u32, 1);
        m.set_i32(0xB7740C, cfg.random_id);
        m.set_i32(SK_AIS, cfg.ais);
        m.set_i32(SK_TECH, cfg.tech);
        m.set_i32(SK_PLANETS, cfg.planets);
        m.set_i32(SK_MONEY, cfg.money);
        let difficulty = if cfg.difficulty == 3 { 4 } else { cfg.difficulty };
        m.set_i32(SK_POPULATION, cfg.population);
        m.set_i32(va::NEHEZSEG, difficulty);
        let init = Init { mode: 3, campaign: 0, difficulty, seed: cfg.seed, ticks: cfg.ticks };
        self.init_new_game(&init, None, data, data.image);
        self.state.rebuild_template();
    }

    /// `Betolt_Scenario` (exe 0x4f5ba0, spec §6) on [`Game::before_start`]'s state.
    pub fn start_scenario(&mut self, cfg: &NewScenario, data: &StartData) {
        self.prologue(data, 2);
        let m = self.m();
        m.set_i32(va::EN, 1);
        m.set_u8(va::PLAYER_KI + 1, 1);
        let init = Init { mode: 1, campaign: 0, difficulty: cfg.difficulty, seed: cfg.seed, ticks: cfg.ticks };
        self.init_new_game(&init, None, data, data.image);
        let techs0 = (data.image)(TALALMANYOK0, TECHS * TECH_SIZE);
        let ops = self.load_scenario(&cfg.text, 0, &techs0);
        self.scenario_stories(ops);
        // FelfedezBirodalom(i, false) for the empires 1..=8 the human knows.
        let en = self.state.mem.i32(va::EN).clamp(0, 18) as usize;
        let knows: Vec<usize> = (1..9).filter(|&e| self.state.mem.u8(va::EMPIRE_KNOWS + (19 * en + e) as u32) != 0).collect();
        self.with_diplomacy(|d, w| {
            for &e in &knows {
                d.discover(w, e);
            }
        });
        self.battlefield_cameras();
        // SetIdoSpeed(1, false), then step 55's tech patches again.
        let m = self.m();
        m.set_i32(0xBAE258, 1);
        m.set_i32(va::IDO_SPEED, 1);
        m.set_i32(va::BATTLE_SPEED, 1);
        self.tech_patches();
        self.state.rebuild_template();
    }

    /// `TutorialSetCampaignGameOptions(n)` (exe 0x504250, re/rules/tutorial.md) on [`Game::before_start`]'s state:
    /// the §6 prologue (PlayerFaj[17] = 1), EN = 1, PlayerKi[1] = 1, `InitNewGame(ticks, 1, 0, 0)`, then
    /// `TutorialLoadScenario(n)` = `LoadScenario("tutorial/scn/tutorial<n>.scn", 0)` (`cfg.text`). Unlike
    /// `Betolt_Scenario` no empire is discovered, no camera set and no step-55 patch repeated. Verified byte-exact
    /// against chapters 1..=5 started in ig2.exe (`run_newgame.py --tutorial N`, tests/new_game.rs).
    pub fn start_tutorial(&mut self, cfg: &NewScenario, data: &StartData) {
        self.prologue(data, 1);
        let m = self.m();
        m.set_i32(va::EN, 1);
        m.set_u8(va::PLAYER_KI + 1, 1);
        // The exe passes difficulty 0 (exe 0x504250); the scenario file sets the game's own.
        let init = Init { mode: 1, campaign: 0, difficulty: 0, seed: cfg.seed, ticks: cfg.ticks };
        self.init_new_game(&init, None, data, data.image);
        let techs0 = (data.image)(TALALMANYOK0, TECHS * TECH_SIZE);
        let ops = self.load_scenario(&cfg.text, 0, &techs0);
        self.scenario_stories(ops);
        self.state.rebuild_template();
    }

    /// The story side of a scenario file's lines (`LoadScenario` mode 0: `V`, `B`, story starts).
    fn scenario_stories(&mut self, ops: Vec<ScenarioStory>) {
        let mut st = self.state.story();
        for op in ops {
            match op {
                ScenarioStory::Var(k, v) => st.v_set(k, v),
                ScenarioStory::Planet(p, k) => st.b_add_id(p, k),
                ScenarioStory::Start(id) => {
                    st.start_story(id, false);
                }
                ScenarioStory::SzStart(id, days) => st.sz_start(id, days),
            }
        }
        self.state.store_story(&st);
    }

    fn m(&mut self) -> &mut Memory {
        &mut self.state.mem
    }

    fn prec(&self, p: usize) -> Vec<u8> {
        self.state.mem.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE)
    }
    fn put_prec(&mut self, p: usize, r: &[u8]) {
        self.state.mem.write(va::PLANETS + (p * PLANET_SIZE) as u32, r);
    }

    fn rnd_stream(&mut self, stream: usize, lo: i32, hi: i32) -> i32 {
        let mut r = self.state.rng();
        let v = r.range(stream, lo, hi);
        self.state.store_rng(&r);
        v
    }

    fn campaign_start(&mut self, cfg: &NewCampaign, data: &StartData) {
        self.prologue(data, 1);
        let init = Init { mode: 0, campaign: cfg.campaign, difficulty: cfg.difficulty, seed: cfg.seed, ticks: cfg.ticks };
        self.init_new_game(&init, Some(cfg), data, data.image);
    }

    /// The callers' common prologue (spec §6; also `SingleStartGame`, `SetIdoSpeed(1, false)`): eight players of
    /// 18 empires, every empire the computer's, each its own race but 15..=18 (`race17`: 1 from the race menu, 2
    /// from the scenario menu), named after its race.
    fn prologue(&mut self, data: &StartData, race17: u8) {
        let m = self.m();
        m.set_i32(0xBAE544, 0);
        m.set_u8(va::MULTIPLAYER, 0);
        m.set_i32(0xBAE0AC, 1);
        m.set_i32(0xBAE258, 1);
        m.set_i32(va::IDO_SPEED, 1);
        m.set_i32(va::BATTLE_SPEED, 1);
        m.set_i32(0xBAE550, 1);
        m.set_i32(va::PROGRAM_PLAYERS, 8);
        m.set_i32(va::PROGRAM_PLAYERS2, 0x12);
        for i in 1..EMPIRES {
            let race = match i {
                15 | 18 => 1,
                16 => 3,
                17 => race17,
                _ => i as u8,
            };
            m.set_u8(va::PLAYER_KI + i as u32, 2);
            m.set_u8(va::PLAYER_FAJ + i as u32, race);
            // strcpy into the 11-byte slot: a longer name runs into the next one, which is written after it.
            let name = data.race_names.get(race as usize).cloned().unwrap_or_default();
            m.write(0xBAE588 + 11 * i as u32, &[crate::messages::cstr(&name), &[0]].concat());
        }
    }

    /// `InitNewGame(seed, mode, campaign, difficulty)` (exe 0x4e1130), spec §1: mode 0 (a campaign; `camp` its
    /// set-up) or 1 (a scenario, loaded after).
    fn init_new_game(&mut self, cfg: &Init, camp: Option<&NewCampaign>, data: &StartData, image: &dyn Fn(u32, usize) -> Vec<u8>) {
        let mode = cfg.mode;
        let ct = if mode != 0 { 0 } else if (1..=3).contains(&cfg.campaign) { cfg.campaign } else { 1 };
        let m = self.m();
        // 0: the scenario's leftovers.
        m.set_u8(0xBAD0E8, 0);
        m.set_i32(0xBAD108, 0);
        m.set_u8(0x996474, 0);
        m.write(0xF66F2C, &[0; 20]);
        // 2, 3.
        m.set_i32(va::CAMPAIGN_TYPE, ct);
        if cfg.difficulty > 0 {
            m.set_i32(va::NEHEZSEG, cfg.difficulty);
        }
        if !(1..=4).contains(&m.i32(va::NEHEZSEG)) {
            m.set_i32(va::NEHEZSEG, 2);
        }
        let difficulty = m.i32(va::NEHEZSEG);
        m.write(0xB79FB8, &[0; 0x48]);
        // 4: InitializeStarLog, 5: InitializeInfoCharts.
        for k in 0..200u32 {
            m.set_i32(STARLOG + 0x2c * k, 0);
            m.set_u8(STARLOG + 0x2c * k + 8, 0);
        }
        m.set_i32(0xB84BF0, 0);
        m.set_i32(0xB84BF4, 0);
        for k in 0..0x44c0 / 4 {
            m.set_f32(INFO_CHARTS + 4 * k, -1.0);
        }
        m.set_i32(0xB84C1C, -1);
        m.set_i32(va::INFO_MONTH, -1);
        m.set_i32(0xB84C70, 0);
        m.set_i32(0xB84C74, 0);
        m.set_i32(0xB84C00, 0x135);
        let s = 2700.0f64.sqrt() as f32;
        m.set_i32(0xB84C04, (((s + 50.0) - s) + (s * 2.0) / 5.0) as i32);
        // 6: the campaign's human.
        if mode == 0 {
            let en = [1, 3, 5][ct as usize - 1];
            m.set_i32(va::EN, en);
            m.set_u8(va::PLAYER_KI + 1, 2);
            m.set_u8(va::PLAYER_KI + en as u32, 1);
            m.set_i32(0xBAE26C, ct);
        }
        let en = m.i32(va::EN);
        // 7: RandomInit(seed): srand, then 21 MSVC rand() values into RandSeed[0..21).
        let mut libc = Libc(cfg.seed);
        for k in 0..21u32 {
            let v = libc.rand();
            m.set_i32(va::RAND_SEED + 4 * k, v);
        }
        // 8: the on-screen texts.
        m.write(0xF16EB8, &[0; 32]);
        // 9: the battlefields' ids.
        for i in 0..200u32 {
            let b = va::WARFARE + i * va::WARFARE_STRIDE;
            m.set_i32(b + 0x14, i as i32 + 1);
            m.set_i32(b + 0x18c, i as i32 + 1);
            m.set_i32(b + 0x1d0, i as i32 + 1);
            m.set_i32(b + 0x148, -1);
            m.set_i32(b + 0x8d0, 0);
        }
        // 10: per-planet tables and the empires' GDP/tax.
        for p in 1..=200u32 {
            for at in [va::FORTRESS, va::GROUND, va::ION_GUN, va::FIRE_BRIGADE, va::NEEDED_MORALE, va::BUILDINGS_OCCUPIED] {
                m.set_i32(at + 4 * p, 0);
            }
        }
        for e in 1..EMPIRES as u32 {
            m.set_i32(va::FULL_GDP + 4 * e, 0);
            m.set_i32(va::FULL_TAX + 4 * e, 0);
        }
        // 11: the fleet, ship and building arrays reallocated (slot 0 only).
        for p in 1..=200u32 {
            m.set_i32(va::BUILDING_BLOCKS + 4 * p, 0);
            m.set_i32(va::BUILDING_SLOTS + 4 * p, 0);
        }
        self.state.fleets = vec![[0; 0x80]];
        self.state.ships = vec![[0; 6]];
        self.state.waypoints = Vec::new();
        self.state.buildings = (0..=200).map(|p| if p == 0 { Vec::new() } else { vec![[0; BUILDING_SIZE]] }).collect();
        // The battlefields as they are now become the chunk template the ground war reads them from.
        self.state.rebuild_template();
        // 12: NEWGAME_Story.
        let mut st = self.state.story();
        st.new_game();
        self.state.store_story(&st);
        let m = self.m();
        m.set_i32(va::STORY_DESIGN_NEXT, 10000);
        m.set_i32(va::STORY_DESIGN_COUNT, 0);
        // 13: the empires.
        let pp2 = m.i32(va::PROGRAM_PLAYERS2);
        let human = |m: &Memory, e: usize| m.u8(va::PLAYER_KI + e as u32) == 1;
        const KNOWN: [i32; 5] = [0, 0x708, 0x708, 0x12c, 0];
        for e in 1..EMPIRES {
            let mut money = 50000.0;
            if human(m, e) && difficulty == 2 {
                money = 80000.0;
            } else if human(m, e) && difficulty == 1 {
                money = 125000.0;
            }
            if mode == 3 {
                money = m.i32(SK_MONEY) as f64;
            }
            m.set_f64(va::MONEY + 8 * e as u32, money);
            // The multiplayer galaxy only switches the humans on; the lobby set the others. A skirmish has no
            // computer empires past the eight players.
            if mode != 2 {
                m.set_u8(va::EMPIRE_ALIVE + e as u32, (e as i32 <= pp2) as u8);
            } else if human(m, e) {
                m.set_u8(va::EMPIRE_ALIVE + e as u32, 1);
            }
            if mode == 3 && !human(m, e) && e > 8 {
                m.set_u8(va::EMPIRE_ALIVE + e as u32, 0);
            }
            for k in 1..EMPIRES {
                m.set_u8(va::EMPIRE_KNOWS + (19 * e + k) as u32, 0);
                let since = if human(m, k) || difficulty != 4 { KNOWN[difficulty as usize] } else { 600 };
                m.set_i32(va::EMPIRE_KNOWN_SINCE + (0x4c * e + 4 * k) as u32, since);
            }
            m.set_u8(va::EMPIRE_KNOWS + (19 * e + e) as u32, 1);
            m.set_u8(va::EMPIRE_KNOWS + (19 * e + 10) as u32, 1);
            for at in [va::TRADE_LAST, va::TRADE_THIS, va::TAX_LAST, va::TAX_THIS] {
                m.set_i32(at + 4 * e as u32, 0);
            }
        }
        // 14: a skirmish switches off 7 − HanyGepiJatekos computer empires of 1..=8 at random.
        let ais = m.i32(SK_AIS);
        if mode == 3 && ais != 7 && 7 - ais >= 0 {
            for _ in 0..7 - ais {
                loop {
                    let x = self.rnd_stream(1, 1, 8) as u32;
                    let m = self.m();
                    if m.u8(va::EMPIRE_ALIVE + x) != 0 && m.u8(va::PLAYER_KI + x) != 1 {
                        m.set_u8(va::EMPIRE_ALIVE + x, 0);
                        break;
                    }
                }
            }
        }
        let m = self.m();
        // 15: no planets until the campaign map or the scenario; the multiplayer galaxy keeps the lobby's count, a
        // skirmish takes the menu's; both are generated, empire 3 getting three more buildings on each of its planets.
        if mode == 3 {
            let n = m.i32(SK_PLANETS);
            m.set_i32(va::PLANET_COUNT, n);
        }
        if mode == 2 || mode == 3 {
            let n = m.i32(va::PLANET_COUNT);
            m.set_i32(va::PLANET_COUNT2, n);
            self.generate_stars(data);
            if self.state.alive()[3] {
                for p in 1..=self.state.planet_count() {
                    if self.prec(p)[0x20] == 3 {
                        for kind in [0x30, 0x32, 0x34] {
                            self.place(p, kind, true, 0);
                        }
                    }
                }
            }
        } else {
            m.set_i32(va::PLANET_COUNT, 0);
            m.set_i32(va::PLANET_COUNT2, 0);
        }
        let m = self.m();
        // 16.
        m.set_u8(va::EMPIRE_ALIVE + 9, 1);
        m.set_u8(va::EMPIRE_ALIVE + 10, 1);
        m.set_u8(va::DIPLOMACY_ENABLED, 1);
        m.set_u8(0xBAE3AE, 1);
        m.set_u8(va::SPY_ENABLED, 1);
        // 17: MoneyInit(true, 0).
        self.money_init_new();
        // 18: the calendar.
        let m = self.m();
        m.set_i32(va::EV, 2327);
        m.set_i32(va::HONAP, 3);
        m.set_i32(va::NAP, 12);
        m.set_i32(va::OLD_NAP, 12);
        m.set_i32(va::IDO, GameTime::from_date(2327, 3, 12, 0).0);
        m.set_i32(va::IDO2, 0);
        m.set_i32(0xBAE054, 0);
        m.set_i32(0xBAE05C, 0);
        m.set_i32(0xF6919C, 0);
        m.set_i32(0xBAE0AC, 1);
        m.set_i32(va::MP_SPACE_BATTLE, 0);
        m.set_i32(0xBAD13C, 0);
        // 19: the spies.
        for i in 1..=8u32 {
            for at in [va::SPY_COUNT, va::SPY_CAPACITY, va::SPY_CENTRES, va::SPIES_WORKING] {
                m.set_i32(at + 4 * i, 0);
            }
            for p in 1..=200u32 {
                m.set_i32(va::SPY_CENTRES_BY_PLANET + 0x324 * i + 4 * p, 0);
            }
            for k in 0..13u32 {
                let r = va::KEMEK + 0x3dc * i + 0x4c * k;
                m.write(r + 0x10, &[0; 2]);
                m.write(r + 0x16, &[0; 3]);
                m.write(r + 0x1a, &[0; 2]);
                m.set_i32(r + 0x38, 0);
            }
        }
        let races = self.state.races();
        for i in 1..=8usize {
            if races[i] != 5 {
                let kemek0 = image(0xBB1FA8, 0x3dc);
                self.m().write(va::KEMEK + 0x3dc * i as u32, &kemek0);
                self.with_spies(|sp, w| {
                    sp.new_spy(w, i);
                });
            }
        }
        let m = self.m();
        for i in 14..=26u32 {
            m.set_u8(va::MISSIONS + 0x20 * i, 0);
        }
        // 20, 21: the counts; every empire's designs reallocated (records 0..=0x2e).
        m.set_i32(va::WAYPOINT_COUNT, 0);
        m.set_i32(va::FLEET_COUNT, 0);
        m.set_i32(va::SHIP_COUNT, 0);
        m.set_i32(va::VEHICLE_COUNT, 0);
        for e in 1..EMPIRES as u32 {
            m.set_i32(0xB9C078 + 4 * e, 0);
            m.set_i32(0xB9C028 + 4 * e, DESIGNS as i32);
        }
        // 22: the tech table (`Talalmanyok0`, each empire's status by its race).
        let t0 = image(TALALMANYOK0, TECHS * TECH_SIZE);
        let mut techs = t0.clone();
        for t in 1..TECHS {
            if let Some(n) = data.tech_names.get(t) {
                strncpy(&mut techs[t * TECH_SIZE..], n, 0x10);
            }
        }
        for t in 1..TECHS {
            for e in 1..=pp2.min(18) as usize {
                techs[t * TECH_SIZE + 0x45 + e] = t0[t * TECH_SIZE + 0x45 + races[e] as usize];
            }
        }
        self.m().write(va::TECHS, &techs);
        // 23: the design templates (`Tervek0`), each fitted (`TervAutoDesign`) and recomputed.
        self.state.designs = vec![Vec::new()];
        for e in 1..EMPIRES {
            let mut list = Vec::with_capacity(DESIGNS + 1);
            for i in 0..=DESIGNS {
                let mut r: [u8; DESIGN_SIZE] = image(TERVEK0 + (DESIGN_SIZE * i) as u32, DESIGN_SIZE).try_into().unwrap();
                if let Some(n) = data.design_names.get(i) {
                    strncpy(&mut r, n, 0x14);
                }
                put_i16(&mut r, 0x40, races[e] as i16);
                list.push(r);
            }
            self.state.designs.push(list);
        }
        self.with_ai(|_, w, _| {
            for e in 1..EMPIRES {
                for i in 1..DESIGN_TEMPLATES {
                    w.auto_design(e, i, 0);
                    w.recompute_design(e, i);
                }
            }
        });
        // 24: InitAdvancedTalalmanyok(mode): the AIs' head start in a campaign, the menu's tech level in a skirmish.
        self.advanced_techs(mode, difficulty, image);
        // 25: InitScirmishBolygok.
        if mode == 3 {
            self.skirmish_planets(image);
        }
        // 26: up to each empire's `OsszHanyTerv` (a skirmish's head start added the human's automatic designs).
        let counts: Vec<usize> = (0..EMPIRES).map(|e| self.state.mem.i32(0xB9C028 + 4 * e as u32).max(0) as usize).collect();
        self.with_ai(|_, w, _| {
            for e in 1..EMPIRES {
                for i in DESIGN_TEMPLATES..=counts[e] {
                    w.auto_design(e, i, 0);
                    w.recompute_design(e, i);
                }
            }
        });
        let m = self.m();
        // 27, 28.
        m.set_i32(va::TRADE_OFFER_COUNT, 0);
        for e in 1..EMPIRES as u32 {
            m.set_i32(va::COLONY_SHIP_POOL + 4 * e, 0);
            m.set_i32(va::TERRAFORMER_POOL + 4 * e, 0);
            m.set_i32(va::FLEET_NUMBERS + 4 * e, 0);
            m.set_i32(0x9ABAE8 + 4 * e, -1);
            for o in [4, 8, 0xc] {
                m.set_i32(va::LABS + 0x10 * e + o, 0);
                m.set_i32(va::LAB_BUILDINGS + 0x10 * e + o, 0);
            }
        }
        // 29: every planet unseen, without satellites, stations or problems, with a random colour and look
        // (none yet but in a generated galaxy).
        for p in 1..=self.state.mem.i32(va::PLANET_COUNT2).max(0) as usize {
            let mut r = self.prec(p);
            r[0xda..0xfe].fill(0);
            r[0xc5..0xd7].fill(0);
            put_i32(&mut r, 0x114, 0);
            r[0x11c..0x168].fill(0);
            put_i32(&mut r, 0x174, 0);
            put_i32(&mut r, 0x178, 0);
            let (c1, c2, c3) = (self.rnd_stream(1, 0x80, 0xff), self.rnd_stream(1, 0x80, 0xff), self.rnd_stream(1, 0x80, 0xff));
            put_i32(&mut r, 0x17c, (c1 << 16) | (c2 << 8) | c3);
            for (o, lo, hi) in [(0x180, 0, 99), (0x181, 5, 9), (0x182, 8, 15), (0x183, 0, 99), (0x184, 20, 99), (0x185, 50, 99)] {
                r[o] = self.rnd_stream(1, lo, hi) as u8;
            }
            self.put_prec(p, &r);
            let m = self.m();
            m.set_i32(0xB99340 + 4 * p as u32, 0);
            m.set_i32(0xB99668 + 4 * p as u32, 0);
        }
        let m = self.m();
        // 30.
        m.set_i32(va::GLOBAL_MORALE, 0);
        m.set_i32(va::GLOBAL_MORALE_SPEED, 0);
        for e in 1..EMPIRES as u32 {
            m.set_i32(0xB9A9B8 + 4 * e, 0);
        }
        // 31: the starmap.
        m.set_i32(va::STARMAP_MODE, 1);
        m.set_u8(0xB9A306, 0);
        m.set_i32(va::AKT_PLANET, 1);
        let n = self.state.planet_count();
        if let Some(p) = (1..=n).rev().find(|&p| self.prec(p)[0x20] as i32 == en) {
            self.m().set_i32(va::AKT_PLANET, p as i32);
        }
        let m = self.m();
        m.set_i32(va::AKT_FLEET, 1);
        m.set_i32(0xB9A97C, 0);
        m.set_u8(0xB99339, 0);
        m.set_i32(va::WORMHOLE_COUNT, 0);
        // 32: the radar tables.
        let radar = crate::radar::RadarTables::new_game(&self.rules.radar);
        let m = self.m();
        for e in 1..EMPIRES {
            for k in 0..8 {
                m.set_f32(va::RADAR2 + (0x20 * e + 4 * k) as u32, radar.range[e][k]);
                m.set_f32(va::RADAR2_SQ + (0x20 * e + 4 * k) as u32, radar.range_sq[e][k]);
            }
            m.set_f32(va::RADAR_MULT + 4 * e as u32, radar.mult[e]);
        }
        // 33: UI defaults, then NEWGAME_Research's.
        m.set_i32(0xB9C074, 0);
        m.set_i32(0xB9C0C4, 1);
        m.set_i32(0xB9AA04, 5);
        for e in 1..EMPIRES as u32 {
            m.set_u8(0xB9C014 + e, 0);
        }
        m.set_i32(0xB9AAEC, 1);
        m.set_i32(0xB9AA64, 3);
        m.set_i32(0xB9AA10, 1);
        m.set_i32(0xB9A9B4, 3);
        m.set_i32(0x919598, 3);
        m.set_i32(0xB9AA08, 0x21);
        // 35: NEWGAME_Colony.
        m.set_i32(0x9ABAE4, 0);
        m.set_i32(0x9ABB70, 0);
        m.set_i32(0x9ABB74, 0);
        // 36.
        m.set_i32(0xBB2384, 1);
        m.set_i32(va::SPY_REPORT_COUNT, 0);
        for e in 1..=8u32 {
            m.set_i32(va::KIDNAP + 4 * e, 0);
        }
        // 37: NEWGAME_Messages.
        m.set_i32(0xDABDD4, 0);
        for at in [0xD90DC8, 0xDABDB8, 0xDABDB4, 0xDABDD8, 0xDABDA0, 0xDABDA4, 0xDABDA8, 0xDABDAC, 0xDABDB0, 0xDABDC0] {
            m.set_i32(at, -1);
        }
        m.set_u8(0xD909BA, 1);
        m.set_u8(0xDA65E7, 1);
        m.set_u8(0xD909AE, 0);
        m.set_u8(0xD909AF, 0);
        m.set_i32(0xD909A8, 0);
        m.set_i32(0xDABE38, data.message_kinds);
        // 38: NEWGAME_Diplomacy.
        m.set_i32(va::MAX_PLAYERS, 0);
        for i in 0..20u32 {
            let r = va::CONTACTS + 0xc4 * i;
            m.set_i32(r, 0);
            m.set_i32(r + 8, 0);
            m.set_u8(r + 0xc, 0);
            m.set_i32(r + 0x10, 0);
            m.set_u8(r + 0x14, 1);
            m.write(r + 0x16, &[0; 2]);
            for o in [0x1c, 0x20, 0x24] {
                m.set_i32(r + o, -1);
            }
        }
        m.set_u8(va::DIP_STARTED, 1);
        let mut libc = Libc(cfg.ticks);
        for e in 1..EMPIRES as u32 {
            m.set_i32(0xA41CD8 + 4 * e, libc.rand() % 10);
            m.set_i32(0xA41D28 + 4 * e, libc.rand() % 10);
            m.set_i32(0xA41D78 + 4 * e, libc.rand() % 12 + 1);
            m.set_i32(0xA41DC8 + 4 * e, libc.rand() % 10);
            m.set_i32(va::RULER_AGE + 4 * e, libc.rand() % 10 + 30);
            m.set_u8(va::GREETED + e, 0);
        }
        m.set_i32(va::DIP_OLD_NAP, -1);
        self.libc = libc;
        // 40: Research_TalalmanyUjraSzamol (a map's or scenario's load runs it again in its tail).
        let g = self.globals();
        let ctx = self.research_context(&g);
        let mut techs = self.state.tech();
        techs.recompute(ctx.players, ctx.all_techs);
        self.state.store_tech(&techs);
        // 41: NEWGAME_Trade.
        let m = self.m();
        m.set_i32(va::TRADE_ETA_MONTH, -1);
        m.set_i32(va::TRADE_OLD_DAY, -1);
        for e in 1..EMPIRES as u32 {
            m.set_i32(va::TRADE_DELIVERY_COUNTS + 4 * e, 0);
        }
        // 42, 43: the scroll queues; Story_Generate.
        m.set_i32(va::SCROLL_COUNT, 0);
        m.set_i32(va::SCROLL_HISTORY_COUNT, 0);
        m.set_i32(0xDABDD0, 0);
        m.write(0xF2F8AC, &[0; 17]);
        m.set_i32(0xF22738, 0);
        m.set_i32(0xF23A18, 30);
        let mut st = self.state.story();
        st.generate();
        self.state.store_story(&st);
        // 44: InitStory(mode): the campaign map in mode 0.
        match camp {
            Some(c) => self.init_story_campaign(c, data),
            None => {
                let mut st = self.state.story();
                st.init_story_cleared();
                self.state.store_story(&st);
            }
        }
        // 45: ClearGameInfo.
        self.m().set_i32(0xE3E3A4, 0);
        // 46: the starting fleets, then the traders.
        if mode != 1 {
            self.starting_fleets(difficulty, image);
        }
        // 47: INIT_MI.
        self.init_mi();
        // 48: the campaign's first story.
        if mode == 0 {
            let mut st = self.state.story();
            st.start_story(400, false);
            self.state.store_story(&st);
        }
        // 50, 51: the extra planets.
        let en = en as usize;
        if difficulty == 1 && mode != 1 && mode != 2 {
            self.easy_extras(en);
        }
        if mode == 0 {
            for e in [1, 3, 5] {
                if e != en {
                    self.add_planet(e);
                }
            }
        }
        // 52: StarmapSetSideView, copied to the starmap camera.
        self.side_view(en);
        // 53, 54: the battlefields' cameras; owned planets' autobuild and tax.
        self.battlefield_cameras();
        let n = self.state.mem.i32(va::PLANET_COUNT2).max(0) as u32;
        for p in 1..=n as usize {
            let mut r = self.prec(p);
            if r[0x20] != 0 {
                r[0x6c] = 1;
                r[0x6d] = 2;
                r[0x34] = 3;
                self.put_prec(p, &r);
            }
        }
        // 55, 56: tech patches.
        self.tech_patches();
        let mp = self.state.mem.u8(va::MULTIPLAYER) != 0;
        let m = self.m();
        if difficulty == 1 && !mp {
            m.set_u8(va::TECHS + 83 * TECH_SIZE as u32 + 0x45 + en as u32, 5);
        }
        // 57.
        let v = self.rnd_stream(1, 0xa0, 0xf0) * 0x32;
        self.m().set_i32(STORY_ADAT + 196, v);
        // 58: SetIdoSpeed(1, true), SetIdoPause(false, true), FRISSIT_Viszony.
        let m = self.m();
        m.write(0xF66F2C, &[0; 20]);
        m.set_i32(va::SPEED_LOCKED, 0);
        m.set_i32(0xBAE258, 1);
        m.set_i32(va::IDO_SPEED, 1);
        m.set_i32(va::BATTLE_SPEED, 1);
        m.set_u8(0xBAE264, 0);
        self.diplomacy_daily(false);
    }

    /// Step 53: every planet's battlefield camera at its default (`WF[i]` for i < `HanyBolygo2`).
    fn battlefield_cameras(&mut self) {
        let n = self.state.mem.i32(va::PLANET_COUNT2).max(0) as u32;
        const CAMERA: [u32; 5] = [0xC4016383, 0x43F9EBEB, 0xC4898000, 0xBF490FDB, 0];
        for i in 0..n {
            let p = i as usize + 1;
            if self.state.ground_fields().contains_key(&p) {
                let mut f = self.state.take_ground_field(p);
                f.tail.f1a8 = CAMERA;
                f.countdown = -1;
                self.state.store_ground_field(p, f);
                continue;
            }
            let b = va::WARFARE + i * va::WARFARE_STRIDE;
            let m = self.m();
            for (k, v) in CAMERA.into_iter().enumerate() {
                m.set_i32(b + 0x1a8 + 4 * k as u32, v as i32);
            }
            m.set_i32(b + 0x148, -1);
        }
    }

    /// Step 55: techs 89, 124 and 159 unknown to every empire (and unseen by the first eight), tech 147's `+0x2e`
    /// set to 7.
    fn tech_patches(&mut self) {
        let pp2 = self.state.mem.i32(va::PROGRAM_PLAYERS2);
        let m = self.m();
        for e in 1..=pp2.min(18) as u32 {
            for t in [89u32, 124, 159] {
                m.set_u8(va::TECHS + t * TECH_SIZE as u32 + 0x45 + e, 0);
                if e <= 8 {
                    for o in 1..=8 {
                        m.set_u8(va::TECHS + t * TECH_SIZE as u32 + 0xc0 + 9 * e + o, 0);
                    }
                }
            }
            m.set_u8(va::TECHS + 0x9EAA, 7);
        }
    }

    /// `INIT_MI` (exe 0x5ade70, `ai-empire.md` §13): every empire's AI state and flags reset and the AI empires'
    /// designs (the AI's part, `AiState::init`), then the relations reset, `FRISSIT_Viszony`, empires without planets
    /// dead. The relations loop zeroes `DiplomaciaMikorHivtaUtoljara[a·9 + b]` for a, b up to `ProgramPlayers2`,
    /// which runs past the table into `MiTervMutato` (its rows 0..=7, after the designs): kept.
    fn init_mi(&mut self) {
        let pp2 = self.state.mem.i32(va::PROGRAM_PLAYERS2).clamp(0, 18) as u32;
        let m = self.m();
        for e in 1..=pp2 {
            m.set_u8(va::AI_EMPTY_PLANETS + e, 0);
            m.set_u8(va::EMPIRE_ALIVE + e, 1);
            m.set_i32(va::EMPIRE_COUNTDOWN + 4 * e, 100);
            m.set_i32(va::AI_DANGER + 4 * e, 1);
        }
        self.with_ai(|st, w, h| st.init(w, h));
        let m = self.m();
        for a in 1..=pp2 {
            for b in 1..=pp2 {
                let r = va::RELATIONS + 0x2d2 * a + 0x26 * b;
                for (k, v) in [(0x11, 5000u16), (5, 0), (0, 0), (1, 5000), (2, 5000), (3, 5000), (4, 0), (6, 5000), (7, 5000), (8, 5000), (9, 0),
                    (10, 0), (0xb, 0), (0xc, 0), (0xd, 0), (0xe, 5000), (0xf, 5000), (0x10, 0)] {
                    m.write(r + 2 * k, &v.to_le_bytes());
                }
                m.set_u8(r + 0x25, 3);
                m.set_u8(r + 0x24, 0);
                m.set_i32(va::LAST_CONTACT + 4 * (9 * a + b), 0);
            }
        }
        self.diplomacy_daily(false);
        let m = self.m();
        for e in 1..=pp2 {
            if m.i32(va::EMPIRE_PLANETS + 4 * e) == 0 {
                m.set_u8(va::EMPIRE_ALIVE + e, 0);
            }
        }
    }

    /// `MoneyInit(true, 0)` (exe 0x540060): every empire's sliders `{0, 40, 40, 20, 10, 10}`, the shares, the
    /// pools dealt from its money and the reserves cleared.
    fn money_init_new(&mut self) {
        let m = self.m();
        for e in 1..EMPIRES as u32 {
            for (k, v) in [0.0, 40.0, 40.0, 20.0, 10.0, 10.0].into_iter().enumerate() {
                m.set_f64(va::SLIDERS + 0x30 * e + 8 * k as u32, v);
            }
            m.set_f64(va::RESERVES + 0x30 * e, 0.0);
        }
        self.money_init(0);
    }

    /// `InitAdvancedTalalmanyok(mode)` (exe 0x4e0f70): in a campaign the AIs' head start by difficulty; in a
    /// skirmish every live empire of 1..=8 gets each tech it may have whose `ScirmishTalalmanyFejlesztes` is at most
    /// the menu's level (one more for a hard game's AIs), then `Research_TalalmanyUjraSzamol`; then every tech two of
    /// the first eight empires both own marks each as seen by the other.
    fn advanced_techs(&mut self, mode: i32, difficulty: i32, image: &dyn Fn(u32, usize) -> Vec<u8>) {
        let g = self.globals();
        let ctx = self.research_context(&g);
        let develop = |game: &mut Self, e: usize, t: usize| {
            let mut techs = game.state.tech();
            let mut res = game.state.research();
            let mut events: Vec<ResearchEvent> = Vec::new();
            research::developed(&mut techs, &mut res, e, t, &game.rules.research, &ctx, &mut events);
            game.state.store_tech(&techs);
            game.state.store_research(&res);
            // `TervezAutomatikus`: the human's new designs.
            for ev in events {
                if let ResearchEvent::AutoDesign { empire, tech } = ev {
                    game.automatic_design(empire as usize, tech);
                }
            }
        };
        if mode == 0 {
            let mut list = Vec::new();
            if difficulty > 2 {
                list.extend([6, 0x12, 0x16]);
            }
            if difficulty > 1 {
                list.push(0x99);
            }
            if difficulty > 3 {
                list.extend([0x92, 0xa2]);
            }
            for e in 1..=g.pp2.min(18) {
                if g.human[e] {
                    continue;
                }
                for &t in &list {
                    develop(self, e, t);
                }
            }
        } else if mode == 3 {
            let start = image(SKIRMISH_TECHS, 4 * TECHS);
            let level0 = self.state.mem.i32(SK_TECH);
            for e in 1..=8usize {
                let m = &self.state.mem;
                if m.u8(va::EMPIRE_ALIVE + e as u32) == 0 {
                    continue;
                }
                let level = level0 + (m.u8(va::PLAYER_KI + e as u32) == 2 && difficulty > 3) as i32;
                for t in 1..TECHS {
                    let status = self.state.mem.u8(va::TECHS + (t * TECH_SIZE + 0x45 + e) as u32);
                    let at = i32::from_le_bytes(start[4 * t..4 * t + 4].try_into().unwrap());
                    if status != 0 && status != 5 && at <= level {
                        develop(self, e, t);
                    }
                }
            }
            let g = self.globals();
            let ctx = self.research_context(&g);
            let mut techs = self.state.tech();
            techs.recompute(ctx.players, ctx.all_techs);
            self.state.store_tech(&techs);
        }
        let m = self.m();
        for t in 1..TECHS as u32 {
            let rec = va::TECHS + t * TECH_SIZE as u32;
            for a in 1..=8 {
                if m.u8(rec + 0x45 + a) != 5 {
                    continue;
                }
                for b in 1..=8 {
                    if m.u8(rec + 0x45 + b) == 5 {
                        m.set_u8(rec + 0xc0 + 9 * a + b, 1);
                        m.set_u8(rec + 0xc0 + 9 * b + a, 1);
                    }
                }
            }
        }
    }

    /// `InitScirmishBolygok` (exe 0x4e2a10, spec §3): each live empire of 1..=8 a random planet of its own peopled
    /// by the menu's population setting, more colonies for the larger settings (the free planets nearest it), each
    /// with the buildings its people need and the power plants to run them.
    fn skirmish_planets(&mut self, image: &dyn Fn(u32, usize) -> Vec<u8>) {
        let m = &self.state.mem;
        let (n, ais) = (m.i32(va::PLANET_COUNT), m.i32(SK_AIS));
        let (mut first, second) = ([12000, 3500, 8500], [15000, 23000, 18000]);
        let count = match m.i32(SK_POPULATION) {
            1 => 1 + (n > 30) as i32 + (n > 60) as i32,
            2 => {
                first = [24000, 14000, 21000];
                n / 15
            }
            3 => {
                first = [51000, 14000, 21000];
                n / (ais + 3)
            }
            _ => 1,
        };
        let prod = self.state.mem.read(PGYARTAS0, 0x40);
        // Without tech 93 no planet holds more than 32000 people (then 31700..=32300).
        let peopled = |game: &mut Self, p: usize, e: usize, pop: i32| {
            let mut r = game.prec(p);
            put_i32(&mut r, 0x28, pop);
            if game.state.mem.u8(va::TECHS + (93 * TECH_SIZE + 0x45 + e) as u32) != 5 && pop > 32000 {
                put_i32(&mut r, 0x28, game.rnd_stream(1, 0, 600) + 0x7bd4);
            }
            game.put_prec(p, &r);
            game.civil_buildings(p, image);
            game.energy_buildings(p, image);
        };
        for e in 1..=8usize {
            if self.state.mem.u8(va::EMPIRE_ALIVE + e as u32) == 0 {
                continue;
            }
            let h = self.random_planet_of(e, 2);
            let pop = first[0] + self.rnd_stream(1, 0, 600) - 300;
            peopled(self, h, e, pop);
            for k in 1..count {
                let r = self.prec(h);
                let q = self.nearest_free(rd_f32(&r, 0x18) as i32, rd_f32(&r, 0x1c) as i32);
                self.generate_planet(q, e, e as u8, true, false, &prod);
                let pop = if count < 3 {
                    first[count as usize] + self.rnd_stream(1, 0, 600) - 300
                } else {
                    second[(k as u32 % 3) as usize] * self.rnd_stream(1, 80, 120) / 100
                };
                peopled(self, q, e, pop);
            }
        }
    }

    /// Whether the owner of `p` has tech `t` (status 5).
    fn owner_has(&self, p: usize, t: usize) -> bool {
        let o = self.prec(p)[0x20] as usize;
        self.state.mem.u8(va::TECHS + (t * TECH_SIZE + 0x45 + o) as u32) == 5
    }

    /// The kinds of `p`'s used building slots (`Epuletek[p][1..=HanyEpuletLefoglalva[p]]`, the kind byte).
    fn building_kinds(&self, p: usize) -> Vec<u8> {
        let n = self.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32).max(0) as usize;
        self.state.buildings.get(p).map_or_else(Vec::new, |b| b.iter().skip(1).take(n).map(|r| r[0]).collect())
    }

    /// `RndClass(5)` over the building kinds `lo..=hi` whose tech `p`'s owner has, the `i`-th kind weighted `i`
    /// (non-logging draws on stream 5): the pick, `lo` when none.
    fn pick_kind(&mut self, p: usize, lo: usize, hi: usize, kinds: &[u8]) -> usize {
        let (mut t, mut total) = (lo, 0);
        for k in lo..=hi {
            if self.owner_has(p, kinds[0x24 * k] as usize) {
                let w = (k - lo + 1) as i32;
                total += w;
                if self.rnd_stream(5, 1, total) <= w {
                    t = k;
                }
            }
        }
        t
    }

    /// `GenerateCivilBuildings(p)` (exe 0x5c20c0): living quarters, hospitals, shops, police, a park and the
    /// amusements, as many as the planet's people call for (the better kinds when the owner has their techs).
    fn civil_buildings(&mut self, p: usize, image: &dyn Fn(u32, usize) -> Vec<u8>) {
        let kinds = image(BUILDING_KINDS, 0x24 * 56);
        let mut n = [0i32; 15];
        for k in self.building_kinds(p) {
            if (1..=14).contains(&k) {
                n[k as usize] += 1;
            }
        }
        let v = i32::from_le_bytes(self.prec(p)[0x28..0x2c].try_into().unwrap());
        while n[2] * 7000 + 5000 + n[3] * 12000 < v {
            let t = if self.owner_has(p, 83) { self.rnd_stream(5, 2, 3) as usize } else { 2 };
            n[t] += 1;
            self.place(p, t as u8, true, 0);
        }
        while n[4] * 9000 + 5000 + n[5] * 18000 < v {
            let t = if self.owner_has(p, 85) { self.rnd_stream(5, 4, 5) as usize } else { 4 };
            n[t] += 1;
            self.place(p, t as u8, true, 0);
        }
        for (kind, step) in [(6, 0x1d4c), (7, 10000)] {
            while n[kind] * step + step < v {
                self.place(p, kind as u8, true, 0);
                n[kind] += 1;
            }
        }
        if v > 20000 && n[8] == 0 {
            self.place(p, 8, true, 0);
            n[8] += 1;
        }
        if self.owner_has(p, kinds[0x24 * 13] as usize) {
            while n[13] * 30000 + 25000 < v {
                self.place(p, 13, true, 0);
                n[13] += 1;
            }
        }
        while n[10] * 4000 + 3000 + n[11] * 8000 + n[12] * 14000 + n[13] * 30000 < v {
            let t = self.pick_kind(p, 10, 13, &kinds);
            self.place(p, t as u8, true, 0);
            n[t] += 1;
        }
    }

    /// `GenerateEnergyBuildings(p)` (exe 0x5c23f0): power plants until the planet's buildings have the energy they
    /// use (the better kinds the owner has).
    fn energy_buildings(&mut self, p: usize, image: &dyn Fn(u32, usize) -> Vec<u8>) {
        let kinds = image(BUILDING_KINDS, 0x24 * 56);
        let field = |k: usize, o: usize| i32::from_le_bytes(kinds[0x24 * k + o..0x24 * k + o + 4].try_into().unwrap());
        let mut e = 0i32;
        for k in self.building_kinds(p) {
            let k = k as usize;
            if k == 1 || (0x34..=0x37).contains(&k) {
                e += field(k, 0x14) * 100;
            } else {
                e -= field(k, 0xc);
            }
        }
        while e < 0 {
            let t = self.pick_kind(p, 0x34, 0x37, &kinds);
            self.place(p, t as u8, true, 0);
            e += field(t, 0x14) * 100;
        }
    }

    /// `EpuletNewSearchLocation(p, kind, done, dmg, −1, −1)` (exe 0x53cca0): the first free spot, the record, the
    /// building's turrets.
    pub(super) fn place(&mut self, p: usize, kind: u8, done: bool, damage: i32) {
        let g = self.globals();
        let econ = self.economy_context(&g);
        let mut c = self.state.colony();
        let cells = self.state.vehicle_cells(p);
        let mut slot = 0;
        if let Some((x, y)) = self.rules.placement.locate(&c.planets[p], kind, &cells, None) {
            slot = c.new_building(p, kind, done, (x as f32, y as f32), damage, &self.rules.side, &econ);
        }
        self.state.store_colony(&c);
        self.fill_fresh_buildings();
        if slot > 0 {
            let owner = self.planet_owner(p) as i32;
            self.with_war(p, |w| w.add_building_turrets(kind, owner, slot as i32));
        }
    }

    /// The `n`-th (`rndlog(stream, 1, count)`) planet of `e` the human sees (`st_bSearch(−e, mode)`, exe 0x603d30).
    fn random_planet_of(&mut self, e: usize, stream: usize) -> usize {
        let en = self.state.mem.i32(va::EN).max(0) as usize;
        let n = self.state.planet_count();
        let mine: Vec<usize> = (1..=n).filter(|&p| {
            let r = self.prec(p);
            r[0x20] as usize == e && rd_i16(&r, 0xd8 + 2 * en) >= 0
        }).collect();
        let k = self.rnd_stream(stream, 1, mine.len() as i32);
        if k >= 1 { mine.get(k as usize - 1).copied().unwrap_or(0) } else { 0 }
    }

    /// `st_bGetNearest(x, y, 0)` (exe 0x603980): the closest free planet the human sees.
    fn nearest_free(&self, x: i32, y: i32) -> usize {
        let en = self.state.mem.i32(va::EN).max(0) as usize;
        let (mut best, mut pick) = (i32::MAX, 0);
        for p in 1..=self.state.planet_count() {
            let r = self.prec(p);
            if r[0x20] != 0 || rd_i16(&r, 0xd8 + 2 * en) < 0 {
                continue;
            }
            let (dy, dx) = (y as f32 - rd_f32(&r, 0x1c), x as f32 - rd_f32(&r, 0x18));
            let d = dy * dy + dx * dx;
            if d < best as f32 {
                best = d as i32;
                pick = p;
            }
        }
        pick
    }

    /// `Generate_Planet0(p, owner, race, initial)` (exe 0x59adf0): a fresh colony (`initial`: an empire's home world
    /// in a generated galaxy, more people).
    fn generate_planet0(&mut self, p: usize, owner: usize, race: u8, image: &[u8], initial: bool) {
        let pop = if initial { self.rnd_stream(1, 0x22c4, 0x238c) + 2000 } else { self.rnd_stream(1, 0xb54, 0xc1c) };
        let mut r = self.prec(p);
        r[0x20] = owner as u8;
        r[0x21] = race;
        put_i32(&mut r, 0x28, pop);
        put_i32(&mut r, 0x2c, 0);
        put_i32(&mut r, 0x30, 500);
        r[0x34] = 3;
        for o in [0x38, 0x3c, 0x40, 0x44, 0x58, 0x5c, 0x68, 0x70, 0x74] {
            put_i32(&mut r, o, 0);
        }
        r[0x54] = 0;
        r[0x55] = 0;
        r[0x6c] = 1;
        r[0x6d] = 2;
        r[0x78..0xb8].copy_from_slice(image);
        put_i16(&mut r, 0xd8 + 2 * owner, 1000);
        self.put_prec(p, &r);
    }

    /// `AddPlanet(e)` (exe 0x5911f0): the human's story planets 10001..=10003 if free, else the free planet nearest
    /// a random one of `e`'s; colonised for `e` with a colony hub. Returns the planet (0: none).
    fn add_planet(&mut self, e: usize) -> usize {
        let en = self.state.mem.i32(va::EN).max(0) as usize;
        let story = self.state.story();
        let id = |k: i32| story.planets.iter().find(|x| x.0 == k).map_or(0, |x| x.1 as usize);
        let mut p = 0;
        if e == en {
            for k in [0x2711, 0x2712, 0x2713] {
                p = id(k);
                if p != 0 && self.prec(p)[0x20] == 0 {
                    break;
                }
            }
        }
        if p == 0 || self.prec(p)[0x20] != 0 {
            let h = self.random_planet_of(e, story::STREAM);
            let r = self.prec(h);
            p = self.nearest_free(rd_f32(&r, 0x18) as i32, rd_f32(&r, 0x1c) as i32);
        }
        let race = self.state.races()[e];
        let prod = self.state.mem.read(PGYARTAS0, 0x40);
        self.generate_planet0(p, e, race, &prod, false);
        let dmg = self.rnd_stream(1, 0, 0);
        if p != 0 {
            self.place(p, 1, true, dmg);
        }
        p
    }

    /// Step 50 (`Nehezseg == 1`): the human's first planet gets buildings and a second planet.
    fn easy_extras(&mut self, en: usize) {
        let n = self.state.planet_count();
        let h = (1..=n).find(|&p| self.prec(p)[0x20] as usize == en).unwrap_or(n + 1);
        let q = self.add_planet(en);
        if h <= n && q != 0 {
            let mut r = self.prec(h);
            put_i32(&mut r, 0x24, 0x96);
            self.put_prec(h, &r);
            let mut r = self.prec(q);
            put_i32(&mut r, 0x24, 0x7d);
            self.put_prec(q, &r);
            for k in [0xe, 7, 0x34, 4, 6] {
                self.place(h, k, true, 0);
            }
            for k in [0x15, 0xf, 0x10, 4, 6, 7, 3, 0x34, 0x34, 0x34, 10] {
                self.place(q, k, true, 0);
            }
            for (p, pop) in [(h, 0x3E2C), (q, 0x3B24)] {
                let mut r = self.prec(p);
                put_i32(&mut r, 0x28, pop);
                self.put_prec(p, &r);
            }
        }
    }

    /// Step 46: every live empire 1..=8 a fleet (3 destroyers, more for an easy human; 8 + 2 vehicles) 15 above a
    /// random planet of its own; then the eight traders.
    fn starting_fleets(&mut self, difficulty: i32, image: &dyn Fn(u32, usize) -> Vec<u8>) {
        let g = self.globals();
        let ctx = self.production_context(&g, g.speed);
        for e in 1..=8 {
            if !g.alive[e] {
                continue;
            }
            let h = self.random_planet_of(e, 2);
            let r = self.prec(h);
            let name = if g.human[e] { b"Conqueror".to_vec() } else { ctx.default_fleet_name.clone() };
            let mut yard = self.shipyard();
            self.fresh_fleet_slot(&mut yard);
            let mut ev = Vec::new();
            let f = yard.new_fleet(&name, e as u8, 0, rd_f32(&r, 0x18), rd_f32(&r, 0x1c) - 15.0, 0.0, &ctx, &mut ev);
            // The third destroyer: not for empire 5 in a multiplayer game.
            let mut ships = if g.multiplayer && e == 5 { vec![0x2b, 0x2b] } else { vec![0x2b, 0x2b, 0x2b] };
            if !g.multiplayer && g.human[e] {
                match difficulty {
                    1 => ships.extend([0x2b, 0x2c, 0x2d]),
                    2 => ships.extend([0x2b, 0x2c]),
                    _ => {}
                }
            }
            for d in ships {
                yard.new_ship(f, d, &mut ev);
            }
            yard.new_vehicles(f as i32, 0x2a, 8, &mut ev);
            yard.new_vehicles(f as i32, 0x29, 2, &mut ev);
            self.store_shipyard(&yard, &ev);
            self.m().set_i32(va::AKT_FLEET, f as i32);
            if e == g.en {
                self.m().set_i32(va::AKT_PLANET, h as i32);
            }
        }
        for e in 1..=8u32 {
            let a = self.rnd_stream(1, -250, 250);
            let b = self.rnd_stream(1, -250, 250);
            let ptr = u32::from_le_bytes(image(TRADER_NAMES + 4 * (e - 1), 4).try_into().unwrap());
            let mut name = image(ptr, 16);
            name.truncate(name.iter().position(|&c| c == 0).unwrap_or(name.len()));
            let mut yard = self.shipyard();
            self.fresh_fleet_slot(&mut yard);
            let mut ev = Vec::new();
            // The exe passes the second draw as x and the first as y (spec §10).
            let f = yard.new_fleet(&name, 9, 10000 + e as i16, b as f32, a as f32, 5.0, &ctx, &mut ev);
            yard.fleets[f].fleet.flags &= !0x0800;
            yard.new_ship(f, 8, &mut ev);
            self.store_shipyard(&yard, &ev);
            self.m().set_i32(va::AKT_FLEET, f as i32);
        }
    }

    /// `FlottaNew` appends a slot when none is free: the record starts as the heap's bytes ([`Game::uninit`]), which
    /// the fields it doesn't write keep. Offered to the shipyard as a free slot.
    fn fresh_fleet_slot(&mut self, yard: &mut crate::production::Shipyard) {
        if (1..yard.fleets.len()).any(|g| yard.fleets[g].fleet.owner == crate::fleet::DELETED) {
            return;
        }
        let raw = self.fresh::<0x80>(super::Fresh::Fleet(yard.fleets.len()));
        let mut fleet = crate::fleet::Fleet::from_record(&raw);
        fleet.owner = crate::fleet::DELETED;
        yard.fleets.push(crate::production::YardFleet { fleet, totals: Default::default() });
    }

    /// `StarmapSetSideView` (exe 0x5b0c60) and the copy to the starmap camera: the box of the planets the human
    /// sees or owns, zoomed out to fit.
    fn side_view(&mut self, en: usize) {
        let m = &self.state.mem;
        let mut zoom = m.f32(0xB9A978);
        let (mut max_x, mut max_y, mut min_x, mut min_y) = (-300.0f32, -300.0f32, 300.0f32, 300.0f32);
        let mut count = 0;
        for p in 1..=self.state.planet_count() {
            let r = self.prec(p);
            if rd_i16(&r, 0xd8 + 2 * en) > 0 || r[0x20] as usize == en {
                let (x, y) = (rd_f32(&r, 0x18), rd_f32(&r, 0x1c));
                max_x = max_x.max(x);
                min_x = min_x.min(x);
                max_y = max_y.max(y);
                min_y = min_y.min(y);
                count += 1;
            }
        }
        let (mut tx, mut ty) = (self.state.mem.f32(0xF16F1C), self.state.mem.f32(0xF16F20));
        if count > 0 {
            tx = (max_x + min_x) * 0.5;
            ty = (min_y + max_y) * -0.5;
            let zx = (((max_x - min_x).abs() * 0.5 * 600.0) / 320.0 - 600.0).abs();
            let zy = ((((-max_y) - (-min_y)).abs() * 0.5 * 600.0) / 240.0 - 600.0).abs();
            zoom = zx.min(zy) / 1.5;
            self.m().set_f32(0x92798C, zoom);
        }
        let m = self.m();
        m.set_f32(0xB9A968, tx);
        m.set_f32(0xB9A96C, ty);
        m.set_f32(0xB9A978, zoom);
        m.set_f32(0xB9A974, f32::from_bits(0xBF733333));
        m.set_f32(0xB9A970, 0.0);
    }

    /// `InitStory(0)` (exe 0x5c2940, ported in `campaign.rs`) with the campaign map loaded into this game.
    fn init_story_campaign(&mut self, cfg: &NewCampaign, data: &StartData) {
        let words: Vec<i32> = (data.image)(CampaignTables::VA, 4 * CampaignTables::WORDS)
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        let tables = CampaignTables::new(words);
        let mut st = self.state.story();
        let rng = self.state.rng();
        let mut w = InitWorld { game: self, cfg, data, tables, events: Vec::new(), rng };
        st.init_story(0, &mut w);
        let events = std::mem::take(&mut w.events);
        let rng = w.rng.clone();
        self.state.store_rng(&rng);
        self.state.store_story(&st);
        for ev in events {
            match ev {
                StoryEvent::Mirror { x, y } => self.mirror(x, y),
                StoryEvent::PlaceBuilding { planet, kind } => self.place(planet as usize, kind as u8, true, 0),
                _ => {}
            }
        }
    }

    /// `InitStory`'s mirror flags: every planet's x / y sign flipped.
    fn mirror(&mut self, x: bool, y: bool) {
        for p in 1..=self.state.planet_count() {
            let mut r = self.prec(p);
            if x {
                r[0x1b] ^= 0x80;
            }
            if y {
                r[0x1f] ^= 0x80;
            }
            self.put_prec(p, &r);
        }
    }

    /// `LoadScenario(file, mode)` (exe 0x545b40, spec §7): mode 1 (`InitStory`'s campaign map) takes only the lines
    /// that act in every mode (planets, buildings, garrisons, the empires' standing, story variables); mode 0 (a
    /// scenario) the whole saved state besides. Then the tail. Returns what the story engine gets, in file order.
    fn load_scenario(&mut self, text: &str, mode: i32, techs0: &[u8]) -> Vec<ScenarioStory> {
        let mut out = Vec::new();
        let m = self.m();
        m.set_i32(va::STORY_LIST_COUNTS + 12, 0);
        m.set_i32(va::PLANET_COUNT, 0);
        m.set_i32(va::PLANET_COUNT2, 0);
        let mut cur = Cursor::default();
        if mode == 0 {
            self.scenario_preamble(&mut cur);
        }
        let mut p = 0usize;
        let mut b: Option<usize> = None;
        for line in ig2_formats::text::lines(text) {
            let t = line.trim_start_matches(|c: char| c.is_whitespace() || c == ',').trim_end_matches(|c: char| c.is_control());
            let (key, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
            let toks: Vec<&str> = rest.split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()).collect();
            let int = |k: usize| toks.get(k).map_or(0, |s| atoi(s));
            let flt = |k: usize| toks.get(k).map_or(0.0, |s| atof(s));
            let key = key.to_ascii_lowercase();
            if mode == 0 && self.scenario_line(&mut cur, &key, rest, &toks) {
                continue;
            }
            let m = &mut self.state.mem;
            match key.as_str() {
                "birodalomero" => (1..EMPIRES).for_each(|e| m.set_f32(va::EMPIRE_POWER + 4 * e as u32, flt(e - 1))),
                "birodalomhanybolygo" => (1..EMPIRES).for_each(|e| m.set_i32(va::EMPIRE_PLANETS + 4 * e as u32, int(e - 1))),
                "birodalomel" => (1..EMPIRES).for_each(|e| m.set_u8(va::EMPIRE_ALIVE + e as u32, (int(e - 1) != 0) as u8)),
                "birodalomvisszaszamlalas" => (1..EMPIRES).for_each(|e| m.set_i32(va::EMPIRE_COUNTDOWN + 4 * e as u32, int(e - 1))),
                "vset" => out.push(ScenarioStory::Var(int(0), int(1))),
                "szstart" => out.push(ScenarioStory::SzStart(int(0), int(1))),
                "story" => out.push(ScenarioStory::Start(int(0))),
                "planet" => {
                    p = m.i32(va::PLANET_COUNT) as usize + 1;
                    m.set_i32(va::PLANET_COUNT, p as i32);
                    m.set_i32(va::PLANET_COUNT2, p as i32);
                    m.set_i32(va::BUILDINGS_OCCUPIED + 4 * p as u32, 0);
                    m.set_i32(va::BUILDING_SLOTS + 4 * p as u32, 0);
                    m.write(va::PLANETS + (p * PLANET_SIZE) as u32, &[0; PLANET_SIZE]);
                    b = None;
                }
                "pazon" => out.push(ScenarioStory::Planet(p as i32, int(0))),
                "building" if p != 0 => {
                    let at = 4 * p as u32;
                    let (blocks, used) = (m.i32(va::BUILDING_BLOCKS + at), m.i32(va::BUILDING_SLOTS + at));
                    if blocks <= used {
                        // `OldReallocate` by 20: the new records are the heap's bytes.
                        m.set_i32(va::BUILDING_BLOCKS + at, blocks + 20);
                        let have = self.state.buildings[p].len();
                        for j in have..blocks as usize + 21 {
                            let r = self.fresh::<BUILDING_SIZE>(super::Fresh::Building { planet: p, slot: j });
                            self.state.buildings[p].push(r);
                        }
                    }
                    let m = &mut self.state.mem;
                    m.set_i32(va::BUILDING_SLOTS + at, used + 1);
                    let k = m.i32(va::BUILDINGS_OCCUPIED + at) + 1;
                    m.set_i32(va::BUILDINGS_OCCUPIED + at, k);
                    let mut r = [0u8; BUILDING_SIZE];
                    r[10] = p as u8;
                    self.state.buildings[p][k as usize] = r;
                    b = Some(k as usize);
                }
                "ptank" if p != 0 => {
                    // `TankNew(−p, d, n)` at once: the vehicles enter the planet's battlefield (`AddOwnTankToPlanet`,
                    // rearranged) before the planet's buildings and their turrets.
                    let (d, n) = (int(0) as i16, if toks.len() > 1 { int(1) } else { 1 });
                    let mut yard = self.shipyard();
                    let mut ev = Vec::new();
                    yard.new_vehicles(-(p as i32), d, n, &mut ev);
                    self.store_shipyard(&yard, &ev);
                    self.events.extend(ev.into_iter().map(super::GameEvent::Production));
                    self.battlefield_hand_offs();
                }
                k if p != 0 => {
                    if let Some(j) = b.filter(|_| k.starts_with('e')) {
                        let r = &mut self.state.buildings[p][j];
                        building_field(r, k, &toks);
                    } else {
                        let mut r = m.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE);
                        if planet_field(&mut r, k, rest, &toks) {
                            m.write(va::PLANETS + (p * PLANET_SIZE) as u32, &r);
                        }
                    }
                }
                _ => {}
            }
        }
        // The fleets' ships and vehicles (`HajoNew`, `TankNew`), in file order.
        for (f, unit) in std::mem::take(&mut cur.units) {
            let mut yard = self.shipyard();
            let mut ev = Vec::new();
            match unit {
                Unit::Ships(d, n) => (0..n).for_each(|_| yard.new_ship(f, d, &mut ev)),
                Unit::Vehicles(d, n) => yard.new_vehicles(f as i32, d, n, &mut ev),
            }
            self.store_shipyard(&yard, &ev);
        }
        self.map_tail(techs0);
        if cur.redesign {
            self.redesign_all();
        }
        self.design_names_from_techs();
        out
    }

    /// Mode 0's preamble (spec §7): no spies, 40 designs per empire (the rest kept aside: a `TERV` line reuses the
    /// record, which keeps its bytes), every relation at its defaults.
    fn scenario_preamble(&mut self, cur: &mut Cursor) {
        let m = self.m();
        for e in 1..=8u32 {
            m.set_i32(va::SPY_COUNT + 4 * e, 0);
        }
        for e in 1..EMPIRES as u32 {
            m.set_i32(va::DESIGN_COUNT + 4 * e, 0x28);
        }
        for a in 1..EMPIRES as u32 {
            for b in 1..EMPIRES as u32 {
                let r = va::RELATIONS + 0x2d2 * a + 0x26 * b;
                for (o, v) in [(0, 0u16), (2, 5000), (4, 5000), (6, 5000), (8, 0), (0xa, 0), (0x1c, 5000), (0x1e, 5000), (0x20, 0), (0x22, 2500)] {
                    m.write(r + o, &v.to_le_bytes());
                }
                m.set_u8(r + 0x24, 0);
                m.set_u8(r + 0x25, 3);
            }
        }
        cur.spare = vec![Vec::new(); EMPIRES];
        for e in 1..EMPIRES {
            if let Some(list) = self.state.designs.get_mut(e) {
                if list.len() > 0x29 {
                    cur.spare[e] = list.split_off(0x29);
                }
            }
        }
    }

    /// One mode-0-only line of `LoadScenario` (spec §7). `false` for the lines every mode reads.
    fn scenario_line(&mut self, cur: &mut Cursor, key: &str, rest: &str, toks: &[&str]) -> bool {
        let int = |k: usize| toks.get(k).map_or(0, |s| atoi(s));
        let flt = |k: usize| toks.get(k).map_or(0.0, |s| atof(s));
        let m = &mut self.state.mem;
        let per_empire_i32 = |m: &mut Memory, at: u32| (1..EMPIRES).for_each(|e| m.set_i32(at + 4 * e as u32, int(e - 1)));
        match key {
            "en" => m.set_i32(va::EN, int(0)),
            "money" => (1..EMPIRES).for_each(|e| m.set_f64(va::MONEY + 8 * e as u32, flt(e - 1) as f64)),
            "ev" => m.set_i32(va::EV, int(0)),
            "honap" => m.set_i32(va::HONAP, int(0)),
            "nap" => m.set_i32(va::NAP, int(0)),
            "ido" => m.set_i32(va::IDO, int(0)),
            "programmode" => m.set_i32(0xBAE544, int(0)),
            "programdifficulty" => m.set_i32(0xBAE548, int(0)),
            "programsebesseg" => m.set_i32(0xBAE54C, int(0)),
            "programplayers" => m.set_i32(va::PROGRAM_PLAYERS, int(0)),
            "programplayers2" => m.set_i32(va::PROGRAM_PLAYERS2, int(0)),
            "programmachines" => m.set_i32(0xBAE550, int(0)),
            "programgalaxyshape" => m.set_i32(0xBAE554, int(0)),
            "playerki" => (1..EMPIRES).for_each(|e| m.set_u8(va::PLAYER_KI + e as u32, int(e - 1) as u8)),
            "playerfaj" => (1..EMPIRES).for_each(|e| m.set_u8(va::PLAYER_FAJ + e as u32, int(e - 1) as u8)),
            "birodalomismeri" => {
                let row = int(0).clamp(0, 18) as u32;
                (1..EMPIRES).for_each(|k| m.set_u8(va::EMPIRE_KNOWS + 19 * row + k as u32, (int(k) != 0) as u8));
            }
            "randseed" => (1..=20).for_each(|k| m.set_i32(va::RAND_SEED + 4 * k as u32, int(k - 1))),
            "csaladtagelrabolvaki" => (1..=8).for_each(|e| m.set_i32(va::KIDNAP + 4 * e as u32, int(e - 1))),
            "csaladtagelrabolvakialtal" => (1..=8).for_each(|e| m.set_i32(0xBB4080 + 4 * e as u32, int(e - 1))),
            "aktplanet" => m.set_i32(va::AKT_PLANET, int(0)),
            "aktfleet" => m.set_i32(va::AKT_FLEET, int(0)),
            "starmapmode" => m.set_i32(va::STARMAP_MODE, int(0)),
            "starmapxstart" => m.set_f32(0xB9A968, flt(0)),
            "starmapystart" => m.set_f32(0xB9A96C, flt(0)),
            "starmaprotate" => m.set_f32(0xB9A970, flt(0)),
            "starmappitch" => m.set_f32(0xB9A974, flt(0)),
            "starmapzoom" => m.set_f32(0xB9A978, flt(0)),
            "starmapplanetkering" => m.set_f32(0xB9A97C, flt(0)),
            "kutatassurgos" => (1..EMPIRES).for_each(|e| m.set_u8(0xB9C014 + e as u32, (int(e - 1) != 0) as u8)),
            "researchaktfejlesztett" => per_empire_i32(m, 0xB9A9B8),
            "researchterulet" => m.set_i32(0xB9A9B4, int(0)),
            "researchmezo" => m.set_i32(0xB9AA04, int(0)),
            "researchtalalmany" => m.set_i32(0xB9AA08, int(0)),
            "laborszint" => {
                let row = int(0).clamp(0, 18) as u32;
                (1..=3).for_each(|k| m.set_i32(va::LABS + 0x10 * row + 4 * k as u32, int(k)));
            }
            "designtervsorszam" => m.set_i32(0xB9AA10, int(0)),
            "akttervmutato" => m.set_i32(0xB9AAEC, int(0)),
            "nehezseg" => m.set_i32(va::NEHEZSEG, int(0)),
            "diplomaciatiltas" => m.set_u8(va::DIPLOMACY_ENABLED, 0),
            // The exe clears the diplomacy switch here too, not trade's (spec §7).
            "tradetiltas" => m.set_u8(va::DIPLOMACY_ENABLED, 0),
            "spytiltas" => m.set_u8(va::SPY_ENABLED, 0),
            "viszonynemvaltozhat" => m.set_i32(STORY_ADAT + 176, 1),
            "tervredesign" => cur.redesign = true,
            "spy" => {
                let e = int(0).clamp(0, 8) as u32;
                let k = m.i32(va::SPY_COUNT + 4 * e) + 1;
                m.set_i32(va::SPY_COUNT + 4 * e, k);
                let r = va::KEMEK + 0x3dc * e + 0x4c * k as u32;
                m.write(r, &[0; 0x4c]);
                cur.spy = Some(r);
            }
            "talalmany" => cur.tech += 1,
            "terv" => {
                let e = int(0).clamp(0, 18) as usize;
                let i = m.i32(va::DESIGN_COUNT + 4 * e as u32) + 1;
                m.set_i32(va::DESIGN_COUNT + 4 * e as u32, i);
                let i = i as usize;
                while self.state.designs[e].len() <= i {
                    let j = self.state.designs[e].len();
                    let r = if j - 0x29 < cur.spare[e].len() { cur.spare[e][j - 0x29] } else { self.fresh::<DESIGN_SIZE>(super::Fresh::Design { empire: e, design: j }) };
                    self.state.designs[e].push(r);
                }
                cur.design = Some((e, i));
            }
            "fleet" => {
                let owner = int(2) as u8;
                let mut r = [0u8; 0x80];
                r[0x15] = owner;
                r[0x30..0x34].copy_from_slice(&flt(0).to_le_bytes());
                r[0x34..0x38].copy_from_slice(&flt(1).to_le_bytes());
                let name = self.fleet_name.clone().unwrap_or_default();
                strncpy(&mut r, &name, 0x14);
                self.state.fleets.push(r);
                let n = self.state.fleets.len() as i32 - 1;
                let m = self.m();
                m.set_i32(va::FLEET_COUNT, n);
                let at = va::FLEET_NUMBERS + 4 * owner.min(18) as u32;
                m.set_i32(at, m.i32(at) + 1);
                cur.fleet = Some(n as usize);
            }
            "dummyfleet" => {
                let n = self.state.fleets.len();
                let mut r = self.fresh::<0x80>(super::Fresh::Fleet(n));
                r[0x15] = 0xff;
                self.state.fleets.push(r);
                self.m().set_i32(va::FLEET_COUNT, n as i32);
                cur.fleet = Some(n);
            }
            "viszony" => {
                let (a, b) = (int(0).clamp(0, 18) as u32, int(1).clamp(0, 18) as u32);
                cur.relation = Some(va::RELATIONS + 0x2d2 * a + 0x26 * b);
            }
            "" => return false,
            k => {
                // A record's field: the newest record of its kind (none yet: the line is still consumed).
                let (at, len): (Option<u32>, usize) = match k.as_bytes()[0] {
                    b's' => (cur.spy, 0x4c),
                    b'r' => (Some(va::TECHS + (cur.tech.min(TECHS - 1) * TECH_SIZE) as u32), TECH_SIZE),
                    b'v' => (cur.relation, 0x26),
                    b't' => {
                        let mut scratch = [0u8; DESIGN_SIZE];
                        let r = match cur.design {
                            Some((e, i)) => &mut self.state.designs[e][i],
                            None => &mut scratch,
                        };
                        return design_field(r, k, rest, toks);
                    }
                    b'f' => {
                        let mut scratch = [0u8; 0x80];
                        let r = match cur.fleet {
                            Some(f) => &mut self.state.fleets[f],
                            None => &mut scratch,
                        };
                        return match fleet_field(r, k, rest, toks) {
                            Some(u) => {
                                if let (Some(u), Some(f)) = (u, cur.fleet) {
                                    cur.units.push((f, u));
                                }
                                true
                            }
                            None => false,
                        };
                    }
                    _ => return false,
                };
                let mut r = at.map_or_else(|| vec![0; len], |a| m.read(a, len));
                let ok = match k.as_bytes()[0] {
                    b's' => spy_field(&mut r, k, rest, toks),
                    b'r' => tech_field(&mut r, k, toks),
                    _ => relation_field(&mut r, k, toks),
                };
                if let (true, Some(a)) = (ok, at) {
                    m.write(a, &r);
                }
                return ok;
            }
        }
        true
    }

    /// `tervredesign` (spec §7 tail 9): the live empires' scenario designs refitted and recomputed, named after
    /// their hull tech, then `FlottakUjraSzamol`.
    fn redesign_all(&mut self) {
        let alive = self.state.alive();
        let list: Vec<(usize, usize)> = (1..=8)
            .filter(|&e| alive[e])
            .flat_map(|e| (DESIGN_TEMPLATES..self.state.designs[e].len()).map(move |i| (e, i)))
            .filter(|&(e, i)| rd_i16(&self.state.designs[e][i], 0x16) >= 0)
            .collect();
        self.with_ai(|_, w, _| {
            for &(e, i) in &list {
                w.auto_design(e, i, 0);
                w.recompute_design(e, i);
            }
        });
        self.design_names_from_techs();
        self.with_ai(|_, w, _| w.recount());
    }

    /// The tail's last step (spec §7 tail 10): unless the language is Hungarian, every live empire's scenario
    /// design takes its hull tech's name.
    fn design_names_from_techs(&mut self) {
        if self.state.mem.i32(LANG) == 1 {
            return;
        }
        let alive = self.state.alive();
        for e in 1..=8 {
            if !alive[e] {
                continue;
            }
            for i in DESIGN_TEMPLATES..self.state.designs[e].len() {
                let r = self.state.designs[e][i];
                if rd_i16(&r, 0x16) < 0 {
                    continue;
                }
                let t = i32::from_le_bytes(r[0x48..0x4c].try_into().unwrap()).clamp(0, TECHS as i32 - 1) as u32;
                let name = self.state.mem.read(va::TECHS + t * TECH_SIZE as u32, 0x10);
                strncpy(&mut self.state.designs[e][i], &name, 0x13);
            }
        }
    }

    /// `LoadScenario`'s tail (spec §7, every mode).
    fn map_tail(&mut self, techs0: &[u8]) {
        let n = self.state.planet_count();
        for p in 1..=200usize {
            let mut r = self.prec(p);
            r[0x98..0xa0].copy_from_slice(&[1, 2, 3, 4, 1, 2, 3, 4]);
            self.put_prec(p, &r);
        }
        for p in 1..=n {
            let owner = self.prec(p)[0x20];
            if owner == 0 {
                continue;
            }
            let used = self.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32).max(0) as usize;
            for j in 1..=used {
                let r = self.state.buildings[p][j];
                if r[0] != 0 && rd_i16(&r, 8) == 0 {
                    self.with_war(p, |w| {
                        w.add_building_turrets(r[0], owner as i32, j as i32);
                        w.building_built(j as i32);
                    });
                }
            }
        }
        for f in self.state.fleets.iter_mut().skip(1) {
            if rd_i16(f, 0x52) & 0x400 != 0 {
                f[0x15] = 0xff;
            }
        }
        let en = self.state.mem.i32(va::EN);
        self.m().set_i32(0xBB6798, en);
        self.money_init_new();
        let m = self.m();
        for e in 1..=8u32 {
            let mut k = 0u32;
            while k as i32 <= m.i32(va::SPY_COUNT + 4 * k) {
                let at = va::KEMEK + 0x3dc * e + 0x4c * k + 0x12;
                match m.u8(at) {
                    0x0b => m.set_u8(at, 4),
                    0x05 => m.set_u8(at, 7),
                    _ => {}
                }
                k += 1;
            }
        }
        self.refit_buildings();
        let pp2 = self.state.mem.i32(va::PROGRAM_PLAYERS2).clamp(0, 18) as u32;
        for e in 1..=pp2 {
            for t in 1..TECHS as u32 {
                let at = t * TECH_SIZE as u32 + 0x45 + e;
                // The default status indexed by the empire, not its race (spec §7, kept).
                if techs0[at as usize] == 0 {
                    let m = self.m();
                    m.set_u8(va::TECHS + at, 0);
                    if e < 9 {
                        for o in 1..=8 {
                            m.set_u8(va::TECHS + t * TECH_SIZE as u32 + 0xc0 + 9 * e + o, 0);
                        }
                    }
                }
            }
        }
        let g = self.globals();
        let ctx = self.research_context(&g);
        let mut techs = self.state.tech();
        techs.recompute(ctx.players, ctx.all_techs);
        self.state.store_tech(&techs);
        self.refresh_planets();
    }

    /// `FRISSIT_Planet(false, 0)` (exe 0x474930): every planet's display values.
    fn refresh_planets(&mut self) {
        let g = self.globals();
        let mut ctx = self.economy_context(&g);
        let mut c = self.state.colony();
        c.display_refresh(&self.rules.side, &mut ctx);
        self.state.store_colony(&c);
        self.store_economy_context(&ctx);
    }

    /// `Generate_StarNames()` (exe 0x59b860): the planets grouped into constellations around spread-out centres,
    /// each named "<constellation> <k>"; one random member of each gets story id 499 + c. Returns those ids and the
    /// constellation count.
    fn star_names(&mut self, names: &[Vec<u8>]) -> (Vec<(i32, i32)>, i32) {
        let n = self.state.planet_count();
        let k = match n {
            0..51 => n / 7,
            51..101 => n / 9,
            101..151 => n / 11,
            _ => n / 13,
        }
        .min(15);
        let pos: Vec<(f32, f32)> = (0..=n).map(|p| {
            let r = self.prec(p);
            (rd_f32(&r, 0x18), rd_f32(&r, 0x1c))
        }).collect();
        let d2 = |a: usize, b: usize| {
            let (dx, dy) = (pos[a].0 - pos[b].0, pos[a].1 - pos[b].1);
            (dx * dx + dy * dy) as i32
        };
        let mut label = vec![0usize; n + 1];
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); k + 1];
        if k > 0 {
            let first = self.rnd_stream(1, 1, n as i32) as usize;
            label[first] = 1;
            members[1].push(first);
            for c in 2..=k {
                let (mut best, mut pick) = (0, 0);
                for p in 1..=n {
                    if label[p] != 0 {
                        continue;
                    }
                    let d = (1..=n).filter(|&q| label[q] != 0 && members[label[q]][0] == q).map(|q| d2(p, q)).min().unwrap_or(0);
                    if d > best {
                        best = d;
                        pick = p;
                    }
                }
                label[pick] = c;
                members[c].push(pick);
            }
            for _ in k..n {
                let (mut best, mut pick) = (0i64, (0, 0));
                for p in 1..=n {
                    if label[p] != 0 {
                        continue;
                    }
                    let sums: Vec<i64> = (0..=k).map(|c| if c == 0 { 0 } else { members[c].iter().map(|&q| d2(p, q) as i64).sum() }).collect();
                    for c in 1..=k {
                        if members[c].len() >= 20 {
                            continue;
                        }
                        let other = (1..=k).filter(|&c2| c2 != c).map(|c2| sums[c2]).fold(10_000_000, i64::min);
                        let score = (other / (sums[c] + 1)) * 100 / members[c].len() as i64;
                        if score > best {
                            best = score;
                            pick = (p, c);
                        }
                    }
                }
                let (p, c) = pick;
                if p == 0 {
                    break;
                }
                label[p] = c;
                members[c].push(p);
            }
        }
        for p in 1..=n {
            let mut r = self.prec(p);
            r[..0x14].fill(0);
            r[..8].copy_from_slice(b"ERROR!!!");
            self.put_prec(p, &r);
        }
        let mut ids = Vec::new();
        for c in 1..=k {
            for (j, &p) in members[c].iter().enumerate() {
                let mut name = crate::messages::cstr(names.get(c - 1).map_or(&[][..], |v| &v[..])).to_vec();
                name.push(b' ');
                name.extend((j + 1).to_string().bytes());
                name.truncate(0x13);
                let mut r = self.prec(p);
                r[..0x14].fill(0);
                r[..name.len()].copy_from_slice(&name);
                self.put_prec(p, &r);
            }
            let pick = self.rnd_stream(1, 1, members[c].len() as i32) as usize;
            if let Some(&p) = members[c].get(pick.wrapping_sub(1)) {
                ids.push((p as i32, 499 + c as i32));
            }
        }
        (ids, k as i32)
    }

    /// `Generate_Stars()` (exe 0x59bf40, spec §3): `HanyBolygo` planets from the empty template record, placed,
    /// named into constellations and given their data, the live empires' home worlds colonised.
    fn generate_stars(&mut self, data: &StartData) {
        let n = self.state.planet_count();
        let blank = self.prec(0);
        for p in 1..=n {
            self.put_prec(p, &blank);
        }
        self.star_coords();
        let (ids, k) = self.star_names(&data.constellations);
        let mut st = self.state.story();
        for (p, id) in ids {
            st.b_add_id(p, id);
        }
        st.v_set(500, k);
        self.state.store_story(&st);
        self.star_data(data.image);
        self.m().set_u8(0xB99339, 1);
    }

    /// `Generate_StarCoords` (exe 0x59b120): random points in a disc sized by the planet count, at least 20 apart,
    /// then re-ordered along a nearest-neighbour chain from planet 1 (truncated to whole numbers).
    fn star_coords(&mut self) {
        let n = self.state.planet_count();
        let r = match n {
            0..=30 => 100,
            31..=50 => 160,
            51..=70 => 230,
            _ => 300,
        };
        let h = (r as u32 * 0x104 / 300) as i32;
        let mut pos: Vec<(f32, f32)> = vec![(0.0, 0.0); n + 1];
        for p in 1..=n {
            loop {
                let x = self.rnd_stream(1, -h, h);
                let y = self.rnd_stream(1, -h, h);
                let (fx, fy) = (x as f32, y as f32);
                let d = ((fx * fx + fy * fy) as f64).sqrt() as f32;
                if (r as f32) < d {
                    continue;
                }
                let near = (1..p).any(|q| {
                    let (dx, dy) = (pos[q].0 - fx, pos[q].1 - fy);
                    (((dx * dx + dy * dy) as f64).sqrt() as f32) < 20.0
                });
                if near {
                    continue;
                }
                pos[p] = (fx, fy);
                break;
            }
        }
        let xs: Vec<i32> = pos.iter().map(|q| q.0 as i32).collect();
        let ys: Vec<i32> = pos.iter().map(|q| q.1 as i32).collect();
        let mut chain = vec![0usize; n + 1];
        let mut used = vec![false; n + 1];
        if n > 0 {
            chain[1] = 1;
            used[1] = true;
        }
        for c in 2..=n {
            let last = chain[c - 1];
            let (mut best, mut pick) = (10_000_000, 0);
            for p in 1..=n {
                if used[p] {
                    continue;
                }
                let (dx, dy) = ((xs[p] - xs[last]) as f32, (ys[p] - ys[last]) as f32);
                let d = (dx * dx + dy * dy) as i32;
                if d < best {
                    best = d;
                    pick = p;
                }
            }
            chain[c] = pick;
            used[pick] = true;
        }
        for i in 1..=n {
            let mut rec = self.prec(i);
            rec[0x18..0x1c].copy_from_slice(&(xs[chain[i]] as f32).to_le_bytes());
            rec[0x1c..0x20].copy_from_slice(&(ys[chain[i]] as f32).to_le_bytes());
            self.put_prec(i, &rec);
        }
    }

    /// `Generate_StarData` (exe 0x59b520): every planet's kind, size and wealth; one start planet per empire 1..=8,
    /// each far from the earlier ones; the live empires' start planets of their race's favourite kind; the colony
    /// maps; the home worlds colonised with their first buildings.
    fn star_data(&mut self, image: &dyn Fn(u32, usize) -> Vec<u8>) {
        let n = self.state.planet_count();
        let wealth = |k: i32| i32::from_le_bytes(image(0x90B8E8 + 4 * k as u32, 4).try_into().unwrap());
        let prod = self.state.mem.read(PGYARTAS0, 0x40);
        for p in 1..=n {
            let kind = self.rnd_stream(1, 1, 8);
            let size = self.rnd_stream(1, 1, 6);
            let w = wealth(self.rnd_stream(1, 1, 5));
            let mut r = self.prec(p);
            r[0x22] = kind as u8;
            r[0x23] = size as u8;
            put_i32(&mut r, 0x24, w);
            r[0xb8..0xbc].fill(0);
            put_i32(&mut r, 0xbc, 0);
            put_i32(&mut r, 0xc0, -1);
            r[0x64] = 0;
            put_i32(&mut r, 0x70, 0);
            put_i32(&mut r, 0x74, 0);
            r[0x78..0xb8].copy_from_slice(&prod);
            r[0xc5..0xd7].fill(0);
            r[0x20] = 0;
            r[0xda..0xfe].fill(0);
            put_i32(&mut r, 0x114, 0);
            r[0x16a..0x174].fill(0);
            r[0x186] = 0;
            r[0x187] = 0;
            self.put_prec(p, &r);
        }
        // The start planets (`BirodalomBolygo`, `BolygoBirodalom`).
        const HOME: u32 = 0xF17394;
        const TAKEN: u32 = 0xF173B8;
        for p in 1..=n as u32 {
            self.m().set_i32(TAKEN + 4 * p, 0);
        }
        let first = self.rnd_stream(1, 1, n as i32);
        let pos: Vec<(f32, f32)> = (0..=n).map(|p| {
            let r = self.prec(p);
            (rd_f32(&r, 0x18), rd_f32(&r, 0x1c))
        }).collect();
        let mut home = [0usize; 9];
        home[1] = first.max(0) as usize;
        self.m().set_i32(HOME + 4, first);
        self.m().set_i32(TAKEN + 4 * first as u32, 1);
        let players = self.state.mem.i32(va::PROGRAM_PLAYERS).clamp(0, 8) as usize;
        // A start found for no one keeps the previous pick (the ELF reads an uninitialised local).
        let mut pick = home[1];
        for c in 2..=players {
            let mut best = 0;
            for p in 1..=n {
                if self.state.mem.i32(TAKEN + 4 * p as u32) != 0 {
                    continue;
                }
                let mut score = 10_000_000;
                for j in 1..c {
                    let q = home[j];
                    let (dx, dy) = (pos[p].0 - pos[q].0, pos[p].1 - pos[q].1);
                    let f = self.rnd_stream(1, 10, 0x23) as f32;
                    score = score.min(((dx * dx + dy * dy) * f) as i32);
                }
                if score > best {
                    best = score;
                    pick = p;
                }
            }
            home[c] = pick;
            self.m().set_i32(HOME + 4 * c as u32, pick as i32);
            self.m().set_i32(TAKEN + 4 * pick as u32, c as i32);
        }
        let alive = self.state.alive();
        let races = self.state.races();
        for c in 1..=players {
            if !alive[c] {
                continue;
            }
            let race = races[c] as u32;
            let liking = |k: u32| i32::from_le_bytes(image(0x90B9D8 + 4 * (k + 9 * race), 4).try_into().unwrap());
            let m = (1..=8).map(liking).fold(0, i32::max);
            let (mut kind, mut ties) = (0, 0);
            for k in 1..=8 {
                if liking(k) == m {
                    ties += 1;
                    if self.rnd_stream(1, 1, ties) <= 1 {
                        kind = k;
                    }
                }
            }
            let b = home[c];
            let size = self.rnd_stream(1, 1, 6);
            let mut r = self.prec(b);
            put_i32(&mut r, 0x24, wealth(3));
            r[0x22] = kind as u8;
            r[0x23] = size as u8;
            self.put_prec(b, &r);
        }
        // SetPlanetSurfaces: each planet's colony map, the next of its kind.
        let mut c = self.state.colony();
        c.map_cursor = [0; 8];
        let maps: Vec<u8> = (1..=n).map(|p| {
            let kind = self.prec(p)[0x22].wrapping_sub(1);
            c.next_colony_map(kind, &self.rules.placement)
        }).collect();
        self.state.store_colony(&c);
        for (i, map) in maps.into_iter().enumerate() {
            let mut r = self.prec(i + 1);
            r[0x23] = map;
            self.put_prec(i + 1, &r);
        }
        // InitGrWar: every battlefield a group size of 1 (the constructor's value already).
        for p in 0..n as u32 {
            self.m().set_i32(va::WARFARE + p * va::WARFARE_STRIDE + 0x18, 1);
        }
        // The home worlds: Generate_Planet(home, c, c, true, true, 0, 0).
        for c in 1..=players {
            if alive[c] {
                self.generate_planet(home[c], c, c as u8, true, true, &prod);
            }
        }
    }

    /// `Generate_Planet(p, owner, race, done, extra, 0, 0)` (exe 0x59af50): colonised, a colony hub, and with
    /// `extra` the first buildings; each placement draws a (zero) damage first.
    /// `Generate_Planet(p, owner, race, done, extra, 0, 0)` (exe 0x59af50) as the stories call it during a game, with
    /// the production template `pGyartas0` from the image in the state.
    pub(super) fn story_colonise(&mut self, p: usize, owner: usize, race: u8, done: bool, extra: bool) {
        let prod = match &self.rules.story.production_template {
            t if t.len() == 0x40 => t.clone(),
            _ => self.state.mem.read(PGYARTAS0, 0x40),
        };
        self.generate_planet(p, owner, race, done, extra, &prod);
    }

    fn generate_planet(&mut self, p: usize, owner: usize, race: u8, done: bool, extra: bool, prod: &[u8]) {
        self.generate_planet0(p, owner, race, prod, extra);
        let dmg = self.rnd_stream(1, 0, 0);
        self.place(p, 1, done, dmg);
        if extra {
            for kind in [2, 2, 4, 6, 0x13, 0x34, 0x34, 0xe, 10] {
                let dmg = self.rnd_stream(1, 0, 0);
                self.place(p, kind, done, dmg);
            }
        }
    }
}

/// `atoi`.
fn atoi(t: &str) -> i32 {
    let (sign, d) = match t.as_bytes().first() {
        Some(b'-') => (-1, &t[1..]),
        Some(b'+') => (1, &t[1..]),
        _ => (1, t),
    };
    let end = d.find(|c: char| !c.is_ascii_digit()).unwrap_or(d.len());
    sign * d[..end].parse::<i64>().unwrap_or(0) as i32
}

/// `atof` as an f32 (`GetParamF`).
fn atof(t: &str) -> f32 {
    let end = t.find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E'))).unwrap_or(t.len());
    t[..end].parse::<f64>().unwrap_or(0.0) as f32
}

/// A planet line of `LoadScenario` (spec §7): writes `r`; `false` for other keys.
fn planet_field(r: &mut [u8], key: &str, raw: &str, toks: &[&str]) -> bool {
    let i = |k: usize| toks.get(k).map_or(0, |s| atoi(s));
    let f = |k: usize| toks.get(k).map_or(0.0, |s| atof(s));
    let w32 = |r: &mut [u8], o: usize, v: i32| r[o..o + 4].copy_from_slice(&v.to_le_bytes());
    match key {
        "pnev" => {
            let b = raw.as_bytes();
            let n = b.len().min(0x14);
            r[..0x14].fill(0);
            r[..n].copy_from_slice(&b[..n]);
        }
        "px" => r[0x18..0x1c].copy_from_slice(&f(0).to_le_bytes()),
        "py" => r[0x1c..0x20].copy_from_slice(&f(0).to_le_bytes()),
        "ptulaj" => r[0x20] = i(0) as u8,
        "pfaj" => r[0x21] = i(0) as u8,
        "pfelszin" => r[0x22] = i(0) as u8,
        "pmap" => r[0x23] = i(0) as u8,
        "pgazdagsag" => w32(r, 0x24, i(0)),
        "plakossag" => w32(r, 0x28, i(0)),
        "plakossagutolsovaltozas" => w32(r, 0x2c, i(0)),
        "phangulat" => w32(r, 0x30, i(0)),
        "pado" => r[0x34] = i(0) as u8,
        "paktgdp" => w32(r, 0x38, i(0)),
        "poldgdp" => w32(r, 0x3c, i(0)),
        "pakttax" => w32(r, 0x40, i(0)),
        "poldtax" => w32(r, 0x44, i(0)),
        "paramtermeles" => w32(r, 0x48, i(0)),
        "paramigeny" => w32(r, 0x4c, i(0)),
        "pemberigeny" => w32(r, 0x50, i(0)),
        "ptechlevel" => r[0x54] = i(0) as u8,
        "peconomytype" => r[0x55] = i(0) as u8,
        "pautobuildosszegyultprioritas" => w32(r, 0x68, i(0)),
        "pautobuild" => r[0x6c] = i(0) as u8,
        "pprioritas" => r[0x6d] = i(0) as u8,
        "pradarrange" => r[0x6e] = i(0) as u8,
        "ptermeleskapacitas0" => w32(r, 0x70, i(0)),
        "ptermeleskapacitas1" => w32(r, 0x74, i(0)),
        "pvirusido" => r[0xb8..0xba].copy_from_slice(&(i(0) as i16).to_le_bytes()),
        "pvillogas" => r[0xba] = i(0) as u8,
        "pkellszolnifelfedezeskor" => r[0xbb] = (i(0) != 0) as u8,
        "pbarat" => r[0xbc] = (i(0) != 0) as u8,
        "pmuholdak" => (1..EMPIRES).for_each(|k| r[0xc4 + k] = i(k - 1) as u8),
        "pfelderites" => (1..EMPIRES).for_each(|k| r[0xd8 + 2 * k..0xda + 2 * k].copy_from_slice(&(i(k - 1) as i16).to_le_bytes())),
        "pspbase" => (1..=10).for_each(|k| r[0xfe + 2 * k..0x100 + 2 * k].copy_from_slice(&(i(k - 1) as i16).to_le_bytes())),
        "pspbasehany" => w32(r, 0x114, i(0)),
        "pproblemak" => (1..=19).for_each(|k| w32(r, 0x118 + 4 * k, i(k - 1))),
        "pspecials" => (1..=5).for_each(|k| r[0x168 + 2 * k..0x16a + 2 * k].copy_from_slice(&(i(k - 1) as i16).to_le_bytes())),
        "pmoralcsokkentobarkacs" => w32(r, 0x174, i(0)),
        "pmoralcsokkentobarkacssebesseg" => w32(r, 0x178, i(0)),
        "szin" => w32(r, 0x17c, i(0)),
        "bolygomeret" => r[0x180] = i(0) as u8,
        "csillagmeret" => r[0x181] = i(0) as u8,
        "spinspeed" => r[0x182] = i(0) as u8,
        "yaw" => r[0x183] = i(0) as u8,
        "dist" => r[0x184] = i(0) as u8,
        "speed" => r[0x185] = i(0) as u8,
        _ => return false,
    }
    true
}

/// What a fleet's `fShip` / `fTank` line adds (`HajoNew` n times, `TankNew(f, d, n)`).
enum Unit {
    Ships(i16, i32),
    Vehicles(i16, i32),
}

/// The newest record of each kind while a scenario loads (mode 0).
#[derive(Default)]
struct Cursor {
    /// The spy record's address (`SPY e`).
    spy: Option<u32>,
    /// The tech `TALALMANY` lines count up to (0 before the first).
    tech: usize,
    /// `TERV e`: the empire and design.
    design: Option<(usize, usize)>,
    fleet: Option<usize>,
    /// `VISZONY a b`: the relation record's address.
    relation: Option<u32>,
    /// `tervredesign` seen.
    redesign: bool,
    /// The design records past the 40 the preamble keeps, per empire: a `TERV` line reuses them as they are.
    spare: Vec<Vec<[u8; DESIGN_SIZE]>>,
    units: Vec<(usize, Unit)>,
}

/// A name field, `my_strncpy(rec + o, rest of the line, n)` (exe 0x553c90): `strncpy`, and a name of `n` bytes or
/// more cut to `n − 1`.
fn name_field(r: &mut [u8], o: usize, n: usize, raw: &str) {
    let b = raw.as_bytes();
    let k = b.len().min(n);
    r[o..o + n].fill(0);
    r[o..o + k].copy_from_slice(&b[..k]);
    if b.len() >= n {
        r[o + n - 1] = 0;
    }
}

fn w16(r: &mut [u8], o: usize, v: i32) {
    r[o..o + 2].copy_from_slice(&(v as i16).to_le_bytes());
}

/// A spy line (`s…`, spec §7) on the 0x4c-byte record.
fn spy_field(r: &mut [u8], key: &str, raw: &str, toks: &[&str]) -> bool {
    let i = |k: usize| toks.get(k).map_or(0, |s| atoi(s));
    match key {
        "snev" => name_field(r, 0, 0xf, raw),
        "sstatusz" => w16(r, 0x10, i(0)),
        "sfaj" => r[0x12] = i(0) as u8,
        "skep" => w16(r, 0x14, i(0)),
        "sujonnanjelentkezett" => w16(r, 0x16, i(0)),
        "skinekkettoskem" => r[0x18] = i(0) as u8,
        "skuldetes" => w16(r, 0x1a, i(0)),
        "scelfaj" => r[0x1c] = i(0) as u8,
        "scel" => w16(r, 0x1e, i(0)),
        "scel2" => w16(r, 0x20, i(0)),
        "sido" => put_i32(r, 0x24, i(0)),
        "sidohatra" => put_i32(r, 0x28, i(0)),
        "sidoelkapasellenorzes" => put_i32(r, 0x2c, i(0)),
        "studas" => (1..=4).for_each(|k| r[0x30 + k] = i(k - 1) as u8),
        "sszetoszthato" => put_i32(r, 0x38, i(0)),
        "sxp" => put_i32(r, 0x3c, i(0)),
        "srang" => r[0x40] = i(0) as u8,
        "sismerik" => (1..=8).for_each(|k| r[0x41 + k] = (i(k - 1) != 0) as u8),
        _ => return false,
    }
    true
}

/// A tech line (`r…`, spec §7) on the tech record; the cost and prerequisite lines are read and dropped.
fn tech_field(r: &mut [u8], key: &str, toks: &[&str]) -> bool {
    let i = |k: usize| toks.get(k).map_or(0, |s| atoi(s));
    match key {
        "rprojectallapot" => (1..EMPIRES).for_each(|e| r[0x45 + e] = i(e - 1) as u8),
        "rkutatasaktido" => (1..EMPIRES).for_each(|e| put_i32(r, 0x58 + 4 * e, i(e - 1))),
        "rneedednullazva" => (1..EMPIRES).for_each(|e| r[0xa4 + e] = (i(e - 1) != 0) as u8),
        "rkemkedeshezismeri" => {
            let row = i(0).clamp(0, 8) as usize;
            (1..=8).for_each(|k| r[0xc0 + 9 * row + k] = i(k) as u8);
        }
        "rneeded" | "rkutatasszint" | "rkutatasmoney" | "rkutatasido" => {}
        _ => return false,
    }
    true
}

/// A design line (`t…`, spec §7) on the 0x84-byte record.
fn design_field(r: &mut [u8; DESIGN_SIZE], key: &str, raw: &str, toks: &[&str]) -> bool {
    let i = |k: usize| toks.get(k).map_or(0, |s| atoi(s));
    let b = |k: usize| (i(k) != 0) as u8;
    match key {
        "tnev" => name_field(r, 0, 0x15, raw),
        "ttipus" => w16(r, 0x16, i(0)),
        "teredetitulaj" => w16(r, 0x40, i(0)),
        "tlathato" => r[0x18] = b(0),
        "tnemlehetmodositani" => r[0x19] = b(0),
        "teltuntetnihanincs" =>r[0x1a] = b(0),
        "tupgrademire" => put_i32(r, 0x1c, i(0)),
        "tgyartasido" => put_i32(r, 0x20, i(0)),
        "tar" => put_i32(r, 0x24, i(0)),
        "tmaxtank" => r[0x28] = i(0) as u8,
        "tero" => put_i32(r, 0x2c, i(0)),
        "thp" => put_i32(r, 0x30, i(0)),
        "tkolonizalo" => r[0x34] = b(0),
        "tterraformalo" => r[0x35] = b(0),
        "tvadasz" => r[0x36] = b(0),
        "turbazis" => r[0x37] = b(0),
        "turbazismozgathato" => r[0x38] = b(0),
        "tsatellite" => r[0x39] = b(0),
        "tsebesseg" => r[0x3c..0x40].copy_from_slice(&toks.first().map_or(0.0, |s| atof(s)).to_le_bytes()),
        "talap" => put_i32(r, 0x44, i(0)),
        "talaptalalmany" => put_i32(r, 0x48, i(0)),
        "thanypod" => put_i32(r, 0x4c, i(0)),
        "tpodtalalmany" => (1..=12).for_each(|k| r[0x50 + k] = i(k - 1) as u8),
        "tpoddarab" => (1..=12).for_each(|k| r[0x5d + k] = i(k - 1) as u8),
        "tradar" => r[0x6a] = i(0) as u8,
        "tfleetcloaking" => r[0x6b] = b(0),
        "tkamudarab" => put_i32(r, 0x6c, i(0)),
        "tkamukills" => r[0x70] = i(0) as u8,
        "tkamubeadasiev" => put_i32(r, 0x74, i(0)),
        "tkamubeadasihonap" => put_i32(r, 0x78, i(0)),
        "tkamubeadasinap" => put_i32(r, 0x7c, i(0)),
        _ => return false,
    }
    true
}

/// A fleet line (`f…`, spec §7) on the 0x80-byte record: `None` for other keys, `Some(unit)` when it adds ships or
/// vehicles.
fn fleet_field(r: &mut [u8; 0x80], key: &str, raw: &str, toks: &[&str]) -> Option<Option<Unit>> {
    let i = |k: usize| toks.get(k).map_or(0, |s| atoi(s));
    let f = |k: usize| toks.get(k).map_or(0.0, |s| atof(s));
    let flag = |r: &mut [u8; 0x80], bit: u32| {
        let v = u16::from_le_bytes([r[0x52], r[0x53]]);
        let v = if i(0) != 0 { v | 1 << bit } else { v & !(1 << bit) };
        r[0x52..0x54].copy_from_slice(&v.to_le_bytes());
    };
    let count = if toks.len() > 1 { i(1) } else { 1 };
    match key {
        "fazon" | "fazonosito" => w16(r, 0x16, i(0)),
        "fnev" => name_field(r, 0, 0x15, raw),
        "fhidden" => r[0x18] = (r[0x18] & !1) | (i(0) != 0) as u8,
        "feltunes" => r[0x18] = (r[0x18] & !2) | (((i(0) != 0) as u8) << 1),
        "flathato" => (1..EMPIRES).for_each(|k| r[0x19 + k] = i(k - 1) as u8),
        "fhovavonulvissza" => w16(r, 0x2c, i(0)),
        "fsebesseg" => r[0x38..0x3c].copy_from_slice(&f(0).to_le_bytes()),
        "fabszolutsebesseg" => r[0x3c..0x40].copy_from_slice(&f(0).to_le_bytes()),
        "fmozgastipus" => w16(r, 0x40, i(0)),
        "fceltipus" => w16(r, 0x42, i(0)),
        "fcelx" => r[0x44..0x48].copy_from_slice(&f(0).to_le_bytes()),
        "fcely" => r[0x48..0x4c].copy_from_slice(&f(0).to_le_bytes()),
        "fspecvirus" => flag(r, 2),
        "fspeckolonizalo" => flag(r, 3),
        "fspecterraformalo" => flag(r, 4),
        "fspecatiranyitott" => flag(r, 5),
        "fkellszolnifelfedezeskor" => flag(r, 6),
        "fvillogas" => flag(r, 7),
        "fmindiglathato" => flag(r, 8),
        "fbarat" => flag(r, 9),
        "fwormhole" => flag(r, 10),
        "fnevelatszik" => flag(r, 11),
        "fcloaking" => flag(r, 12),
        "fspecwormholedeploy" => flag(r, 13),
        "fmeteor" => flag(r, 14),
        "fradar" => r[0x7c..0x80].copy_from_slice(&f(0).to_le_bytes()),
        "ftank" => return Some(Some(Unit::Vehicles(i(0) as i16, count))),
        "fship" => {
            let d = if i(0) == 0x2b && r[0x15] == 9 { 8 } else { i(0) };
            return Some(Some(Unit::Ships(d as i16, count)));
        }
        _ => return None,
    }
    Some(None)
}

/// A relation line (`v…`, spec §7) on the 0x26-byte record.
fn relation_field(r: &mut [u8], key: &str, toks: &[&str]) -> bool {
    let v = toks.first().map_or(0, |s| atoi(s));
    let o = match key {
        "vlebukottkemek" => 0,
        "vregihaboruk" => 2,
        "vstoryviszony" => 4,
        "vdiplomviszony" => 6,
        "vdiplomtulsokszorzavartal" => 8,
        "vmegbizhatatlan" => 0xa,
        "veroviszony" => 0x1c,
        "vszovetsegirendszer" => 0x1e,
        "vtavolsag" => 0x20,
        "vviszony" => 0x22,
        "vtradeagreement" => {
            r[0x24] = (v != 0) as u8;
            return true;
        }
        "vszovetsegiallapot" => {
            r[0x25] = v as u8;
            return true;
        }
        _ => return false,
    };
    w16(r, o, v);
    true
}

/// A building line of `LoadScenario` (spec §7).
fn building_field(r: &mut [u8; BUILDING_SIZE], key: &str, toks: &[&str]) {
    let i = |k: usize| toks.get(k).map_or(0, |s| atoi(s));
    let f = |k: usize| toks.get(k).map_or(0.0, |s| atof(s));
    let bit = |r: &mut [u8; BUILDING_SIZE], b: u8| {
        if i(0) != 0 {
            r[0x19] |= b;
        } else {
            r[0x19] &= !b;
        }
    };
    match key {
        "etipus" => r[0] = i(0) as u8,
        "etulaj" => r[1] = i(0) as u8,
        "edamage" => r[4..8].copy_from_slice(&i(0).to_le_bytes()),
        "eepido" => r[8..10].copy_from_slice(&(i(0) as i16).to_le_bytes()),
        "ex" => r[0xc..0x10].copy_from_slice(&f(0).to_le_bytes()),
        "ey" => r[0x10..0x14].copy_from_slice(&f(0).to_le_bytes()),
        "ez" => r[0x14..0x18].copy_from_slice(&f(0).to_le_bytes()),
        "emodosito" => r[0x18] = i(0) as u8,
        "eaktiv" => bit(r, 1),
        "erepair" => bit(r, 2),
        "etoodamaged" => bit(r, 4),
        "enincsaram" => bit(r, 8),
        "emaffia" => bit(r, 0x10),
        "emennyienergy" => r[0x1a..0x1c].copy_from_slice(&(i(0) as i16).to_le_bytes()),
        "emennyiember" => r[0x1c..0x1e].copy_from_slice(&(i(0) as i16).to_le_bytes()),
        "emennyiremukodik" => r[0x1e..0x20].copy_from_slice(&(i(0) as i16).to_le_bytes()),
        _ => {}
    }
}

/// The story engine's view during `InitStory` inside the new game.
struct InitWorld<'a, 'g, 'r> {
    game: &'a mut Game<'r>,
    cfg: &'a NewCampaign,
    data: &'a StartData<'g>,
    tables: CampaignTables,
    events: Vec<StoryEvent>,
    /// The streams while the story engine holds them (handed to the game around its own calls).
    rng: crate::rng::Streams,
}

impl InitWorld<'_, '_, '_> {
    fn mem(&self) -> &Memory {
        &self.game.state.mem
    }
    /// Run `f` on the game with the streams in its state.
    fn with_game<R>(&mut self, f: impl FnOnce(&mut Game) -> R) -> R {
        self.game.state.store_rng(&self.rng);
        let r = f(self.game);
        self.rng = self.game.state.rng();
        r
    }
}

impl StoryWorld for InitWorld<'_, '_, '_> {
    fn ido(&self) -> i32 {
        self.mem().i32(va::IDO)
    }
    fn speed_mult(&self) -> i32 {
        self.mem().i32(va::IDO_SPEED)
    }
    fn human(&self) -> i32 {
        self.mem().i32(va::EN)
    }
    fn difficulty(&self) -> i32 {
        self.mem().i32(va::NEHEZSEG)
    }
    fn multiplayer(&self) -> bool {
        false
    }
    fn rng(&mut self) -> &mut crate::rng::Streams {
        &mut self.rng
    }
    fn planet_count(&self) -> i32 {
        self.game.state.planet_count() as i32
    }
    fn planet_owner(&self, p: i32) -> i32 {
        self.game.prec(p.max(0) as usize)[0x20] as i32
    }
    fn planet_value(&self, p: i32) -> i32 {
        i32::from_le_bytes(self.game.prec(p.max(0) as usize)[0x28..0x2c].try_into().unwrap())
    }
    fn planet_visibility(&self, p: i32) -> i32 {
        let en = self.mem().i32(va::EN).max(0) as usize;
        rd_i16(&self.game.prec(p.max(0) as usize), 0xd8 + 2 * en) as i32
    }
    fn fleet_count(&self) -> i32 {
        self.game.state.fleets.len() as i32 - 1
    }
    fn fleet_owner(&self, f: i32) -> i32 {
        self.game.state.fleets.get(f.max(0) as usize).map_or(0xff, |r| r[0x15] as i32)
    }
    fn fleet_id(&self, f: i32) -> i32 {
        self.game.state.fleets.get(f.max(0) as usize).map_or(0, |r| rd_i16(r, 0x16) as i32)
    }
    fn empire_alive(&self, e: i32) -> bool {
        self.game.state.alive().get(e.max(0) as usize).copied().unwrap_or(false)
    }
    fn planets_of(&self, e: i32) -> i32 {
        self.mem().i32(va::EMPIRE_PLANETS + 4 * e.max(0) as u32)
    }
    fn money(&self, e: i32) -> f64 {
        self.mem().f64(va::MONEY + 8 * e.clamp(0, 18) as u32)
    }
    fn tech_status(&self, tech: i32, e: i32) -> i32 {
        self.mem().u8(va::TECHS + (tech.max(0) as usize * TECH_SIZE) as u32 + 0x45 + e.max(0) as u32) as i32
    }
    fn relation_level(&self, a: i32, b: i32) -> i32 {
        self.mem().u8(va::RELATIONS + ((a.max(0) as usize * EMPIRES + b.max(0) as usize) * 0x26) as u32 + 0x25) as i32
    }
    fn event(&mut self, ev: StoryEvent) {
        self.events.push(ev);
    }
    fn campaign_type(&self) -> i32 {
        self.mem().i32(va::CAMPAIGN_TYPE)
    }
    fn campaign_tables(&self) -> &CampaignTables {
        &self.tables
    }
    fn race(&self, e: i32) -> i32 {
        self.game.state.races().get(e.max(0) as usize).copied().unwrap_or(0) as i32
    }
    fn config_int(&self, key: &str) -> i32 {
        match key {
            "storyline" => self.cfg.storyline,
            "storymap" => self.cfg.storymap,
            _ => 0,
        }
    }
    fn registry_storyline(&self, _campaign: i32) -> Option<i32> {
        self.cfg.last_storyline
    }
    fn load_scenario(&mut self, n: i32) -> Vec<ScenarioStory> {
        match (self.data.scenario)(n) {
            Some(text) => {
                let techs0 = (self.data.image)(TALALMANYOK0, TECHS * TECH_SIZE);
                self.with_game(|g| g.load_scenario(&text, 1, &techs0))
            }
            None => Vec::new(),
        }
    }
    fn generate_star_names(&mut self) -> (Vec<(i32, i32)>, i32) {
        // The mirror and the human's first buildings come before the names (InitStory's order).
        let events = std::mem::take(&mut self.events);
        let names = self.data.constellations.clone();
        let (rest, out) = self.with_game(|g| {
            let mut rest = Vec::new();
            for ev in events {
                match ev {
                    StoryEvent::Mirror { x, y } => g.mirror(x, y),
                    StoryEvent::PlaceBuilding { planet, kind } => g.place(planet as usize, kind as u8, true, 0),
                    other => rest.push(other),
                }
            }
            (rest, g.star_names(&names))
        });
        self.events = rest;
        out
    }
}
