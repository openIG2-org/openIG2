//! Colony economy: buildings, energy and workers, problems, morale, population, taxes and
//! production capacity (`FRISSIT_Planet`, exe 0x474930). Spec: `re/rules/colony-economy.md`.
//!
//! The side systems that need the whole world (rebellion, change of owner, autobuild, money,
//! per-empire totals, yearly income) are in [`crate::colony_side`] (`re/rules/colony-side.md`);
//! [`economy_step`] here reports them as [`ColonyEvent`]s instead.

use crate::rng::{stream, Streams};

/// Building types (descriptor indices) with fixed roles in the economy (spec §2.4).
pub mod kind {
    pub const COLONY_HUB: usize = 1;
    pub const SMALL_HOUSE: usize = 2;
    pub const LARGE_HOUSE: usize = 3;
    pub const FOOD_FACTORY: usize = 4;
    pub const AUTO_FOOD_FACTORY: usize = 5;
    pub const HOSPITAL: usize = 6;
    pub const POLICE: usize = 7;
    pub const WATER_PLANT: usize = 9;
    pub const PARK: usize = 10;
    pub const BAR: usize = 11;
    pub const RECREATION: usize = 12;
    pub const STADIUM: usize = 13;
    /// Ship, advanced ship, tank, advanced tank factories.
    pub const FACTORIES: std::ops::RangeInclusive<usize> = 19..=22;
    /// Factories plus two unused factory-like slots: the polluters.
    pub const POLLUTERS: std::ops::RangeInclusive<usize> = 19..=24;
    pub const PRODUCTION_IMPROVEMENT: usize = 25;
    /// Defences keep working when badly damaged.
    pub const DEFENCES: std::ops::RangeInclusive<usize> = 28..=43;
    /// Planetary guns 1–4: their presence sets the spy-hunting level.
    pub const GUNS: std::ops::RangeInclusive<usize> = 28..=31;
    pub const SELF_DESTRUCT: usize = 43;
    /// Fusion, solar, magma, mezon plants (energy producers, with the colony hub).
    pub const POWER_PLANTS: std::ops::RangeInclusive<usize> = 52..=55;
}

/// Problem slots (spec §5.2; names from the game's string table).
pub mod problem {
    pub const ELECTRICITY: usize = 1;
    pub const FOOD: usize = 2;
    pub const LIVING_SPACE: usize = 3;
    pub const HOSPITAL: usize = 4;
    pub const FAMILY_CAPTURED: usize = 6;
    pub const RECREATION: usize = 10;
    pub const SELF_DESTRUCT_FEAR: usize = 11;
    pub const COLONY_HUB: usize = 12;
    pub const STADIUM: usize = 13;
    pub const CRIMINALS: usize = 16;
    pub const CONTAMINATION: usize = 18;
    pub const HIGH_TAXES: usize = 19;
    /// Slots 1..=19 are problems; slot 0 is the morale curve.
    pub const COUNT: usize = 20;
}

/// Building instance flags (`+0x19`).
pub mod flag {
    pub const WORKING: u8 = 0x01;
    pub const DAMAGED: u8 = 0x04;
    pub const NO_ENERGY: u8 = 0x08;
    pub const SWITCHED_OFF: u8 = 0x20;
    pub const NO_WORKERS: u8 = 0x40;
}

/// Planet event codes read by the economy (`BolygoInfoIs` arguments; i16 slots on the planet).
pub mod event {
    pub const FACTORIES_STOPPED: i16 = -0x673;
    pub const FOOD_HALVED: i16 = 0x5d3;
    /// Codes 200..=204 reduce food capacity to 90, 85, 75, 65, 50 %.
    pub const FOOD_LOSS_FIRST: i16 = 200;
    pub const FOOD_LOSS_LAST: i16 = 204;
    pub const EPIDEMIC: i16 = 100;
    pub const HOSPITAL_SHORTAGE: i16 = 0x65;
    pub const MORALE_PENALTY: i16 = 0x91b;
    pub const MORALE_CAP_190: i16 = -0x1f7c;
    pub const POPULATION_FROZEN: i16 = -0x1f4a;
    pub const GDP_BONUS: i16 = 0xc30;
    pub const NO_TAXES: i16 = -0xc44;
    pub const RICHNESS_20_A: i16 = 0xc3a;
    pub const RICHNESS_20_B: i16 = 0x1f41;
    pub const RICHNESS_10: i16 = 0x1f42;
}

/// The building-descriptor fields the economy reads (`EpuletekLeiro`, spec §2.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildingKind {
    /// +0: the tech that prices the building (and its research state for autobuild).
    pub tech: u8,
    pub priority: u8,
    /// +4: construction work in days (×50 when placed).
    pub build_work: i32,
    pub max_hp: i32,
    pub energy: i32,
    pub workers: i32,
    pub output: i32,
    /// +32 bit 0: effect = scaled output (else 1 per building).
    pub scaled_effect: bool,
    /// +33 bit 0: the effect persists while not working.
    pub passive_effect: bool,
}

/// One `ProblemakLeiro` entry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Curve {
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub d: i32,
    pub rate: i32,
    pub weight: i32,
}

impl Curve {
    /// Spec §5.1: `c` at no capacity, else `c × (a/c)^((x/cap × 100/d)^−b)`, truncated.
    pub fn eval(&self, x: f64, cap: f64) -> i32 {
        if cap == 0.0 {
            return self.c;
        }
        let r = x / cap * 100.0 / self.d as f64;
        let e = r.powf(-(self.b as f64));
        (self.c as f64 * (self.a as f64 / self.c as f64).powf(e)) as i32
    }
}

/// Rule tables the economy needs, normally read from the user's exe (`ig2-rules`, ADR 0004).
#[derive(Debug, Clone, Default)]
pub struct ColonyRules {
    pub buildings: Vec<BuildingKind>,
    /// 20 curves: 0 = morale, 1..=19 = problems.
    pub curves: Vec<Curve>,
    /// `[race][surface]` growth factor in percent (`FajBolygoViszony`).
    pub surface_growth: Vec<[i32; 9]>,
    /// Per race (`FajFelelemOnmegsemmisitotolSzint`).
    pub self_destruct_fear: Vec<i32>,
    /// Per kidnapped-family-member state (`CsaladtagMoralCsokkenesLeiro`).
    pub kidnap_morale: Vec<i32>,
    /// Per race production multiplier (`FajSpeckoGyartasKapacitasSzorzo`).
    pub race_production: Vec<f32>,
}

/// One building slot (32-byte instance, spec §2.2). Empty slots have `kind == 0`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Building {
    pub kind: u8,
    /// `+1`: owner when built.
    pub owner: u8,
    pub damage: i32,
    /// Construction work left; 0 = finished.
    pub work_left: u16,
    /// `+0xa`: planet index (low byte).
    pub planet: u8,
    /// `+0xc / +0x10`: position on the colony map.
    pub x: f32,
    pub y: f32,
    pub yield_pct: u8,
    pub flags: u8,
    pub shown_energy: i16,
    pub shown_workers: i16,
    /// `256 × hp / max_hp`, refreshed while working.
    pub efficiency: i16,
}

impl Building {
    pub fn working(&self) -> bool {
        self.flags & flag::WORKING != 0
    }
}

/// The economy-relevant state of one planet (`BolygoTipus`, spec §2.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Planet {
    /// `+0x18 / +0x1c`: starmap position.
    pub x: f32,
    pub y: f32,
    pub owner: u8,
    pub race: u8,
    pub surface: u8,
    /// `+0x23`: colony map (`colony/gen_maps/<n>.map`, `Terepek[n]`) for building placement.
    pub colony_map: u8,
    pub richness: i32,
    pub population: i32,
    pub population_change: i32,
    pub morale: i32,
    pub tax_level: u8,
    pub gdp_acc: i32,
    pub gdp_last: i32,
    pub tax_acc: i32,
    pub tax_last: i32,
    pub energy_produced: i32,
    pub energy_demand: i32,
    pub workers_demand: i32,
    /// `+0x54` (u16) and `+0x58 / +0x5c` (i32): cleared for a new colony. `+0x58 / +0x5c` are the garrison's
    /// vehicles and their ground strength (`TankFlottahozHozzaszamol`); `+0x60` is cleared with them by
    /// `FlottakUjraSzamol`.
    pub field_54: u16,
    pub field_58: i32,
    pub field_5c: i32,
    pub field_60: i32,
    /// `+0x64 != 0` (the exe tests the whole byte): no rebellion counting (SP) and never abandoned.
    pub protected: bool,
    pub autobuild_counter: i32,
    /// `+0x6c`: autobuild mode 1..=10 (`autobldN.txt`), 0 = off; `+0x6d`: aggressiveness 1..=3, 0 = off.
    pub autobuild_mode: u8,
    pub autobuild_aggr: u8,
    pub ship_capacity: i32,
    pub vehicle_capacity: i32,
    /// `+0x78..+0xb8`: production queue (starmap production; reset from `pGyartas0`).
    pub production: [u8; 64],
    pub epidemic: i16,
    /// Spy level per empire (`+0xc4 + e`).
    pub spies: [u8; 19],
    /// Visibility per empire (`+0xd8 + 2e`, radar module).
    pub visibility: [i16; 19],
    /// `+0x100 + 2k`: space stations (design ids) `1..=field_114`.
    pub stations: [i16; 10],
    /// `+0x114`: the station count.
    pub field_114: i32,
    /// Smoothed problem levels (`+0x118 + 4p`), index 1..=19.
    pub problems: [i32; problem::COUNT],
    /// Event codes, slots 1..=5 at `+0x16a` (`BolygoInfoIs` never reads `+0x168`).
    pub events: [i16; 5],
    pub morale_penalty: i32,
    pub morale_penalty_rate: i32,
    /// `+0x186`: can't rebel.
    pub loyal: bool,
    /// `+0x187`: can't be taken by the AI (rebels can't side with a neighbour).
    pub ai_proof: bool,
    /// Rebellion counter (`BolygoMennyiIdejeLazad[p]`).
    pub unrest: i32,
    /// Slots 1..=n of `Epuletek[p]`.
    pub buildings: Vec<Building>,
}

impl Default for Planet {
    fn default() -> Self {
        Self {
            x: 0.0, y: 0.0, owner: 0, race: 0, surface: 0, colony_map: 0, richness: 0, population: 0, population_change: 0,
            morale: 0, tax_level: 0, gdp_acc: 0, gdp_last: 0, tax_acc: 0, tax_last: 0, energy_produced: 0,
            energy_demand: 0, workers_demand: 0, field_54: 0, field_58: 0, field_5c: 0, field_60: 0, protected: false,
            autobuild_counter: 0, autobuild_mode: 0, autobuild_aggr: 0, ship_capacity: 0, vehicle_capacity: 0,
            production: [0; 64], epidemic: 0, spies: [0; 19], visibility: [0; 19], stations: [0; 10], field_114: 0,
            problems: [0; problem::COUNT], events: [0; 5], morale_penalty: 0, morale_penalty_rate: 0,
            loyal: false, ai_proof: false, unrest: 0, buildings: Vec::new(),
        }
    }
}

impl Planet {
    /// `BolygoInfoIs`.
    pub fn has_event(&self, code: i16) -> bool {
        self.events.contains(&code)
    }

    /// `BolygoInfoIsAny`.
    pub fn any_event(&self) -> bool {
        self.events.iter().any(|&e| e != 0)
    }

    /// `BolygoInfoRange`: the first event code in `lo..=hi`.
    pub fn event_in(&self, lo: i16, hi: i16) -> Option<i16> {
        self.events.iter().copied().find(|e| (lo..=hi).contains(e))
    }
}

/// Story-script globals the economy reads or writes (`StoryAdat`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StoryHooks {
    /// `+0x20`: planet whose food capacity is halved.
    pub food_halved_planet: i32,
    /// `+0x24`: empire whose lab levels are also credited to the human player.
    pub lab_bonus_empire: i32,
    /// `+0x2c`: running total of food capacity lost to events (f32 add, truncated).
    pub food_lost: i32,
    /// `+0x30`: non-zero: set/clear the hospital-shortage event.
    pub hospital_watch: i32,
    /// `+0x9c..+0xa8`: the human's starting lab levels (single player), re-applied every step.
    pub lab_start: [i32; 3],
}

/// Global inputs of one economy step.
#[derive(Debug, Clone, Default)]
pub struct EconomyContext {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub multiplayer: bool,
    /// The local human's empire (`EN`).
    pub human_empire: u8,
    /// `PlayerKi[e] == 1`.
    pub human_controlled: [bool; 19],
    /// `PlayerFaj[e]`.
    pub empire_race: [u8; 19],
    /// `ProgramPlayers2`.
    pub players: i32,
    /// `CsaladtagElrabolvaKi[e]`, empires 0..9.
    pub kidnap_state: [i32; 9],
    /// `GlobalisMoralCSokkentoBarkacs` and its decay speed.
    pub global_morale: i32,
    pub global_morale_speed: i32,
    pub story: StoryHooks,
    /// Story-script variables (`st_vGet`) read by new buildings' yields; unset = 0.
    pub story_values: Vec<(i32, u8)>,
}

impl EconomyContext {
    /// `st_vGet(code)`.
    pub fn story_value(&self, code: i32) -> u8 {
        self.story_values.iter().find(|(c, _)| *c == code).map_or(0, |&(_, v)| v)
    }
}

/// Something the economy step asks other systems to do, or reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColonyEvent {
    /// Construction finished on this slot (0-based): `EpuletFelepult`.
    BuildingFinished { slot: usize },
    /// The unrest counter passed 30: run `BolygoFellazad` (stream 8).
    Rebellion,
    /// Warn the human owner about unrest.
    UnrestWarning,
    /// Epidemic over: remove event 100 and run the virus cleanup.
    EpidemicEnded,
    /// Add (true) or remove (false) a planet event code.
    SetEvent { code: i16, on: bool },
    /// The spy of this empire was caught.
    SpyExpelled { empire: u8 },
    /// No buildings left: the planet is lost (`BolygoElfoglal(p, 0)`).
    Abandoned,
}

/// Per-type results of the building pass (spec §4 step 6).
#[derive(Debug, Clone)]
pub struct Effects {
    pub effect: [i32; 56],
    pub active: [i32; 56],
    pub inactive: [i32; 56],
    pub under_construction: [i32; 56],
    /// Occupied slots (`local_30`).
    pub occupied: i32,
    /// Energy demand and production of this pass (the step keeps using these locals even after a
    /// construction refresh rewrites `+0x48/+0x4c`, exe 0x474930).
    pub energy_demand: i32,
    pub energy_produced: i32,
}

impl Default for Effects {
    fn default() -> Self {
        Self { effect: [0; 56], active: [0; 56], inactive: [0; 56], under_construction: [0; 56], occupied: 0, energy_demand: 0, energy_produced: 0 }
    }
}

impl Effects {
    fn sum(a: &[i32; 56], r: std::ops::RangeInclusive<usize>) -> i32 {
        a[r].iter().sum()
    }
}

/// Cut `pool` (indexed by priority class) from class 6 down until its total fits `limit`.
fn cut_to(pool: &mut [i32; 7], limit: i32) {
    let mut total: i32 = pool.iter().sum();
    for class in (0..7).rev() {
        if total <= limit {
            break;
        }
        let excess = total - limit;
        if pool[class] < excess {
            total -= pool[class];
            pool[class] = 0;
        } else {
            pool[class] -= excess;
            total = limit;
        }
    }
}

/// Spec §4: work out which buildings run, the planet's energy and worker figures, and the
/// per-type effects. Runs on every call (economy step and refreshes).
pub fn building_pass(planet: &mut Planet, rules: &ColonyRules, owner_is_human: bool) -> Effects {
    let mut fx = Effects::default();
    let mut energy = [0i32; 7];
    let mut workers = [0i32; 7];
    for b in planet.buildings.iter_mut() {
        if b.kind == 0 {
            b.flags &= !flag::WORKING;
            continue;
        }
        fx.occupied += 1;
        if b.work_left != 0 {
            b.flags &= !flag::WORKING;
            continue;
        }
        if !owner_is_human {
            b.flags &= !flag::SWITCHED_OFF;
        }
        let k = &rules.buildings[b.kind as usize];
        let damaged = k.max_hp < b.damage * 2 && !kind::DEFENCES.contains(&(b.kind as usize));
        b.flags = (b.flags & !flag::DAMAGED) | if damaged { flag::DAMAGED } else { 0 };
        if b.flags & (flag::DAMAGED | flag::SWITCHED_OFF) == 0 {
            energy[k.priority as usize] += k.energy;
            workers[k.priority as usize] += k.workers;
            b.flags |= flag::WORKING;
        } else {
            b.flags &= !flag::WORKING;
        }
    }

    planet.workers_demand = workers.iter().sum();
    cut_to(&mut workers, planet.population);
    let factories_stopped = planet.has_event(event::FACTORIES_STOPPED);
    for b in planet.buildings.iter_mut() {
        if factories_stopped && kind::FACTORIES.contains(&(b.kind as usize)) {
            b.flags &= !flag::WORKING;
        }
        b.flags &= !(flag::NO_WORKERS | flag::NO_ENERGY);
        if b.kind != 0 && b.working() {
            let k = &rules.buildings[b.kind as usize];
            let pool = &mut workers[k.priority as usize];
            if *pool < k.workers {
                b.flags = (b.flags & !flag::WORKING) | flag::NO_WORKERS;
            } else {
                *pool -= k.workers;
            }
        }
    }

    let produced: i32 = planet
        .buildings
        .iter()
        .filter(|b| b.working() && (b.kind == 1 || kind::POWER_PLANTS.contains(&(b.kind as usize))))
        .map(|b| {
            let k = &rules.buildings[b.kind as usize];
            k.output * b.yield_pct as i32 * (k.max_hp - b.damage) / k.max_hp
        })
        .sum();
    planet.energy_produced = produced;
    planet.energy_demand = energy.iter().sum();
    (fx.energy_demand, fx.energy_produced) = (planet.energy_demand, produced);
    cut_to(&mut energy, produced);
    for b in planet.buildings.iter_mut().filter(|b| b.kind != 0 && b.working()) {
        let k = &rules.buildings[b.kind as usize];
        let pool = &mut energy[k.priority as usize];
        if *pool < k.energy {
            b.flags = (b.flags & !flag::WORKING) | flag::NO_ENERGY;
        } else {
            *pool -= k.energy;
            b.flags &= !flag::NO_ENERGY;
        }
    }

    for b in planet.buildings.iter_mut().filter(|b| b.kind != 0) {
        let t = b.kind as usize;
        let k = &rules.buildings[t];
        let scaled = |eff: i16| k.output * b.yield_pct as i32 * eff as i32 / 256;
        if b.working() {
            b.shown_energy = k.energy as i16;
            b.shown_workers = k.workers as i16;
            b.efficiency = (256 * (k.max_hp - b.damage) / k.max_hp) as i16;
            fx.effect[t] += if k.scaled_effect { scaled(b.efficiency) } else { 1 };
            fx.active[t] += 1;
        } else {
            if b.work_left == 0 && k.passive_effect {
                fx.effect[t] += if k.scaled_effect { scaled(b.efficiency) } else { 1 };
            }
            fx.inactive[t] += 1;
            if b.work_left != 0 {
                fx.under_construction[t] += 1;
            }
        }
    }
    fx
}

/// `GetBolygoGazdagsag` (exe 0x4aca20): richness × race multiplier + event bonus, in f32.
pub fn production_base(planet: &Planet, rules: &ColonyRules, ctx: &EconomyContext) -> i32 {
    let mut bonus = 0;
    if !ctx.multiplayer && planet.any_event() {
        if planet.has_event(event::RICHNESS_20_A) || planet.has_event(event::RICHNESS_20_B) {
            bonus = 20;
        }
        if planet.has_event(event::RICHNESS_10) {
            bonus = 10;
        }
        if planet.has_event(event::MORALE_PENALTY) {
            bonus = 15;
        }
    }
    let empire = if planet.owner == 0 { ctx.human_empire } else { planet.owner };
    let race = ctx.empire_race[empire as usize] as usize;
    (planet.richness as f32 * rules.race_production[race] + bonus as f32) as i32
}

/// Spec §8: ship and vehicle production capacity from the factories' effects.
pub fn production_capacity(planet: &mut Planet, fx: &Effects, rules: &ColonyRules, ctx: &EconomyContext) {
    let base = production_base(planet, rules, ctx);
    planet.ship_capacity = base * (fx.effect[19] + fx.effect[20]) / 100;
    planet.vehicle_capacity = base * (fx.effect[21] + fx.effect[22]) / 100;
    if fx.effect[kind::PRODUCTION_IMPROVEMENT] > 0 {
        planet.ship_capacity = (planet.ship_capacity as f64 * 1.5) as i32;
        planet.vehicle_capacity = (planet.vehicle_capacity as f64 * 1.5) as i32;
    }
}

/// A display refresh (`FRISSIT_Planet(false, p)`): building pass and production only. Returns the recount of the
/// used buildings, `HanyEpulet[p]` (exe 0xBA9518; it heals a count an empty slot's demolish took down).
pub fn refresh(planet: &mut Planet, rules: &ColonyRules, ctx: &EconomyContext) -> i32 {
    let human = ctx.human_controlled[planet.owner as usize];
    let fx = building_pass(planet, rules, human);
    production_capacity(planet, &fx, rules, ctx);
    fx.occupied
}

/// `exp(ln(n) × 1.5)` with the exe's f32 rounding of the argument and of `ln` (spec §10).
fn pow_1_5(n: i32) -> f64 {
    if n < 1 {
        return 0.0;
    }
    (((n as f32 as f64).ln() as f32) as f64 * 1.5).exp()
}

/// The inverse, `exp(ln(s) / 1.5)`, truncated (spec §6).
fn root_1_5(s: i32) -> i32 {
    if s == 0 {
        return 0;
    }
    (((s as f32 as f64).ln() as f32) as f64 / 1.5).exp() as i32
}

/// Spec §6: the morale implied by a weighted problem sum.
fn morale_from(sum: i32, curve: &Curve) -> i32 {
    let level = root_1_5(sum);
    let m = if level == -10000 { 0 } else { curve.eval((level + 10000) as f64, 10000.0) };
    1000 - 10 * m
}

/// Per-planet results of an economy step that live outside the planet record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanetReport {
    /// `BolygoNeededMoral[p]`: the morale the targets imply.
    pub needed_morale: i32,
    /// This step's problem targets ×10000 (read by autobuild).
    pub targets: [i32; problem::COUNT],
    /// Monthly payout credited to the owner (money pool 0), with the month's GDP.
    pub tax_paid: i32,
    pub gdp: i32,
    pub events: Vec<ColonyEvent>,
}

/// Raw problem targets (before ×10000), spec §5.2. Consumes stream-3 rolls in the original order.
fn problem_targets(
    index: i32,
    planet: &mut Planet,
    fx: &Effects,
    rules: &ColonyRules,
    ctx: &mut EconomyContext,
    rng: &mut Streams,
    events: &mut Vec<ColonyEvent>,
) -> [i32; problem::COUNT] {
    let mut t = [0i32; problem::COUNT];
    let c = &rules.curves;
    let pop = planet.population;
    let popf = pop as f64;
    let e = &fx.effect;

    t[problem::ELECTRICITY] = if fx.energy_demand == 0 {
        0
    } else {
        c[1].eval(fx.energy_demand as f64, fx.energy_produced as f64)
    };

    let food_cap = e[kind::FOOD_FACTORY] + 5000 + e[kind::AUTO_FOOD_FACTORY];
    let (mut keep, mut lost) = (1.0f32, 0.0f32);
    let loss_event = planet.event_in(event::FOOD_LOSS_FIRST, event::FOOD_LOSS_LAST);
    if let Some(code) = loss_event {
        keep = [0.9f32, 0.85, 0.75, 0.65, 0.5][(code - event::FOOD_LOSS_FIRST) as usize];
        lost = 1.0 - keep;
    }
    if planet.has_event(event::FOOD_HALVED) || ctx.story.food_halved_planet == index {
        keep = (keep as f64 * 0.5) as f32;
        lost = (lost as f64 * 0.5) as f32;
    }
    let food_capf = food_cap as f32 * keep;
    t[problem::FOOD] = if pop == 0 {
        0
    } else if food_capf == 0.0 {
        c[2].c
    } else {
        c[2].eval(popf, food_capf as f64)
    };
    if loss_event.is_some() {
        ctx.story.food_lost = (food_cap as f32 * lost + ctx.story.food_lost as f32) as i32;
    }

    let living = e[kind::LARGE_HOUSE] * 12000 + 5000 + e[kind::SMALL_HOUSE] * 7000;
    t[problem::LIVING_SPACE] = if pop == 0 { 0 } else { c[3].eval(popf, living as f64) };

    let hospitals = (e[kind::HOSPITAL] + 1) * 7500;
    if planet.epidemic == 0 {
        t[problem::HOSPITAL] = if pop == 0 { 0 } else { c[4].eval(popf, hospitals as f64) };
        if ctx.story.hospital_watch != 0 {
            events.push(ColonyEvent::SetEvent { code: event::HOSPITAL_SHORTAGE, on: pop < hospitals / 2 });
        }
    } else {
        let half = hospitals / 2;
        t[problem::HOSPITAL] = if pop == 0 { 0 } else { c[4].eval(popf, half as f64) };
        // The original divides by the population even when it is 0 (a crash); colonies never drop
        // below 1100 people, so treat that case as "no cure" instead.
        let whole = half.checked_div(pop).unwrap_or(0);
        let frac = if pop == 0 { 0.0 } else { (half as f32 / pop as f32 - whole as f32) * 100.0 };
        let roll = rng.range(stream::PLANET, 1, 100);
        let cured = whole + ((roll as f32) < frac) as i32;
        planet.epidemic = planet.epidemic.wrapping_sub(cured as i16);
        if planet.epidemic < 1 {
            planet.epidemic = 0;
            events.push(ColonyEvent::EpidemicEnded);
        }
    }

    let kidnap = if planet.owner < 9 { ctx.kidnap_state[planet.owner as usize] } else { 0 };
    t[problem::FAMILY_CAPTURED] = rules.kidnap_morale.get(kidnap as usize).copied().unwrap_or(0);

    let g = |n: i32| 2.0f32 - 2.0 / (n as f32 + 1.0);
    let fun = (g(e[kind::PARK]) * 4000.0 + 3000.0
        + g(e[kind::BAR]) * 8000.0
        + g(e[kind::RECREATION]) * 14000.0
        + g(e[kind::STADIUM]) * 30000.0) as i32;
    t[problem::RECREATION] = if pop == 0 { 0 } else { c[10].eval(popf, fun as f64) };

    if e[kind::SELF_DESTRUCT] >= 1 {
        let fear = rules.self_destruct_fear[planet.race as usize];
        let v = (rng.range(stream::PLANET, -1, 1)).min(fear);
        t[problem::SELF_DESTRUCT_FEAR] = v.max(fear / 2);
    }
    if e[kind::COLONY_HUB] == 0 {
        t[problem::COLONY_HUB] = rng.range(stream::PLANET, -1, 1).clamp(10, 20);
    }
    if e[kind::STADIUM] == 0 && pop != 0 {
        t[problem::STADIUM] = c[13].eval(popf, 30000.0);
    }
    t[problem::CRIMINALS] = if pop == 0 { 0 } else { c[16].eval(popf, (e[kind::POLICE] + 10000) as f64) };
    let dirt = Effects::sum(e, kind::POLLUTERS);
    t[problem::CONTAMINATION] =
        if dirt == 0 { 0 } else { c[18].eval(dirt as f64, (e[kind::PARK] * 8000 + 5000) as f64) };

    const TAX: [i32; 8] = [0, 6, 10, 15, 21, 27, 35, 43];
    let base = TAX.get(planet.tax_level as usize).copied().unwrap_or(0);
    let wobble = ((ctx.year + ctx.month + index) * 3) % 7 - 3;
    t[problem::HIGH_TAXES] = (wobble + (pop + 250_000) / 250_000 * base).max(0);
    t
}

/// Spec §5.3 and §6: move the problem levels toward their targets and derive morale.
/// Returns the needed morale.
fn smooth_and_morale(planet: &mut Planet, targets: &[i32; problem::COUNT], rules: &ColonyRules, ctx: &EconomyContext) -> i32 {
    let (mut now, mut want) = (0i32, 0i32);
    // Rewritten in the global table before every planet (spec §5.3).
    let hospital = if planet.epidemic == 0 { (10, 100) } else { (200, 400) };
    let slots = targets.iter().zip(planet.problems.iter_mut()).zip(&rules.curves).enumerate().skip(1);
    for (p, ((&raw_target, level), &table)) in slots {
        let mut curve = table;
        if p == problem::HOSPITAL {
            (curve.rate, curve.weight) = hospital;
        }
        let target = raw_target * 10000;
        let cur = *level;
        let r = cur as f32 / (target + 1) as f32;
        let mut step = if r > 1.5 || (r as f64) < 0.66 { 250 } else { 50 };
        if r > 2.0 || r < 0.5 {
            step *= 5;
        }
        let limit = step * curve.rate;
        *level = if target > cur { cur + (target - cur).min(limit) } else { cur - (cur - target).min(limit) };
        now = (now as f64 + pow_1_5(*level * curve.weight / 10000)) as i32;
        want = (want as f64 + pow_1_5(curve.weight * target / 10000)) as i32;
    }

    let mut morale = morale_from(now, &rules.curves[0]);
    let needed = morale_from(want, &rules.curves[0]).clamp(0, 999);
    if !ctx.multiplayer {
        let goal = if planet.has_event(event::MORALE_PENALTY) { 200 } else { 0 };
        if planet.morale_penalty > goal {
            planet.morale_penalty = (planet.morale_penalty - planet.morale_penalty_rate).max(goal);
        }
        if planet.morale_penalty < goal {
            planet.morale_penalty = (planet.morale_penalty + planet.morale_penalty_rate).min(goal);
        }
        morale -= planet.morale_penalty + ctx.global_morale;
        if planet.has_event(event::MORALE_CAP_190) && morale > 200 {
            morale = 190;
        }
        morale = morale.clamp(0, 999);
    }
    planet.morale = morale;
    needed
}

/// Spec §7: the monthly population change (momentum toward a morale-driven target).
fn grow(planet: &mut Planet, fx: &Effects, rules: &ColonyRules, ctx: &EconomyContext, rng: &mut Streams) {
    let mr = planet.morale - planet.morale % 100;
    let mut f = rules.surface_growth[planet.race as usize][planet.surface as usize] as f32 / 100.0;
    if fx.effect[kind::WATER_PLANT] > 0 {
        f = (f as f64 + 0.1) as f32;
    }
    if mr < 400 {
        f = 1.0;
    }
    let f300 = f * 300.0;
    let target = ((mr / 10 - 40) as f32 * f300 / 50.0) as i32;
    let spread = (f300 / 5.0) as i32;
    let roll = rng.range(stream::PLANET, spread / 2, (spread as f64 * 1.5) as i32);
    let last = planet.population_change;
    let mut m = if target < 1 { last - roll } else { last + roll };
    if m.abs() > target.abs() {
        m = target + rng.range(stream::PLANET, -30, 30);
    }
    if fx.effect[kind::COLONY_HUB] == 0 && m > 0 {
        m = 0;
    }
    let before = planet.population;
    if !planet.has_event(event::POPULATION_FROZEN) || ctx.multiplayer {
        planet.population += m + rng.range(stream::PLANET, -10, 10);
    }
    if planet.population < 1100 {
        planet.population = 1100 + rng.range(stream::PLANET, 0, 100);
    }
    planet.population_change = planet.population - before;
}

/// Spec §11: the spy-hunting level from planetary guns, and the expulsion rolls.
fn hunt_spies(planet: &mut Planet, fx: &Effects, ctx: &EconomyContext, rng: &mut Streams, events: &mut Vec<ColonyEvent>) {
    const ODDS: [i32; 15] = [0, 0, 0, 0, 0, -1, 120, 90, 60, 30, -1, 1440, 720, 540, 360];
    let level = kind::GUNS.filter(|&t| fx.active[t] > 0).map(|t| t - 27).max().unwrap_or(0);
    if level == 0 {
        return;
    }
    for e in 1..=ctx.players.clamp(0, 18) as usize {
        let spy = planet.spies[e] as usize;
        if spy == 0 {
            continue;
        }
        let odds = ODDS.get(spy * 5 + level).copied().unwrap_or(0);
        if rng.range(stream::PLANET, 1, odds / 6) == 1 {
            planet.spies[e] = 0;
            events.push(ColonyEvent::SpyExpelled { empire: e as u8 });
        }
    }
}

/// One planet's part of the economy step (`FRISSIT_Planet(true, 0)`), spec §3–§9 and §11.
/// `index` is the planet's 1-based number; `monthly` is `Nap == 1`. A rebellion is only reported
/// ([`ColonyEvent::Rebellion`]); [`crate::colony_side`] runs it between [`economy_head`] and [`economy_tail`].
pub fn economy_planet(
    index: i32,
    planet: &mut Planet,
    rules: &ColonyRules,
    ctx: &mut EconomyContext,
    rng: &mut Streams,
    monthly: bool,
) -> PlanetReport {
    let (fx, report) = economy_head(planet, rules, ctx);
    economy_tail(index, planet, &fx, report, rules, ctx, rng, monthly)
}

/// The building pass and the unrest counter (spec §4, §11): everything before a rebellion.
pub fn economy_head(planet: &mut Planet, rules: &ColonyRules, ctx: &EconomyContext) -> (Effects, PlanetReport) {
    let mut report = PlanetReport::default();
    let human = ctx.human_controlled[planet.owner as usize];
    let fx = building_pass(planet, rules, human);

    let can_rebel = planet.owner < 9 && !planet.loyal;
    if can_rebel && (ctx.multiplayer || !planet.protected) {
        let m = planet.morale;
        if m < 100 {
            planet.unrest += 1;
        }
        if m < 200 {
            planet.unrest += 1;
        }
        if m > 299 {
            planet.unrest -= 1;
        }
        if m > 399 {
            planet.unrest = 0;
        }
        if m < 200 {
            if planet.unrest < 30 && planet.unrest % 5 == 0 && planet.owner == ctx.human_empire {
                report.events.push(ColonyEvent::UnrestWarning);
            }
            if planet.unrest > 30 {
                report.events.push(ColonyEvent::Rebellion);
            }
        }
    }
    (fx, report)
}

/// The rest of the planet's step, after any rebellion: construction, problems, morale, growth, taxes,
/// the autobuild counter, spy expulsion, production capacity and the abandonment check.
/// `fx` is the head's building pass (the original keeps using it after a rebellion).
#[allow(clippy::too_many_arguments)]
pub fn economy_tail(
    index: i32,
    planet: &mut Planet,
    fx: &Effects,
    mut report: PlanetReport,
    rules: &ColonyRules,
    ctx: &mut EconomyContext,
    rng: &mut Streams,
    monthly: bool,
) -> PlanetReport {
    let progress = (fx.effect[kind::COLONY_HUB] * 3 + 1) * 6 * 50;
    for slot in 0..planet.buildings.len() {
        let b = &mut planet.buildings[slot];
        if b.kind == 0 || b.work_left == 0 {
            continue;
        }
        b.work_left = (b.work_left as i32 - progress).max(0) as u16;
        if b.work_left == 0 {
            report.events.push(ColonyEvent::BuildingFinished { slot });
            refresh(planet, rules, ctx);
        }
    }

    let targets = problem_targets(index, planet, fx, rules, ctx, rng, &mut report.events);
    report.needed_morale = smooth_and_morale(planet, &targets, rules, ctx);
    report.targets = targets.map(|t| t * 10000);

    if monthly {
        grow(planet, fx, rules, ctx, rng);
    }

    if ctx.day % 6 == 1 {
        let mut gdp = planet.population / 500;
        if planet.has_event(event::GDP_BONUS) {
            gdp = (gdp as f64 * 1.1) as i32;
        }
        if !planet.has_event(event::NO_TAXES) {
            planet.gdp_acc += gdp;
            planet.tax_acc += gdp * planet.tax_level as i32 * 5;
        }
        if monthly {
            report.gdp = planet.gdp_acc;
            report.tax_paid = planet.tax_acc;
            planet.gdp_last = planet.gdp_acc;
            planet.tax_last = planet.tax_acc;
            planet.gdp_acc = 0;
            planet.tax_acc = 0;
        }
    }
    planet.autobuild_counter = (planet.autobuild_counter + 1).min(100);

    hunt_spies(planet, fx, ctx, rng, &mut report.events);
    production_capacity(planet, fx, rules, ctx);

    if fx.occupied == 0 && !planet.protected {
        report.events.push(ColonyEvent::Abandoned);
    }
    report
}

/// Spec §3 prologue: the global morale modifier decays toward 0 (on every `only == 0` call).
pub fn decay_global_morale(ctx: &mut EconomyContext) {
    let (g, s) = (ctx.global_morale, ctx.global_morale_speed);
    ctx.global_morale = if g > 0 { (g - s).max(0) } else if g < 0 { (g + s).min(0) } else { 0 };
}

/// Monthly totals per empire (`FullGDP`, `FullTax`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EmpireTotals {
    pub gdp: [i32; 19],
    pub tax: [i32; 19],
}

/// The whole economy step over `planets[1..]` (index 0 unused, like the original).
/// Unowned planets are skipped. Returns per-planet reports (index-aligned) and the monthly totals.
pub fn economy_step(
    planets: &mut [Planet],
    rules: &ColonyRules,
    ctx: &mut EconomyContext,
    rng: &mut Streams,
) -> (Vec<PlanetReport>, Option<EmpireTotals>) {
    let monthly = ctx.day == 1;
    decay_global_morale(ctx);
    let mut totals = monthly.then(EmpireTotals::default);
    let mut reports = vec![PlanetReport::default(); planets.len()];
    for (i, planet) in planets.iter_mut().enumerate().skip(1) {
        if planet.owner == 0 {
            continue;
        }
        let r = economy_planet(i as i32, planet, rules, ctx, rng, monthly);
        if let Some(t) = totals.as_mut() {
            t.gdp[planet.owner as usize] += r.gdp;
            t.tax[planet.owner as usize] += r.tax_paid;
        }
        reports[i] = r;
    }
    (reports, totals)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> ColonyRules {
        // Synthetic tables (not the game's values): simple, round numbers.
        let mut buildings = vec![BuildingKind::default(); 56];
        buildings[1] = BuildingKind { priority: 0, max_hp: 100, energy: 0, workers: 10, output: 5, ..Default::default() };
        buildings[6] = BuildingKind { priority: 2, max_hp: 100, energy: 100, workers: 30, output: 0, ..Default::default() };
        buildings[19] = BuildingKind { priority: 5, max_hp: 100, energy: 200, workers: 40, output: 10, scaled_effect: true, ..Default::default() };
        buildings[52] = BuildingKind { priority: 0, max_hp: 100, energy: 0, workers: 10, output: 4, scaled_effect: true, ..Default::default() };
        let curve = Curve { a: 10, b: 2, c: 100, d: 100, rate: 10, weight: 100 };
        ColonyRules {
            buildings,
            curves: vec![curve; 20],
            surface_growth: vec![[100; 9]; 15],
            self_destruct_fear: vec![40; 15],
            kidnap_morale: vec![0; 6],
            race_production: vec![1.0; 15],
        }
    }

    fn b(kind: u8) -> Building {
        Building { kind, yield_pct: 100, efficiency: 256, ..Default::default() }
    }

    #[test]
    fn curve_shape() {
        let c = Curve { a: 10, b: 2, c: 100, d: 100, rate: 0, weight: 0 };
        assert_eq!(c.eval(1.0, 0.0), 100, "no capacity = maximum");
        assert_eq!(c.eval(100.0, 100.0), 10, "at the threshold the value is a");
        assert!(c.eval(50.0, 100.0) < 10 && c.eval(200.0, 100.0) > 10);
    }

    #[test]
    fn workers_are_cut_from_the_highest_class_first() {
        let r = rules();
        let mut p = Planet { owner: 1, population: 50, buildings: vec![b(1), b(52), b(19), b(6)], ..Default::default() };
        let fx = building_pass(&mut p, &r, false);
        assert_eq!(p.workers_demand, 90);
        // 50 people: hub (10) + plant (10) + hospital (30); the factory (class 5) gets nobody.
        assert_eq!(p.buildings[2].flags & flag::NO_WORKERS, flag::NO_WORKERS);
        assert!(p.buildings[3].working());
        // Energy: hub 5×100 + plant 4×100 = 900 produced; hospital needs 100.
        assert_eq!((p.energy_produced, p.energy_demand), (900, 300));
        assert_eq!(fx.effect[19], 0);
        assert_eq!(fx.effect[1], 1);
    }

    #[test]
    fn energy_shortage_stops_buildings_and_damage_reduces_output() {
        let r = rules();
        let mut plant = b(52);
        plant.damage = 50; // half health: 4 × 100 × 50 / 100 = 200
        let mut p = Planet { owner: 1, population: 1000, buildings: vec![plant, b(19), b(6)], ..Default::default() };
        building_pass(&mut p, &r, false);
        assert_eq!(p.energy_produced, 200);
        // Demand 300; cut from class 6 down: the factory (class 5, 200) loses its energy first.
        assert_eq!(p.buildings[1].flags & flag::NO_ENERGY, flag::NO_ENERGY);
        assert!(p.buildings[2].working());
    }

    #[test]
    fn taxes_accrue_every_sixth_day_and_pay_monthly() {
        let r = rules();
        let mut rng = Streams::default();
        let mut ctx = EconomyContext { day: 7, players: 2, ..Default::default() };
        let mut p = Planet { owner: 1, population: 10_000, tax_level: 3, buildings: vec![b(1)], ..Default::default() };
        economy_planet(1, &mut p, &r, &mut ctx, &mut rng, false);
        assert_eq!((p.gdp_acc, p.tax_acc), (20, 300));
        ctx.day = 1;
        let rep = economy_planet(1, &mut p, &r, &mut ctx, &mut rng, true);
        // Day 1: growth runs first, then the day's accrual, then the payout of the whole month.
        let g = p.population / 500;
        assert_eq!((rep.gdp, rep.tax_paid), (20 + g, 300 + g * 15));
        assert_eq!((p.gdp_acc, p.tax_acc, p.tax_last), (0, 0, 300 + g * 15));
    }

    #[test]
    fn construction_finishes_and_refreshes() {
        let r = rules();
        let mut rng = Streams::default();
        let mut ctx = EconomyContext { day: 13, ..Default::default() };
        let mut site = b(6);
        site.work_left = 1000; // one hub: (3 + 1) × 300 = 1200 per step
        let mut p = Planet { owner: 1, population: 5000, buildings: vec![b(1), b(52), site], ..Default::default() };
        let rep = economy_planet(1, &mut p, &r, &mut ctx, &mut rng, false);
        assert_eq!(p.buildings[2].work_left, 0);
        assert!(p.buildings[2].working(), "the refresh switched it on");
        assert!(rep.events.contains(&ColonyEvent::BuildingFinished { slot: 2 }));
    }

    #[test]
    fn global_morale_decays_toward_zero() {
        let mut ctx = EconomyContext { global_morale: 25, global_morale_speed: 10, ..Default::default() };
        decay_global_morale(&mut ctx);
        assert_eq!(ctx.global_morale, 15);
        ctx.global_morale = -5;
        decay_global_morale(&mut ctx);
        assert_eq!(ctx.global_morale, 0);
    }
}
