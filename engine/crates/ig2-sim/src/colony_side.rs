//! Colony side systems around the economy step: rebellion (`BolygoFellazad`, exe 0x53bc40), change of
//! owner (`BolygoElfoglal`, 0x53b720) with tech capture, colonisation (`Generate_Planet0`, 0x59adf0),
//! autobuild (scoring in `FRISSIT_Planet` 0x474930, purchase in its epilogue), money (`MoneyMennyi`
//! 0x540170, `MoneyHozzaadOld` 0x53fbd0), new buildings (`EpuletNew`, 0x53c7d0), per-empire totals and
//! the yearly income roll-over. Spec: `re/rules/colony-side.md`.
//!
//! New buildings get their position from a [`Placement`]: normally the colony-map search
//! [`crate::placement::PlacementRules::place`]. Not ported here: fleets, garrisons and ground-war state
//! (reported as [`SideEvent`]s), UI messages.

use crate::colony::{self, ColonyEvent, ColonyRules, EconomyContext, Effects, Planet, PlanetReport};
use crate::fleet::{Arrivals, StarPlanet};
use crate::placement::PlacementRules;
use crate::rng::{stream, Streams};

/// Empire slots (index 0 = none; 1..=8 players, 9.. pirates, rebels and story empires).
pub const EMPIRES: usize = 19;
/// Planet slots of the per-planet tables (index 1..=200).
pub const PLANET_SLOTS: usize = 201;
/// Tech records in the live table (index 0..=192).
pub const TECHS: usize = 193;
pub const TECH_STRIDE: usize = 0x114;

/// Autobuild categories (`AutoBuildMitEpitMin/Max`, `AutoBuildHanyasProblema`), index 1..=28.
const CATEGORIES: usize = 29;
/// Building types with a fixed role here.
mod kind {
    pub const COLONY_HUB: usize = 1;
    pub const FIRE_BRIGADE: usize = 8;
    pub const SPY_CENTRE: usize = 26;
    pub const TRADE_PORT: usize = 50;
    pub const SOLAR: u8 = 53;
    pub const MAGMA: u8 = 54;
}

/// Tech status values (`Talalmanyok + 0x45 + e`).
pub mod status {
    pub const UNKNOWN: u8 = 1;
    pub const SEEN: u8 = 2;
    pub const RESEARCHABLE: u8 = 3;
    /// Being researched by the AI (`FRISSIT_MI_FEJLESZTES`).
    pub const RESEARCHING: u8 = 4;
    pub const OWNED: u8 = 5;
}

/// One `autobldN.txt` mode (`INIT_AutoBuild`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutobuildMode {
    /// `AutoBuildProblemaAlsoSzazalek[m]`: a problem counts once its level reaches this ×10000.
    pub problem_threshold: i32,
    /// `AutoBuildEgyebSzazalek[m]`: priority of the listed buildings.
    pub listed_priority: i32,
    /// `AutoBuildEgyebEpitendok[m][1..=50]`: categories to keep built, in order.
    pub list: Vec<i32>,
}

impl AutobuildMode {
    /// Map the entries of an `autobldN.txt` file to categories with the exe's names
    /// (`AutoBuildNevek[1..=16]`, `names[0]` unused): exact matches, at most 50, up to `END`.
    pub fn from_entries(threshold: i32, listed_priority: i32, entries: &[String], names: &[String]) -> Self {
        let mut list = Vec::new();
        for entry in entries {
            for (c, name) in names.iter().enumerate().skip(1).take(16) {
                if name == entry {
                    list.push(c as i32);
                }
            }
            if entry == "END" || list.len() >= 50 {
                break;
            }
        }
        Self { problem_threshold: threshold, listed_priority, list }
    }

    /// `EgyebEpitendok[m][j]` (0 past the end).
    fn entry(&self, j: usize) -> i32 {
        self.list.get(j.wrapping_sub(1)).copied().unwrap_or(0)
    }
}

/// The compiled-in autobuild tables (exe 0x8F1D50 / 0x8F1DD0 / 0x8F1E50) and the per-mode files.
#[derive(Debug, Clone, Default)]
pub struct AutobuildRules {
    /// Per category (index 0 unused): problem slot it answers (0 = listed category, −1 = none).
    pub problem: Vec<i32>,
    /// Building type range of the category.
    pub min: Vec<i32>,
    pub max: Vec<i32>,
    /// Modes 1..=10 (index 0 unused).
    pub modes: Vec<AutobuildMode>,
}

/// Rules the side systems read besides the economy's.
#[derive(Debug, Clone)]
pub struct SideRules {
    pub colony: ColonyRules,
    /// `TalalmanyokrEro[tech]` (static, exe 0x8EB730): ground strength of a defence building.
    pub firepower: Vec<i32>,
    /// `TalalmanyEroHP[tech]` (exe 0xBA9210, filled at start-up).
    pub tech_hp: Vec<i32>,
    /// `FajSpeckoKutatasIdoSzorzo[race]`.
    pub race_research_time: Vec<f32>,
    /// `NehezsegPenzSzorzo[difficulty]`: AI income multiplier.
    pub difficulty_money: Vec<f32>,
    /// `pGyartas0`: the empty production queue.
    pub production_default: [u8; 64],
    pub autobuild: AutobuildRules,
}

impl Default for SideRules {
    fn default() -> Self {
        Self {
            colony: ColonyRules::default(),
            firepower: Vec::new(),
            tech_hp: Vec::new(),
            race_research_time: Vec::new(),
            difficulty_money: Vec::new(),
            production_default: [0; 64],
            autobuild: AutobuildRules::default(),
        }
    }
}

/// The live tech table (`Talalmanyok`, 0x114 bytes per tech): static fields plus per-empire state.
/// Kept as bytes because the original writes past a record's end (the seen-building matrix, spec §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechTable {
    pub bytes: Vec<u8>,
}

impl Default for TechTable {
    fn default() -> Self {
        Self { bytes: vec![0; TECHS * TECH_STRIDE] }
    }
}

impl TechTable {
    fn at(&self, t: usize, off: usize) -> usize {
        t * TECH_STRIDE + off
    }
    fn i32(&self, o: usize) -> i32 {
        self.bytes.get(o..o + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))
    }
    /// `+0x14`: price.
    pub fn price(&self, t: usize) -> i32 {
        self.i32(self.at(t, 0x14))
    }
    /// `+0x38`: base research time.
    pub fn base_research_time(&self, t: usize) -> i32 {
        self.i32(self.at(t, 0x38))
    }
    /// `+0x26` (u8) / `+0x28` (i32): either non-zero → automatic design when a human develops it.
    pub fn auto_design(&self, t: usize) -> bool {
        self.bytes[self.at(t, 0x26)] != 0 || self.i32(self.at(t, 0x28)) != 0
    }
    /// `+0x31..+0x33`: lab levels needed (ship, vehicle, building research).
    pub fn labs_needed(&self, t: usize) -> [u8; 3] {
        let o = self.at(t, 0x31);
        [self.bytes[o], self.bytes[o + 1], self.bytes[o + 2]]
    }
    /// `+0x34`: research cost.
    pub fn research_cost(&self, t: usize) -> i32 {
        self.i32(self.at(t, 0x34))
    }
    /// `+0xb7 + r`: the AI's research priority for race `r` (0 = none, lower first).
    pub fn ai_priority(&self, t: usize, r: usize) -> u8 {
        self.bytes.get(self.at(t, 0xb7 + r)).copied().unwrap_or(0)
    }
    /// `+0x2d..+0x2f`.
    pub fn prerequisites(&self, t: usize) -> [usize; 3] {
        let o = self.at(t, 0x2d);
        [self.bytes[o] as usize, self.bytes[o + 1] as usize, self.bytes[o + 2] as usize]
    }
    /// Byte `off` of tech `t`'s record (0 past the table).
    pub fn byte(&self, t: usize, off: usize) -> u8 {
        self.bytes.get(self.at(t, off)).copied().unwrap_or(0)
    }
    /// Tech `t`'s name at `+0x00`, up to and including its NUL (what `strcpy` copies).
    pub fn name_bytes(&self, t: usize) -> Vec<u8> {
        let o = self.at(t, 0);
        let rec = &self.bytes[o..(o + TECH_STRIDE).min(self.bytes.len())];
        let n = rec.iter().position(|&c| c == 0).map_or(rec.len(), |i| i + 1);
        rec[..n].to_vec()
    }
    /// `+0x45 + e`.
    pub fn status(&self, t: usize, e: usize) -> u8 {
        self.bytes[self.at(t, 0x45 + e)]
    }
    pub fn set_status(&mut self, t: usize, e: usize, s: u8) {
        let o = self.at(t, 0x45 + e);
        self.bytes[o] = s;
    }
    /// `+0x58 + 4e`: research time for empire `e`.
    pub fn research_time(&self, t: usize, e: usize) -> i32 {
        self.i32(self.at(t, 0x58 + 4 * e))
    }
    pub fn set_research_time(&mut self, t: usize, e: usize, v: i32) {
        let o = self.at(t, 0x58 + 4 * e);
        self.bytes[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    /// `+0xa4 + e`: granted without research (counts as a met prerequisite).
    pub fn given(&self, t: usize, e: usize) -> bool {
        self.bytes[self.at(t, 0xa4 + e)] != 0
    }
    pub fn set_given(&mut self, t: usize, e: usize) {
        let o = self.at(t, 0xa4 + e);
        self.bytes[o] = 1;
    }
    /// Seen-building matrix byte `+0xc0 + 9·spy + owner`, which runs into the next record for
    /// large indices (as in the original); writes past the table are dropped.
    pub fn mark_seen(&mut self, t: usize, spy: usize, owner: usize) {
        let o = self.at(t, 0xc0 + 9 * spy + owner);
        if let Some(b) = self.bytes.get_mut(o) {
            *b = 1;
        }
    }

    /// The seen-matrix byte [`TechTable::mark_seen`] sets (0 past the table).
    pub fn seen(&self, t: usize, spy: usize, owner: usize) -> bool {
        self.bytes.get(self.at(t, 0xc0 + 9 * spy + owner)).is_some_and(|&b| b != 0)
    }

    /// `Research_TalalmanyUjraSzamol` (exe 0x5245c0), spec §4.1.
    pub fn recompute(&mut self, players: i32, all_techs: bool) {
        for e in 1..=players.clamp(0, EMPIRES as i32 - 1) as usize {
            for t in 1..TECHS {
                let pre = self.prerequisites(t);
                let given = self.given(t, e);
                let have = pre.map(|x| self.status(x, e) == status::OWNED);
                if self.status(t, e) == status::UNKNOWN
                    && ((0..3).any(|i| pre[i] != 0 && have[i]) || pre == [0, 0, 0] || given)
                {
                    self.set_status(t, e, status::SEEN);
                }
                // Re-read: a tech may be its own prerequisite.
                let have = pre.map(|x| self.status(x, e) == status::OWNED);
                if self.status(t, e) == status::SEEN && (0..3).all(|i| have[i] || pre[i] == 0 || given) {
                    self.set_status(t, e, status::RESEARCHABLE);
                }
                if all_techs && matches!(self.status(t, e), status::UNKNOWN | status::SEEN) {
                    self.set_status(t, e, status::RESEARCHABLE);
                }
            }
        }
    }
}

/// One empire's money (`Money`, `MoneyTeruletenkent`, `MoneyHatar`, `MoneyTeruletSzorzo`), spec §6.4.
/// Pools 1..=3 are spendable (2 = buildings); 4 and 5 only receive income.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Treasury {
    pub total: f64,
    pub pools: [f64; 6],
    /// `[0]`: overall reserve; `[k]`: reserve of pool `k` (autobuild sets `[2]`).
    pub reserve: [f64; 6],
    /// Income share per pool.
    pub share: [f64; 6],
}

impl Treasury {
    fn surplus(&self, k: usize) -> f64 {
        let d = self.pools[k] - self.reserve[k];
        if d <= 0.0 { 0.0 } else { d }
    }

    /// Surplus of the spendable pools other than `k`, summed in pool order from 0.
    fn others(&self, k: usize) -> f64 {
        (1..=3).filter(|&i| i != k).fold(0.0, |acc, i| acc + self.surplus(i))
    }

    fn resum(&mut self) {
        self.total = self.pools[1] + 0.0 + self.pools[2] + self.pools[3];
    }

    /// `MoneyMennyi(e, k)`: what pool `k` can spend.
    pub fn available(&self, k: i32) -> f64 {
        if k < 1 {
            return self.total;
        }
        let k = k as usize;
        let mut x = self.pools[k] + self.others(k);
        if self.total - x < self.reserve[0] {
            x = self.total - self.reserve[0];
        }
        if self.share[k] < 0.01 {
            return 0.0;
        }
        x
    }

    /// `MoneyHozzaadOld(e, k, x)`. `ai_income` is the multiplier for a non-human owner's income
    /// (already halved in MP), `None` for a human. Returns false when a spend fails.
    pub fn add(&mut self, k: i32, mut x: f64, ai_income: Option<f64>) -> bool {
        if x > 0.0 {
            if let Some(m) = ai_income {
                x *= m;
            }
        }
        let k = k.max(0) as usize;
        if x > 0.0 {
            if k == 0 {
                for i in 1..=5 {
                    self.pools[i] += self.share[i] * x;
                }
            } else {
                self.pools[k] += x;
            }
            self.resum();
            return true;
        }
        let mut a = -x;
        let sur = self.others(k);
        let own = if k > 0 { self.pools[k].min(a) } else { 0.0 };
        if k == 0 {
            let [_, p1, p2, p3, ..] = self.pools;
            if p1 + 0.0 + p2 + p3 < a {
                let debt = a - p1 - p2 - p3;
                for i in 1..=3 {
                    self.pools[i] = 0.0 - self.share[i] * debt;
                }
                self.resum();
                return false;
            }
        } else if own + sur < a {
            return false;
        }
        a -= own;
        self.pools[k] -= own;
        let r = if sur > 0.001 { if a <= sur { a / sur } else { 1.0 } } else { 0.0 };
        for i in (1..=3).filter(|&i| i != k) {
            let d = self.surplus(i) * r;
            a -= d;
            self.pools[i] -= d;
        }
        if k == 0 && a > 0.01 {
            let tot = self.pools[1] + 0.0 + self.pools[2] + self.pools[3];
            let r = if tot > 0.001 { if a <= tot { a / tot } else { 1.0 } } else { 0.0 };
            for i in 1..=3 {
                self.pools[i] -= self.pools[i] * r;
            }
        }
        self.resum();
        true
    }
}

/// Per-empire tables rebuilt or accumulated by the economy step (spec §7, §9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmpireTables {
    /// `LaborSzint[e][1..=3]`: lab levels.
    pub labs: [[i32; 4]; EMPIRES],
    /// `LaborSzintOsszes[e][1..=3]`: lab buildings.
    pub lab_buildings: [[i32; 4]; EMPIRES],
    /// `HanyKemLehet` (max 12) and `HanyKemLehetOsszesen`, empires 0..9.
    pub spy_capacity: [i32; 9],
    pub spy_centres: [i32; 9],
    /// `HanyKemKozpontBolygonkent[e][p]`: never reset by the game (spec §7).
    pub spy_centres_by_planet: Vec<[i32; PLANET_SLOTS]>,
    pub trade_ports: [i32; 9],
    pub full_gdp: [i32; EMPIRES],
    pub full_tax: [i32; EMPIRES],
    pub tax_this_year: [i32; EMPIRES],
    pub tax_last_year: [i32; EMPIRES],
    pub trade_this_year: [i32; EMPIRES],
    pub trade_last_year: [i32; EMPIRES],
}

impl EmpireTables {
    /// `HanyTradePortVan[owner] += 1` without a bound check, as in the autobuild purchase: for
    /// owners ≥ 9 the write lands in `HanyKemKozpontBolygonkent`, which follows it in memory
    /// (seen at runtime for the rebels, owner 10).
    pub fn bump_trade_ports(&mut self, owner: usize) {
        if let Some(n) = self.trade_ports.get_mut(owner) {
            *n += 1;
        } else {
            let k = owner - self.trade_ports.len();
            self.spy_centres_by_planet[k / PLANET_SLOTS][k % PLANET_SLOTS] += 1;
        }
    }
}

impl Default for EmpireTables {
    fn default() -> Self {
        Self {
            labs: [[0; 4]; EMPIRES],
            lab_buildings: [[0; 4]; EMPIRES],
            spy_capacity: [0; 9],
            spy_centres: [0; 9],
            spy_centres_by_planet: vec![[0; PLANET_SLOTS]; 9],
            trade_ports: [0; 9],
            full_gdp: [0; EMPIRES],
            full_tax: [0; EMPIRES],
            tax_this_year: [0; EMPIRES],
            tax_last_year: [0; EMPIRES],
            trade_this_year: [0; EMPIRES],
            trade_last_year: [0; EMPIRES],
        }
    }
}

/// Per-planet defence totals (`BolygoMennyiFoldiEro`, `…ErodEro`, `…IonGunEro`, `…Tuzoltosag`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanetStrength {
    pub ground: Vec<i32>,
    pub fortress: Vec<i32>,
    pub ion_gun: Vec<i32>,
    pub fire_brigade: Vec<i32>,
}

impl Default for PlanetStrength {
    fn default() -> Self {
        let v = vec![0; PLANET_SLOTS];
        Self { ground: v.clone(), fortress: v.clone(), ion_gun: v.clone(), fire_brigade: v }
    }
}

/// The state the side systems read and write. `planets[0]` is unused, like the original.
#[derive(Debug, Clone, Default)]
pub struct ColonyWorld {
    pub planets: Vec<Planet>,
    /// `HanyEpulet[p]`: occupied slots at the last building pass (read by `EpuletNew`).
    pub occupied: Vec<i32>,
    /// `BolygoNeededMoral[p]`.
    pub needed_morale: Vec<i32>,
    /// `BolygoUtolsoKozeliFlotta[p]`.
    pub last_near_fleet: Vec<i32>,
    pub strength: PlanetStrength,
    pub empires: EmpireTables,
    pub money: [Treasury; EMPIRES],
    pub tech: TechTable,
    /// `BirodalomEl[e]`.
    pub alive: [bool; EMPIRES],
    /// `BirodalomIsmeri[a][b]`: `a` knows `b`.
    pub knows: [[u8; EMPIRES]; EMPIRES],
    /// The "all techs" switch (exe 0xB77254).
    pub all_techs: bool,
    /// `Nehezseg`: index into `NehezsegPenzSzorzo`.
    pub difficulty: usize,
    /// `AutoBuildMitEpit[c]`: the last category → building pick (a persistent global).
    pub autobuild_pick: [i32; CATEGORIES],
    /// How a pick of building 0 resolves (undefined in the original).
    pub zero_pick: ZeroPick,
    /// `NextColonyMap`'s per-surface-kind cursor (exe 0x996420, `u8` per kind 0..=7).
    pub map_cursor: [u8; 8],
}

/// Something for systems outside the colony port to do, or to report to the player.
#[derive(Debug, Clone, PartialEq)]
pub enum SideEvent {
    /// A planet economy event (index = planet).
    Colony { planet: usize, event: ColonyEvent },
    /// The unrest counter passed 30 on `owner`'s planet: `BolygoFellazad` follows (message 8 to the human
    /// owner, `info-messages.md` §3.2).
    Revolt { planet: usize, owner: u8 },
    /// `BolygoFellazad` ran; story special 0x15 (result 2) or 0x16 (result 3) follows in the step.
    Rebelled { planet: usize, result: i32, new_owner: u8 },
    /// Fleets with `order 2, kind 1, target planet` get `FlottaMozgat(f, 0, 0, 0, 0)`.
    RecallAttackers { planet: usize },
    /// The planet's garrison (`Tankok` entries with fleet −planet) is deleted and leaves its battlefield (owned by
    /// `old_owner`), whose turrets switch to `new_owner` (`BolygoElfoglal` exe 0x53b812..0x53b8ff; `Game` carries
    /// it out).
    GarrisonLost { planet: usize, old_owner: u8, new_owner: u8 },
    /// `empire` got `tech` with a captured building (message 7 when it is the human).
    TechCaptured { empire: u8, tech: usize },
    /// `who` now knows `whom` (story special 0xb and `FelfedezBirodalom` when `who` is the human).
    Met { who: u8, whom: u8 },
    /// Epidemic dropped on a change of owner: run the virus cleanup.
    VirusCleanup { planet: usize },
    /// Autobuild bought a building (slot is 1-based) or found no room (counter set to −1000).
    AutobuildPlaced { empire: u8, planet: usize, kind: u8, slot: usize },
    AutobuildNoRoom { empire: u8, planet: usize, kind: u8 },
    /// The planet had no buildings and was given up.
    Abandoned { planet: usize, owner: u8 },
    /// 1 January: last year's tax and trade income (messages 0x14 / 0x15 to the human).
    YearClosed,
}

/// The colony-map search for a free position of a new building of `kind` on planet `p` (given with its
/// current state): `LocateSuitableLocation`, [`crate::placement::PlacementRules::place`]. `None` = no room.
pub type Placement<'a> = &'a mut dyn FnMut(&Planet, usize, u8) -> Option<(f32, f32)>;

/// `TalalmanyEroHP[t]`, filled by `InitNewGame` (ELF 0x583577): `(int)sqrt((f32)firepower × (f32)hp)`
/// for techs 1..=192 (tech `+0x20` hit points; index 0 stays 0).
pub fn tech_strength(firepower: &[i32], hp: &[i32]) -> Vec<i32> {
    (0..firepower.len().min(hp.len()))
        .map(|t| if t == 0 { 0 } else { ((firepower[t] as f32 * hp[t] as f32) as f64).sqrt() as i32 })
        .collect()
}

/// `GetGyartasAr(e, t)` (exe 0x4acaf0).
pub fn price(tech: &TechTable, race: u8, t: usize) -> i32 {
    if (race == 4 || race == 7) && t == 13 {
        return (tech.price(13) as f64 * 0.75) as i32;
    }
    if race == 5 && !(81..=144).contains(&t) && t != 13 && t != 14 {
        return (tech.price(t) as f64 * 0.7000001) as i32;
    }
    tech.price(t)
}

impl ColonyWorld {
    fn ai_income(&self, e: usize, rules: &SideRules, ctx: &EconomyContext) -> Option<f64> {
        if ctx.human_controlled[e] {
            return None;
        }
        let m = rules.difficulty_money.get(self.difficulty).copied().unwrap_or(1.0) as f64;
        Some(if ctx.multiplayer { m * 0.5 } else { m })
    }

    /// `MoneyHozzaad(e, k, x)` with the owner's income multiplier.
    pub fn add_money(&mut self, e: usize, k: i32, x: f64, rules: &SideRules, ctx: &EconomyContext) -> bool {
        let m = self.ai_income(e, rules, ctx);
        self.money[e].add(k, x, m)
    }

    /// `EpuletNew(p, kind, done, x, y, dmg%)`: returns the 1-based slot.
    #[allow(clippy::too_many_arguments)]
    pub fn new_building(&mut self, p: usize, kind: u8, done: bool, pos: (f32, f32), damage_pct: i32, rules: &SideRules, ctx: &EconomyContext) -> usize {
        let planet = &mut self.planets[p];
        let slots = planet.buildings.len();
        let slot = if (self.occupied[p] as usize) < slots {
            planet.buildings.iter().position(|b| b.kind == 0).map_or(slots + 1, |j| j + 1)
        } else {
            planet.buildings.push(colony::Building::default());
            slots + 1
        };
        if slot > planet.buildings.len() {
            return slot;
        }
        self.occupied[p] += 1;
        let desc = rules.colony.buildings[kind as usize];
        let mut work = desc.build_work * 50;
        if ctx.empire_race[planet.owner as usize] == 4 {
            work = (work as f64 / 1.5) as i32;
        }
        let mut y = 100u8;
        if kind == kind::SOLAR {
            if planet.has_event(0x619) {
                y = 0;
            } else if planet.has_event(0x5fb) {
                y = ctx.story_value(0x5fb);
            }
        } else if (19..=22).contains(&kind) {
            if planet.has_event(0x60f) {
                y = ctx.story_value(0x60f);
            } else if planet.has_event(0x669) {
                y = 120;
            }
            if planet.has_event(colony::event::FACTORIES_STOPPED) {
                y = 0;
            }
        }
        if kind == kind::MAGMA && planet.has_event(0x64b) {
            y = (y as f64 * 1.33) as i32 as u8;
        }
        if (kind == kind::SOLAR && planet.surface == 3) || (kind == kind::MAGMA && planet.surface == 4) {
            y = (y as f64 * 1.5) as i32 as u8;
        }
        let owner = planet.owner;
        let b = &mut planet.buildings[slot - 1];
        *b = colony::Building {
            kind,
            owner,
            damage: desc.max_hp * damage_pct / 100,
            work_left: if done { 0 } else { work as u16 },
            planet: p as u8,
            x: pos.0,
            y: pos.1,
            yield_pct: y,
            flags: b.flags & 0x80,
            shown_energy: desc.energy as i16,
            shown_workers: desc.workers as i16,
            efficiency: 256,
        };
        slot
    }

    /// `EpuletNewSearchLocation(p, kind, done, dmg%, −1, −1)`: 0 when there is no room.
    #[allow(clippy::too_many_arguments)]
    pub fn place_building(&mut self, p: usize, kind: u8, done: bool, damage_pct: i32, place: Placement, rules: &SideRules, ctx: &EconomyContext) -> usize {
        match place(&self.planets[p], p, kind) {
            Some(pos) => self.new_building(p, kind, done, pos, damage_pct, rules, ctx),
            None => 0,
        }
    }

    /// `BolygoElfoglal(p, new)` (exe 0x53b720), spec §4.
    pub fn transfer(&mut self, p: usize, new: u8, rules: &SideRules, ctx: &EconomyContext, events: &mut Vec<SideEvent>) {
        if p == 0 {
            return;
        }
        let n = new as usize;
        self.planets[p].production = rules.production_default;
        if self.planets[p].owner != 0 {
            events.push(SideEvent::RecallAttackers { planet: p });
        }
        events.push(SideEvent::GarrisonLost { planet: p, old_owner: self.planets[p].owner, new_owner: new });
        for q in 1..self.planets.len() {
            let other = &mut self.planets[q];
            if other.owner != 0 && other.production[0x34] as usize == p {
                other.production[0x34] = 0;
            }
        }
        let captured: Vec<usize> = self.planets[p]
            .buildings
            .iter()
            .filter(|b| b.kind != 0 && b.work_left == 0)
            .map(|b| rules.colony.buildings[b.kind as usize].tech as usize)
            .collect();
        for t in captured {
            if self.tech.status(t, n) < status::OWNED {
                self.tech.set_given(t, n);
                let race = ctx.empire_race[n] as usize;
                let mult = rules.race_research_time.get(race).copied().unwrap_or(1.0);
                self.tech.set_research_time(t, n, (self.tech.base_research_time(t) as f32 * mult) as i32);
                self.tech.set_status(t, n, status::OWNED);
                self.tech.recompute(ctx.players, self.all_techs);
                events.push(SideEvent::TechCaptured { empire: new, tech: t });
            }
        }
        self.last_near_fleet[p] = 0;
        let planet = &mut self.planets[p];
        planet.unrest = 0;
        planet.owner = new;
        planet.autobuild_mode = 1;
        planet.autobuild_aggr = 2;
        planet.tax_level = 3;
        planet.autobuild_counter = 0;
        planet.morale = 500;
        planet.field_114 = 0;
        if (1..=8).contains(&n) {
            for e in 1..=8 {
                if self.alive[e] && e != n && self.knows[e][n] == 0 && self.planets[p].visibility[e] > 0 {
                    // Every human (multiplayer: each keeps its own contact list; diplomacy picks the ones that do).
                    if e == ctx.human_empire as usize || ctx.human_controlled[e] {
                        events.push(SideEvent::Met { who: e as u8, whom: new });
                    }
                    self.knows[e][n] = 1;
                }
            }
        }
        let planet = &mut self.planets[p];
        if n > 0 {
            for v in &mut planet.visibility[1..] {
                if *v < 0 {
                    *v = 0;
                }
            }
        }
        if !ctx.multiplayer && new != ctx.human_empire && planet.epidemic > 0 {
            if let Some(slot) = planet.events.iter_mut().find(|e| **e == colony::event::EPIDEMIC) {
                *slot = 0;
            }
            planet.epidemic = 0;
            events.push(SideEvent::VirusCleanup { planet: p });
        }
    }

    /// `BolygoFellazad(p, to, force)` (exe 0x53bc40), spec §3. Returns 1..=4.
    #[allow(clippy::too_many_arguments)]
    pub fn rebel(&mut self, p: usize, to: u8, force: bool, rules: &SideRules, ctx: &EconomyContext, rng: &mut Streams, events: &mut Vec<SideEvent>) -> i32 {
        let (owner, px, py, morale, ai_proof) = {
            let pl = &self.planets[p];
            (pl.owner as usize, pl.x, pl.y, pl.morale, pl.ai_proof)
        };
        if !(owner < 9 && !ai_proof) && !ctx.multiplayer {
            return 4;
        }
        rng.choice_begin();
        let mut choice = 0u8;
        for e in 1..=8usize {
            if e == owner || self.knows[owner][e] == 0 {
                continue;
            }
            let (mut count, mut sum) = (0.0f32, 0.0f32);
            for q in self.planets.iter().skip(1).filter(|q| q.owner as usize == e) {
                count += 1.0;
                sum += (py - q.y).abs() + (px - q.x).abs();
            }
            let avg = if count != 0.0 { sum / count } else { 0.0 };
            if self.alive[e] && count > 0.0 && rng.choice_offer(stream::GENERATION, (1000.0f32 / (avg + 1.0)) as i32) {
                choice = e as u8;
            }
        }
        let r = rng.range(stream::REBELLION_VIRUS, 1, 100);
        if r < morale / 20 && !force {
            let pl = &mut self.planets[p];
            pl.morale_penalty = 400;
            pl.morale_penalty_rate = 1;
            return 4;
        }
        let mut k = rng.range(stream::REBELLION_VIRUS, 1, 3);
        let new = if choice == 0 {
            k = 2;
            10
        } else if k == 2 {
            10
        } else if k == 1 && to >= 1 {
            to
        } else {
            choice
        };
        self.transfer(p, new, rules, ctx, events);
        let result = if new == to && to > 0 { 1 } else { k };
        events.push(SideEvent::Rebelled { planet: p, result, new_owner: new });
        result
    }

    /// Colonisation arrival (spec §5): `BolygoElfoglal`, `Generate_Planet0(p, owner, race, false)`,
    /// `rndlog(1, 0, 0)` and a colony hub under construction. Returns the hub's slot (0 = no room).
    #[allow(clippy::too_many_arguments)]
    pub fn colonise(&mut self, p: usize, owner: u8, place: Placement, rules: &SideRules, ctx: &EconomyContext, rng: &mut Streams, events: &mut Vec<SideEvent>) -> usize {
        self.transfer(p, owner, rules, ctx, events);
        let race = ctx.empire_race[owner as usize];
        let pop = rng.range(stream::GENERATION, 2900, 3100);
        let pl = &mut self.planets[p];
        pl.owner = owner;
        pl.race = race;
        pl.population = pop;
        pl.population_change = 0;
        pl.morale = 500;
        pl.tax_level = 3;
        (pl.gdp_acc, pl.gdp_last, pl.tax_acc, pl.tax_last) = (0, 0, 0, 0);
        (pl.field_58, pl.field_5c, pl.field_54) = (0, 0, 0);
        (pl.autobuild_mode, pl.autobuild_aggr, pl.autobuild_counter) = (1, 2, 0);
        pl.production = rules.production_default;
        (pl.ship_capacity, pl.vehicle_capacity) = (0, 0);
        pl.visibility[owner as usize] = 1000;
        let damage = rng.range(stream::GENERATION, 0, 0);
        self.place_building(p, kind::COLONY_HUB as u8, false, damage, place, rules, ctx)
    }
}

impl ColonyWorld {
    /// Terraforming arrival (spec §5.1) on unowned planet `p` by `owner`: the new surface and colony map.
    pub fn terraform(&mut self, p: usize, owner: u8, rules: &SideRules, maps: &PlacementRules) -> (u8, u8) {
        // The affinity row is the owner's empire number, not its race (original quirk).
        let row = rules.colony.surface_growth.get(owner as usize).copied().unwrap_or([0; 9]);
        let mut surface = 1;
        for s in 2..=8 {
            if row[s] > row[surface] {
                surface = s;
            }
        }
        let map = self.next_colony_map(surface as u8 - 1, maps);
        let pl = &mut self.planets[p];
        pl.surface = surface as u8;
        pl.colony_map = map;
        (surface as u8, map)
    }

    /// `NextColonyMap(kind)` (exe 0x486920, spec §5.1): the next terrain of that type, round-robin.
    pub fn next_colony_map(&mut self, kind: u8, maps: &PlacementRules) -> u8 {
        let count = maps.terrains.len();
        let is = |i: usize| maps.terrains[i].kind == kind;
        let k = kind as usize;
        let from = self.map_cursor[k] as usize;
        let u = (from..count).find(|&i| is(i)).or_else(|| (0..count).find(|&i| is(i))).unwrap_or(count);
        self.map_cursor[k] = if u + 1 != count { (u + 1) as u8 } else { 0 };
        if u == 60 { 63 } else { u as u8 }
    }
}

/// The colony side of fleet arrivals (`fleet-movement.md` §4.3 cases 1–2), run inside the movement loop
/// by [`crate::fleet::Starmap::tick`]: colonisation with the colony-map search, terraforming.
pub struct ColonyArrivals<'a> {
    pub world: &'a mut ColonyWorld,
    pub rules: &'a SideRules,
    pub ctx: &'a EconomyContext,
    pub placement: &'a PlacementRules,
    /// The cells of planet `p`'s ground-war units on the map ([`crate::save::SaveState::vehicle_cells`]).
    pub vehicles: &'a dyn Fn(usize) -> Vec<(i32, i32)>,
    pub events: Vec<SideEvent>,
}

impl Arrivals for ColonyArrivals<'_> {
    fn colonise(&mut self, p: usize, owner: u8, star: &mut StarPlanet, rng: &mut Streams) {
        let (placement, vehicles) = (self.placement, self.vehicles);
        let mut place = |pl: &Planet, p: usize, k: u8| placement.place(pl, k, &vehicles(p));
        self.world.colonise(p, owner, &mut place, self.rules, self.ctx, rng, &mut self.events);
        // The starmap's view of the planet.
        let pl = &self.world.planets[p];
        star.owner = pl.owner;
        star.visible = pl.visibility;
        star.epidemic = pl.epidemic;
        star.has_hub = pl.buildings.iter().any(|b| b.kind == kind::COLONY_HUB as u8 && b.work_left == 0);
    }

    fn terraform(&mut self, p: usize, owner: u8) -> u8 {
        self.world.terraform(p, owner, self.rules, self.placement).0
    }

    fn trader_paid(&mut self, e: u8, amount: i32) {
        let e = e as usize;
        self.world.add_money(e, 0, amount as f64, self.rules, self.ctx);
        let t = &mut self.world.empires.trade_this_year[e];
        *t = t.wrapping_add(amount);
    }
}

/// What autobuild does with a pick of building 0 (category 1 without a researched power plant). The
/// original then tests `inactive[0]`, a stack slot it never initialises (spec §6.1), so both outcomes
/// occur in the GOG exe depending on what an earlier call left there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ZeroPick {
    /// The slot was positive: the pick scores 0 (no purchase). The sane choice, and the default.
    #[default]
    Skip,
    /// The slot was ≤ 0: building 0 is bought; it can't be placed, so the planet's counter becomes −1000.
    Attempt,
}

/// The best autobuild candidate of one empire in this step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Pick {
    kind: i32,
    score: i32,
    planet: usize,
}

/// Prologue snapshots read by autobuild (spec §7).
struct Before {
    lab_buildings: [[i32; 4]; EMPIRES],
    spy_centres: [i32; 9],
    trade_ports: [i32; 9],
}

impl ColonyWorld {
    /// Prologue resets (spec §7), returning the snapshots autobuild reads.
    fn reset_totals(&mut self, ctx: &EconomyContext) -> Before {
        let before = Before {
            lab_buildings: self.empires.lab_buildings,
            spy_centres: self.empires.spy_centres,
            trade_ports: self.empires.trade_ports,
        };
        let t = &mut self.empires;
        for e in 1..EMPIRES {
            for i in 1..=3 {
                t.labs[e][i] = if e == ctx.human_empire as usize && !ctx.multiplayer { ctx.story.lab_start[i - 1] } else { 0 };
                t.lab_buildings[e][i] = 0;
            }
        }
        for e in 1..9 {
            t.spy_capacity[e] = 1;
            t.spy_centres[e] = 1;
            t.trade_ports[e] = 0;
        }
        let s = &mut self.strength;
        for p in 1..PLANET_SLOTS {
            (s.ground[p], s.fortress[p], s.ion_gun[p], s.fire_brigade[p]) = (0, 0, 0, 1);
        }
        before
    }

    /// Per-planet totals (spec §7), with the owner after any rebellion.
    fn add_totals(&mut self, p: usize, fx: &Effects, rules: &SideRules, ctx: &EconomyContext) {
        let owner = self.planets[p].owner as usize;
        let tech = |t: usize| rules.colony.buildings[t].tech as usize;
        let t = &mut self.empires;
        for i in 1..=3 {
            let lab = 13 + i;
            if ctx.story.lab_bonus_empire == owner as i32 && !ctx.multiplayer {
                t.labs[ctx.human_empire as usize][i] += fx.effect[lab];
            }
            t.labs[owner][i] += fx.effect[lab];
            t.lab_buildings[owner][i] += fx.active[lab] + fx.inactive[lab];
        }
        let s = &mut self.strength;
        for k in 28..=43 {
            s.ground[p] += (fx.active[k] + fx.inactive[k]) * rules.firepower[tech(k)];
        }
        for k in 32..=35 {
            s.fortress[p] += fx.active[k] * rules.tech_hp[tech(k)];
        }
        for k in 28..=31 {
            s.ion_gun[p] += fx.active[k] * rules.tech_hp[tech(k)];
        }
        s.fire_brigade[p] = 8 * fx.active[kind::FIRE_BRIGADE] + 1;
        if owner < 9 {
            let spies = fx.active[kind::SPY_CENTRE] + fx.inactive[kind::SPY_CENTRE];
            t.spy_capacity[owner] = (t.spy_capacity[owner] + fx.effect[kind::SPY_CENTRE]).min(12);
            t.spy_centres[owner] += spies;
            t.spy_centres_by_planet[owner][p] += spies;
            t.trade_ports[owner] += fx.active[kind::TRADE_PORT] + fx.inactive[kind::TRADE_PORT];
        }
    }

    /// Autobuild scoring for one planet (spec §6.1); `targets` are this step's problem targets ×10000.
    #[allow(clippy::too_many_arguments)]
    fn autobuild_score(&mut self, p: usize, fx: &Effects, targets: &[i32; colony::problem::COUNT], before: &Before, best: &mut [Pick; EMPIRES], rules: &SideRules, ctx: &EconomyContext) {
        let pl = &self.planets[p];
        let (mode, aggr) = (pl.autobuild_mode as usize, pl.autobuild_aggr as i32);
        if (ctx.year == 2327 && ctx.month == 3) || mode == 0 || aggr == 0 {
            return;
        }
        let owner = pl.owner as usize;
        if fx.active[kind::COLONY_HUB] < 1 {
            if fx.inactive[kind::COLONY_HUB] == 0 {
                best[owner] = Pick { kind: 1, score: 2_000_000_000, planet: p };
            }
            return;
        }
        let ab = &rules.autobuild;
        let tech = |t: usize| rules.colony.buildings[t].tech as usize;
        for c in 1..CATEGORIES {
            let range = ab.min[c].max(0) as usize..=ab.max[c].max(0) as usize;
            self.autobuild_pick[c] =
                range.rev().find(|&t| self.tech.status(tech(t), owner) == status::OWNED).unwrap_or(0) as i32;
        }
        let pick = self.autobuild_pick;
        let mut have = [0i32; 17];
        for (c, h) in have.iter_mut().enumerate().skip(1) {
            *h = (ab.min[c].max(0) as usize..=ab.max[c].max(0) as usize).map(|t| fx.active[t] + fx.inactive[t]).sum();
        }
        let empty = AutobuildMode::default();
        let m = ab.modes.get(mode).unwrap_or(&empty);
        let at = |a: &[i32; 17], c: i32| a.get(c as usize).copied().unwrap_or(0);
        let mut need = [0i32; 17];
        let mut j = 1;
        let last = loop {
            let mut c = m.entry(j);
            if c > 0 && (c as usize) < need.len() {
                need[c as usize] += 1;
            }
            let last = j;
            j += 1;
            if c == 16 && owner < 9 && before.trade_ports[owner] > 0 {
                c = 0;
            }
            let next = m.entry(j);
            let more = j < 51 && next > 0 && (at(&have, c) >= at(&need, c) || pick[c as usize] == 0);
            if !more {
                break last;
            }
        };
        let mut want = m.entry(last);
        if at(&have, want) < at(&need, want) {
            if (5..=7).contains(&want) {
                let l = &before.lab_buildings[owner];
                want = 5;
                if l[2] * 18 < l[1] * 10 {
                    want = 6;
                }
                if l[3] * 18 < l[1] * 10 && l[3] * 10 < l[2] * 10 {
                    want = 7;
                }
            }
            if want == 13 && owner < 9 && before.spy_centres[owner] > 11 {
                want = 0;
            }
        } else {
            want = 0;
        }

        let (mut best_c, mut best_s) = (0usize, 0i32);
        if fx.energy_demand < fx.energy_produced {
            let mut lim = m.problem_threshold;
            if aggr == 1 {
                lim = (lim as f64 * 1.5) as i32;
            }
            if aggr == 3 {
                lim = (lim as f64 * 0.66) as i32;
            }
            for (c, &b) in pick.iter().enumerate().skip(1) {
                let k = ab.problem[c];
                if k < 0 || b <= 0 {
                    continue;
                }
                let (k, b) = (k as usize, b as usize);
                let level = pl.problems.get(k).copied().unwrap_or(0);
                let ok = (k == 0 || (lim * 10000 <= level && lim * 10000 <= targets[k]))
                    && (k > 0 || (pl.workers_demand + rules.colony.buildings[b].workers <= pl.population && c as i32 == want))
                    && fx.inactive[b] == 0;
                if !ok {
                    continue;
                }
                let mut n = if k == 10 {
                    fx.inactive[10..=13].iter().sum()
                } else {
                    (ab.min[c].max(0) as usize..=ab.max[c].max(0) as usize).map(|t| fx.inactive[t]).sum::<i32>()
                };
                n += fx.under_construction[1..=51].iter().sum::<i32>();
                if n != 0 || (k == 10 && fx.effect[b] >= 4) {
                    continue;
                }
                let mut s = if k == 0 { m.listed_priority * 10000 / 10 } else { level.checked_div(lim).unwrap_or(0) };
                if k == 10 {
                    s = (s as f32 / (3.0f32 - 2.0 / (fx.effect[b] as f32 + 1.0))) as i32;
                }
                if best_s < s {
                    best_s = s;
                    best_c = c;
                }
            }
        } else {
            best_c = 1;
            best_s = 1_000_000;
        }
        // With no power-plant tech, category 1's building is 0 and the original reads `inactive[0]`, a
        // stack slot it never zeroes (it clears types 1..=55): see [`ZeroPick`].
        let b = pick[best_c].max(0) as usize;
        if best_c == 0 || fx.inactive[b] > 0 || (b == 0 && self.zero_pick == ZeroPick::Skip) {
            best_s = 0;
        }
        let score = (pl.autobuild_counter / 20 + (aggr + 2) * 10).wrapping_mul(best_s);
        if best[owner].score < score {
            best[owner] = Pick { kind: pick[best_c], score, planet: p };
        }
    }

    /// The autobuild purchases of the epilogue (spec §6.2).
    fn autobuild_buy(&mut self, best: &[Pick; EMPIRES], place: Placement, rules: &SideRules, ctx: &EconomyContext, events: &mut Vec<SideEvent>) {
        for (e, b) in best.iter().enumerate().skip(1) {
            if b.score < 1 {
                self.money[e].reserve[2] = 0.0;
                continue;
            }
            let t = rules.colony.buildings[b.kind as usize].tech as usize;
            let cost = price(&self.tech, ctx.empire_race[e], t);
            if cost as f64 <= self.money[e].available(2) {
                let kind = b.kind as u8;
                // Building 0 has no footprint (the search reads before the table) and never fits.
                let slot = if kind == 0 { 0 } else { self.place_building(b.planet, kind, false, 0, place, rules, ctx) };
                if slot == 0 {
                    self.planets[b.planet].autobuild_counter = -1000;
                    events.push(SideEvent::AutobuildNoRoom { empire: e as u8, planet: b.planet, kind });
                } else {
                    self.add_money(e, 2, -(cost as f64), rules, ctx);
                    self.planets[b.planet].autobuild_counter = 0;
                    if b.kind as usize == kind::TRADE_PORT {
                        let owner = self.planets[b.planet].owner as usize;
                        self.empires.bump_trade_ports(owner);
                    }
                    events.push(SideEvent::AutobuildPlaced { empire: e as u8, planet: b.planet, kind, slot });
                }
                self.money[e].reserve[2] = 0.0;
            } else {
                self.money[e].reserve[2] = (cost + 100) as f64;
            }
        }
    }

    /// The display refresh of every planet, `FRISSIT_Planet(false, 0)` (colony-economy.md §3): the prologue's
    /// resets and global-morale decay, then each owned planet's building pass (§4), production (§8) and totals.
    pub fn display_refresh(&mut self, rules: &SideRules, ctx: &mut EconomyContext) {
        self.reset_totals(ctx);
        colony::decay_global_morale(ctx);
        for p in 1..self.planets.len() {
            if self.planets[p].owner == 0 {
                continue;
            }
            let human = ctx.human_controlled[self.planets[p].owner as usize];
            let fx = colony::building_pass(&mut self.planets[p], &rules.colony, human);
            colony::production_capacity(&mut self.planets[p], &fx, &rules.colony, ctx);
            self.occupied[p] = fx.occupied;
            self.add_totals(p, &fx, rules, ctx);
        }
    }

    /// The whole economy step `FRISSIT_Planet(true, 0)` with its side systems (colony-economy.md §3,
    /// colony-side.md §10). Reports are index-aligned with `planets`.
    pub fn economy_step(&mut self, rules: &SideRules, ctx: &mut EconomyContext, rng: &mut Streams, place: Placement) -> (Vec<PlanetReport>, Vec<SideEvent>) {
        let monthly = ctx.day == 1;
        let mut events = Vec::new();
        if monthly {
            self.empires.full_gdp = [0; EMPIRES];
            self.empires.full_tax = [0; EMPIRES];
        }
        let before = self.reset_totals(ctx);
        colony::decay_global_morale(ctx);
        let mut best = [Pick::default(); EMPIRES];
        let mut reports = vec![PlanetReport::default(); self.planets.len()];
        for (p, slot) in reports.iter_mut().enumerate().skip(1) {
            if self.planets[p].owner == 0 {
                continue;
            }
            let (fx, report) = colony::economy_head(&mut self.planets[p], &rules.colony, ctx);
            self.occupied[p] = fx.occupied;
            if report.events.contains(&ColonyEvent::Rebellion) {
                events.push(SideEvent::Revolt { planet: p, owner: self.planets[p].owner });
                let result = self.rebel(p, 0, true, rules, ctx, rng, &mut events);
                let _ = result;
            }
            let report = colony::economy_tail(p as i32, &mut self.planets[p], &fx, report, &rules.colony, ctx, rng, monthly);
            self.needed_morale[p] = report.needed_morale;
            let owner = self.planets[p].owner as usize;
            if monthly {
                self.empires.full_gdp[owner] += report.gdp;
                self.empires.full_tax[owner] += report.tax_paid;
                self.add_money(owner, 0, report.tax_paid as f64, rules, ctx);
            }
            self.autobuild_score(p, &fx, &report.targets, &before, &mut best, rules, ctx);
            self.add_totals(p, &fx, rules, ctx);
            for ev in &report.events {
                match ev {
                    ColonyEvent::Rebellion => {}
                    ColonyEvent::Abandoned => {
                        if best[owner].planet == p {
                            best[owner] = Pick::default();
                        }
                        events.push(SideEvent::Abandoned { planet: p, owner: owner as u8 });
                        self.transfer(p, 0, rules, ctx, &mut events);
                    }
                    other => events.push(SideEvent::Colony { planet: p, event: *other }),
                }
            }
            *slot = report;
        }
        self.autobuild_buy(&best, place, rules, ctx, &mut events);
        if monthly {
            let t = &mut self.empires;
            for e in 1..EMPIRES {
                t.tax_this_year[e] += t.full_tax[e];
                if ctx.month == 1 {
                    t.tax_last_year[e] = t.tax_this_year[e];
                    t.tax_this_year[e] = 0;
                    t.trade_last_year[e] = t.trade_this_year[e];
                    t.trade_this_year[e] = 0;
                }
            }
            if ctx.month == 1 {
                events.push(SideEvent::YearClosed);
            }
        }
        self.mark_seen_buildings(rules);
        (reports, events)
    }

    /// Spec §9: every building on a planet where an empire has a spy is "seen" by it.
    fn mark_seen_buildings(&mut self, rules: &SideRules) {
        for pl in self.planets.iter().skip(1).filter(|pl| pl.owner != 0) {
            for i in 1..EMPIRES {
                if !(i == 7 || pl.spies[i] != 0) {
                    continue;
                }
                for b in pl.buildings.iter().filter(|b| b.kind != 0) {
                    self.tech.mark_seen(rules.colony.buildings[b.kind as usize].tech as usize, i, pl.owner as usize);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placement::Terrain;

    #[test]
    fn colony_maps_rotate_per_surface_kind_and_skip_map_60() {
        let kinds = [0, 1, 0, 1, 1];
        let mut maps = PlacementRules {
            terrains: kinds.iter().map(|&kind| Terrain { mask: [0; 650], kind, start: (100, 100) }).collect(),
            ..Default::default()
        };
        let mut w = ColonyWorld::default();
        let picks: Vec<u8> = (0..4).map(|_| w.next_colony_map(1, &maps)).collect();
        assert_eq!(picks, [1, 3, 4, 1], "round-robin; the last map wraps the cursor to 0");
        assert_eq!((w.next_colony_map(0, &maps), w.next_colony_map(0, &maps), w.next_colony_map(0, &maps)), (0, 2, 0));
        maps.terrains = (0..62).map(|i| Terrain { mask: [0; 650], kind: (i == 60) as u8 + 2, start: (100, 100) }).collect();
        assert_eq!(w.next_colony_map(3, &maps), 63, "map 60 is replaced by 63");
        assert_eq!(w.map_cursor[3], 61);
    }

    #[test]
    fn terraforming_uses_the_owner_number_as_the_affinity_row() {
        let mut rules = SideRules::default();
        rules.colony.surface_growth = vec![[0; 9]; 4];
        rules.colony.surface_growth[3] = [9, 1, 5, 7, 7, 2, 0, 0, 0]; // surface 0 isn't a candidate; first max wins
        let maps = PlacementRules {
            terrains: (0..4).map(|kind| Terrain { mask: [0; 650], kind, start: (100, 100) }).collect(),
            ..Default::default()
        };
        let mut w = ColonyWorld { planets: vec![Planet::default(); 2], ..Default::default() };
        assert_eq!(w.terraform(1, 3, &rules, &maps), (3, 2), "surface 3 → a map of kind 2");
        assert_eq!((w.planets[1].surface, w.planets[1].colony_map, w.planets[1].owner), (3, 2, 0));
    }

    fn treasury() -> Treasury {
        let mut t = Treasury { share: [0.0, 0.5, 0.3, 0.2, 0.0, 0.0], ..Default::default() };
        t.pools[1..4].copy_from_slice(&[1000.0, 500.0, 200.0]);
        t.resum();
        t
    }

    #[test]
    fn spending_from_a_pool_takes_its_own_money_first() {
        let mut t = treasury();
        assert_eq!(t.available(2), 1700.0);
        assert!(t.add(2, -300.0, None));
        assert_eq!(t.pools[1..4], [1000.0, 200.0, 200.0]);
        assert!(t.add(2, -400.0, None));
        // 200 from pool 2, the other 200 shared by the surplus of pools 1 and 3 (1000 : 200).
        assert_eq!(t.pools[2], 0.0);
        assert!((t.pools[1] - (1000.0 - 200.0 * 1000.0 / 1200.0)).abs() < 1e-9);
        assert!(!t.add(2, -5000.0, None), "not enough money");
    }

    #[test]
    fn income_is_shared_and_scaled_for_the_ai() {
        let mut t = treasury();
        assert!(t.add(0, 100.0, Some(1.5)));
        assert_eq!(t.pools[1..4], [1075.0, 545.0, 230.0]);
        assert_eq!(t.total, 1075.0 + 545.0 + 230.0);
    }

    #[test]
    fn overspending_the_whole_treasury_books_a_debt() {
        let mut t = treasury();
        assert!(!t.add(0, -2700.0, None));
        assert_eq!(t.pools[1..4], [-500.0, -300.0, -200.0]);
    }

    #[test]
    fn autobuild_mode_names_map_to_categories() {
        let names: Vec<String> = ["", "A", "B", "C"].iter().map(|s| s.to_string()).collect();
        let entries: Vec<String> = ["B", "X", "A", "END", "C"].iter().map(|s| s.to_string()).collect();
        let m = AutobuildMode::from_entries(10, 30, &entries, &names);
        assert_eq!(m.list, vec![2, 1]);
        assert_eq!(m.entry(1), 2);
        assert_eq!(m.entry(3), 0);
    }

    #[test]
    fn tech_status_follows_prerequisites() {
        let mut t = TechTable::default();
        let o = 5 * TECH_STRIDE + 0x2d;
        t.bytes[o] = 4; // tech 5 needs tech 4
        for x in 1..TECHS {
            t.set_status(x, 1, status::UNKNOWN);
        }
        t.recompute(1, false);
        assert_eq!(t.status(5, 1), status::UNKNOWN);
        assert_eq!(t.status(4, 1), status::RESEARCHABLE);
        t.set_status(4, 1, status::OWNED);
        t.recompute(1, false);
        assert_eq!(t.status(5, 1), status::RESEARCHABLE);
    }
}
