//! One sim tick of the whole strategic game, the systems in the exe's order (`re/rules/time-and-rng.md` §1), on a
//! [`SaveState`]:
//!
//! ```text
//! FRISSIT_Datum                       Game::advance_clock
//! if IdoNapValtozott:                 Game::run_tick from here
//!     Nap % 6 == 1: FRISSIT_Planet    colony_side::ColonyWorld::economy_step
//!     FRISSIT_Viszony                 diplomacy::Diplomacy::daily
//!     FRISSIT_MI                      ai::AiState::daily
//!     FRISSIT_Spy                     spy::Spies::daily
//!     MI_DIPLOMACY                    diplomacy::Diplomacy::ai_messages
//! FRISSIT_Research                    research::tick
//! FRISSIT_Story                       story::Story::tick (the scenario stories)
//! FRISSIT_Diplomacy                   diplomacy::Diplomacy::tick
//! FRISSIT_Starmap                     fleet::Starmap::tick (colonisation arrivals), then production::Shipyard::tick
//! IdoMultiPlayerBattle += 1
//! ```
//!
//! Not ported, so skipped: `FRISSIT_Trade`, `FRISSIT_Info`, `FRISSIT_Messages`, `FRISSIT_Design`, `FRISSIT_Epuletek`,
//! the victory check, the ground and space battles' real-time ticks. Their effects on the other systems (a fleet
//! stopped by a story, a war the AI declares) are reported as [`GameEvent`]s, and the few the replay needs are
//! carried out here. Each system decodes its state from the [`SaveState`] and stores it back, so systems see each
//! other's writes as in the exe.

use crate::ai::{self, AiContext, AiHooks, AiRules};
use crate::colony::{EconomyContext, Planet, StoryHooks};
use crate::colony_side::{ColonyArrivals, SideEvent, SideRules, EMPIRES};
use crate::diplomacy::{self, DiploEvent, DiploPlanet, Diplomacy, DiplomacyContext, DiplomacyRules};
use crate::fleet::{order, StarmapContext, StarmapEvent};
use crate::placement::PlacementRules;
use crate::production::{ProductionContext, ProductionEvent, Queue, Shipyard, YardFleet, YardPlanet};
use crate::radar::RadarRules;
use crate::research::{self, ResearchContext, ResearchEvent, ResearchRules};
use crate::rng::Streams;
use crate::save::{fleet_totals_put, put_fleet, va, SaveState, PLANET_SIZE};
use crate::spacewar::{Ship, Vehicle};
use crate::spy::{self, SpyContext, SpyEvent, SpyRules, StoryCheck};
use crate::story::{ScenarioStories, StoryEvent, StoryWorld};
use crate::time::GameTime;

/// The rule tables every system reads (from the user's exe and install, ADR 0004).
#[derive(Debug, Clone, Default)]
pub struct GameRules {
    pub side: SideRules,
    pub placement: PlacementRules,
    pub research: ResearchRules,
    pub diplomacy: DiplomacyRules,
    pub spy: SpyRules,
    pub ai: AiRules,
    pub radar: RadarRules,
    /// `NehezsegGyartasSzorzo` / `NehezsegPenzSzorzo` by difficulty.
    pub difficulty_production: Vec<f32>,
    pub difficulty_money: Vec<f32>,
}

/// The MSVC CRT `rand()` (`holdrand · 214013 + 2531011`, bits 16..30): the unseeded libc draws of the original.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Libc(pub u32);

impl Libc {
    pub fn rand(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as i32
    }
}

/// What a tick asked of systems that aren't ported, or reported to the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum GameEvent {
    Side(SideEvent),
    Diplomacy(DiploEvent),
    Ai(ai::AiEvent),
    Spy(SpyEvent),
    Research(ResearchEvent),
    Story(StoryEvent),
    Starmap(StarmapEvent),
    Production(ProductionEvent),
    /// The AI declared war (`ElkuldHaduzenet`); carried out by diplomacy.
    WarDeclared { a: u8, b: u8 },
}

/// The game: its state and the rules.
pub struct Game<'r> {
    pub state: SaveState,
    pub rules: &'r GameRules,
    pub libc: Libc,
    /// `AlapFlottaNev` (the localised default fleet name; `None` = the English "Fleet").
    pub fleet_name: Option<Vec<u8>>,
    /// `OldEtaHonap` of the unported `FRISSIT_Trade`: on a new month it draws one libc `rand()` (the merchants'
    /// arrival days), which shifts the later draws of the tick. `None` = the current month (as after a load).
    pub trade_month: Option<i32>,
    /// The bytes of records appended to grown arrays before the code writes them (uninitialised heap in the
    /// original): `None` = zero.
    pub uninit: Option<Box<dyn FnMut(Fresh) -> Vec<u8> + 'r>>,
    pub events: Vec<GameEvent>,
}

/// An appended record whose untouched bytes [`Game::uninit`] supplies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fresh {
    Fleet(usize),
    Design { empire: usize, design: usize },
}

/// The tick's global inputs read from the state.
struct Globals {
    year: i32,
    month: i32,
    day: i32,
    tick: i32,
    ido2: i32,
    speed: i32,
    multiplayer: bool,
    en: usize,
    human: [bool; EMPIRES],
    race: [u8; EMPIRES],
    alive: [bool; EMPIRES],
    difficulty: i32,
    pp: usize,
    pp2: usize,
    all_techs: bool,
    speed_locked: bool,
    program_speed: i32,
    tutorial: bool,
}

fn adat_i32(adat: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(adat[off..off + 4].try_into().unwrap())
}

impl<'r> Game<'r> {
    pub fn new(state: SaveState, rules: &'r GameRules) -> Self {
        Self { state, rules, libc: Libc::default(), fleet_name: None, trade_month: None, uninit: None, events: Vec::new() }
    }

    fn globals(&self) -> Globals {
        let m = &self.state.mem;
        Globals {
            year: m.i32(va::EV),
            month: m.i32(va::HONAP),
            day: m.i32(va::NAP),
            tick: m.i32(va::IDO),
            ido2: m.i32(va::IDO2),
            speed: m.i32(va::IDO_SPEED),
            multiplayer: m.u8(va::MULTIPLAYER) != 0,
            en: m.i32(va::EN).clamp(0, EMPIRES as i32 - 1) as usize,
            human: self.state.human_controlled(),
            race: self.state.races(),
            alive: self.state.alive(),
            difficulty: m.i32(va::NEHEZSEG),
            pp: m.i32(va::PROGRAM_PLAYERS).max(0) as usize,
            pp2: m.i32(va::PROGRAM_PLAYERS2).max(0) as usize,
            all_techs: m.i32(va::ALL_TECHS) != 0,
            speed_locked: m.i32(va::SPEED_LOCKED) != 0,
            program_speed: m.i32(va::PROGRAM_SEBESSEG),
            tutorial: m.u8(va::TUTORIAL_ACTIVE) != 0 || m.u8(va::TUTORIAL_PAUSED) != 0,
        }
    }

    fn adat(&self) -> Vec<u8> {
        self.state.mem.read(va::STORY_ADAT, crate::story::ADAT)
    }

    /// `FRISSIT_Datum`: `Ido += IdoSpeedSzorzo`, `Ido2 += 1`, the calendar.
    pub fn advance_clock(&mut self) {
        let m = &mut self.state.mem;
        let ido = m.i32(va::IDO).wrapping_add(m.i32(va::IDO_SPEED));
        let t = GameTime(ido);
        m.set_i32(va::IDO, ido);
        m.set_i32(va::IDO2, m.i32(va::IDO2).wrapping_add(1));
        m.set_i32(va::EV, t.year());
        m.set_i32(va::HONAP, t.month());
        m.set_i32(va::NAP, t.day());
    }

    /// The rest of the tick after the clock step (see the module docs).
    pub fn run_tick(&mut self) {
        let m = &mut self.state.mem;
        let (nap, old) = (m.i32(va::NAP), m.i32(va::OLD_NAP));
        if nap != old {
            m.set_i32(va::OLD_NAP, nap);
            self.daily();
        }
        self.trade();
        self.research();
        self.story();
        self.diplomacy_tick();
        self.starmap();
        let m = &mut self.state.mem;
        m.set_i32(0xBAE054, m.i32(0xBAE054).wrapping_add(1));
    }

    /// What the unported `FRISSIT_Trade` does to the ported systems' inputs: its libc draw on a new month.
    fn trade(&mut self) {
        let month = self.state.mem.i32(va::HONAP);
        if *self.trade_month.get_or_insert(month) != month {
            self.trade_month = Some(month);
            self.libc.rand();
        }
    }

    fn daily(&mut self) {
        if self.state.mem.i32(va::NAP) % 6 == 1 {
            self.economy();
        }
        self.diplomacy_daily(false);
        if self.state.mem.i32(va::AI_OFF) == 0 {
            self.ai();
        }
        self.spies();
        if self.state.mem.i32(va::AI_OFF) == 0 {
            self.diplomacy_daily(true);
        }
    }

    // -------------------------------------------------------------------------------------------- contexts

    fn economy_context(&self, g: &Globals) -> EconomyContext {
        let a = self.adat();
        let k = self.state.kidnap();
        EconomyContext {
            year: g.year,
            month: g.month,
            day: g.day,
            multiplayer: g.multiplayer,
            human_empire: g.en as u8,
            human_controlled: g.human,
            empire_race: g.race,
            players: g.pp2 as i32,
            kidnap_state: std::array::from_fn(|e| k[e]),
            global_morale: self.state.mem.i32(va::GLOBAL_MORALE),
            global_morale_speed: self.state.mem.i32(va::GLOBAL_MORALE_SPEED),
            story: StoryHooks {
                food_halved_planet: adat_i32(&a, 0x20),
                lab_bonus_empire: adat_i32(&a, 0x24),
                food_lost: adat_i32(&a, 0x2c),
                hospital_watch: adat_i32(&a, 0x30),
                lab_start: std::array::from_fn(|i| adat_i32(&a, 0x9c + 4 * i)),
            },
            story_values: [0x5fb, 0x60f].iter().map(|&c| (c, self.story_var(c) as u8)).collect(),
        }
    }

    /// `st_vGet(id)` without the side effects (unset = 0).
    fn story_var(&self, id: i32) -> i32 {
        self.state.story().vars.get(id).unwrap_or(0)
    }

    fn store_economy_context(&mut self, ctx: &EconomyContext) {
        let m = &mut self.state.mem;
        m.set_i32(va::GLOBAL_MORALE, ctx.global_morale);
        m.set_i32(va::GLOBAL_MORALE_SPEED, ctx.global_morale_speed);
        m.set_i32(va::STORY_ADAT + 0x2c, ctx.story.food_lost);
    }

    fn research_context(&self, g: &Globals) -> ResearchContext {
        ResearchContext {
            speed_mult: g.speed,
            day: g.day,
            difficulty: g.difficulty.max(0) as usize,
            multiplayer: g.multiplayer,
            human_empire: g.en as u8,
            human_controlled: g.human,
            race: g.race,
            story_bonus: adat_i32(&self.adat(), 0x6c),
            cheat: self.state.mem.i32(va::RESEARCH_CHEAT) != 0,
            all_techs: g.all_techs,
            players: g.pp2 as i32,
            speed_locked: g.speed_locked,
            program_speed: g.program_speed,
        }
    }

    fn starmap_context(&self, g: &Globals) -> StarmapContext {
        let m = &self.state.mem;
        let a = self.adat();
        let rel = m.read(va::RELATIONS, EMPIRES * EMPIRES * 0x26);
        let tech = self.state.tech();
        StarmapContext {
            tick: g.ido2,
            speed_mult: g.speed,
            multiplayer: g.multiplayer,
            speed_locked: g.speed_locked,
            program_speed: g.program_speed,
            human_empire: g.en as u8,
            human_controlled: g.human,
            alive: g.alive,
            empire_race: g.race,
            debug: m.u8(va::DEBUGX) != 0,
            all_fleets_visible: m.i32(va::STORY_ALL_FLEETS_VISIBLE) != 0,
            debug_speed: m.i32(va::DEBUG_SPEED) != 0,
            relations: (0..EMPIRES).map(|x| std::array::from_fn(|y| (rel[(x * EMPIRES + y) * 0x26 + 0x24], rel[(x * EMPIRES + y) * 0x26 + 0x25]))).collect(),
            default_fleet_name: self.fleet_name(),
            program_players: g.pp,
            program_players2: g.pp2,
            radar_tech: std::array::from_fn(|e| (1..=4u8).rev().find(|&c| tech.status(0x20 + c as usize, e) == 5).unwrap_or(0)),
            story_tracked_fleet: adat_i32(&a, 0x28),
            story_radar_bonus: adat_i32(&a, 0x38) != 0,
            story_no_supply: adat_i32(&a, 0x48) != 0,
            story_supply_planet: adat_i32(&a, 0x50),
            tutorial_sighting: g.tutorial && m.i32(va::TUTORIAL_EVENT) == 1,
            radar_rules: self.rules.radar.clone(),
            in_battle: Vec::new(),
        }
    }

    /// `AlapFlottaNev` (localised; "Fleet" in English).
    pub fn fleet_name(&self) -> Vec<u8> {
        self.fleet_name.clone().unwrap_or_else(|| b"Fleet".to_vec())
    }

    fn diplomacy_context(&self, g: &Globals) -> DiplomacyContext {
        let m = &self.state.mem;
        let a = self.adat();
        DiplomacyContext {
            program_players: g.pp,
            program_players2: g.pp2,
            alive: g.alive,
            ai: m.flags(va::AI_EMPIRE),
            may_attack: m.flags(va::MAY_ATTACK),
            human: g.human,
            race: g.race,
            human_empire: g.en,
            multiplayer: g.multiplayer,
            debug: m.u8(va::DEBUGX) != 0,
            enabled: m.u8(va::DIPLOMACY_ENABLED) != 0,
            ai_off: m.i32(va::AI_OFF) != 0,
            story_quiet: adat_i32(&a, 0xb0) != 0,
            spare_human: std::array::from_fn(|e| adat_i32(&a, 0xd0 + 4 * e) != 0),
            difficulty: g.difficulty,
            tutorial: g.tutorial,
            forced_answer: m.i32(va::FORCED_ANSWER),
            main_fleet: m.i32s(va::AI_MAIN_FLEET),
            month: g.month,
            day: g.day,
            kidnap: self.state.kidnap(),
            all_techs: g.all_techs,
        }
    }

    fn diplo_planets(&self) -> Vec<DiploPlanet> {
        let m = &self.state.mem;
        (0..=self.state.planet_count())
            .map(|p| {
                let r = m.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE);
                let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
                DiploPlanet {
                    owner: r[0x20],
                    x: f32::from_bits(i32_at(0x18) as u32),
                    y: f32::from_bits(i32_at(0x1c) as u32),
                    population: i32_at(0x28),
                    defence: i32_at(0x5c),
                    ship_capacity: i32_at(0x70),
                    vehicle_capacity: i32_at(0x74),
                    fortification: m.i32(va::FORTRESS + 4 * p as u32),
                    visible: std::array::from_fn(|e| i16::from_le_bytes([r[0xd8 + 2 * e], r[0xd9 + 2 * e]])),
                }
            })
            .collect()
    }

    fn store_diplo_planets(&mut self, pl: &[DiploPlanet]) {
        for (p, d) in pl.iter().enumerate() {
            let at = va::PLANETS + (p * PLANET_SIZE) as u32;
            self.state.mem.set_u8(at + 0x20, d.owner);
            for e in 0..EMPIRES {
                self.state.mem.write(at + 0xd8 + 2 * e as u32, &d.visible[e].to_le_bytes());
            }
        }
    }

    fn spy_context(&self, g: &Globals) -> SpyContext {
        let m = &self.state.mem;
        SpyContext {
            alive: g.alive,
            human: g.human,
            race: g.race,
            human_empire: g.en,
            program_players: g.pp as i32,
            multiplayer: g.multiplayer,
            difficulty: g.difficulty,
            debug: m.u8(va::DEBUGX) != 0,
            tutorial: g.tutorial,
            no_recruits: m.u8(va::TUTORIAL_NO_RECRUITS) != 0,
            forced: m.i32(va::SPY_FORCED),
            day: g.day,
        }
    }

    // -------------------------------------------------------------------------------------------- systems

    fn economy(&mut self) {
        let g = self.globals();
        let mut ctx = self.economy_context(&g);
        let mut w = self.state.colony();
        let mut rng = self.state.rng();
        let placement = &self.rules.placement;
        let mut place = |pl: &Planet, _: usize, k: u8| placement.place(pl, k, &[]);
        let (_, events) = w.economy_step(&self.rules.side, &mut ctx, &mut rng, &mut place);
        self.state.store_colony(&w);
        self.state.store_rng(&rng);
        self.store_economy_context(&ctx);
        for e in &events {
            if let SideEvent::RecallAttackers { planet } = *e {
                self.recall_attackers(planet);
            }
        }
        self.events.extend(events.into_iter().map(GameEvent::Side));
    }

    /// `FlottaMozgat(f, 0, 0, 0, 0)` for every fleet attacking planet `p` (`order 2, kind 1`).
    fn recall_attackers(&mut self, p: usize) {
        let g = self.globals();
        let ctx = self.starmap_context(&g);
        let mut map = self.state.starmap();
        let mut ev = Vec::new();
        for f in 1..map.fleets.len() {
            let fl = &map.fleets[f];
            if fl.alive() && fl.order == order::ATTACK && fl.kind == order::PLANET && fl.tx == p as f32 {
                map.order(f, 0, 0, 0.0, 0.0, &ctx, &mut ev);
            }
        }
        self.state.store_starmap(&map);
        self.events.extend(ev.into_iter().map(GameEvent::Starmap));
    }

    /// `FRISSIT_Viszony` (`messages == false`) or `MI_DIPLOMACY`.
    fn diplomacy_daily(&mut self, messages: bool) {
        self.with_diplomacy(|d, w| if messages { d.ai_messages(w) } else { d.daily(w) });
    }

    /// Run `f` on diplomacy's state and world (decoded from the save state, stored back afterwards).
    fn with_diplomacy(&mut self, f: impl FnOnce(&mut Diplomacy, &mut diplomacy::World)) {
        let g = self.globals();
        let mut ctx = self.diplomacy_context(&g);
        let star = self.starmap_context(&g);
        let mut d = self.state.diplomacy();
        let mut pl = self.diplo_planets();
        let mut map = self.state.starmap();
        let mut tech = self.state.tech();
        let mut rng = self.state.rng();
        let mut libc = self.libc;
        let mut draw = || libc.rand();
        let events = {
            let mut w = diplomacy::World {
                ctx: &mut ctx,
                planets: &mut pl,
                starmap: &mut map,
                star_ctx: &star,
                tech: &mut tech,
                rng: &mut rng,
                rules: &self.rules.diplomacy,
                libc: &mut draw,
                events: Vec::new(),
            };
            f(&mut d, &mut w);
            w.events
        };
        self.libc = libc;
        self.store_diplomacy_world(&d, &ctx, &pl, &map, &tech, &rng);
        self.events.extend(events.into_iter().map(GameEvent::Diplomacy));
    }

    fn store_diplomacy_world(&mut self, d: &Diplomacy, ctx: &DiplomacyContext, pl: &[DiploPlanet], map: &crate::fleet::Starmap,
        tech: &crate::colony_side::TechTable, rng: &Streams) {
        self.state.store_diplomacy(d);
        self.store_diplo_planets(pl);
        self.state.store_starmap(map);
        self.state.store_tech(tech);
        self.state.store_rng(rng);
        self.state.store_kidnap(&ctx.kidnap);
        for (e, &a) in ctx.alive.iter().enumerate() {
            if !a {
                self.state.mem.set_u8(va::EMPIRE_ALIVE + e as u32, 0);
            }
        }
    }

    fn diplomacy_tick(&mut self) {
        self.with_diplomacy(|d, w| d.tick(w));
    }

    fn ai(&mut self) {
        self.with_ai(|st, w, hooks| st.daily(w, hooks));
    }

    /// Run `f` on the AI's world (decoded from the state, stored back afterwards).
    fn with_ai(&mut self, f: impl FnOnce(&mut ai::AiState, &mut ai::World, &mut dyn AiHooks)) {
        let g = self.globals();
        let m = &self.state.mem;
        let a = self.adat();
        let ctx = AiContext {
            ai: m.flags(va::AI_EMPIRE),
            program_players2: g.pp2,
            spies_enabled: m.u8(va::SPY_ENABLED) != 0,
            story_resale: adat_i32(&a, 0xac) != 0,
            may_attack: m.flags(va::MAY_ATTACK),
            story_no_war: adat_i32(&a, 0xb0) != 0,
            story_guard: [adat_i32(&a, 0xb8) != 0, adat_i32(&a, 0xbc) != 0],
            spare_human: std::array::from_fn(|e| adat_i32(&a, 0xd0 + 4 * e) != 0),
            year: g.year,
            month: g.month,
            tick: g.tick,
            ai_off: m.i32(va::AI_OFF) != 0,
            design_debug: m.i32(va::DESIGN_DEBUG) != 0,
        };
        let sctx = self.spy_context(&g);
        let econ = self.economy_context(&g);
        let rctx = self.research_context(&g);
        let star = self.starmap_context(&g);
        let mut st = self.state.ai();
        let mut colony = self.state.colony();
        let mut combat = self.state.combat();
        let mut research = self.state.research();
        let mut relations = self.state.relations();
        let mut kidnap = self.state.kidnap();
        let mut rng = self.state.rng();
        let mut spies = self.state.spies();
        let mut map = self.state.starmap();
        let mut fleet_numbers = m.i32s(va::FLEET_NUMBERS);
        let danger = m.i32s(va::AI_DANGER);
        let power = m.f32s(va::EMPIRE_POWER);
        let planet_count = m.i32s(va::EMPIRE_PLANETS);
        let mut known = self.state.known_since();
        let mut story = |_: StoryCheck| false;
        let mut hooks = WarHooks { wars: Vec::new(), uninit: self.uninit.as_mut() };
        let (events, spy_events) = {
            let mut w = ai::World {
                ctx: &ctx,
                rules: &self.rules.ai,
                spy: spy::World {
                    ctx: &sctx,
                    colony: &mut colony,
                    side_rules: &self.rules.side,
                    econ: &econ,
                    combat: &mut combat,
                    research: &mut research,
                    research_rules: &self.rules.research,
                    research_ctx: &rctx,
                    relations: &mut relations,
                    kidnap: &mut kidnap,
                    rng: &mut rng,
                    rules: &self.rules.spy,
                    story: &mut story,
                    events: Vec::new(),
                },
                spies: &mut spies,
                starmap: &mut map,
                star_ctx: &star,
                fleet_numbers: &mut fleet_numbers,
                danger: &danger,
                power: &power,
                planet_count: &planet_count,
                known_since: &mut known,
                events: Vec::new(),
            };
            f(&mut st, &mut w, &mut hooks);
            (w.events, w.spy.events)
        };
        self.state.store_ai(&st);
        self.state.store_colony(&colony);
        self.state.store_combat(&combat);
        self.state.store_starmap(&map);
        self.state.store_research(&research);
        self.state.store_relations(&relations);
        self.state.store_kidnap(&kidnap);
        self.state.store_rng(&rng);
        self.state.store_spies(&spies);
        self.state.mem.set_i32s(va::FLEET_NUMBERS, &fleet_numbers);
        self.state.store_known_since(&known);
        self.events.extend(hooks.wars.into_iter().map(|(a, b)| GameEvent::WarDeclared { a: a as u8, b: b as u8 }));
        self.events.extend(events.into_iter().map(GameEvent::Ai));
        self.events.extend(spy_events.into_iter().map(GameEvent::Spy));
    }

    fn spies(&mut self) {
        let g = self.globals();
        let sctx = self.spy_context(&g);
        let econ = self.economy_context(&g);
        let rctx = self.research_context(&g);
        let mut colony = self.state.colony();
        let mut combat = self.state.combat();
        let mut research = self.state.research();
        let mut relations = self.state.relations();
        let mut kidnap = self.state.kidnap();
        let mut rng = self.state.rng();
        let mut spies = self.state.spies();
        let mut story = |_: StoryCheck| false;
        let events = {
            let mut w = spy::World {
                ctx: &sctx,
                colony: &mut colony,
                side_rules: &self.rules.side,
                econ: &econ,
                combat: &mut combat,
                research: &mut research,
                research_rules: &self.rules.research,
                research_ctx: &rctx,
                relations: &mut relations,
                kidnap: &mut kidnap,
                rng: &mut rng,
                rules: &self.rules.spy,
                story: &mut story,
                events: Vec::new(),
            };
            spies.daily(&mut w);
            w.events
        };
        self.state.store_colony(&colony);
        self.state.store_combat(&combat);
        self.state.store_research(&research);
        self.state.store_relations(&relations);
        self.state.store_kidnap(&kidnap);
        self.state.store_rng(&rng);
        self.state.store_spies(&spies);
        self.events.extend(events.into_iter().map(GameEvent::Spy));
    }

    fn research(&mut self) {
        let g = self.globals();
        let mut ctx = self.research_context(&g);
        let mut techs = self.state.tech();
        let mut res = self.state.research();
        let labs: [[i32; 4]; EMPIRES] = std::array::from_fn(|e| self.state.mem.i32s(va::LABS + 16 * e as u32));
        let mut money = self.state.treasuries();
        let events = research::tick(&mut techs, &mut res, &labs, &mut money, &self.rules.research, &mut ctx);
        self.state.store_tech(&techs);
        self.state.store_research(&res);
        self.state.store_treasuries(&money);
        self.state.mem.set_i32(va::IDO_SPEED, ctx.speed_mult);
        // A finished project: story special 0x0D(e, t), then the AI's designs `FRISSIT_MI_DESIGN(e, false)`
        // (production-research.md §2; inside the empire loop in the exe, which nothing later in it reads).
        for e in &events {
            if let ResearchEvent::Completed { empire, tech } = *e {
                self.story_special([0x0d, empire as i32, tech as i32, 0, 0, 0, 0, 0]);
                self.with_ai(|st, w, hooks| st.design(w, empire as usize, false, hooks));
            }
        }
        self.events.extend(events.into_iter().map(GameEvent::Research));
    }

    /// `Story_Special(data)`: the story engine checks the scripts against it.
    pub fn story_special(&mut self, data: [i32; 8]) {
        self.with_story(|st, w| st.story_special(data, &ScenarioStories, w));
    }

    fn story(&mut self) {
        self.with_story(|st, w| st.tick(&ScenarioStories, w));
    }

    /// Run `f` on the story engine (its state decoded from the save state, stored back afterwards); the
    /// scripts' requests to other systems that the replay needs are carried out.
    fn with_story(&mut self, f: impl FnOnce(&mut crate::story::Story, &mut StoryView)) {
        let g = self.globals();
        let mut st = self.state.story();
        let mut rng = self.state.rng();
        let events = {
            let mut w = StoryView { state: &self.state, g: &g, rng: &mut rng, events: Vec::new() };
            f(&mut st, &mut w);
            w.events
        };
        self.state.store_story(&st);
        self.state.store_rng(&rng);
        for e in &events {
            if let StoryEvent::StopFleet(f) = *e {
                let ctx = self.starmap_context(&g);
                let mut map = self.state.starmap();
                let mut ev = Vec::new();
                map.order(f.max(0) as usize, 0, 0, 0.0, 0.0, &ctx, &mut ev);
                self.state.store_starmap(&map);
            }
        }
        self.events.extend(events.into_iter().map(GameEvent::Story));
    }

    fn starmap(&mut self) {
        let g = self.globals();
        let mut ctx = self.starmap_context(&g);
        let ships = self.state.ship_designs();
        let mut map = self.state.starmap();
        let mut rng = self.state.rng();
        let mut colony = self.state.colony();
        let econ = self.economy_context(&g);
        let (events, side) = {
            let mut arrivals = ColonyArrivals { world: &mut colony, rules: &self.rules.side, ctx: &econ, placement: &self.rules.placement, events: Vec::new() };
            let ev = map.tick(&ships, &mut ctx, &mut rng, &mut arrivals);
            (ev, arrivals.events)
        };
        self.state.store_colony(&colony);
        self.state.store_starmap(&map);
        self.state.store_rng(&rng);
        self.state.mem.set_i32(va::IDO_SPEED, ctx.speed_mult);
        let battle = events.iter().any(|e| matches!(e, StarmapEvent::SpaceBattle { .. } | StarmapEvent::PlanetAttack { .. }));
        // The human met an empire: story special 0x0B(EN, o) and `FelfedezBirodalom(o)` (fleet-movement.md §6.3;
        // inside the radar pass in the exe, which nothing after it in the tick reads).
        for e in &events {
            if let StarmapEvent::EmpireMet { by, empire } = *e {
                if by as usize == g.en {
                    self.story_special([0x0b, by as i32, empire as i32, 0, 0, 0, 0, 0]);
                    self.with_diplomacy(|d, w| d.discover(w, empire as usize));
                }
            }
        }
        self.events.extend(events.into_iter().map(GameEvent::Starmap));
        self.events.extend(side.into_iter().map(GameEvent::Side));
        // The production block ends `FRISSIT_Starmap` (production-research.md §3); a battle returns earlier.
        if !battle {
            self.production(&g, ctx.speed_mult);
        }
    }

    fn production(&mut self, g: &Globals, speed: i32) {
        let d = g.difficulty.max(0) as usize;
        let name = self.fleet_name();
        let ctx = ProductionContext {
            speed_mult: speed,
            day: g.day,
            multiplayer: g.multiplayer,
            human_controlled: g.human,
            difficulty_production: self.rules.difficulty_production.get(d).copied().unwrap_or(1.0),
            difficulty_money: self.rules.difficulty_money.get(d).copied().unwrap_or(1.0),
            default_fleet_name: name.clone(),
            numbered_fleet_name: name,
        };
        let s = &self.state;
        let m = &s.mem;
        let combat = s.combat();
        let mut yard = Shipyard {
            planets: (0..=s.planet_count()).map(|p| yard_planet(&m.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE))).collect(),
            fleets: s.fleets.iter().map(|r| YardFleet { fleet: crate::fleet::Fleet::from_record(r), totals: crate::spacewar::FleetTotals::from_record(r) }).collect(),
            ships: combat.ships.clone(),
            vehicles: combat.vehicles.clone(),
            designs: combat.designs.clone(),
            last_fleet: (0..crate::colony_side::PLANET_SLOTS).map(|p| m.i32(va::LAST_NEAR_FLEET + 4 * p as u32)).collect(),
            fleet_numbers: m.i32s(va::FLEET_NUMBERS),
            colony_ship_pool: m.i32s(va::COLONY_SHIP_POOL),
            terraformer_pool: m.i32s(va::TERRAFORMER_POOL),
        };
        let mut money = s.treasuries();
        let events = yard.tick(&mut money, &ctx);
        let st = &mut self.state;
        st.store_treasuries(&money);
        for (p, y) in yard.planets.iter().enumerate() {
            let at = va::PLANETS + (p * PLANET_SIZE) as u32;
            let mut r = st.mem.read(at, PLANET_SIZE);
            put_yard_planet(&mut r, y);
            st.mem.write(at, &r);
        }
        st.fleets.resize(yard.fleets.len(), [0; 0x80]);
        st.mem.set_i32(va::FLEET_COUNT, yard.fleets.len() as i32 - 1);
        for (r, y) in st.fleets.iter_mut().zip(&yard.fleets) {
            put_fleet(r, &y.fleet);
            fleet_totals_put(r, &y.totals);
        }
        let mut cw = st.combat();
        cw.ships = yard.ships.iter().map(|s| Ship { fleet: s.fleet, design: s.design }).collect();
        cw.vehicles = yard.vehicles.iter().map(|v| Vehicle { fleet: v.fleet, design: v.design, count: v.count }).collect();
        cw.colony_ship_pool = yard.colony_ship_pool;
        cw.terraformer_pool = yard.terraformer_pool;
        // The fleet records were written above; keep their totals.
        cw.fleets = st.fleets.iter().map(|r| crate::save::combat_fleet(r)).collect();
        st.store_combat(&cw);
        st.mem.set_i32s(va::LAST_NEAR_FLEET, &yard.last_fleet);
        st.mem.set_i32s(va::FLEET_NUMBERS, &yard.fleet_numbers);
        self.events.extend(events.into_iter().map(GameEvent::Production));
    }
}

fn yard_planet(r: &[u8]) -> YardPlanet {
    let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
    YardPlanet {
        owner: r[0x20],
        x: f32::from_bits(i32_at(0x18) as u32),
        y: f32::from_bits(i32_at(0x1c) as u32),
        halted: r[0x64],
        capacity: [i32_at(0x70), i32_at(0x74)],
        queue: Queue::from_bytes(&r[0x78..0xb8]),
        garrison: i32_at(0x58),
        garrison_strength: i32_at(0x5c),
        stations: std::array::from_fn(|k| i16::from_le_bytes([r[0x100 + 2 * k], r[0x101 + 2 * k]])),
        station_count: i32_at(0x114),
    }
}

fn put_yard_planet(r: &mut [u8], p: &YardPlanet) {
    r[0x78..0xb8].copy_from_slice(&p.queue.to_bytes());
    r[0x58..0x5c].copy_from_slice(&p.garrison.to_le_bytes());
    r[0x5c..0x60].copy_from_slice(&p.garrison_strength.to_le_bytes());
    for (k, s) in p.stations.iter().enumerate() {
        r[0x100 + 2 * k..0x102 + 2 * k].copy_from_slice(&s.to_le_bytes());
    }
    r[0x114..0x118].copy_from_slice(&p.station_count.to_le_bytes());
}

/// The AI's hooks: war declarations are recorded (diplomacy carries them out when the replay meets one); appended
/// records come from [`Game::uninit`].
struct WarHooks<'a, 'r> {
    wars: Vec<(usize, usize)>,
    uninit: Option<&'a mut Box<dyn FnMut(Fresh) -> Vec<u8> + 'r>>,
}

impl WarHooks<'_, '_> {
    fn fresh<const N: usize>(&mut self, what: Fresh) -> [u8; N] {
        let mut out = [0; N];
        if let Some(f) = self.uninit.as_mut() {
            let b = f(what);
            let n = b.len().min(N);
            out[..n].copy_from_slice(&b[..n]);
        }
        out
    }
}

impl AiHooks for WarHooks<'_, '_> {
    fn declare_war(&mut self, _w: &mut ai::World<'_>, a: usize, b: usize) {
        self.wars.push((a, b));
    }

    fn fresh_fleet(&mut self, f: usize) -> [u8; 0x80] {
        self.fresh(Fresh::Fleet(f))
    }

    fn fresh_design(&mut self, e: usize, d: usize) -> [u8; 0x84] {
        self.fresh(Fresh::Design { empire: e, design: d })
    }
}

/// The story engine's view of the state.
struct StoryView<'a> {
    state: &'a SaveState,
    g: &'a Globals,
    rng: &'a mut Streams,
    events: Vec<StoryEvent>,
}

impl StoryView<'_> {
    fn planet(&self, p: i32) -> Vec<u8> {
        self.state.mem.read(va::PLANETS + (p.max(0) as usize * PLANET_SIZE) as u32, PLANET_SIZE)
    }
    fn fleet(&self, f: i32) -> Option<&[u8; 0x80]> {
        self.state.fleets.get(f.max(0) as usize)
    }
}

impl StoryWorld for StoryView<'_> {
    fn ido(&self) -> i32 {
        self.g.tick
    }
    fn speed_mult(&self) -> i32 {
        self.g.speed
    }
    fn human(&self) -> i32 {
        self.g.en as i32
    }
    fn difficulty(&self) -> i32 {
        self.g.difficulty
    }
    fn multiplayer(&self) -> bool {
        self.g.multiplayer
    }
    fn rng(&mut self) -> &mut Streams {
        self.rng
    }
    fn planet_count(&self) -> i32 {
        self.state.planet_count() as i32
    }
    fn planet_owner(&self, p: i32) -> i32 {
        self.planet(p)[0x20] as i32
    }
    fn planet_value(&self, p: i32) -> i32 {
        i32::from_le_bytes(self.planet(p)[0x28..0x2c].try_into().unwrap())
    }
    fn planet_visibility(&self, p: i32) -> i32 {
        let e = self.g.en;
        let r = self.planet(p);
        i16::from_le_bytes([r[0xd8 + 2 * e], r[0xd9 + 2 * e]]) as i32
    }
    fn fleet_count(&self) -> i32 {
        self.state.fleets.len() as i32 - 1
    }
    fn fleet_owner(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0xff, |r| r[0x15] as i32)
    }
    fn fleet_id(&self, f: i32) -> i32 {
        self.fleet(f).map_or(0, |r| i16::from_le_bytes([r[0x16], r[0x17]]) as i32)
    }
    fn empire_alive(&self, e: i32) -> bool {
        self.g.alive.get(e.max(0) as usize).copied().unwrap_or(false)
    }
    fn planets_of(&self, e: i32) -> i32 {
        self.state.mem.i32(va::EMPIRE_PLANETS + 4 * e.max(0) as u32)
    }
    fn money(&self, e: i32) -> f64 {
        self.state.treasuries()[e.clamp(0, EMPIRES as i32 - 1) as usize].available(0)
    }
    fn tech_status(&self, tech: i32, e: i32) -> i32 {
        self.state.mem.u8(va::TECHS + (tech.max(0) as usize * crate::save::TECH_SIZE) as u32 + 0x45 + e.max(0) as u32) as i32
    }
    fn relation_level(&self, a: i32, b: i32) -> i32 {
        self.state.mem.u8(va::RELATIONS + ((a.max(0) as usize * EMPIRES + b.max(0) as usize) * 0x26) as u32 + 0x25) as i32
    }
    fn planet_infected(&self, p: i32) -> bool {
        let r = self.planet(p);
        i16::from_le_bytes([r[0xb8], r[0xb9]]) > 0
    }
    fn fleet_infected(&self, f: i32) -> bool {
        self.fleet(f).is_some_and(|r| r[0x15] != 0xff && r[0x52] & 4 != 0)
    }
    fn event(&mut self, ev: StoryEvent) {
        self.events.push(ev);
    }
}
