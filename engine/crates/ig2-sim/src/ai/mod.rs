//! The AI empire (`FRISSIT_MI`, exe 0x50a6e0). Spec: `re/rules/ai-empire.md`.
//!
//! Part 1 (this file): the daily driver, the empire bookkeeping (`FRISSIT_MI_SZAMOLAS` 0x50dd00), the AI's colony
//! settings and money sliders, the income shares (`MoneyTeruletSzorzoCalculate` 0x540280), the production choices
//! (`FRISSIT_MI_GYARTAS` 0x50c6d0) and the spy missions (`FRISSIT_MI_KEMKEDES` 0x50d0d0). Part 2: colonisation
//! ([`colonise`], `FRISSIT_MI_KOLONIZALAS` 0x50d9a0), fleets ([`fleets`], `FRISSIT_MI_FLOTTAK` 0x50b1c0), the AI's
//! designs ([`design`], `FRISSIT_MI_DESIGN` 0x50abf0) and the new-game set-up; [`units`] holds the fleet, ship and
//! vehicle bookkeeping they share. The research pick is `research::ai_pick`.
//!
//! The AI draws from seeded stream 10 and, through the espionage helpers, stream 4; no libc `rand()`.
//!
//! The AI works on the other ports' views of the game state ([`World`]): the colony world, ships, vehicles,
//! designs and fleet totals (`spacewar::CombatWorld`), the fleet records and waypoints (`fleet::Starmap`), the spy
//! rosters and diplomacy's danger state, contact ages, relations, power and planet counts. Effects owned by other
//! systems are [`AiEvent`]s; the war declaration goes through [`AiHooks`].

pub mod colonise;
pub mod design;
pub mod fleets;
pub mod units;

use crate::colony::{production_base, Planet};
use crate::colony_side::{Treasury, EMPIRES};
use crate::fleet::{Starmap, StarmapContext};
use crate::research::{self, ResearchEvent};
use crate::rng::stream;
use crate::spacewar::{CombatEvent, DesignRules, NO_DESIGN};
use crate::spy::{self, MissionRule, Spies, ROSTERS};

/// The AI stream (`rndlog(10, …)`, `RndValasztas(10, …)`).
pub const STREAM: usize = stream::AI;
/// Unit categories 0..=10 (`MiGyarthatoCuccok[e][c]`; 1..=10 used).
pub const CATEGORIES: usize = 11;
/// Design slots per empire (`MiTervMutato[e][s]`).
pub const DESIGN_SLOTS: usize = 25;
/// Size of a category record.
pub const CATEGORY_SIZE: usize = 0x24;
/// `MiTervMutato` (best) and `MiTervMutatoSpec` (special): index into [`AiState::designs`].
pub const BEST: usize = 0;
pub const SPECIAL: usize = 1;

/// Categories with a fixed role in `FRISSIT_MI_GYARTAS`.
pub mod category {
    /// Colony ships: wanted while there are planets to colonise.
    pub const COLONY_SHIP: usize = 3;
    /// The default vehicle.
    pub const VEHICLE: usize = 6;
    /// The default warship and its alternative for the second queue slot.
    pub const SHIP: usize = 8;
    pub const SHIP_ALT: usize = 10;
}

/// Fleet `+0x52` bits used here.
const FLAG_NOT_MAIN: u16 = 0x18;
const FLAG_20: u16 = 0x20;
const FLAG_MAIN: u16 = 0x800;
/// Techs a mission looks at (1..=192).
const LAST_TECH: usize = 192;
/// Spy states and missions (`espionage.md` §1.1).
const AT_HOME: i16 = spy::state::HOME;
const ON_MISSION: i16 = spy::state::MISSION;
const COUNTER: u16 = spy::mission::COUNTER_INTELLIGENCE as u16;
/// Mission weight by relation level (war … alliance), `FRISSIT_MI_KEMKEDES`.
const LEVEL_WEIGHT: [i32; 6] = [0, 100, 40, 25, 15, 8];

/// `MiGyarthatoCuccok[e][c]` (0x24 bytes; every byte round-trips).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Category {
    pub w0: i16,
    /// `+0x02` / `+0x04`: the best and the special design of the category.
    pub design: [i16; 2],
    pub b6: [u8; 6],
    /// `+0x0c` / `+0x10`: units of those designs in service.
    pub units: [i32; 2],
    /// `+0x14`: how many the main planet should build (categories with mode 1).
    pub want: i32,
    pub w18: i32,
    /// `+0x1c` / `+0x20`: queue entries of those designs.
    pub queued: [i32; 2],
}

fn rd_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}
fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl Category {
    pub fn from_bytes(b: &[u8]) -> Self {
        Self {
            w0: rd_i16(b, 0),
            design: [rd_i16(b, 2), rd_i16(b, 4)],
            b6: b[6..12].try_into().unwrap(),
            units: [rd_i32(b, 0xc), rd_i32(b, 0x10)],
            want: rd_i32(b, 0x14),
            w18: rd_i32(b, 0x18),
            queued: [rd_i32(b, 0x1c), rd_i32(b, 0x20)],
        }
    }

    pub fn to_bytes(&self) -> [u8; CATEGORY_SIZE] {
        let mut b = [0; CATEGORY_SIZE];
        b[0..2].copy_from_slice(&self.w0.to_le_bytes());
        b[2..4].copy_from_slice(&self.design[0].to_le_bytes());
        b[4..6].copy_from_slice(&self.design[1].to_le_bytes());
        b[6..12].copy_from_slice(&self.b6);
        for (o, x) in [(0xc, self.units[0]), (0x10, self.units[1]), (0x14, self.want), (0x18, self.w18), (0x1c, self.queued[0]), (0x20, self.queued[1])] {
            b[o..o + 4].copy_from_slice(&x.to_le_bytes());
        }
        b
    }

    /// In service plus queued, of the best (`j` 0) or the special design (`j` 1).
    fn total(&self, j: usize) -> i32 {
        self.units[j].wrapping_add(self.queued[j])
    }
}

/// Scouting grid cells per side (`MiFelderitesRacs[e][21][21]`, cells 1..=20 used).
pub const GRID: usize = 21;

/// State owned by the AI (spec §1.1, §8.1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiState {
    /// `MiFoBolygo`, `MiFoFlotta`.
    pub main_planet: [i32; EMPIRES],
    pub main_fleet: [i32; EMPIRES],
    /// `MiBirodalomKozeppontX/Y`.
    pub centre_x: [i32; EMPIRES],
    pub centre_y: [i32; EMPIRES],
    /// `[BEST]` = `MiTervMutato`, `[SPECIAL]` = `MiTervMutatoSpec` (set by `FRISSIT_MI_DESIGN`).
    pub designs: [[[i16; DESIGN_SLOTS]; EMPIRES]; 2],
    /// `MiGyarthatoCuccok`.
    pub categories: [[Category; CATEGORIES]; EMPIRES],
    /// `VanUresBolygo` (set by `FRISSIT_MI_KOLONIZALAS`).
    pub empty_planets: [u8; EMPIRES],
    /// `MoneyTeruletCsuszka`: income sliders, pools 0..=5.
    pub sliders: [[f64; 6]; EMPIRES],
    /// `MiUtolsoSzarKolonizalas`: the planet the last fleeing colony fleet was heading for.
    pub last_bad_colony: [i32; EMPIRES],
    /// `MiMennyiIdeigNemPakolhatFelTankot`, `MiMennyiIdeigTartATankPakoloFazis`,
    /// `MiMennyiIdeigNemLehetTervUpgrade`: day counters.
    pub no_load: [i32; EMPIRES],
    pub load_phase: [i32; EMPIRES],
    pub no_upgrade: [i32; EMPIRES],
    /// `MiFelderitesRacs[e][x][y]`: 0 unknown, 1 to scout, 2 known.
    pub scout: [[[u8; GRID]; GRID]; EMPIRES],
}

/// A pod type (`PodTipusok`): its id and the 16 techs that fit it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PodRule {
    pub id: i32,
    pub techs: [i32; 16],
}

/// Rule tables (from the user's exe, ADR 0004; spec §1.2, §8.2).
#[derive(Debug, Clone, Default)]
pub struct AiRules {
    /// `MiGyartasMod` / `MiGyartasMitMin` / `MiGyartasMitMax`, categories 0..=10.
    pub build_mode: Vec<u8>,
    pub build_min: Vec<u8>,
    pub build_max: Vec<u8>,
    /// `FajMiBolygokHanyadReszeGyartas[race]`: % of the planets that get a production mode.
    pub production_share: Vec<i32>,
    /// `FajMiBolygokMilyenKatonaiAutiBuild[race]`: the military autobuild mode.
    pub military_mode: Vec<i32>,
    /// `TrainingKuldetesAr[level]`, `KemKuldetesPenzSzorzoFajonkent[race]`.
    pub training_cost: Vec<i32>,
    pub mission_cost_mult: Vec<f32>,
    /// What `TervUjraSzamol` reads (hulls `Alapok`, tech prices and stats, pod slot types).
    pub design: DesignRules,
    /// `PodFajok[table][hull][slot].max_count` (`GetMaxRarakhato`).
    pub pod_slot_max: Vec<Vec<Vec<i32>>>,
    /// `PodTipusok`.
    pub pods: Vec<PodRule>,
    /// `Talalmanyok2[tech][race]`: the special variant's tech priority on ground hulls.
    pub ground_priority: Vec<u8>,
}

/// Global inputs besides the espionage context (`spy::SpyContext`: alive, human, race, `EN`, `ProgramPlayers`,
/// MP, difficulty, day).
#[derive(Debug, Clone, Default)]
pub struct AiContext {
    /// `BirodalomMIzik[e]`: the AI plays this empire.
    pub ai: [bool; EMPIRES],
    /// `ProgramPlayers2`.
    pub program_players2: usize,
    /// `SpyEngedelyezve`.
    pub spies_enabled: bool,
    /// `StoryAdat+0xac != 0`: the story halves the resale of the human's story designs.
    pub story_resale: bool,
    /// `BirodalomTamadhat[e]`: may attack.
    pub may_attack: [bool; EMPIRES],
    /// `StoryAdat+0xb0 != 0`: no war declarations (SP).
    pub story_no_war: bool,
    /// `StoryAdat+0xb8` / `+0xbc != 0`: protect empire 1 / 5 unless it is among the two strongest (SP).
    pub story_guard: [bool; 2],
    /// `StoryAdat+0xd0 + 4e != 0`: `e` must not pick the human as a target.
    pub spare_human: [bool; EMPIRES],
    /// `Ev`, `Honap`, `Ido`.
    pub year: i32,
    pub month: i32,
    pub tick: i32,
    /// exe 0xB77244: the AI is switched off (no design either).
    pub ai_off: bool,
    /// `DesignBarmiRarakhato` (exe 0xA3F0B0): techs 0xa1..=0xa9 fit every slot.
    pub design_debug: bool,
}

/// Effects for the UI or other systems.
#[derive(Debug, Clone, PartialEq)]
pub enum AiEvent {
    /// The new main fleet is renamed `sprintf(MiFoFlottaNev, Faj_Nev[race])` (localised text).
    MainFleetNamed { empire: u8, fleet: usize, race: u8 },
    /// A ship was sold on day 20 (its resale price was credited, `HajoDelete` ran).
    ShipSold { empire: u8, ship: usize, design: i16, price: i32 },
    /// A new design's name: `DesignPoenosanElnevez(empire, slot, special)` (localised text, SP).
    DesignNamed { empire: u8, design: usize, slot: usize, special: bool },
    /// `TutorialCheckEvent(id)`.
    Tutorial { id: i32 },
    /// Vehicles entered (`count` > 0) or left a planet's ground-war map (`AddOwnTankToPlanet`,
    /// `RemoveTankFromPlanet`, `WF::RearrangeTankGroups`).
    GarrisonMap { planet: usize, design: i16, count: i32 },
    /// MP: the human's emptied fleet is reported over the network (`Multi_Send(0x1b)`).
    HumanFleetEmptied { fleet: usize },
    Combat(CombatEvent),
    Research(ResearchEvent),
}

/// Effects the AI hands to other systems synchronously.
pub trait AiHooks {
    /// `ElkuldHaduzenet(a, b)` (`diplomacy.md` §5): the declaration and its answer, at once. The relations
    /// (`w.spy.relations`) and contacts (`w.spy.colony.knows`) must reflect the outcome before this returns: the
    /// fleet pass reads them right after.
    fn declare_war(&mut self, _w: &mut World<'_>, _a: usize, _b: usize) {}
    /// The bytes of a fleet slot appended at index `f` before `FlottaNew` writes it (the original reuses
    /// whatever the grown array held there).
    fn fresh_fleet(&mut self, _f: usize) -> [u8; 0x80] {
        [0; 0x80]
    }
    /// The bytes of design record `d` appended to `e`'s table by `TervNew` (uninitialised in the original).
    fn fresh_design(&mut self, _e: usize, _d: usize) -> [u8; 0x84] {
        [0; 0x84]
    }
}

/// No other systems (unit tests): a declaration does nothing.
impl AiHooks for () {}

/// The world the AI works on.
pub struct World<'a> {
    pub ctx: &'a AiContext,
    pub rules: &'a AiRules,
    /// Colony world (planets, money, techs, contacts, lab and spy-centre counts), ships, vehicles, designs and
    /// fleet totals, research, relations, the kidnap table, the streams, the espionage rules and context.
    pub spy: spy::World<'a>,
    pub spies: &'a mut Spies,
    /// The fleet records (`Flottak[0..=n]`, the same slots as `spy.combat.fleets`), waypoints and radar tables.
    /// Its planet view and pools are not used: the colony world's planets and `spy.combat`'s pools are.
    pub starmap: &'a mut Starmap,
    /// `PlayerKi`, `AlapFlottaNev` for orders and new fleets.
    pub star_ctx: &'a StarmapContext,
    /// `OsszHanySajatFlotta`: fleet name counter.
    pub fleet_numbers: &'a mut [i32; EMPIRES],
    /// `MiVeszelyAllapot`, `BirodalomEro`, `BirodalomHanyBolygo` (diplomacy).
    pub danger: &'a [i32; EMPIRES],
    pub power: &'a [f32; EMPIRES],
    pub planet_count: &'a [i32; EMPIRES],
    /// `BirodalomMiotaIsmeri` (diplomacy).
    pub known_since: &'a mut [[i32; EMPIRES]; EMPIRES],
    pub events: Vec<AiEvent>,
}

impl World<'_> {
    fn alive(&self, e: usize) -> bool {
        self.spy.ctx.alive.get(e).copied().unwrap_or(false)
    }
    fn human(&self, e: usize) -> bool {
        self.spy.ctx.human.get(e).copied().unwrap_or(false)
    }
    fn race(&self, e: usize) -> u8 {
        self.spy.ctx.race.get(e).copied().unwrap_or(0)
    }
    fn players2(&self) -> usize {
        self.ctx.program_players2.min(EMPIRES - 1)
    }
    fn owned(&self, e: usize) -> Vec<usize> {
        let pl = &self.spy.colony.planets;
        (1..pl.len()).filter(|&p| pl[p].owner as usize == e).collect()
    }
    /// The owner of fleet `f` (`Flottak[f].+0x15`; slot 0 and deleted fleets included).
    fn fleet_owner(&self, f: i16) -> u8 {
        self.spy.combat.fleets.get(f.max(0) as usize).map_or(0xff, |x| x.owner)
    }
    fn design(&self, e: usize, d: i32) -> Option<&[u8; 0x84]> {
        self.spy.combat.designs.get(e)?.get(usize::try_from(d).ok()?).map(|x| &x.raw)
    }
    fn design_i32(&self, e: usize, d: i32, o: usize) -> i32 {
        self.design(e, d).map_or(0, |r| rd_i32(r, o))
    }
    fn mission(&self, m: usize) -> MissionRule {
        self.spy.rules.missions.get(m).copied().unwrap_or_default()
    }
    fn r(&mut self, lo: i32, hi: i32) -> i32 {
        self.spy.rng.range(STREAM, lo, hi)
    }
}

/// SSE `cvttsd2si`: truncation, out of range or NaN → `0x80000000`.
fn cvtt(x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 { i32::MIN } else { x as i32 }
}

/// Wealth class 1..=5 of a planet (spec §3.2).
fn wealth_class(g: i32) -> usize {
    match g {
        ..75 => 1,
        75..100 => 2,
        100..125 => 3,
        125..150 => 4,
        _ => 5,
    }
}

/// An i16 word of a planet record at `off` (the queue counts read `+0x78 + 8·(class − 1)`, which leaves the
/// queue for classes other than 1 and 2). Outside `+0x70..+0xb8` the port reads 0.
fn planet_word(p: &Planet, off: i32) -> i16 {
    match off {
        0x70..=0x76 => {
            let mut b = [0u8; 8];
            b[..4].copy_from_slice(&p.ship_capacity.to_le_bytes());
            b[4..].copy_from_slice(&p.vehicle_capacity.to_le_bytes());
            let o = (off - 0x70) as usize;
            i16::from_le_bytes([b[o], b[o + 1]])
        }
        0x78..=0xb6 => rd_i16(&p.production, (off - 0x78) as usize),
        _ => 0,
    }
}

/// Write into the production queue bytes (`+0x78..+0xb8`) at planet offset `off`.
fn queue_put(p: &mut Planet, off: usize, v: &[u8]) {
    p.production[off - 0x78..off - 0x78 + v.len()].copy_from_slice(v);
}

impl AiState {
    /// `FRISSIT_MI`: the daily pass (spec §2–§4, §9, §11). `hooks` carries out war declarations.
    pub fn daily(&mut self, w: &mut World, hooks: &mut dyn AiHooks) {
        self.bookkeeping(w);
        for e in 1..=w.players2() {
            if !w.alive(e) || w.human(e) || !w.ctx.ai[e] {
                continue;
            }
            self.planet_settings(w, e);
            self.production(w, e);
            if e as i32 <= w.spy.ctx.program_players && w.ctx.spies_enabled {
                spy_missions(w, e);
            }
            self.colonise(w, e, hooks);
            self.fleets(w, e, hooks);
            research_pick(w, e);
            self.set_sliders(e, w.danger[e]);
        }
        self.income_shares(&mut w.spy.colony.money);
    }

    /// The per-empire reset `INIT_MI` and `INIT_MI_KIESETT` share (§13): no colony targets, no main fleet or
    /// planet, counters and centre 0, no design slots.
    fn reset_empire(&mut self, e: usize) {
        self.empty_planets[e] = 0;
        self.main_fleet[e] = 0;
        self.main_planet[e] = 0;
        self.last_bad_colony[e] = 0;
        self.no_load[e] = 0;
        self.load_phase[e] = 0;
        self.no_upgrade[e] = 0;
        self.centre_x[e] = 0;
        self.centre_y[e] = 0;
        for s in 1..DESIGN_SLOTS {
            self.designs[BEST][e][s] = 0;
            self.designs[SPECIAL][e][s] = 0;
        }
    }

    /// `INIT_MI` at a new game (spec §13), the AI's share: every empire 1..=`ProgramPlayers2` is reset (with its
    /// scouting grid) and the AI empires get their designs; then the categories are cleared. Diplomacy does the
    /// rest (alive flags, the countdown, danger 1, the relations' reset, `FRISSIT_Viszony`, empires without
    /// planets dying), between the designs and the categories.
    pub fn init(&mut self, w: &mut World, hooks: &mut dyn AiHooks) {
        for e in 1..=w.players2() {
            self.reset_empire(e);
            self.scout[e] = [[0; GRID]; GRID];
            if w.ctx.ai[e] {
                self.design(w, e, true, hooks);
            }
        }
        for e in 1..=w.players2() {
            for c in 1..CATEGORIES {
                let cat = &mut self.categories[e][c];
                (cat.units, cat.queued, cat.design, cat.want) = ([0; 2], [0; 2], [0; 2], 0);
            }
        }
    }

    /// `INIT_MI_KIESETT(e)`: an empire handed to the AI mid-game (alive, not human-controlled) is reset, its
    /// designs wait 720 days for an upgrade (danger 1 is diplomacy's), and the AI designs for it (spec §13).
    pub fn init_dropped(&mut self, w: &mut World, e: usize, hooks: &mut dyn AiHooks) {
        if !w.alive(e) || w.star_ctx.human_controlled.get(e).copied().unwrap_or(false) {
            return;
        }
        self.reset_empire(e);
        self.no_upgrade[e] = 720;
        if w.ctx.ai[e] {
            self.design(w, e, true, hooks);
        }
    }

    /// `FRISSIT_MI_SZAMOLAS` (spec §2): centres, main fleets, contact ages, for every living empire.
    pub fn bookkeeping(&mut self, w: &mut World) {
        let n = w.players2();
        for e in 1..=n {
            if !w.alive(e) {
                continue;
            }
            let (mut sx, mut sy, mut count) = (0f32, 0f32, 0);
            for p in w.owned(e) {
                let pl = &w.spy.colony.planets[p];
                sx += pl.x;
                sy += pl.y;
                count += 1;
            }
            if count > 0 {
                self.centre_x[e] = cvtt((sx / count as f32) as f64);
                self.centre_y[e] = cvtt((sy / count as f32) as f64);
            } else {
                self.centre_x[e] = 0;
                self.centre_y[e] = 0;
            }

            let (mut best, mut top) = (0, -1);
            for f in 1..w.starmap.fleets.len() {
                let fl = &w.starmap.fleets[f];
                if fl.owner as usize == e && fl.flags & FLAG_NOT_MAIN == 0 && fl.story_id == 0 && top < fl.strength {
                    top = fl.strength;
                    best = f;
                }
            }
            let human = w.human(e);
            let old = self.main_fleet[e];
            if !human && old > 0 {
                if let Some(fl) = w.starmap.fleets.get_mut(old as usize).filter(|fl| fl.owner as usize == e) {
                    fl.flags &= !FLAG_MAIN;
                    w.mirror(old as usize);
                }
            }
            self.main_fleet[e] = best as i32;
            if let Some(fl) = w.starmap.fleets.get_mut(best) {
                fl.flags &= !FLAG_20;
                w.mirror(best);
            }
            if best > 0 && !human {
                w.starmap.fleets[best].flags |= FLAG_MAIN;
                w.mirror(best);
                let race = w.race(e);
                w.events.push(AiEvent::MainFleetNamed { empire: e as u8, fleet: best, race });
            }

            for i in 1..=n {
                if w.alive(i) && w.spy.colony.knows[e][i] != 0 {
                    w.known_since[e][i] = w.known_since[e][i].wrapping_sub(1);
                }
            }
        }
    }

    /// Spec §3.1–§3.3: the main planet and every planet's autobuild mode, aggressiveness and tax level.
    pub fn planet_settings(&mut self, w: &mut World, e: usize) {
        let mine = w.owned(e);
        let planets = &w.spy.colony.planets;
        let mut pick = 0;
        for &p in &mine {
            if pick == 0 || planets[pick].ship_capacity == 0 {
                pick = p;
            }
        }
        let score = |p: &Planet| p.ship_capacity.wrapping_add(p.population / 20);
        let old = self.main_planet[e];
        let keep = usize::try_from(old).ok().filter(|&o| o > 0).and_then(|o| planets.get(o)).is_some_and(|o| {
            o.owner as usize == e && score(o).wrapping_add(250) >= score(&planets[pick]) && o.ship_capacity != 0
        });
        if !keep {
            self.main_planet[e] = pick as i32;
        }

        let colony_rules = &w.spy.side_rules.colony;
        let class = |p: &Planet| wealth_class(production_base(p, colony_rules, w.spy.econ));
        let race = w.race(e) as usize;
        let mut have = [0i32; 6];
        for &p in &mine {
            have[class(&planets[p])] += 1;
        }
        let mut quota = (mine.len() as i32).wrapping_mul(w.rules.production_share.get(race).copied().unwrap_or(0)) / 100;
        let mut take = [0i32; 6];
        for c in 1..=5 {
            let t = have[c].min(quota);
            quota -= t;
            take[c] += t;
        }
        let shipyards = mine.iter().filter(|&&p| planets[p].ship_capacity > 0).count();
        let factories = mine.iter().filter(|&&p| planets[p].vehicle_capacity > 0).count();
        let classes: Vec<usize> = mine.iter().map(|&p| class(&planets[p])).collect();

        let danger = w.danger[e];
        let labs = w.spy.colony.empires.lab_buildings[e];
        let military = w.rules.military_mode.get(race).copied().unwrap_or(0) as u8;
        let spy_centres = w.spy.colony.empires.spy_centres.get(e).copied();
        for (&p, &c) in mine.iter().zip(&classes) {
            let colony = &mut *w.spy.colony;
            let ground = colony.strength.ground.get(p).copied().unwrap_or(0);
            let no_centre = colony.empires.spy_centres_by_planet.get(e).is_some_and(|row| row[p] == 0);
            let pl = &mut colony.planets[p];
            pl.autobuild_mode = if take[c] < 1 { if shipyards > 0 && factories == 0 { 6 } else { 2 } } else { 3 };
            if labs[1] > 18 && labs[2] > 10 && labs[3] > 10 {
                pl.autobuild_mode = 2;
            }
            take[c] -= 1;
            pl.autobuild_aggr = if danger > 2 { 2 } else { 3 };
            pl.tax_level = if pl.unrest > 0 { 0 } else { 3 };
            let mut k = if danger > 1 { 9 } else { 6 };
            if p as i32 == self.main_planet[e] {
                k = (k + 2).min(9);
            }
            if ground < pl.population.wrapping_sub(10000).wrapping_mul(k) / 5000 {
                pl.autobuild_mode = military;
            }
            if spy_centres.is_some_and(|n| n < 12) && pl.population > 7000 && no_centre && race != 5 {
                pl.autobuild_mode = 7;
            }
        }
    }

    /// Spec §3.5: the AI's income sliders by danger state.
    pub fn set_sliders(&mut self, e: usize, danger: i32) {
        self.sliders[e] = match danger {
            1 => [0.0, 40.0, 30.0, 30.0, 10.0, 10.0],
            2 => [0.0, 50.0, 30.0, 20.0, 10.0, 10.0],
            _ => [0.0, 80.0, 10.0, 10.0, 10.0, 10.0],
        };
    }

    /// `MoneyTeruletSzorzoCalculate` (spec §4): every empire's income shares from its sliders. All-zero sliders
    /// give NaN shares and a long walk up in 0.01 steps, as in the original.
    pub fn income_shares(&self, money: &mut [Treasury; EMPIRES]) {
        for (s, m) in self.sliders.iter().zip(money.iter_mut()).skip(1) {
            let sh = &mut m.share;
            let sum = ((s[1] + 0.0) + s[2]) + s[3];
            for i in 1..=5 {
                sh[i] = cvtt(s[i] * 100.0 / sum) as f64 / 100.0;
            }
            let mut t;
            loop {
                t = ((sh[1] + 0.0) + sh[2]) + sh[3];
                if t.is_nan() || t >= 1.0 {
                    break;
                }
                if sh[1] > 0.0 {
                    sh[1] += 0.01;
                } else if sh[2] > 0.0 {
                    sh[2] += 0.01;
                } else {
                    sh[3] += 0.01;
                }
            }
            if t > 1.0 {
                sh[1] -= t - 1.0;
                if sh[1] < 0.0 {
                    sh[2] += sh[1];
                    sh[1] = 0.0;
                }
                if sh[2] < 0.0 {
                    sh[3] += sh[2];
                    sh[2] = 0.0;
                }
            }
        }
    }

    /// `FRISSIT_MI_GYARTAS(e)` (spec §5): unit counts, idle production lines, scrapping on day 20.
    pub fn production(&mut self, w: &mut World, e: usize) {
        let mine = w.owned(e);
        for c in 1..CATEGORIES {
            let rule = |v: &Vec<u8>| v.get(c).copied().unwrap_or(0) as usize;
            let (lo, hi, mode) = (rule(&w.rules.build_min), rule(&w.rules.build_max), rule(&w.rules.build_mode));
            let mut cat = self.categories[e][c];
            cat.units = [0; 2];
            cat.queued = [0; 2];
            cat.want = 0;
            for j in [BEST, SPECIAL] {
                cat.design[j] = (lo..=hi.min(DESIGN_SLOTS - 1)).fold(0, |d, s| if self.designs[j][e][s] > 0 { self.designs[j][e][s] } else { d });
            }
            if mode == 1 || mode == 2 {
                for j in [BEST, SPECIAL] {
                    let d = cat.design[j];
                    let mut queued = 0;
                    if d > 0 {
                        let row = w.design(e, d as i32).map_or(0, |r| rd_i16(r, 0x16) as i32) - 1;
                        for &p in &mine {
                            let pl = &w.spy.colony.planets[p];
                            queued += (0..4).filter(|k| planet_word(pl, 0x78 + 8 * row + 2 * k) == d).count() as i32;
                        }
                    }
                    cat.queued[j] = queued;
                    let cw = &w.spy.combat;
                    let mut units = (1..cw.ships.len()).filter(|&i| cw.ships[i].design == d && w.fleet_owner(cw.ships[i].fleet) as usize == e).count() as i32;
                    for v in cw.vehicles.iter().skip(1) {
                        if v.design != d {
                            continue;
                        }
                        let owner = if v.fleet < 1 {
                            w.spy.colony.planets.get(v.fleet.unsigned_abs() as usize).map_or(0, |p| p.owner)
                        } else {
                            w.fleet_owner(v.fleet)
                        };
                        if owner as usize == e {
                            units = units.wrapping_add(v.count);
                        }
                    }
                    cat.units[j] = units;
                }
            }
            self.categories[e][c] = cat;
        }
        let cats = &mut self.categories[e];
        cats[1].want = 0;
        cats[4].want = 0;
        cats[2].want = 0;
        cats[category::COLONY_SHIP].want = (self.empty_planets[e] != 0) as i32;

        let main = self.main_planet[e];
        let danger = w.danger[e];
        let cats = self.categories[e];
        for &p in &mine {
            queue_put(&mut w.spy.colony.planets[p], 0xac, &[if p as i32 == main { 0 } else { main as u8 }]);
            let mut ship = cats[category::SHIP].design[BEST];
            let alt_cat = &cats[category::SHIP_ALT];
            let mut alt = alt_cat.design[BEST];
            if alt_cat.total(SPECIAL) < alt_cat.total(BEST) && alt_cat.design[SPECIAL] > 0 {
                alt = alt_cat.design[SPECIAL];
            }
            let (idle, yard) = {
                let pl = &w.spy.colony.planets[p];
                (planet_word(pl, 0x78) == 0, pl.ship_capacity > 0)
            };
            if idle && yard {
                if (danger < 3 || w.r(1, 3) == 1) && p as i32 == main {
                    for (c, cat) in cats.iter().enumerate().skip(1) {
                        if w.rules.build_mode.get(c) == Some(&1) && cat.design[BEST] > 0 && cat.total(BEST) < cat.want {
                            ship = cat.design[BEST];
                            alt = 0;
                        }
                    }
                }
                let count = if w.design(e, ship as i32).is_some_and(|r| r[0x36] != 0) { 3 } else { 1 };
                let time = w.design_i32(e, ship as i32, 0x20);
                let pl = &mut w.spy.colony.planets[p];
                queue_put(pl, 0x78, &ship.to_le_bytes());
                for (o, v) in [(0x88, count), (0x90, 0), (0xad, 0), (0x89, 1), (0x91, 0), (0xae, 0), (0xa8, 0)] {
                    queue_put(pl, o, &[v]);
                }
                queue_put(pl, 0x7a, &alt.to_le_bytes());
                queue_put(pl, 0xa0, &time.to_le_bytes());
            }

            let veh_cat = &cats[category::VEHICLE];
            let mut veh = veh_cat.design[BEST];
            if veh_cat.total(SPECIAL) < veh_cat.total(BEST) && veh_cat.design[SPECIAL] > 0 {
                veh = veh_cat.design[SPECIAL];
            }
            let time = w.design_i32(e, veh as i32, 0x20);
            let pl = &mut w.spy.colony.planets[p];
            if planet_word(pl, 0x80) == 0 && pl.field_58 < 10 && pl.vehicle_capacity > 0 {
                queue_put(pl, 0x80, &veh.to_le_bytes());
                for (o, v) in [(0x8c, 1), (0x94, 0), (0xb1, 0), (0xa9, 0)] {
                    queue_put(pl, o, &[v]);
                }
                queue_put(pl, 0xa4, &time.to_le_bytes());
            }
        }

        if w.spy.ctx.day == 20 {
            scrap(w, e);
        }
    }
}

/// Spec §5.3: the bucket of a design's `+0x3c` (`(int)((f64)(f32)(x · 100) + 0.5)`, 0..=16).
fn scrap_bucket(r: &[u8; 0x84]) -> usize {
    let x = f32::from_le_bytes(r[0x3c..0x40].try_into().unwrap());
    cvtt((x * 100.0) as f64 + 0.5).clamp(0, 16) as usize
}

/// Ships that can be scrapped: none of the design flags `+0x34`, `+0x35`, `+0x37`, `+0x38`.
fn scrappable(r: &[u8; 0x84]) -> bool {
    [0x34, 0x35, 0x37, 0x38].iter().all(|&o| r[o] == 0)
}

/// Sell the obsolete warships of `e` (spec §5.3).
fn scrap(w: &mut World, e: usize) {
    let mut sum = [0i32; 17];
    let mut total = 0i32;
    for i in 1..w.spy.combat.ships.len() {
        let s = w.spy.combat.ships[i];
        if s.design == NO_DESIGN || w.fleet_owner(s.fleet) as usize != e {
            continue;
        }
        let Some(r) = w.design(e, s.design as i32).filter(|r| scrappable(r)) else { continue };
        let b = scrap_bucket(r);
        let p = rd_i32(r, 0x2c).wrapping_mul(rd_i32(r, 0x30));
        let weight = cvtt((p as f32 as f64).sqrt() as f32 as f64);
        sum[b] = sum[b].wrapping_add(weight);
        total = total.wrapping_add(weight);
    }
    let Some(a) = (1..=16).find(|&b| sum[b] > 0) else { return };
    if (sum[a] as f64) >= total as f64 * 0.1 {
        return;
    }
    let mut i = 1;
    while i < w.spy.combat.ships.len() {
        let s = w.spy.combat.ships[i];
        let story = w.starmap.fleets.get(s.fleet.max(0) as usize).map_or(0, |f| f.story_id);
        if s.design != NO_DESIGN && w.fleet_owner(s.fleet) as usize == e && story == 0 {
            if let Some(r) = w.design(e, s.design as i32).filter(|r| scrappable(r) && scrap_bucket(r) == a) {
                let price = resale(r, w.ctx, w.spy.ctx.human_empire);
                let sw = &mut w.spy;
                sw.colony.add_money(e, 0, price as f64, sw.side_rules, sw.econ);
                w.events.push(AiEvent::ShipSold { empire: e as u8, ship: i, design: s.design, price });
                w.delete_ship(i);
            }
        }
        i += 1;
    }
}

/// `CsokkentettTervAr` with `GetTervEladasOszto` (spec §5.3).
fn resale(r: &[u8; 0x84], ctx: &AiContext, human_empire: usize) -> i32 {
    let mut f: f32 = if r[0x36] != 0 { 10.0 } else { 2.0 };
    if ctx.story_resale && rd_i32(r, 0x44) == 5 && rd_i16(r, 0x40) as i32 == human_empire as i32 {
        f = 0.5;
    }
    let id = rd_i32(r, 0x80);
    if (0x6a5..3000).contains(&id) && ![0x8fc, 0x8fd, 0x906].contains(&id) && !(0x924..=0x937).contains(&id) {
        f = -1.0;
    }
    if f < 0.0 { 0 } else { cvtt((rd_i32(r, 0x24) as f32 / f) as f64) }
}

/// `FRISSIT_MI_FEJLESZTES(e)` (`production-research.md` §2.2).
pub fn research_pick(w: &mut World, e: usize) {
    let mut ev = Vec::new();
    let colony = &mut *w.spy.colony;
    research::ai_pick(&mut colony.tech, w.spy.research, &colony.empires.labs, e, w.spy.research_ctx, &mut ev);
    w.events.extend(ev.into_iter().map(AiEvent::Research));
}

// ------------------------------------------------------------------------------------------------
// Spy missions

/// `FRISSIT_MI_KEMKEDES(e)` (spec §6): skill points, firing, and new missions for the spies at home.
pub fn spy_missions(w: &mut World, e: usize) {
    if e >= ROSTERS {
        return;
    }
    let count = |w: &World| w.spies.count[e].clamp(0, spy::SLOTS as i32 - 1) as usize;
    // The roster up to the first empty slot.
    let prefix = |w: &World| (1..=count(w)).take_while(|&k| w.spies.roster[e][k].state > 0).collect::<Vec<_>>();
    let mut list = prefix(w);
    let n = list.len();
    let mut score = [0i32; spy::SLOTS];
    let mut sel = [false; spy::SLOTS];
    for &k in &list {
        let s = &w.spies.roster[e][k];
        score[k] = (s.skills[spy::skill::INFILTRATION] as i32 * 2000) / (s.skills[spy::skill::COUNTER] as i32 + 1);
        sel[k] = true;
    }
    if n as i32 > w.spy.colony.empires.spy_capacity[e] || n >= 12 {
        let (mut lo, mut lx) = (0, 99_999_999);
        for &k in &list {
            if w.spies.roster[e][k].xp < lx {
                lx = w.spies.roster[e][k].xp;
                lo = k;
            }
        }
        if lo > 0 {
            w.spies.roster[e][lo].state = 0;
        }
    }
    for i in 1..n {
        for j in (1..=i).rev() {
            if score[list[j - 1]] < score[list[j]] {
                list.swap(j - 1, j);
            }
        }
    }
    if n >= 2 {
        sel[list[n - 1]] = false;
    }
    if n >= 3 {
        sel[list[n - 2]] = false;
    }
    if n >= 7 {
        sel[list[n - 3]] = false;
    }
    let mut best = 0;
    for k in prefix(w) {
        let inf = |k: usize| w.spies.roster[e][k].skills[spy::skill::INFILTRATION];
        if sel[k] && (best == 0 || inf(best) < inf(k)) {
            best = k;
        }
    }

    let (mut target, mut p1, mut p2, mut busy) = (0usize, 0i16, 0i16, false);
    let mut k = 1;
    while k <= count(w) && w.spies.roster[e][k].state > 0 {
        let s = w.spies.roster[e][k];
        if s.state == AT_HOME || (s.state == ON_MISSION && s.mission == COUNTER) {
            let skills = if sel[k] { [0, 2] } else { [1, 2] };
            let sp = &mut w.spies.roster[e][k];
            while sp.points > 0 {
                let i = if sp.skills[skills[1]] < sp.skills[skills[0]] { skills[1] } else { skills[0] };
                sp.points -= 1;
                sp.skills[i] = sp.skills[i].wrapping_add(1);
            }
            let s = *sp;
            let mut m = 0;
            if s.xp < 2000 && ((sel[k] && k == best) || (!sel[k] && !busy)) {
                m = 1;
            }
            if k == best && w.r(1, 500 - 200 * w.spy.ctx.difficulty) <= 1 && w.spy.colony.money[e].pools[4] >= 0.0 {
                let mut pick = None;
                w.spy.rng.choice_begin();
                for t in 1..=w.spy.ctx.program_players.max(0) as usize {
                    if t == e || !w.alive(t) || w.spy.colony.knows[e][t] == 0 || w.race(t) == 5 {
                        continue;
                    }
                    for m2 in 3..=10 {
                        if w.mission(m2).min_rank > s.rank {
                            continue;
                        }
                        let g = best_parameter(w, e, m2, t) as i16;
                        (p1, p2) = if m2 == spy::mission::SABOTAGE { (0, g) } else { (g, 0) };
                        if !possible(w, e, m2, t, p1, p2) {
                            continue;
                        }
                        let level = w.spy.relations[e][t].level as usize;
                        let mut weight = LEVEL_WEIGHT.get(level).copied().unwrap_or(0).wrapping_mul(40 * chance(w, e, k, m2, t) / 100);
                        match m2 {
                            3 | 8 => weight = weight.wrapping_mul(2),
                            5 => weight /= 2,
                            _ => {}
                        }
                        if level == 5 {
                            weight = 0;
                        }
                        if w.spy.rng.choice_offer(STREAM, weight) {
                            pick = Some((t, m2, p1, p2));
                        }
                    }
                }
                if let Some((t, m2, a, b)) = pick {
                    (target, m, p1, p2) = (t, m2, a, b);
                    if m >= 2 {
                        let veto = match w.spy.relations[e][target].level {
                            5 => w.r(1, 100) < 75,
                            4 => w.r(1, 100) < 50,
                            3 => w.r(1, 100) < 25,
                            _ => false,
                        };
                        if veto {
                            m = 0;
                            w.spy.colony.money[e].pools[4] += -1000.0;
                        }
                    }
                }
            }
            if m == 0 && !sel[k] && s.mission != COUNTER {
                m = COUNTER as usize;
            }
            if m == 0 && sel[k] && s.mission == COUNTER {
                let sp = &mut w.spies.roster[e][k];
                sp.state = AT_HOME;
                sp.mission = 0;
            }
            if m > 0 && (1..=8).contains(&target) {
                if !sel[k] {
                    busy = true;
                }
                let rule = w.mission(m);
                let len = rule.days.wrapping_mul(50);
                let catch_at = w.r(len / 3, len - 30);
                let sp = &mut w.spies.roster[e][k];
                sp.state = ON_MISSION;
                sp.mission = m as u16;
                sp.target = target as u8;
                sp.param1 = p1;
                sp.param2 = p2;
                sp.length = len;
                sp.left = len;
                sp.catch_at = catch_at;
                if m != COUNTER as usize {
                    let cost = if m == spy::mission::TRAINING {
                        usize::try_from(p1).ok().and_then(|i| w.rules.training_cost.get(i)).copied().unwrap_or(0)
                    } else {
                        rule.cost
                    };
                    let mult = w.rules.mission_cost_mult.get(w.race(e) as usize).copied().unwrap_or(0.0);
                    w.spy.colony.money[e].pools[4] += -(mult * cost as f32) as f64;
                }
            }
        }
        k += 1;
    }
}

/// `KemEselyGetBest(e, k, m, t)` (spec §6.3): whether a best parameter exists.
fn best_parameter(w: &mut World, e: usize, m: usize, t: usize) -> bool {
    match m {
        spy::mission::TRAINING => true,
        spy::mission::SABOTAGE => {
            w.spy.rng.range(spy::STREAM, 1, 5);
            true
        }
        spy::mission::DESTROY_SHIP => {
            let cw = &w.spy.combat;
            (1..cw.ships.len()).any(|i| {
                let s = cw.ships[i];
                s.design != NO_DESIGN && w.fleet_owner(s.fleet) as usize == t && w.design_i32(t, s.design as i32, 0x48) > 0
            })
        }
        spy::mission::STEAL_TECH => {
            let tech = &w.spy.colony.tech;
            (1..=LAST_TECH).any(|x| tech.status(x, e) < 5 && tech.status(x, t) == 5)
        }
        spy::mission::REBELLION => w.owned(t).iter().any(|&p| w.spy.colony.planets[p].population > 0),
        _ => false,
    }
}

/// `KemEselyLehetseges(e, k, m, t, p1, p2)` (spec §6.4).
fn possible(w: &mut World, e: usize, m: usize, t: usize, p1: i16, p2: i16) -> bool {
    let rule = w.mission(m);
    let alive = !(rule.needs_target && !w.alive(t));
    if rule.kind == 1 && p1 > 0 && w.spy.colony.planets.get(p1 as usize).map_or(0, |p| p.owner) as usize != t {
        return false;
    }
    if !alive {
        return false;
    }
    let tech = &w.spy.colony.tech;
    match m {
        spy::mission::PLANET_REPORT => p1 != 0 || w.owned(t).iter().any(|&p| w.spy.colony.planets[p].visibility[e] == 0),
        spy::mission::TECH_REPORT => (1..=LAST_TECH).any(|x| tech.status(x, t) == 5 && !tech.seen(x, e, t)),
        spy::mission::STEAL_MONEY => {
            let a = w.spy.colony.money[t].available(0);
            a >= 100.0 && cvtt(a * 0.25 / 100.0 * 100.0) > 0
        }
        spy::mission::SABOTAGE => {
            let (p1, p2) = (p1 as i32, p2 as i32);
            [(p1, p2), (p1, 0), (0, p2), (0, 0)].into_iter().any(|(p, x)| spy::search_building(&mut w.spy, t, p, x).0 > 0)
        }
        spy::mission::DESTROY_SHIP => {
            let cw = &w.spy.combat;
            (1..cw.ships.len()).any(|i| cw.ships[i].design != NO_DESIGN && w.fleet_owner(cw.ships[i].fleet) as usize == t)
        }
        spy::mission::STEAL_TECH => {
            if p1 == 0 {
                (1..=LAST_TECH).any(|x| tech.status(x, e) < 5 && tech.status(x, t) == 5)
            } else {
                let x = p1 as usize;
                tech.status(x, e) < 5 && tech.status(x, t) == 5
            }
        }
        spy::mission::REBELLION => w.owned(t).iter().any(|&p| p1 == 0 || p == p1 as usize),
        spy::mission::ASSASSINATE => {
            t < ROSTERS && (1..=w.spies.count[t].clamp(0, spy::SLOTS as i32 - 1) as usize).any(|j| matches!(w.spies.roster[t][j].state, 1 | 2))
        }
        spy::mission::KIDNAP => w.spy.kidnap.get(t).copied().unwrap_or(0) == 0 && (w.spy.ctx.multiplayer || t != 2),
        _ => true,
    }
}

/// `KemEselySzamolas2(e, k, m, t)` (spec §6.5): the success chance the AI weighs.
fn chance(w: &World, e: usize, k: usize, m: usize, t: usize) -> i32 {
    let rule = w.mission(m);
    let mut c = spy::chance(&rule, w.spies.roster[e][k].skills[spy::skill::INFILTRATION], m, w.spy.ctx, e);
    if rule.no_countdown {
        c = 0;
    }
    let mp = w.spy.ctx.multiplayer;
    if m == spy::mission::KIDNAP && (t == 0 || w.spy.kidnap.get(t).copied().unwrap_or(0) != 0 || (!mp && t == 5)) {
        c = 0;
    }
    if !mp && w.race(t) == 5 {
        c /= 2;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::{EconomyContext, Planet};
    use crate::colony_side::{ColonyWorld, SideRules};
    use crate::fleet::Fleet;
    use crate::diplomacy::Relation;
    use crate::research::{Research, ResearchContext, ResearchRules};
    use crate::rng::Streams;
    use crate::spacewar::{CombatFleet, CombatWorld, Design, Ship, Vehicle};
    use crate::spy::{Spy, SpyContext, SpyRules, StoryCheck};

    struct Fixture {
        ctx: AiContext,
        rules: AiRules,
        sctx: SpyContext,
        colony: ColonyWorld,
        side: SideRules,
        econ: EconomyContext,
        combat: CombatWorld,
        research: Research,
        rr: ResearchRules,
        rc: ResearchContext,
        rel: Vec<[Relation; EMPIRES]>,
        kidnap: [i32; 18],
        rng: Streams,
        spy_rules: SpyRules,
        spies: Spies,
        starmap: Starmap,
        star_ctx: StarmapContext,
        fleet_numbers: [i32; EMPIRES],
        power: [f32; EMPIRES],
        planet_count: [i32; EMPIRES],
        danger: [i32; EMPIRES],
        known: [[i32; EMPIRES]; EMPIRES],
    }

    fn fixture() -> Fixture {
        let mut sctx = SpyContext { program_players: 8, difficulty: 2, ..Default::default() };
        for e in 1..=4 {
            sctx.alive[e] = true;
            sctx.race[e] = e as u8;
        }
        sctx.human[1] = true;
        let human = sctx.human;
        let mut ctx = AiContext { program_players2: 8, spies_enabled: true, ..Default::default() };
        for e in 2..=4 {
            ctx.ai[e] = true;
        }
        let rules = AiRules {
            build_mode: vec![0, 1, 1, 1, 1, 3, 2, 3, 2, 2, 2],
            build_min: vec![0, 5, 14, 13, 18, 15, 19, 23, 1, 5, 5],
            build_max: vec![0, 5, 14, 13, 18, 17, 22, 23, 4, 8, 11],
            production_share: vec![50; 15],
            military_mode: vec![9; 15],
            training_cost: vec![0, 1000, 2000, 5000, 10000, 20000],
            mission_cost_mult: vec![1.0; 15],
            ..Default::default()
        };
        let mut missions = vec![MissionRule::default(); 27];
        for (m, r) in missions.iter_mut().enumerate().skip(1).take(13) {
            *r = MissionRule { skill_floor: 1, skill_full: 5, chance: 60, needs_target: true, days: 10, cost: 500, min_rank: 1, ..Default::default() };
            if m == 13 {
                r.no_countdown = true;
            }
        }
        let mut colony = ColonyWorld { planets: vec![Planet::default(); 6], ..Default::default() };
        colony.alive = sctx.alive;
        Fixture {
            ctx,
            rules,
            sctx,
            colony,
            side: SideRules::default(),
            econ: EconomyContext::default(),
            combat: CombatWorld { fleets: vec![CombatFleet { owner: 0xff, ..Default::default() }], ships: vec![Ship { fleet: 0, design: 0 }], vehicles: vec![Vehicle { fleet: 0, design: 0, count: 0 }], designs: vec![Vec::new(); EMPIRES], ..Default::default() },
            research: Research::default(),
            rr: ResearchRules::default(),
            rc: ResearchContext::default(),
            rel: vec![[Relation::default(); EMPIRES]; EMPIRES],
            kidnap: [0; 18],
            rng: Streams::new(21),
            spy_rules: SpyRules { missions, ..Default::default() },
            spies: Spies::default(),
            starmap: Starmap { fleets: vec![Fleet::from_record(&[0xffu8; 0x80])], ..Default::default() },
            star_ctx: StarmapContext { human_controlled: human, ..Default::default() },
            fleet_numbers: [0; EMPIRES],
            power: [0.0; EMPIRES],
            planet_count: [0; EMPIRES],
            danger: [1; EMPIRES],
            known: [[0; EMPIRES]; EMPIRES],
        }
    }

    fn run(fx: &mut Fixture, f: impl FnOnce(&mut World)) -> Vec<AiEvent> {
        let mut story = |_: StoryCheck| false;
        let mut w = World {
            ctx: &fx.ctx,
            rules: &fx.rules,
            spy: spy::World {
                ctx: &fx.sctx,
                colony: &mut fx.colony,
                side_rules: &fx.side,
                econ: &fx.econ,
                combat: &mut fx.combat,
                research: &mut fx.research,
                research_rules: &fx.rr,
                research_ctx: &fx.rc,
                relations: &mut fx.rel,
                kidnap: &mut fx.kidnap,
                rng: &mut fx.rng,
                rules: &fx.spy_rules,
                story: &mut story,
                events: Vec::new(),
            },
            spies: &mut fx.spies,
            starmap: &mut fx.starmap,
            star_ctx: &fx.star_ctx,
            fleet_numbers: &mut fx.fleet_numbers,
            power: &fx.power,
            planet_count: &fx.planet_count,
            danger: &fx.danger,
            known_since: &mut fx.known,
            events: Vec::new(),
        };
        f(&mut w);
        w.events
    }

    fn planet(owner: u8, x: f32, pop: i32, ships: i32) -> Planet {
        Planet { owner, x, y: 2.0 * x, population: pop, ship_capacity: ships, richness: 100, ..Default::default() }
    }

    #[test]
    fn centres_main_fleet_and_contact_age() {
        let mut fx = fixture();
        fx.colony.planets[1] = planet(2, 10.0, 0, 0);
        fx.colony.planets[2] = planet(2, 15.0, 0, 0);
        fx.colony.knows[2][3] = 1;
        fx.known[2][3] = 7;
        let mut rec = [0u8; 0x80];
        rec[0x15] = 2;
        rec[0x60..0x64].copy_from_slice(&50i32.to_le_bytes());
        fx.starmap.fleets.push(Fleet::from_record(&rec));
        rec[0x60..0x64].copy_from_slice(&80i32.to_le_bytes());
        rec[0x52] = 0x08; // bit 3: never the main fleet
        fx.starmap.fleets.push(Fleet::from_record(&rec));
        let mut ai = AiState::default();
        let ev = run(&mut fx, |w| ai.bookkeeping(w));
        assert_eq!((ai.centre_x[2], ai.centre_y[2]), (12, 25));
        assert_eq!(ai.main_fleet[2], 1);
        assert_eq!(fx.starmap.fleets[1].flags & FLAG_MAIN, FLAG_MAIN);
        assert_eq!(fx.known[2][3], 6);
        assert_eq!(ev, vec![AiEvent::MainFleetNamed { empire: 2, fleet: 1, race: 2 }]);
    }

    #[test]
    fn main_planet_and_planet_settings() {
        let mut fx = fixture();
        fx.colony.planets[1] = planet(2, 1.0, 20_000, 0);
        fx.colony.planets[1].vehicle_capacity = 1;
        fx.colony.planets[2] = planet(2, 2.0, 30_000, 10);
        fx.colony.planets[3] = planet(2, 3.0, 8_000, 0);
        fx.colony.planets[3].unrest = 4;
        fx.side.colony.race_production = vec![1.0; 15];
        fx.colony.empires.spy_centres[2] = 12;
        fx.colony.strength.ground[2] = 1_000_000;
        fx.colony.strength.ground[3] = 1_000_000;
        fx.danger[2] = 3;
        let mut ai = AiState::default();
        run(&mut fx, |w| ai.planet_settings(w, 2));
        assert_eq!(ai.main_planet[2], 2, "the first planet with a shipyard");
        let p = &fx.colony.planets;
        // 50 % of 3 planets → 1 production planet (mode 3), the rest mode 2.
        assert_eq!([p[1].autobuild_mode, p[2].autobuild_mode, p[3].autobuild_mode], [9, 2, 2], "planet 1 lacks ground forces");
        assert_eq!([p[1].autobuild_aggr, p[1].tax_level, p[3].tax_level], [2, 3, 0]);
    }

    #[test]
    fn income_shares_round_and_fill_up() {
        let mut ai = AiState::default();
        for e in 1..EMPIRES {
            ai.set_sliders(e, 3);
        }
        ai.set_sliders(2, 1);
        ai.sliders[3] = [0.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let mut money = [Treasury::default(); EMPIRES];
        ai.income_shares(&mut money);
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        for (got, want) in money[2].share[1..].iter().zip([0.4, 0.3, 0.3, 0.1, 0.1]) {
            assert!(near(*got, want), "{:?}", money[2].share);
        }
        // 0.33 × 3 = 0.99 → pool 1 gets the missing 0.01.
        assert!(near(money[3].share[1], 0.34) && near(money[3].share[2], 0.33), "{:?}", money[3].share);
        assert!(((money[3].share[1] + 0.0) + money[3].share[2]) + money[3].share[3] >= 1.0);
    }

    fn design(class: i16, time: i32, flag36: u8) -> Design {
        let mut raw = [0u8; 0x84];
        raw[0x16..0x18].copy_from_slice(&class.to_le_bytes());
        raw[0x20..0x24].copy_from_slice(&time.to_le_bytes());
        raw[0x36] = flag36;
        Design::from_bytes(&raw)
    }

    #[test]
    fn production_fills_idle_lines() {
        let mut fx = fixture();
        fx.colony.planets[1] = planet(2, 1.0, 20_000, 5);
        fx.colony.planets[1].vehicle_capacity = 3;
        fx.colony.planets[2] = planet(2, 2.0, 20_000, 5);
        fx.combat.designs[2] = vec![design(0, 0, 0), design(1, 111, 0), design(1, 222, 1), design(2, 333, 0)];
        let mut ai = AiState::default();
        ai.main_planet[2] = 1;
        ai.empty_planets[2] = 1;
        ai.designs[BEST][2][1] = 1; // ships (category 8: slots 1..=4)
        ai.designs[BEST][2][13] = 2; // colony ships (category 3)
        ai.designs[BEST][2][19] = 3; // vehicles (category 6)
        run(&mut fx, |w| ai.production(w, 2));
        let q = |p: usize| crate::production::Queue::from_bytes(&fx.colony.planets[p].production);
        let (a, b) = (q(1), q(2));
        assert_eq!((a.design[0][0], a.count[0][0], a.left[0]), (2, 3, 222), "the main planet builds the wanted colony ship");
        assert_eq!((a.design[1][0], a.left[1], a.rally), (3, 333, 0));
        assert_eq!((b.design[0][0], b.design[0][1], b.left[0], b.rally), (1, 0, 111, 1));
        assert_eq!(ai.categories[2][category::COLONY_SHIP].want, 1);
        assert_eq!(ai.categories[2][category::COLONY_SHIP].queued[BEST], 0, "counted before the queues are filled");
    }

    #[test]
    fn scrapping_sells_the_rare_slow_bucket() {
        let mut fx = fixture();
        fx.sctx.day = 20;
        let mk = |speed: f32, attack: i32, price: i32| {
            let mut d = design(1, 0, 0);
            d.raw[0x3c..0x40].copy_from_slice(&speed.to_le_bytes());
            d.raw[0x2c..0x30].copy_from_slice(&attack.to_le_bytes());
            d.raw[0x30..0x34].copy_from_slice(&attack.to_le_bytes());
            d.raw[0x24..0x28].copy_from_slice(&price.to_le_bytes());
            Design::from_bytes(&d.raw)
        };
        fx.combat.designs[2] = vec![design(0, 0, 0), mk(0.02, 10, 900), mk(0.1, 100, 0)];
        let mut fleet = CombatFleet { owner: 2, ..Default::default() };
        fleet.totals.ships = 2;
        fx.combat.fleets.push(fleet);
        fx.starmap.fleets.push(Fleet::from_record(&{
            let mut r = [0u8; 0x80];
            r[0x15] = 2;
            r
        }));
        fx.combat.ships = vec![Ship { fleet: 0, design: 0 }, Ship { fleet: 1, design: 2 }, Ship { fleet: 1, design: 1 }];
        fx.colony.money[2].share = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        fx.side.difficulty_money = vec![1.0; 5];
        let mut ai = AiState::default();
        let ev = run(&mut fx, |w| ai.production(w, 2));
        assert_eq!(fx.combat.ships[2].design, NO_DESIGN, "bucket 2 holds 10 of 110 strength: sold");
        assert_eq!(fx.combat.ships[1].design, 2);
        assert_eq!(fx.colony.money[2].pools[1], 450.0);
        assert!(ev.contains(&AiEvent::ShipSold { empire: 2, ship: 2, design: 1, price: 450 }));
    }

    fn spy(state: i16, inf: u8, counter: u8, xp: i32) -> Spy {
        let mut s = Spy { state, xp, rank: 5, ..Default::default() };
        s.skills = [inf, counter, 3, 3];
        s
    }

    #[test]
    fn spies_get_points_training_and_counter_intelligence() {
        let mut fx = fixture();
        fx.spies.count[2] = 4;
        fx.spies.roster[2][1] = spy(1, 9, 1, 3000);
        fx.spies.roster[2][2] = spy(1, 2, 8, 5000);
        fx.spies.roster[2][3] = spy(1, 3, 1, 2500);
        fx.spies.roster[2][4] = spy(1, 1, 1, 100);
        fx.spies.roster[2][1].points = 2;
        fx.spies.roster[2][2].points = 1;
        fx.colony.empires.spy_capacity[2] = 3;
        // Nobody knows another empire, so no target is ever picked and nothing is assigned.
        let ev = run(&mut fx, |w| spy_missions(w, 2));
        assert!(ev.is_empty());
        let r = &fx.spies.roster[2];
        assert_eq!(r[1].skills, [9, 1, 5, 3], "selected: infiltration and combat, lowest first");
        assert_eq!(r[2].skills, [2, 8, 4, 3], "the rest: counter-intelligence and combat");
        assert_eq!(r[4].state, 0, "4 spies > capacity 3: the least experienced (first on ties) is fired");
    }

    #[test]
    fn mission_cost_and_length() {
        let mut fx = fixture();
        fx.spies.count[2] = 1;
        fx.spies.roster[2][1] = spy(1, 5, 0, 5000);
        fx.colony.empires.spy_capacity[2] = 5;
        fx.colony.knows[2][3] = 1;
        fx.colony.planets[1] = planet(3, 1.0, 100, 0);
        fx.rel[2][3].level = 1;
        fx.sctx.difficulty = 3; // rndlog(10, 1, −100) = 1: the best spy always looks for a mission
        fx.colony.money[2].pools[4] = 5000.0;
        run(&mut fx, |w| spy_missions(w, 2));
        let s = fx.spies.roster[2][1];
        assert_eq!((s.state, s.target), (ON_MISSION, 3));
        assert_eq!((s.length, s.left), (500, 500));
        assert!((166..=470).contains(&s.catch_at));
        assert_eq!(fx.colony.money[2].pools[4], 4500.0);
    }

    // -------------------------------------------------------------------------------------------- part 2

    /// A fleet of `owner` at (x, y) in both fleet views; returns its slot.
    fn add_fleet(fx: &mut Fixture, owner: u8, x: f32, y: f32) -> usize {
        let mut rec = [0u8; 0x80];
        rec[0x15] = owner;
        rec[0x19..0x2c].fill(1);
        rec[0x30..0x34].copy_from_slice(&x.to_le_bytes());
        rec[0x34..0x38].copy_from_slice(&y.to_le_bytes());
        rec[0x38..0x3c].copy_from_slice(&0.06f32.to_le_bytes());
        fx.starmap.fleets.push(Fleet::from_record(&rec));
        fx.combat.fleets.push(CombatFleet { owner, x, y, ..Default::default() });
        fx.starmap.fleets.len() - 1
    }

    fn ship_design(attack: i32, hp: i32, colony: bool) -> Design {
        let mut raw = [0u8; 0x84];
        raw[0x16..0x18].copy_from_slice(&1i16.to_le_bytes());
        raw[0x2c..0x30].copy_from_slice(&attack.to_le_bytes());
        raw[0x30..0x34].copy_from_slice(&hp.to_le_bytes());
        raw[0x34] = colony as u8;
        Design::from_bytes(&raw)
    }

    /// `HajoNew(f, d)` as far as the sums go.
    fn add_ship(fx: &mut Fixture, f: usize, d: i16) {
        fx.combat.ships.push(Ship { fleet: f as i16, design: d });
        let owner = fx.combat.fleets[f].owner as usize;
        let des = fx.combat.designs[owner][d as usize].clone();
        fx.combat.fleets[f].totals.add_ship(&des, 1);
        if des.hull_flags[0] != 0 {
            fx.starmap.fleets[f].colony_ships += 1;
            fx.combat.fleets[f].colony_ships += 1;
            fx.combat.colony_ship_pool[owner] += 1;
        }
        let t = fx.combat.fleets[f].totals;
        let r = &mut fx.starmap.fleets[f];
        (r.ship_count, r.strength) = (t.ships, t.attack);
    }

    #[test]
    fn angle_and_point_safety() {
        let mut fx = fixture();
        let f = add_fleet(&mut fx, 2, 0.0, 0.0);
        run(&mut fx, |w| {
            assert!((w.angle(f, 10.0, 0.0, 0.0, 10.0) - 90.0).abs() < 1e-4);
            assert!((w.angle(f, 10.0, 0.0, -10.0, 0.0) - 171.89).abs() < 0.01, "clamped at −0.99");
            assert!((w.angle(f, 10.0, 0.0, 20.0, 0.0) - 8.109614).abs() < 1e-4, "clamped at 0.99");
            assert_eq!(w.angle(f, 0.0, 0.0, 5.0, 5.0), 0.0, "a point on the fleet");
        });
        // Without radar reach: the own term is 10000 − 10·distance, the foe term (no planets: 1e5 away) the far branch.
        fx.colony.planets[1] = Planet { owner: 2, x: 30.0, y: 40.0, ..Default::default() };
        run(&mut fx, |w| assert_eq!(w.safety(2, 3, 0.0, 0.0), (10000.0 - 500.0) + 50000.0));
    }

    #[test]
    fn danger_near_a_foe_at_war() {
        let mut fx = fixture();
        let f = add_fleet(&mut fx, 2, 0.0, 0.0);
        let g = add_fleet(&mut fx, 3, 5.0, 0.0);
        fx.combat.designs[3] = vec![ship_design(0, 0, false), ship_design(50, 40, false)];
        add_ship(&mut fx, g, 1);
        fx.planet_count[2] = 3;
        fx.rel[2][3].level = 1;
        // Same speed (class 5, reach 20): 25 < (20·0.33)² and Δ = 0 ≤ 0.02 → close danger.
        run(&mut fx, |w| assert_eq!(w.danger_at(f, 0.0, 0.0, 20.0), (3, 5.0, g)));
        fx.rel[2][3].level = 4;
        run(&mut fx, |w| assert_eq!(w.danger_at(f, 0.0, 0.0, 20.0).0, 0, "a pact partner is no threat"));
        fx.rel[2][3].level = 1;
        fx.planet_count[2] = 0;
        run(&mut fx, |w| assert_eq!(w.danger_at(f, 0.0, 0.0, 20.0).0, 0, "an empire without planets fears nothing"));
    }

    #[test]
    fn idle_fleets_regroup() {
        let mut fx = fixture();
        fx.combat.designs[2] = vec![ship_design(0, 0, false), ship_design(10, 10, false)];
        let f = add_fleet(&mut fx, 2, 0.0, 0.0);
        let g = add_fleet(&mut fx, 2, 3.0, 4.0);
        add_ship(&mut fx, f, 1);
        add_ship(&mut fx, g, 1);
        let mut ai = AiState::default();
        let res = run(&mut fx, |w| assert_eq!(ai.flee(w, f, 0, 0), 1));
        assert!(res.is_empty());
        let (a, b) = (&fx.starmap.fleets[f], &fx.starmap.fleets[g]);
        assert_eq!((a.order, a.kind, a.tx), (1, 2, g as f32));
        assert_eq!((b.order, b.kind, b.tx), (1, 2, f as f32));
        assert_eq!(b.flags & 0x20, 0x20, "the partner ignores radar while joining");
        assert_eq!(a.flags & 0x20, 0);
    }

    #[test]
    fn colony_ship_goes_to_the_nearest_free_planet() {
        let mut fx = fixture();
        fx.combat.designs[2] = vec![ship_design(0, 0, false), ship_design(0, 5, true)];
        let f = add_fleet(&mut fx, 2, 0.0, 0.0);
        add_ship(&mut fx, f, 1);
        for (p, x) in [(1, 10.0f32), (2, 50.0)] {
            fx.colony.planets[p] = Planet { x, y: 0.0, ..Default::default() };
            fx.colony.planets[p].visibility[2] = 1;
        }
        fx.planet_count[2] = 8;
        let mut ai = AiState::default();
        ai.categories[2][category::COLONY_SHIP].units[BEST] = 1;
        run(&mut fx, |w| ai.colonise(w, 2, &mut ()));
        let r = &fx.starmap.fleets[f];
        assert_eq!(&r.name[..13], b"Colonization\0");
        assert_eq!((r.order, r.kind, r.tx), (1, 1, 1.0), "planet 1 (100 · up to 2) beats planet 2 (2500 · up to 2)");
        assert_eq!(r.flags & 8, 8);
        assert_eq!(fx.combat.colony_ship_pool[2], 0, "the ship left the pool");
        assert_eq!(ai.empty_planets[2], 1);
    }

    #[test]
    fn merging_moves_ships_and_vehicles() {
        let mut fx = fixture();
        fx.star_ctx.default_fleet_name = b"Fleet".to_vec();
        let mut tank = ship_design(4, 9, false);
        tank.class = 2;
        fx.combat.designs[2] = vec![ship_design(0, 0, false), ship_design(10, 10, false), tank];
        let a = add_fleet(&mut fx, 2, 0.0, 0.0);
        let b = add_fleet(&mut fx, 2, 1.0, 0.0);
        fx.starmap.fleets[a].name[..6].copy_from_slice(b"Fleet\0");
        fx.starmap.fleets[b].name[..6].copy_from_slice(b"Alpha\0");
        add_ship(&mut fx, a, 1);
        add_ship(&mut fx, b, 1);
        add_ship(&mut fx, b, 1);
        fx.combat.vehicles.push(Vehicle { fleet: b as i16, design: 2, count: 3 });
        fx.fleet_numbers[2] = 2;
        run(&mut fx, |w| {
            w.recount();
            w.merge(a, b);
        });
        assert_eq!(&fx.starmap.fleets[a].name[..6], b"Alpha\0");
        assert_eq!(fx.starmap.fleets[a].flags & 0x800, 0x800, "named");
        assert_eq!(fx.starmap.fleets[b].owner, 0xff);
        assert_eq!(fx.combat.fleets[a].totals.ships, 3);
        assert_eq!((fx.combat.fleets[a].totals.ground, fx.combat.fleets[a].totals.ground_strength), (3, 18), "3 × (int)sqrt(36)");
        assert!(fx.combat.ships.iter().skip(1).all(|s| s.fleet as usize == a));
        assert_eq!(fx.fleet_numbers[2], 1);
    }

    #[test]
    fn design_deletion_clears_queues_stations_and_ships() {
        let mut fx = fixture();
        let mut other = ship_design(1, 1, false).raw;
        other[0x20..0x24].copy_from_slice(&77i32.to_le_bytes());
        fx.combat.designs[2] = vec![ship_design(0, 0, false), ship_design(10, 10, false), Design::from_bytes(&other)];
        let f = add_fleet(&mut fx, 2, 0.0, 0.0);
        add_ship(&mut fx, f, 1);
        let p = &mut fx.colony.planets[1];
        p.owner = 2;
        for (k, d) in [1i16, 2, 1, 0].iter().enumerate() {
            p.production[2 * k..2 * k + 2].copy_from_slice(&d.to_le_bytes());
            p.production[0x10 + k] = k as u8 + 1;
        }
        p.stations[..2].copy_from_slice(&[1, 2]);
        p.field_114 = 2;
        run(&mut fx, |w| w.delete_design(2, 1, 0, false));
        let q = crate::production::Queue::from_bytes(&fx.colony.planets[1].production);
        assert_eq!(q.design[0], [2, 0, 0, 0], "both entries of design 1 went, the rest moved up");
        assert_eq!(q.count[0][0], 2);
        assert_eq!(q.left[0], 77, "the new first entry's build time");
        assert_eq!((fx.colony.planets[1].stations[0], fx.colony.planets[1].field_114), (2, 1));
        assert_eq!(fx.combat.ships[1].design, NO_DESIGN);
        assert_eq!(fx.starmap.fleets[f].owner, 0xff, "the emptied fleet was deleted");
    }

    #[test]
    fn design_pass_creates_a_design_per_buildable_slot() {
        let mut fx = fixture();
        let mut hulls = vec![crate::spacewar::Hull { class: 1, flags: [0; 6], tech: 11 }; DESIGN_SLOTS];
        hulls[1].tech = 10;
        let mut slot_types = vec![vec![vec![0; 12]; 27]; 14];
        slot_types[0][1][0] = 7;
        let mut slot_max = vec![vec![vec![0; 12]; 27]; 14];
        slot_max[0][1][0] = 3;
        fx.rules.design = DesignRules {
            hulls,
            tech_price: vec![0; 193],
            tech_time: vec![5; 193],
            tech_hp: vec![1; 193],
            tech_capacity: vec![0; 193],
            firepower: vec![2; 193],
            race_pod_table: vec![1; 19],
            pod_slot_types: slot_types,
            tank_speeds: vec![0.0; 5],
        };
        fx.rules.pod_slot_max = slot_max;
        fx.rules.pods = vec![PodRule { id: 7, techs: [20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }];
        let tech = &mut fx.colony.tech;
        for t in [10, 20] {
            tech.set_status(t, 2, 5);
        }
        tech.bytes[20 * crate::colony_side::TECH_STRIDE + 0x24] = 2;
        fx.combat.designs[2] = vec![ship_design(0, 0, false); 0x29];
        let mut ai = AiState::default();
        let ev = run(&mut fx, |w| ai.design(w, 2, true, &mut ()));
        let (best, special) = (ai.designs[BEST][2][1] as usize, ai.designs[SPECIAL][2][1] as usize);
        assert_eq!((best, special), (0x29, 0x2a), "two new records at the end");
        for d in [best, special] {
            let x = &fx.combat.designs[2][d];
            assert_eq!((x.hull, x.pod_table, x.pods[0], x.counts[0]), (1, 2, 20, 2), "tech 20 in slot 1, min(2, 3) of it");
            assert_eq!(x.build_time, 5 + 2 * 5);
        }
        assert_eq!(ai.designs[BEST][2][2], 0, "hull tech 11 isn't developed");
        assert_eq!(ev.iter().filter(|e| matches!(e, AiEvent::DesignNamed { .. })).count(), 2);
    }

    #[test]
    fn a_dropped_player_waits_for_upgrades() {
        let mut fx = fixture();
        let mut ai = AiState::default();
        ai.main_fleet[3] = 4;
        ai.designs[BEST][3][5] = 9;
        ai.no_upgrade[3] = 1;
        run(&mut fx, |w| ai.init_dropped(w, 3, &mut ()));
        assert_eq!((ai.main_fleet[3], ai.designs[BEST][3][5], ai.no_upgrade[3]), (0, 0, 720));
        run(&mut fx, |w| ai.init_dropped(w, 1, &mut ()));
        assert_eq!(ai.no_upgrade[1], 0, "the human is left alone");
    }

    #[test]
    fn fleet_pass_counts_days_down() {
        let mut fx = fixture();
        let mut ai = AiState::default();
        (ai.no_load[2], ai.load_phase[2]) = (2, 1);
        run(&mut fx, |w| ai.fleets(w, 2, &mut ()));
        assert_eq!((ai.no_load[2], ai.load_phase[2]), (1, 0));
    }
}
