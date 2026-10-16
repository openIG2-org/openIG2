//! Campaign scripting: the story engine (`Story_Check`, exe 0x5c5610; `FRISSIT_Story`, 0x5c1ff0; the story
//! queues, variables, logic flags, counters, planet ids, event flags and the undo list) with its hooks
//! (`Story_Special`, `Story_FleetArrivedSave`, …) and the scenario stories 360..375 (`st_Scen_include`,
//! 0x601580). Spec: `re/rules/campaign.md`.
//!
//! The original stories are C code; here each script is a [`Script`] run by [`Story::check`] in the exe's
//! order. Only [`ScenarioStories`] is ported; the campaign's scripts (§8) are not yet. The world the scripts
//! read comes through [`StoryWorld`], and what they do to it (the game won, an empire dies, info lines, undo
//! actions that reach other systems, fleet orders) goes out as [`StoryEvent`]s. Randomness: stream 5
//! (`st_bSearch`, `st_fSearch`).

use crate::rng::{stream, Streams};

/// `rndlog(5, …)`.
pub const STREAM: usize = stream::STORY;
/// Story buckets (`StoryVodor[256]`, by `id & 0xff`).
pub const BUCKETS: usize = 256;
/// Event flags (`StoryEsemeny[17]`).
pub const FLAGS: usize = 17;
/// `StoryAdat` size.
pub const ADAT: usize = 0x128;
/// Table capacities (`st_*SearchCreate`).
pub const MAX_COUNTERS: usize = 300;
pub const MAX_LOGIC: usize = 150;
pub const MAX_VARS: usize = 500;
/// `st_Ozonviz` adds entries while the list is shorter than this.
pub const MAX_UNDO: usize = 299;
/// Ticks per day (`Ido` units): counters run in days × 50.
pub const DAY: i32 = 50;

/// `StoryEsemeny` indices.
pub mod flag {
    pub const ARRIVED: usize = 0;
    pub const SPACE_START: usize = 1;
    pub const GROUND_START: usize = 2;
    pub const SPACE_END: usize = 3;
    pub const GROUND_END: usize = 4;
    pub const COUNTER: usize = 5;
    pub const MESSAGE: usize = 6;
    pub const PROMOTED: usize = 7;
    pub const DISABLED: usize = 8;
    pub const READ: usize = 9;
    pub const RADAR_FLEET: usize = 10;
    pub const RADAR_PLANET: usize = 11;
    pub const SIGNAL: usize = 12;
    pub const DIPLOMACY: usize = 13;
    pub const SPECIAL: usize = 14;
    pub const SPY: usize = 15;
    pub const DAILY: usize = 16;
}

/// `StoryAdat` offsets the engine itself touches.
pub mod adat {
    /// Set by the epidemic stories; cleared when it is over (spec §4 step 3).
    pub const EPIDEMIC: usize = 0x30;
    /// Cleared by every battle hook.
    pub const BATTLE_88: usize = 0x88;
    /// A space battle's no-retreat switches (story 372 sets them).
    pub const NO_RETREAT_94: usize = 0x94;
    pub const NO_RETREAT_98: usize = 0x98;
}

/// `StoryVodorClass`: parallel `(id, value)` arrays (spec §2.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Queue {
    pub entries: Vec<(i32, i32)>,
}

impl Queue {
    pub fn add(&mut self, id: i32, value: i32) {
        self.entries.push((id, value));
    }

    /// `Remove(x)`: the first entry whose id is `x` or `−x`.
    pub fn remove(&mut self, id: i32) {
        if let Some(k) = self.entries.iter().position(|&(e, _)| e == id || e == id.wrapping_neg()) {
            self.entries.remove(k);
        }
    }

    pub fn remove_first(&mut self) {
        if !self.entries.is_empty() {
            self.entries.remove(0);
        }
    }

    pub fn contains(&self, id: i32) -> bool {
        self.entries.iter().any(|&(e, _)| e == id)
    }

    /// `GetDual(x)`: the value of the first entry with id `x`, else 0.
    pub fn value(&self, id: i32) -> i32 {
        self.entries.iter().find(|&&(e, _)| e == id).map_or(0, |&(_, v)| v)
    }

    pub fn first(&self) -> Option<(i32, i32)> {
        self.entries.first().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A 1-based `{id, value}` table with a count (`sSzamlalok`, `sLogikaiak`, `sValtozok`). Slots past the count
/// keep whatever they held: a slot `st_szStop` creates keeps a stale id (spec §2.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Slots 0..=capacity; slot 0 is unused.
    pub slots: Vec<(i32, i32)>,
    pub count: usize,
}

impl Table {
    pub fn new(capacity: usize) -> Self {
        Self { slots: vec![(0, 0); capacity + 1], count: 0 }
    }

    pub fn capacity(&self) -> usize {
        self.slots.len() - 1
    }

    /// The live entries 1..=count.
    pub fn live(&self) -> &[(i32, i32)] {
        &self.slots[1..=self.count]
    }

    /// Replace the live entries (slots past them are left as they are).
    pub fn set_live(&mut self, entries: &[(i32, i32)]) {
        self.slots[1..=entries.len()].copy_from_slice(entries);
        self.count = entries.len();
    }

    /// `st_*Search`: the first slot 1..=count with `id`.
    pub fn find(&self, id: i32) -> Option<usize> {
        (1..=self.count).find(|&i| self.slots[i].0 == id)
    }

    /// `st_*SearchCreate`: an existing slot, else a new one while there is room (the id isn't written).
    fn find_or_create(&mut self, id: i32) -> Option<usize> {
        self.find(id).or_else(|| {
            (self.count < self.capacity()).then(|| {
                self.count += 1;
                self.count
            })
        })
    }

    pub fn get(&self, id: i32) -> Option<i32> {
        self.find(id).map(|i| self.slots[i].1)
    }
}

/// A space-battle participant as `Story_SpaceBattleFinished` gets it (`spwsflotta`, 0x30 bytes).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Participant {
    /// `+0x18`.
    pub owner: i32,
    /// `+0x20`.
    pub slot_id: i32,
    /// `+0x24`: the fleet's story id.
    pub fleet_id: i32,
}

/// The arriving fleet (`Story_FleetArrivedSave`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Arrival {
    /// `ErkezettFlotta`: the fleet's story id.
    pub fleet_id: i32,
    /// `ErkezettFlottaNumber`: its slot.
    pub fleet: i32,
    /// `FleetArrivedCel`: the target (planet or fleet slot).
    pub target: i32,
    /// `FleetArrivedCelTipus`: 0 a point, 1 a planet, 2 a fleet.
    pub target_type: i32,
    /// `FleetArrivedCelFlottaAzonosito`.
    pub target_fleet_id: i32,
}

/// The ground battle (`Story_GroundBattle*`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GroundBattle {
    pub fleet: i32,
    pub fleet_id: i32,
    pub fleet_owner: i32,
    pub planet: i32,
    pub old_owner: i32,
    pub attacker_won: bool,
    pub defender_won: bool,
}

/// What the scripts do to the rest of the game.
#[derive(Debug, Clone, PartialEq)]
pub enum StoryEvent {
    /// `MEGNYERES(1)`: the human wins.
    Won,
    /// `MEGHALAS(e)` (`DIE()` passes `EN`).
    EmpireDies(i32),
    /// `AddGameInfo(id, text)`: a starmap info line (the scenario countdown passes a number).
    GameInfo { id: i32, value: i32 },
    /// `RemoveGameInfo(id)`.
    RemoveGameInfo(i32),
    /// An undo action of a finished story that reaches another system (types 1–12, spec §2.3).
    Undo { kind: i32, value: i32 },
    /// A pending `last_addship` (`st_fAddShip(last_addship)`).
    AddShip(i32),
    /// `StoryAdat+0x30` cleared: remove info 0x65 from every planet that has it.
    EpidemicOver,
    /// `FlottaMozgat(f, 0, 0, 0, 0)`: the fleet stops.
    StopFleet(i32),
    /// `st_AddFleetToSpWar(f)`: the fleet joins the starting battle.
    JoinBattle(i32),
}

/// The world as the story engine and the scenario stories read it. Indices are the original's: planets and
/// fleets 1-based with slot 0 readable (a failed search returns 0 and the scripts read slot 0).
pub trait StoryWorld {
    fn ido(&self) -> i32;
    /// `IdoSpeedSzorzo`.
    fn speed_mult(&self) -> i32;
    /// `EN`.
    fn human(&self) -> i32;
    /// `Nehezseg`.
    fn difficulty(&self) -> i32;
    fn multiplayer(&self) -> bool;
    fn rng(&mut self) -> &mut Streams;
    /// `HanyBolygo`.
    fn planet_count(&self) -> i32;
    /// `+0x20`.
    fn planet_owner(&self, p: i32) -> i32;
    /// `+0x28` (story 369's threshold).
    fn planet_value(&self, p: i32) -> i32;
    /// `+0xd8[EN]`: the human's view of the planet (−1 unknown).
    fn planet_visibility(&self, p: i32) -> i32;
    /// `OsszHanyFlotta`.
    fn fleet_count(&self) -> i32;
    /// `+0x15` (0xff = a free slot).
    fn fleet_owner(&self, f: i32) -> i32;
    /// `+0x16`: the story id.
    fn fleet_id(&self, f: i32) -> i32;
    /// `BirodalomEl[e]`.
    fn empire_alive(&self, e: i32) -> bool;
    /// `BirodalomHanyBolygo[e]`.
    fn planets_of(&self, e: i32) -> i32;
    /// `MoneyMennyi(e, 0)`.
    fn money(&self, e: i32) -> f64;
    /// `Talalmany[tech].+0x45 + e`.
    fn tech_status(&self, tech: i32, e: i32) -> i32;
    /// `Viszony[a][b].+0x25`.
    fn relation_level(&self, a: i32, b: i32) -> i32;
    /// Planet `+0xb8 > 0` (an epidemic is running there).
    fn planet_infected(&self, _p: i32) -> bool {
        false
    }
    /// A live fleet (owner ≠ 0xff) with `+0x52` bit 2 (it carries the epidemic).
    fn fleet_infected(&self, _f: i32) -> bool {
        false
    }
    fn event(&mut self, ev: StoryEvent);
}

/// One `st_*_include` script.
pub trait Script {
    fn run(&self, story: &mut Story, world: &mut dyn StoryWorld);
}

/// The story engine's state (spec §2).
#[derive(Debug, Clone, PartialEq)]
pub struct Story {
    /// `StoryVodor`: the running stories with their start flags.
    pub buckets: Vec<Queue>,
    /// `HistoryVodor`: `(id, Ido)` of the finished stories.
    pub history: Queue,
    /// `StartVodor`: stories waiting for the next top-level check.
    pub starting: Queue,
    /// `VegzesVodor`: "finished" signals waiting.
    pub finished: Queue,
    /// `TalalmanyVodor`: `(type, data1)` specials waiting.
    pub specials: Queue,
    /// `OlvasasVodor`: message blocks read.
    pub read: Queue,
    /// `sBolygok`: `(id, planet)` (1-based in the original; no capacity check).
    pub planets: Vec<(i32, u8)>,
    pub counters: Table,
    pub logic: Table,
    pub vars: Table,
    /// `StoryError`.
    pub error: i32,
    /// `StoryEsemeny`.
    pub flags: [bool; FLAGS],
    /// `StoryLastChecked`.
    pub last_checked: i32,
    /// `SzamlaloMelyikVegzett`.
    pub counter_done: i32,
    /// `(StorySignalMelyik, StorySignalTipus)`.
    pub signal: (i32, i32),
    /// `FRISSIT_Story::OldIdo`.
    pub old_ido: i32,
    pub delayed: bool,
    /// `Story_SkipFirst`.
    pub skip_first: i32,
    /// `StoryAktKuldetesNowReStarting`.
    pub restarting: i32,
    /// `Story_check_locked` (never set by the exe).
    pub locked: bool,
    /// `Story_Check::ent_count`.
    pub depth: i32,
    /// `last_addship`.
    pub last_addship: i32,
    pub adat: [u8; ADAT],
    /// `st_vOzonviz`: `(type, a, owner)`.
    pub undo: Vec<(i32, i32, i32)>,
    /// `SpecialType`, `SpecialData1..7`.
    pub special: [i32; 8],
    /// `ArrivedToRadarBolygo`, `ArrivedToRadarFleet`.
    pub radar: (i32, i32),
    /// `StoryDiplomaciaMelyik`, `…Birodalom`, `…Valasz`.
    pub diplomacy: (i32, i32, i32),
    pub arrival: Arrival,
    pub ground: GroundBattle,
    /// `SpWarYouWasInBattle`.
    pub you_were_in_battle: bool,
    /// `SpwFlottak[..SpwFlottaNum]`.
    pub battle: Vec<Participant>,
    /// `SpWarKiNyert`: the winning owner.
    pub battle_winner: i32,
    /// `KemkedesTipus`.
    pub spy_kind: i32,
    /// `Story_SpWarTiltas`, `Story_SpWarSimulateTiltas`, `Story_GrWarTiltas`, `Story_GrWarSimulateTiltas`.
    pub space_forbidden: bool,
    pub space_simulate_forbidden: bool,
    pub ground_forbidden: bool,
    pub ground_simulate_forbidden: bool,
    /// `StoryUrharcViszony[19]`, `StoryUrharcVisszavonulhate[19]`, `StoryUrharcZachkinBarkacs`.
    pub battle_pact: [i32; 19],
    pub battle_retreat: [i32; 19],
    pub zachkin: i32,
}

impl Default for Story {
    fn default() -> Self {
        Self {
            buckets: vec![Queue::default(); BUCKETS],
            history: Queue::default(),
            starting: Queue::default(),
            finished: Queue::default(),
            specials: Queue::default(),
            read: Queue::default(),
            planets: Vec::new(),
            counters: Table::new(MAX_COUNTERS),
            logic: Table::new(MAX_LOGIC),
            vars: Table::new(MAX_VARS),
            error: 0,
            flags: [false; FLAGS],
            last_checked: 0,
            counter_done: 0,
            signal: (0, 0),
            old_ido: 0,
            delayed: false,
            skip_first: 0,
            restarting: 0,
            locked: false,
            depth: 0,
            last_addship: 0,
            adat: [0; ADAT],
            undo: Vec::new(),
            special: [0; 8],
            radar: (0, 0),
            diplomacy: (0, 0, 0),
            arrival: Arrival::default(),
            ground: GroundBattle::default(),
            you_were_in_battle: false,
            battle: Vec::new(),
            battle_winner: 0,
            spy_kind: 0,
            space_forbidden: false,
            space_simulate_forbidden: false,
            ground_forbidden: false,
            ground_simulate_forbidden: false,
            battle_pact: [0; 19],
            battle_retreat: [0; 19],
            zachkin: 0,
        }
    }
}

const fn bucket(id: i32) -> usize {
    (id as u32 & 0xff) as usize
}

impl Story {
    // ---------------------------------------------------------------- StoryAdat
    pub fn adat_i32(&self, off: usize) -> i32 {
        i32::from_le_bytes(self.adat[off..off + 4].try_into().unwrap())
    }

    pub fn set_adat_i32(&mut self, off: usize, v: i32) {
        self.adat[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }

    // ---------------------------------------------------------------- stories (spec §2.1–§2.3)
    /// `st_Story(id, 1)`: is `id` running; when it is, `StoryLastChecked := id`.
    pub fn running(&mut self, id: i32) -> bool {
        let found = self.buckets[bucket(id)].contains(id);
        if found {
            self.last_checked = id;
        }
        found
    }

    /// `st_Story(id, 0)`.
    pub fn is_running(&self, id: i32) -> bool {
        self.buckets[bucket(id)].contains(id)
    }

    /// `st_StoryStart(0)`: the story being checked sees its first check.
    pub fn starting_now(&self) -> bool {
        let id = self.last_checked;
        self.buckets[bucket(id)].value(id) != 0
    }

    /// `StartStory(id, now)`.
    pub fn start_story(&mut self, id: i32, now: bool) -> bool {
        if id == 0 || self.running(id) {
            return false;
        }
        if now {
            self.buckets[bucket(id)].add(id, 1);
        } else {
            self.starting.add(id, 0);
        }
        true
    }

    /// `WIN(id, history)`: the story ends (spec §2.3). `id == 0` means the story being checked.
    pub fn win(&mut self, id: i32, history: bool, world: &mut dyn StoryWorld) {
        let id = if id == 0 { self.last_checked } else { id };
        self.running(id);
        for k in 0..self.undo.len() {
            let (kind, a, owner) = self.undo[k];
            if owner != id {
                continue;
            }
            match kind {
                0 | 12 => {}
                13 => {
                    let off = 0xd0usize.wrapping_add((a as usize).wrapping_mul(4));
                    if off + 4 <= ADAT && self.adat_i32(off) > 0 {
                        let v = self.adat_i32(off) - 1;
                        self.set_adat_i32(off, v);
                    }
                }
                14 => self.set_adat_i32(0x28, 0),
                15 => self.set_adat_i32(0x24, 0),
                k if (1..=11).contains(&k) => world.event(StoryEvent::Undo { kind, value: a }),
                _ => {}
            }
            self.undo[k] = (0, 0, 0);
        }
        while self.undo.last().is_some_and(|e| e.0 == 0) {
            self.undo.pop();
        }
        self.buckets[bucket(id)].remove(id);
        self.finished.add(id, 1);
        if history && !self.history.contains(id) {
            self.history.add(id, world.ido());
        }
    }

    /// `st_Ozonviz(type, a, owner)`: an undo action for `WIN(owner)`.
    pub fn add_undo(&mut self, kind: i32, a: i32, owner: i32) {
        if self.undo.len() >= MAX_UNDO {
            return;
        }
        match self.undo.iter().position(|e| e.0 == 0) {
            Some(k) => self.undo[k] = (kind, a, owner),
            None => self.undo.push((kind, a, owner)),
        }
    }

    /// `st_AddGameInfo(id, text, …, owner)` with the scenario countdown's number as the text.
    pub fn add_game_info(&mut self, id: i32, value: i32, owner: i32, world: &mut dyn StoryWorld) {
        let owner = if owner == -1 { id / 10 } else { owner };
        world.event(StoryEvent::GameInfo { id, value });
        let known = self.undo.contains(&(4, id, owner));
        if owner != 0 && !known {
            self.add_undo(4, id, owner);
        }
    }

    /// `st_RemoveGameInfo(id)`.
    pub fn remove_game_info(&mut self, id: i32, world: &mut dyn StoryWorld) {
        for e in self.undo.iter_mut() {
            if e.0 == 4 && e.1 == id {
                *e = (0, 0, 0);
            }
        }
        world.event(StoryEvent::RemoveGameInfo(id));
    }

    // ---------------------------------------------------------------- variables, flags, counters (§2.4, §2.5)
    pub fn v_set(&mut self, id: i32, v: i32) {
        match self.vars.find_or_create(id) {
            Some(i) => {
                self.vars.slots[i] = (id, v);
                self.error = 0;
            }
            None => self.error = 1,
        }
    }

    pub fn v_get(&mut self, id: i32) -> i32 {
        let v = self.vars.get(id);
        self.error = v.is_none() as i32;
        v.unwrap_or(0)
    }

    fn v_update(&mut self, id: i32, f: impl FnOnce(i32) -> i32) {
        let i = self.vars.find(id);
        if let Some(i) = i {
            self.vars.slots[i].1 = f(self.vars.slots[i].1);
        }
        self.error = i.is_none() as i32;
    }

    pub fn v_add(&mut self, id: i32, x: i32) {
        self.v_update(id, |v| v.wrapping_add(x));
    }

    pub fn v_sub(&mut self, id: i32, x: i32) {
        self.v_update(id, |v| v.wrapping_sub(x));
    }

    pub fn v_dec(&mut self, id: i32) {
        self.v_update(id, |v| v.wrapping_sub(1));
    }

    pub fn v_inc(&mut self, id: i32) {
        let v = self.v_get(id);
        self.v_set(id, v.wrapping_add(1));
    }

    fn v_test(&mut self, id: i32, f: impl FnOnce(i32) -> bool) -> bool {
        let v = self.vars.get(id);
        self.error = v.is_none() as i32;
        v.is_some_and(f)
    }

    pub fn v_is_equal(&mut self, id: i32, x: i32) -> bool {
        self.v_test(id, |v| v == x)
    }

    /// `st_vGreater`: `v ≥ x`.
    pub fn v_greater(&mut self, id: i32, x: i32) -> bool {
        self.v_test(id, |v| v >= x)
    }

    /// `st_vSmaller`: `v ≤ x`.
    pub fn v_smaller(&mut self, id: i32, x: i32) -> bool {
        self.v_test(id, |v| v <= x)
    }

    /// `st_lSetToTrue` / `st_lSetToFalse` (`st_lSearchCreate` writes the id itself).
    pub fn l_set(&mut self, id: i32, on: bool) {
        match self.logic.find_or_create(id) {
            Some(i) => {
                self.logic.slots[i] = (id, on as i32);
                self.error = 0;
            }
            None => self.error = 1,
        }
    }

    /// `st_lIsTrue` (= `st_lGet`).
    pub fn l_is_true(&mut self, id: i32) -> bool {
        let v = self.logic.get(id);
        self.error = v.is_none() as i32;
        v.is_some_and(|b| b & 1 != 0)
    }

    /// `st_lIsFalse`: also true when the flag doesn't exist.
    pub fn l_is_false(&mut self, id: i32) -> bool {
        let v = self.logic.get(id);
        self.error = v.is_none() as i32;
        v.is_none_or(|b| b & 1 == 0)
    }

    /// `st_szStart(id, days)`.
    pub fn sz_start(&mut self, id: i32, days: i32) {
        match self.counters.find_or_create(id) {
            Some(i) => {
                self.counters.slots[i] = (id, days.wrapping_mul(DAY));
                self.error = 0;
            }
            None => self.error = 1,
        }
    }

    /// `st_szStop(id)`: a new slot keeps its stale id.
    pub fn sz_stop(&mut self, id: i32) {
        match self.counters.find_or_create(id) {
            Some(i) => {
                self.counters.slots[i].1 = -1;
                self.error = 0;
            }
            None => self.error = 1,
        }
    }

    /// `st_szGet(id)`: days left, rounded up.
    pub fn sz_get(&mut self, id: i32) -> i32 {
        let v = self.counters.get(id);
        self.error = v.is_none() as i32;
        v.map_or(0, |t| t.wrapping_add(DAY - 1) / DAY)
    }

    pub fn sz_is_now_going(&mut self, id: i32) -> bool {
        let v = self.counters.get(id);
        self.error = v.is_none() as i32;
        v.is_some_and(|t| t > 0)
    }

    pub fn sz_is_ever_started(&mut self, id: i32) -> bool {
        let v = self.counters.get(id);
        self.error = v.is_none() as i32;
        v.is_some()
    }

    // ---------------------------------------------------------------- planet and fleet ids (§2.7)
    /// `st_bAddNewAzonosito(p, id)`.
    pub fn b_add_id(&mut self, p: i32, id: i32) {
        if p == 0 {
            return;
        }
        match self.planets.iter().position(|e| e.0 == id) {
            Some(k) => self.planets[k] = (id, p as u8),
            None => self.planets.push((id, p as u8)),
        }
    }

    /// `st_bRemoveAzonosito(id)`: the entry is zeroed, the count stays.
    pub fn b_remove_id(&mut self, id: i32) {
        if id < 0 {
            return;
        }
        if let Some(k) = self.planets.iter().position(|e| e.0 == id) {
            self.planets[k] = (0, 0);
        }
    }

    /// `st_bSearch(id, mode)`. `id ≤ 0`: a random planet of empire `−id` the human can see (stream 5, or 2
    /// when `mode == 2`). The original aborts the script when `id == 0`, `mode != 1` and there is none; the
    /// port returns 0 (the ported stories never ask for it).
    pub fn b_search(&mut self, id: i32, mode: i32, world: &mut dyn StoryWorld) -> i32 {
        if id > 0 {
            return self.planets.iter().find(|e| e.0 == id).map_or(0, |e| e.1 as i32);
        }
        let n = world.planet_count();
        let fits = |w: &dyn StoryWorld, q: i32| w.planet_owner(q) == id.wrapping_neg() && w.planet_visibility(q) >= 0;
        let count = (1..=n).filter(|&q| fits(&*world, q)).count() as i32;
        let s = if mode == 2 { stream::GROUND_COMBAT } else { STREAM };
        let r = world.rng().range(s, 1, count);
        let mut k = 0;
        let mut found = 0;
        for q in 1..=n {
            if fits(&*world, q) {
                k += 1;
                if k == r {
                    found = q;
                }
            }
        }
        found
    }

    /// `st_fSearch(id, _)`. `id == 0` means `−EN`; `id < 0`: a random fleet of empire `−id` (stream 5).
    pub fn f_search(&mut self, id: i32, world: &mut dyn StoryWorld) -> i32 {
        let id = if id == 0 { -world.human() } else { id };
        let n = world.fleet_count();
        if id > 0 {
            return (1..=n).find(|&f| world.fleet_id(f) == id && world.fleet_owner(f) != 0xff).unwrap_or(0);
        }
        let count = (1..=n).filter(|&f| world.fleet_owner(f) == -id).count() as i32;
        let r = world.rng().range(STREAM, 1, count);
        let mut k = 0;
        let mut found = 0;
        for f in 1..=n {
            if world.fleet_owner(f) == -id {
                k += 1;
                if k == r {
                    found = f;
                }
            }
        }
        found
    }

    // ---------------------------------------------------------------- event predicates (§5)
    pub fn daily(&self) -> bool {
        self.flags[flag::DAILY]
    }

    /// `st_Counter(id)`.
    pub fn counter(&self, id: i32) -> bool {
        self.flags[flag::COUNTER] && (id == 0 || self.counter_done == id)
    }

    /// `st_Signal(id, t)`.
    pub fn signal(&self, id: i32, t: i32) -> bool {
        self.flags[flag::SIGNAL] && (id == 0 || self.signal.0 == id) && (t == 0 || self.signal.1 == t)
    }

    /// `st_StoryFinish(id)`.
    pub fn story_finished(&self, id: i32) -> bool {
        self.flags[flag::SIGNAL] && (id == 0 || self.signal.0 == id) && self.signal.1 == 1
    }

    /// `st_Special(type, d1, d2, d3)`.
    pub fn special_is(&self, t: i32, d1: i32, d2: i32, d3: i32) -> bool {
        let s = &self.special;
        self.flags[flag::SPECIAL] && s[0] == t && (d1 == 0 || s[1] == d1) && (d2 == 0 || s[2] == d2) && (d3 == 0 || s[3] == d3)
    }

    /// `st_ArrivedS(fid)`: a fleet reached a point.
    pub fn arrived_point(&self, fid: i32) -> bool {
        self.flags[flag::ARRIVED] && self.arrival.target_type == 0 && (fid == 0 || self.arrival.fleet_id == fid)
    }

    /// `st_ArrivedB(fid, bid)`: a fleet reached a planet.
    pub fn arrived_planet(&mut self, fid: i32, bid: i32, world: &mut dyn StoryWorld) -> bool {
        if !(self.flags[flag::ARRIVED] && self.arrival.target_type == 1 && (fid == 0 || self.arrival.fleet_id == fid)) {
            return false;
        }
        bid == 0 || self.b_search(bid, 0, world) == self.arrival.target
    }

    /// `st_ArrivedF(fid, fid2)`: a fleet reached a fleet.
    pub fn arrived_fleet(&self, fid: i32, fid2: i32) -> bool {
        self.flags[flag::ARRIVED]
            && self.arrival.target_type == 2
            && (fid == 0 || self.arrival.fleet_id == fid)
            && (fid2 == 0 || self.arrival.target_fleet_id == fid2)
    }

    /// `st_SpWarPart(fid)`: the 1-based participant with story id `fid` in the finished battle (`fid == 0`:
    /// 1 if a battle finished).
    pub fn battle_part(&self, fid: i32) -> i32 {
        if !self.flags[flag::SPACE_END] {
            return 0;
        }
        if fid == 0 {
            return 1;
        }
        if self.battle.len() < 3 {
            // The fast path looks at the first two entries only.
            return (0..2).find(|&i| self.battle.get(i).is_some_and(|p| p.fleet_id == fid)).map_or(0, |i| i as i32 + 1);
        }
        self.battle.iter().position(|p| p.fleet_id == fid).map_or(0, |i| i as i32 + 1)
    }

    /// `st_SpWarWin(fid)`.
    pub fn battle_won(&self, fid: i32) -> bool {
        let i = self.battle_part(fid);
        i != 0 && self.battle.get(i as usize - 1).is_some_and(|p| p.owner == self.battle_winner)
    }

    /// `st_Diff(a, b, c, d)` by difficulty.
    pub fn diff(difficulty: i32, a: i32, b: i32, c: i32, d: i32) -> i32 {
        match difficulty {
            1 => a,
            2 => b,
            3 => c,
            _ => d,
        }
    }

    /// `st_GetValue(key)`: a `values.txt` value scaled by difficulty (f32, truncated); 10 when missing.
    pub fn get_value(values: &[(i32, i32)], key: i32, difficulty: i32) -> i32 {
        const SCALE: [f32; 5] = [0.0, 2.0, 1.3, 1.3, 1.0];
        match values.iter().find(|e| e.0 == key) {
            Some(&(_, v)) => (v as f32 * SCALE.get(difficulty as usize).copied().unwrap_or(0.0)) as i32,
            None => 10,
        }
    }

    // ---------------------------------------------------------------- hooks (§1)
    /// `NEWGAME_Story` (the story data files are read separately: `ig2_formats::story_data`).
    pub fn new_game(&mut self) {
        self.undo.clear();
        self.planets.clear();
        self.counters.count = 0;
        self.logic.count = 0;
        self.vars.count = 0;
        self.adat = [0; ADAT];
    }

    /// `Story_Generate`.
    pub fn generate(&mut self) {
        self.flags = [false; FLAGS];
    }

    /// `FRISSIT_Story` (spec §3): every tick.
    pub fn tick(&mut self, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        let mut i = 1;
        while i <= self.counters.count {
            let t = self.counters.slots[i].1;
            if t >= 0 {
                let t = t.wrapping_sub(world.speed_mult());
                self.counters.slots[i].1 = t;
                if t < 1 {
                    self.flags[flag::COUNTER] = true;
                    self.counter_done = self.counters.slots[i].0;
                    self.counters.slots[i].1 = -1;
                    self.check(true, scripts, world);
                }
            }
            i += 1;
        }
        let ido = world.ido();
        if ido < self.old_ido || self.old_ido.wrapping_add(DAY) < ido {
            self.old_ido = ido;
            self.flags[flag::DAILY] = true;
            self.check(true, scripts, world);
        }
    }

    /// `Story_Check(top)` (spec §4).
    pub fn check(&mut self, top: bool, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if self.locked {
            return;
        }
        self.depth += 1;
        self.delayed = false;
        self.add_ship(world);
        self.restarting = 0;
        scripts.run(self, world);
        if self.adat_i32(adat::EPIDEMIC) != 0 && self.daily() {
            let planets = world.planet_count();
            let fleets = world.fleet_count();
            if !(1..=planets).any(|p| world.planet_infected(p)) && !(1..=fleets).any(|f| world.fleet_infected(f)) {
                self.set_adat_i32(adat::EPIDEMIC, 0);
                world.event(StoryEvent::EpidemicOver);
            }
        }
        self.add_ship(world);
        self.flags = [false; FLAGS];
        let keep = self.restarting;
        for b in self.buckets.iter_mut() {
            for e in b.entries.iter_mut() {
                if e.1 != 0 && e.0 != keep {
                    e.1 = 0;
                }
            }
        }
        if top {
            while let Some((id, kind)) = self.finished.first() {
                self.signal = (id, kind);
                self.flags[flag::SIGNAL] = true;
                self.finished.remove(id);
                self.check(false, scripts, world);
            }
        }
        if self.delayed {
            self.check(true, scripts, world);
        }
        if top {
            while let Some((id, _)) = self.starting.first() {
                self.buckets[bucket(id)].add(id, 1);
                self.starting.remove_first();
                self.check(false, scripts, world);
            }
            while let Some((t, d1)) = self.specials.first() {
                self.specials.remove_first();
                self.flags[flag::SPECIAL] = true;
                self.special[0] = t;
                self.special[1] = d1;
                self.check(false, scripts, world);
            }
        }
        self.skip_first = 0;
        self.depth -= 1;
        self.locked = false;
    }

    fn add_ship(&mut self, world: &mut dyn StoryWorld) {
        if self.last_addship != 0 {
            world.event(StoryEvent::AddShip(self.last_addship));
            self.last_addship = 0;
        }
    }

    /// `Story_Special(type, d1..d7)`.
    pub fn story_special(&mut self, data: [i32; 8], scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.special = data;
        self.flags[flag::SPECIAL] = true;
        self.check(true, scripts, world);
    }

    /// `Story_FleetArrivedSave(f)`: `order` is the fleet's target type `+0x42`, `target` its `(int) +0x44`.
    pub fn fleet_arrived_save(&mut self, f: i32, order: i32, target: i32, world: &dyn StoryWorld) {
        if !(0..=2).contains(&order) {
            return;
        }
        self.arrival = Arrival {
            fleet_id: world.fleet_id(f),
            fleet: f,
            target,
            target_type: order,
            target_fleet_id: if order == 2 { world.fleet_id(target) } else { 0 },
        };
        self.flags[flag::ARRIVED] = true;
    }

    /// `Story_FleetArrivedCheck()`.
    pub fn fleet_arrived_check(&mut self, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if self.flags[flag::ARRIVED] {
            self.check(true, scripts, world);
        }
    }

    /// `Story_SpaceBattleFinished(list, n, check)`.
    pub fn space_battle_finished(&mut self, list: &[Participant], check: bool, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        let en = world.human();
        self.you_were_in_battle = list.iter().any(|p| p.owner == en);
        self.battle = list.to_vec();
        self.flags[flag::SPACE_END] = true;
        self.set_adat_i32(adat::BATTLE_88, 0);
        if check {
            self.check(true, scripts, world);
        }
    }

    /// `Story_SpaceBattleFinishedCheck()`.
    pub fn space_battle_finished_check(&mut self, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if self.flags[flag::SPACE_END] {
            self.set_adat_i32(adat::BATTLE_88, 0);
            self.check(true, scripts, world);
        }
    }

    /// `Story_SpaceBattleStarted()` with the two fleets of `spFlottak` (exe 0xBAA1C6 / 0xBAA1CC).
    pub fn space_battle_started(&mut self, a: i32, b: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.space_forbidden = false;
        self.space_simulate_forbidden = false;
        for off in [0x88, adat::NO_RETREAT_94, adat::NO_RETREAT_98, 0x7c, 0x84] {
            self.set_adat_i32(off, 0);
        }
        for e in 1..19 {
            self.battle_pact[e] = 0;
            self.battle_retreat[e] = 0;
        }
        self.zachkin = 0;
        // Story 108: the scripted chase of fleet 0x43a by 0x439.
        if self.l_is_true(0x439) && world.fleet_id(a) == 0x43a && world.fleet_id(b) == 0x439 {
            let f = self.f_search(0x439, world);
            world.event(StoryEvent::StopFleet(f));
            let g = self.f_search(0x43a, world);
            world.event(StoryEvent::StopFleet(g));
            self.arrival.fleet_id = 0x43a;
            self.arrival.fleet = self.f_search(0x43a, world);
            self.arrival.target = self.f_search(0x439, world);
            self.arrival.target_type = 2;
            self.flags[flag::ARRIVED] = true;
            self.space_forbidden = true;
            return;
        }
        if self.running(0x6c) && self.l_is_false(0x439) && world.fleet_owner(a) == world.human() && world.fleet_id(b) == 0x43a {
            let f = self.f_search(0x439, world);
            if f != 0 {
                world.event(StoryEvent::JoinBattle(f));
                self.battle_pact[9] = 1;
                self.set_adat_i32(adat::NO_RETREAT_94, 1);
                self.set_adat_i32(adat::NO_RETREAT_98, 1);
            }
            return;
        }
        self.flags[flag::SPACE_START] = true;
        self.check(true, scripts, world);
    }

    /// `Story_GroundBattleStarted(f, p)`; `owner` = the planet's owner.
    pub fn ground_battle_started(&mut self, f: i32, p: i32, owner: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if self.adat_i32(0x54) != 0 {
            return;
        }
        if f == 0 || world.fleet_owner(f) == 0xff {
            self.ground.fleet = 0;
            self.ground.fleet_id = 0;
            self.ground.fleet_owner = 0;
        } else {
            self.ground.fleet = f;
            self.ground.fleet_id = world.fleet_id(f);
            self.ground.fleet_owner = world.fleet_owner(f);
        }
        self.set_adat_i32(0xf4, owner);
        self.set_adat_i32(0x40, 0);
        self.ground_forbidden = false;
        self.ground_simulate_forbidden = false;
        self.flags[flag::GROUND_START] = true;
        self.ground.planet = p;
        self.check(true, scripts, world);
    }

    /// `Story_GroundBattleFinishedSave(f, p, won)`; `owner` = the planet's owner now.
    pub fn ground_battle_finished_save(&mut self, f: i32, p: i32, owner: i32, attacker_won: bool, world: &dyn StoryWorld) {
        self.ground.fleet = f;
        if f == 0 || world.fleet_owner(f) == 0xff {
            self.ground.fleet_id = 0;
            self.ground.fleet_owner = 0;
        } else {
            self.ground.fleet_id = world.fleet_id(f);
            self.ground.fleet_owner = world.fleet_owner(f);
        }
        self.ground.planet = p;
        self.ground.old_owner = owner;
        self.flags[flag::GROUND_END] = true;
        self.ground.attacker_won = attacker_won;
        self.ground.defender_won = !attacker_won;
        self.set_adat_i32(adat::BATTLE_88, 0);
    }

    /// `Story_GroundBattleFinishedCheck()`: `buildings` = the planet's building count (`HanyEpulet`),
    /// `owner` its owner now.
    pub fn ground_battle_finished_check(&mut self, buildings: i32, owner: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if !self.flags[flag::GROUND_END] {
            return;
        }
        if buildings == 0 {
            self.ground.attacker_won = true;
            self.ground.old_owner = self.adat_i32(0xf4);
        } else if self.ground.attacker_won && owner == self.ground.old_owner {
            self.flags[flag::GROUND_END] = false;
            return;
        }
        self.check(true, scripts, world);
    }

    /// `Story_ArrivedToRadarRangeFleet(f)`.
    pub fn radar_fleet(&mut self, f: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.flags[flag::RADAR_FLEET] = true;
        self.radar.1 = f;
        self.check(true, scripts, world);
    }

    /// `Story_ArrivedToRadarRangeBolygo(p)`.
    pub fn radar_planet(&mut self, p: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.flags[flag::RADAR_PLANET] = true;
        self.radar.0 = p;
        self.check(true, scripts, world);
    }

    /// `Story_Diplomacy_Special(which, e)`.
    pub fn diplomacy_special(&mut self, which: i32, e: i32, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        self.diplomacy = (which, e, 0);
        self.flags[flag::DIPLOMACY] = true;
        self.check(true, scripts, world);
    }

    /// `Story_KemEsemeny()` (SP only).
    pub fn spy_event(&mut self, scripts: &dyn Script, world: &mut dyn StoryWorld) {
        if world.multiplayer() {
            return;
        }
        self.flags[flag::SPY] = true;
        self.spy_kind = 3;
        self.check(true, scripts, world);
    }
}

// ---------------------------------------------------------------- the scenario stories (§6)

/// `st_Scen_include` (exe 0x601580): stories 360..375.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScenarioStories;

/// The human wins: `WIN(0, 1)` + `MEGNYERES(1)`.
fn victory(s: &mut Story, w: &mut dyn StoryWorld) {
    s.win(0, true, w);
    w.event(StoryEvent::Won);
}

/// `DIE()`: `MEGHALAS(EN)`.
fn defeat(w: &mut dyn StoryWorld) {
    let en = w.human();
    w.event(StoryEvent::EmpireDies(en));
}

impl Script for ScenarioStories {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        let en = w.human();
        // 360: the fleet with id 3601 (or slot 0) is owned by anyone but empire 0.
        if s.running(360) && s.daily() {
            let f = s.f_search(3601, w);
            if w.fleet_owner(f) != 0 {
                victory(s, w);
            }
        }
        // 361: every listed empire is dead.
        if s.running(361) && s.daily() {
            let mut all_dead = true;
            for k in 1..8 {
                let e = s.v_get(3610 + k);
                if e != 0 && w.empire_alive(e) {
                    all_dead = false;
                }
            }
            if all_dead {
                victory(s, w);
            }
        }
        // 362: capture planet 3621.
        if s.running(362) && s.daily() {
            let p = s.b_search(3621, 0, w);
            if w.planet_owner(p) == en {
                victory(s, w);
            }
        }
        // 363: destroy fleet 3631.
        if s.running(363) && s.daily() && s.f_search(3631, w) == 0 {
            victory(s, w);
        }
        // 364: reach planet 3641 with one of the human's fleets, or own it.
        if s.running(364) {
            if s.flags[flag::ARRIVED] && s.arrival.target_type == 1 {
                let p = s.b_search(3641, 0, w);
                if p == s.arrival.target && w.fleet_owner(s.arrival.fleet) == en {
                    victory(s, w);
                }
            }
            if s.daily() {
                let p = s.b_search(3641, 0, w);
                if w.planet_owner(p) == en {
                    victory(s, w);
                }
            }
        }
        // 365: find planet 3651.
        if s.running(365) && s.daily() {
            let p = s.b_search(3651, 0, w);
            if w.planet_visibility(p) > 0 {
                victory(s, w);
            }
        }
        // 366: survive v(3661) days (no MEGNYERES).
        if s.running(366) {
            if s.starting_now() {
                let d = s.v_get(3661);
                s.sz_start(3661, d);
            }
            if s.counter(3661) {
                s.win(0, true, w);
            }
            if s.daily() {
                s.sz_get(3661);
            }
        }
        // 367: more money than v(3671).
        if s.running(367) && s.daily() {
            let money = w.money(en);
            if (s.v_get(3671) as f64) < money {
                victory(s, w);
            }
        }
        // 368: more planets than v(3681).
        if s.running(368) && s.daily() && s.v_get(3681) < w.planets_of(en) {
            victory(s, w);
        }
        // 369: own planet 3691 with more than v(3691) (+0x28).
        if s.running(369) && s.daily() {
            let p = s.b_search(3691, 0, w);
            if w.planet_owner(p) == en {
                let p = s.b_search(3691, 0, w);
                if s.v_get(3691) < w.planet_value(p) {
                    victory(s, w);
                }
            }
        }
        // 370: protect empire v(3701).
        if s.running(370) && s.daily() {
            let e = s.v_get(3701);
            if !w.empire_alive(e) {
                defeat(w);
            }
        }
        // 371: a deadline of v(3711) days, shown as an info line.
        if s.running(371) {
            if s.starting_now() {
                let d = s.v_get(3711);
                s.sz_start(3711, d);
                s.v_set(3712, 0);
            }
            if s.counter(3711) {
                defeat(w);
            }
            if s.daily() {
                let shown = s.v_get(3712);
                if shown != 0 {
                    s.remove_game_info(shown + 3710, w);
                }
                let d = s.sz_get(3711);
                let (id, value, mark) = if d >= 361 {
                    (3711, (d + 359) / 360, 1)
                } else if d >= 31 {
                    (3712, (d + 29) / 30, 2)
                } else {
                    (3713, d, 3)
                };
                if d >= 2 {
                    s.add_game_info(id, value, -1, w);
                    s.v_set(3712, mark);
                }
            }
        }
        // 372: fleet 3721 must fall in a space battle; a battle it survives kills the human.
        if s.running(372) {
            if s.flags[flag::SPACE_START] {
                s.set_adat_i32(adat::NO_RETREAT_94, 1);
                s.set_adat_i32(adat::NO_RETREAT_98, 1);
            }
            if s.battle_part(0) != 0 {
                if s.f_search(3721, w) == 0 {
                    victory(s, w);
                } else {
                    defeat(w);
                }
            }
        }
        // 373: empire v(3731) must not develop tech v(3732).
        if s.running(373) && s.daily() {
            let tech = s.v_get(3732);
            let e = s.v_get(3731);
            if w.tech_status(tech, e) == 5 {
                defeat(w);
            }
        }
        // 374: ally with one of the listed empires.
        if s.running(374) && s.daily() {
            let mut allied = false;
            for k in 1..8 {
                let e = s.v_get(3740 + k);
                if e != 0 {
                    let e = s.v_get(3740 + k);
                    if w.relation_level(en, e) == 5 {
                        allied = true;
                    }
                }
            }
            if allied {
                victory(s, w);
            }
        }
        // 375: wipe out empire v(3751) (no planet the human can see) with more planets than v(3751).
        if s.running(375) && s.daily() {
            let e = s.v_get(3751);
            if s.b_search(e.wrapping_neg(), 1, w) == 0 {
                if s.v_get(3751) < w.planets_of(en) {
                    victory(s, w);
                } else {
                    defeat(w);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct W {
        ido: i32,
        speed: i32,
        rng: Streams,
        owners: Vec<i32>,
        vis: Vec<i32>,
        fleets: Vec<(i32, i32)>,
        alive: [bool; 19],
        events: Vec<StoryEvent>,
    }

    impl StoryWorld for W {
        fn ido(&self) -> i32 {
            self.ido
        }
        fn speed_mult(&self) -> i32 {
            self.speed
        }
        fn human(&self) -> i32 {
            1
        }
        fn difficulty(&self) -> i32 {
            1
        }
        fn multiplayer(&self) -> bool {
            false
        }
        fn rng(&mut self) -> &mut Streams {
            &mut self.rng
        }
        fn planet_count(&self) -> i32 {
            self.owners.len() as i32 - 1
        }
        fn planet_owner(&self, p: i32) -> i32 {
            self.owners[p as usize]
        }
        fn planet_value(&self, _p: i32) -> i32 {
            0
        }
        fn planet_visibility(&self, p: i32) -> i32 {
            self.vis[p as usize]
        }
        fn fleet_count(&self) -> i32 {
            self.fleets.len() as i32 - 1
        }
        fn fleet_owner(&self, f: i32) -> i32 {
            self.fleets[f as usize].0
        }
        fn fleet_id(&self, f: i32) -> i32 {
            self.fleets[f as usize].1
        }
        fn empire_alive(&self, e: i32) -> bool {
            self.alive.get(e as usize).copied().unwrap_or(false)
        }
        fn planets_of(&self, _e: i32) -> i32 {
            3
        }
        fn money(&self, _e: i32) -> f64 {
            1000.0
        }
        fn tech_status(&self, _t: i32, _e: i32) -> i32 {
            0
        }
        fn relation_level(&self, _a: i32, _b: i32) -> i32 {
            3
        }
        fn event(&mut self, ev: StoryEvent) {
            self.events.push(ev);
        }
    }

    fn world() -> W {
        W { speed: 1, owners: vec![0, 1, 2, 2, 0], vis: vec![0, 1000, 1, -1, 1], fleets: vec![(0, 0), (1, 7), (2, 9), (0xff, 9)], ..W::default() }
    }

    #[test]
    fn queue_remove_matches_negated_ids() {
        let mut q = Queue::default();
        q.add(5, 1);
        q.add(-7, 2);
        q.add(7, 3);
        q.remove(7);
        assert_eq!(q.entries, vec![(5, 1), (7, 3)]);
        assert_eq!(q.value(7), 3);
        assert_eq!(q.value(8), 0);
    }

    #[test]
    fn variables_set_error_and_cap() {
        let mut s = Story::default();
        assert_eq!(s.v_get(3), 0);
        assert_eq!(s.error, 1);
        s.v_set(3, 9);
        s.v_add(3, 2);
        assert_eq!(s.v_get(3), 11);
        assert_eq!(s.error, 0);
        s.v_add(4, 1); // missing: no new entry
        assert_eq!((s.vars.count, s.error), (1, 1));
        for i in 0..600 {
            s.v_set(100 + i, i);
        }
        assert_eq!(s.vars.count, MAX_VARS);
        assert_eq!(s.error, 1);
        assert!(s.l_is_false(1));
        s.l_set(1, true);
        assert!(s.l_is_true(1));
    }

    #[test]
    fn counter_stop_keeps_a_stale_id() {
        let mut s = Story::default();
        s.sz_start(10, 2);
        s.sz_start(11, 1);
        s.counters.count = 1; // as after a new game: slot 2 still holds (11, 50)
        s.sz_stop(99);
        assert_eq!(s.counters.slots[2], (11, -1));
        assert_eq!(s.sz_get(10), 2);
        assert!(!s.sz_is_ever_started(99));
    }

    #[test]
    fn counters_fire_and_daily_needs_more_than_fifty() {
        let mut s = Story::default();
        let mut w = world();
        w.speed = 16;
        s.sz_start(3661, 1);
        s.old_ido = 0;
        w.ido = 50;
        s.tick(&ScenarioStories, &mut w);
        assert_eq!(s.counters.live(), &[(3661, 34)]);
        assert_eq!(s.old_ido, 0, "Ido 50 is not more than OldIdo + 50");
        w.ido = 51;
        for _ in 0..3 {
            s.tick(&ScenarioStories, &mut w);
        }
        assert_eq!(s.counters.live(), &[(3661, -1)]);
        assert_eq!(s.counter_done, 3661);
        assert_eq!(s.old_ido, 51);
        assert!(s.flags.iter().all(|&f| !f));
    }

    #[test]
    fn started_story_runs_its_start_once_and_wins() {
        let mut s = Story::default();
        let mut w = world();
        s.v_set(3661, 1);
        s.start_story(366, false);
        s.check(true, &ScenarioStories, &mut w);
        assert!(s.is_running(366));
        assert_eq!(s.buckets[366 & 0xff].entries, vec![(366, 0)]);
        assert_eq!(s.counters.live(), &[(3661, 50)]);
        w.ido = 120;
        for _ in 0..50 {
            s.tick(&ScenarioStories, &mut w);
        }
        assert!(!s.is_running(366));
        assert_eq!(s.history.entries, vec![(366, 120)]);
        assert!(s.finished.is_empty(), "the signal is consumed by the nested check");
        assert_eq!(s.signal, (366, 1));
        assert!(w.events.is_empty(), "366 only finishes");
    }

    #[test]
    fn win_applies_story_undo_entries() {
        let mut s = Story::default();
        let mut w = world();
        s.set_adat_i32(0x24, 7);
        s.set_adat_i32(0xd0 + 8, 2);
        s.add_undo(15, 0, 370);
        s.add_undo(13, 2, 370);
        s.add_undo(4, 3701, 371);
        s.add_undo(2, 0x30001, 370);
        s.win(370, true, &mut w);
        assert_eq!(s.adat_i32(0x24), 0);
        assert_eq!(s.adat_i32(0xd8), 1);
        assert_eq!(s.undo, vec![(0, 0, 0), (0, 0, 0), (4, 3701, 371)]);
        assert_eq!(w.events, vec![StoryEvent::Undo { kind: 2, value: 0x30001 }]);
        assert_eq!(s.finished.entries, vec![(370, 1)]);
        s.win(370, true, &mut w);
        assert_eq!(s.history.entries.len(), 1);
        assert_eq!(s.finished.entries.len(), 2);
    }

    #[test]
    fn planet_and_fleet_searches() {
        let mut s = Story::default();
        let mut w = world();
        s.b_add_id(3, 3621);
        s.b_add_id(0, 3622);
        assert_eq!(s.b_search(3621, 0, &mut w), 3);
        assert_eq!(s.b_search(3622, 0, &mut w), 0);
        // Empire 2 owns planets 2 and 3; the human sees only 2.
        let before = w.rng.state(STREAM);
        assert_eq!(s.b_search(-2, 1, &mut w), 2);
        assert_ne!(w.rng.state(STREAM), before);
        // No planet: still one draw.
        let before = w.rng.state(STREAM);
        assert_eq!(s.b_search(-5, 1, &mut w), 0);
        assert_eq!(w.rng.state(STREAM), crate::rng::step(before));
        assert_eq!(s.f_search(9, &mut w), 2);
        assert_eq!(s.f_search(0, &mut w), 1);
    }

    #[test]
    fn scenario_361_needs_every_listed_empire_dead() {
        let mut s = Story::default();
        let mut w = world();
        w.alive[3] = true;
        s.v_set(3611, 3);
        s.v_set(3612, 0);
        s.start_story(361, true);
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert!(w.events.is_empty());
        w.alive[3] = false;
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(w.events, vec![StoryEvent::Won]);
        assert!(!s.is_running(361));
    }

    #[test]
    fn scenario_371_countdown_info_lines() {
        let mut s = Story::default();
        let mut w = world();
        s.v_set(3711, 400);
        s.start_story(371, true);
        s.check(true, &ScenarioStories, &mut w);
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(w.events, vec![StoryEvent::GameInfo { id: 3711, value: 2 }]);
        assert_eq!(s.undo, vec![(4, 3711, 371)]);
        assert_eq!(s.vars.get(3712), Some(1));
        s.counters.slots[1].1 = 40 * DAY;
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(&w.events[1..], &[StoryEvent::RemoveGameInfo(3711), StoryEvent::GameInfo { id: 3712, value: 2 }]);
        assert_eq!(s.undo, vec![(4, 3712, 371)]);
    }

    #[test]
    fn epidemic_switch_clears_when_nothing_is_infected() {
        let mut s = Story::default();
        let mut w = world();
        s.set_adat_i32(adat::EPIDEMIC, 1);
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(s.adat_i32(adat::EPIDEMIC), 1, "only on the daily check");
        s.flags[flag::DAILY] = true;
        s.check(true, &ScenarioStories, &mut w);
        assert_eq!(s.adat_i32(adat::EPIDEMIC), 0);
        assert_eq!(w.events, vec![StoryEvent::EpidemicOver]);
    }

    #[test]
    fn get_value_scales_by_difficulty() {
        let values = [(5301, 1600)];
        assert_eq!(Story::get_value(&values, 5301, 1), 3200);
        assert_eq!(Story::get_value(&values, 5301, 2), 2080);
        assert_eq!(Story::get_value(&values, 5301, 4), 1600);
        assert_eq!(Story::get_value(&values, 1, 4), 10);
        assert_eq!(Story::diff(3, 1, 2, 3, 4), 3);
    }
}
