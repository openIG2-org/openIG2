//! Ground combat (`WF*`): routing, landing, group arrangement and the auto-resolved ground war
//! (`GroundWarIndit` exe 0x59c2d0, `AddTanksToPlanet` 0x45e320, `WF::RearrangeTankGroups` 0x48ca00,
//! `WF::Simulate` 0x4a3730, `GroundWarFinished` 0x47c5d0). Spec: `re/rules/ground-combat.md`
//! (exe-normative; section numbers below refer to it).
//!
//! Every planet owns a persistent battlefield [`Field`] of unit groups ([`Group`]): the garrison and the
//! defence-building turrets live there, attackers land on it. The auto-resolve is a pass loop: every unit
//! loses 10 hit points per pass (attrition), guns fire on their period (`timing % period`), air strikes
//! and kamikazes hit up to three enemies. Seeded randomness is stream 2 ([`stream::GROUND_COMBAT`]);
//! the original's libc `rand()` calls (a voice pick per hit, one per new unit object, one per exploding
//! mine; none changes the outcome) come from the injected [`War::libc`] (ADR 0003).
//!
//! The battle AI (`WF::MI`, stream 13) is [`War::mi`] (`ground/ai.rs`, `re/rules/ground-ai.md`).
//! The real-time battle (spec §11) lives in `ground/{realtime, battle, tick, path, voices}.rs`, its
//! transporters (the dropships and the landing orders, §11.16) in `ground/transport.rs`.

use crate::colony::{flag as bflag, Building};
use crate::rng::{stream, Streams};
use crate::spacewar::{CombatWorld, Design, DesignRules, Vehicle, NO_DESIGN};

mod ai;
mod atjaras;
mod battle;
mod path;
mod realtime;
mod tick;
mod transport;
mod voices;
pub use ai::GroundAi;
pub use atjaras::{generate_map, Passability};
pub use battle::*;
pub use path::PathOut;
pub use realtime::*;
pub use tick::Clock;
pub use transport::drop;
pub use voices::{VoiceLine, Voices};

/// "No unit" (`10000` throughout `grwar.cpp`).
pub const NONE: i32 = 10000;
/// Groups per owner on one field (`GetFreeGroupSpaceC`).
pub const MAX_GROUPS: i32 = 20;
/// `GetStatus`: nobody left on the field.
pub const EMPTY: i32 = -10;
/// `GetStatus`: still contested.
pub const CONTESTED: i32 = -1;

/// Group flags (`WF_Tank +0x188`, spec §1.2).
pub mod flag {
    /// Damage ×1.2 (set by the constructor).
    pub const BONUS: u32 = 1;
    /// Not placed yet (in the transporter); a turret: destroyed.
    pub const NOT_PLACED: u32 = 2;
    /// A story "mad tank" (`StoryMadTankFrissit`, real time only).
    pub const MAD: u32 = 4;
    pub const MESH: u32 = 0x10;
    pub const MESH2: u32 = 0x20;
    pub const PLACED: u32 = 0x40;
    pub const HAS_TARGET: u32 = 0x100;
    pub const TARGET_AUX: u32 = 0x800;
    pub const PARTNER: u32 = 0x8000;
    pub const PARALYSED: u32 = 0x10000;
    pub const MINE: u32 = 0x20000;
    pub const STEALTH: u32 = 0x40000;
    pub const HULL2: u32 = 0x200000;
    pub const TRANSPORTER: u32 = 0x400000;
    pub const RADAR: u32 = 0x800000;
    pub const SEIZED: u32 = 0x1000000;
    pub const TURRET: u32 = 0x4000000;
    pub const LANDED: u32 = 0x8000000;
    pub const KEEP_ORDERS: u32 = 0x20000000;
    pub const NOT_COUNTED: u32 = 0x40000000;
    /// The six pod bits `CheckDesign` recomputes.
    pub const POD_BITS: u32 = 0x40000 | 0x80000 | 0x100000 | 0x200000 | 0x400000 | 0x800000;
}

/// Weapons (`WF_Tank +0x1d0`) and their `WF_Weapons` entry (spec §1.2).
pub mod weapon {
    pub const W1: u32 = 1;
    pub const W2: u32 = 2;
    pub const PARALYSER: u32 = 4;
    pub const BIG_SHIP: u32 = 8;
    pub const AMMO: u32 = 0x10;
    pub const FIGHTER: u32 = 0x20;
    pub const KAMIKAZE: u32 = 0x40;
    pub const BIG_KAMIKAZE: u32 = 0x80;
    pub const W100: u32 = 0x100;
    pub const W200: u32 = 0x200;
    pub const W400: u32 = 0x400;
    pub const W800: u32 = 0x800;
    pub const MANIPULATOR: u32 = 0x1000;
    pub const W2000: u32 = 0x2000;
    /// The guns that set `+0x130`.
    pub const GUNS: u32 = 0x100 | 0x200 | 2 | 0x400 | 0x800 | 1 | 0x2000 | 4 | 0x1000;
}

/// Rule values the ground war reads (from the user's exe, `ig2-rules`).
#[derive(Debug, Clone, Default)]
pub struct GroundRules {
    /// `WF_Weapons`: 14 × [min, max, period, unk] (spec §1.3).
    pub weapons: Vec<[i32; 4]>,
    /// Member hit points by design hull id (`0x8F1EC8 + 4·hull`).
    pub hull_hp: Vec<i32>,
    /// `EpuletekLeiro[type].+8` building hit points, by building type.
    pub building_hp: Vec<i32>,
    /// Battlefield size for map positions (exe 0x8EC960/0x8EC964).
    pub map_size: (f32, f32),
    /// Pod slot tables (`GetMaxPodok`).
    pub designs: DesignRules,
    /// Real-time weapons: `WF_NonSimulateWeapons` (units) and `WF_FortressWeapons` (turrets), 14 ×
    /// [min, max, reload, range] (spec §11).
    pub rt_weapons: Vec<[i32; 4]>,
    pub fortress: Vec<[i32; 4]>,
    /// Turning speeds by ground hull index (exe 0x8F8560).
    pub turn_speeds: Vec<f32>,
    /// Armour factors `[hull·3 + side]` (exe 0x8F858C).
    pub armour: Vec<f32>,
}

impl GroundRules {
    fn w(&self, k: usize) -> [i32; 4] {
        self.weapons.get(k).copied().unwrap_or([0, 0, 1, 0])
    }
}

/// One unit group (`WF_Tank`, exe 0x290 bytes; spec §1.2). Only the fields the strategic ground war
/// reads or writes; the rest belongs to the real-time battle.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Group {
    /// `+0x15c` design (of the owner's `Tervek`).
    pub design: i32,
    /// `+0x194` current owner, `+0x198` side (the original owner).
    pub owner: i32,
    pub side: i32,
    /// `+0x158` max hit points per member; `+0x154` members' hit points, `+0x19c` their origins
    /// (fleet index, −planet for a garrison, −1 for a partner).
    pub max_hp: i32,
    pub hp: Vec<i32>,
    pub origin: Vec<i32>,
    /// `+0x188` [`flag`], `+0x18c` flags2 (bit 1 speed reset pending, bit 2 not counted).
    pub flags: u32,
    pub flags2: u32,
    /// `+0x1d0` [`weapon`], `+0x11c` weapon pods, `+0x120` ammunition, `+0x1cc` regeneration,
    /// `+0x288` air-strike units, `+0x130` has a gun.
    pub weapons: u32,
    pub pods: i32,
    pub ammo: i32,
    pub regen: i32,
    pub air: i32,
    pub has_gun: bool,
    /// `+0x134` ground hull (design hull − 0x13), `+0x138` size before the last `AddTank`.
    pub hull: i32,
    pub prev_size: i32,
    /// `+0x1a0/+0x1a4` grid cell, `+0x1a8/+0x1ac` last cell, `+0x1b0/+0x1b4` move destination,
    /// `+0x1b8/+0x1bc` movement state, `+0x1d4/+0x1d8` map position.
    pub cell: (i32, i32),
    pub last_cell: (i32, i32),
    pub dest: (i32, i32),
    pub move_state: (i32, i32),
    pub pos: (f32, f32),
    /// `+0x1e0` carrier (of a partner), `+0x1e4` partner unit (−1 none).
    pub carrier: i32,
    pub partner: i32,
    /// `+0x1e8` paralysis and `+0x1ec` seizure countdowns.
    pub paralysed: i32,
    pub seized: i32,
    /// `+0x238` building slot (turrets), `+0x23c` target (with [`flag::HAS_TARGET`]), `+0x244` own index.
    pub building: i32,
    pub target: i32,
    pub index: i32,
    /// `+0x25c/+0x260` last hit (10000, attacker); `None` until the first hit (uninitialised in the exe).
    pub last_hit: Option<(i32, i32)>,
    /// `+0x27c` (cleared to 10000 by `RemoveTarget`).
    pub aim: i32,
    /// `+0x278` transporter state bits.
    pub transport: u32,
    /// The real-time fields (spec §11).
    pub rt: TankRt,
    /// Saved bytes no ported system reads.
    pub tail: UnitTail,
}

/// The bytes of a unit `WF_Tank::LoadSave` (exe 0x4859a0, `re/formats/save.md` §4.13) saves that no ported system
/// reads or writes, kept so that a battlefield written back holds them. Constructor values in [`Default`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnitTail {
    /// `+0x14` and `+0x274` (both uninitialised by the constructor), `+0xe8` (0), `+0x240` (2), `+0x270` (0).
    pub f14: u32,
    pub fe8: u32,
    pub f240: u32,
    pub f270: u32,
    pub f274: u32,
    /// `+0x13c..+0x146` and `+0x146` (zero).
    pub f13c: [u8; 10],
    pub f146: u8,
    /// A full record saves the member-origin pointer `+0x19c` itself as a scalar first: a heap address.
    pub origin_ptr: u32,
}

impl Default for UnitTail {
    fn default() -> Self {
        Self { f14: 0, fe8: 0, f240: 2, f270: 0, f274: 0, f13c: [0; 10], f146: 0, origin_ptr: 0 }
    }
}

/// The saved unit bytes `WF_Tank::WF_Tank` (exe 0x457020) and `WF::AddTank` leave uninitialised: `+0xc/+0x10` (the
/// render position; a turret's is never set), `+0x14`, `+0x18`
/// (aim), `+0x98/+0x9c` (driving direction), `+0x1b0/+0x1b4` (destination) and `+0x23c` (target) until orders are
/// given, `+0x1e0` (carrier), `+0x24c` (AI home), `+0x25c/+0x260` (last hit), `+0x274`, and the origin pointer a full
/// record saves at `+0x19c`. A replay takes them from the exe's save after the tick, which holds whatever the tick
/// wrote over them too.
pub const UNINIT: [usize; 15] = [0xc, 0x10, 0x14, 0x18, 0x98, 0x9c, 0x19c, 0x1b0, 0x1b4, 0x1e0, 0x23c, 0x24c, 0x25c, 0x260, 0x274];

impl Group {
    /// Take [`UNINIT`] fields from `pairs`: `(offset, value)` as two little-endian u32 each (the fields a source
    /// has; the others are left alone).
    pub fn fill_uninit(&mut self, pairs: &[u8]) {
        for p in pairs.chunks_exact(8) {
            let (o, v) = (u32::from_le_bytes(p[..4].try_into().unwrap()), u32::from_le_bytes(p[4..].try_into().unwrap()));
            let f = f32::from_bits(v);
            match o {
                0xc => self.rt.step.0 = f,
                0x10 => self.rt.step.1 = f,
                0x14 => self.tail.f14 = v,
                0x18 => self.rt.aim = f,
                0x98 => self.rt.dir.0 = f,
                0x9c => self.rt.dir.1 = f,
                0x19c => self.tail.origin_ptr = v,
                0x1b0 => self.dest.0 = v as i32,
                0x1b4 => self.dest.1 = v as i32,
                0x1e0 => self.carrier = v as i32,
                0x23c => self.target = v as i32,
                0x24c => self.rt.ai_home = v as i32,
                0x25c => self.last_hit = Some((v as i32, self.last_hit.map_or(0, |h| h.1))),
                0x260 => self.last_hit = Some((self.last_hit.map_or(0, |h| h.0), v as i32)),
                0x274 => self.tail.f274 = v,
                _ => {}
            }
        }
    }
}

/// The battlefield bytes `WF::LoadSave` (exe 0x485420) saves that no ported system reads or writes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FieldTail {
    /// `+0x18`, `+0x184` (a byte), `+0x18c` the planet (`WF_Tank::LoadSave`'s planet), `+0x1a8..+0x1b8`.
    pub f18: u32,
    pub f184: u8,
    pub planet: i32,
    pub f1a8: [u32; 5],
    /// `+0x1ec..+0x32c` as saved; the fighter wave ([`Lists::fighters`]) is written over it.
    pub strike: Vec<u8>,
    /// `+0x33c..+0x37c`: empire 0's (unused) AI block.
    pub ai0: Vec<u8>,
    /// The first 8 bytes of every list-B / list-C object (saved whole, not modelled), in list order.
    pub missile_heads: Vec<[u8; 8]>,
    pub bolt_heads: Vec<[u8; 8]>,
    /// Unit slots that got a new object (`operator new` + `WF_Tank::WF_Tank`) since the owner last drained this:
    /// their saved bytes the constructor leaves uninitialised ([`UNINIT`]) are heap garbage in the exe.
    pub created: Vec<usize>,
    /// Unit slots whose member arrays (`+0x154`/`+0x19c`) grew since the owner last drained this: `realloc` may have
    /// moved them, so the origin pointer the record saves (`+0x19c`) is heap state too.
    pub grown: Vec<usize>,
    /// Unit slots loaded without an object (`loadsave_ptr`'s flag 0): saved as absent until a unit takes the slot.
    /// Every other slot keeps its object after the unit dies (`RemoveTankGroup` 0x497c90 only clears `alive`).
    pub absent: Vec<bool>,
}

impl Default for FieldTail {
    fn default() -> Self {
        Self {
            f18: 0,
            f184: 0,
            planet: 0,
            f1a8: [0; 5],
            strike: vec![0; 0x140],
            ai0: vec![0; 0x40],
            missile_heads: Vec::new(),
            bolt_heads: Vec::new(),
            created: Vec::new(),
            grown: Vec::new(),
            absent: Vec::new(),
        }
    }
}

impl Default for Group {
    /// The constructor's values (`WF_Tank::WF_Tank` exe 0x457020).
    fn default() -> Self {
        Self {
            design: 0,
            owner: 0,
            side: 0,
            max_hp: 0,
            hp: Vec::new(),
            origin: Vec::new(),
            flags: flag::BONUS,
            flags2: 0,
            weapons: 0,
            pods: 0,
            ammo: 0,
            regen: 0,
            air: 0,
            has_gun: false,
            hull: -1,
            prev_size: -1,
            cell: (0, 0),
            last_cell: (0, 0),
            dest: (0, 0),
            move_state: (0, -1),
            pos: (0.0, 0.0),
            carrier: -1,
            partner: -1,
            paralysed: 0,
            seized: 0,
            building: 0,
            target: NONE,
            index: 0,
            last_hit: None,
            aim: NONE,
            transport: 1,
            rt: TankRt::default(),
            tail: UnitTail::default(),
        }
    }
}

impl Group {
    pub fn size(&self) -> i32 {
        self.hp.len() as i32
    }
    fn turret(&self) -> bool {
        self.flags & flag::TURRET != 0
    }
    /// A turret whose guns are gone (flags 2 and 0x4000000).
    pub fn destroyed_turret(&self) -> bool {
        self.flags & flag::NOT_PLACED != 0 && self.turret()
    }
    /// `(f64)(flags & 1)·0.2 + 1.0`.
    fn bonus(&self) -> f64 {
        (self.flags & flag::BONUS) as f64 * 0.2 + 1.0
    }
    /// Orders cleared (`WF_Tank::NoTarget` + `Stop`, inlined at a battle start and after a split).
    fn reset_orders(&mut self) {
        self.move_state = (0, -1);
        self.flags &= !0xd80;
        self.dest = self.cell;
        self.target = NONE;
    }
}

/// A planet's battlefield (`Warfare[p]`, exe `0x9C3244 + p·0x8EC`; spec §1.1). A dead slot keeps its last
/// unit's data (the exe's freed object), which only the original's undefined reads see.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Field {
    pub units: Vec<Group>,
    pub alive: Vec<bool>,
    /// `+0x338` the attacking fleet, `+0x334` the self-destruct countdown (−1 idle; deleting the `'+'`
    /// building cancels it; `FRISSIT_Grwar` counts it down, [`War::grwar_planet`]).
    pub attack_fleet: i32,
    pub self_destruct: i32,
    /// `+0x148` end-of-battle countdown (−1 idle), `+0x88c` `Simulate` running.
    pub countdown: i32,
    pub simulating: bool,
    /// `+0x8b1 + s` side present, `+0x8d8 + e` radar unit present.
    pub present: [u8; 19],
    pub radar: [u8; 19],
    /// `+0x8c8` last status, `+0x8cc` cache valid.
    pub last_status: i32,
    pub status_valid: bool,
    /// The landing zone (`LandingZones[p]`, 200 × 200 levels), generated at the first real-time landing
    /// search and freed by `GroundWarFinished`.
    pub landing: Option<Vec<i8>>,
    /// `+0x7fc + 8e / +0x800 + 8e`: per-empire call-in timers (capped 200 / 300 by `WF::Refresh`).
    pub timers: [(i32, i32); 18],
    /// `+0x190` the field was found finished at `StartGrWar`.
    pub finished_flag: bool,
    /// `+0x88d + e`: empire `e + 1` isn't looking at this planet; `+0x89f + s`: side `s + 1` retreats.
    pub not_watching: [u8; 18],
    pub retreat: [u8; 18],
    /// The projectile lists of the real-time battle (`+0xd4..+0xe8`).
    pub lists: Lists,
    /// The planet's passability maps by unit kind 1..=5 (`PlanetAtjarasMap` 0xA33388 + p·0x24, 200 × 200
    /// u16: 65000 free, 65001 impassable), for the path search.
    pub passability: Vec<Vec<u16>>,
    /// `+0x33c + 0x40·e`: the ground AI's state per empire `e` (index 0 unused; `ground-ai.md` §2.1).
    pub ai: [GroundAi; 19],
    /// `+0x8d0` the weather (0 none, 1 snow, 2 rain: `InitWeather`) and `+0x8d4` the background (`melyikhatter`, a
    /// `[hatter]` of `terepleiro.txt`), both drawn in the economy step's prologue ([`weather`]).
    pub weather: i32,
    pub background: i32,
    /// Saved bytes no ported system reads.
    pub tail: FieldTail,
}

impl Field {
    /// The cells of the units on the map: used, placed (flags without 2 / 0x40000000) and without flags2 bit 2.
    /// `LocateSuitableLocation` blocks each as a 1 × 1 footprint (`colony-placement.md` §1),
    /// `LocateTankSuitablePosition` as a 3 × 3 block (§11.1).
    pub fn placed_cells(&self) -> Vec<(i32, i32)> {
        (0..self.units.len())
            .filter(|&i| {
                let u = &self.units[i];
                self.alive[i] && u.flags & (flag::NOT_PLACED | flag::NOT_COUNTED) == 0 && u.flags2 & 4 == 0
            })
            .map(|i| self.units[i].cell)
            .collect()
    }
}

impl Default for Field {
    fn default() -> Self {
        Self {
            units: Vec::new(),
            alive: Vec::new(),
            attack_fleet: 0,
            self_destruct: -1,
            countdown: -1,
            simulating: false,
            present: [0; 19],
            radar: [0; 19],
            last_status: -1,
            status_valid: false,
            landing: None,
            timers: [(0, 0); 18],
            finished_flag: false,
            not_watching: [0; 18],
            retreat: [0; 18],
            lists: Lists::default(),
            passability: Vec::new(),
            ai: Default::default(),
            weather: 0,
            background: 0,
            tail: FieldTail::default(),
        }
    }
}

/// The battle planet's state the ground war touches.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GroundPlanet {
    pub index: i32,
    pub owner: u8,
    /// `+0x58 / +0x5c` garrison vehicles and their strength.
    pub garrison: i32,
    pub garrison_strength: i32,
    /// `+0x64` ground war in progress.
    pub in_war: u8,
    /// `Epuletek[p]` slots (index 0 unused).
    pub buildings: Vec<Building>,
    /// The number of buildings (exe 0xBA9518[p]).
    pub building_count: i32,
    /// The buildings' heights on the map (`Epuletek[p][slot].+0x14`), by slot: targets for the real-time
    /// battle.
    pub building_z: Vec<f32>,
}

/// Globals the ground war reads.
#[derive(Debug, Clone, Default)]
pub struct GroundContext {
    /// `EN`: the local human empire.
    pub human: i32,
    pub multiplayer: bool,
    /// `StoryAdat+0x40`: the campaign planet (story hold, §5.1).
    pub story_planet: i32,
    /// `StoryAdat+0x70 + StoryAdat+0x74`: the human's damage bonus in percent.
    pub story_bonus: i32,
    /// `ProgramPlayers2`.
    pub players: i32,
    /// `Bolygok[p − 1].owner`: `GroundWarFinished` rearranges this owner's groups (§6.1 step 11).
    pub prev_planet_owner: u8,
    /// `GrWar_MultiCsata_folyamatban` (exe 0xBAD13C): an MP ground war runs (`WF::Refresh` ends it when a side holds
    /// the field).
    pub mp_war: bool,
    /// The battlefield is on the colony screen (`AktKep == 3`, `AktPlanet == p`, not entering or leaving
    /// it): new units load their meshes, which has state effects ([`War::load_mesh`]).
    pub on_screen: bool,
    /// `PlayerKi[e]` (exe 0xBAE560): 1 = a human plays empire `e` (the ground AI skips it).
    pub player_ki: [u8; 19],
}

/// Side effects for other systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundEvent {
    /// `EpuletDelete(p, slot)`: the building of `kind` fell (colony map, 3D model: the colony side).
    BuildingDestroyed { slot: usize, kind: u8 },
    /// A mine exploded (`RemoveTankGroup` on a 0x20000 unit; spec §5.7, §11.10).
    MineExploded { unit: usize },
    /// `SelfDestruct(p)` ran (the colony screen drops its selected building, exe 0xB9C074).
    SelfDestructed,
    /// `BolygoElfoglal(p, owner)` with `BattlesWon[owner] += 1` (a won war).
    Captured { owner: u8 },
    /// `BolygoElfoglal(p, 0)`: no building left.
    Abandoned,
    /// MP: `WF::Refresh` found the field held by side `holder` and ran `GroundWarFinished(p, holder, false)`.
    Ended { holder: i32 },
    /// `WF::ShotSpecWeapon`: the ground AI called an air strike on a cell (before the spread; the bombs or
    /// the fighter wave are in [`Field::lists`], `ground-combat.md` §11.15).
    AirStrike { empire: i32, cell: (i32, i32), big_ships: bool },
}

/// What a war did (diagnostics for the trace report; no gameplay effect).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WarStats {
    /// Shots per `WF_Weapons` entry (air strikes count per hit).
    pub shots: [u32; 14],
    pub paralysed: u32,
    pub seized: u32,
    pub kamikazes: u32,
    pub splits: u32,
    pub joins: u32,
    pub partner_hits: u32,
    pub dodges: u32,
}

/// `LocateTankSuitablePosition(p, &x, &y, start)`: `None` start = the default; whether a cell was found, and it.
pub type PlaceFn<'a> = dyn FnMut(Option<(i32, i32)>) -> (bool, (i32, i32)) + 'a;

/// The ground war on one planet.
pub struct War<'a> {
    pub field: &'a mut Field,
    pub planet: &'a mut GroundPlanet,
    pub world: &'a mut CombatWorld,
    pub rules: &'a GroundRules,
    pub ctx: &'a GroundContext,
    pub rng: &'a mut Streams,
    /// libc `rand()` (0..=32767; ADR 0003): a voice pick per hit, one per new unit object, a mine's sound.
    pub libc: &'a mut dyn FnMut() -> i32,
    /// `LocateTankSuitablePosition(p, &x, &y, start)` (not ported): `None` start = the default; returns
    /// whether a cell was found and the cell.
    pub place: &'a mut PlaceFn<'a>,
    pub events: Vec<GroundEvent>,
    pub stats: WarStats,
    /// The global `timing` (exe 0x9B2EB8): passes since `Simulate` started.
    pub timing: i32,
    /// The global `RemoveBuilding` (exe 0x9B2913): set inside `RemoveBuildingTurrets`.
    remove_building: bool,
    /// The colony map for the real placement and heights ([`War::with_map`]); without it, [`War::place`].
    pub map: Option<&'a GroundMap>,
    /// The map of the planet on the colony screen (`AktPlanet`): unit orientation reads its heights.
    pub view_map: Option<&'a GroundMap>,
    /// The last cell a placement search wrote (a failed search leaves the caller's outputs as they were).
    place_out: (i32, i32),
    /// The colony screen's voice lines ([`War::with_voices`]); without them no line plays.
    pub voices: Option<&'a mut Voices>,
}

/// `cvttsd2si`: out of range or NaN → `i32::MIN` (the "integer indefinite" value).
fn cvtt(x: f64) -> i32 {
    if x.is_nan() || x >= 2147483648.0 || x <= -2147483649.0 {
        i32::MIN
    } else {
        x as i32
    }
}

fn design_of(designs: &[Vec<Design>], owner: i32, d: i32) -> Option<&Design> {
    designs.get(usize::try_from(owner).ok()?)?.get(usize::try_from(d).ok()?)
}

impl<'a> War<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        field: &'a mut Field,
        planet: &'a mut GroundPlanet,
        world: &'a mut CombatWorld,
        rules: &'a GroundRules,
        ctx: &'a GroundContext,
        rng: &'a mut Streams,
        libc: &'a mut dyn FnMut() -> i32,
        place: &'a mut PlaceFn<'a>,
    ) -> Self {
        Self {
            field,
            planet,
            world,
            rules,
            ctx,
            rng,
            libc,
            place,
            events: Vec::new(),
            stats: WarStats::default(),
            timing: 0,
            remove_building: false,
            map: None,
            view_map: None,
            place_out: (0, 0),
            voices: None,
        }
    }

    /// The voice lines of the colony screen (they only cost libc draws, while the field is shown).
    pub fn with_voices(mut self, voices: &'a mut Voices) -> Self {
        self.voices = Some(voices);
        self
    }

    /// `WF::PlaySpeechGrWar(line, kind)` (exe 0x48c670): on screen and outside `Simulate` only.
    fn speak(&mut self, line: i32, kind: i32) {
        if !self.ctx.on_screen || self.field.simulating {
            return;
        }
        let en = self.ctx.human;
        if self.voices.as_deref_mut().is_some_and(|v| v.play(line, kind, en)) {
            (self.libc)();
        }
    }

    /// Use the colony map for placement and heights (else the [`War::place`] hook).
    pub fn with_map(mut self, map: &'a GroundMap) -> Self {
        self.map = Some(map);
        self.view_map = Some(map);
        self
    }

    /// The colony screen shows another planet's map (`AktPlanet ≠ p`): unit heights come from it.
    pub fn with_view_map(mut self, map: Option<&'a GroundMap>) -> Self {
        self.view_map = map;
        self
    }

    fn n(&self) -> usize {
        self.field.units.len()
    }
    fn alive(&self, i: i32) -> bool {
        i >= 0 && (i as usize) < self.n() && self.field.alive[i as usize]
    }
    fn u(&self, i: i32) -> &Group {
        &self.field.units[i as usize]
    }
    fn um(&mut self, i: i32) -> &mut Group {
        &mut self.field.units[i as usize]
    }
    fn draw(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng.range(stream::GROUND_COMBAT, lo, hi)
    }
    fn building_flags(&self, slot: i32) -> u8 {
        self.planet.buildings.get(slot.max(0) as usize).map_or(0, |b| b.flags)
    }
    /// A turret only counts while its building works and isn't switched off.
    fn building_up(&self, u: &Group) -> bool {
        let f = self.building_flags(u.building);
        f & bflag::WORKING != 0 && f & bflag::SWITCHED_OFF == 0
    }

    // -------------------------------------------------------------------------------------------- §3

    /// `WF_Tank::SetPosG(x, y)` (exe 0x49c250; f32 with the f64 tail of the exe).
    fn set_pos(&mut self, i: i32, x: i32, y: i32) {
        let (w, h) = self.rules.map_size;
        let axis = |g: i32, size: f32| -> f64 {
            let t = ((g as f64 + 0.5) as f32) * 0.5f32 / 99.0f32 * 100.0f32 * size / 99.0f32;
            t as f64 - size as f64 * 0.5
        };
        let px = (-axis(x, w)) as f32;
        let py = axis(y, h) as f32;
        let u = self.um(i);
        u.pos = (px, py);
        u.cell = (x, y);
        u.last_cell = (x, y);
        u.flags2 &= !2;
        u.rt.moved = 1;
        u.rt.step = (px, py);
        let old = u.flags;
        u.flags = (u.flags & 0xfffff37f) | flag::PLACED;
        if old & (flag::TURRET | flag::MINE) == 0 {
            self.set_matrix(i);
        }
    }

    /// `WF::AddTank(d, hp, owner, origin, merge, x, y, bits)` (exe 0x45ddb0, spec §3.2): the unit index.
    #[allow(clippy::too_many_arguments)]
    pub fn add_tank(&mut self, d: i32, hp: i32, owner: i32, origin: i32, merge: bool, x: i32, y: i32, bits: u32) -> i32 {
        self.field.status_valid = false;
        let n = self.n();
        let mut i = 0;
        while i < n {
            let u = &self.field.units[i];
            if !self.field.alive[i] || (u.design == d && u.owner == owner && hp == u.max_hp && merge) {
                break;
            }
            i += 1;
        }
        if i == n {
            self.field.units.push(Group::default());
            self.field.alive.push(false);
        }
        if !self.field.alive[i] {
            // A new object: the constructor draws a libc `rand()` for a cosmetic float.
            let r = (self.libc)();
            self.field.units[i] = Group::default();
            self.field.units[i].rt.drop = TankRt::dropship_start(r);
            self.field.tail.created.push(i);
        }
        let i = i as i32;
        let u = self.um(i);
        if u.prev_size == -1 {
            u.prev_size = u.size();
        }
        u.design = d;
        let mut hp = hp;
        if hp == 0x8000 {
            hp = 1;
            u.flags |= flag::PARTNER;
        }
        u.origin.push(origin);
        u.hp.push(hp);
        u.max_hp = hp;
        u.rt.alive_cached = 0;
        self.field.alive[i as usize] = true;
        self.field.tail.grown.push(i as usize);
        if self.u(i).size() == 1 {
            let planet = self.planet.index;
            let u = self.um(i);
            u.owner = owner;
            u.side = owner;
            if u.rt.angle != 0.0 {
                u.flags |= 0x200;
                u.rt.angle = 0.0;
            }
            u.rt.turret_angle = 0.0;
            u.rt.speed = 3.0;
            u.rt.max_speed = 2.5;
            u.rt.accel = 0.15;
            u.rt.decel = 0.3;
            let cand = (((((u.flags >> 20) & 1) * 2) as f32) + 1.0) as i32 as f32;
            if u.rt.range < cand {
                u.rt.range = cand;
            }
            u.rt.turn = (0.65, 1.0);
            u.move_state = (0, -1);
            u.rt.planet = planet;
            u.rt.elevation = 0.0;
            u.rt.f228 = 0;
            u.index = i;
            u.rt.ai_group = 0;
            self.set_pos(i, x, y);
            let hull = design_of(&self.world.designs, owner, d).map_or(0, |x| x.hull) - 0x13;
            let u = self.um(i);
            u.hull = hull;
            u.flags2 |= 2;
            u.flags |= bits & (flag::NOT_PLACED | 4 | flag::LANDED);
            self.check_design(i);
            // Real-time speeds by ground hull (`TankAlapSpeed`, the turning table 0x8F8560).
            let hull = self.u(i).hull;
            if (0..5).contains(&hull) {
                let speed = self.rules.designs.tank_speeds.get(hull as usize).copied().unwrap_or(0.0);
                let turn = self.rules.turn_speeds.get(hull as usize).copied().unwrap_or(0.0);
                let u = self.um(i);
                u.rt.max_speed = 2.5;
                u.rt.accel = 0.15;
                u.rt.decel = 0.3;
                u.rt.speed = speed as i32 as f32;
                u.rt.turn = (turn, (turn as f64 * 1.5) as f32);
            }
            if self.ctx.on_screen && self.u(i).flags & flag::MESH == 0 {
                self.load_mesh(i);
            }
        }
        i
    }

    /// `WF::CheckDesign(u)` (exe 0x45fd20, spec §3.3): pods → abilities (the strategic part).
    fn check_design(&mut self, i: i32) {
        let rules = self.rules;
        let u = self.u(i);
        let (owner, d, hull) = (u.owner, u.design, u.hull);
        let des = design_of(&self.world.designs, owner, d).cloned();
        let u = self.um(i);
        u.has_gun = false;
        u.weapons = 0;
        u.regen = 0;
        u.flags &= !flag::POD_BITS;
        u.rt.range = 1.0;
        let Some(des) = des else { return };
        // The unit's owner (an empire) is passed as the pod-table index [original quirk].
        let slots = self.rules.designs.slot_count(owner as i16, des.hull.max(0) as usize);
        let mut count = [0i32; 14];
        let mut special = false;
        let (mut flags, mut weapons, mut regen) = (0u32, 0u32, None);
        for k in 0..slots.max(0) as usize {
            let pod = des.raw.get(0x51 + k).copied().unwrap_or(0);
            let (f, w, c) = match pod {
                0x96 => (0x40000, 0, None),
                0x97 => (0x80000, 0, None),
                0x98 => (0x100000, 0, None),
                0x99 => (0x200000, 0, None),
                0xa9 => (0, weapon::MANIPULATOR, Some(9)),
                0x9e => {
                    regen = Some(20);
                    (0, 0, None)
                }
                0x9f => (flag::TRANSPORTER, 0, None),
                0xa0 => (flag::RADAR, 0, None),
                0xa1 => (0, weapon::W100, Some(4)),
                0xa2 => (0, weapon::W200, Some(5)),
                0xa3 => (0, weapon::W2, Some(8)),
                0xa4 => (0, weapon::W400, Some(6)),
                0xa5 => (0, weapon::W800, Some(7)),
                0xa6 => (0, weapon::W1, Some(0)),
                0xa7 => (0, weapon::W2000, Some(1)),
                0xa8 => (0, weapon::PARALYSER, Some(3)),
                0x9a => (0, weapon::KAMIKAZE, Some(10)),
                0x9b => (0, weapon::BIG_KAMIKAZE, Some(11)),
                0x9d => (0, weapon::BIG_SHIP, Some(12)),
                0x9c => (0, weapon::FIGHTER, Some(13)),
                0xaa..=0xad => {
                    special = true;
                    (0, 0x4000 << (pod - 0xaa), None)
                }
                _ => (0, 0, None),
            };
            flags |= f;
            weapons |= w;
            if let Some(c) = c {
                count[c] += 1;
            }
        }
        let u = self.um(i);
        u.flags |= flags;
        u.weapons = weapons;
        if let Some(r) = regen {
            u.regen = r;
        }
        u.pods = count.iter().copied().fold(0, i32::max);
        if hull == 4 && special {
            u.weapons |= weapon::AMMO;
        }
        u.has_gun = u.weapons & weapon::GUNS != 0;
        if hull == 2 {
            u.flags |= flag::HULL2;
        }
        // Real time: the fire range is the longest weapon's, each weapon's reload its time divided
        // among its pods (`WF_NonSimulateWeapons` entry k, reload slot `+0x1f0 + 4k`).
        let hull2 = (((u.flags >> 20) & 1) * 2) as f32;
        for (bit, k) in [
            (weapon::W1, 0),
            (weapon::W2, 8),
            (weapon::W100, 4),
            (weapon::W200, 5),
            (weapon::W400, 6),
            (weapon::W800, 7),
            (weapon::W2000, 1),
            (weapon::PARALYSER, 3),
            (weapon::MANIPULATOR, 9),
        ] {
            if u.weapons & bit == 0 {
                continue;
            }
            let w = rules.rt_weapons.get(k).copied().unwrap_or([0; 4]);
            let cand = (hull2 + w[3] as f32) as i32 as f32;
            if u.rt.range < cand {
                u.rt.range = cand;
            }
            if count[k] != 0 {
                u.rt.reload[k] = (w[2] / count[k]) as f32;
            }
        }
        u.ammo = if u.weapons & weapon::AMMO != 0 { u.size() * 3 } else { 0 };
        u.air = if u.weapons & (weapon::FIGHTER | weapon::BIG_SHIP) != 0 { u.size() } else { 0 };
    }

    /// `LocateTankSuitablePosition` with a start cell, falling back to the default start.
    fn place_from(&mut self, start: (i32, i32)) -> (i32, i32) {
        let (ok, at) = self.locate_tank(Some(start));
        if ok {
            at
        } else {
            self.locate_tank(None).1
        }
    }

    /// `AddTanksToPlanet(f, p, sim)` (exe 0x45e320, spec §3.1): the fleet's ground vehicles land.
    pub fn land(&mut self, f: i32, sim: bool) {
        self.field.attack_fleet = f;
        let Some(owner) = self.world.fleets.get(f.max(0) as usize).map(|x| x.owner as i32) else { return };
        let mut t = 1;
        while t < self.world.vehicles.len() {
            let Vehicle { fleet, design, count } = self.world.vehicles[t];
            t += 1;
            if design == NO_DESIGN || fleet as i32 != f {
                continue;
            }
            let Some(des) = design_of(&self.world.designs, owner, design as i32) else { continue };
            if des.class != 2 {
                continue;
            }
            let hp = self.rules.hull_hp.get(des.hull.max(0) as usize).copied().unwrap_or(0);
            let human = self.world.human_controlled.get(owner as usize).copied().unwrap_or(false);
            for _ in 0..count {
                let (x, y) = if sim { (100, 100) } else { self.place_from((0x78, 0x78)) };
                let u = self.add_tank(design as i32, hp, owner, f, true, x, y, 0);
                if !self.ctx.multiplayer {
                    self.um(u).flags |= flag::LANDED | flag::NOT_PLACED;
                }
                if !human {
                    self.set_pos(u, x, y);
                    let un = self.um(u);
                    un.flags &= !flag::NOT_PLACED;
                    un.transport = (un.transport & !0x25) | 0x1a;
                }
                if self.u(u).flags & flag::TRANSPORTER != 0 {
                    let (x2, y2) = self.locate_tank(None).1;
                    let v = self.add_tank(design as i32, 0x8000, owner, -1, true, x2, y2, 0);
                    self.set_pos(v, x2, y2);
                    self.um(u).partner = v;
                    let vv = self.um(v);
                    vv.carrier = u;
                    vv.flags |= flag::LANDED | flag::NOT_PLACED;
                }
            }
            self.tank_delete(f, design as i32, count);
        }
        self.rearrange(owner);
    }

    // -------------------------------------------------------------------------------------------- §4

    /// `20 − (#units of o, not turrets/mines/partners)`, at least 0.
    fn free_groups(&self, o: i32) -> i32 {
        let used = (0..self.n())
            .filter(|&i| {
                let u = &self.field.units[i];
                self.field.alive[i] && u.flags & 0x4028000 == 0 && u.owner == o
            })
            .count() as i32;
        (MAX_GROUPS - used).max(0)
    }

    fn groupable(&self, i: usize, o: i32) -> bool {
        let u = &self.field.units[i];
        self.field.alive[i] && u.owner == o && u.flags & 0x4020000 == 0
    }

    /// `WF_Tank::FirstFreeC`: the first member slot with `hp < 1`, else a new one.
    fn first_free(&mut self, dst: i32) -> usize {
        let u = self.um(dst);
        if let Some(k) = u.hp.iter().position(|&h| h < 1) {
            return k;
        }
        u.hp.push(0);
        u.origin.push(0);
        let k = u.hp.len() - 1;
        self.field.tail.grown.push(dst as usize);
        k
    }

    /// `JoinGroups` / `AtlagoldC`: move `n` members `src → dst`, last member first.
    fn move_members(&mut self, src: i32, dst: i32, n: i32, join: bool) {
        for _ in 0..n.max(0) {
            let last = self.u(src).size() as usize - 1;
            let k = self.first_free(dst);
            let (h, o) = (self.u(src).hp[last], self.u(src).origin[last]);
            let d = self.um(dst);
            d.hp[k] = h;
            d.origin[k] = o;
            let s = self.um(src);
            s.hp.pop();
            s.origin.pop();
            if join && s.prev_size != -1 && s.size() <= s.prev_size {
                s.prev_size = -1;
            }
        }
    }

    /// `JoinTwoSmallestGroup(o)` (exe 0x483c60).
    fn join_two_smallest(&mut self, o: i32) -> bool {
        let n = self.n();
        let mut best = 1_000_000_000;
        let mut pick = None;
        for i in (0..n).rev() {
            if !self.groupable(i, o) {
                continue;
            }
            for j in (0..n).rev() {
                if !self.groupable(j, o) || i == j || self.field.units[i].design != self.field.units[j].design {
                    continue;
                }
                if self.field.units[i].size() < best {
                    best = self.field.units[i].size();
                    pick = Some((i as i32, j as i32));
                }
            }
        }
        let Some((a, b)) = pick else { return false };
        let n = self.u(a).size();
        self.stats.joins += 1;
        self.move_members(a, b, n, true);
        self.remove_group(a);
        true
    }

    /// `GetGreatestGroupC(o, d)`: first strictly largest, any flags.
    fn greatest_of(&self, o: i32, d: i32) -> i32 {
        let mut best = 0;
        let mut pick = -1;
        for i in 0..self.n() {
            let u = &self.field.units[i];
            if self.field.alive[i] && u.owner == o && u.design == d && best < u.size() {
                best = u.size();
                pick = i as i32;
            }
        }
        pick
    }

    /// `GetSmallestGroupC(o, d)`: first strictly smallest, not turrets/mines.
    fn smallest_of(&self, o: i32, d: i32) -> i32 {
        let mut best = 100_000_000;
        let mut pick = -1;
        for i in 0..self.n() {
            let u = &self.field.units[i];
            if self.groupable(i, o) && u.design == d && u.size() < best {
                best = u.size();
                pick = i as i32;
            }
        }
        pick
    }

    /// `EqualizeC(o)` (exe 0x474490).
    fn equalize(&mut self, o: i32) {
        let mut designs: Vec<i32> = Vec::new();
        for i in 0..self.n() {
            if self.groupable(i, o) {
                let d = self.field.units[i].design;
                if !designs.contains(&d) {
                    designs.push(d);
                }
            }
        }
        for d in designs {
            loop {
                let g = self.greatest_of(o, d);
                let s = self.smallest_of(o, d);
                if g == -1 || s == -1 || g == s {
                    break;
                }
                let (gs, ss) = (self.u(g).size(), self.u(s).size());
                let k = ((gs + ss) / 2 - ss).min(gs);
                if k < 1 {
                    break;
                }
                self.move_members(g, s, k, false);
            }
        }
    }

    /// `HalveGroupC(g)` (exe 0x47ccb0): split one vehicle off into a new group (at full hit points).
    fn split_one(&mut self, g: i32) {
        self.stats.splits += 1;
        let src = self.u(g).clone();
        let (x, y) = if src.owner as u32 == self.planet.owner as u32 {
            self.locate_tank(None).1
        } else {
            self.place_from((0x78, 0x78))
        };
        let last = *src.origin.last().unwrap_or(&0);
        let n = self.add_tank(src.design, src.max_hp, src.owner, last, false, x, y, src.flags);
        let src = self.u(g).clone();
        let u = self.um(n);
        let own = u.flags & (flag::MESH | flag::MESH2);
        u.flags = (src.flags & !(flag::MESH | flag::MESH2)) | own;
        u.flags2 = src.flags2;
        u.transport = src.transport;
        if u.flags & flag::KEEP_ORDERS == 0 {
            u.reset_orders();
        }
        let s = self.um(g);
        s.hp.pop();
        s.origin.pop();
    }

    /// `WF::RearrangeTankGroups(o)` (exe 0x48ca00, spec §4): at most 20 groups, sizes evened out.
    pub fn rearrange(&mut self, o: i32) {
        self.mute(true);
        while self.free_groups(o) == 0
            && self.join_two_smallest(o)
            && self.join_two_smallest(o)
            && self.join_two_smallest(o)
        {}
        self.equalize(o);
        loop {
            let greatest = (0..self.n()).filter(|&i| self.groupable(i, o)).map(|i| self.field.units[i].size()).max().unwrap_or(0);
            if !(self.free_groups(o) > 0 && greatest > 1) {
                break;
            }
            // `GetRealGreatestGroupC`: first strictly largest of o's units, any flags.
            let mut best = 0;
            let mut g = -1;
            for i in 0..self.n() {
                let u = &self.field.units[i];
                if self.field.alive[i] && u.owner == o && best < u.size() {
                    best = u.size();
                    g = i as i32;
                }
            }
            if g < 0 || self.u(g).size() == 1 {
                return; // skips the final pass and leaves the voices muted [original quirk]
            }
            self.split_one(g);
            self.equalize(o);
        }
        for i in 0..self.n() {
            if self.field.alive[i] {
                let u = &mut self.field.units[i];
                u.prev_size = -1;
                if u.flags2 & 2 != 0 {
                    // The speed reset (`+0x150 := 3.0` and the movement constants).
                    u.rt.speed = 3.0;
                    u.rt.max_speed = 2.5;
                    u.rt.accel = 0.15;
                    u.rt.decel = 0.3;
                    u.flags2 &= !2;
                }
            }
        }
        self.mute(false);
    }

    /// The global `0x9B2EBC` (death voices off while groups merge).
    fn mute(&mut self, on: bool) {
        if let Some(v) = self.voices.as_deref_mut() {
            v.muted = on;
        }
    }

    // -------------------------------------------------------------------------------------------- §5

    fn search_building(&self, kind: u8) -> bool {
        self.planet.buildings.iter().skip(1).any(|b| b.kind == kind && b.work_left == 0)
    }

    /// `WF::GetStatus()` (exe 0x47be90, spec §5.1).
    pub fn status(&mut self) -> i32 {
        let hold = self.ctx.story_planet == self.planet.index
            && (self.search_building(0xe) || self.search_building(0xf) || self.search_building(0x10));
        if self.field.countdown != -1 && !self.ctx.multiplayer {
            return CONTESTED;
        }
        let owner = self.planet.owner as i32;
        let answer = |s: i32| if s == owner || !hold || s == EMPTY { s } else { CONTESTED };
        if self.field.status_valid {
            return answer(self.field.last_status);
        }
        let human = self.ctx.human.clamp(0, 18) as usize;
        let had_human = self.field.present[human] != 0;
        for k in 1..19 {
            self.field.present[k] = 0;
            self.field.radar[k] = 0;
        }
        let mut s = EMPTY;
        for i in 0..self.n() {
            let u = &self.field.units[i];
            if !self.field.alive[i]
                || (u.flags & flag::NOT_PLACED != 0 && u.flags & flag::NOT_COUNTED != 0)
                || u.flags2 & 4 != 0
                || u.flags & flag::MINE != 0
            {
                continue;
            }
            s = if s == EMPTY { u.side } else if s == u.side { s } else { CONTESTED };
            if let Some(p) = self.field.present.get_mut(u.side as usize) {
                *p = 1;
            }
            if u.flags & flag::RADAR != 0 {
                if let Some(r) = self.field.radar.get_mut(u.owner as usize) {
                    *r = 1;
                }
            }
        }
        if !self.ctx.multiplayer
            && self.field.last_status == -1
            && s != CONTESTED
            && (owner == self.ctx.human || had_human)
            && !self.field.simulating
        {
            if hold {
                return CONTESTED;
            }
            self.field.last_status = s;
            self.field.status_valid = true;
            self.field.countdown = 0x50;
            return CONTESTED;
        }
        self.field.last_status = s;
        self.field.status_valid = true;
        answer(s)
    }

    /// `WF::GetEnemyNJ(e)` (exe 0x47aa40).
    fn enemy_nj(&self, e: i32) -> i32 {
        for i in 0..self.n() {
            let u = &self.field.units[i];
            if !self.field.alive[i] || u.flags & flag::PARALYSED != 0 || u.destroyed_turret() {
                continue;
            }
            if u.turret() && !self.building_up(u) {
                continue;
            }
            if u.owner != e && u.flags & flag::STEALTH == 0 {
                return i as i32;
            }
        }
        NONE
    }

    /// `WF::GetEnemy(e)` (exe 0x47a980, spec §5.2).
    pub fn enemy(&self, e: i32) -> i32 {
        let t = self.enemy_nj(e);
        if t != NONE {
            return t;
        }
        let pick = |strict: bool| {
            (0..self.n()).find(|&i| {
                let u = &self.field.units[i];
                self.field.alive[i]
                    && (!strict || u.flags & flag::PARALYSED == 0)
                    && !u.destroyed_turret()
                    && u.owner != e
            })
        };
        pick(true).or_else(|| pick(false)).map_or(NONE, |i| i as i32)
    }

    /// The first alive slot after `from` whose owner isn't `e` (the inlined second-enemy search).
    fn next_enemy_after(&self, from: i32, e: i32) -> i32 {
        let start = from.wrapping_add(1).max(0) as usize;
        (start..self.n())
            .find(|&i| self.field.alive[i] && self.field.units[i].owner != e)
            .map_or(NONE, |i| i as i32)
    }

    fn second_enemy(&self, e: i32) -> i32 {
        self.next_enemy_after(self.enemy(e), e)
    }

    /// `WF::GetThirdEnemy(e)` (exe 0x47c0d0).
    fn third_enemy(&self, e: i32) -> i32 {
        let s = self.second_enemy(e);
        self.next_enemy_after(s, e)
    }

    /// `WF_Tank::SetTarget(t)` (exe 0x49c670, spec §5.2).
    fn set_target(&mut self, i: i32, t: i32) {
        let u = self.u(i);
        if t == u.index || (u.turret() && u.building == -t) || u.flags & 0xa0000000 != 0 || t == NONE {
            return;
        }
        let me = self.um(i);
        if me.target != t {
            me.move_state = (0, -1);
        }
        me.target = t;
        me.flags |= flag::HAS_TARGET;
        let my_owner = me.owner;
        if t < 0 {
            // `+0x14c`: the range plus half the building's width (`Building` 0x8F1F28 `+0x58`).
            let kind = self.planet.buildings.get((-t) as usize).map_or(0, |b| b.kind);
            let w = self.map.map_or(0, |m| m.footprints.get(kind as usize).map_or(0, |f| f.0));
            let me = self.um(i);
            me.rt.bld_range = (w / 2) as f32 + me.rt.range;
        } else if (t as usize) < self.n() {
            let tt = self.um(t);
            if tt.owner != tt.side && tt.owner == my_owner {
                tt.paralysed = 0;
            }
        }
    }

    /// `EpuletDamage(p, slot, dmg, turret, by)` (exe 0x53c6e0): building damage (`+4`, never below 0); unless
    /// `turret`, `WF::DamageBuilding` shares it with the building's turrets (negative: a repair).
    pub fn building_damage(&mut self, slot: i32, dmg: i32, turret: bool, by: i32) {
        if let Some(b) = self.planet.buildings.get_mut(slot.max(0) as usize) {
            b.damage = b.damage.wrapping_add(dmg).max(0);
        }
        if !turret {
            self.turrets_share(slot, dmg, by);
        }
    }

    /// The placed turrets of building `slot` in slot order (`flags & 2` clear).
    fn building_turrets(&self, slot: i32) -> Vec<i32> {
        (0..self.n() as i32)
            .filter(|&i| self.alive(i) && self.u(i).flags & flag::NOT_PLACED == 0 && self.u(i).turret() && self.u(i).building == slot)
            .collect()
    }

    /// `WF::DamageBuilding(p, slot, dmg, by)` (exe 0x46d270, spec §5.5): a defence building (kinds
    /// 0x1c..=0x23) hit directly passes the damage on to its turrets, `dmg / (turrets left)` each in slot
    /// order (`WF_Tank::Damage`; a destroyed turret group is removed); with no turret left the building is
    /// deleted at once. Negative damage repairs them (hit marker set) or re-places the last unplaced one.
    /// Then the building's damage := its hit points − the turrets' first-member hit points.
    fn turrets_share(&mut self, slot: i32, dmg: i32, by: i32) {
        let Some(kind) = self.planet.buildings.get(slot.max(0) as usize).map(|b| b.kind) else { return };
        if !(0x1c..=0x23).contains(&kind) {
            return;
        }
        if dmg < 0 {
            let mut heal = -dmg;
            let worn: Vec<i32> = self.building_turrets(slot).into_iter().filter(|&i| self.u(i).hp.first() != Some(&self.u(i).max_hp)).collect();
            if worn.is_empty() {
                let unplaced = (0..self.n() as i32).rev().find(|&i| self.alive(i) && self.u(i).flags & flag::NOT_PLACED != 0 && self.u(i).building == slot);
                if let Some(i) = unplaced {
                    let u = self.um(i);
                    if let Some(h) = u.hp.first_mut() {
                        *h = heal.min(u.max_hp);
                    }
                    u.flags &= !flag::NOT_PLACED;
                }
            } else {
                let n = worn.len() as i32;
                for (j, &i) in worn.iter().enumerate() {
                    let share = heal / (n - j as i32);
                    let u = self.um(i);
                    if let Some(h) = u.hp.first_mut() {
                        *h = (*h + share).min(u.max_hp);
                    }
                    u.last_hit = Some((10000, by));
                    heal -= share;
                }
            }
        } else {
            let turrets = self.building_turrets(slot);
            if turrets.is_empty() {
                self.building_delete(slot);
                self.remove_target(-slot);
                return;
            }
            let (mut left, n) = (dmg, turrets.len() as i32);
            for (j, &i) in turrets.iter().enumerate() {
                let share = left / (n - j as i32);
                if self.unit_damage(i, share, by) {
                    self.kill(i);
                }
                left -= share;
            }
        }
        let sum: i32 = self.building_turrets(slot).iter().map(|&i| self.u(i).hp.first().copied().unwrap_or(0)).fold(0, i32::wrapping_add);
        // The kind is read again: the last turret's removal may have deleted the building (kind 0).
        let b = &mut self.planet.buildings[slot as usize];
        b.damage = self.rules.building_hp.get(b.kind as usize).copied().unwrap_or(0).wrapping_sub(sum);
    }

    fn building_fallen(&self, slot: i32) -> bool {
        let Some(b) = self.planet.buildings.get(slot.max(0) as usize) else { return true };
        let hp = self.rules.building_hp.get(b.kind as usize).copied().unwrap_or(0);
        b.damage >= hp
    }

    /// `WF::Damage(u, dmg, by)` (exe 0x469820, spec §5.5): whether `u` is destroyed.
    pub fn damage(&mut self, u: i32, dmg: i32, by: i32) -> bool {
        self.field.status_valid = false;
        if u == NONE || (u >= 0 && !self.alive(u)) {
            return false;
        }
        let mut dmg = dmg;
        if by != NONE && self.alive(by) && self.u(by).owner == self.ctx.human && !self.ctx.multiplayer {
            dmg = cvtt((self.ctx.story_bonus + 100).wrapping_mul(dmg) as f64 / 100.0);
        }
        if u >= 0 {
            if (self.libc)() % 15 == 0 {
                // A hit voice: the victim's if it is the human's, else the shooter's.
                if self.ctx.on_screen && self.u(u).owner == self.ctx.human {
                    self.speak(u % 3 + 1, 5);
                } else {
                    self.speak(by % 3 + 1, 8);
                }
            }
            if !self.u(u).turret() {
                return self.unit_damage(u, dmg, by);
            }
            let slot = self.u(u).building;
            self.building_damage(slot, dmg, true, by);
            if !self.building_fallen(slot) {
                return self.unit_damage(u, dmg, by);
            }
            if self.ctx.on_screen {
                // The turret's building falls: 16 slow pieces (exe 0x469820, the turret branch).
                let p = self.building_point_pub(slot);
                self.burst(p, 16, true);
            }
            self.building_delete(slot);
            self.remove_target(slot); // the slot number as a unit index [original bug]
            return true;
        }
        self.building_damage(-u, dmg, false, by);
        if self.building_fallen(-u) {
            if self.ctx.on_screen {
                // A building hit directly falls: 24 pieces.
                let p = self.building_point_pub(-u);
                self.burst(p, 24, false);
            }
            self.building_delete(-u);
            self.remove_target(u);
            return true;
        }
        false
    }

    /// `WF_Tank::MegHaltValamelyik(k)` (exe 0x48b2d0): member `k` dies; returns `origin[k]` as the exe
    /// reads it afterwards (the next member's, or the stale slot past the shrunk array).
    fn member_dies(&mut self, i: i32, k: usize) -> i32 {
        let u = self.um(i);
        if u.size() == 1 {
            return u.origin[k];
        }
        let n = u.hp.len();
        let stale = u.origin[n - 1];
        u.hp.remove(k);
        u.origin.remove(k);
        u.ammo = (u.ammo - 3).max(0);
        if u.air > 0 {
            u.air -= 1;
        }
        if k < u.origin.len() {
            u.origin[k]
        } else {
            stale
        }
    }

    /// `WF_Tank::Damage(dmg, by)` (exe 0x46bfa0, spec §5.6): whether the whole group is dead.
    fn unit_damage(&mut self, i: i32, dmg: i32, by: i32) -> bool {
        let mut dmg = dmg;
        loop {
            let size = self.u(i).size();
            self.draw(0, size - 1);
            if size < 1 || !self.u(i).hp.iter().any(|&h| h > 0) {
                return true;
            }
            let k = self.u(i).hp.iter().position(|&h| h > 0).unwrap();
            let u = self.um(i);
            u.hp[k] = u.hp[k].wrapping_sub(dmg);
            let carry = (-u.hp[k]).max(0);
            if dmg > 0 {
                u.last_hit = Some((NONE, by));
            }
            if u.hp[k] < 1 {
                let partner = u.partner;
                if partner != -1 {
                    self.stats.partner_hits += 1;
                }
                if partner != -1 && self.damage(partner, 1, NONE) {
                    self.remove_group(partner);
                    self.remove_target(partner);
                }
                let origin = self.member_dies(i, k);
                let u = self.u(i);
                if u.flags & (flag::TURRET | flag::PARTNER) == 0 && origin < 0 {
                    let d = u.design;
                    self.tank_delete(origin, d, 1);
                }
            }
            let u = self.u(i);
            let dead = u.size() < 1 || !u.hp.iter().any(|&h| h > 0);
            if dead {
                self.um(i).rt.alive_cached = 0;
                let u = self.u(i);
                if u.flags & flag::PARTNER != 0 {
                    let c = u.carrier;
                    if c >= 0 && (c as usize) < self.n() {
                        self.um(c).partner = -1;
                    }
                } else if self.ctx.on_screen {
                    // The wreck on screen: a flash, 16 pieces of debris, a sound (libc draws only).
                    let p = [u.rt.z, u.pos.0, u.pos.1];
                    self.burst(p, 16, true);
                    (self.libc)();
                }
                return true;
            }
            if carry == 0 {
                return false;
            }
            dmg = carry;
        }
    }

    /// `WF::RemoveTankGroup(u)` (exe 0x497c90, spec §5.7).
    pub fn remove_group(&mut self, i: i32) {
        if i < 0 || i == NONE {
            return;
        }
        self.field.status_valid = false;
        if !self.alive(i) {
            return;
        }
        if self.u(i).turret() && !self.remove_building {
            let slot = self.u(i).building;
            let u = self.um(i);
            u.flags |= flag::NOT_PLACED;
            if let Some(h) = u.hp.first_mut() {
                *h = 0;
            }
            let standing = (0..self.n()).any(|j| {
                let v = &self.field.units[j];
                self.field.alive[j] && v.turret() && v.building == slot && v.flags & flag::NOT_PLACED == 0
            });
            if standing {
                return;
            }
            if self.planet.buildings.get(slot.max(0) as usize).is_some_and(|b| b.kind != 0) {
                self.building_delete(slot);
                self.remove_target(-slot);
            }
            return;
        }
        // The death voice (`FUN_0045f380` = the cached alive-member count, refreshed here).
        if self.alive_members(i) == 0 && !self.voices.as_ref().is_some_and(|v| v.muted) {
            let kind = if self.u(i).owner == self.ctx.human { 7 } else { 9 };
            self.speak(i % 3 + 1, kind);
        }
        self.field.alive[i as usize] = false;
        if self.u(i).flags & flag::MINE != 0 {
            self.events.push(GroundEvent::MineExploded { unit: i as usize });
            self.mine_blast(i);
        }
    }

    /// A mine goes off (`RemoveTankGroup` on a 0x20000 unit, spec §5.7, §11.10): units forget it as their
    /// last mine cell, a burst on screen, the sound draw, and every counted unit within 100 map units
    /// takes `(int)((100 − d)·500)`.
    fn mine_blast(&mut self, i: i32) {
        let (cell, z, x, y) = {
            let u = self.u(i);
            (u.cell, u.rt.z, u.pos.0, u.pos.1)
        };
        for j in 0..self.n() {
            if self.field.alive[j] && self.field.units[j].rt.mine_cell == cell {
                self.field.units[j].rt.mine_cell = (0, 0);
            }
        }
        if self.ctx.on_screen {
            self.burst([z, x, y], 16, true);
        }
        (self.libc)(); // the blast sound
        for j in 0..self.n() as i32 {
            if !self.alive(j) || j == i {
                continue;
            }
            let v = self.u(j);
            if v.flags & (flag::NOT_PLACED | flag::NOT_COUNTED) != 0 || v.flags2 & 4 != 0 {
                continue;
            }
            let dy = v.pos.1 - y;
            let dx = v.pos.0 - x;
            let d2 = dx * dx + dy * dy;
            if d2 < 10000.0 {
                let d = (d2 as f64).sqrt() as f32;
                if self.damage(j, ((100.0 - d) * 500.0) as i32, NONE) {
                    self.remove_group(j);
                    self.remove_target(j);
                }
            }
        }
    }

    /// `WF::RemoveTarget(t)` (exe 0x499070): units aiming at `t` forget it.
    pub fn remove_target(&mut self, t: i32) {
        if t == NONE {
            return;
        }
        for i in 0..self.n() {
            if !self.field.alive[i] {
                continue;
            }
            let u = &mut self.field.units[i];
            let aim = if u.flags & flag::HAS_TARGET != 0 { u.target } else { NONE };
            if aim == t {
                u.flags &= !(flag::HAS_TARGET | flag::TARGET_AUX);
                u.rt.moved = 1;
            }
            if u.aim == t {
                u.aim = NONE;
                u.rt.moved = 1;
            }
        }
        // The projectiles lose the target and the shooter (a bomb's `+0x34` is its ground height).
        let l = &mut self.field.lists;
        for s in l.shells.iter_mut().flatten() {
            if s.target == t {
                s.target = NONE;
            }
            if s.by == t {
                s.by = NONE;
            }
        }
        for b in l.bolts.iter_mut().flatten() {
            if b.target == t {
                b.target = NONE;
            }
        }
        for m in l.missiles.iter_mut().flatten() {
            if m.target == t && m.kind != 1 {
                m.target = NONE;
            }
            if m.by == t && m.kind != 1 {
                m.by = NONE;
            }
        }
    }

    /// `EpuletDelete(p, slot)` from outside the ground war (a planet gun destroyed in a space battle).
    pub fn delete_building(&mut self, slot: i32) {
        self.building_delete(slot);
    }

    /// `EpuletDelete(p, slot)` (exe 0x53c730): turrets go, the slot empties.
    fn building_delete(&mut self, slot: i32) {
        let Some(kind) = self.planet.buildings.get(slot.max(0) as usize).map(|b| b.kind) else { return };
        if kind == b'+' {
            self.field.self_destruct = -1;
        }
        // `Remove3dBuilding` (exe 0x497760) on the colony screen: the ruin (`Rom::Rom` 0x455570) bursts
        // into 24 pieces of debris (libc draws and list-B slots only), while the building still has its 3D
        // model: not when an emptied slot is deleted again (a turreted building whose last turret fell
        // inside `WF::DamageBuilding`, spec §5.5).
        if self.ctx.on_screen && kind != 0 {
            let p = self.building_point_pub(slot);
            self.burst(p, 24, false);
        }
        // `RemoveBuildingTurrets(slot)`.
        self.remove_building = true;
        self.field.status_valid = false;
        for i in 0..self.n() as i32 {
            if self.alive(i) && self.u(i).turret() && self.u(i).building == slot {
                self.remove_group(i);
                self.remove_target(i);
            }
        }
        self.remove_building = false;
        self.planet.buildings[slot as usize].kind = 0;
        self.planet.building_count -= 1;
        self.remove_target(-slot);
        self.events.push(GroundEvent::BuildingDestroyed { slot: slot as usize, kind });
    }

    /// `AddOwnTankToPlanet(d, p)` (exe 0x45cbf0; `TankNew(−p, d, n)` calls it once per vehicle, then
    /// [`War::rearrange`] for the planet's owner): one garrison vehicle of the owner joins the battlefield at
    /// `LocateTankSuitablePosition(p, −1, −1)` (failing: the cell the last search left, uninitialised locals in the
    /// exe), merged into a group of its design (`AddTank(d, hp, owner, −p, merge, x, y, 0)`), the driving speeds
    /// reset.
    pub fn add_own_tank(&mut self, d: i32) {
        let owner = self.planet.owner as i32;
        let hull = design_of(&self.world.designs, owner, d).map_or(-1, |des| des.hull);
        let hp = self.rules.hull_hp.get(hull.max(0) as usize).copied().unwrap_or(0);
        let (_, (x, y)) = self.locate_tank(None);
        let u = self.add_tank(d, hp, owner, -self.planet.index, true, x, y, 0);
        let rt = &mut self.um(u).rt;
        rt.speed = 3.0;
        rt.max_speed = 2.5;
        rt.accel = 0.15;
        rt.decel = 0.3;
    }

    /// `WF::RemoveTank(d, owner)` (exe 0x497af0, via `RemoveTankFromPlanet(d, p, owner)` 0x497c70; `BolygoElfoglal`,
    /// `FlottaDelete` and `TankAtpakol` call it once per garrison vehicle leaving): the last alive group of design
    /// `d` and `owner` that isn't a partner loses its last member; a group of one goes (`RemoveTarget`, its object
    /// freed; the slot keeps the pointer).
    pub fn remove_tank(&mut self, d: i32, owner: i32) {
        let Some(i) = (0..self.n()).rev().find(|&i| {
            let u = &self.field.units[i];
            self.field.alive[i] && u.design == d && u.owner == owner && u.flags & flag::PARTNER == 0
        }) else {
            return;
        };
        if self.field.units[i].size() == 1 {
            self.remove_target(i as i32);
            self.field.alive[i] = false;
        } else {
            let u = &mut self.field.units[i];
            u.hp.pop();
            u.origin.pop();
            u.rt.alive_cached = 0;
        }
    }

    /// `BolygoElfoglal(p, owner)`'s battlefield part after the garrison left (exe 0x53b8a1..0x53b8ff): the planet's
    /// turrets (flag 0x4000000) change hands, and the status cache is invalid.
    pub fn captured(&mut self, owner: i32) {
        for i in 0..self.n() {
            let u = &mut self.field.units[i];
            if self.field.alive[i] && u.flags & flag::TURRET != 0 {
                u.owner = owner;
                u.side = owner;
            }
        }
        self.field.status_valid = false;
    }

    /// `TankDelete(f, d, n)` (exe 0x541510): fleets through [`CombatWorld`], a garrison (f ≤ 0) here.
    pub fn tank_delete(&mut self, f: i32, d: i32, n: i32) {
        if f >= 1 {
            self.world.delete_vehicles(f as i16, d as i16, n);
            return;
        }
        let Some(i) = (1..self.world.vehicles.len())
            .find(|&i| self.world.vehicles[i].fleet as i32 == f && self.world.vehicles[i].design as i32 == d)
        else {
            return;
        };
        let v = &mut self.world.vehicles[i];
        let n = n.min(v.count);
        v.count -= n;
        if v.count < 1 {
            v.design = NO_DESIGN;
        }
        if self.planet.owner == 0xff {
            return;
        }
        let Some(des) = design_of(&self.world.designs, self.planet.owner as i32, d) else { return };
        if des.class == 2 {
            self.planet.garrison -= n;
            let s = (des.hp.wrapping_mul(des.attack) as f64).sqrt() as f32;
            self.planet.garrison_strength = (s * -n as f32 + self.planet.garrison_strength as f32) as i32;
        }
    }

    /// `TankNew(f, d, n)` for a fleet (exe 0x541750 → 0x541800): the last matching entry, else the last
    /// free one, else a new one (≤ 1000).
    fn tank_new(&mut self, f: i32, d: i32, n: i32) {
        let vs = &mut self.world.vehicles;
        if vs.is_empty() {
            vs.push(Vehicle { fleet: 0, design: NO_DESIGN, count: 0 });
        }
        let live = (1..vs.len()).rev().find(|&i| vs[i].design != NO_DESIGN && vs[i].design as i32 == d && vs[i].fleet as i32 == f);
        let slot = match live.or_else(|| (1..vs.len()).rev().find(|&i| vs[i].design == NO_DESIGN)) {
            Some(i) => i,
            None => {
                if vs.len() > 1000 {
                    return;
                }
                vs.push(Vehicle { fleet: 0, design: NO_DESIGN, count: 0 });
                vs.len() - 1
            }
        };
        let v = &mut vs[slot];
        if v.design == NO_DESIGN {
            *v = Vehicle { fleet: f as i16, design: d as i16, count: n };
        } else {
            v.count = v.count.wrapping_add(n);
        }
        let Some(fl) = self.world.fleets.get(f as usize).copied() else { return };
        if !fl.alive() {
            return;
        }
        if let Some(des) = design_of(&self.world.designs, fl.owner as i32, d).cloned() {
            self.world.fleets[f as usize].totals.add_vehicle(&des, n);
        }
    }

    /// `CheckFighterAttack` / `CheckBigShipAttack(e)` (exe 0x460770 / 0x45fc90).
    fn air_ready(&self, e: i32, w: u32) -> bool {
        if e == self.planet.owner as i32 {
            return false;
        }
        (0..self.n()).any(|i| {
            let u = &self.field.units[i];
            self.field.alive[i] && u.flags & flag::NOT_PLACED == 0 && u.weapons & w != 0 && u.owner == e && u.air > 0
        })
    }

    /// Hit `t` with `rndlog(2, W)` × the target's bonus (air strikes, spec §5.3).
    fn air_hit(&mut self, t: i32, w: [i32; 4], k: usize) -> bool {
        self.stats.shots[k] += 1;
        let r = self.draw(w[0], w[1]);
        let dmg = cvtt(r as f64 * self.u(t).bonus());
        if self.damage(t, dmg, 0) {
            self.remove_group(t);
            self.remove_target(t);
            return true;
        }
        false
    }

    fn air_strike(&mut self, e: i32, w: [i32; 4], k: usize) -> bool {
        let mut any = false;
        let t = self.enemy(e);
        if t != NONE {
            any |= self.air_hit(t, w, k);
        }
        let t = self.second_enemy(e);
        if t != NONE {
            any |= self.air_hit(t, w, k);
        }
        let t = self.third_enemy(e);
        if t != NONE {
            any |= self.air_hit(t, w, k);
        }
        any
    }

    fn kill(&mut self, t: i32) {
        self.remove_group(t);
        self.remove_target(t);
    }

    /// One acting unit's weapons (spec §5.4 step 2, the table).
    fn fire(&mut self, i: i32, any: &mut bool) {
        let weapons = self.u(i).weapons;
        let t = self.u(i).target;
        let timing = self.timing;
        let due = |w: [i32; 4]| w[2] != 0 && timing.wrapping_rem(w[2]) == 0;
        for (bit, k) in [(weapon::W100, 4), (weapon::W200, 5), (weapon::W2, 8), (weapon::W800, 7), (weapon::W400, 6)] {
            let w = self.rules.w(k);
            if weapons & bit != 0 && due(w) {
                self.stats.shots[k] += 1;
                let r = self.draw(w[0], w[1]);
                let a = self.u(i);
                let dmg = cvtt(r as f64 * a.bonus() * a.size() as f64 * a.pods as f64);
                if self.damage(t, dmg, 0) {
                    self.kill(t);
                    *any = true;
                }
            }
        }
        let w = self.rules.w(2);
        if weapons & weapon::AMMO != 0 && due(w) {
            self.stats.shots[2] += 1;
            let r = self.draw(w[0], w[1]);
            let dmg = cvtt(r as f64 * self.u(i).bonus());
            if self.damage(t, dmg, 0) {
                self.kill(t);
                *any = true;
            }
        }
        for (bit, k) in [(weapon::PARALYSER, 3), (weapon::MANIPULATOR, 9)] {
            let w = self.rules.w(k);
            if weapons & bit != 0 && due(w) {
                self.stats.shots[k] += 1;
                let size = self.u(i).size();
                if self.damage(t, size.wrapping_mul(1000), NONE) {
                    self.kill(t);
                } else {
                    // A dead `t` (killed by an earlier gun) is read from its freed object.
                    let ts = self.u(t).size();
                    let roll = self.draw(0, 100);
                    if roll <= cvtt(size as f64 * 40.0 / ts as f64) {
                        // Writes to a dead `t` land in its freed object: no effect.
                        let live = self.alive(t);
                        if bit == weapon::PARALYSER {
                            self.stats.paralysed += live as u32;
                            if live {
                                let v = self.um(t);
                                v.flags |= flag::PARALYSED;
                                v.paralysed = 2;
                            }
                        } else {
                            self.stats.seized += live as u32;
                            if live {
                                let v = self.um(t);
                                v.flags |= flag::SEIZED;
                                v.seized = 2;
                            }
                            self.remove_target(t);
                            let owner = self.u(i).owner;
                            if live {
                                self.um(t).owner = owner;
                            }
                        }
                    }
                }
            }
        }
        let w = self.rules.w(0);
        if weapons & weapon::W1 != 0 && due(w) {
            self.stats.shots[0] += 1;
            let dodge = self.draw(0, 1);
            if self.u(t).flags & ((dodge as u32) << 18) != 0 {
                self.stats.dodges += 1;
            } else {
                let r = self.draw(w[0], w[1]);
                let a = self.u(i);
                let dmg = cvtt(r as f64 * a.bonus() * a.size() as f64);
                if self.damage(t, dmg, 0) {
                    self.kill(t);
                    *any = true;
                }
            }
        }
        let w = self.rules.w(1);
        if weapons & weapon::W2000 != 0 && due(w) {
            self.stats.shots[1] += 1;
            let r = self.draw(w[0], w[1]);
            let a = self.u(i);
            let dmg = cvtt(r as f64 * a.bonus() * a.size() as f64);
            if self.damage(t, dmg, 0) {
                self.kill(t);
                *any = true;
            }
        }
        if weapons & weapon::KAMIKAZE != 0 && self.draw(0, 10000) < 50 {
            let w = self.rules.w(10);
            self.blast(i, t, w, false);
            *any = true;
        }
        if self.alive(i) && weapons & weapon::BIG_KAMIKAZE != 0 && self.draw(0, 10000) < 50 {
            let w = self.rules.w(11);
            self.blast(i, t, w, true);
            *any = true;
        }
    }

    /// A kamikaze: the target, the second (and third) enemy, then itself (spec §5.4).
    fn blast(&mut self, i: i32, t: i32, w: [i32; 4], third: bool) {
        self.stats.kamikazes += 1;
        let d = self.draw(w[0], w[1]);
        if self.damage(t, d, 0) {
            self.kill(t);
        }
        let owner = self.u(i).owner;
        let s = self.second_enemy(owner);
        let d = self.draw(w[0] / 2, w[1] / 2);
        if self.damage(s, d, 0) {
            self.kill(t); // the primary target, not `s` [original quirk]
        }
        if third {
            let d = self.draw(w[0] / 3, w[1] / 3);
            let th = self.third_enemy(owner);
            if self.damage(th, d, 0) {
                self.kill(t);
            }
        }
        self.kill(i);
    }

    /// One unit's step in a pass (spec §5.4); `true` = an attrition kill ends the slot loop.
    fn step(&mut self, i: i32, any: &mut bool) -> bool {
        if self.alive(i) && !self.u(i).destroyed_turret() {
            if self.damage(i, 10, NONE) {
                self.kill(i);
                *any = true;
                return true;
            }
            *any = true;
        }
        if !self.alive(i) {
            return false;
        }
        let u = self.u(i);
        if u.destroyed_turret() || (u.turret() && !self.building_up(u)) {
            return false;
        }
        if u.flags & flag::PARALYSED != 0 {
            let u = self.um(i);
            u.paralysed -= 1;
            if u.paralysed == 0 {
                u.flags &= !flag::PARALYSED;
            }
            return false;
        }
        if u.flags & flag::SEIZED != 0 {
            let u = self.um(i);
            u.seized -= 1;
            if u.seized == 0 {
                u.flags &= !flag::SEIZED;
                self.remove_target(i);
                let u = self.um(i);
                u.owner = u.side;
            }
        }
        let u = self.u(i);
        let stale = u.flags & flag::HAS_TARGET == 0
            || u.target == NONE
            || self.field.units.get(u.target.max(0) as usize).is_some_and(|t| t.flags & flag::PARALYSED != 0);
        if stale {
            let t = self.enemy(u.owner);
            self.set_target(i, t);
        }
        let u = self.um(i);
        if u.flags & flag::SEIZED == 0 {
            let half = u.max_hp / 2;
            for h in u.hp.iter_mut() {
                if *h < half {
                    *h = h.wrapping_add(u.regen);
                    if *h > half {
                        *h = half;
                    }
                }
            }
        }
        let u = self.u(i);
        if u.flags & flag::HAS_TARGET != 0 && u.target != NONE && u.flags & flag::PARTNER == 0 {
            self.fire(i, any);
        }
        false
    }

    /// `WF::Simulate()` (exe 0x4a3730, spec §5): fight until one side holds the field, then
    /// `GroundWarFinished(p, status, true)`.
    pub fn simulate(&mut self) {
        self.timing = 0;
        self.field.simulating = true;
        for i in 0..self.n() {
            if !self.field.alive[i] {
                continue;
            }
            let u = &mut self.field.units[i];
            u.flags &= !(flag::HAS_TARGET | flag::TARGET_AUX);
            if u.flags & flag::KEEP_ORDERS == 0 {
                u.reset_orders();
            }
        }
        let mut s = self.status();
        loop {
            if s != CONTESTED {
                let st = self.status();
                self.finish(st, true);
                self.field.simulating = false;
                return;
            }
            loop {
                let mut any = false;
                for e in 1..=self.ctx.players {
                    let w = self.rules.w(13);
                    if self.air_ready(e, weapon::FIGHTER) && w[2] != 0 && self.timing.wrapping_rem(w[2]) == 0 {
                        any |= self.air_strike(e, w, 13);
                    }
                    let w = self.rules.w(12);
                    if self.air_ready(e, weapon::BIG_SHIP) && w[2] != 0 && self.timing.wrapping_rem(w[2]) == 0 {
                        any |= self.air_strike(e, w, 12);
                    }
                }
                for i in 0..self.n() as i32 {
                    if self.step(i, &mut any) {
                        break;
                    }
                }
                self.timing = self.timing.wrapping_add(1);
                if self.timing % 10 == 0 {
                    self.field.status_valid = false;
                    break;
                }
                if any {
                    break;
                }
            }
            s = self.status();
        }
    }

    // -------------------------------------------------------------------------------------------- §6

    /// `GroundWarFinished(p, s, sim)` (exe 0x47c5d0, spec §6.1).
    pub fn finish(&mut self, status: i32, sim: bool) {
        self.init_mines();
        let af = self.field.attack_fleet;
        if af > 0 {
            if let Some(f) = self.world.fleets.get_mut(af as usize) {
                f.flags &= !1;
            }
        }
        self.planet.in_war = 0;
        let owner = self.planet.owner as i32;
        let s = if status == EMPTY { owner } else { status };
        for i in 0..self.n() as i32 {
            if !self.alive(i) {
                continue;
            }
            let u = self.u(i);
            if u.side == owner || u.flags & (flag::PARTNER | flag::MINE) != 0 {
                continue;
            }
            let (side, design, size, partner) = (u.side, u.design, u.size(), u.partner);
            let fleets = &self.world.fleets;
            let owner_of = |g: i32| fleets.get(g as usize).map_or(0xff, |f| f.owner as i32);
            let mut g = (*u.origin.first().unwrap_or(&0)).max(0);
            if owner_of(g) != side {
                g = 0;
            }
            let room = |g: i32| fleets.get(g as usize).is_some_and(|f| f.totals.ground < f.totals.capacity);
            if !room(g) || g == 0 {
                g = 0;
                let mut at = fleets.first().map_or((0.0, 0.0), |f| (f.x, f.y));
                for (c, f) in fleets.iter().enumerate().skip(1) {
                    if f.owner as i32 == side && room(c as i32) && (at.0 - f.x).abs() + (at.1 - f.y).abs() <= 20.0 {
                        g = c as i32;
                        at = (f.x, f.y);
                    }
                }
            }
            if g != 0 {
                self.tank_new(g, design, size);
            }
            if partner != -1 {
                self.kill(partner);
            }
            self.kill(i);
        }
        for i in 0..self.n() as i32 {
            if self.alive(i) && self.u(i).flags & flag::PARTNER != 0 {
                self.remove_target(i);
                self.remove_group(i);
            }
        }
        if !sim && owner != s {
            self.events.push(GroundEvent::Captured { owner: s as u8 });
            self.planet.owner = s as u8;
        }
        if self.planet.building_count == 0 {
            self.events.push(GroundEvent::Abandoned);
        }
        // The landing zone and the real-time lists are freed, the retreat flags cleared.
        self.field.landing = None;
        self.field.retreat = [0; 18];
        // `RepairAllTanks`.
        for i in 0..self.n() {
            if self.field.alive[i] && !self.field.units[i].turret() {
                let u = &mut self.field.units[i];
                let max = u.max_hp;
                for h in u.hp.iter_mut().filter(|h| **h > 0) {
                    *h = max;
                }
            }
        }
        self.rearrange(self.ctx.prev_planet_owner as i32);
    }

    /// The simulated war (`GroundWarIndit` after routing chose it, spec §3, §5, §6.3): lands `f`'s
    /// ground vehicles, fights, re-embarks the survivors; the planet falls to `f`'s owner when `f`
    /// carries ground vehicles again. Returns whether it fell.
    pub fn auto_resolve(&mut self, f: i32) -> bool {
        self.land(f, true);
        self.simulate();
        let fl = self.world.fleets.get(f as usize).copied().unwrap_or_default();
        if fl.totals.ground > 0 {
            self.events.push(GroundEvent::Captured { owner: fl.owner });
            self.planet.owner = fl.owner;
            return true;
        }
        false
    }

    /// `GroundWarSetBattleStatus` for this planet (spec §6.4): sets `+0x64` and marks the attacking
    /// units' fleets in battle while the field is contested.
    pub fn battle_status(&mut self) {
        if self.planet.owner == 0 {
            self.planet.in_war = 0;
            return;
        }
        let contested = self.status() == CONTESTED;
        self.planet.in_war = contested as u8;
        if !contested {
            return;
        }
        let owner = self.planet.owner as i32;
        for i in 0..self.n() {
            let u = &self.field.units[i];
            if !self.field.alive[i]
                || u.flags & flag::NOT_PLACED != 0
                || u.flags & flag::NOT_COUNTED != 0
                || u.flags2 & 4 != 0
                || u.owner == owner
                || u.flags & 0x1028000 != 0
            {
                continue;
            }
            let g = *u.origin.first().unwrap_or(&0);
            if g > 0 {
                if let Some(f) = self.world.fleets.get_mut(g as usize) {
                    f.flags |= 1;
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------ §2

/// What `GroundWarIndit` sees before it starts (spec §2).
#[derive(Debug, Clone, Copy, Default)]
pub struct RouteInput {
    /// Fleet `+0x5c` ground vehicles, `+0x68` ground strength.
    pub fleet_ground: i32,
    pub fleet_strength: i32,
    /// Planet `+0x5c` garrison strength, `BolygoMennyiErodEro[p]` fortress strength.
    pub garrison_strength: i32,
    pub fortress: i32,
    pub attacker_human: bool,
    pub owner_human: bool,
    pub multiplayer: bool,
    pub mp_battle_status: i32,
    /// Debug switches 0xB7723C (never lopsided), 0xB77230 (lopsided off), 0xB77234 (always simulate).
    pub debug_no_lopsided: bool,
    pub debug_lopsided_off: bool,
    pub debug_always_simulate: bool,
    /// `Story_GrWarSimulateTiltas`.
    pub story_ban: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundRoute {
    /// No ground vehicles: nothing happens.
    None,
    RealTime,
    Simulate { lopsided: bool },
}

/// The 5:1 rule: `(f32)((f64 strength + 0.1) / ((f64)(garrison + fortress) + 0.1)) > 5.0f`.
pub fn lopsided(inp: &RouteInput) -> bool {
    if inp.debug_no_lopsided {
        return false;
    }
    let r = ((inp.fleet_strength as f64 + 0.1) / (inp.garrison_strength.wrapping_add(inp.fortress) as f64 + 0.1)) as f32;
    r > 5.0 && !inp.debug_lopsided_off
}

/// `GroundWarIndit`'s choice (spec §2 steps 1, 3, 5).
pub fn route(inp: &RouteInput) -> GroundRoute {
    if inp.fleet_ground < 1 {
        return GroundRoute::None;
    }
    let lop = lopsided(inp);
    let sim = if inp.multiplayer {
        !((inp.mp_battle_status == 0 || inp.mp_battle_status == 2) && inp.attacker_human && inp.owner_human)
    } else {
        ((inp.debug_always_simulate || lop) && !inp.story_ban) || !(inp.attacker_human || inp.owner_human)
    };
    if sim {
        GroundRoute::Simulate { lopsided: lop }
    } else {
        GroundRoute::RealTime
    }
}

/// A battlefield's weather (`FRISSIT_Planet(true, 0)`'s prologue, exe 0x4749b9..0x474a4f, `re/rules/colony-economy.md`
/// §3): surface types 2 and 5 draw a libc `rand()`, rain (2) when it is even or the planet holds info 0x776, else none;
/// type 7 draws one, snow (1) when it is even; every other type has none (0).
pub fn weather(surface: u8, info_776: bool, libc: &mut dyn FnMut() -> i32) -> i32 {
    match surface {
        2 | 5 => {
            if libc() % 2 == 0 || info_776 { 2 } else { 0 }
        }
        7 => (libc() % 2 == 0) as i32,
        _ => 0,
    }
}

/// `CheckWeather`'s picker (exe 0x47a380, ELF `CheckWeather` 0x4c0f50 is its test): the background of a
/// battlefield on a colony map whose surface allows `weathers`. `cursor` (`melyikhatter`'s cursor 0x996488, unsaved;
/// only this function moves it) steps `rand() % 20 + 1` times through `backgrounds` (the `[hatter]` sections' weather
/// lists, wrapping), each step going on to the next background sharing a weather with the surface; the last one is
/// the result. With no such background the exe never returns; the port stops after a full turn.
pub fn pick_background(cursor: &mut i32, weathers: &[i32], backgrounds: &[Vec<i32>], libc: &mut dyn FnMut() -> i32) -> i32 {
    let n = backgrounds.len() as i32;
    let fits = |c: i32| backgrounds.get(c as usize).is_some_and(|b| b.iter().any(|w| weathers.iter().any(|x| *x as u8 == *w as u8)));
    let next = |c: &mut i32| {
        *c += 1;
        if *c == n {
            *c = 0;
        }
    };
    let mut k = libc() % 20 + 1;
    while k > 0 {
        next(cursor);
        let mut tries = 0;
        while !fits(*cursor) && tries <= n {
            next(cursor);
            tries += 1;
        }
        k -= 1;
    }
    *cursor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spacewar::{CombatFleet, FleetTotals};

    fn design(hull: i32, class: i16, pods: &[u8], hp: i32, attack: i32) -> Design {
        let mut raw = [0u8; 0x84];
        raw[0x16..0x18].copy_from_slice(&class.to_le_bytes());
        raw[0x2c..0x30].copy_from_slice(&attack.to_le_bytes());
        raw[0x30..0x34].copy_from_slice(&hp.to_le_bytes());
        raw[0x44..0x48].copy_from_slice(&hull.to_le_bytes());
        for (k, &p) in pods.iter().enumerate() {
            raw[0x51 + k] = p;
        }
        Design::from_bytes(&raw)
    }

    fn rules() -> GroundRules {
        let mut weapons = vec![[10, 20, 3, 0]; 14];
        weapons[4] = [100, 100, 1, 0];
        GroundRules {
            weapons,
            hull_hp: (0..25).map(|h| if (0x13..=0x17).contains(&h) { 200 } else { 0 }).collect(),
            building_hp: vec![1000; 60],
            map_size: (400.0, 400.0),
            designs: DesignRules {
                race_pod_table: vec![1; 19],
                pod_slot_types: vec![vec![vec![1; 3]; 25]],
                ..Default::default()
            },
            ..Default::default()
        }
    }

    struct Fx {
        field: Field,
        planet: GroundPlanet,
        world: CombatWorld,
        rules: GroundRules,
        ctx: GroundContext,
        rng: Streams,
        draws: usize,
    }

    fn fx() -> Fx {
        let mut world = CombatWorld::default();
        let gun = design(0x13, 2, &[0xa1], 50, 20);
        let plain = design(0x14, 2, &[], 40, 10);
        world.designs = vec![vec![], vec![plain.clone(), gun.clone(), plain.clone()], vec![plain.clone(), gun, plain]];
        world.designs.resize(19, vec![]);
        world.fleets = vec![CombatFleet::default(); 3];
        world.fleets[1] = CombatFleet { owner: 1, x: 10.0, y: 10.0, totals: FleetTotals { ground: 6, capacity: 10, ..Default::default() }, ..Default::default() };
        world.fleets[2] = CombatFleet { owner: 1, x: 500.0, y: 0.0, ..Default::default() };
        world.vehicles = vec![
            Vehicle { fleet: 0, design: NO_DESIGN, count: 0 },
            Vehicle { fleet: 1, design: 1, count: 4 },
            Vehicle { fleet: 1, design: 2, count: 2 },
            Vehicle { fleet: -5, design: 2, count: 3 },
        ];
        let mut buildings = vec![Building::default(); 3];
        buildings[1].kind = 7;
        let planet = GroundPlanet { index: 5, owner: 2, garrison: 3, garrison_strength: 60, buildings, building_count: 1, ..Default::default() };
        let ctx = GroundContext { human: 6, players: 8, prev_planet_owner: 2, ..Default::default() };
        Fx { field: Field::default(), planet, world, rules: rules(), ctx, rng: Streams::default(), draws: 0 }
    }

    fn with_war<R>(x: &mut Fx, f: impl FnOnce(&mut War) -> R) -> R {
        let mut n = 0usize;
        let mut libc = || {
            n += 1;
            0
        };
        let mut place = |_s: Option<(i32, i32)>| (true, (50, 60));
        let r = {
            let mut w = War::new(&mut x.field, &mut x.planet, &mut x.world, &x.rules, &x.ctx, &mut x.rng, &mut libc, &mut place);
            f(&mut w)
        };
        x.draws += n;
        r
    }

    #[test]
    fn add_tank_merges_same_design_and_draws_once_per_object() {
        let mut x = fx();
        with_war(&mut x, |w| {
            let a = w.add_tank(1, 200, 1, 1, true, 100, 100, 0);
            let b = w.add_tank(1, 200, 1, 1, true, 100, 100, 0);
            let c = w.add_tank(1, 200, 1, 1, false, 100, 100, 0);
            assert_eq!((a, b, c), (0, 0, 1));
            assert_eq!(w.u(0).size(), 2);
            assert_eq!(w.u(0).weapons, weapon::W100);
            assert_eq!(w.u(0).pods, 1);
            assert!(w.u(0).has_gun);
            assert_eq!(w.u(0).flags & flag::PLACED, flag::PLACED);
        });
        assert_eq!(x.draws, 2);
    }

    #[test]
    fn partner_never_merges() {
        let mut x = fx();
        with_war(&mut x, |w| {
            let a = w.add_tank(1, 0x8000, 1, -1, true, 0, 0, 0);
            let b = w.add_tank(1, 0x8000, 1, -1, true, 0, 0, 0);
            assert_ne!(a, b);
            assert_eq!(w.u(a).max_hp, 1);
            assert!(w.u(a).flags & flag::PARTNER != 0);
        });
    }

    #[test]
    fn landing_moves_vehicles_and_rearranges() {
        let mut x = fx();
        with_war(&mut x, |w| w.land(1, true));
        // 6 vehicles left fleet 1 (4 + 2 of two designs) …
        assert_eq!(x.world.fleets[1].totals.ground, 0);
        assert!(x.world.vehicles[1].design == NO_DESIGN && x.world.vehicles[2].design == NO_DESIGN);
        // … and were split into single-vehicle groups (fewer than 20 groups: every group shrinks to 1).
        let sizes: Vec<i32> = (0..x.field.units.len()).filter(|&i| x.field.alive[i]).map(|i| x.field.units[i].size()).collect();
        assert_eq!(sizes.iter().sum::<i32>(), 6);
        assert!(sizes.iter().all(|&s| s == 1));
        assert_eq!(x.field.attack_fleet, 1);
    }

    #[test]
    fn join_when_over_twenty_groups() {
        let mut x = fx();
        with_war(&mut x, |w| {
            for _ in 0..21 {
                w.add_tank(1, 200, 1, 1, false, 0, 0, 0);
            }
            w.rearrange(1);
            let n = (0..w.n()).filter(|&i| w.field.alive[i]).count();
            assert_eq!(n, 20);
        });
    }

    #[test]
    fn attrition_and_guns_end_the_war() {
        let mut x = fx();
        with_war(&mut x, |w| {
            // Defender: a garrison unit of design 2 (plain) for owner 2; attacker: gun units of owner 1.
            w.add_tank(2, 200, 2, -5, true, 0, 0, 0);
            w.land(1, true);
            w.simulate();
        });
        // The attackers (side 1) can't own the planet in `finish`: they re-embark into fleet 1.
        let alive: Vec<_> = (0..x.field.units.len()).filter(|&i| x.field.alive[i]).collect();
        assert!(alive.iter().all(|&i| x.field.units[i].side == 2));
        assert!(x.world.fleets[1].totals.ground > 0);
        // The garrison vehicle died: its Tankok entry (-5, design 2) lost one, the planet's sums too.
        assert_eq!(x.world.vehicles[3].count, 2);
        assert_eq!(x.planet.garrison, 2);
    }

    #[test]
    fn carry_over_hits_the_next_member() {
        let mut x = fx();
        with_war(&mut x, |w| {
            let u = w.add_tank(2, 30, 2, -5, true, 0, 0, 0);
            w.add_tank(2, 30, 2, -5, true, 0, 0, 0);
            assert!(!w.damage(u, 45, NONE));
            assert_eq!(w.u(u).hp, vec![15]);
            assert!(w.damage(u, 15, NONE));
        });
    }

    fn with_voices<R>(x: &mut Fx, v: &mut Voices, f: impl FnOnce(&mut War) -> R) -> R {
        let mut n = 0usize;
        let mut libc = || {
            n += 1;
            0
        };
        let mut place = |_s: Option<(i32, i32)>| (true, (50, 60));
        let r = {
            let mut w = War::new(&mut x.field, &mut x.planet, &mut x.world, &x.rules, &x.ctx, &mut x.rng, &mut libc, &mut place).with_voices(v);
            f(&mut w)
        };
        x.draws += n;
        r
    }

    fn voices() -> Voices {
        let l = |speaker, kind| VoiceLine { speaker, kind, samples: 3, cooldown: 0 };
        // Race 6 has no speaker offset: lines 1..=3 are speakers 1..=3.
        Voices { silent: false, muted: false, lines: (1..=3).flat_map(|s| [l(s, 5), l(s, 7), l(s, 8), l(s, 9)]).collect() }
    }

    #[test]
    fn remove_target_wakes_the_aimers_and_clears_projectiles() {
        let mut x = fx();
        with_war(&mut x, |w| {
            let a = w.add_tank(1, 200, 1, 1, false, 0, 0, 0);
            let b = w.add_tank(2, 200, 2, -5, false, 0, 0, 0);
            w.um(a).aim = b;
            w.um(a).rt.moved = 0;
            let m = |kind, target| Missile { vel: [0.0; 3], pos: [0.0; 3], dest: [0.0; 3], life: 9, dmg: 1, target, kind, fade: 0.0, by: b };
            w.field.lists.missiles = vec![Some(m(4, b)), Some(m(1, b)), None];
            w.field.lists.shells = vec![Some(Shell { vel: [0.0; 3], pos: [0.0; 3], life: 3, target: b, pull: 1.0, dmg: 1, by: b, jitter: 0 })];
            w.remove_target(b);
            assert_eq!(w.u(a).aim, NONE);
            assert_eq!(w.u(a).rt.moved, 1);
            let l = &w.field.lists;
            assert_eq!(l.missiles[0].as_ref().map(|m| (m.target, m.by)), Some((NONE, NONE)));
            // A bomb's target field is its ground height: kept.
            assert_eq!(l.missiles[1].as_ref().map(|m| (m.target, m.by)), Some((b, b)));
            assert_eq!(l.shells[0].as_ref().map(|s| (s.target, s.by)), Some((NONE, NONE)));
        });
    }

    #[test]
    fn collapse_trims_trailing_slots_but_not_list_c() {
        let mut x = fx();
        with_war(&mut x, |w| {
            for _ in 0..4 {
                w.add_tank(1, 200, 1, 1, false, 0, 0, 0);
            }
            w.field.alive[1] = false;
            w.field.alive[3] = false;
            w.field.lists.missiles = vec![None, Some(Missile { vel: [0.0; 3], pos: [0.0; 3], dest: [0.0; 3], life: 1, dmg: 0, target: NONE, kind: 7, fade: 0.0, by: NONE }), None, None];
            w.field.lists.shells = vec![None, None];
            w.field.lists.bolts = vec![None];
            w.collapse();
            assert_eq!((w.n(), w.field.alive.len()), (3, 3));
            assert_eq!(w.field.lists.missiles.len(), 2);
            assert!(w.field.lists.shells.is_empty());
            assert_eq!(w.field.lists.bolts.len(), 1);
        });
    }

    #[test]
    fn grwar_counts_the_self_destruct_down_per_battle_step() {
        let mut x = fx();
        x.field.self_destruct = 3;
        let clock = Clock { speed: 2, paused: false };
        with_war(&mut x, |w| {
            let u = w.add_tank(2, 200, 2, -5, false, 0, 0, 0);
            w.grwar_planet(clock, 1);
            assert_eq!(w.field.self_destruct, 1);
            assert!(w.alive(u));
            w.grwar_planet(clock, 2);
            // 1 → 0 fires `SelfDestruct` (the timer goes idle), the second step leaves −1 alone.
            assert_eq!(w.field.self_destruct, -1);
            assert!(!w.alive(u));
            assert!(w.events.contains(&GroundEvent::SelfDestructed));
        });
        assert_eq!(x.planet.buildings[1].kind, 0);
        // Off screen: no burst, no voices; the new object's phase is the only draw.
        assert_eq!(x.draws, 1);
    }

    #[test]
    fn voices_cost_a_draw_on_screen_only_off_cooldown() {
        let mut x = fx();
        x.ctx.on_screen = true;
        let mut v = voices();
        with_voices(&mut x, &mut v, |w| {
            let a = w.add_tank(1, 200, 1, 1, false, 0, 0, 0);
            let b = w.add_tank(2, 200, 2, -5, false, 0, 0, 0);
            let before = w.u(b).hp.clone();
            // A hit: `rand() % 15 == 0` (the test's libc is always 0) → the shooter's line 1, kind 8.
            assert!(!w.damage(b, 1, a));
            assert_ne!(w.u(b).hp, before);
        });
        assert_eq!(v.lines.iter().find(|l| (l.speaker, l.kind) == (1, 8)).unwrap().cooldown, 10);
        // Two objects, the hit's pick, the voice: 4 draws; the same hit again within the cooldown: 1.
        assert_eq!(x.draws, 4);
        with_voices(&mut x, &mut v, |w| w.damage(1, 1, 0));
        assert_eq!(x.draws, 5);
    }

    #[test]
    fn death_voice_and_silent_merges() {
        let mut x = fx();
        x.ctx.on_screen = true;
        let mut v = voices();
        with_voices(&mut x, &mut v, |w| {
            let b = w.add_tank(2, 200, 2, -5, false, 0, 0, 0);
            w.um(b).hp = vec![0];
            w.remove_group(b);
        });
        // Unit 0 of an AI: line 1, kind 9.
        assert_eq!(v.lines.iter().find(|l| (l.speaker, l.kind) == (1, 9)).unwrap().cooldown, 10);
        // Groups merged by `RearrangeTankGroups` die silently (the mute), and the mute is lifted after.
        let mut x = fx();
        x.ctx.on_screen = true;
        let mut v = voices();
        with_voices(&mut x, &mut v, |w| {
            for _ in 0..21 {
                w.add_tank(1, 200, 1, 1, false, 0, 0, 0);
            }
            w.rearrange(1);
        });
        assert!(!v.muted);
        assert!(v.lines.iter().all(|l| l.cooldown == 0));
    }

    #[test]
    fn a_building_falling_on_screen_bursts_into_its_ruin() {
        let mut x = fx();
        x.ctx.on_screen = true;
        with_war(&mut x, |w| w.building_delete(1));
        // `Rom::Rom`: a flash and 24 pieces (3 draws each, 2 more per piece in `AddRepesz`).
        assert_eq!(x.draws, 1 + 24 * 5);
        assert_eq!(x.field.lists.missiles.iter().flatten().filter(|m| m.kind == 7).count(), 24);
    }

    #[test]
    fn path_search_goes_straight_or_round_a_wall() {
        let mut x = fx();
        let mut grid = vec![65000u16; 40000];
        x.field.passability = vec![grid.clone(); 6];
        with_war(&mut x, |w| {
            let u = w.add_tank(1, 200, 1, 1, false, 0, 0, 0);
            w.um(u).cell = (100, 100);
            let out = w.find_path((100, 100), (100, 120), 1, u, 5);
            assert_eq!(out, PathOut { next: (100, 120), arrived: false });
            // Already within the radius.
            assert!(w.find_path((100, 100), (100, 102), 5, u, 5).arrived);
        });
        // A wall across x = 90..=110 at y = 110: the next cell is in sight and not behind the wall.
        for gx in 90..=110 {
            grid[gx * 200 + 110] = 65001;
        }
        x.field.passability = vec![grid; 6];
        with_war(&mut x, |w| {
            let u = 0;
            w.um(u).move_state = (0, -1);
            let out = w.find_path((100, 100), (100, 120), 1, u, 5);
            assert_ne!(out.next, (100, 100));
            assert!(out.next.1 < 110 || out.next.0 < 90 || out.next.0 > 110, "{out:?}");
        });
    }

    #[test]
    fn routing() {
        let base = RouteInput { fleet_ground: 3, fleet_strength: 100, garrison_strength: 10, fortress: 5, ..Default::default() };
        assert_eq!(route(&RouteInput { fleet_ground: 0, ..base }), GroundRoute::None);
        assert_eq!(route(&base), GroundRoute::Simulate { lopsided: true });
        let h = RouteInput { attacker_human: true, fleet_strength: 50, ..base };
        assert_eq!(route(&h), GroundRoute::RealTime);
        assert_eq!(route(&RouteInput { fleet_strength: 100, ..h }), GroundRoute::Simulate { lopsided: true });
        assert_eq!(route(&RouteInput { fleet_strength: 100, story_ban: true, ..h }), GroundRoute::RealTime);
        let mp = RouteInput { multiplayer: true, attacker_human: true, owner_human: true, ..base };
        assert_eq!(route(&mp), GroundRoute::RealTime);
        assert_eq!(route(&RouteInput { mp_battle_status: 1, ..mp }), GroundRoute::Simulate { lopsided: true });
        // 5.0 exactly isn't lopsided: (50 + 0.1) / (10 + 0.1) ≈ 4.96.
        assert!(!lopsided(&RouteInput { fleet_strength: 50, garrison_strength: 10, fortress: 0, ..base }));
    }
}
