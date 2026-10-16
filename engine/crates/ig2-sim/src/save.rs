//! An original save game (`ig2_formats::save::SaveGame`, spec `re/formats/save.md`) as the sim's state.
//!
//! The save is the exe's memory written chunk by chunk (`LoadSaveVariables`, exe 0x544220), so the bridge keeps it
//! that way: [`SaveState`] holds the static globals as a sparse image of the exe's address space ([`Memory`], image
//! VAs) and the records the exe keeps behind pointers (fleets, ships, waypoints, designs, buildings) as arrays. Each
//! system's state structs are decoded from that image and stored back after the system ran ([`SaveState::colony`] /
//! [`SaveState::store_colony`] and so on), so overlapping views (a planet record read by the economy, the starmap
//! and diplomacy) and the original's aliasing (contact slot −1 over the diplomacy texts, the last-contact table
//! running into the AI's tables) behave as in the exe. Globals the save doesn't carry (the radar tables, the story
//! engine's statics, cursors) come from [`SaveState::set_volatile`] or keep their new-game values.
//!
//! [`SaveState::chunks`] turns the state back into the save's chunks by name, to compare a replay with the exe's own
//! saves of the same tick.

use std::collections::BTreeMap;

use ig2_formats::save::SaveGame;

use crate::ai::{AiState, Category, CATEGORIES, CATEGORY_SIZE, DESIGN_SLOTS, GRID};
use crate::colony::{Building, Planet};
use crate::colony_side::{ColonyWorld, TechTable, Treasury, EMPIRES, PLANET_SLOTS};
use crate::diplomacy::{Answer, Diplomacy, Letter, Message, PlayerSlot, Relation, LAST_CONTACT_CELLS};
use crate::fleet::{Fleet, StarPlanet, Waypoint, Wormhole};
use crate::radar::ShipDesign;
use crate::research::Research;
use crate::rng::Streams;
use crate::spacewar::{CombatFleet, Design, FleetTotals, Ship, Vehicle};
use crate::spy::{Spies, Spy, RECORD, ROSTERS, SLOTS};
use crate::story::{self, Queue, Story};

pub const PLANET_SIZE: usize = 0x188;
pub const FLEET_SIZE: usize = 0x80;
pub const BUILDING_SIZE: usize = 0x20;
pub const DESIGN_SIZE: usize = 0x84;
pub const TECH_SIZE: usize = 0x114;
pub const TECHS: usize = 193;
/// `AktPlayerek` entry.
pub const PLAYER_SLOT: usize = 0xc4;

/// Exe image VAs of the globals the sim reads (re/formats/save.md §4, the rules specs).
pub mod va {
    pub const EV: u32 = 0xBAE040;
    pub const HONAP: u32 = 0xBAE044;
    pub const NAP: u32 = 0xBAE048;
    pub const IDO: u32 = 0xBAE04C;
    pub const IDO2: u32 = 0xBAE050;
    pub const OLD_NAP: u32 = 0xBAE058;
    pub const NEHEZSEG: u32 = 0x92827C;
    pub const IDO_SPEED: u32 = 0xBAE25C;
    pub const MULTIPLAYER: u32 = 0xBAE2C0;
    pub const SPEED_LOCKED: u32 = 0xBAD0D4;
    pub const AI_OFF: u32 = 0xB77244;
    pub const ALL_TECHS: u32 = 0xB77254;
    pub const RESEARCH_CHEAT: u32 = 0xB7727C;
    pub const DEBUGX: u32 = 0xBAE587;
    pub const DEBUG_SPEED: u32 = 0xB77240;
    pub const FORCED_ANSWER: u32 = 0xB7724C;
    pub const SPY_FORCED: u32 = 0xB77250;
    pub const DESIGN_DEBUG: u32 = 0xA3F0B0;
    pub const TUTORIAL_ACTIVE: u32 = 0xF6915D;
    pub const TUTORIAL_PAUSED: u32 = 0xF66F2A;
    pub const TUTORIAL_EVENT: u32 = 0xF66F40;
    pub const TUTORIAL_NO_RECRUITS: u32 = 0xF66F2B;
    pub const RND_N: u32 = 0xBAEB64;
    pub const RND_TOTAL: u32 = 0xBAEBB4;
    pub const RADAR2: u32 = 0xBB61A8;
    pub const RADAR2_SQ: u32 = 0xBB6408;
    pub const RADAR_MULT: u32 = 0xBB6668;
    pub const RADAR_MULT_SQ: u32 = 0xBB66B8;
    pub const DIP_OLD_NAP: u32 = 0xAB6FFC;
    pub const DIP_STARTED: u32 = 0xAB69B5;

    pub const EN: u32 = 0x912A3C;
    pub const MONEY: u32 = 0xBAD168;
    pub const POOLS: u32 = 0xBAD200;
    pub const RESERVES: u32 = 0xBAD590;
    pub const SLIDERS: u32 = 0xBAD920;
    pub const SHARES: u32 = 0xBADCB0;
    pub const PROGRAM_SEBESSEG: u32 = 0xBAE54C;
    pub const PROGRAM_PLAYERS: u32 = 0x912ACC;
    pub const PROGRAM_PLAYERS2: u32 = 0x912AD0;
    pub const PLAYER_KI: u32 = 0xBAE560;
    pub const PLAYER_FAJ: u32 = 0xBAE574;
    pub const EMPIRE_POWER: u32 = 0xBAEA78;
    pub const EMPIRE_SPACE_POWER: u32 = 0xBAEAC8;
    pub const EMPIRE_PLANETS: u32 = 0xBAEB18;
    pub const EMPIRE_ALIVE: u32 = 0xBAE6AC;
    pub const EMPIRE_COUNTDOWN: u32 = 0xBAEB68;
    pub const EMPIRE_KNOWS: u32 = 0xBAEBB8;
    pub const EMPIRE_KNOWN_SINCE: u32 = 0xBAED28;
    pub const AI_DANGER: u32 = 0xDB27F8;
    pub const GLOBAL_MORALE: u32 = 0xBAEAC4;
    pub const GLOBAL_MORALE_SPEED: u32 = 0xBAEB14;
    pub const RAND_SEED: u32 = 0xBAF2D0;
    pub const FULL_GDP: u32 = 0xBAE060;
    pub const FULL_TAX: u32 = 0xBAE0B0;
    pub const TRADE_LAST: u32 = 0xBAE100;
    pub const TRADE_THIS: u32 = 0xBAE150;
    pub const TAX_LAST: u32 = 0xBAE1A0;
    pub const TAX_THIS: u32 = 0xBAE1F0;
    pub const AI_EMPIRE: u32 = 0x915F58;
    pub const MAY_ATTACK: u32 = 0x915F44;
    pub const SPY_ENABLED: u32 = 0xBAE3AF;
    pub const DIPLOMACY_ENABLED: u32 = 0xBAE3AD;

    pub const CONTACTS: u32 = 0xA41E98;
    pub const CONTACT_GHOST: u32 = 0xA41DD4;
    pub const MAX_PLAYERS: u32 = 0xAB64DC;
    pub const RULER_AGE: u32 = 0xA41E48;
    pub const GREETED: u32 = 0xA41CC4;
    pub const RELATIONS: u32 = 0xDABF38;
    pub const LAST_CONTACT: u32 = 0xDAF520;

    pub const KEMEK: u32 = 0xBAFCC8;
    pub const SPY_COUNT: u32 = 0xBB1F84;
    pub const SPY_CAPACITY: u32 = 0xBB2388;
    pub const SPY_CENTRES: u32 = 0xBB23AC;
    pub const SPIES_WORKING: u32 = 0xBB23D0;
    pub const TRADE_PORTS: u32 = 0xBB23F4;
    pub const SPY_CENTRES_BY_PLANET: u32 = 0xBB2418;
    pub const KIDNAP: u32 = 0xBB405C;
    pub const MISSIONS: u32 = 0x912F48;

    pub const FLEET_COUNT: u32 = 0xBB40A8;
    pub const FLEET_NUMBERS: u32 = 0xBB40B0;
    pub const SHIP_COUNT: u32 = 0xBB4104;
    pub const VEHICLE_COUNT: u32 = 0xBB4154;
    pub const VEHICLES: u32 = 0xBB41A8;
    pub const COLONY_SHIP_POOL: u32 = 0xBB4108;
    pub const TERRAFORMER_POOL: u32 = 0xBB4158;
    pub const WORMHOLE_COUNT: u32 = 0xBB61A0;
    pub const WORMHOLES: u32 = 0xBB60F8;
    pub const WAYPOINT_COUNT: u32 = 0xBB60F0;

    pub const PLANET_COUNT: u32 = 0x912AC8;
    pub const PLANET_COUNT2: u32 = 0xBB6B58;
    pub const PLANETS: u32 = 0xB85EA8;
    pub const LAST_NEAR_FLEET: u32 = 0xB99340;
    pub const UNREST: u32 = 0xB99668;
    pub const GROUND: u32 = 0xB99990;
    pub const FORTRESS: u32 = 0xB99CB8;
    pub const ION_GUN: u32 = 0xB99FE0;
    pub const FIRE_BRIGADE: u32 = 0xB9A308;
    pub const NEEDED_MORALE: u32 = 0xB9A630;

    pub const TECHS: u32 = 0xB9C0C8;
    pub const URGENT: u32 = 0xB9C014;
    pub const RESEARCHING: u32 = 0xB9A9B8;
    pub const LABS: u32 = 0x912728;
    pub const LAB_BUILDINGS: u32 = 0xBA90E0;
    pub const DESIGN_COUNT: u32 = 0xB9C028;
    pub const BUILDINGS_OCCUPIED: u32 = 0xBA9518;
    pub const BUILDING_SLOTS: u32 = 0xBA9840;
    pub const BUILDING_BLOCKS: u32 = 0xBA9B68;

    pub const AI_DESIGNS: u32 = 0xDAF668;
    pub const AI_DESIGNS_SPECIAL: u32 = 0xDAFA20;
    pub const AI_MAIN_PLANET: u32 = 0xDB0548;
    pub const AI_MAIN_FLEET: u32 = 0xDB0598;
    pub const AI_SCOUT: u32 = 0xDB0738;
    pub const AI_EMPTY_PLANETS: u32 = 0xDB0724;
    pub const AI_LAST_BAD_COLONY: u32 = 0xDB05E8;
    pub const AI_NO_LOAD: u32 = 0xDB0638;
    pub const AI_LOAD_PHASE: u32 = 0xDB0688;
    pub const AI_NO_UPGRADE: u32 = 0xDB06D8;
    pub const AI_CATEGORIES: u32 = 0xDB28E8;
    pub const AI_CENTRE_X: u32 = 0xDB2848;
    pub const AI_CENTRE_Y: u32 = 0xDB2898;

    pub const STORY_ADAT: u32 = 0xF273F8;
    pub const STORY_ALL_FLEETS_VISIBLE: u32 = 0xF265D4;
    pub const STORY_UNDO_COUNT: u32 = 0xF265D8;
    pub const STORY_UNDO: u32 = 0xF265E0;
    pub const STORY_LIST_COUNTS: u32 = 0xF23A08;
    pub const STORY_PLANETS: u32 = 0xF23A20;
    pub const STORY_COUNTERS: u32 = 0xF24388;
    pub const STORY_LOGIC: u32 = 0xF24CF0;
    pub const STORY_VARS: u32 = 0xF251A8;
    pub const STORY_ERROR: u32 = 0xF23A1C;
    pub const STORY_SKIP_FIRST: u32 = 0xF23A04;
    pub const STORY_FLAGS: u32 = 0xF2F8AC;
    pub const STORY_LAST_CHECKED: u32 = 0xF2273C;
    pub const STORY_COUNTER_DONE: u32 = 0xF26560;
    pub const STORY_OLD_IDO: u32 = 0xF432B4;
    pub const STORY_DELAYED: u32 = 0xF26530;
    pub const STORY_RESTARTING: u32 = 0xF261FC;
    pub const STORY_SIGNAL: u32 = 0xF26564;
    pub const STORY_LAST_ADDSHIP: u32 = 0xF26200;
    pub const STORY_SPECIAL: u32 = 0xF265A4;
    pub const STORY_RADAR: u32 = 0xF2657C;
    pub const STORY_DIPLOMACY: u32 = 0xF2656C;
    pub const STORY_DIPLOMACY_ANSWER: u32 = 0xF42660;
    pub const STORY_ARRIVAL: u32 = 0xF2654C;
    pub const STORY_GROUND: u32 = 0xF26538;
    pub const STORY_GROUND_WON: u32 = 0xF26532;
    pub const STORY_YOU_WERE_IN_BATTLE: u32 = 0xF26531;
    pub const STORY_SPY_KIND: u32 = 0xF26588;

    pub const AUTOBUILD_PICK: u32 = 0x996530;
    pub const MAP_CURSOR: u32 = 0x996420;
}

// ------------------------------------------------------------------------------------------------ memory

/// A sparse image of the exe's static data: non-overlapping byte runs keyed by their start VA.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Memory {
    runs: BTreeMap<u32, Vec<u8>>,
}

impl Memory {
    /// `len` bytes at `at`; bytes no run covers read 0.
    pub fn read(&self, at: u32, len: usize) -> Vec<u8> {
        let mut out = vec![0; len];
        let end = at as u64 + len as u64;
        for (&s, run) in self.runs.range(..end.min(u32::MAX as u64) as u32) {
            let e = s as u64 + run.len() as u64;
            if e <= at as u64 {
                continue;
            }
            let lo = (s as u64).max(at as u64);
            let hi = e.min(end);
            out[(lo - at as u64) as usize..(hi - at as u64) as usize].copy_from_slice(&run[(lo - s as u64) as usize..(hi - s as u64) as usize]);
        }
        out
    }

    /// Write `bytes` at `at`, merging the runs it touches.
    pub fn write(&mut self, at: u32, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let end = at as u64 + bytes.len() as u64;
        let touched: Vec<u32> = self
            .runs
            .range(..=end.min(u32::MAX as u64) as u32)
            .filter(|&(&s, r)| s as u64 + r.len() as u64 >= at as u64)
            .map(|(&s, _)| s)
            .collect();
        let lo = touched.first().map_or(at, |&s| s.min(at));
        let hi = touched.iter().map(|&s| s as u64 + self.runs[&s].len() as u64).fold(end, u64::max);
        let mut merged = vec![0; (hi - lo as u64) as usize];
        for s in touched {
            let r = self.runs.remove(&s).unwrap();
            merged[(s - lo) as usize..(s - lo) as usize + r.len()].copy_from_slice(&r);
        }
        merged[(at - lo) as usize..(at - lo) as usize + bytes.len()].copy_from_slice(bytes);
        self.runs.insert(lo, merged);
    }

    pub fn u8(&self, at: u32) -> u8 {
        self.read(at, 1)[0]
    }
    pub fn i16(&self, at: u32) -> i16 {
        i16::from_le_bytes(self.read(at, 2).try_into().unwrap())
    }
    pub fn i32(&self, at: u32) -> i32 {
        i32::from_le_bytes(self.read(at, 4).try_into().unwrap())
    }
    pub fn f32(&self, at: u32) -> f32 {
        f32::from_bits(self.i32(at) as u32)
    }
    pub fn f64(&self, at: u32) -> f64 {
        f64::from_le_bytes(self.read(at, 8).try_into().unwrap())
    }
    pub fn set_u8(&mut self, at: u32, v: u8) {
        self.write(at, &[v]);
    }
    pub fn set_i32(&mut self, at: u32, v: i32) {
        self.write(at, &v.to_le_bytes());
    }
    pub fn set_f32(&mut self, at: u32, v: f32) {
        self.write(at, &v.to_le_bytes());
    }
    pub fn set_f64(&mut self, at: u32, v: f64) {
        self.write(at, &v.to_le_bytes());
    }
    /// `n` consecutive i32 at `at`.
    pub fn i32s<const N: usize>(&self, at: u32) -> [i32; N] {
        std::array::from_fn(|i| self.i32(at + 4 * i as u32))
    }
    pub fn set_i32s(&mut self, at: u32, v: &[i32]) {
        let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.write(at, &b);
    }
    pub fn f32s<const N: usize>(&self, at: u32) -> [f32; N] {
        std::array::from_fn(|i| self.f32(at + 4 * i as u32))
    }
    pub fn set_f32s(&mut self, at: u32, v: &[f32]) {
        let b: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.write(at, &b);
    }
    pub fn bytes<const N: usize>(&self, at: u32) -> [u8; N] {
        self.read(at, N).try_into().unwrap()
    }
    pub fn flags<const N: usize>(&self, at: u32) -> [bool; N] {
        self.bytes::<N>(at).map(|b| b != 0)
    }
}

// ------------------------------------------------------------------------------------------------ chunk VAs

/// Where a save chunk lives in the exe (a static global), by the reader's chunk name. `None` = held elsewhere
/// (pointer arrays, the story queues) or not modelled (the chunk passes through unchanged).
pub fn chunk_va(name: &str) -> Option<u32> {
    if let Some(rest) = name.strip_prefix("Talalmanyok[") {
        let (t, field) = rest.split_once(']')?;
        let t: u32 = t.parse().ok()?;
        let off = match field {
            ".status" => 0x45,
            ".time" => 0x58,
            ".given" => 0xa4,
            ".seen" => 0xc0,
            f => u32::from_str_radix(f.strip_prefix("+0x")?, 16).ok()?,
        };
        return Some(va::TECHS + t * TECH_SIZE as u32 + off);
    }
    if let Some(e) = name.strip_prefix("Kemek[").and_then(|r| r.strip_suffix(']')) {
        return Some(va::KEMEK + e.parse::<u32>().ok()? * (SLOTS * RECORD) as u32);
    }
    Some(match name {
        "Ev" => va::EV,
        "Honap" => va::HONAP,
        "Nap" => va::NAP,
        "Ido" => va::IDO,
        "IdoMultiPlayerBattle" => 0xBAE054,
        "OldNap" => va::OLD_NAP,
        "Nehezseg" => va::NEHEZSEG,
        "Ido2" => va::IDO2,
        "scenario_kezdesi_ido" => 0xBAD108,
        "scenario_registry_name" => 0xBAD0E8,
        // Diplomacy_Save.
        "AktPlayerek" => va::CONTACTS,
        "BirodalmiMacskak" => 0xA41CD8,
        "BirodalmiPapagajok" => 0xA41D28,
        "BirodalmiEvfordulok" => 0xA41D78,
        "BirodalmiGyengesegek" => 0xA41DC8,
        "HanyevesAzUralkodo" => va::RULER_AGE,
        "VoltmarUdvozloUzenetEbbenAzEvben" => va::GREETED,
        "AktDipUzenet" => 0xA41E14,
        "AktDipValasz" => 0xA43748,
        "SpecialListaMut" => 0xA41DC4,
        "PlayerSelected" => 0x902F70,
        "PlayerSelectedOld" => 0x902F74,
        "MostHanyPlayerVan" => 0xA41D24,
        "SpecialLista" => 0xA42DE8,
        "MaxPlayerek" => va::MAX_PLAYERS,
        "DiplomaciaEngedelyezve" => va::DIPLOMACY_ENABLED,
        "TradeEngedelyezve" => 0xBAE3AE,
        "SpyEngedelyezve" => va::SPY_ENABLED,
        // Story statics.
        "st_vOzonvizMut" => va::STORY_UNDO_COUNT,
        "st_vOzonviz" => va::STORY_UNDO,
        "StoryInterregnumIdo" => 0xF22738,
        "StoryAdat" => va::STORY_ADAT,
        "CampaignType" => 0x928278,
        "Story_Minden_Flotta_Latszik" => va::STORY_ALL_FLEETS_VISIBLE,
        "sBolygok.n" => va::STORY_LIST_COUNTS,
        "sSzamlalok.n" => va::STORY_LIST_COUNTS + 4,
        "sLogikaiak.n" => va::STORY_LIST_COUNTS + 8,
        "sValtozok.n" => va::STORY_LIST_COUNTS + 12,
        "sBolygok" => va::STORY_PLANETS,
        "sSzamlalok" => va::STORY_COUNTERS,
        "sLogikaiak" => va::STORY_LOGIC,
        "sValtozok" => va::STORY_VARS,
        "StoryTerroristaCselValoszinusege" => 0xF23A18,
        // Empires.
        "EN" => va::EN,
        "Money" => va::MONEY,
        "MoneyTeruletenkent" => va::POOLS,
        "MoneyHatar" => va::RESERVES,
        "MoneyTeruletCsuszka" => va::SLIDERS,
        "MoneyTeruletSzorzo" => va::SHARES,
        "ProgramMode" => 0xBAE544,
        "ProgramDifficulty" => 0xBAE548,
        "ProgramSebesseg" => va::PROGRAM_SEBESSEG,
        "ProgramPlayers" => va::PROGRAM_PLAYERS,
        "ProgramPlayers2" => va::PROGRAM_PLAYERS2,
        "ProgramMachines" => 0xBAE550,
        "ProgramGalaxyShape" => 0xBAE554,
        "MultiplayerBattleStatusz" => 0xBAE558,
        "OnlyServerCanModifyTime" => 0xBAE55C,
        "PlayerKi" => va::PLAYER_KI,
        "PlayerFaj" => va::PLAYER_FAJ,
        "PlayerNev" => 0xBAE588,
        "BirodalomEro" => va::EMPIRE_POWER,
        "BirodalomUrEro" => va::EMPIRE_SPACE_POWER,
        "BirodalomHanyBolygo" => va::EMPIRE_PLANETS,
        "BirodalomEl" => va::EMPIRE_ALIVE,
        "BirodalomVisszaSzamlalas" => va::EMPIRE_COUNTDOWN,
        "BirodalomIsmeri" => va::EMPIRE_KNOWS,
        "BirodalomMiotaIsmeri" => va::EMPIRE_KNOWN_SINCE,
        "MiVeszelyAllapot" => va::AI_DANGER,
        "GlobalisMoralCSokkentoBarkacs" => va::GLOBAL_MORALE,
        "GlobalisMoralCSokkentoBarkacsSebesseg" => va::GLOBAL_MORALE_SPEED,
        "RandSeed" => va::RAND_SEED,
        "FullGDP" => va::FULL_GDP,
        "FullTax" => va::FULL_TAX,
        "TradeIncomeLastYear" => va::TRADE_LAST,
        "TradeIncomeThisYear" => va::TRADE_THIS,
        "TaxIncomeLastYear" => va::TAX_LAST,
        "TaxIncomeThisYear" => va::TAX_THIS,
        // Spies.
        "OsszHanyKem" => va::SPY_COUNT,
        "HanyKemLehet" => va::SPY_CAPACITY,
        "HanyKemVan" => va::SPIES_WORKING,
        "HanyKemLehetOsszesen" => va::SPY_CENTRES,
        "HanyKemKozpontBolygonkent" => va::SPY_CENTRES_BY_PLANET,
        "CsaladtagElrabolvaKi" => va::KIDNAP,
        "CsaladtagElrabolvaKialtal" => va::KIDNAP + 0x24,
        "KemKuldetesek" => va::MISSIONS,
        // Fleets and the rest of the starmap.
        "OsszHanyFlotta" => va::FLEET_COUNT,
        "OsszHanySajatFlotta" => va::FLEET_NUMBERS,
        "OsszHanyHajo" => va::SHIP_COUNT,
        "OsszHanyTank" => va::VEHICLE_COUNT,
        "Tankok" => va::VEHICLES,
        "OsszHanyKolonizalo" => va::COLONY_SHIP_POOL,
        "OsszHanyTerraformalo" => va::TERRAFORMER_POOL,
        "OsszHanyWormhole" => va::WORMHOLE_COUNT,
        "Wormhole" => va::WORMHOLES,
        "OsszHanyWaypoint" => va::WAYPOINT_COUNT,
        // Planets.
        "HanyBolygo" => va::PLANET_COUNT,
        "HanyBolygo2" => va::PLANET_COUNT2,
        "Bolygok" => va::PLANETS,
        "BolygoUtolsoKozeliFlotta" => va::LAST_NEAR_FLEET,
        "BolygoMennyiIdejeLazad" => va::UNREST,
        "BolygoMennyiFoldiEro" => va::GROUND,
        "BolygoMennyiErodEro" => va::FORTRESS,
        "BolygoMennyiIonGunEro" => va::ION_GUN,
        "BolygoMennyiTuzoltosag" => va::FIRE_BRIGADE,
        "BolygoNeededMoral" => va::NEEDED_MORALE,
        // Research.
        "KutatasSurgos" => va::URGENT,
        "ResearchAktFejlesztett" => va::RESEARCHING,
        "ResearchTerulet" => 0xB9A9B4,
        "ResearchMezo" => 0xB9AA04,
        "ResearchTalalmany" => 0xB9AA08,
        "LaborSzint" => va::LABS,
        "LaborSzintOsszes" => va::LAB_BUILDINGS,
        "DesignTervSorszam" => 0xB9AA10,
        "AktTervMutato" => 0xB9AAEC,
        "OsszHanyTerv" => va::DESIGN_COUNT,
        "AktEpuletTipus" => 0xB9C0C4,
        "HanyEpulet" => va::BUILDINGS_OCCUPIED,
        "HanyEpuletLefoglalva" => va::BUILDING_SLOTS,
        "HanyEpuletBlokk" => va::BUILDING_BLOCKS,
        // Relations and the AI.
        "Viszony" => va::RELATIONS,
        "DiplomaciaMikorHivtaUtoljara" => va::LAST_CONTACT,
        "MiTervMutato" => va::AI_DESIGNS,
        "MiTervMutatoSpec" => va::AI_DESIGNS_SPECIAL,
        "MiTervHanyVan" => 0xDAFDD8,
        "MiFoBolygo" => va::AI_MAIN_PLANET,
        "MiFoFlotta" => va::AI_MAIN_FLEET,
        "MiFelderitesRacs" => va::AI_SCOUT,
        "VanUresBolygo[0]" | "VanUresBolygo" => va::AI_EMPTY_PLANETS,
        "MiUtolsoSzarKolonizalas" => va::AI_LAST_BAD_COLONY,
        "MiMennyiIdeigNemPakolhatFelTankot" => va::AI_NO_LOAD,
        "MiMennyiIdeigTartATankPakoloFazis" => va::AI_LOAD_PHASE,
        "MiMennyiIdeigNemLehetTervUpgrade" => va::AI_NO_UPGRADE,
        "MiGyarthatoCuccok" => va::AI_CATEGORIES,
        "MiBirodalomKozeppontX" => va::AI_CENTRE_X,
        "MiBirodalomKozeppontY" => va::AI_CENTRE_Y,
        _ => return None,
    })
}

// ------------------------------------------------------------------------------------------------ state

/// The four story queues the save carries (`StoryVodor` by bin, `HistoryVodor`, `StartVodor`, `OlvasasVodor`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoryQueues {
    pub buckets: Vec<Queue>,
    pub history: Queue,
    pub starting: Queue,
    pub read: Queue,
    /// `VegzesVodor` / `TalalmanyVodor` (not saved; empty between ticks unless set from a live dump).
    pub finished: Queue,
    pub specials: Queue,
}

/// The whole game state of a save.
#[derive(Debug, Clone)]
pub struct SaveState {
    pub mem: Memory,
    /// `Flottak[0..=OsszHanyFlotta]`, `Hajok`, `Waypointek` (empty when the save has none), `Tervek[e]`,
    /// `Epuletek[p]` (index 0 of the outer vectors unused; each inner vector starts with slot 0).
    pub fleets: Vec<[u8; FLEET_SIZE]>,
    pub ships: Vec<[u8; 6]>,
    pub waypoints: Vec<[u8; 0xc]>,
    pub designs: Vec<Vec<[u8; DESIGN_SIZE]>>,
    pub buildings: Vec<Vec<[u8; BUILDING_SIZE]>>,
    pub story: StoryQueues,
    pub multiplayer_blob: bool,
    /// Every chunk of the loaded save in order (the template for [`SaveState::chunks`]).
    template: Vec<(String, Vec<u8>)>,
}

fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn rd_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}
fn rd_f32(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn put(b: &mut [u8], o: usize, v: &[u8]) {
    b[o..o + v.len()].copy_from_slice(v);
}

/// Story queue chunk names (`<queue>.n`, then `.key` / `.value` pairs).
const QUEUES: [&str; 4] = ["StoryVodor", "HistoryVodor", "StartVodor", "OlvasasVodor"];

impl SaveState {
    /// The state of `save`. `tech_defaults` = `Talalmanyok0` (`ig2_rules::Rules::tech_defaults`): the live tech
    /// table starts from it (`InitNewGame`) and the save overwrites each tech's per-empire state.
    pub fn from_save(save: &SaveGame, tech_defaults: &[u8]) -> Self {
        let mut mem = Memory::default();
        mem.write(va::TECHS, tech_defaults);
        let template: Vec<(String, Vec<u8>)> =
            save.chunks.iter().map(|c| (c.name.clone(), save.body[c.range.clone()].to_vec())).collect();
        let mut story = StoryQueues { buckets: vec![Queue::default(); story::BUCKETS], ..Default::default() };
        let mut pending: Option<(usize, i32)> = None;
        for (name, data) in &template {
            if let Some(q) = QUEUES.iter().position(|q| name.strip_suffix(".key") == Some(q)) {
                pending = Some((q, rd_i32(data, 0)));
                continue;
            }
            if let Some(q) = QUEUES.iter().position(|q| name.strip_suffix(".value") == Some(q)) {
                let (pq, key) = pending.take().expect("a queue key before its value");
                assert_eq!(pq, q);
                let value = rd_i32(data, 0);
                match q {
                    0 => story.buckets[(key as u32 & 0xff) as usize].entries.push((key, value)),
                    1 => story.history.entries.push((key, value)),
                    2 => story.starting.entries.push((key, value)),
                    _ => story.read.entries.push((key, value)),
                }
                continue;
            }
            if let Some(at) = chunk_va(name) {
                mem.write(at, data);
            }
        }
        // `LoadSave` sets `HanyBolygo2 = HanyBolygo`, `ENGRAFIKA = EN` and clears the mission table's +0x160.
        mem.set_i32(va::PLANET_COUNT2, mem.i32(va::PLANET_COUNT));
        mem.set_i32(va::MISSIONS + 0x160, 0);
        Self {
            mem,
            fleets: save.fleets.clone(),
            ships: save.ships.clone(),
            waypoints: save.waypoints.clone(),
            designs: save.designs.clone(),
            buildings: save.buildings.clone(),
            story,
            multiplayer_blob: save.multiplayer,
            template,
        }
    }

    /// Globals the save doesn't carry, as a live dump gave them: `(va, bytes)` runs.
    pub fn set_volatile(&mut self, runs: &[(u32, Vec<u8>)]) {
        for (at, b) in runs {
            self.mem.write(*at, b);
        }
    }

    /// The save's chunks for this state, grouped by name (repeated names concatenated in order), as
    /// [`group_chunks`] groups a parsed save. Chunks the state doesn't model keep the loaded save's bytes.
    pub fn chunks(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut add = |n: &str, b: &[u8]| out.entry(n.to_string()).or_default().extend_from_slice(b);
        let mut queues_done = false;
        for (name, data) in &self.template {
            if QUEUES.iter().any(|q| name.starts_with(q) && name[q.len()..].starts_with('.')) {
                if !queues_done {
                    queues_done = true;
                    let s = &self.story;
                    let all: Vec<(i32, i32)> = s.buckets.iter().flat_map(|q| q.entries.iter().copied()).collect();
                    for (q, list) in QUEUES.iter().zip([&all, &s.history.entries, &s.starting.entries, &s.read.entries]) {
                        add(&format!("{q}.n"), &(list.len() as i32).to_le_bytes());
                        for &(k, v) in list.iter() {
                            add(&format!("{q}.key"), &k.to_le_bytes());
                            add(&format!("{q}.value"), &v.to_le_bytes());
                        }
                    }
                }
                continue;
            }
            let bytes = match name.as_str() {
                "Flottak" => self.fleets.concat(),
                "Hajok" => self.ships.concat(),
                "Waypointek" => self.waypoints.concat(),
                "Tankok" => self.mem.read(va::VEHICLES, 8 * (self.mem.i32(va::VEHICLE_COUNT).max(0) as usize + 1)),
                "Bolygok" => self.mem.read(va::PLANETS, PLANET_SIZE * (self.mem.i32(va::PLANET_COUNT).max(0) as usize + 1)),
                n if n.starts_with("Tervek[") => self.designs[index_of(n)].concat(),
                n if n.starts_with("Epuletek[") => self.buildings[index_of(n)].concat(),
                n if n.starts_with("Kemek[") => {
                    let e = index_of(n);
                    let count = self.mem.i32(va::SPY_COUNT + 4 * e as u32).max(0) as usize + 1;
                    self.mem.read(chunk_va(n).unwrap(), count * RECORD)
                }
                n if n.ends_with(".n") && n.starts_with('s') && chunk_va(n).is_some() => self.mem.read(chunk_va(n).unwrap(), 4),
                "sBolygok" | "sSzamlalok" | "sLogikaiak" | "sValtozok" => {
                    let k = ["sBolygok", "sSzamlalok", "sLogikaiak", "sValtozok"].iter().position(|x| x == name).unwrap();
                    let n = self.mem.i32(va::STORY_LIST_COUNTS + 4 * k as u32).max(0) as usize;
                    self.mem.read(chunk_va(name).unwrap(), (n + 1) * 8)
                }
                "st_vOzonviz" => self.mem.read(va::STORY_UNDO, 0xc * self.mem.i32(va::STORY_UNDO_COUNT).max(0) as usize),
                // A count saved with its array: from the array.
                "OsszHanyFlotta" => ((self.fleets.len() as i32 - 1).max(0)).to_le_bytes().to_vec(),
                "OsszHanyHajo" => ((self.ships.len() as i32 - 1).max(0)).to_le_bytes().to_vec(),
                "OsszHanyWaypoint" => ((self.waypoints.len() as i32 - 1).max(0)).to_le_bytes().to_vec(),
                // `VanUresBolygo[0]` is saved before the whole array.
                "VanUresBolygo[0]" => self.mem.read(va::AI_EMPTY_PLANETS, 1),
                n => match chunk_va(n) {
                    Some(at) => self.mem.read(at, data.len()),
                    None => data.clone(),
                },
            };
            add(name, &bytes);
        }
        out
    }
}

/// Whether the state models chunk `name` (a mapped global, a pointer array or a story queue); other chunks
/// pass through [`SaveState::chunks`] as loaded.
pub fn modelled(name: &str) -> bool {
    chunk_va(name).is_some()
        || matches!(name, "Flottak" | "Hajok" | "Waypointek")
        || name.starts_with("Tervek[")
        || name.starts_with("Epuletek[")
        || QUEUES.iter().any(|q| name.starts_with(q) && name[q.len()..].starts_with('.'))
}

/// The index in `Name[i]`.
fn index_of(name: &str) -> usize {
    let s = name.split_once('[').unwrap().1;
    s[..s.find(']').unwrap()].parse().unwrap()
}

/// A parsed save's chunks grouped by name, the form [`SaveState::chunks`] returns.
pub fn group_chunks(save: &SaveGame) -> BTreeMap<String, Vec<u8>> {
    let mut out: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for c in &save.chunks {
        out.entry(c.name.clone()).or_default().extend_from_slice(&save.body[c.range.clone()]);
    }
    out
}

// ------------------------------------------------------------------------------------------------ views

/// A planet record and its building slots (`colony-economy.md` §2).
pub fn planet_from(r: &[u8], slots: &[[u8; BUILDING_SIZE]], unrest: i32) -> Planet {
    let mut p = Planet {
        x: rd_f32(r, 0x18),
        y: rd_f32(r, 0x1c),
        owner: r[0x20],
        race: r[0x21],
        surface: r[0x22],
        colony_map: r[0x23],
        richness: rd_i32(r, 0x24),
        population: rd_i32(r, 0x28),
        population_change: rd_i32(r, 0x2c),
        morale: rd_i32(r, 0x30),
        tax_level: r[0x34],
        gdp_acc: rd_i32(r, 0x38),
        gdp_last: rd_i32(r, 0x3c),
        tax_acc: rd_i32(r, 0x40),
        tax_last: rd_i32(r, 0x44),
        energy_produced: rd_i32(r, 0x48),
        energy_demand: rd_i32(r, 0x4c),
        workers_demand: rd_i32(r, 0x50),
        field_54: u16::from_le_bytes([r[0x54], r[0x55]]),
        field_58: rd_i32(r, 0x58),
        field_5c: rd_i32(r, 0x5c),
        field_60: rd_i32(r, 0x60),
        protected: r[0x64] != 0,
        autobuild_counter: rd_i32(r, 0x68),
        autobuild_mode: r[0x6c],
        autobuild_aggr: r[0x6d],
        ship_capacity: rd_i32(r, 0x70),
        vehicle_capacity: rd_i32(r, 0x74),
        production: r[0x78..0xb8].try_into().unwrap(),
        epidemic: rd_i16(r, 0xb8),
        stations: std::array::from_fn(|k| rd_i16(r, 0x100 + 2 * k)),
        field_114: rd_i32(r, 0x114),
        morale_penalty: rd_i32(r, 0x174),
        morale_penalty_rate: rd_i32(r, 0x178),
        loyal: r[0x186] != 0,
        ai_proof: r[0x187] != 0,
        unrest,
        ..Default::default()
    };
    p.spies.copy_from_slice(&r[0xc4..0xc4 + 19]);
    for e in 0..19 {
        p.visibility[e] = rd_i16(r, 0xd8 + 2 * e);
    }
    for k in 1..p.problems.len() {
        p.problems[k] = rd_i32(r, 0x118 + 4 * k);
    }
    for k in 0..5 {
        p.events[k] = rd_i16(r, 0x16a + 2 * k);
    }
    p.buildings = slots.iter().map(|s| building_from(s)).collect();
    p
}

/// Write the fields [`planet_from`] reads back into the record (other bytes stay).
pub fn put_planet(r: &mut [u8], p: &Planet) {
    put(r, 0x18, &p.x.to_le_bytes());
    put(r, 0x1c, &p.y.to_le_bytes());
    r[0x20] = p.owner;
    r[0x21] = p.race;
    r[0x22] = p.surface;
    r[0x23] = p.colony_map;
    for (o, v) in [(0x24, p.richness), (0x28, p.population), (0x2c, p.population_change), (0x30, p.morale), (0x38, p.gdp_acc),
        (0x3c, p.gdp_last), (0x40, p.tax_acc), (0x44, p.tax_last), (0x48, p.energy_produced), (0x4c, p.energy_demand),
        (0x50, p.workers_demand), (0x58, p.field_58), (0x5c, p.field_5c), (0x60, p.field_60), (0x68, p.autobuild_counter),
        (0x70, p.ship_capacity), (0x74, p.vehicle_capacity), (0x114, p.field_114), (0x174, p.morale_penalty),
        (0x178, p.morale_penalty_rate)] {
        put(r, o, &v.to_le_bytes());
    }
    r[0x34] = p.tax_level;
    put(r, 0x54, &p.field_54.to_le_bytes());
    // `protected` is the whole byte in the exe: keep a nonzero value as it was.
    if p.protected != (r[0x64] != 0) {
        r[0x64] = p.protected as u8;
    }
    r[0x6c] = p.autobuild_mode;
    r[0x6d] = p.autobuild_aggr;
    put(r, 0x78, &p.production);
    put(r, 0xb8, &p.epidemic.to_le_bytes());
    put(r, 0xc4, &p.spies);
    for e in 0..19 {
        put(r, 0xd8 + 2 * e, &p.visibility[e].to_le_bytes());
    }
    for (k, s) in p.stations.iter().enumerate() {
        put(r, 0x100 + 2 * k, &s.to_le_bytes());
    }
    for k in 1..p.problems.len() {
        put(r, 0x118 + 4 * k, &p.problems[k].to_le_bytes());
    }
    for k in 0..5 {
        put(r, 0x16a + 2 * k, &p.events[k].to_le_bytes());
    }
    if p.loyal != (r[0x186] != 0) {
        r[0x186] = p.loyal as u8;
    }
    if p.ai_proof != (r[0x187] != 0) {
        r[0x187] = p.ai_proof as u8;
    }
}

pub fn building_from(s: &[u8]) -> Building {
    Building {
        kind: s[0],
        owner: s[1],
        damage: rd_i32(s, 4),
        work_left: u16::from_le_bytes([s[8], s[9]]),
        planet: s[0xa],
        x: rd_f32(s, 0xc),
        y: rd_f32(s, 0x10),
        yield_pct: s[0x18],
        flags: s[0x19],
        shown_energy: rd_i16(s, 0x1a),
        shown_workers: rd_i16(s, 0x1c),
        efficiency: rd_i16(s, 0x1e),
    }
}

pub fn put_building(s: &mut [u8], b: &Building) {
    s[0] = b.kind;
    s[1] = b.owner;
    put(s, 4, &b.damage.to_le_bytes());
    put(s, 8, &b.work_left.to_le_bytes());
    s[0xa] = b.planet;
    put(s, 0xc, &b.x.to_le_bytes());
    put(s, 0x10, &b.y.to_le_bytes());
    s[0x18] = b.yield_pct;
    s[0x19] = b.flags;
    put(s, 0x1a, &b.shown_energy.to_le_bytes());
    put(s, 0x1c, &b.shown_workers.to_le_bytes());
    put(s, 0x1e, &b.efficiency.to_le_bytes());
}

pub fn fleet_totals_put(r: &mut [u8], t: &FleetTotals) {
    for (o, v) in [(0x54, t.ships), (0x58, t.fighters), (0x5c, t.ground), (0x60, t.attack), (0x64, t.fighter_attack),
        (0x68, t.ground_strength), (0x6c, t.hp), (0x70, t.fighter_hp), (0x74, t.field_74), (0x78, t.capacity)] {
        put(r, o, &v.to_le_bytes());
    }
}

/// Write a starmap fleet's fields back (aggregates from ships at +0x54.. are the combat view's).
pub fn put_fleet(r: &mut [u8], f: &Fleet) {
    put(r, 0, &f.name);
    r[0x15] = f.owner;
    put(r, 0x16, &f.story_id.to_le_bytes());
    r[0x18] = f.state;
    put(r, 0x19, &f.visible);
    for (o, v) in [(0x30, f.x), (0x34, f.y), (0x38, f.speed), (0x3c, f.speed_override), (0x44, f.tx), (0x48, f.ty), (0x7c, f.radar)] {
        put(r, o, &v.to_le_bytes());
    }
    for (o, v) in [(0x40, f.order), (0x42, f.kind), (0x4c, f.colony_ships), (0x4e, f.terraformers), (0x50, f.first_waypoint)] {
        put(r, o, &v.to_le_bytes());
    }
    put(r, 0x52, &f.flags.to_le_bytes());
}

pub fn combat_fleet(r: &[u8]) -> CombatFleet {
    CombatFleet {
        owner: r[0x15],
        flags: u16::from_le_bytes([r[0x52], r[0x53]]),
        x: rd_f32(r, 0x30),
        y: rd_f32(r, 0x34),
        totals: FleetTotals::from_record(r),
        colony_ships: rd_i16(r, 0x4c),
        terraformers: rd_i16(r, 0x4e),
    }
}

pub fn wormhole_from(w: &[u8]) -> Wormhole {
    Wormhole {
        ax: rd_f32(w, 0),
        ay: rd_f32(w, 4),
        revealed_a: w[8] != 0,
        fleet_a: rd_i16(w, 10),
        bx: rd_f32(w, 0xc),
        by: rd_f32(w, 0x10),
        revealed_b: w[0x14] != 0,
        fleet_b: rd_i16(w, 0x16),
    }
}

pub fn put_wormhole(r: &mut [u8], w: &Wormhole) {
    put(r, 0, &w.ax.to_le_bytes());
    put(r, 4, &w.ay.to_le_bytes());
    r[8] = w.revealed_a as u8;
    put(r, 10, &w.fleet_a.to_le_bytes());
    put(r, 0xc, &w.bx.to_le_bytes());
    put(r, 0x10, &w.by.to_le_bytes());
    r[0x14] = w.revealed_b as u8;
    put(r, 0x16, &w.fleet_b.to_le_bytes());
}

fn message(b: &[u8], o: usize) -> Message {
    Message { cat: rd_i32(b, o), sub: rd_i32(b, o + 4), value: rd_i32(b, o + 8), sub2: rd_i32(b, o + 12), value2: rd_i32(b, o + 16) }
}
fn put_message(b: &mut [u8], o: usize, m: &Message) {
    for (k, v) in [m.cat, m.sub, m.value, m.sub2, m.value2].into_iter().enumerate() {
        put(b, o + 4 * k, &v.to_le_bytes());
    }
}

/// An answer record (`diplomacy.md` §10.1) at `o`.
fn answer(b: &[u8], o: usize) -> Answer {
    Answer {
        from: rd_i32(b, o),
        to: rd_i32(b, o + 4),
        msg: Message { cat: rd_i32(b, o + 8), sub: rd_i32(b, o + 0x14), value: rd_i32(b, o + 0x18), sub2: rd_i32(b, o + 0x1c), value2: rd_i32(b, o + 0x20) },
        counter: b[o + 0x10] != 0,
        reply: b[o + 0x11] != 0,
        yes: b[o + 0x12] != 0,
    }
}
/// A flag byte: written only when its truth changes (the bytes of slot −1 alias other data).
fn put_flag(b: &mut [u8], o: usize, on: bool) {
    if (b[o] != 0) != on {
        b[o] = on as u8;
    }
}

fn put_answer(b: &mut [u8], o: usize, a: &Answer) {
    put(b, o, &a.from.to_le_bytes());
    put(b, o + 4, &a.to.to_le_bytes());
    put(b, o + 8, &a.msg.cat.to_le_bytes());
    put_flag(b, o + 0x10, a.counter);
    put_flag(b, o + 0x11, a.reply);
    put_flag(b, o + 0x12, a.yes);
    for (k, v) in [a.msg.sub, a.msg.value, a.msg.sub2, a.msg.value2].into_iter().enumerate() {
        put(b, o + 0x14 + 4 * k, &v.to_le_bytes());
    }
}

pub fn player_slot(b: &[u8]) -> PlayerSlot {
    PlayerSlot {
        empire: rd_i32(b, 0),
        screen: b[0x0c],
        open: b[0x14],
        asked: b[0x16],
        waiting: b[0x17],
        answered: b[0x18],
        countered: b[0x2c],
        letter: Letter { from: rd_i32(b, 0x38), to: rd_i32(b, 0x3c), declaration: b[0x30] != 0, msg: message(b, 0x40) },
        cancelled: b[0x58],
        answer: answer(b, 0x64),
        counter_answer: answer(b, 0x94),
    }
}

pub fn put_player_slot(b: &mut [u8], s: &PlayerSlot) {
    put(b, 0, &s.empire.to_le_bytes());
    b[0x0c] = s.screen;
    b[0x14] = s.open;
    b[0x16] = s.asked;
    b[0x17] = s.waiting;
    b[0x18] = s.answered;
    b[0x2c] = s.countered;
    put_flag(b, 0x30, s.letter.declaration);
    put(b, 0x38, &s.letter.from.to_le_bytes());
    put(b, 0x3c, &s.letter.to.to_le_bytes());
    put_message(b, 0x40, &s.letter.msg);
    b[0x58] = s.cancelled;
    put_answer(b, 0x64, &s.answer);
    put_answer(b, 0x94, &s.counter_answer);
}

impl SaveState {
    pub fn planet_count(&self) -> usize {
        self.mem.i32(va::PLANET_COUNT).max(0) as usize
    }
    fn planet_record(&self, p: usize) -> Vec<u8> {
        self.mem.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE)
    }
    fn per_planet(&self, at: u32) -> Vec<i32> {
        (0..PLANET_SLOTS).map(|p| self.mem.i32(at + 4 * p as u32)).collect()
    }
    /// The used building slots of planet `p` (`Epuletek[p][1..=HanyEpuletLefoglalva[p]]`).
    fn slots(&self, p: usize) -> &[[u8; BUILDING_SIZE]] {
        let n = self.mem.i32(va::BUILDING_SLOTS + 4 * p as u32).max(0) as usize;
        self.buildings.get(p).map_or(&[][..], |b| &b[1.min(b.len())..(n + 1).min(b.len())])
    }

    pub fn human_controlled(&self) -> [bool; EMPIRES] {
        self.mem.bytes::<EMPIRES>(va::PLAYER_KI).map(|k| k == 1)
    }
    pub fn races(&self) -> [u8; EMPIRES] {
        self.mem.bytes(va::PLAYER_FAJ)
    }
    pub fn alive(&self) -> [bool; EMPIRES] {
        self.mem.flags(va::EMPIRE_ALIVE)
    }

    pub fn treasuries(&self) -> [Treasury; EMPIRES] {
        let m = &self.mem;
        std::array::from_fn(|e| {
            let six = |at: u32| -> [f64; 6] { std::array::from_fn(|k| m.f64(at + (e * 48 + 8 * k) as u32)) };
            Treasury { total: m.f64(va::MONEY + 8 * e as u32), pools: six(va::POOLS), reserve: six(va::RESERVES), share: six(va::SHARES) }
        })
    }
    pub fn store_treasuries(&mut self, money: &[Treasury; EMPIRES]) {
        for (e, t) in money.iter().enumerate() {
            self.mem.set_f64(va::MONEY + 8 * e as u32, t.total);
            for k in 0..6 {
                let o = (e * 48 + 8 * k) as u32;
                self.mem.set_f64(va::POOLS + o, t.pools[k]);
                self.mem.set_f64(va::RESERVES + o, t.reserve[k]);
                self.mem.set_f64(va::SHARES + o, t.share[k]);
            }
        }
    }

    pub fn tech(&self) -> TechTable {
        TechTable { bytes: self.mem.read(va::TECHS, TECHS * TECH_SIZE) }
    }
    pub fn store_tech(&mut self, t: &TechTable) {
        self.mem.write(va::TECHS, &t.bytes);
    }

    pub fn rng(&self) -> Streams {
        let mut r = Streams::new(21);
        for i in 0..21 {
            r.set_state(i, self.mem.i32(va::RAND_SEED + 4 * i as u32) as u32);
        }
        r.set_choice_state(self.mem.i32(va::RND_N), self.mem.i32(va::RND_TOTAL));
        r
    }
    pub fn store_rng(&mut self, r: &Streams) {
        for i in 0..21 {
            self.mem.set_i32(va::RAND_SEED + 4 * i as u32, r.state(i) as i32);
        }
        let (n, total) = r.choice_state();
        self.mem.set_i32(va::RND_N, n);
        self.mem.set_i32(va::RND_TOTAL, total);
    }

    /// The colony side systems' world (planets 0..=HanyBolygo with their buildings, money, techs, contacts, the
    /// per-empire and per-planet tables).
    pub fn colony(&self) -> ColonyWorld {
        let n = self.planet_count();
        let unrest = self.per_planet(va::UNREST);
        let planets = (0..=n).map(|p| planet_from(&self.planet_record(p), if p == 0 { &[] } else { self.slots(p) }, unrest[p])).collect();
        let m = &self.mem;
        let mut w = ColonyWorld {
            planets,
            occupied: self.per_planet(va::BUILDINGS_OCCUPIED),
            needed_morale: self.per_planet(va::NEEDED_MORALE),
            last_near_fleet: self.per_planet(va::LAST_NEAR_FLEET),
            money: self.treasuries(),
            tech: self.tech(),
            alive: self.alive(),
            all_techs: m.i32(va::ALL_TECHS) != 0,
            difficulty: m.i32(va::NEHEZSEG).max(0) as usize,
            ..Default::default()
        };
        w.strength.ground = self.per_planet(va::GROUND);
        w.strength.fortress = self.per_planet(va::FORTRESS);
        w.strength.ion_gun = self.per_planet(va::ION_GUN);
        w.strength.fire_brigade = self.per_planet(va::FIRE_BRIGADE);
        let t = &mut w.empires;
        for e in 0..EMPIRES {
            t.labs[e] = m.i32s(va::LABS + 16 * e as u32);
            t.lab_buildings[e] = m.i32s(va::LAB_BUILDINGS + 16 * e as u32);
        }
        t.spy_capacity = m.i32s(va::SPY_CAPACITY);
        t.spy_centres = m.i32s(va::SPY_CENTRES);
        for e in 0..9 {
            for p in 0..PLANET_SLOTS {
                t.spy_centres_by_planet[e][p] = m.i32(va::SPY_CENTRES_BY_PLANET + 4 * (e * PLANET_SLOTS + p) as u32);
            }
        }
        t.trade_ports = m.i32s(va::TRADE_PORTS);
        t.full_gdp = m.i32s(va::FULL_GDP);
        t.full_tax = m.i32s(va::FULL_TAX);
        t.tax_this_year = m.i32s(va::TAX_THIS);
        t.tax_last_year = m.i32s(va::TAX_LAST);
        t.trade_this_year = m.i32s(va::TRADE_THIS);
        t.trade_last_year = m.i32s(va::TRADE_LAST);
        for a in 0..EMPIRES {
            w.knows[a] = m.bytes(va::EMPIRE_KNOWS + (a * EMPIRES) as u32);
        }
        w.autobuild_pick = m.i32s(va::AUTOBUILD_PICK);
        w.map_cursor = m.bytes(va::MAP_CURSOR);
        w
    }

    pub fn store_colony(&mut self, w: &ColonyWorld) {
        for (p, pl) in w.planets.iter().enumerate() {
            let mut r = self.planet_record(p);
            put_planet(&mut r, pl);
            self.mem.write(va::PLANETS + (p * PLANET_SIZE) as u32, &r);
            if p == 0 {
                continue;
            }
            if self.buildings.len() <= p {
                self.buildings.resize(p + 1, Vec::new());
            }
            let slots = &mut self.buildings[p];
            if slots.is_empty() {
                slots.push([0; BUILDING_SIZE]);
            }
            if slots.len() < pl.buildings.len() + 1 {
                slots.resize(pl.buildings.len() + 1, [0; BUILDING_SIZE]);
            }
            for (j, b) in pl.buildings.iter().enumerate() {
                put_building(&mut slots[j + 1], b);
            }
            let blocks = slots.len() as i32 - 1;
            self.mem.set_i32(va::BUILDING_SLOTS + 4 * p as u32, pl.buildings.len() as i32);
            if self.mem.i32(va::BUILDING_BLOCKS + 4 * p as u32) < blocks {
                self.mem.set_i32(va::BUILDING_BLOCKS + 4 * p as u32, blocks);
            }
            self.mem.set_i32(va::UNREST + 4 * p as u32, pl.unrest);
        }
        let n = w.planets.len().min(PLANET_SLOTS);
        for (at, v) in [(va::BUILDINGS_OCCUPIED, &w.occupied), (va::NEEDED_MORALE, &w.needed_morale), (va::LAST_NEAR_FLEET, &w.last_near_fleet),
            (va::GROUND, &w.strength.ground), (va::FORTRESS, &w.strength.fortress), (va::ION_GUN, &w.strength.ion_gun),
            (va::FIRE_BRIGADE, &w.strength.fire_brigade)] {
            self.mem.set_i32s(at, &v[..n.max(v.len().min(PLANET_SLOTS))]);
        }
        self.store_treasuries(&w.money);
        self.store_tech(&w.tech);
        let alive: Vec<u8> = w.alive.iter().map(|&a| a as u8).collect();
        // `BirodalomEl` holds more than a flag in places: only clear what died.
        for (e, &a) in alive.iter().enumerate() {
            let at = va::EMPIRE_ALIVE + e as u32;
            if (self.mem.u8(at) != 0) != (a != 0) {
                self.mem.set_u8(at, a);
            }
        }
        let t = &w.empires;
        for e in 0..EMPIRES {
            self.mem.set_i32s(va::LABS + 16 * e as u32, &t.labs[e]);
            self.mem.set_i32s(va::LAB_BUILDINGS + 16 * e as u32, &t.lab_buildings[e]);
        }
        self.mem.set_i32s(va::SPY_CAPACITY, &t.spy_capacity);
        self.mem.set_i32s(va::SPY_CENTRES, &t.spy_centres);
        for e in 0..9 {
            self.mem.set_i32s(va::SPY_CENTRES_BY_PLANET + 4 * (e * PLANET_SLOTS) as u32, &t.spy_centres_by_planet[e]);
        }
        self.mem.set_i32s(va::TRADE_PORTS, &t.trade_ports);
        for (at, v) in [(va::FULL_GDP, &t.full_gdp), (va::FULL_TAX, &t.full_tax), (va::TAX_THIS, &t.tax_this_year),
            (va::TAX_LAST, &t.tax_last_year), (va::TRADE_THIS, &t.trade_this_year), (va::TRADE_LAST, &t.trade_last_year)] {
            self.mem.set_i32s(at, v);
        }
        for a in 0..EMPIRES {
            self.mem.write(va::EMPIRE_KNOWS + (a * EMPIRES) as u32, &w.knows[a]);
        }
        self.mem.set_i32s(va::AUTOBUILD_PICK, &w.autobuild_pick);
        self.mem.write(va::MAP_CURSOR, &w.map_cursor);
    }

    /// The starmap's fleets (slot 0 included), waypoints, wormholes, planets, radar tables, contacts and pools.
    pub fn starmap(&self) -> crate::fleet::Starmap {
        let m = &self.mem;
        let n = self.planet_count();
        let planets = (0..=n)
            .map(|p| {
                let r = self.planet_record(p);
                let slots = if p == 0 { &[][..] } else { self.slots(p) };
                let count = |kind: u8| {
                    slots.iter().filter(|s| s[0] == kind && rd_i16(&s[..], 8) == 0 && s[0x19] & 1 != 0).count() as i32
                };
                StarPlanet {
                    x: rd_f32(&r, 0x18),
                    y: rd_f32(&r, 0x1c),
                    owner: r[0x20],
                    epidemic: rd_i16(&r, 0xb8),
                    trade_centres: count(48),
                    banks: count(49),
                    trade_ports: count(50),
                    radar_level: r[0x6e],
                    has_hub: slots.iter().any(|s| s[0] == 1 && rd_i16(&s[..], 8) == 0),
                    visible: std::array::from_fn(|e| rd_i16(&r, 0xd8 + 2 * e)),
                    notice: r[0xbb],
                }
            })
            .collect();
        let nw = m.i32(va::WORMHOLE_COUNT).max(0) as usize;
        let mut s = crate::fleet::Starmap {
            fleets: self.fleets.iter().map(|r| Fleet::from_record(r)).collect(),
            waypoints: self.waypoints.iter().map(|w| Waypoint { x: rd_f32(w, 0), y: rd_f32(w, 4), next: rd_i16(w, 8) }).collect(),
            wormholes: (0..=nw).map(|i| wormhole_from(&m.read(va::WORMHOLES + 0x1c * i as u32, 0x1c))).collect(),
            planets,
            colony_ship_pool: m.i32s(va::COLONY_SHIP_POOL),
            terraformer_pool: m.i32s(va::TERRAFORMER_POOL),
            ..Default::default()
        };
        for e in 0..EMPIRES {
            let o = (e * 32) as u32;
            s.radar.range[e] = m.f32s(va::RADAR2 + o);
            s.radar.range_sq[e] = m.f32s(va::RADAR2_SQ + o);
            s.radar.mult[e] = m.f32(va::RADAR_MULT + 4 * e as u32);
            s.radar.mult_sq[e] = m.f32(va::RADAR_MULT_SQ + 4 * e as u32);
            s.met[e] = m.bytes(va::EMPIRE_KNOWS + (e * EMPIRES) as u32);
        }
        s
    }

    pub fn store_starmap(&mut self, s: &crate::fleet::Starmap) {
        self.resize_fleets(s.fleets.len());
        for (r, f) in self.fleets.iter_mut().zip(&s.fleets) {
            put_fleet(r, f);
        }
        self.waypoints.resize(s.waypoints.len(), [0; 0xc]);
        for (r, w) in self.waypoints.iter_mut().zip(&s.waypoints) {
            put(r, 0, &w.x.to_le_bytes());
            put(r, 4, &w.y.to_le_bytes());
            put(r, 8, &w.next.to_le_bytes());
        }
        for (i, w) in s.wormholes.iter().enumerate() {
            let at = va::WORMHOLES + 0x1c * i as u32;
            let mut r = self.mem.read(at, 0x1c);
            put_wormhole(&mut r, w);
            self.mem.write(at, &r);
        }
        for (p, sp) in s.planets.iter().enumerate() {
            let at = va::PLANETS + (p * PLANET_SIZE) as u32;
            let mut r = self.mem.read(at, PLANET_SIZE);
            r[0x20] = sp.owner;
            r[0x6e] = sp.radar_level;
            r[0xbb] = sp.notice;
            for e in 0..19 {
                put(&mut r, 0xd8 + 2 * e, &sp.visible[e].to_le_bytes());
            }
            self.mem.write(at, &r);
        }
        let m = &mut self.mem;
        m.set_i32s(va::COLONY_SHIP_POOL, &s.colony_ship_pool);
        m.set_i32s(va::TERRAFORMER_POOL, &s.terraformer_pool);
        for e in 0..EMPIRES {
            let o = (e * 32) as u32;
            m.set_f32s(va::RADAR2 + o, &s.radar.range[e]);
            m.set_f32s(va::RADAR2_SQ + o, &s.radar.range_sq[e]);
            m.set_f32(va::RADAR_MULT + 4 * e as u32, s.radar.mult[e]);
            m.set_f32(va::RADAR_MULT_SQ + 4 * e as u32, s.radar.mult_sq[e]);
            m.write(va::EMPIRE_KNOWS + (e * EMPIRES) as u32, &s.met[e]);
        }
    }

    /// Grow or shrink `Flottak` to `n` records (a new slot is zero: the exe's uninitialised heap).
    fn resize_fleets(&mut self, n: usize) {
        self.fleets.resize(n, [0; FLEET_SIZE]);
        self.mem.set_i32(va::FLEET_COUNT, n as i32 - 1);
    }

    /// Ships with their designs' speed and radar, for the starmap's refresh ticks.
    pub fn ship_designs(&self) -> Vec<ShipDesign> {
        self.ships
            .iter()
            .skip(1)
            .filter_map(|h| {
                let (f, d) = (rd_i16(h, 0), rd_i16(h, 2));
                if d == 0x7fff || f < 1 || f as usize >= self.fleets.len() {
                    return None;
                }
                let owner = self.fleets[f as usize][0x15] as usize;
                let rec = self.designs.get(owner)?.get(d as usize)?;
                Some(ShipDesign { fleet: f as usize, speed: rd_f32(rec, 0x3c), design_type: rd_i32(rec, 0x48), radar: rec[0x6a], stealth: rec[0x6b] })
            })
            .collect()
    }

    /// Ships, vehicles, designs and the fleets' totals.
    pub fn combat(&self) -> crate::spacewar::CombatWorld {
        let m = &self.mem;
        let nt = m.i32(va::VEHICLE_COUNT).max(0) as usize;
        let tanks = m.read(va::VEHICLES, 8 * (nt + 1));
        crate::spacewar::CombatWorld {
            fleets: self.fleets.iter().map(|r| combat_fleet(r)).collect(),
            ships: self.ships.iter().map(|c| Ship { fleet: rd_i16(c, 0), design: rd_i16(c, 2) }).collect(),
            vehicles: tanks.chunks(8).map(|c| Vehicle { fleet: rd_i16(c, 0), design: rd_i16(c, 2), count: rd_i32(c, 4) }).collect(),
            designs: self.designs.iter().map(|d| d.iter().map(|r| Design::from_bytes(r)).collect()).collect(),
            human_controlled: self.human_controlled(),
            colony_ship_pool: m.i32s(va::COLONY_SHIP_POOL),
            terraformer_pool: m.i32s(va::TERRAFORMER_POOL),
            ..Default::default()
        }
    }

    pub fn store_combat(&mut self, w: &crate::spacewar::CombatWorld) {
        self.resize_fleets(self.fleets.len().max(w.fleets.len()));
        for (r, c) in self.fleets.iter_mut().zip(&w.fleets) {
            r[0x15] = c.owner;
            put(r, 0x52, &c.flags.to_le_bytes());
            put(r, 0x30, &c.x.to_le_bytes());
            put(r, 0x34, &c.y.to_le_bytes());
            fleet_totals_put(r, &c.totals);
            put(r, 0x4c, &c.colony_ships.to_le_bytes());
            put(r, 0x4e, &c.terraformers.to_le_bytes());
        }
        self.ships.resize(w.ships.len(), [0; 6]);
        for (r, s) in self.ships.iter_mut().zip(&w.ships) {
            put(r, 0, &s.fleet.to_le_bytes());
            put(r, 2, &s.design.to_le_bytes());
        }
        self.mem.set_i32(va::SHIP_COUNT, w.ships.len() as i32 - 1);
        let mut tanks = self.mem.read(va::VEHICLES, 8 * w.vehicles.len());
        for (i, v) in w.vehicles.iter().enumerate() {
            put(&mut tanks, 8 * i, &v.fleet.to_le_bytes());
            put(&mut tanks, 8 * i + 2, &v.design.to_le_bytes());
            put(&mut tanks, 8 * i + 4, &v.count.to_le_bytes());
        }
        self.mem.write(va::VEHICLES, &tanks);
        self.mem.set_i32(va::VEHICLE_COUNT, w.vehicles.len() as i32 - 1);
        if self.designs.len() < w.designs.len() {
            self.designs.resize(w.designs.len(), Vec::new());
        }
        for (e, ds) in w.designs.iter().enumerate() {
            let rec = &mut self.designs[e];
            rec.resize(ds.len(), [0; DESIGN_SIZE]);
            for (r, d) in rec.iter_mut().zip(ds) {
                *r = d.to_bytes();
            }
            if e > 0 {
                self.mem.set_i32(va::DESIGN_COUNT + 4 * e as u32, ds.len() as i32 - 1);
            }
        }
        self.mem.set_i32s(va::COLONY_SHIP_POOL, &w.colony_ship_pool);
        self.mem.set_i32s(va::TERRAFORMER_POOL, &w.terraformer_pool);
    }

    pub fn spies(&self) -> Spies {
        let m = &self.mem;
        let mut sp = Spies::default();
        for e in 0..ROSTERS {
            for j in 0..SLOTS {
                sp.roster[e][j] = Spy::from_bytes(&m.read(va::KEMEK + ((e * SLOTS + j) * RECORD) as u32, RECORD));
            }
        }
        sp.count = m.i32s(va::SPY_COUNT);
        sp.working = m.i32s(va::SPIES_WORKING);
        sp.enabled = (0..27).map(|k| m.u8(va::MISSIONS + 0x20 * k)).collect();
        sp
    }

    pub fn store_spies(&mut self, sp: &Spies) {
        for e in 0..ROSTERS {
            for j in 0..SLOTS {
                self.mem.write(va::KEMEK + ((e * SLOTS + j) * RECORD) as u32, &sp.roster[e][j].to_bytes());
            }
        }
        self.mem.set_i32s(va::SPY_COUNT, &sp.count);
        self.mem.set_i32s(va::SPIES_WORKING, &sp.working);
        for (k, &x) in sp.enabled.iter().enumerate() {
            self.mem.set_u8(va::MISSIONS + 0x20 * k as u32, x);
        }
    }

    pub fn relations(&self) -> Vec<[Relation; EMPIRES]> {
        let b = self.mem.read(va::RELATIONS, EMPIRES * EMPIRES * Relation::SIZE);
        (0..EMPIRES).map(|a| std::array::from_fn(|c| Relation::from_bytes(&b[(a * EMPIRES + c) * Relation::SIZE..]))).collect()
    }
    pub fn store_relations(&mut self, r: &[[Relation; EMPIRES]]) {
        for (a, row) in r.iter().enumerate() {
            for (c, x) in row.iter().enumerate() {
                self.mem.write(va::RELATIONS + ((a * EMPIRES + c) * Relation::SIZE) as u32, &x.to_bytes());
            }
        }
    }

    pub fn kidnap(&self) -> [i32; 18] {
        self.mem.i32s(va::KIDNAP)
    }
    pub fn store_kidnap(&mut self, k: &[i32; 18]) {
        self.mem.set_i32s(va::KIDNAP, k);
    }

    pub fn known_since(&self) -> [[i32; EMPIRES]; EMPIRES] {
        std::array::from_fn(|a| self.mem.i32s(va::EMPIRE_KNOWN_SINCE + (4 * a * EMPIRES) as u32))
    }
    pub fn store_known_since(&mut self, k: &[[i32; EMPIRES]; EMPIRES]) {
        for (a, row) in k.iter().enumerate() {
            self.mem.set_i32s(va::EMPIRE_KNOWN_SINCE + (4 * a * EMPIRES) as u32, row);
        }
    }

    /// Diplomacy's own state (relations, power, contacts, the contact list with slot −1).
    pub fn diplomacy(&self) -> Diplomacy {
        let m = &self.mem;
        let max = m.i32(va::MAX_PLAYERS).clamp(0, 20) as usize;
        Diplomacy {
            relations: self.relations(),
            power: m.f32s(va::EMPIRE_POWER),
            space_power: m.f32s(va::EMPIRE_SPACE_POWER),
            planets: m.i32s(va::EMPIRE_PLANETS),
            countdown: m.i32s(va::EMPIRE_COUNTDOWN),
            danger: m.i32s(va::AI_DANGER),
            last_contact: (0..LAST_CONTACT_CELLS).map(|i| m.i32(va::LAST_CONTACT + 4 * i as u32)).collect(),
            known_since: self.known_since(),
            players: (0..max).map(|i| player_slot(&m.read(va::CONTACTS + (i * PLAYER_SLOT) as u32, PLAYER_SLOT))).collect(),
            ghost: player_slot(&m.read(va::CONTACT_GHOST, PLAYER_SLOT)),
            ruler_age: m.i32s(va::RULER_AGE),
            greeted: m.bytes(va::GREETED),
            last_day: m.i32(va::DIP_OLD_NAP),
            started: m.u8(va::DIP_STARTED) != 0,
        }
    }

    pub fn store_diplomacy(&mut self, d: &Diplomacy) {
        self.store_relations(&d.relations);
        let m = &mut self.mem;
        m.set_f32s(va::EMPIRE_POWER, &d.power);
        m.set_f32s(va::EMPIRE_SPACE_POWER, &d.space_power);
        m.set_i32s(va::EMPIRE_PLANETS, &d.planets);
        m.set_i32s(va::EMPIRE_COUNTDOWN, &d.countdown);
        m.set_i32s(va::AI_DANGER, &d.danger);
        m.set_i32s(va::LAST_CONTACT, &d.last_contact);
        // Slot −1 first: the list's own slots win where they overlap nothing, the ghost's aliases land in the
        // texts before the list.
        let mut g = m.read(va::CONTACT_GHOST, PLAYER_SLOT);
        put_player_slot(&mut g, &d.ghost);
        m.write(va::CONTACT_GHOST, &g);
        m.set_i32s(va::RULER_AGE, &d.ruler_age);
        m.write(va::GREETED, &d.greeted);
        for (i, s) in d.players.iter().enumerate() {
            let at = va::CONTACTS + (i * PLAYER_SLOT) as u32;
            let mut b = m.read(at, PLAYER_SLOT);
            put_player_slot(&mut b, s);
            m.write(at, &b);
        }
        m.set_i32(va::MAX_PLAYERS, d.players.len() as i32);
        m.set_i32(va::DIP_OLD_NAP, d.last_day);
        m.set_u8(va::DIP_STARTED, d.started as u8);
        self.store_known_since(&d.known_since);
    }

    pub fn research(&self) -> Research {
        Research { current: self.mem.i32s(va::RESEARCHING), urgent: self.mem.bytes(va::URGENT) }
    }
    pub fn store_research(&mut self, r: &Research) {
        self.mem.set_i32s(va::RESEARCHING, &r.current);
        self.mem.write(va::URGENT, &r.urgent);
    }

    pub fn ai(&self) -> AiState {
        let m = &self.mem;
        let mut a = AiState {
            main_planet: m.i32s(va::AI_MAIN_PLANET),
            main_fleet: m.i32s(va::AI_MAIN_FLEET),
            centre_x: m.i32s(va::AI_CENTRE_X),
            centre_y: m.i32s(va::AI_CENTRE_Y),
            ..Default::default()
        };
        for e in 0..EMPIRES {
            for k in 0..DESIGN_SLOTS {
                a.designs[crate::ai::BEST][e][k] = m.i16(va::AI_DESIGNS + (e * 0x32 + 2 * k) as u32);
                a.designs[crate::ai::SPECIAL][e][k] = m.i16(va::AI_DESIGNS_SPECIAL + (e * 0x32 + 2 * k) as u32);
            }
            for c in 0..CATEGORIES {
                a.categories[e][c] = Category::from_bytes(&m.read(va::AI_CATEGORIES + (e * 0x18c + c * CATEGORY_SIZE) as u32, CATEGORY_SIZE));
            }
            a.sliders[e] = std::array::from_fn(|i| m.f64(va::SLIDERS + (e * 48 + 8 * i) as u32));
            for x in 0..GRID {
                a.scout[e][x] = m.bytes(va::AI_SCOUT + (e * GRID * GRID + x * GRID) as u32);
            }
        }
        a.empty_planets = m.bytes(va::AI_EMPTY_PLANETS);
        a.last_bad_colony = m.i32s(va::AI_LAST_BAD_COLONY);
        a.no_load = m.i32s(va::AI_NO_LOAD);
        a.load_phase = m.i32s(va::AI_LOAD_PHASE);
        a.no_upgrade = m.i32s(va::AI_NO_UPGRADE);
        a
    }

    pub fn store_ai(&mut self, a: &AiState) {
        let m = &mut self.mem;
        m.set_i32s(va::AI_MAIN_PLANET, &a.main_planet);
        m.set_i32s(va::AI_MAIN_FLEET, &a.main_fleet);
        m.set_i32s(va::AI_CENTRE_X, &a.centre_x);
        m.set_i32s(va::AI_CENTRE_Y, &a.centre_y);
        for e in 0..EMPIRES {
            for k in 0..DESIGN_SLOTS {
                m.write(va::AI_DESIGNS + (e * 0x32 + 2 * k) as u32, &a.designs[crate::ai::BEST][e][k].to_le_bytes());
                m.write(va::AI_DESIGNS_SPECIAL + (e * 0x32 + 2 * k) as u32, &a.designs[crate::ai::SPECIAL][e][k].to_le_bytes());
            }
            for c in 0..CATEGORIES {
                m.write(va::AI_CATEGORIES + (e * 0x18c + c * CATEGORY_SIZE) as u32, &a.categories[e][c].to_bytes());
            }
            for i in 0..6 {
                m.set_f64(va::SLIDERS + (e * 48 + 8 * i) as u32, a.sliders[e][i]);
            }
            for x in 0..GRID {
                m.write(va::AI_SCOUT + (e * GRID * GRID + x * GRID) as u32, &a.scout[e][x]);
            }
        }
        m.write(va::AI_EMPTY_PLANETS, &a.empty_planets);
        m.set_i32s(va::AI_LAST_BAD_COLONY, &a.last_bad_colony);
        m.set_i32s(va::AI_NO_LOAD, &a.no_load);
        m.set_i32s(va::AI_LOAD_PHASE, &a.load_phase);
        m.set_i32s(va::AI_NO_UPGRADE, &a.no_upgrade);
    }

    /// The story engine's state: the saved queues, tables, undo list and `StoryAdat`, plus the statics a live
    /// dump gave ([`SaveState::set_volatile`]).
    pub fn story(&self) -> Story {
        let m = &self.mem;
        let mut st = Story::default();
        let q = &self.story;
        st.buckets = q.buckets.clone();
        st.history = q.history.clone();
        st.starting = q.starting.clone();
        st.read = q.read.clone();
        st.finished = q.finished.clone();
        st.specials = q.specials.clone();
        let list = |k: u32, at: u32| -> Vec<(i32, i32)> {
            let n = m.i32(va::STORY_LIST_COUNTS + 4 * k).max(0) as u32;
            (1..=n).map(|i| (m.i32(at + 8 * i), m.i32(at + 8 * i + 4))).collect()
        };
        st.planets = list(0, va::STORY_PLANETS).into_iter().map(|(id, p)| (id, p as u8)).collect();
        st.counters.set_live(&list(1, va::STORY_COUNTERS));
        st.logic.set_live(&list(2, va::STORY_LOGIC).into_iter().map(|(id, v)| (id, v & 0xff)).collect::<Vec<_>>());
        st.vars.set_live(&list(3, va::STORY_VARS));
        st.error = m.i32(va::STORY_ERROR);
        for (k, f) in st.flags.iter_mut().enumerate() {
            *f = m.u8(va::STORY_FLAGS + k as u32) != 0;
        }
        st.last_checked = m.i32(va::STORY_LAST_CHECKED);
        st.counter_done = m.i32(va::STORY_COUNTER_DONE);
        st.old_ido = m.i32(va::STORY_OLD_IDO);
        st.delayed = m.u8(va::STORY_DELAYED) != 0;
        st.skip_first = m.i32(va::STORY_SKIP_FIRST);
        st.restarting = m.i32(va::STORY_RESTARTING);
        st.adat.copy_from_slice(&m.read(va::STORY_ADAT, story::ADAT));
        let n = m.i32(va::STORY_UNDO_COUNT).max(0) as u32;
        st.undo = (0..n).map(|i| {
            let at = va::STORY_UNDO + 0xc * i;
            (m.i32(at), m.i32(at + 4), m.i32(at + 8))
        }).collect();
        st.signal = (m.i32(va::STORY_SIGNAL), m.i32(va::STORY_SIGNAL + 4));
        st.last_addship = m.i32(va::STORY_LAST_ADDSHIP);
        st.special = m.i32s(va::STORY_SPECIAL);
        st.radar = (m.i32(va::STORY_RADAR), m.i32(va::STORY_RADAR + 4));
        st.diplomacy = (m.i32(va::STORY_DIPLOMACY), m.i32(va::STORY_DIPLOMACY + 4), m.i32(va::STORY_DIPLOMACY_ANSWER));
        let a: [i32; 5] = m.i32s(va::STORY_ARRIVAL);
        st.arrival = story::Arrival { fleet_id: a[0], fleet: a[1], target: a[2], target_type: a[3], target_fleet_id: a[4] };
        let g: [i32; 5] = m.i32s(va::STORY_GROUND);
        st.ground = story::GroundBattle {
            fleet: g[0],
            fleet_id: g[1],
            fleet_owner: g[2],
            planet: g[3],
            old_owner: g[4],
            attacker_won: m.u8(va::STORY_GROUND_WON) != 0,
            defender_won: m.u8(va::STORY_GROUND_WON + 1) != 0,
        };
        st.you_were_in_battle = m.u8(va::STORY_YOU_WERE_IN_BATTLE) != 0;
        st.spy_kind = m.i32(va::STORY_SPY_KIND);
        st
    }

    pub fn store_story(&mut self, st: &Story) {
        let q = &mut self.story;
        q.buckets = st.buckets.clone();
        q.history = st.history.clone();
        q.starting = st.starting.clone();
        q.read = st.read.clone();
        q.finished = st.finished.clone();
        q.specials = st.specials.clone();
        let m = &mut self.mem;
        // `sBolygok` and `sLogikaiak` hold a byte at +4 (the rest of the word is left as it was).
        let mut list = |k: u32, at: u32, entries: &[(i32, i32)], byte: bool| {
            m.set_i32(va::STORY_LIST_COUNTS + 4 * k, entries.len() as i32);
            for (i, &(id, v)) in entries.iter().enumerate() {
                let o = at + 8 * (i as u32 + 1);
                m.set_i32(o, id);
                if byte {
                    m.set_u8(o + 4, v as u8);
                } else {
                    m.set_i32(o + 4, v);
                }
            }
        };
        list(0, va::STORY_PLANETS, &st.planets.iter().map(|&(id, p)| (id, p as i32)).collect::<Vec<_>>(), true);
        list(1, va::STORY_COUNTERS, st.counters.live(), false);
        list(2, va::STORY_LOGIC, st.logic.live(), true);
        list(3, va::STORY_VARS, st.vars.live(), false);
        m.set_i32(va::STORY_ERROR, st.error);
        for (k, &f) in st.flags.iter().enumerate() {
            m.set_u8(va::STORY_FLAGS + k as u32, f as u8);
        }
        m.set_i32(va::STORY_LAST_CHECKED, st.last_checked);
        m.set_i32(va::STORY_COUNTER_DONE, st.counter_done);
        m.set_i32(va::STORY_OLD_IDO, st.old_ido);
        m.set_u8(va::STORY_DELAYED, st.delayed as u8);
        m.set_i32(va::STORY_SKIP_FIRST, st.skip_first);
        m.set_i32(va::STORY_RESTARTING, st.restarting);
        m.write(va::STORY_ADAT, &st.adat);
        m.set_i32(va::STORY_UNDO_COUNT, st.undo.len() as i32);
        for (i, &(a, b, c)) in st.undo.iter().enumerate() {
            m.set_i32s(va::STORY_UNDO + 0xc * i as u32, &[a, b, c]);
        }
        m.set_i32s(va::STORY_SIGNAL, &[st.signal.0, st.signal.1]);
        m.set_i32(va::STORY_LAST_ADDSHIP, st.last_addship);
        m.set_i32s(va::STORY_SPECIAL, &st.special);
        m.set_i32s(va::STORY_RADAR, &[st.radar.0, st.radar.1]);
        m.set_i32s(va::STORY_DIPLOMACY, &[st.diplomacy.0, st.diplomacy.1]);
        m.set_i32(va::STORY_DIPLOMACY_ANSWER, st.diplomacy.2);
        let a = &st.arrival;
        m.set_i32s(va::STORY_ARRIVAL, &[a.fleet_id, a.fleet, a.target, a.target_type, a.target_fleet_id]);
        let g = &st.ground;
        m.set_i32s(va::STORY_GROUND, &[g.fleet, g.fleet_id, g.fleet_owner, g.planet, g.old_owner]);
        m.write(va::STORY_GROUND_WON, &[g.attacker_won as u8, g.defender_won as u8]);
        m.set_u8(va::STORY_YOU_WERE_IN_BATTLE, st.you_were_in_battle as u8);
        m.set_i32(va::STORY_SPY_KIND, st.spy_kind);
    }
}

#[cfg(test)]
mod tests {
    use super::Memory;

    #[test]
    fn memory_merges_runs_and_reads_gaps_as_zero() {
        let mut m = Memory::default();
        m.write(10, &[1, 2, 3]);
        m.write(20, &[9]);
        assert_eq!(m.read(8, 6), [0, 0, 1, 2, 3, 0]);
        m.write(12, &[7, 7, 7, 7, 7, 7, 7, 7, 7]);
        assert_eq!(m.read(10, 11), [1, 2, 7, 7, 7, 7, 7, 7, 7, 7, 7]);
        m.write(5, &[4, 4, 4, 4, 4]);
        assert_eq!(m.read(5, 7), [4, 4, 4, 4, 4, 1, 2]);
        assert_eq!(m.runs.len(), 1);
    }
}
