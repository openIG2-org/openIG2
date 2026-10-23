//! The real-time space battle (`SpwC`): one battle tick, `SpwC::SpaceWarFrissit` (exe 0x56dff0), as a
//! fixed-step deterministic simulation. Spec: `re/rules/space-combat.md` §7.
//!
//! The exe is normative: every float formula follows its asm (`mulss` = f32, `mulsd` = f64), the vector
//! helpers keep its summation order, and its quirks are kept (they are listed in the spec). Gameplay
//! randomness is stream 6 only. The original's libc `rand()` calls in a battle are all cosmetic (impact
//! sparks, explosions, lightning jitter, speech) and touch no battle state, so the port draws none; the
//! renderer gets [`BattleEvent`]s instead and uses its own generator (ADR 0003).
//!
//! What the battle does to the strategic map (ships and fighters lost, fleets deleted, planet ships and
//! buildings hit, fleets retreating) goes through [`BattleWorld`]. A battle is set up by [`new_battle`]
//! (`SpaceWar_NewBattle`: `InitSpaceWar`, `AddFleet`, formations, planet guns, fighters, asteroids; spec §7.10)
//! from the strategic state and ended by [`Battle::finish`] (`SpaceWarVege`, §7.11).

use crate::rng::{stream, Streams};

/// `rndlog(6, …)`.
pub const STREAM: usize = stream::SPACE_COMBAT;
/// Empires (`[19]` tables).
pub const EMPIRES: usize = 19;
/// Projectile forms (`fegyveralakok`).
pub const FORMS: usize = 40;
/// Path-search grid: 128 × 128 × 8 cells of u16 (`tomb`, exe 0xF69218), x-major.
const GRID_X: usize = 128;
const GRID_Y: usize = 128;
const GRID_Z: usize = 8;
const GRID_CELLS: usize = GRID_X * GRID_Y * GRID_Z;
/// Cell values of the search grid.
const CELL_FREE: u16 = 0xfde8;
const CELL_BLOCKED: u16 = 0xfde9;
/// Open list capacity (`nyitott`, 1-based binary heap).
const OPEN_MAX: usize = 1000;

// ------------------------------------------------------------------------------------------------
// Vector helpers (exe 0x583a00..0x583c50, 0x4501f0..0x450a60): f32 throughout, square roots through f64.

/// `PONTTYPE`: 16 bytes, the fourth float is carried along by struct copies.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct V4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl V4 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z, w: 0.0 }
    }
    fn sub(self, o: V4) -> V4 {
        V4 { x: self.x - o.x, y: self.y - o.y, z: self.z - o.z, w: self.w }
    }
    fn scale(self, k: f32) -> V4 {
        V4 { x: self.x * k, y: self.y * k, z: self.z * k, w: self.w }
    }
    /// `vektorhossznegyzet` (0x4509c0).
    fn len2(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }
    /// `vektorhossz` (0x450970).
    fn len(self) -> f32 {
        sqrt32(self.len2())
    }
}

#[inline]
fn sqrt32(v: f32) -> f32 {
    (v as f64).sqrt() as f32
}

/// `tavnegyzet(a, b)` (0x583c50): note the summation order (y, x, z).
fn dist2(a: V4, b: V4) -> f32 {
    (b.y - a.y) * (b.y - a.y) + (b.x - a.x) * (b.x - a.x) + (b.z - a.z) * (b.z - a.z)
}

/// `tav(a, b)` (0x583bf0).
fn dist(a: V4, b: V4) -> f32 {
    sqrt32(dist2(a, b))
}

/// `normalizevektor(v)` (0x4501f0): in place; a vector shorter than 1e-5 is left alone. Returns the length.
fn normalize(v: &mut V4) -> f32 {
    let l = sqrt32(v.x * v.x + v.y * v.y + v.z * v.z);
    if l < 1e-5 {
        return 0.0;
    }
    v.x /= l;
    v.y /= l;
    v.z /= l;
    l
}

/// `spwnormalizevektor(v, out)` (0x583a80): copies `v` unchanged when its squared length is ≤ 1e-5.
fn normalized(v: V4) -> V4 {
    let l2 = v.x * v.x + v.y * v.y + v.z * v.z;
    if 1e-5 < l2 {
        let l = sqrt32(l2);
        V4 { x: v.x / l, y: v.y / l, z: v.z / l, w: v.w }
    } else {
        v
    }
}

/// `vektormul(a, b, out)` (0x450a00): the cross product a × b.
fn cross(a: V4, b: V4) -> V4 {
    V4::new(b.z * a.y - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - b.x * a.y)
}

/// `vektorszog(a, b)` (0x450a60): the angle between two unit vectors, `2·asin(min(|a − b|, 2)/2)`.
fn angle(a: V4, b: V4) -> f32 {
    let d = sqrt32(dist2(a, b)).min(2.0);
    ((d * 0.5) as f64).asin() as f32 * 2.0
}

/// `creatematrixfromaxis(axis, angle, m)` (0x44f9f0): a rotation matrix from a unit axis (quaternion form).
fn axis_matrix(axis: V4, a: f32) -> [f32; 9] {
    let s = ((a * 0.5) as f64).sin() as f32;
    let qx = axis.x * s;
    let qy = axis.y * s;
    let qz = axis.z * s;
    let c = ((a * 0.5) as f64).cos() as f32;
    // Products as the asm groups them (the decompile shows `zz` as (z2·z)·s; it is z2·(z·s)).
    let z2 = qz * 2.0;
    let y2 = qy * 2.0;
    let xw = qx * 2.0 * c;
    let xx = qx * 2.0 * qx;
    let zz = z2 * qz;
    [
        1.0 - (zz + y2 * qy),
        y2 * qx - z2 * c,
        z2 * qx + y2 * c,
        y2 * qx + z2 * c,
        1.0 - (zz + xx),
        z2 * qy - xw,
        z2 * qx - y2 * c,
        z2 * qy + xw,
        1.0 - (y2 * qy + xx),
    ]
}

/// `rotatepointmatrix(p, out, m)` (0x4504b0).
fn rotate(p: V4, m: &[f32; 9]) -> V4 {
    V4 {
        x: m[1] * p.y + p.x * m[0] + m[2] * p.z,
        y: m[3] * p.x + m[4] * p.y + m[5] * p.z,
        z: m[6] * p.x + m[7] * p.y + m[8] * p.z,
        w: 0.0,
    }
}

// ------------------------------------------------------------------------------------------------
// Rules the battle reads

/// A space weapon (`fegyverek`, exe 0x91D858, 0x5c bytes; live values: `fegyverszamol` rewrites the damage
/// at `InitSpaceWar`). Spec §2.2.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WeaponRules {
    /// +0: 2 = an anti-planet weapon (fired by `Lov2` only).
    pub kind: i32,
    pub reload: i32,
    /// +8: damage per target type 0..=11.
    pub damage: [i32; 12],
    /// +0x38: hit chance in percent.
    pub hit: i32,
    /// +0x3c: aim spread (/1000 per axis).
    pub spread: f32,
    /// +0x40: homing turn rate (0 = flies straight).
    pub turn: f32,
    /// +0x44: projectile speed.
    pub speed: f32,
    /// +0x48: acceleration of a homing projectile.
    pub accel: f32,
    /// +0x4c: lifetime in ticks (×10 for shots at planet guns); also the disable/takeover duration.
    pub duration: i32,
    /// +0x50: range.
    pub range: i32,
    /// +0x54: projectile form (pool).
    pub form: i32,
    /// +0x58: aimed at the target (else fired along the shooter's heading).
    pub aimed: bool,
    /// +0x5a: a beam (sticks to its target).
    pub beam: bool,
}

/// A projectile form (`fegyveralakok`, 0x78 bytes): its pool size and the drawing data the battle reads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormRules {
    /// The form has a model (`+0` name non-empty): only then can it be fired.
    pub loaded: bool,
    /// +0x14: pool slots (the last one is never used).
    pub slots: usize,
    /// +0x10: sprite size.
    pub size: f32,
}

/// The ship-type record a unit points at (`+0x214`, 0x30 bytes).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShipType {
    /// +0x10: model scale.
    pub scale: f32,
    /// +0x14: top speed.
    pub top_speed: f32,
    /// +0x1c: acceleration.
    pub accel: f32,
    /// +0x24: turn rate.
    pub turn: f32,
    /// +0x28 / +0x2c: footprint in grid cells (horizontal, vertical).
    pub size_xy: i32,
    pub size_z: i32,
}

/// Rule values and global switches a battle tick reads.
#[derive(Debug, Clone, Default)]
pub struct BattleRules {
    pub weapons: Vec<WeaponRules>,
    pub forms: Vec<FormRules>,
    /// `MultiPlayerMode`.
    pub multiplayer: bool,
    /// `StoryAdat +0x78 + +0x7c`: the human's hit-chance bonus (SP), `+0x80 + +0x84`: damage bonus.
    pub story_hit_bonus: i32,
    pub story_damage_bonus: i32,
    /// `IdoPause`: a paused battle only counts ticks.
    pub paused: bool,
    /// Debug switch 0xB77244: no battle AI.
    pub no_ai: bool,
    /// Retreat bans for the human (story, 0xF2748C/0xF27490) and the scripted no-retreat flags
    /// (0xF6915D, 0xF66F2A).
    pub story_no_retreat_check: bool,
    pub story_no_retreat: bool,
    pub story_retreat_lock_a: bool,
    pub story_retreat_lock_b: bool,
    /// The screen fade (exe 0xE72E14): while it is strictly between 0 and 1 the end check waits.
    pub fade: f32,
}

/// Everything a battle reads from the rules (ADR 0004: the user's exe and install), for the game.
#[derive(Debug, Clone, Default)]
pub struct SpaceRules {
    /// The weapon table (`fegyverek`) and each record as 46 i32 words reaching into the next records (the
    /// out-of-table damage reads, [`WeaponRules::damage_vs`]).
    pub weapons: Vec<WeaponRules>,
    pub weapon_words: Vec<Vec<i32>>,
    pub forms: Vec<FormRules>,
    /// `felallas0` .. `felallas9` (`dataraw/spacewar/felallas<n>.bin`, [`FORMATION_FILES`]).
    pub formations: Vec<Vec<FormationSlot>>,
    /// `tipusnevek` as compiled (the setup rewrites the footprints of the live copy, [`crate::game`] keeps it).
    pub ship_types: Vec<ShipType>,
    /// `EpuletekLeiro[type].+0` (tech) and `+8` (hit points).
    pub building_tech: Vec<u8>,
    pub building_hp: Vec<i32>,
}

impl SpaceRules {
    /// The weapon rules and words from the raw `fegyverek` table (40 × 0x5c bytes).
    pub fn weapons_from_table(table: &[u8]) -> (Vec<WeaponRules>, Vec<Vec<i32>>) {
        let i32_at = |o: usize| table.get(o..o + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()));
        let f32_at = |o: usize| f32::from_bits(i32_at(o) as u32);
        let n = table.len() / 0x5c;
        let weapons = (0..n)
            .map(|w| {
                let o = w * 0x5c;
                WeaponRules {
                    kind: i32_at(o),
                    reload: i32_at(o + 4),
                    damage: std::array::from_fn(|k| i32_at(o + 8 + 4 * k)),
                    hit: i32_at(o + 0x38),
                    spread: f32_at(o + 0x3c),
                    turn: f32_at(o + 0x40),
                    speed: f32_at(o + 0x44),
                    accel: f32_at(o + 0x48),
                    duration: i32_at(o + 0x4c),
                    range: i32_at(o + 0x50),
                    form: i32_at(o + 0x54),
                    aimed: table[o + 0x58] != 0,
                    beam: table[o + 0x5a] != 0,
                }
            })
            .collect();
        let words = (0..n).map(|w| (0..46).map(|k| i32_at(w * 0x5c + 4 * k)).collect()).collect();
        (weapons, words)
    }

    /// The rules a battle tick reads, with the state's switches (`BattleRules`).
    pub fn tick_rules(&self) -> BattleRules {
        BattleRules { weapons: self.weapons.clone(), forms: self.forms.clone(), ..Default::default() }
    }

    pub fn setup_rules(&self) -> SetupRules {
        SetupRules {
            weapons: self.weapons.clone(),
            forms: self.forms.clone(),
            formations: self.formations.clone(),
            building_tech: self.building_tech.clone(),
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Battle state

/// A weapon mount of a unit (0x70 bytes, `fegyverhozzaad`).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mount {
    /// +0: weapon id.
    pub weapon: i32,
    /// +4: the weapon's kind.
    pub kind: i32,
    /// +8: barrels (weapons of this id on all ships of the unit).
    pub count: i32,
    /// +0xc: ticks until it can fire (negative = ready); +0x10 the value it was set to.
    pub timer: i32,
    pub timer_set: i32,
    /// +0x14: reload time.
    pub reload: i32,
    /// +0x28: the copied damage against target type 4 (what a lost planet ship is worth).
    pub damage4: i32,
    /// +0x60: range.
    pub range: i32,
}

/// What a unit was told to do (`+0x15c`).
pub mod order {
    pub const NONE: i32 = 0;
    pub const MOVE: i32 = 1;
    pub const ATTACK: i32 = 3;
}

/// What a unit's target is (`+0x158`).
pub mod target_kind {
    pub const NONE: i32 = 0;
    pub const UNIT: i32 = 1;
    pub const GUN: i32 = 2;
}

/// Special-pod flags (`+0x1a8`).
pub mod pod {
    /// Pod 0x25: damage ×1.3.
    pub const DAMAGE: u32 = 0x20;
    /// Jams enemies within 100: they can't fire weapons 9..=12.
    pub const JAMMER: u32 = 0x40;
    /// Missiles aimed at it go astray (1 % per tick).
    pub const DECOY: u32 = 0x80;
    /// Missiles aimed at it go astray (2 % per tick).
    pub const DECOY2: u32 = 0x100;
    /// Marks enemies within 100 (+0x1d6), and its own missiles ignore decoys.
    pub const SCANNER: u32 = 0x200;
    /// Cloak: hidden unless an enemy is within 20.
    pub const CLOAK: u32 = 0x400;
}

/// A battle unit (`urhajotipus`, exe 600 bytes): one ship, or several ships of one type, owner and fleet.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Unit {
    /// +0.
    pub alive: bool,
    /// +4: the ship-type row (design `+0x40`; 0 for scenery): the type record is `race·12 + kind`.
    pub race: i32,
    /// +8: ship type (< 8 capital ships; 8.. small craft, scenery 0x12).
    pub kind: i32,
    /// +0xc: the design (`0x8000 + t` for scenery); +0x10 the design's hull tech (`+0x48`, ships join a unit
    /// only with the same one).
    pub design: i32,
    pub model: i32,
    /// +0x14: owner (0 = scenery).
    pub owner: i32,
    /// +0x18 damage taken, +0x1c hit points, +0x20 shield used, +0x24 shield capacity.
    pub damage: i32,
    pub hp: i32,
    pub shield_used: i32,
    pub shield: i32,
    /// +0x28: the ships (`Hajok` indices; −k = defence ship k of the battle planet). The array keeps its
    /// stale tail: only the first `ship_count` (+0x1b0) are live.
    pub ships: Vec<i32>,
    pub ship_count: i32,
    /// +0x1b4: 1 when the unit is created and never raised as ships join it: `dechp` divides the hit points
    /// by it to get one ship's share, so a group's "ship" is worth all its hit points (original quirk).
    pub hp_share: i32,
    /// +0x2c: waypoints queued (`+0x74 + 16k`, the first is the destination).
    pub waypoint_count: i32,
    pub waypoints: [[i32; 4]; 4],
    /// +0x44: position; +0x54 its grid cell.
    pub pos: V4,
    pub cell: [i32; 4],
    /// +0x64: destination.
    pub dest: V4,
    /// +0xb4: the next path step and its cell (+0xc4).
    pub step: V4,
    pub step_cell: [i32; 4],
    /// +0xd4: velocity.
    pub vel: V4,
    /// +0x108: speed; +0x110 heading; +0x120 half-way heading (drawn).
    pub speed: f32,
    pub heading: V4,
    pub mid_heading: V4,
    /// +0x130 position at the last path search, +0x140 its tick.
    pub plan_pos: V4,
    pub plan_tick: i32,
    /// +0x144.
    pub f144: i32,
    /// +0x148: scenery spin.
    pub spin: [f32; 3],
    /// +0x154: target (a unit or a gun by `target_kind`).
    pub target: Option<usize>,
    pub target_kind: i32,
    pub order: i32,
    /// +0x160: last turn step.
    pub turn: f32,
    /// +0x168 / +0x16c: shortest and longest weapon range; +0x170 hit radius.
    pub range_min: f32,
    pub range_max: f32,
    pub radius: f32,
    /// +0x174.
    pub f174: f32,
    /// +0x17c: explosion animation tick.
    pub exploding: i32,
    /// +0x180: the path search stopped short.
    pub path_short: bool,
    /// +0x190: the nearest unit targeting this one, +0x194 its squared distance.
    pub attacker: Option<usize>,
    pub attacker_dist2: f32,
    /// +0x198: the unit it picked as a target this tick.
    pub picked: i32,
    /// +0x19c: id.
    pub id: i32,
    /// +0x1a4: experience.
    pub experience: i32,
    /// +0x1a8: special pods (and the shield flags 1..0x10, 0x800).
    pub pods: u32,
    /// +0x1ac: `rndlog(6, 0, 3)` at creation [cosmetic].
    pub look: i32,
    /// +0x1bc: enabled weapons (bit per weapon id).
    pub weapon_mask: u32,
    /// +0x1c0: disabled (weapon 9) ticks; +0x1c4 taken over (weapon 10) ticks; +0x1c8 original owner.
    pub disabled: i32,
    pub taken: i32,
    pub original_owner: i32,
    /// +0x1cc: ticks since it began to retreat; +0x1d0 it blows up when it speeds past 3.
    pub retreat_ticks: i32,
    pub retreat_explodes: bool,
    /// +0x1d4: retreating; +0x1d5 jammed; +0x1d6 scanned; +0x1d7 cloaked.
    pub retreating: bool,
    pub jammed: bool,
    pub scanned: bool,
    pub cloaked: bool,
    /// +0x1d8: cloak cooldown, also set by firing.
    pub cloak_cooldown: i32,
    /// +0x214.
    pub ship_type: ShipType,
    /// +0x218/+0x21c.
    pub mounts: Vec<Mount>,
    /// +0x220: ticks standing still (bobbing phase).
    pub idle: i32,
    /// +0x244/+0x248: struck by a chain-lightning shot (pool slot).
    pub lightning: Option<ShotRef>,
    /// +0x254: path searches that ended short in a row.
    pub path_fails: i32,
}

/// A fighter group (`vadasztipus`, 0xa8 bytes).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Fighter {
    pub alive: bool,
    /// +4 position, +0x14 destination, +0x24 velocity, +0x40 direction.
    pub pos: V4,
    pub dest: V4,
    pub vel: V4,
    pub dir: V4,
    /// +0x34/+0x38: target (a unit or a fighter).
    pub target: Option<usize>,
    pub target_kind: i32,
    /// +0x50 last turn, +0x54 speed, +0x58 distance to the goal, +0x5c the previous one.
    pub turn: f32,
    pub speed: f32,
    pub goal_dist: f32,
    pub goal_dist_prev: f32,
    /// +0x60: 0 idle, 1 attacking, 2 passing through, 3 retreating.
    pub state: i32,
    /// +0x64: a counter re-rolled every tick; +0x68 ticks until it can fire.
    pub jitter: i32,
    pub timer: i32,
    /// +0x6c design, +0x70 owner, +0x74 race (the design's `+0x40`), +0x78 weapon (+30; set to the design's
    /// `+0x44` − 1), +0x7c fleet, +0x80 count, +0x84 hit points each.
    pub design: i32,
    pub owner: i32,
    pub race: i32,
    pub weapon: i32,
    pub fleet: i32,
    pub count: i32,
    pub hp: i32,
    /// +0x88 retreating, +0x89 dies on the way out, +0x8c ticks since.
    pub retreating: bool,
    pub doomed: bool,
    pub retreat_ticks: i32,
    /// +0x90/+0x94: struck by a chain-lightning shot.
    pub lightning: Option<ShotRef>,
}

/// A planet gun (`bolygofegyvertipus`, 0xa8 bytes).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gun {
    pub pos: V4,
    /// +0x10: hits scored.
    pub hits: i32,
    /// +0x14: weapon id (−1 = destroyed).
    pub weapon: i32,
    /// +0x20 timer, +0x24 the value it was set to, +0x28 reload.
    pub timer: i32,
    pub timer_set: i32,
    pub reload: i32,
    /// +0x84: the building slot on the planet (0 = gone); +0x88 targetable.
    pub building: i32,
    pub active: bool,
    /// +0x98: forced target unit (−1 = any); +0x9c the last unit that targeted it.
    pub target: i32,
    pub attacker: Option<usize>,
    /// +0xa0: armour (the building damage divisor when > 1).
    pub armour: f32,
}

/// Which pool slot a projectile is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShotRef {
    pub form: usize,
    pub slot: usize,
}

/// Who fires at what (`+0x1c`).
pub mod shot_kind {
    /// Unit at unit.
    pub const SHIP: i32 = 0;
    /// Fighter at unit.
    pub const FIGHTER_SHIP: i32 = 1;
    /// Planet gun at unit.
    pub const GUN: i32 = 2;
    /// Unit at planet gun.
    pub const AT_GUN: i32 = 3;
    /// Unit at fighter.
    pub const AT_FIGHTER: i32 = 4;
    /// Fighter at fighter.
    pub const FIGHTER_FIGHTER: i32 = 5;
}

/// A projectile (`lovedektipus`, exe 0xa8 bytes). A slot is in use while it has a shooter.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shot {
    pub weapon: i32,
    pub owner: i32,
    /// +8: barrels behind it (damage multiplier).
    pub count: i32,
    /// +0xc: damage factor.
    pub mult: f32,
    pub age: i32,
    /// +0x14 shooter, +0x18 target (units, fighters or guns by `kind`).
    pub shooter: Option<usize>,
    pub target: usize,
    pub kind: i32,
    pub pos: V4,
    pub vel: V4,
    pub dir: V4,
    /// +0x5c: last homing correction; +0x60 speed.
    pub turn: f32,
    pub speed: f32,
    /// +0x64 will hit, +0x65 attached (beams), +0x66 astray.
    pub hits: bool,
    pub attached: bool,
    pub astray: bool,
    /// +0x68: the tick it attached.
    pub attached_tick: i32,
    /// +0x70: no more homing.
    pub straight: bool,
    /// The sprite the drawing pass left for this slot (`alaktipus +0x3c`, 0x4c each): position and size,
    /// read back only by weapon 11's area damage.
    pub sprite_pos: V4,
    pub sprite_size: f32,
}

/// A pool of projectile slots for one form.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pool {
    pub shots: Vec<Shot>,
}

/// A participating fleet (`+0xe98 + 0x30k`, `+0x1420` of them).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Participant {
    pub owner: i32,
    /// +4: 3 fighting, 1 destroyed, 2 retreated (or destroyed while retreating).
    pub status: i32,
    pub fleet: i32,
    /// +0xc: the fleet's story id (`+0x16`); +0x10/+0x14 its hit points (`+0x6c + +0x70`) at the start.
    pub story_id: i32,
    pub hp: [i32; 2],
}

/// A battle AI squad (0x1c bytes): units that gather, then attack.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Squad {
    /// +4: 0 gathering, 1 attacking.
    pub state: i32,
    /// +8: centre of its living units.
    pub centre: V4,
    pub units: Vec<usize>,
}

/// Per-empire battle record (0x6c bytes at `+0x6c·e`).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Side {
    /// +0x1c: where the side's fighters start; +0x2c the rotation its new units get (`+0x148`).
    pub centre: V4,
    pub rotation: [f32; 3],
    pub squads: Vec<Squad>,
    /// +0x40: fighters prefer ships over fighters this often (%).
    pub fighter_aggression: i32,
    /// +0x4c: squads formed (the AI's first turn forms them).
    pub ai_turns: i32,
    /// +0x50: the battle AI plays this empire.
    pub ai: bool,
    /// +0x54: strength at the start; +0x58 attack lost.
    pub strength: i32,
    pub lost: i32,
    /// +0x64: fighters the last `AddFleet` brought for it.
    pub fighter_count: i32,
    /// +0x68: may retreat.
    pub may_retreat: bool,
}

/// The empires present (`resztvevobirodalmakujraszamol`): `+0xe09` ids, `+0xe1c` what each still has
/// (bit 0 ships, 1 fighters, 2 guns).
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Present {
    pub empires: Vec<(u8, u8)>,
}

/// One battle (`SpwC`, exe 0x2D9C bytes per slot at 0xE417C0).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Battle {
    /// +0xc4c.
    pub tick: i32,
    pub units: Vec<Unit>,
    pub fighters: Vec<Fighter>,
    pub guns: Vec<Gun>,
    pub pools: Vec<Pool>,
    pub sides: Vec<Side>,
    /// +0xca0: `[a][b] == 1`: a and b fight each other.
    pub hostile: [[u8; EMPIRES]; EMPIRES],
    /// +0xc3c: the human's empire.
    pub human: i32,
    /// +0xc8c planet owner (−1 none), +0x1f28 the planet.
    pub planet_owner: i32,
    pub planet: i32,
    /// +0x804 cell size, +0x810 grid size, +0x81c bounds (x, y, z pairs), +0x834 arena width.
    pub cell: [f32; 3],
    pub grid: [i32; 3],
    pub bounds: [f32; 6],
    pub width: f32,
    /// +0x1f64: where retreating units head (away from).
    pub retreat_point: V4,
    /// +0x1f88: drawing scale (weapon 11 compares unscaled positions with scaled sprites).
    pub scale: f32,
    /// +0x1f2c: shown on screen (the end-of-battle flight only happens then).
    pub shown: bool,
    /// +0xc40: AI timer; +0xc44 auto-target timer; +0xc50/+0xc58 end check countdown and period.
    pub ai_timer: f32,
    pub aim_timer: i32,
    pub end_timer: i32,
    pub end_period: i32,
    /// +0xc54: nobody takes damage (debug).
    pub invulnerable: bool,
    /// +0x1f84: special-pod pass countdown.
    pub pod_timer: i32,
    /// +0xc48: winner (0 none yet).
    pub winner: i32,
    /// +0x1fb8: the battle has been closed (no more end checks); +0x1fba: an end check found it over.
    pub ended: bool,
    pub finished: bool,
    /// +0x1f30: the victory flight has started.
    pub victory: bool,
    /// +0x2004: ground units wait for a planet battle won.
    pub ground_war: bool,
    /// +0x84c: the planet's position (victory flight target, gun hit marker).
    pub planet_pos: [i32; 3],
    pub present: Present,
    pub participants: Vec<Participant>,
    /// +0x1f2e: the slot holds a battle.
    pub active: bool,
    /// +0x858: the planet's size (gun placement).
    pub planet_size: i32,
    /// +0x1f78/+0x1f7c: where on the starmap (the first fleet's position).
    pub map_pos: [f32; 2],
    /// +0xe34: `StoryUrharcVisszavonulhate` per empire, copied at setup.
    pub story_retreat: [i32; EMPIRES],
    /// +0x2d70..+0x2d7c: the asteroids this battle may get (drawn by `InitSpaceWar`, placed by `AddFleet`).
    pub asteroids: Option<[i32; 3]>,
    /// +0x2d85: the human may pick a formation (≥ 6 units, SP) [UI].
    pub formation_choice: bool,
    /// The human's side's list of ships (side `+0x5c`/`+0x60`, 0x1c-byte entries) as the last `AddFleet` left
    /// it: what a formation pick places anew ([`Battle::pick_formation`]).
    pub human_list: Vec<Entry>,
    /// +0x2005: the ground war after the battle has been started.
    pub ground_war_started: bool,
    /// +0x2d84/+0x2d86: an AI tick was due while paused (UI flags kept for fidelity).
    pub f2d84: bool,
    pub f2d86: bool,
    /// +0x2d90 / +0x2d91: the human / an AI started retreating.
    pub human_retreated: bool,
    pub ai_retreated: bool,
    /// +0x2b40: units, fighters and guns per empire at the last end check.
    pub counts: [i32; EMPIRES],
    /// +0x1424: the empire the AI is playing this pass.
    ai_empire: usize,
    /// Path-search scratch (`tomb`, `nyitott`, the result globals).
    search: Search,
}

/// What a battle does to the strategic map (spec §7.8). The trace replay implements it on raw memory; the
/// game on [`crate::spacewar::CombatWorld`].
pub trait BattleWorld {
    /// `Hajok[ship].fleet`.
    fn ship_fleet(&self, ship: i32) -> i32;
    /// `Tervek[owner][Hajok[ship].design].attack` (+0x2c).
    fn ship_attack(&self, owner: i32, ship: i32) -> i32;
    /// `HajoDelete(ship)` (exe 0x53e5d0).
    fn delete_ship(&mut self, ship: i32);
    /// The fleet has no ships and no fighters left (`+0x54 == 0 && +0x58 == 0`).
    fn fleet_empty(&self, fleet: i32) -> bool;
    /// `FlottaDelete(fleet)` (exe 0x53d040).
    fn delete_fleet(&mut self, fleet: i32);
    /// `Flottak[fleet].owner` (+0x15).
    fn fleet_owner(&self, fleet: i32) -> i32;
    /// `TankDelete(fleet, design, n)` (exe 0x5414c0).
    fn delete_fighters(&mut self, fleet: i32, design: i32, n: i32);
    /// Defence ship `k` (1-based) of the battle planet is lost: the list closes up (`+0xfe + 2k`, count +0x114).
    fn remove_planet_ship(&mut self, planet: i32, k: i32);
    /// Building `slot` of the battle planet takes `damage` (exe 0x474160); true when it is destroyed.
    fn damage_building(&mut self, planet: i32, slot: i32, damage: i32) -> bool;
    /// The fleet backs off on the starmap (exe 0x597cb0).
    fn retreat_fleet(&mut self, fleet: i32);
}

/// Things the renderer and sound want to know about; none of them feeds back into the battle. The
/// original draws libc `rand()` for most of these effects (sparks, debris, lightning jitter).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BattleEvent {
    /// A projectile left `shooter` (kind per [`shot_kind`]).
    Fired { shot: ShotRef, kind: i32 },
    /// A projectile of `weapon` struck unit `unit` for `damage` (a beam strikes every tick it holds).
    Hit { unit: usize, damage: i32, beam: bool, weapon: i32 },
    /// One ship of the unit is gone (`felrobban`).
    ShipLost { unit: usize },
    /// Fighters lost.
    FightersLost { fighter: usize, count: i32 },
    /// A planet gun's building was destroyed.
    GunDestroyed { gun: usize },
    /// A shot struck an armoured planet gun (`karikakirak` in `FoldTalalat`): the shield ring's place.
    ShieldHit { gun: usize, at: V4 },
    /// An empire started to retreat.
    Retreat { empire: i32 },
    /// A capital ship's voice line (`dechp`): 6 dying (its hull failing), 7 the human's dead, 9 another's.
    Voice { unit: usize, kind: i32 },
}

/// The search grid and open list of `Utkeres` (exe 0x6133a0) and its result globals.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Search {
    /// 128 × 128 × 8 u16, plus the 4 bytes after it (open-list slot 0, always 0) that a z overflow reads.
    /// Scratch: every search refills it (not in a snapshot).
    #[cfg_attr(feature = "serde", serde(skip))]
    grid: Vec<u16>,
    /// 1-based binary heap of (x, y, z, f); read only up to `open_len`.
    #[cfg_attr(feature = "serde", serde(skip))]
    open: Vec<[u16; 4]>,
    open_len: i32,
    goal: [i32; 3],
    result: [i32; 3],
    result_w: i32,
    max_cost: i32,
    max_heu: i32,
    done: bool,
    widened: bool,
    /// `UtkeresUtolsoElotti` / `UtkeresUtolso`: one step short / no step to take.
    one_short: bool,
    arrived: bool,
}

/// `cvttss2si`: out-of-range and NaN give `i32::MIN`.
#[inline]
fn trunc(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

#[inline]
fn trunc64(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `rndlog(6, lo, hi)`.
#[inline]
fn roll(rng: &mut Streams, lo: i32, hi: i32) -> i32 {
    rng.range(STREAM, lo, hi)
}

/// A reload roll: `rndlog(6, 2r/3, r − 1)`.
#[inline]
fn reload_roll(rng: &mut Streams, reload: i32) -> i32 {
    roll(rng, reload.wrapping_mul(2) / 3, reload - 1)
}

impl WeaponRules {
    /// Damage against ship type `kind`: the exe indexes the 12-entry table with any type, so scenery
    /// (0x12) reads the words after it (`raw`, the record as i32 words).
    pub fn damage_vs(&self, raw: &[i32], kind: i32) -> i32 {
        if (0..12).contains(&kind) {
            self.damage[kind as usize]
        } else {
            raw.get((2 + kind) as usize).copied().unwrap_or(0)
        }
    }
}

/// The context of a tick: rules, the stream, the strategic map and the event sink.
pub struct Tick<'a> {
    pub rules: &'a BattleRules,
    /// Each weapon record as 23 i32 words (for out-of-table damage reads).
    pub weapon_words: &'a [Vec<i32>],
    pub rng: &'a mut Streams,
    pub world: &'a mut dyn BattleWorld,
    pub events: &'a mut Vec<BattleEvent>,
}

impl Battle {
    fn hostile(&self, a: i32, b: i32) -> u8 {
        let i = a * EMPIRES as i32 + b;
        if (0..(EMPIRES * EMPIRES) as i32).contains(&i) {
            self.hostile[(i / EMPIRES as i32) as usize][(i % EMPIRES as i32) as usize]
        } else {
            0
        }
    }

    /// One battle tick (`SpwC::SpaceWarFrissit`, exe 0x56dff0) followed by the end check (`vegevizsgal`
    /// 0x5833d0, skipped once the battle is closed); returns true when the check finds the battle over.
    pub fn step(&mut self, t: &mut Tick) -> bool {
        self.tick += 1;
        self.count_reloads();
        if !t.rules.paused {
            if self.f2d84 {
                self.f2d86 = true;
            }
            self.f2d84 = false;
            self.ai_timer -= 1.0;
            if self.ai_timer < 0.0 {
                self.ai_timer = 10.0;
                for e in 0..EMPIRES {
                    if !t.rules.no_ai && self.sides[e].ai {
                        self.ai(e, t);
                    }
                }
            }
            self.mark_attackers();
            self.planet_guns(t);
            self.move_shots(t);
            self.special_pods();
            self.move_fighters(t);
            self.aim_and_fire(t);
            self.move_units(t);
            self.unit_deaths(t);
            self.fighter_deaths(t);
        }
        // `frissit2` (0x573b40) runs the special-pod pass once more, paused or not.
        self.special_pods();
        if self.ended || !self.check_end(t.rules) {
            return false;
        }
        self.finished = true;
        true
    }

    /// Reload timers count down every tick, paused or not (exe 0x576220); a unit's hit radius is filled
    /// in from its type the first time.
    fn count_reloads(&mut self) {
        for u in &mut self.units {
            if u.disabled == 0 {
                for m in &mut u.mounts {
                    m.timer -= 1;
                }
            }
            if u.radius == 0.0 && u.alive {
                u.radius = u.ship_type.scale * 1000.0;
            }
        }
    }

    /// `mitamadjabeallit` (0x57c3a0): every unit learns the nearest unit that picked it as a target last
    /// tick; a gun learns who targets it.
    fn mark_attackers(&mut self) {
        for u in &mut self.units {
            u.attacker = None;
            u.attacker_dist2 = 0.0;
        }
        for i in 0..self.units.len() {
            let p = self.units[i].picked;
            if p != -1 {
                let j = p as usize;
                if self.units[j].attacker_dist2 == 0.0 {
                    self.units[j].attacker = Some(i);
                } else {
                    let d = dist2(self.units[i].pos, self.units[j].pos);
                    if d < self.units[j].attacker_dist2 {
                        self.units[j].attacker = Some(i);
                        self.units[j].attacker_dist2 = d;
                    }
                }
            }
            if self.units[i].target_kind == target_kind::GUN {
                if let Some(g) = self.units[i].target {
                    self.guns[g].attacker = Some(i);
                }
            }
        }
    }

    // --------------------------------------------------------------------------------------------
    // Firing

    /// `Loves2` (0x569fa0): put a projectile in the first free slot of the weapon's pool.
    #[allow(clippy::too_many_arguments)]
    fn fire(&mut self, t: &mut Tick, owner: i32, origin: V4, kind: i32, shooter: usize, target: usize, weapon: i32,
            count: i32) {
        if weapon == -1 {
            return;
        }
        if 8 < weapon && kind == shot_kind::SHIP && self.units[shooter].jammed && (9..=12).contains(&weapon) {
            return;
        }
        let w = &t.rules.weapons[weapon as usize];
        let form = w.form as usize;
        if !t.rules.forms[form].loaded {
            return;
        }
        let slots = t.rules.forms[form].slots as i32;
        let pool = &mut self.pools[form].shots;
        let mut i = 0i32;
        while (i as usize) < pool.len() && pool[i as usize].shooter.is_some() && i < slots - 1 {
            i += 1;
        }
        if i == slots - 1 {
            return;
        }
        let si = i as usize;
        let mut hit = w.hit;
        {
            let s = &mut self.pools[form].shots[si];
            s.kind = kind;
            s.count = count;
            s.mult = 1.0;
            s.weapon = weapon;
            s.attached = false;
            s.astray = false;
            s.straight = false;
        }
        if kind == shot_kind::SHIP || kind == shot_kind::AT_FIGHTER || kind == shot_kind::AT_GUN {
            self.units[shooter].cloak_cooldown = roll(t.rng, 0, 20) + 40;
        }
        if kind == shot_kind::SHIP {
            if weapon == 5 || weapon == 6 {
                let d = dist2(self.units[shooter].pos, self.units[target].pos);
                self.pools[form].shots[si].mult = 1.0 / (d / 1000.0) + 1.0;
            }
            let u = &self.units[shooter];
            if 0 < u.experience {
                let x = (u.experience as f32 / u.ship_count as f32) as f64;
                let m = self.pools[form].shots[si].mult;
                self.pools[form].shots[si].mult = ((x / 200.0 + 1.0) * m as f64) as f32;
            }
        }
        // The human's story bonus tests the slot's previous owner: the new one isn't written yet.
        if !t.rules.multiplayer && self.pools[form].shots[si].owner == self.human {
            let b = (t.rules.story_hit_bonus as f64 / 100.0 + 1.0) as f32;
            hit = trunc(100.0 - (100 - hit) as f32 / b);
            let k = (t.rules.story_damage_bonus as f64 / 100.0 + 1.0) as f32;
            let s = &mut self.pools[form].shots[si];
            s.mult *= k;
        }
        if kind == shot_kind::SHIP && self.units[shooter].pods & pod::DAMAGE != 0 {
            let s = &mut self.pools[form].shots[si];
            s.mult = (s.mult as f64 * 1.3) as f32;
        }
        if !w.aimed {
            if kind == shot_kind::SHIP {
                let u = &self.units[shooter];
                let mut d = u.heading;
                normalize(&mut d);
                let sp = w.spread / 1000.0;
                d.x += (roll(t.rng, 0, 99) as f32 - 50.0) * sp;
                d.y += (roll(t.rng, 0, 99) as f32 - 50.0) * sp;
                d.z += (roll(t.rng, 0, 99) as f32 - 50.0) * sp;
                let scale = u.ship_type.scale;
                let mut p = u.pos;
                p.x += ((roll(t.rng, 0, 99) - 50) as f32 / 100.0) * scale * 1024.0;
                p.y += ((roll(t.rng, 0, 99) - 50) as f32 / 100.0) * scale * 1024.0;
                p.z += ((roll(t.rng, 0, 99) - 50) as f32 / 100.0) * scale * 1024.0;
                let hits = roll(t.rng, 0, 99) < hit;
                let (o, speed) = (u.owner, u.speed);
                let s = &mut self.pools[form].shots[si];
                s.dir = d;
                s.vel = d.scale(0.2);
                s.pos = p;
                s.shooter = Some(shooter);
                s.target = target;
                s.hits = hits;
                s.owner = o;
                s.speed = speed;
                s.age = 0;
                t.events.push(BattleEvent::Fired { shot: ShotRef { form, slot: si }, kind });
            }
        } else {
            let mut p = origin;
            let mut sp = 0.0f32;
            let d;
            if kind == shot_kind::SHIP {
                let scale = self.units[shooter].ship_type.scale;
                p.x += ((roll(t.rng, 0, 99) - 50) as f32 / 100.0) * scale * 1024.0;
                p.y += ((roll(t.rng, 0, 99) - 50) as f32 / 100.0) * scale * 1024.0;
                p.z += ((roll(t.rng, 0, 99) - 50) as f32 / 100.0) * scale * 1024.0;
                if self.units[target].speed != 0.0 && roll(t.rng, 0, 99) < 40 {
                    self.pools[form].shots[si].straight = true;
                }
            }
            match kind {
                shot_kind::SHIP | shot_kind::FIGHTER_SHIP | shot_kind::GUN => d = self.units[target].pos.sub(p),
                shot_kind::AT_GUN => {
                    d = self.guns[target].pos.sub(p);
                    self.pools[form].shots[si].straight = true;
                }
                _ => {
                    // At a fighter: lead it by its velocity.
                    let f = &self.fighters[target];
                    let e = f.pos.sub(p);
                    let tt = e.len() / w.speed;
                    let lead = V4 { x: f.vel.x * tt + f.pos.x, y: f.vel.y * tt + f.pos.y, z: f.vel.z * tt + f.pos.z,
                                    w: f.pos.w };
                    d = lead.sub(p);
                }
            }
            let mut n = normalized(V4 { w: 0.0, ..d });
            if kind != shot_kind::AT_FIGHTER {
                sp = w.spread / 1000.0;
            }
            n.x += (roll(t.rng, 0, 99) as f32 - 50.0) * sp;
            n.y += (roll(t.rng, 0, 99) as f32 - 50.0) * sp;
            n.z += (roll(t.rng, 0, 99) as f32 - 50.0) * sp;
            normalize(&mut n);
            let hits = roll(t.rng, 0, 99) < hit;
            let s = &mut self.pools[form].shots[si];
            s.pos = p;
            s.dir = n;
            s.vel = V4::new(w.speed * n.x, n.y * w.speed, n.z * w.speed);
            s.shooter = Some(shooter);
            s.target = target;
            s.hits = hits;
            s.owner = owner;
            s.speed = w.speed;
            s.age = 0;
            if kind == shot_kind::AT_FIGHTER {
                s.hits = true;
            }
            t.events.push(BattleEvent::Fired { shot: ShotRef { form, slot: si }, kind });
        }
    }

    /// Mounts ready to fire, in mount order.
    fn ready(&self, u: usize, m: usize) -> bool {
        let x = &self.units[u];
        let mt = &x.mounts[m];
        mt.timer < 0 && (x.weapon_mask >> (mt.weapon & 31)) & 1 != 0
    }

    fn rearm(&mut self, t: &mut Tick, u: usize, m: usize, line_roll: bool) {
        let _ = line_roll;
        let r = reload_roll(t.rng, self.units[u].mounts[m].reload);
        let mt = &mut self.units[u].mounts[m];
        mt.timer = r;
        mt.timer_set = r;
    }

    /// `Lov` (0x569cf0): every ready ship weapon in range fires at unit `target`.
    fn fire_at_unit(&mut self, t: &mut Tick, u: usize, target: usize, d: f32) {
        for m in 0..self.units[u].mounts.len() {
            let mt = &self.units[u].mounts[m];
            if self.ready(u, m) && mt.kind != 2 && d < mt.range as f32 {
                let (w, c) = (mt.weapon, mt.count);
                self.rearm(t, u, m, true);
                let (o, p) = (self.units[u].owner, self.units[u].pos);
                self.fire(t, o, p, shot_kind::SHIP, u, target, w, c);
            }
        }
    }

    fn live_guns(&self) -> Vec<usize> {
        (0..self.guns.len()).filter(|&g| self.guns[g].active && self.guns[g].building != 0).collect()
    }

    /// `Lov2` (0x569910): anti-planet weapons fire at a random live gun.
    fn fire_at_any_gun(&mut self, t: &mut Tick, u: usize) {
        if self.hostile(self.units[u].owner, self.planet_owner) != 1 {
            return;
        }
        for m in 0..self.units[u].mounts.len() {
            if !(self.ready(u, m) && self.units[u].mounts[m].kind == 2) {
                continue;
            }
            let guns = self.live_guns();
            if guns.is_empty() {
                continue;
            }
            let (w, c) = (self.units[u].mounts[m].weapon, self.units[u].mounts[m].count);
            self.rearm(t, u, m, true);
            let k = roll(t.rng, 0, guns.len() as i32 - 1) as usize;
            let (o, p) = (self.units[u].owner, self.units[u].pos);
            self.fire(t, o, p, shot_kind::AT_GUN, u, guns[k], w, c);
        }
    }

    /// `Lov3` (0x569ab0): every ready weapon fires at gun `g` while any gun stands.
    fn fire_at_gun(&mut self, t: &mut Tick, u: usize, g: usize) {
        if self.hostile(self.units[u].owner, self.planet_owner) != 1 {
            return;
        }
        for m in 0..self.units[u].mounts.len() {
            if !self.ready(u, m) || self.live_guns().is_empty() {
                continue;
            }
            let (w, c) = (self.units[u].mounts[m].weapon, self.units[u].mounts[m].count);
            self.rearm(t, u, m, true);
            let (o, p) = (self.units[u].owner, self.units[u].pos);
            self.fire(t, o, p, shot_kind::AT_GUN, u, g, w, c);
        }
    }

    /// `Lov4` (0x569c40): light weapons (ids < 5) fire at fighter group `f`.
    fn fire_at_fighter(&mut self, t: &mut Tick, u: usize, f: usize) {
        for m in 0..self.units[u].mounts.len() {
            if !(self.ready(u, m) && self.units[u].mounts[m].weapon < 5) {
                continue;
            }
            let (w, c) = (self.units[u].mounts[m].weapon, self.units[u].mounts[m].count);
            self.rearm(t, u, m, true);
            let (o, p) = (self.units[u].owner, self.units[u].pos);
            self.fire(t, o, p, shot_kind::AT_FIGHTER, u, f, w, c);
        }
    }

    /// `Iranyitas` (0x568dc0): disabled/taken-over counters, ordered attacks, and every tick an automatic
    /// pick of the nearest fighter, unit and gun in range.
    fn aim_and_fire(&mut self, t: &mut Tick) {
        self.aim_timer -= 1;
        for i in 0..self.units.len() {
            if !self.units[i].alive {
                continue;
            }
            let mut out_of_range = false;
            self.units[i].picked = -1;
            if self.units[i].target_kind != target_kind::GUN {
                self.fire_at_any_gun(t, i);
            }
            let u = &mut self.units[i];
            if u.disabled != 0 {
                u.disabled -= 1;
            }
            if u.taken != 0 {
                u.taken -= 1;
                if u.taken == 0 {
                    u.owner = u.original_owner;
                }
            }
            let u = &self.units[i];
            if u.order == order::ATTACK {
                if let (Some(tg), target_kind::UNIT) = (u.target, u.target_kind) {
                    if !self.units[tg].alive {
                        let u = &mut self.units[i];
                        u.order = order::NONE;
                        u.target = None;
                        u.target_kind = target_kind::NONE;
                    } else {
                        let d = dist(self.units[i].pos, self.units[tg].pos);
                        if self.units[i].range_max <= d {
                            out_of_range = true;
                        } else {
                            self.fire_at_unit(t, i, tg, d);
                            self.units[i].picked = self.units[tg].id;
                        }
                    }
                }
                let u = &self.units[i];
                if u.order == order::ATTACK && u.target_kind == target_kind::GUN {
                    if let Some(g) = u.target {
                        if self.guns[g].building != 0 {
                            self.fire_at_gun(t, i, g);
                        }
                    }
                }
            }
            let u = &self.units[i];
            if (u.order != order::ATTACK || u.target.is_none() || out_of_range) && self.aim_timer < 0 {
                let owner = u.owner;
                let pos = u.pos;
                let r = u.range_max;
                // Nearest hostile fighter group within range (ties: the later one).
                let mut best2 = r * r;
                let mut pick = None;
                for f in 0..self.fighters.len() {
                    let fg = &self.fighters[f];
                    if fg.alive && self.hostile(owner, fg.owner) == 1 {
                        let d = dist2(pos, fg.pos);
                        if d <= best2 {
                            pick = Some(f);
                            best2 = d;
                        }
                    }
                }
                if let Some(f) = pick {
                    self.fire_at_fighter(t, i, f);
                }
                // Nearest hostile, visible unit within range.
                let mut best = r;
                let mut pick = None;
                for j in 0..self.units.len() {
                    let v = &self.units[j];
                    if v.alive && self.hostile(owner, v.owner) == 1 && v.owner != 0 && !v.cloaked {
                        let d = dist(pos, v.pos);
                        if d <= best {
                            pick = Some(j);
                            best = d;
                        }
                    }
                }
                if let Some(j) = pick {
                    self.units[i].picked = j as i32;
                    self.fire_at_unit(t, i, j, best);
                }
                // The last standing gun, if the planet's owner is hostile.
                if self.hostile(self.planet_owner, self.units[i].owner) == 1 {
                    if let Some(g) = (0..self.guns.len()).rev().find(|&g| self.guns[g].building != 0) {
                        self.fire_at_gun(t, i, g);
                    }
                }
            }
        }
        if self.aim_timer < 0 {
            self.aim_timer = 0;
        }
    }

    // --------------------------------------------------------------------------------------------
    // Damage

    /// `dechp(unit, ship, damage)` (0x572260): shields absorb first; a single ship dies when its damage
    /// passes its hit points, a group loses ships in proportion.
    fn take_damage(&mut self, t: &mut Tick, u: usize, k: i32, dmg: i32) {
        let mut rest = dmg;
        if !self.invulnerable {
            let x = &mut self.units[u];
            let room = x.shield - x.shield_used;
            if 0 < room {
                let absorbed = if room < dmg {
                    rest = dmg - room;
                    room
                } else {
                    rest = 0;
                    dmg
                };
                x.shield_used += absorbed;
            }
            x.damage += rest;
        }
        let x = &self.units[u];
        let (hp, damage, n) = (x.hp, x.damage, x.ship_count());
        let left = hp - damage;
        if n == 1 {
            if (left as f32) < 0.0 {
                if x.kind < 8 {
                    let kind = if x.owner == self.human { 7 } else { 9 };
                    t.events.push(BattleEvent::Voice { unit: u, kind });
                }
                self.explode(t, u, k, true, true);
                return;
            }
            // "Hull failing" speech: draws stream 6, the ship speaks on a 0.
            if x.kind < 8 && hp < damage + rest * 2 && 0.9 < ((damage as f32 / hp as f32) as f64) && roll(t.rng, 0, 2) == 0 {
                t.events.push(BattleEvent::Voice { unit: u, kind: 6 });
            }
        } else {
            if (left as f32) < 0.0 || hp < damage {
                for _ in 0..n.max(0) {
                    self.explode(t, u, 0, true, true);
                }
                return;
            }
            if hp != 0 && x.hp_share != 0 && n != 0 {
                let per = hp as f32 / x.hp_share as f32;
                if per != 0.0 {
                    let m = trunc(rest as f32 / per);
                    if m < 1 {
                        if 1.5 / (n as f64) < ((damage as f32 / hp as f32) as f64) {
                            self.explode(t, u, k, true, true);
                        }
                    } else if m == 1 {
                        self.explode(t, u, k, true, true);
                    } else {
                        for _ in 0..m {
                            self.explode(t, u, 0, true, true);
                        }
                    }
                }
            }
        }
    }

    /// `felrobban(unit, ship, …, count, real)` (0x5737e0): ship `k` of the unit is lost (the whole unit
    /// when it is the last, or when `real` is false).
    fn explode(&mut self, t: &mut Tick, u: usize, k: i32, count: bool, real: bool) {
        let owner = self.units[u].owner;
        if real && owner != 0 {
            let x = &self.units[u];
            let lost: i32 = if x.ships.first().copied().unwrap_or(0) < 0 {
                x.mounts.iter().map(|m| m.damage4).fold(0i32, i32::wrapping_add)
            } else {
                (0..x.ship_count().max(0) as usize)
                    .filter(|&j| x.ships[j] >= 0)
                    .map(|j| t.world.ship_attack(owner, x.ships[j]))
                    .fold(0i32, i32::wrapping_add)
            };
            let s = &mut self.sides[owner as usize];
            s.lost = s.lost.wrapping_add(lost);
        }
        t.events.push(BattleEvent::ShipLost { unit: u });
        if self.units[u].ship_count() == 1 || !real {
            let x = &mut self.units[u];
            x.alive = false;
            x.lightning = None;
            if count {
                x.exploding += 1;
            }
            self.end_timer = self.end_period;
            if real && owner != 0 {
                self.units[u].ship_count -= 1;
                self.clear_ship(t, u, k);
            }
        } else if owner != 0 {
            self.clear_ship(t, u, k);
            let x = &mut self.units[u];
            x.ship_count -= 1;
            let n = x.ship_count;
            let mut j = k;
            while j < n {
                x.ships[j as usize] = x.ships[(j + 1) as usize];
                j += 1;
            }
            let f = n as f32 / (n + 1) as f32;
            x.damage = trunc(x.damage as f32 * f);
            for m in &mut x.mounts {
                if m.count == 1 {
                    m.count = 0;
                } else {
                    m.count = trunc(m.count as f32 * f);
                    if m.count == 0 {
                        m.count = 1;
                    }
                }
            }
        }
        self.recount_present();
    }

    /// `clearship(unit, k)` (0x571fe0): the strategic side of a lost ship.
    fn clear_ship(&mut self, t: &mut Tick, u: usize, k: i32) {
        let idx = self.units[u].ships.get(k.max(0) as usize).copied().unwrap_or(0);
        if idx < 0 {
            if self.planet_owner != 0 && self.planet != 0 {
                t.world.remove_planet_ship(self.planet, -idx);
            }
            return;
        }
        let fleet = t.world.ship_fleet(idx);
        t.world.delete_ship(idx);
        if t.world.fleet_empty(fleet) {
            t.world.delete_fleet(fleet);
            let retreating = self.units[u].retreating;
            if let Some(p) = self.participants.iter_mut().find(|p| p.fleet == fleet) {
                p.status = if retreating { 2 } else { 1 };
            }
        }
    }

    /// `resztvevobirodalmakujraszamol` (0x57ec00): which empires still have ships, fighters, guns.
    fn recount_present(&mut self) {
        let mut list: Vec<(u8, u8)> = Vec::new();
        let note = |list: &mut Vec<(u8, u8)>, e: i32, bit: u8| {
            let mut found = false;
            for x in list.iter_mut() {
                if x.0 as u32 == e as u32 {
                    x.1 |= bit;
                    found = true;
                }
            }
            if !found {
                list.push((e as u8, bit));
            }
        };
        for u in &self.units {
            if u.alive && u.owner != 0 {
                note(&mut list, u.owner, 1);
            }
        }
        for f in &self.fighters {
            if f.alive && f.owner != 0 {
                note(&mut list, f.owner, 2);
            }
        }
        for g in &self.guns {
            if g.building != 0 {
                note(&mut list, self.planet_owner, 4);
            }
        }
        self.present.empires = list;
    }

}

impl Unit {
    /// `+0x1b0`.
    pub fn ship_count(&self) -> i32 {
        self.ship_count
    }
}

impl Battle {
    // --------------------------------------------------------------------------------------------
    // Projectiles

    /// `SpaceWarLovedekek` (0x56f320): every pool, in form order, moves its projectiles
    /// (`SpaceWarLovedekMozgat` 0x56e890) and then refreshes their sprites (`SpaceWarLovedekKirajzol`
    /// 0x56e1c0; only weapon 11 reads a sprite back).
    fn move_shots(&mut self, t: &mut Tick) {
        let scenery: Vec<usize> = (0..self.units.len()).filter(|&i| self.units[i].alive && self.units[i].owner == 0).collect();
        for form in 0..FORMS.min(self.pools.len()) {
            if !t.rules.forms[form].loaded {
                continue;
            }
            for slot in 0..t.rules.forms[form].slots.min(self.pools[form].shots.len()) {
                if self.pools[form].shots[slot].shooter.is_some() {
                    self.move_shot(t, ShotRef { form, slot }, &scenery);
                }
            }
            self.refresh_sprites(t, form);
        }
    }

    fn move_shot(&mut self, t: &mut Tick, r: ShotRef, scenery: &[usize]) {
        let w = t.rules.weapons[self.shot(r).weapon as usize].clone();
        // Missiles (13..=15) at a ship may be lured away by decoys.
        let s = self.shot(r).clone();
        if 0xc < s.weapon && s.weapon < 0x10 && s.kind == shot_kind::SHIP {
            let tg = &self.units[s.target];
            let sh = &self.units[s.shooter.unwrap()];
            if tg.pods & pod::DECOY != 0 && sh.pods & pod::SCANNER == 0 && roll(t.rng, 0, 100) < 1 {
                let x = self.shot_mut(r);
                x.astray = true;
                x.hits = false;
            }
            if self.shot(r).kind == shot_kind::SHIP && self.units[s.target].pods & pod::DECOY2 != 0
                && roll(t.rng, 0, 100) < 2 {
                let x = self.shot_mut(r);
                x.astray = true;
                x.hits = false;
            }
        }
        if !self.shot(r).attached {
            if w.turn != 0.0 && !self.shot(r).straight {
                self.home(r, &w);
                let x = self.shot(r);
                let mut target = w.speed;
                if x.kind == shot_kind::AT_GUN {
                    target *= 3.0;
                }
                let mut dv = target - x.speed;
                if w.accel < dv {
                    dv = w.accel;
                }
                if dv < -w.accel {
                    dv = -w.accel;
                }
                let x = self.shot_mut(r);
                x.speed += dv;
                x.vel.x = x.dir.x * x.speed;
                x.vel.y = x.dir.y * x.speed;
                x.vel.z = x.dir.z * x.speed;
            }
            let x = self.shot_mut(r);
            x.pos.x += x.vel.x;
            x.pos.y += x.vel.y;
            x.pos.z += x.vel.z;
        } else {
            let p = self.units[self.shot(r).target].pos;
            self.shot_mut(r).pos = p;
        }
        if !self.shot(r).hits {
            if self.shot(r).weapon < 0xd {
                self.shot_mut(r).straight = true;
            }
        } else {
            let s = self.shot(r).clone();
            if s.kind == shot_kind::AT_FIGHTER || s.kind == shot_kind::FIGHTER_FIGHTER {
                let f = &self.fighters[s.target];
                if f.alive && dist(s.pos, f.pos) < 3.0 && Self::passed(f.pos, s.pos, s.vel) {
                    self.hit_fighters(t, r);
                }
            }
            let s = self.shot(r).clone();
            if s.kind == shot_kind::AT_GUN && dist(s.pos, self.guns[s.target].pos) < 100.0 {
                self.hit_gun(t, r);
            }
            let s = self.shot(r).clone();
            if s.kind < shot_kind::AT_GUN {
                let tg = &self.units[s.target];
                if tg.alive && 0.0 < s.speed {
                    let d = dist2(s.pos, tg.pos);
                    let rr = tg.radius * 3.0;
                    if d < rr * rr && Self::passed(tg.pos, s.pos, s.vel) {
                        self.hit_unit(t, r);
                    }
                }
                for &a in scenery {
                    let s = self.shot(r);
                    let u = &self.units[a];
                    if dist(s.pos, u.pos) < u.ship_type.scale * 1024.0 {
                        self.shot_mut(r).target = a;
                        self.hit_unit(t, r);
                    }
                }
            }
        }
        self.shot_mut(r).age += 1;
        let s = self.shot(r).clone();
        if s.weapon == 0xb {
            // Area damage around the sprite: the sprite is in drawing scale, the units are not.
            for u in 0..self.units.len() {
                if self.units[u].alive && dist(self.units[u].pos, s.sprite_pos) < s.sprite_size / 3.0 {
                    // The ship count is read again every round (a wiped group stops taking hits).
                    let mut k = 0;
                    while k < self.units[u].ship_count() {
                        let dmg = trunc((w.damage_vs(&t.weapon_words[s.weapon as usize], self.units[u].kind).wrapping_mul(s.count)) as f32 * s.mult);
                        self.take_damage(t, u, k, dmg);
                        k += 1;
                    }
                }
            }
        }
        let s = self.shot(r).clone();
        if s.weapon == 0xc && s.kind == shot_kind::SHIP {
            // Chain lightning: the target burns every tick.
            let tg = s.target;
            let mut k = 0;
            while k < self.units[tg].ship_count() {
                let dmg = trunc((w.damage_vs(&t.weapon_words[s.weapon as usize], self.units[tg].kind).wrapping_mul(s.count)) as f32 * s.mult);
                self.take_damage(t, tg, k, dmg);
                k += 1;
            }
            if !self.units[tg].alive {
                self.shot_mut(r).shooter = None;
            }
        }
        let mut life = w.duration;
        if self.shot(r).kind == shot_kind::AT_GUN {
            life *= 10;
        }
        if life < self.shot(r).age {
            self.shot_mut(r).shooter = None;
        }
        let s = self.shot(r).clone();
        if s.weapon == 0xc && s.kind == shot_kind::SHIP {
            if let Some(sh) = s.shooter {
                // The arc jumps to every unit and fighter within a sixteenth of its length (squared).
                let reach = trunc(dist2(self.units[sh].pos, self.units[s.target].pos) * 0.0625) as f32;
                let tp = self.units[s.target].pos;
                for u in 0..self.units.len() {
                    if !self.units[u].alive || u == s.target {
                        continue;
                    }
                    let d = dist2(self.units[u].pos, tp);
                    if self.units[u].lightning.is_none() {
                        if d <= reach {
                            self.units[u].lightning = Some(r);
                        }
                    } else if reach < d {
                        self.units[u].lightning = None;
                    }
                }
                for f in 0..self.fighters.len() {
                    if !self.fighters[f].alive {
                        continue;
                    }
                    let d = dist2(self.fighters[f].pos, tp);
                    if self.fighters[f].lightning.is_none() {
                        if d <= reach {
                            self.fighters[f].lightning = Some(r);
                        }
                    } else if reach < d {
                        self.fighters[f].lightning = None;
                    }
                }
            }
        }
        let s = self.shot(r);
        if s.attached && s.kind == shot_kind::SHIP {
            if let Some(sh) = s.shooter {
                if !self.units[sh].alive || !self.units[s.target].alive {
                    self.shot_mut(r).shooter = None;
                }
            }
        }
    }

    /// The projectile is inside the target's hit sphere and this tick's move carries it past the centre:
    /// `|n(c − (p + v)) − n(c − p)| > 1.414` (f64 compare).
    fn passed(c: V4, p: V4, v: V4) -> bool {
        let ahead = V4::new(v.x + p.x, v.y + p.y, v.z + p.z);
        let mut a = c.sub(ahead);
        let mut b = c.sub(p);
        normalize(&mut a);
        normalize(&mut b);
        (b.sub(a).len() as f64) > 1.414
    }

    /// `setlovedekrotate/speed/vector` (0x580130): a homing projectile turns towards its target.
    fn home(&mut self, r: ShotRef, w: &WeaponRules) {
        let s = self.shot(r).clone();
        let (mut goal, alive) = match s.kind {
            shot_kind::SHIP | shot_kind::GUN | shot_kind::FIGHTER_SHIP => {
                let u = &self.units[s.target];
                (u.pos, u.alive)
            }
            shot_kind::AT_GUN => {
                let g = &self.guns[s.target];
                (g.pos, g.building != 0)
            }
            _ => {
                let f = &self.fighters[s.target];
                (f.pos, f.alive)
            }
        };
        if !alive {
            self.shot_mut(r).straight = true;
        }
        if s.astray {
            let x = goal.y * -10.0;
            goal.y = goal.z * -8.0;
            goal.z = x * -7.0;
            goal.x = x;
        }
        let d = V4::new(goal.x - s.pos.x, goal.y - s.pos.y, goal.z - s.pos.z);
        let n = normalized(d);
        let e = V4::new(n.x - s.dir.x, n.y - s.dir.y, n.z - s.dir.z);
        let m = e.y * e.y + e.x * e.x + e.z * e.z;
        let step = if w.turn < m { w.turn } else { m };
        let x = self.shot_mut(r);
        x.turn = m;
        if m != 0.0 {
            x.dir.x += (step * e.x) / m;
            x.dir.y += (step * e.y) / m;
            x.dir.z += (step * e.z) / m;
        }
    }

    /// `Talalat` (0x570d60): a hit on a unit.
    fn hit_unit(&mut self, t: &mut Tick, r: ShotRef) {
        let s = self.shot(r).clone();
        let w = &t.rules.weapons[s.weapon as usize];
        let u = s.target;
        let tg = &self.units[u];
        if !tg.alive || tg.retreating {
            if !w.beam {
                self.shot_mut(r).shooter = None;
            }
            return;
        }
        if 8 <= tg.kind && (s.weapon == 9 || s.weapon == 10) {
            self.shot_mut(r).shooter = None;
            return;
        }
        let k = roll(t.rng, 0, tg.ship_count() - 1);
        let base = trunc((w.damage_vs(&t.weapon_words[s.weapon as usize], tg.kind).wrapping_mul(s.count)) as f32 * s.mult);
        let mut d = s.dir;
        normalize(&mut d);
        let back = tg.heading.sub(d).len2();
        let mut dmg = trunc(base as f32 / ((4.0 - back) * 0.25 + 1.0));
        if dmg < 1 {
            dmg = 1;
        }
        if w.beam {
            let p = self.units[u].pos;
            let tick = self.tick;
            let x = self.shot_mut(r);
            x.vel = V4 { w: x.vel.w, ..V4::default() };
            x.speed = 0.0;
            x.attached = true;
            x.pos = p;
            x.attached_tick = tick;
        }
        let dur = w.duration;
        if s.weapon == 9 {
            if self.units[u].disabled == 0 {
                let x = &mut self.units[u];
                x.disabled = dur - s.age;
                x.order = order::NONE;
                x.target = None;
                x.target_kind = target_kind::NONE;
            } else {
                self.shot_mut(r).shooter = None;
            }
        }
        if s.weapon == 10 {
            if self.units[u].taken == 0 {
                let x = &mut self.units[u];
                x.owner = s.owner;
                x.taken = dur - s.age;
            } else {
                self.shot_mut(r).shooter = None;
            }
            let x = &mut self.units[u];
            x.order = order::MOVE;
            x.target = None;
            x.target_kind = target_kind::NONE;
        }
        let s2 = self.shot(r).clone();
        if s2.kind == shot_kind::GUN {
            if let Some(g) = s2.shooter {
                self.guns[g].hits += 1;
                self.guns[g].active = true;
            }
        }
        t.events.push(BattleEvent::Hit { unit: u, damage: dmg, beam: w.beam, weapon: s2.weapon });
        let beam = w.beam;
        self.take_damage(t, u, k, dmg);
        if !beam {
            self.shot_mut(r).shooter = None;
        }
    }

    /// `VadaszTalalat` (0x5711a0): fighters lose `damage[1] × count / hp` craft (at least one).
    fn hit_fighters(&mut self, t: &mut Tick, r: ShotRef) {
        let s = self.shot(r).clone();
        let w = &t.rules.weapons[s.weapon as usize];
        let f = &self.fighters[s.target];
        let mut n = w.damage[1].wrapping_mul(s.count) / f.hp;
        if n < 1 {
            n = 1;
        }
        if f.count < n {
            n = f.count;
        }
        if n != 0 {
            self.lose_fighters(t, s.target, n, true);
        }
    }

    /// `VadaszRobban`-like loss of `n` fighters of group `f` (0x5710f0 / 0x566ad0 / 0x5711a0).
    fn lose_fighters(&mut self, t: &mut Tick, f: usize, n: i32, strategic: bool) {
        if strategic {
            let (fleet, design) = (self.fighters[f].fleet, self.fighters[f].design);
            t.world.delete_fighters(fleet, design, n);
            if t.world.fleet_empty(fleet) {
                t.world.delete_fleet(fleet);
            }
        }
        t.events.push(BattleEvent::FightersLost { fighter: f, count: n });
        let g = &mut self.fighters[f];
        g.count -= n;
        if g.count == 0 {
            g.alive = false;
            g.lightning = None;
        }
        self.recount_present();
    }

    /// `FoldTalalat` (0x566ff0): a hit on a planet gun damages its building.
    fn hit_gun(&mut self, t: &mut Tick, r: ShotRef) {
        let s = self.shot(r).clone();
        if (9..=12).contains(&s.weapon) {
            return;
        }
        let g = s.target;
        if self.guns[g].armour > 1.0 {
            t.events.push(BattleEvent::ShieldHit { gun: g, at: s.pos });
        }
        let slot = self.guns[g].building;
        if slot != 0 {
            let w = &t.rules.weapons[s.weapon as usize];
            let raw = trunc((w.damage[5].wrapping_mul(s.count)) as f32 * s.mult / self.guns[g].armour);
            let dmg = trunc64(raw as f64 * 6.666666666666667 * 2.0);
            if t.world.damage_building(self.planet, slot, dmg) {
                self.guns[g].building = 0;
                self.guns[g].weapon = -1;
                t.events.push(BattleEvent::GunDestroyed { gun: g });
                self.recount_present();
            }
            self.shot_mut(r).shooter = None;
        }
    }

    /// The sprite pass (0x56e1c0), reduced to what the battle reads back: a free slot's sprite has no
    /// size; weapon 11's sprite sits at the scaled position with a size that swells once it is attached.
    fn refresh_sprites(&mut self, t: &Tick, form: usize) {
        let size = t.rules.forms[form].size;
        let scale = self.scale;
        let tick = self.tick;
        for slot in 0..t.rules.forms[form].slots.min(self.pools[form].shots.len()) {
            let s = &mut self.pools[form].shots[slot];
            if s.shooter.is_none() {
                s.sprite_size = 0.0;
                continue;
            }
            if s.weapon != 0xb {
                continue;
            }
            s.sprite_pos = V4 { x: s.pos.x * scale, y: s.pos.y * scale, z: s.pos.z * scale, w: s.sprite_pos.w };
            let dur = t.rules.weapons[0xb].duration;
            s.sprite_size = if s.attached {
                let since = s.attached_tick - tick + s.age;
                let rest = dur - since;
                let gone = s.age - since;
                let f = if gone < rest / 2 { gone as f32 } else { rest as f32 - gone as f32 };
                (80.0 - (1.0 / (f / 20.0 + 1.0)) * (80.0 - size)) * scale
            } else {
                size * scale
            };
        }
    }

    fn shot(&self, r: ShotRef) -> &Shot {
        &self.pools[r.form].shots[r.slot]
    }

    fn shot_mut(&mut self, r: ShotRef) -> &mut Shot {
        &mut self.pools[r.form].shots[r.slot]
    }

    // --------------------------------------------------------------------------------------------
    // Planet guns

    /// `SpaceWarBolygo` (0x56d740): each standing gun fires when its timer runs out, at its forced target
    /// or else at the first hostile, visible unit.
    fn planet_guns(&mut self, t: &mut Tick) {
        for g in 0..self.guns.len() {
            if self.guns[g].weapon == -1 {
                continue;
            }
            self.guns[g].timer -= 1;
            if 0 <= self.guns[g].timer {
                continue;
            }
            let ft = self.guns[g].target;
            if ft != -1 && !self.units[ft as usize].alive {
                self.guns[g].target = -1;
            }
            let reload = self.guns[g].reload;
            let r = roll(t.rng, 0, reload / 3 - 1) + reload;
            self.guns[g].timer = r;
            self.guns[g].timer_set = r;
            let target = if self.guns[g].target == -1 {
                let po = self.planet_owner;
                (0..self.units.len()).find(|&i| {
                    let u = &self.units[i];
                    u.alive && !u.cloaked && self.hostile(po, u.owner) == 1 && u.owner != 0
                })
            } else {
                Some(self.guns[g].target as usize)
            };
            if let Some(u) = target {
                let (o, p, w) = (self.planet_owner, self.guns[g].pos, self.guns[g].weapon);
                self.fire(t, o, p, shot_kind::GUN, g, u, w, 1);
            }
        }
    }

    // --------------------------------------------------------------------------------------------
    // Special pods

    /// `SpaceWarSpecialpodfrissit` (0x570480), every tenth call (it runs twice a tick): jammers, scanners
    /// and cloaks act on hostile units within 100 (a cloak breaks within 20), then every unit turns to
    /// face its business (`seturhajopos`).
    fn special_pods(&mut self) {
        self.pod_timer -= 1;
        if self.pod_timer != 0 {
            return;
        }
        self.pod_timer = 10;
        for u in &mut self.units {
            if u.alive {
                u.jammed = false;
                u.scanned = false;
                u.cloaked = false;
                if 0 < u.cloak_cooldown {
                    u.cloak_cooldown -= 1;
                }
            }
        }
        for i in 0..self.units.len() {
            let a = &self.units[i];
            if !a.alive || (a.pods & pod::JAMMER == 0 && a.pods & (pod::SCANNER | pod::CLOAK) == 0) {
                continue;
            }
            for j in 0..self.units.len() {
                if !self.units[j].alive || self.hostile(self.units[j].owner, self.units[i].owner) != 1 {
                    continue;
                }
                let d = dist2(self.units[j].pos, self.units[i].pos);
                if d < 10000.0 {
                    let pods = self.units[i].pods;
                    if pods & pod::JAMMER != 0 {
                        self.units[j].jammed = true;
                    }
                    if pods & pod::SCANNER != 0 {
                        self.units[j].scanned = true;
                    }
                    if pods & pod::CLOAK != 0 && self.units[i].cloak_cooldown == 0 && d < 400.0 {
                        self.units[i].cloak_cooldown = 50;
                    }
                }
            }
        }
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.alive && u.pods & pod::CLOAK != 0 && u.cloak_cooldown == 0 {
                self.units[i].cloaked = true;
            }
            self.face(i);
        }
    }
}

impl Battle {
    // --------------------------------------------------------------------------------------------
    // Fighters

    /// `SpaceVadaszMozgat` (0x56d530).
    fn move_fighters(&mut self, t: &mut Tick) {
        for i in 0..self.fighters.len() {
            if self.fighters[i].alive {
                self.fighter_think(t, i);
                self.fighter_steer(i);
                let f = &mut self.fighters[i];
                let mut want = (i as i32 % 5) as f32 * 0.025 + 0.5;
                if f.retreating {
                    want *= 7.0;
                }
                let mut dv = want - f.speed;
                // Not `clamp`: the exe compares in f64 with 0.1 but stores 0.1f32.
                #[allow(clippy::manual_clamp)]
                if (dv as f64) > 0.1 {
                    dv = 0.1;
                }
                if -0.1 > (dv as f64) {
                    dv = -0.1;
                }
                f.speed += dv;
                normalize(&mut f.dir);
                let s = f.speed;
                f.vel.x = s * f.dir.x;
                f.vel.y = f.dir.y * s;
                f.vel.z = f.dir.z * s;
                f.jitter = roll(t.rng, 0, 2) + 3;
                if self.fighters[i].timer < 1 {
                    self.fighter_fire(t, i);
                }
                let f = &mut self.fighters[i];
                f.timer -= 1;
                f.jitter -= 1;
                f.pos.x += f.vel.x;
                f.pos.y += f.vel.y;
                f.pos.z += f.vel.z;
            }
            if let Some(r) = self.fighters[i].lightning {
                if !self.fighters[i].alive || self.shot(r).shooter.is_none() {
                    self.fighters[i].lightning = None;
                } else {
                    let mut dmg = t.rules.weapons[0xc].damage[1] / 8;
                    if dmg < 1 {
                        dmg = 1;
                    }
                    let mut n = dmg / self.fighters[i].hp;
                    if n < 1 {
                        n = 1;
                    }
                    if self.fighters[i].count < n {
                        n = self.fighters[i].count;
                    }
                    if n != 0 {
                        self.lose_fighters(t, i, n, true);
                    }
                }
            }
        }
    }

    /// `setvadasztarget` (0x581100): pick, chase, overshoot, repeat.
    fn fighter_think(&mut self, t: &mut Tick, i: usize) {
        let f = &self.fighters[i];
        let (d, prev) = (f.goal_dist, f.goal_dist_prev);
        match f.state {
            0 => {
                if prev <= d {
                    let aggression = self.sides[f.owner as usize].fighter_aggression;
                    if roll(t.rng, 0, 99) < aggression {
                        if !self.fighter_pick_ship(i) {
                            self.fighter_pick_fighter(t, i);
                        }
                    } else if !self.fighter_pick_fighter(t, i) {
                        self.fighter_pick_ship(i);
                    }
                }
            }
            1 => {
                if d < prev {
                    let near = match f.target_kind {
                        1 => self.units[f.target.unwrap()].ship_type.scale * 1024.0 + 20.0,
                        2 => 8.0,
                        _ => f32::NAN,
                    };
                    if d < near {
                        let tp = match f.target_kind {
                            1 => self.units[f.target.unwrap()].pos,
                            2 => self.fighters[f.target.unwrap()].pos,
                            _ => f.dest,
                        };
                        let pos = f.pos;
                        let mut e = pos.sub(tp);
                        e.x += (roll(t.rng, -100, 100) / 60) as f32;
                        e.y += (roll(t.rng, -100, 100) / 60) as f32;
                        let rz = roll(t.rng, -100, 100) / 60;
                        let f = &mut self.fighters[i];
                        f.state = 2;
                        f.dest.x = pos.x + e.x;
                        f.dest.y = pos.y + e.y;
                        f.dest.z = pos.z + (rz as f32 + e.z);
                    }
                }
            }
            2
                if d < 7.0 => {
                    let f = &mut self.fighters[i];
                    f.state = 0;
                    f.target_kind = 0;
                    f.target = None;
                }
            _ => {}
        }
        let f = &self.fighters[i];
        if let (Some(tg), 1) = (f.target, f.target_kind) {
            let u = &self.units[tg];
            if u.retreating || !u.alive || u.cloaked {
                let f = &mut self.fighters[i];
                f.state = 0;
                f.target_kind = 0;
                f.target = None;
            }
        }
    }

    /// The last present empire hostile to `owner` that still has ships or fighters (0x572530).
    fn fighter_enemy(&self, owner: i32) -> i32 {
        let mut e = -1;
        for &(id, bits) in &self.present.empires {
            if self.hostile(owner, id as i32) == 1 && bits & 3 != 0 {
                e = id as i32;
            }
        }
        e
    }

    /// `vadsszhajotvalaszt` (0x5830d0 / 0x583190): the enemy unit with the lowest score
    /// `(int)(distance + damage %)` (−40 if it is busy with this owner's craft).
    fn fighter_pick_ship(&mut self, i: usize) -> bool {
        let owner = self.fighters[i].owner;
        let e = self.fighter_enemy(owner);
        let pos = self.fighters[i].pos;
        let mut best: Option<(i32, usize)> = None;
        for (j, u) in self.units.iter().enumerate() {
            if !u.alive || u.owner != e || u.cloaked {
                continue;
            }
            let mut score = if u.hp == 0 { 1 } else { u.damage.wrapping_mul(100) / u.hp };
            if let Some(a) = u.attacker {
                if self.units[a].owner == owner {
                    score -= 0x28;
                }
            }
            let v = trunc(dist(u.pos, pos) + score as f32);
            if best.is_none_or(|(b, _)| v < b) {
                best = Some((v, j));
            }
        }
        let Some((_, j)) = best else { return false };
        let p = self.units[j].pos;
        let f = &mut self.fighters[i];
        f.target = Some(j);
        f.dest.x = p.x;
        f.dest.y = p.y;
        f.dest.z = p.z;
        f.state = 1;
        f.target_kind = 1;
        true
    }

    /// `vadszvadasztvalaszt` (0x583130 / 0x583310): a random enemy fighter group.
    fn fighter_pick_fighter(&mut self, t: &mut Tick, i: usize) -> bool {
        let e = self.fighter_enemy(self.fighters[i].owner);
        let list: Vec<usize> = (0..self.fighters.len()).filter(|&j| self.fighters[j].alive && self.fighters[j].owner == e).collect();
        if list.is_empty() {
            return false;
        }
        let j = list[roll(t.rng, 0, list.len() as i32 - 1) as usize];
        let p = self.fighters[j].pos;
        let f = &mut self.fighters[i];
        f.target = Some(j);
        f.dest.x = p.x;
        f.dest.y = p.y;
        f.dest.z = p.z;
        f.state = 1;
        f.target_kind = 2;
        true
    }

    /// `setvadaszrotate/vector` (0x580f70): turn towards the goal, at most 0.08 a tick (no normalising).
    fn fighter_steer(&mut self, i: usize) {
        let f = &self.fighters[i];
        let d = if let (1, Some(tg), k) = (f.state, f.target, f.target_kind) {
            match k {
                1 => {
                    let p = self.units[tg].pos;
                    V4::new(p.x - f.pos.x, p.y - f.pos.y, p.z - f.pos.z)
                }
                2 => {
                    let p = self.fighters[tg].pos;
                    V4::new(p.x - f.pos.x, p.y - f.pos.y, p.z - f.pos.z)
                }
                _ => V4::new(f.dest.x - f.pos.x, f.dest.y - f.pos.y, f.dest.z - f.pos.z),
            }
        } else {
            V4::new(f.dest.x - f.pos.x, f.dest.y - f.pos.y, f.dest.z - f.pos.z)
        };
        let f = &mut self.fighters[i];
        f.goal_dist_prev = f.goal_dist;
        f.goal_dist = d.len();
        let n = normalized(d);
        let e = V4::new(n.x - f.dir.x, n.y - f.dir.y, n.z - f.dir.z);
        let m = e.y * e.y + e.x * e.x + e.z * e.z;
        let step = if (m as f64) > 0.08 { 0.08f32 } else { m };
        f.turn = m;
        if m != 0.0 {
            f.dir.x += step * e.x;
            f.dir.y += step * e.y;
            f.dir.z += step * e.z;
        }
    }

    /// `vadaszloves` (0x582dd0): fire when the target is within 35 and dead ahead.
    fn fighter_fire(&mut self, t: &mut Tick, i: usize) {
        let f = &self.fighters[i];
        let (Some(tg), kind) = (f.target, f.target_kind) else { return };
        let weapon = f.weapon + 0x1e;
        let tp = match kind {
            1 if self.units[tg].alive && self.units[tg].owner != f.owner => Some(self.units[tg].pos),
            2 if self.fighters[tg].alive && self.fighters[tg].owner != f.owner => Some(self.fighters[tg].pos),
            _ => None,
        };
        let Some(tp) = tp else { return };
        let mut d = tp.sub(f.pos);
        if d.len() < 35.0 {
            normalize(&mut d);
            let e = d.sub(f.dir);
            if (e.len() as f64) < 0.6 && f.timer < 1 {
                let (o, p, c) = (f.owner, f.pos, f.count);
                if kind == 1 {
                    self.fire(t, o, p, shot_kind::FIGHTER_SHIP, i, tg, weapon, 1);
                } else {
                    self.fire(t, o, p, shot_kind::FIGHTER_FIGHTER, i, tg, weapon, c);
                }
                let reload = t.rules.weapons[weapon as usize].reload;
                self.fighters[i].timer = roll(t.rng, 0, reload / 3 - 1) + reload;
            }
        }
    }

    // --------------------------------------------------------------------------------------------
    // Units: movement

    fn grid_of(&self, p: V4) -> [i32; 3] {
        let half = [self.grid[0] / 2, self.grid[1] / 2, self.grid[2] / 2];
        [
            trunc((self.cell[0] * 0.5 + p.y) / self.cell[0] + half[0] as f32),
            trunc((self.cell[1] * 0.5 + p.z) / self.cell[1] + half[1] as f32),
            trunc((self.cell[2] * 0.5 + p.x) / self.cell[2] + half[2] as f32),
        ]
    }

    fn pos_of(&self, c: [i32; 3]) -> V4 {
        V4::new(
            (c[2] - self.grid[2] / 2) as f32 * self.cell[2],
            (c[0] - self.grid[0] / 2) as f32 * self.cell[0],
            (c[1] - self.grid[1] / 2) as f32 * self.cell[1],
        )
    }

    /// Keep a point two cells inside the arena.
    fn clamp_inside(&self, p: &mut V4) {
        let lo = self.bounds[0] + self.cell[0] * 2.0;
        if p.y < lo {
            p.y = lo;
        }
        let hi = self.bounds[1] - self.cell[0] * 2.0;
        if hi < p.y {
            p.y = hi;
        }
        let lo = self.bounds[2] + self.cell[1] * 2.0;
        if p.z < lo {
            p.z = lo;
        }
        let hi = self.bounds[3] - self.cell[1] * 2.0;
        if hi < p.z {
            p.z = hi;
        }
        let lo = self.cell[2] * 2.0 + self.bounds[4];
        if p.x < lo {
            p.x = lo;
        }
        let hi = self.bounds[5] - self.cell[2] * 2.0;
        if hi < p.x {
            p.x = hi;
        }
    }

    /// `Mozgatas` (0x56b590).
    fn move_units(&mut self, t: &mut Tick) {
        for i in 0..self.units.len() {
            if self.units[i].retreating {
                // Away from the retreat point at boosted speed.
                let u = &self.units[i];
                let mut d = u.pos.sub(self.retreat_point);
                if d.len() == 0.0 {
                    d = V4::new(1.0, 0.0, 0.0);
                }
                normalize(&mut d);
                let p = u.pos;
                let u = &mut self.units[i];
                u.step.x = d.x * 300.0 + p.x;
                u.step.y = d.y * 300.0 + p.y;
                u.step.z = d.z * 300.0 + p.z;
                self.steer(i, true);
                let u = &mut self.units[i];
                u.pos.x += u.vel.x;
                u.pos.y += u.vel.y;
                u.pos.z += u.vel.z;
                let c = self.grid_of(self.units[i].pos);
                self.units[i].cell[..3].copy_from_slice(&c);
            } else if self.units[i].alive && self.units[i].order != order::NONE {
                self.plan(i);
                if self.steer(i, false) {
                    let u = &mut self.units[i];
                    u.speed = 0.0;
                    u.f144 = 0;
                    u.pos = u.step;
                    u.vel.x = 0.0;
                    u.vel.y = 0.0;
                    u.vel.z = 0.0;
                    u.cell = u.step_cell;
                    if u.order == order::MOVE && u.step_cell[..3] == u.waypoints[0][..3] {
                        if u.waypoint_count < 2 {
                            u.order = order::NONE;
                        } else {
                            self.next_waypoint(i);
                        }
                    }
                }
                let u = &mut self.units[i];
                u.pos.x += u.vel.x;
                u.pos.y += u.vel.y;
                u.pos.z += u.vel.z;
                let c = self.grid_of(self.units[i].pos);
                self.units[i].cell[..3].copy_from_slice(&c);
                let u = &self.units[i];
                if u.alive && u.order == order::ATTACK && u.target_kind == target_kind::UNIT {
                    if let Some(tg) = u.target {
                        if self.units[tg].cloaked {
                            let u = &mut self.units[i];
                            u.order = order::NONE;
                            u.target = None;
                            u.target_kind = target_kind::NONE;
                        }
                    }
                }
            }
            let u = &mut self.units[i];
            let n = i as i32;
            if u.owner == 0 {
                u.spin[0] += (n.wrapping_mul(0xc4845f) % 0x2b) as f32 / 1500.0;
                u.spin[1] += (n.wrapping_mul(0xbc6e0f) % 0x2b) as f32 / 1500.0;
                u.spin[2] += (n.wrapping_mul(0xc47f50) % 0x2b) as f32 / 1500.0;
            }
            if (u.speed as f64) <= 0.01 {
                u.idle += 1;
            } else {
                u.idle = 0;
            }
            // Bobbing.
            let a = u.idle as f32 / ((n.wrapping_mul(0x16df2b) % 0x2a) as f32 * 0.25 + 5.0);
            let s = (a as f64).sin() as f32;
            u.pos.x = (s as f64 * 0.01 + u.pos.x as f64) as f32;
            self.face(i);
            if let Some(r) = self.units[i].lightning {
                let sh = self.shot(r).shooter;
                if !self.units[i].alive || sh.is_none() {
                    self.units[i].lightning = None;
                } else {
                    let kind = self.units[i].kind;
                    let dmg = t.rules.weapons[0xc].damage_vs(&t.weapon_words[0xc], kind) / 2;
                    let mut k = 0;
                    while k < self.units[i].ship_count() {
                        self.take_damage(t, i, k, dmg);
                        k += 1;
                    }
                }
            }
        }
    }

    /// Re-plan the path when the unit drifted more than 1.6 cells' worth (squared) or every 50 ticks.
    fn plan(&mut self, i: usize) {
        let u = &self.units[i];
        let drift = dist2(u.pos, u.plan_pos) as f64;
        if !((self.cell[0] as f64) * 1.6 < drift || 0x32 < self.tick - u.plan_tick) {
            return;
        }
        self.units[i].path_short = false;
        let u = &self.units[i];
        if u.order == order::ATTACK && u.target_kind == target_kind::UNIT {
            let tc = self.units[u.target.unwrap()].cell;
            let mut dest = self.pos_of([tc[0], tc[1], tc[2]]);
            dest.w = self.units[i].dest.w;
            self.units[i].waypoints[0] = tc;
            self.clamp_inside(&mut dest);
            let c = self.grid_of(dest);
            let u = &mut self.units[i];
            u.dest = dest;
            u.waypoints[0][..3].copy_from_slice(&c);
        }
        let u = &self.units[i];
        let radius = if u.order == order::MOVE { 1 } else { trunc(u.range_min / self.cell[0]) };
        let clamp = |w: &mut [i32; 4]| {
            w[0] = w[0].clamp(1, 0x7f);
            w[1] = w[1].clamp(1, 0x7f);
            w[2] = w[2].clamp(1, 7);
        };
        let mut from_step = false;
        if u.order == order::ATTACK && u.target_kind == target_kind::UNIT {
            let dx = (u.cell[1] - u.step_cell[1]) as f32;
            let dy = (u.cell[0] - u.step_cell[0]) as f32;
            let dz = (u.cell[2] - u.step_cell[2]) as f32;
            if sqrt32(dx * dx + dy * dy + dz * dz) < 1.5 {
                from_step = true;
            }
        }
        if from_step {
            clamp(&mut self.units[i].waypoints[0]);
            let u = &self.units[i];
            let (s, w) = (u.step_cell, u.waypoints[0]);
            self.path_search([s[0], s[1], s[2]], [w[0], w[1], w[2]], radius, i);
            self.units[i].path_short = self.search.one_short || self.search.arrived;
        } else if !self.units[i].path_short {
            clamp(&mut self.units[i].waypoints[0]);
            let u = &self.units[i];
            let (c, w) = (u.cell, u.waypoints[0]);
            self.path_search([c[0], c[1], c[2]], [w[0], w[1], w[2]], radius, i);
            let u = &mut self.units[i];
            if (!self.search.one_short && !self.search.arrived) || 2 < u.order {
                u.path_fails = 0;
            } else {
                u.path_fails += 1;
                if 6 < u.path_fails {
                    u.order = order::NONE;
                }
            }
        }
        let r = self.search.result;
        let p = self.pos_of(r);
        let tick = self.tick;
        let u = &mut self.units[i];
        u.path_short = self.search.one_short || self.search.arrived;
        u.step_cell[..3].copy_from_slice(&r);
        u.step.y = p.y;
        u.step.z = p.z;
        u.step.x = p.x;
        u.plan_pos = u.pos;
        u.plan_tick = tick;
    }

    /// `setvector22(unit, boost)` (0x581380): turn the heading towards the next path step, then speed up
    /// or brake. Returns true when the unit is already there.
    fn steer(&mut self, i: usize, boost: bool) -> bool {
        let u = &self.units[i];
        let ty = u.ship_type;
        let (top, mut accel, mut turn) = if boost {
            (ty.top_speed * 10.0, ty.accel, ty.turn)
        } else {
            (
                (ty.top_speed as f64 / 2.5) as f32,
                (ty.accel as f64 / 2.5) as f32,
                (ty.turn as f64 / 2.5) as f32,
            )
        };
        if (top as f64) < 0.01 {
            return false;
        }
        let short = u.path_short;
        let mut h = u.heading;
        let mut d = u.step.sub(u.pos);
        d.w = 0.0;
        let len = d.len();
        let mut parked = false;
        if (len as f64) < 0.1 {
            if short {
                return true;
            }
            parked = true;
            d = h;
        }
        normalize(&mut d);
        normalize(&mut h);
        let mut mid = V4 { x: (h.x + d.x) * 0.5, y: (h.y + d.y) * 0.5, z: (h.z + d.z) * 0.5, w: 0.0 };
        normalize(&mut mid);
        let a = angle(d, h);
        let u = &mut self.units[i];
        u.mid_heading = V4 { w: u.mid_heading.w, ..mid };
        let step = if (a as f64) <= 0.01 || parked {
            0.0
        } else {
            let mut axis = cross(h, d);
            if axis.y * axis.y + axis.x * axis.x + axis.z * axis.z <= 0.0 {
                axis.x = 1.0;
            }
            if 3.0 < a && (h.x as f64) < 0.1 {
                axis = V4::new(1.0, 0.0, 0.0);
            }
            normalize(&mut axis);
            if a * 0.25 < turn {
                turn = a * 0.25;
            }
            let s = if turn < a { turn } else { a };
            let m = axis_matrix(axis, s);
            let r = rotate(h, &m);
            u.heading = V4 { w: u.heading.w, ..r };
            s
        };
        u.turn = step;
        if (((len / 3.0) as f64 / 2.5) as f32) <= a {
            u.speed -= accel;
            if u.speed < 0.0 {
                u.speed = 0.0;
            }
        } else {
            if short {
                // Ease into the step: aim to stop on it.
                let sp = u.speed;
                let v = if sp == 0.0 {
                    if len == 0.0 { None } else { Some(accel * 0.5) }
                } else if len == 0.0 {
                    None
                } else {
                    Some(sp / ((len * 2.0) / sp))
                };
                accel = match v {
                    None => 0.0,
                    Some(v) if v != 0.0 => {
                        if accel <= v { -v } else { accel }
                    }
                    Some(v) => v,
                };
            }
            u.speed += accel;
            if top < u.speed {
                u.speed = top;
            }
        }
        let sp = u.speed;
        u.vel.z = sp * u.heading.z;
        u.vel.x = sp * u.heading.x;
        u.vel.y = sp * u.heading.y;
        false
    }

    /// The idle turn of `seturhajopos` (0x580540): a unit that stands still at its step turns to face its
    /// target, its nearest attacker or its pick, or levels out.
    fn face(&mut self, i: usize) {
        let u = &self.units[i];
        if !u.alive {
            return;
        }
        let gap = u.pos.sub(u.step).len2();
        if 0.010000000000000002 <= gap as f64 || u.speed != 0.0 || u.ship_type.turn == 0.0 {
            return;
        }
        let pick_unit = |j: usize| {
            let v = &self.units[j];
            (v.alive && !v.cloaked).then_some(v.pos)
        };
        let look = if let (order::ATTACK, target_kind::UNIT, Some(tg)) = (u.order, u.target_kind, u.target) {
            pick_unit(tg).or_else(|| u.attacker.and_then(pick_unit))
        } else {
            u.attacker.and_then(pick_unit)
        };
        let look = match look {
            Some(p) => Some(p),
            None if u.picked == -1 => {
                if (u.heading.x.abs() as f64) <= 0.01 {
                    None
                } else {
                    let mut h = u.heading;
                    h.x = 0.0;
                    if normalize(&mut h) == 0.0 {
                        Some(V4::new(0.0, 1.0, 0.0))
                    } else {
                        Some(V4::new(u.pos.x + h.x, u.pos.y + h.y, u.pos.z + h.z))
                    }
                }
            }
            None => pick_unit(u.picked as usize),
        };
        let Some(look) = look else { return };
        let mut h = u.heading;
        let mut d = look.sub(u.pos);
        d.w = 0.0;
        let len = d.len();
        if (len as f64) < 0.1 {
            d = h;
        }
        normalize(&mut d);
        normalize(&mut h);
        let mut mid = V4 { x: (h.x + d.x) * 0.5, y: (h.y + d.y) * 0.5, z: (h.z + d.z) * 0.5, w: 0.0 };
        normalize(&mut mid);
        let a = angle(d, h);
        let u = &mut self.units[i];
        u.mid_heading = V4 { w: u.mid_heading.w, ..mid };
        if 0.01 < a as f64 && (len as f64) >= 0.1 {
            let mut axis = cross(h, d);
            if axis.y * axis.y + axis.x * axis.x + axis.z * axis.z <= 0.0 {
                axis.x = 1.0;
            }
            normalize(&mut axis);
            let mut s = 0.1f32;
            if a * 0.25 < 0.1 {
                s = a * 0.25;
            }
            let s = if s < a { s } else { a };
            let m = axis_matrix(axis, s);
            let r = rotate(h, &m);
            u.heading = V4 { w: u.heading.w, ..r };
            normalize(&mut u.heading);
        }
    }

    /// `nextwaypoint` (0x57d200): head for the next queued point.
    fn next_waypoint(&mut self, i: usize) {
        // The exe passes the reached point again, with its x cell for both y and z (original bug).
        let w = self.units[i].waypoints[0];
        let p = V4::new(
            (w[2] - self.grid[2] / 2) as f32 * self.cell[2],
            (w[0] - self.grid[0] / 2) as f32 * self.cell[0],
            (w[0] - self.grid[0] / 2) as f32 * self.cell[0],
        );
        self.move_to(i, p);
        let u = &mut self.units[i];
        let n = u.waypoint_count;
        if n != 1 && -1 < n - 1 {
            for k in 0..(n - 1) as usize {
                u.waypoints[k] = u.waypoints[k + 1];
            }
        }
        u.waypoint_count -= 1;
    }

    /// `moveship(x, y, z, unit)` (0x57c4c0), outside the editor: a move order (ignored while disabled).
    fn move_to(&mut self, i: usize, p: V4) {
        if self.units[i].disabled != 0 {
            return;
        }
        let mut d = self.units[i].dest;
        d.x = p.x;
        d.y = p.y;
        d.z = p.z;
        let c = self.grid_of(d);
        let snapped = self.pos_of(c);
        let u = &mut self.units[i];
        u.waypoints[0][..3].copy_from_slice(&c);
        u.dest = V4 { w: d.w, ..snapped };
        u.step = u.dest;
        u.order = order::MOVE;
        u.f174 = -1.0;
        u.step_cell = u.waypoints[0];
        u.path_short = true;
        u.plan_pos = V4 { w: u.plan_pos.w, ..V4::default() };
        u.plan_tick = -1000;
        u.path_fails = 0;
    }

    // --------------------------------------------------------------------------------------------
    // Path search (`Utkeres` 0x6133a0)

    fn search_box(&mut self, mut lo: [i32; 3], mut hi: [i32; 3]) {
        // `TeglaRajzol` (0x613290): mark a box blocked.
        lo[0] = lo[0].clamp(0, 0x7f);
        hi[0] = hi[0].clamp(0, 0x7f);
        lo[1] = lo[1].clamp(0, 0x7f);
        hi[1] = hi[1].clamp(0, 0x7f);
        lo[2] = lo[2].clamp(0, 7);
        hi[2] = hi[2].clamp(0, 7);
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    self.search.grid[(x as usize * GRID_Y + y as usize) * GRID_Z + z as usize] = CELL_BLOCKED;
                }
            }
        }
    }

    fn heuristic(&self, x: i32, y: i32, z: i32) -> i32 {
        let g = self.search.goal;
        let ax = (x - g[0]).abs();
        let ay = (y - g[1]).abs();
        let az = (z - g[2]).abs();
        let lo = ay.min(ax);
        let hi = ay.max(ax);
        let extra = (az - hi).max(0);
        extra * 5 + (lo + az) * 2 + hi * 7
    }

    fn cell_index(x: i32, y: i32, z: i32) -> i64 {
        (x as i64 * GRID_Y as i64 + y as i64) * GRID_Z as i64 + z as i64
    }

    fn cell_at(&self, idx: i64) -> u16 {
        if (0..self.search.grid.len() as i64).contains(&idx) { self.search.grid[idx as usize] } else { 0 }
    }

    /// `CheckNyitott` (0x612b50): relax one neighbour.
    fn search_relax(&mut self, x: u16, y: u16, z: u16, g: u16, w: u16) {
        let (x, y, z) = (x as i32, y as i32, z as i32);
        if self.grid[0] - 1 < x || self.grid[1] - 1 < y || self.grid[2] - 1 < z {
            return;
        }
        let idx = Self::cell_index(x, y, z);
        let v = self.cell_at(idx);
        let cost = g.wrapping_add(w);
        if v == 0 {
            let h = self.heuristic(x, y, z);
            let score = h * 5 + cost as i32 * 2;
            if score < self.search.result_w || (!self.search.done && !self.search.widened) {
                self.search.result = [x, y, z];
                self.search.result_w = score;
            }
            self.search.done = true;
            return;
        }
        if self.search.done || v <= cost || 65000 < v {
            return;
        }
        let h = self.heuristic(x, y, z);
        let f = (h + cost as i32) as u16;
        if self.search.max_cost < f as i32 || self.search.max_heu < h {
            return;
        }
        let mut m = (h * 2 + 0x23).min(self.search.max_heu);
        if m < 0x8c {
            m = 0x8c;
        }
        self.search.max_heu = m;
        let score = h * 5 + cost as i32 * 2;
        if score < self.search.result_w {
            self.search.result = [x, y, z];
            self.search.result_w = score;
        }
        let n = self.search.open_len;
        let mut slot = n + 1;
        if v < 65000 {
            if let Some(k) = (1..=n).find(|&k| {
                let e = self.search.open[k as usize];
                e[0] as i32 == x && e[1] as i32 == y && e[2] as i32 == z
            }) {
                slot = k;
            }
        }
        if n < slot {
            if 999 < self.search.open_len {
                self.search.open_len -= 1;
            }
            self.search.open_len += 1;
            slot = self.search.open_len;
            while 1 < slot && f < self.search.open[(slot >> 1) as usize][3] {
                self.search.open[slot as usize] = self.search.open[(slot >> 1) as usize];
                slot >>= 1;
            }
        }
        self.search.open[slot as usize] = [x as u16, y as u16, z as u16, f];
        self.search.grid[idx as usize] = cost;
    }

    /// `KifejtBestNyitott` (0x612e50): pop the best open cell and relax its 26 neighbours.
    fn search_expand(&mut self) {
        if self.search.open_len < 1 {
            self.search.done = true;
            return;
        }
        if self.search.done {
            return;
        }
        let [x, y, z, _] = self.search.open[1];
        let g = self.cell_at(Self::cell_index(x as i32, y as i32, z as i32));
        let n = self.search.open_len as usize;
        self.search.open[1] = self.search.open[n];
        self.search.open_len -= 1;
        let len = self.search.open_len;
        let mut c = 1i32;
        loop {
            let (l, r) = (c * 2, c * 2 + 1);
            let mut m = c;
            if l <= len && self.search.open[l as usize][3] < self.search.open[c as usize][3] {
                m = l;
            }
            if r <= len && self.search.open[r as usize][3] < self.search.open[m as usize][3] {
                m = r;
            }
            if m == c {
                break;
            }
            self.search.open.swap(c as usize, m as usize);
            c = m;
        }
        let (xm, xp, ym, yp, zm, zp) =
            (x.wrapping_sub(1), x + 1, y.wrapping_sub(1), y + 1, z.wrapping_sub(1), z + 1);
        for (a, b, cc, w) in [
            (xm, ym, zm, 8), (x, ym, zm, 7), (xp, ym, zm, 8), (xm, y, zm, 7), (xp, y, zm, 7), (xm, yp, zm, 8),
            (x, yp, zm, 7), (xp, yp, zm, 8), (xm, ym, z, 6), (x, ym, z, 5), (xp, ym, z, 6), (xm, y, z, 5),
            (xp, y, z, 5), (xm, yp, z, 6), (x, yp, z, 5), (xp, yp, z, 6), (xm, ym, zp, 8), (x, ym, zp, 7),
            (xp, ym, zp, 8), (xm, y, zp, 7), (xp, y, zp, 7), (xm, yp, zp, 8), (x, yp, zp, 7), (xp, yp, zp, 8),
            (x, y, zm, 5), (x, y, zp, 5),
        ] {
            self.search_relax(a, b, cc, g, w);
        }
    }

    /// `CheckLine` (0x612a10): no blocked cell on the straight line.
    fn search_line(&self, a: [i32; 3], b: [i32; 3]) -> bool {
        let dx = (a[0] - b[0]).abs();
        let dy = (a[1] - b[1]).abs();
        let dz = (a[2] - b[2]).abs();
        let mut n = dy.max(dx);
        if dz < n {
            n = dy.max(dx);
        } else {
            n = dz;
        }
        let step = (1.0f64 / ((n + 1) as f32) as f64) as f32;
        let mut f = step;
        while (f as f64) <= 1.001 {
            let x = trunc64(((b[0] - a[0]) as f32 * f + a[0] as f32) as f64 + 0.5);
            let y = trunc64(((b[1] - a[1]) as f32 * f + a[1] as f32) as f64 + 0.5);
            let z = trunc64(((b[2] - a[2]) as f32 * f + a[2] as f32) as f64 + 0.5);
            if self.cell_at(Self::cell_index(x, y, z)) == CELL_BLOCKED {
                return false;
            }
            f += step;
        }
        true
    }

    /// `Utkeres(from, to, radius, unit)`: the next step towards `to` (stopping within `radius` cells),
    /// around the other units. Result in `self.search`.
    fn path_search(&mut self, from: [i32; 3], to: [i32; 3], mut radius: i32, unit: usize) {
        let from = [from[0].clamp(0, 0x7f), from[1].clamp(0, 0x7f), from[2].clamp(0, 7)];
        let to = [to[0].clamp(0, 0x7f), to[1].clamp(0, 0x7f), to[2].clamp(0, 7)];
        if radius < 1 {
            radius = 1;
        }
        if self.search.grid.len() != GRID_CELLS + 4 {
            self.search.grid = vec![0; GRID_CELLS + 4];
        }
        if self.search.open.len() != OPEN_MAX + 2 {
            self.search.open = vec![[0; 4]; OPEN_MAX + 2];
        }
        self.search.result_w = 500000;
        let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        if d[2] * d[2] + d[1] * d[1] + d[0] * d[0] < radius * radius {
            self.search.goal = to;
            self.search.result = from;
            self.search.arrived = true;
            return;
        }
        self.search.open_len = 0;
        self.search.goal = to;
        self.search.result = from;
        self.search.grid[..GRID_CELLS].fill(CELL_FREE);
        self.search.open_len += 1;
        self.search.open[1] = [from[0] as u16, from[1] as u16, from[2] as u16, 50000];
        let gi = Self::cell_index(to[0], to[1], to[2]) as usize;
        self.search.grid[gi] = 0;
        let si = Self::cell_index(from[0], from[1], from[2]) as usize;
        self.search.grid[si] = 1;
        if self.search.grid[gi] == 1 {
            self.search.arrived = true;
            return;
        }
        let h = {
            let ax = (from[0] - to[0]).abs();
            let ay = (from[1] - to[1]).abs();
            let az = (from[2] - to[2]).abs();
            let lo = ay.min(ax);
            let hi = ay.max(ax);
            (az - hi).max(0) * 5 + (lo + az) * 2 + hi * 7
        };
        self.search.max_cost = (h * 2 + 0x46).max(0x70);
        self.search.max_heu = (h * 2 + 0x23).max(0x8c);
        self.search.result_w = h * 5 + 2;
        // Other units are obstacles (own-side cloaked ones too, hostile cloaked ones not).
        let me = &self.units[unit];
        let (mx, mz, owner) = (me.ship_type.size_xy, me.ship_type.size_z, me.owner);
        for j in 0..self.units.len() {
            let v = &self.units[j];
            if !v.alive || j == unit || !(v.owner == owner || !v.cloaked) {
                continue;
            }
            let sxy = v.ship_type.size_xy + mx - 2;
            let sz = v.ship_type.size_z + mz - 2;
            let c = if v.ship_type.size_xy < 2 { v.step_cell } else { v.cell };
            let mut lo = [c[0] - sxy, c[1] - sxy, c[2] - sz];
            let mut hi = [c[0] + sxy, c[1] + sxy, c[2] + sz];
            if lo[0] <= from[0] && from[0] <= hi[0] && lo[1] <= from[1] && from[1] <= hi[1] && lo[2] <= from[2]
                && from[2] <= hi[2]
            {
                // The searcher stands inside the box: open the side it is on.
                let ey = (c[1] - from[1]).abs();
                let ez = (c[2] - from[2]).abs();
                let m = ez.max(ey);
                let ex = (c[0] - from[0]).abs();
                let m = if ex <= m { ez.max(ey) } else { ex };
                if m == c[0] - from[0] {
                    lo[0] = from[0] + 1;
                }
                if m == from[0] - c[0] {
                    hi[0] = from[0] - 1;
                }
                if m == c[1] - from[1] {
                    lo[1] = from[1] + 1;
                }
                if m == from[1] - c[1] {
                    hi[1] = from[1] - 1;
                }
                if m == c[2] - from[2] {
                    lo[2] = from[2] + 1;
                }
                if m == from[2] - c[2] {
                    hi[2] = from[2] - 1;
                }
            }
            self.search_box(lo, hi);
        }
        if self.search.grid[gi] == CELL_FREE {
            self.search.grid[gi] = 0;
        }
        self.search.widened = false;
        self.search.grid[si] = 1;
        // The goal sphere: grow it until it holds a free cell.
        loop {
            let mut free = (self.search.grid[gi] == 0) as i32;
            for x in (to[0] - radius).max(0)..=(to[0] + radius).min(self.grid[0] - 1) {
                for y in (to[1] - radius).max(0)..=(to[1] + radius).min(self.grid[1] - 1) {
                    for z in (to[2] - radius).max(0)..=(to[2] + radius).min(self.grid[2] - 1) {
                        let (a, b, c) = (to[0] - x, to[1] - y, to[2] - z);
                        if b * b + c * c + a * a < radius * radius {
                            let idx = Self::cell_index(x, y, z);
                            let v = self.cell_at(idx);
                            if v == 1 {
                                self.search.arrived = true;
                                return;
                            }
                            if v == CELL_FREE {
                                free += 1;
                                self.search.grid[idx as usize] = 0;
                            }
                        }
                    }
                }
            }
            if free != 0 {
                break;
            }
            radius += 1;
            self.search.widened = true;
        }
        self.search.done = false;
        while !self.search.done {
            self.search_expand();
        }
        let r = self.search.result;
        let dr = sqrt32(
            (r[1] - to[1]) as f32 * (r[1] - to[1]) as f32
                + (r[0] - to[0]) as f32 * (r[0] - to[0]) as f32
                + (r[2] - to[2]) as f32 * (r[2] - to[2]) as f32,
        );
        let ds = sqrt32(
            (from[1] - to[1]) as f32 * (from[1] - to[1]) as f32
                + (from[0] - to[0]) as f32 * (from[0] - to[0]) as f32
                + (from[2] - to[2]) as f32 * (from[2] - to[2]) as f32,
        );
        if ds <= dr {
            self.search.result = from;
        }
        // Walk back down the costs to the start.
        let mut path: Vec<[i32; 3]> = Vec::new();
        let mut moves = 0;
        loop {
            let [x, y, z] = self.search.result;
            path.push([x, y, z]);
            let base = Self::cell_index(x, y, z);
            let mut best = self.cell_at(base) as u32;
            if best == 0 {
                best = 64000;
            }
            let mut next = [x, y, z];
            for (dx, dy, dz) in BACKTRACK {
                let v = self.cell_at(base + (dx * (GRID_Y * GRID_Z) as i64) + dy * GRID_Z as i64 + dz) as u32;
                if v < best && v != 0 {
                    best = v;
                    next = [x + dx as i32, y + dy as i32, z + dz as i32];
                }
            }
            if next != [x, y, z] {
                moves += 1;
            }
            if best == 1 || next == [x, y, z] {
                self.search.one_short = moves == 1;
                self.search.arrived = moves == 0;
                break;
            }
            self.search.result = next;
        }
        // Cut corners: the earliest step the start can see. The exe keeps the path as u16 and reads it back
        // zero-extended, so a step that wrapped below z = 0 is checked (and taken) as z + 65536.
        let mut pick = [0i32; 3];
        for &c in path.iter().rev() {
            let c = [c[0] as u16 as i32, c[1] as u16 as i32, c[2] as u16 as i32];
            if self.search_line(from, c) {
                pick = c;
            }
        }
        if 0 < pick[0] {
            self.search.result = pick;
        }
        if self.search.result == from {
            self.search.arrived = true;
        }
    }
}

/// The backtrack's neighbour order (`Utkeres`, exe 0x613b00..0x614040): (dx, dy, dz).
const BACKTRACK: [(i64, i64, i64); 26] = [
    (-1, -1, 0), (0, -1, 0), (1, -1, 0), (-1, 0, 0), (1, 0, 0), (-1, 1, 0), (0, 1, 0), (1, 1, 0),
    (-1, -1, -1), (0, -1, -1), (1, -1, -1), (-1, 0, -1), (1, 0, -1), (-1, 1, -1), (0, 1, -1), (1, 1, -1),
    (-1, -1, 1), (0, -1, 1), (1, -1, 1), (-1, 0, 1), (1, 0, 1), (-1, 1, 1), (0, 1, 1), (1, 1, 1),
    (0, 0, -1), (0, 0, 1),
];

impl Battle {
    // --------------------------------------------------------------------------------------------
    // Losses, retreat, the end

    /// `urhajomeghalas` (0x582550): scenery spins, retreating units may blow up or leave the arena, and
    /// the explosion animation counter of dead units runs.
    fn unit_deaths(&mut self, t: &mut Tick) {
        for i in 0..self.units.len() {
            if self.units[i].alive {
                if self.units[i].owner == 0 {
                    let x = (i as f64 * 1.23) / 0.015;
                    let u = &mut self.units[i];
                    u.spin[0] = ((x - x.floor()) * 0.015 + 0.005 + u.spin[0] as f64) as f32;
                    self.face(i);
                }
                if self.units[i].alive && self.units[i].retreating {
                    self.units[i].retreat_ticks += 1;
                    if self.units[i].retreat_explodes && 3.0 < self.units[i].speed {
                        self.explode(t, i, 0, true, true);
                    }
                    let d = dist(self.units[i].pos, self.retreat_point);
                    if self.width < d {
                        let u = &mut self.units[i];
                        u.alive = false;
                        u.lightning = None;
                        self.end_timer = self.end_period;
                        self.recount_present();
                    }
                }
            }
            let u = &mut self.units[i];
            if !u.alive && 0 < u.exploding {
                u.exploding += 1;
                if !u.retreating && 0x2d < u.exploding {
                    u.exploding = 0;
                }
            }
        }
    }

    /// `vadaszmeghalas` (0x582fb0): retreating fighters may be lost on the way, or leave.
    fn fighter_deaths(&mut self, t: &mut Tick) {
        for i in 0..self.fighters.len() {
            if !(self.fighters[i].alive && self.fighters[i].retreating) {
                continue;
            }
            self.fighters[i].retreat_ticks += 1;
            if self.fighters[i].doomed && roll(t.rng, 0x14, 0x104) < self.fighters[i].retreat_ticks
                && roll(t.rng, 0, 5) < 1 {
                let n = self.fighters[i].count;
                self.lose_fighters(t, i, n, true);
            }
            if self.width < dist(self.fighters[i].pos, self.retreat_point) {
                let f = &mut self.fighters[i];
                f.count = 0;
                f.alive = false;
                f.lightning = None;
                self.recount_present();
            }
        }
    }

    /// `retreat(empire)` (0x57ee20): the empire's mobile units and fighters turn tail; its fleets back
    /// off on the starmap.
    fn retreat(&mut self, t: &mut Tick, e: i32) {
        if (e == self.human && t.rules.story_no_retreat) || !self.sides[e as usize].may_retreat {
            return;
        }
        for i in 0..self.units.len() {
            let u = &self.units[i];
            if u.alive && u.original_owner == e && u.disabled == 0 && !u.retreating && u.taken == 0
                && u.ship_type.top_speed != 0.0 {
                self.units[i].retreating = true;
                self.units[i].retreat_explodes = roll(t.rng, 0, 2) < 1;
                self.flag_retreat(e);
            }
        }
        for i in 0..self.fighters.len() {
            let f = &self.fighters[i];
            if f.alive && f.owner == e && !f.retreating {
                let mut d = f.pos.sub(self.retreat_point);
                if d.len() == 0.0 {
                    d = V4::new(1.0, 0.0, 0.0);
                }
                normalize(&mut d);
                let p = f.pos;
                let f = &mut self.fighters[i];
                f.target = None;
                f.target_kind = 0;
                f.dest.x = d.x * 600.0 + p.x;
                f.state = 3;
                f.retreating = true;
                f.dest.y = d.y * 600.0 + p.y;
                f.dest.z = d.z * 600.0 + p.z;
                let r = roll(t.rng, 0, 2);
                f.retreat_ticks = 0;
                f.doomed = r < 1;
                self.flag_retreat(e);
            }
        }
        if self.human_retreated || self.ai_retreated {
            for p in &mut self.participants {
                if t.world.fleet_owner(p.fleet) == e && p.status != 2 {
                    t.world.retreat_fleet(p.fleet);
                    p.status = 2;
                }
            }
        }
        t.events.push(BattleEvent::Retreat { empire: e });
    }

    fn flag_retreat(&mut self, e: i32) {
        if e == self.human {
            self.human_retreated = true;
        } else {
            self.ai_retreated = true;
        }
    }

    /// `vegevizsgal` (0x5833d0), every `end_period` ticks: over when no two present, mutually hostile
    /// empires can still hurt each other (ships/fighters vs ships/fighters, guns vs ships). Units count for
    /// their original owner. On screen, a won planet battle with ground units waiting first flies every
    /// unit to the planet.
    pub fn check_end(&mut self, rules: &BattleRules) -> bool {
        if !self.victory {
            self.end_timer -= 1;
            if -1 < self.end_timer {
                return false;
            }
            self.end_timer = self.end_period;
        }
        if 0.0 < rules.fade && rules.fade < 1.0 {
            return false;
        }
        let mut count = [0i32; EMPIRES];
        let mut has = [0u8; EMPIRES];
        for u in &self.units {
            if u.alive && u.owner != 0 {
                count[u.original_owner as usize] += 1;
                has[u.original_owner as usize] |= 1;
            }
        }
        for f in &self.fighters {
            if f.alive && f.owner != 0 {
                count[f.owner as usize] += 1;
                has[f.owner as usize] |= 2;
            }
        }
        for g in &self.guns {
            if g.building != 0 {
                count[self.planet_owner as usize] += 1;
                has[self.planet_owner as usize] |= 4;
            }
        }
        self.counts = count;
        let present: Vec<usize> = (1..EMPIRES).filter(|&e| 0 < count[e]).collect();
        for a in 1..EMPIRES {
            if count[a] == 0 {
                continue;
            }
            for b in 1..EMPIRES {
                if a != b && count[b] != 0 && self.hostile[a][b] == 1 {
                    let (x, y) = (has[a], has[b]);
                    if (x & 3 != 0 && y & 3 != 0) || (x & 4 != 0 && y & 1 != 0) || (y & 4 != 0 && x & 1 != 0) {
                        return false;
                    }
                }
            }
        }
        if self.shown {
            if present.is_empty() {
                self.winner = 0;
                return true;
            }
            if -1 < self.planet_owner && self.hostile(present[0] as i32, self.planet_owner) != 0 && self.ground_war {
                self.victory = true;
                let p = self.planet_pos;
                for u in &mut self.units {
                    if u.alive {
                        u.dest.x = p[0] as f32;
                        u.dest.y = p[1] as f32;
                        u.order = order::MOVE;
                        u.f174 = -1.0;
                        u.path_short = false;
                        u.dest.z = p[2] as f32;
                        u.step = u.dest;
                        u.step_cell = u.cell;
                    }
                }
            }
        }
        self.winner = present.first().map_or(0, |&e| e as i32);
        true
    }

    // --------------------------------------------------------------------------------------------
    // The battle AI (`SpaceWarMI` 0x56f3c0), every 10 ticks for each AI-played empire

    fn ai(&mut self, e: usize, t: &mut Tick) {
        self.ai_empire = e;
        if self.sides[e].ai_turns == 0 {
            self.form_squads(e, 2);
            self.sides[e].ai_turns += 1;
        } else {
            self.ai_retreat_check(t);
            self.ai_attack(t);
        }
    }

    fn centroid(&self, list: &[usize]) -> V4 {
        let (mut x, mut y, mut z) = (0.0f32, 0.0f32, 0.0f32);
        for &i in list {
            x += self.units[i].pos.x;
            y += self.units[i].pos.y;
            z += self.units[i].pos.z;
        }
        let n = list.len() as f32;
        V4::new(x / n, y / n, z / n)
    }

    /// `MIcsapatokraoszt2(empire, k)` (0x56acd0): split the empire's units into squads of
    /// `(n − 1)/k + 1`, each grown greedily from the unit farthest from the centre.
    fn form_squads(&mut self, e: usize, k: i32) {
        self.sides[e].squads.clear();
        let mut list: Vec<usize> = (0..self.units.len()).filter(|&i| self.units[i].alive && self.units[i].owner == e as i32).collect();
        if list.is_empty() {
            return;
        }
        let per = (list.len() as i32 - 1) / k;
        let c = self.centroid(&list);
        let far = self.extreme(&list, c, false);
        let mut p = self.units[list[far]].pos;
        let mut filled = 0;
        while !list.is_empty() {
            if filled == 0 {
                self.sides[e].squads.push(Squad::default());
            }
            let j = self.extreme(&list, p, true);
            let s = self.sides[e].squads.last_mut().unwrap();
            s.units.push(list[j]);
            filled += 1;
            if per < filled {
                filled = 0;
            }
            let members = s.units.clone();
            let m = self.centroid(&members);
            p = V4 { w: p.w, ..m };
            list.remove(j);
        }
    }

    /// The nearest (`MIcsapatlegkozelebbi` 0x56ab70) or farthest (0x56ac20) of `list` from `p`, by the
    /// truncated squared distance (first wins ties).
    fn extreme(&self, list: &[usize], p: V4, nearest: bool) -> usize {
        let mut best = 0;
        let mut v = trunc(dist2(p, self.units[list[0]].pos));
        for (k, &i) in list.iter().enumerate().skip(1) {
            let d = trunc(dist2(p, self.units[i].pos));
            if (nearest && d < v) || (!nearest && v < d) {
                v = d;
                best = k;
            }
        }
        best
    }

    /// `MIvisszavonulasvizsgalat` (0x56b3b0).
    fn ai_retreat_check(&mut self, t: &mut Tick) {
        let e = self.ai_empire;
        let r = t.rules;
        if !(r.multiplayer
            || ((e as i32 == self.human || !r.story_no_retreat_check) && !r.story_retreat_lock_a && !r.story_retreat_lock_b))
        {
            return;
        }
        let n = self.units.iter().filter(|u| u.alive && u.owner == e as i32).count();
        if n <= 2 {
            return;
        }
        let enemy: i32 = (0..EMPIRES).filter(|&k| self.hostile[e][k] == 1).map(|k| self.sides[k].strength).fold(0, i32::wrapping_add);
        let s = &self.sides[e];
        if (s.strength as f64) < s.lost as f64 * 0.6 && s.strength.wrapping_mul(2) < enemy {
            self.retreat(t, e as i32);
        }
        for u in &mut self.units {
            if u.alive && u.owner == e as i32 {
                u.weapon_mask = u32::MAX;
            }
        }
    }

    /// `MIellensegmegtamad` (0x56af60): gathering squads spread out around the common centre, then each
    /// idle unit attacks the enemy with the best `max(distance, shortest range) + own damage %`.
    fn ai_attack(&mut self, t: &mut Tick) {
        let _ = &t;
        let e = self.ai_empire;
        let mut total = V4::default();
        let mut m = 0;
        for g in 0..self.sides[e].squads.len() {
            if self.sides[e].squads[g].units.is_empty() {
                continue;
            }
            let alive: Vec<usize> = self.sides[e].squads[g].units.iter().copied().filter(|&i| self.units[i].alive).collect();
            let c = if alive.is_empty() { V4::default() } else { self.centroid(&alive) };
            let s = &mut self.sides[e].squads[g];
            s.centre = V4 { w: s.centre.w, ..c };
            total.x += c.x;
            total.y += c.y;
            total.z += c.z;
            m += 1;
        }
        if m != 0 {
            let n = m as f32;
            total = V4::new(total.x / n, total.y / n, total.z / n);
        }
        for g in 0..self.sides[e].squads.len() {
            let s = self.sides[e].squads[g].clone();
            if s.units.is_empty() {
                continue;
            }
            if s.state == 0 {
                for &u in &s.units {
                    let mut d = s.centre.sub(total);
                    let d2 = dist2(total, s.centre);
                    d = d.scale(2.0);
                    if d2 == 0.0 {
                        d = V4::default();
                    } else if 9.0 <= d2 {
                        if 100.0 < d2 {
                            normalize(&mut d);
                            d = d.scale(10.0);
                        }
                    } else {
                        normalize(&mut d);
                        d = d.scale(3.0);
                    }
                    let mut dest = V4::new(s.centre.x + d.x, s.centre.y + d.y, d.z + s.centre.z);
                    if self.units[u].alive && self.units[u].order == order::NONE {
                        self.clamp_inside(&mut dest);
                        self.move_to(u, dest);
                    }
                }
                self.sides[e].squads[g].state = 1;
            } else if s.state == 1 {
                for &u in &s.units {
                    if !(self.units[u].alive && self.units[u].order == order::NONE) {
                        continue;
                    }
                    let enemies: Vec<usize> = (0..self.units.len())
                        .filter(|&j| {
                            let v = &self.units[j];
                            v.owner != 0 && !v.cloaked && v.alive && self.hostile[e][v.owner as usize] == 1
                        })
                        .collect();
                    if let Some(j) = self.ai_pick(u, &enemies) {
                        self.attack_order(u, enemies[j]);
                    }
                }
            }
        }
    }

    /// 0x578750: the attacker's own damage % is added to every candidate (original quirk).
    fn ai_pick(&self, u: usize, list: &[usize]) -> Option<usize> {
        let a = &self.units[u];
        if list.is_empty() {
            return None;
        }
        let hurt = if a.hp == 0 { 1 } else { a.damage.wrapping_mul(100) / a.hp };
        let score = |j: usize| {
            let mut d = trunc(dist(a.pos, self.units[j].pos));
            if (d as f32) < a.range_min {
                d = trunc(a.range_min);
            }
            d
        };
        let mut best = score(list[0]) + hurt;
        let mut k = 0;
        for (i, &j) in list.iter().enumerate().skip(1) {
            let v = score(j);
            if v + hurt < best {
                best = v + hurt;
                k = i;
            }
        }
        Some(k)
    }

    /// 0x571a50: an attack order on unit `target` (ignored while disabled or unarmed).
    fn attack_order(&mut self, u: usize, target: usize) {
        if self.units[u].disabled != 0 || self.units[u].range_min == 0.0 {
            return;
        }
        let mut d = self.units[target].pos;
        self.clamp_inside(&mut d);
        let c = self.grid_of(d);
        let p = self.pos_of(c);
        let x = &mut self.units[u];
        x.waypoints[0][..3].copy_from_slice(&c);
        x.dest = V4 { w: d.w, ..p };
        x.f174 = -1.0;
        x.target = Some(target);
        x.target_kind = target_kind::UNIT;
        x.order = order::ATTACK;
        x.path_short = true;
        x.plan_pos = V4 { w: x.plan_pos.w, ..V4::default() };
        x.plan_tick = -1000;
    }
}

// ------------------------------------------------------------------------------------------------
// The human's orders (spec §7.12): `MULTIGET_SpaceWar` (exe 0x5615b0, ELF 0x67ef60)

/// An order from the battle screen (`MULTIGET_SpaceWar`'s messages; the battle slot travels with it). Indices are
/// the battle's own (units, guns, mounts); the handlers trust them, so callers check them first
/// ([`Battle::order_valid`]).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Order {
    /// 0x70, `moveship(x, y, z, unit)` (0x57c4c0) outside the editor: move to the point (x = height).
    Move { unit: usize, x: f32, y: f32, z: f32 },
    /// 0x71, `attack(unit, target)` (0x571a50): attack another unit.
    Attack { unit: usize, target: usize },
    /// 0x72, `retreat(issuer)` (0x57ee20): the issuer's side retreats.
    Retreat,
    /// 0x73, `shipattackscolonygun(unit, gun)` (0x5817a0): attack a planet gun.
    AttackGun { unit: usize, gun: usize },
    /// 0x74, `colonygunattacksship(gun, unit)` (0x5720d0): a planet gun's forced target.
    GunTarget { gun: usize, unit: usize },
    /// 0x75 (cheat console): every unit of the issuer in every battle gets one barrel of weapons 0..20. Handled
    /// per battle by [`Battle::arm_all`].
    ArmAll,
    /// 0x76 (cheat console): every ship of the unit blows up (`felrobban(unit, 0, 0, 1, 1)` × its ship count).
    Explode { unit: usize },
    /// 0x77 (`infoszamol`'s fighter stance button): side `side`'s fighter aggression (`+0x6c·side + 0x40`).
    FighterStance { side: usize, value: i32 },
    /// 0x44e, `DirectFire(unit, target, mount)` (0x566ba0): fire one mount at a unit now.
    DirectFire { unit: usize, target: usize, mount: usize },
    /// 0x44f, `FegyverKibeKapcsol(unit, keep, set)` (0x566e80): weapon mask `+0x1bc := (mask & keep) | set`.
    Weapons { unit: usize, keep: u32, set: u32 },
}

impl Battle {
    /// A move target as the battle screen sends it: `JobbClick` (ELF 0x6b0210) clamps the point two cells inside the
    /// arena (y, then z, then the height x) before message 0x70; the handler itself doesn't.
    pub fn screen_point(&self, x: f32, y: f32, z: f32) -> (f32, f32, f32) {
        let mut p = V4 { x, y, z, w: 0.0 };
        self.clamp_inside(&mut p);
        (p.x, p.y, p.z)
    }

    /// Whether every index the order names exists in this battle (the exe reads whatever is there).
    pub fn order_valid(&self, o: &Order) -> bool {
        let unit = |u: usize| u < self.units.len();
        let gun = |g: usize| g < self.guns.len();
        match *o {
            Order::Move { unit: u, .. } | Order::Explode { unit: u } | Order::Weapons { unit: u, .. } => unit(u),
            Order::Attack { unit: u, target } => unit(u) && unit(target),
            Order::AttackGun { unit: u, gun: g } | Order::GunTarget { gun: g, unit: u } => unit(u) && gun(g),
            Order::DirectFire { unit: u, target, mount } => unit(u) && unit(target) && mount < self.units[u].mounts.len(),
            Order::FighterStance { side, .. } => side < self.sides.len(),
            Order::Retreat | Order::ArmAll => true,
        }
    }

    /// Carry out an order of empire `issuer` (spec §7.12). [`Order::ArmAll`] is the caller's ([`Battle::arm_all`]
    /// on every active battle).
    pub fn order(&mut self, t: &mut Tick, issuer: i32, o: Order) {
        match o {
            Order::Move { unit, x, y, z } => self.move_to(unit, V4 { x, y, z, w: 0.0 }),
            Order::Attack { unit, target } => self.attack_order(unit, target),
            Order::Retreat => self.retreat(t, issuer),
            Order::AttackGun { unit, gun } => {
                let u = &mut self.units[unit];
                if u.range_min != 0.0 {
                    u.target = Some(gun);
                    u.target_kind = target_kind::GUN;
                    u.order = order::ATTACK;
                }
            }
            Order::GunTarget { gun, unit } => self.guns[gun].target = unit as i32,
            Order::ArmAll => {}
            Order::Explode { unit } => {
                for _ in 0..self.units[unit].ship_count.max(0) {
                    self.explode(t, unit, 0, true, true);
                }
            }
            Order::FighterStance { side, value } => self.sides[side].fighter_aggression = value,
            Order::DirectFire { unit, target, mount } => {
                if !self.units[unit].alive {
                    return;
                }
                // `rndlog(6, 2r/3, r − 1)` (spwc.cpp line 0x3702) into the mount's timer and its set value.
                let r = reload_roll(t.rng, self.units[unit].mounts[mount].reload);
                let m = &mut self.units[unit].mounts[mount];
                m.timer = r;
                m.timer_set = r;
                let (weapon, count) = (m.weapon, m.count);
                let (owner, pos) = (self.units[unit].owner, self.units[unit].pos);
                self.fire(t, owner, pos, shot_kind::SHIP, unit, target, weapon, count);
            }
            Order::Weapons { unit, keep, set } => {
                let u = &mut self.units[unit];
                if u.alive {
                    u.weapon_mask = (u.weapon_mask & keep) | set;
                }
            }
        }
    }

    /// The cheat order 0x75 on this battle: every live unit of `owner` gets one barrel of each weapon 0..20
    /// (`fegyverhozzaad(unit, w, 1, ships[0])`, a new mount drawing its reload on stream 6).
    pub fn arm_all(&mut self, weapons: &[WeaponRules], rng: &mut Streams, owner: i32) {
        if !self.active {
            return;
        }
        for u in 0..self.units.len() {
            if self.units[u].alive && self.units[u].owner == owner {
                for w in 0..20 {
                    self.add_weapon(weapons, rng, u, w, 1);
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Setup (spec §7.10): `SpaceWar_NewBattle` (exe 0x563700), `InitSpaceWar` (0x5684e0), `AddFleet` (0x5646b0)

/// Ship-type records per race (`tipusnevek`, exe 0x91F488): record `race·12 + type`.
pub const TYPES_PER_RACE: i32 = 12;
/// From this many units on, `AddUjUrhajo` only reuses dead ones (the array holds 120).
const UNIT_REUSE_FROM: usize = 0x77;
const MAX_GUNS: usize = 50;
/// Where the scenery designs start (`0x8000 + t`: asteroids of type t).
pub const SCENERY: i32 = 0x8000;
/// `felallas0.bin` .. `felallas9.bin`: the set-up's two and the formation phase's nine (`felallas1` both).
pub const FORMATION_FILES: usize = 10;

/// A formation slot (`felallastipus` record, 0x54 bytes; `dataraw/spacewar/felallas<n>.bin`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FormationSlot {
    /// Grid cell (x, y, layer) and the slot class `w` (−1 once taken or blocked).
    pub cell: [i32; 4],
    /// +0x10: waypoints the ship placed here gets, +0x14 their cells.
    pub waypoint_count: i32,
    pub waypoints: [[i32; 4]; 4],
}

impl FormationSlot {
    /// From a file record (`ig2_formats::formation::Slot`: cell, the u32 at +0x10, the 64 bytes at +0x14).
    pub fn from_record(cell: [i32; 4], waypoint_count: u32, waypoints: &[u8; 64]) -> Self {
        let w = |k: usize, j: usize| i32::from_le_bytes(waypoints[16 * k + 4 * j..16 * k + 4 * j + 4].try_into().unwrap());
        FormationSlot {
            cell,
            waypoint_count: waypoint_count as i32,
            waypoints: std::array::from_fn(|k| std::array::from_fn(|j| w(k, j))),
        }
    }
}

/// Rule values the setup reads.
#[derive(Debug, Clone, Default)]
pub struct SetupRules {
    /// The live weapon table (`fegyverszamol` leaves it as loaded, spec §2.2).
    pub weapons: Vec<WeaponRules>,
    pub forms: Vec<FormRules>,
    /// `felallas<n>` by file number: 0 for sides with fewer than six ships, 1 for the others, 1..=9 the human's
    /// picks in the formation phase ([`Battle::pick_formation`]).
    pub formations: Vec<Vec<FormationSlot>>,
    /// `EpuletekLeiro[type].+0`: the tech of each building type (exe 0x8EBA38 × 0x24).
    pub building_tech: Vec<u8>,
}

/// The parts of a fleet record (`Flottak`, 0x80 bytes) the setup reads.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SetupFleet {
    /// +0x15.
    pub owner: u8,
    /// +0x16.
    pub story_id: i16,
    /// `+0x6c + +0x70`: hit points of its ships and fighters.
    pub hp: i32,
    /// +0x30 / +0x34: starmap position.
    pub x: f32,
    pub y: f32,
}

/// A `Hajok` entry (6 bytes): fleet, design and experience.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetupShip {
    pub fleet: i16,
    pub design: i16,
    /// +4.
    pub experience: u8,
}

/// A building of the battle planet (`Epuletek[p]`, slot k = index + 1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetupBuilding {
    /// +0: the building type.
    pub kind: u8,
    /// +8 == 0: construction finished.
    pub finished: bool,
    /// +0x19 & 1: switched on.
    pub on: bool,
}

/// The battle planet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SetupPlanet {
    /// +0x20.
    pub owner: u8,
    /// `+0xfe + 2k` for k = 1..=`+0x114`: its defence ships' designs.
    pub ships: Vec<i16>,
    pub buildings: Vec<SetupBuilding>,
}

/// The strategic state `SpaceWar_NewBattle` reads.
pub struct SetupInput<'a> {
    /// `spFlottak` (exe 0xBAA1C6): the fleets to add (−1 = none; a join sets the ones already fighting to −1).
    pub fleets: Vec<i32>,
    /// `spKozeliBolygo` (0xBAA1E4).
    pub planet: i32,
    /// `spFoldiharcElottiUrharc` (0xB9C027): ground units wait for the battle.
    pub ground_war: bool,
    /// `EN`: the human's empire.
    pub human: i32,
    /// `PlayerKi[e] == 1`.
    pub human_controlled: [bool; EMPIRES],
    pub multiplayer: bool,
    /// `asteroidak` (exe 0xE72DD2).
    pub asteroids: bool,
    /// `StoryUrharcViszony` (0xF26150), `StoryUrharcZachkinBarkacs` (0xF2619C), `StoryUrharcVisszavonulhate`
    /// (0xF261A0): as `SpaceWarIndit` left them.
    pub story_pact: [i32; EMPIRES],
    pub story_zachkin: i32,
    pub story_retreat: [i32; EMPIRES],
    /// `Flottak` by index, `Hajok` and `Tankok` (1-based, index 0 unused), `Tervek[e]`.
    pub fleet_records: &'a [SetupFleet],
    pub ships: &'a [SetupShip],
    pub vehicles: &'a [crate::spacewar::Vehicle],
    pub designs: &'a [Vec<crate::spacewar::Design>],
    /// `Bolygok[planet]` (when `planet ≠ 0`).
    pub planet_record: Option<&'a SetupPlanet>,
    /// What the exe reads as `Hajok[−k].fleet` for defence ship k = 1..=8 (the heap bytes before the array:
    /// undefined). None: `−k`. Only a fidelity test passes the recorded bytes.
    pub defence_fleets: Option<[i32; 8]>,
}

/// A ship waiting for its unit (`AddFleet`'s per-side list, 0x1c bytes: race, design, owner, ship, colour, unit,
/// experience; the colour is the screen's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entry {
    pub race: i32,
    pub design: i32,
    pub owner: i32,
    /// `Hajok` index, −k for defence ship k of the planet.
    pub ship: i32,
    pub unit: i32,
    pub experience: i32,
}

/// Fighters of one design in one fleet (`AddFleet`'s temporary list, 0xa8 bytes).
#[derive(Debug, Clone, Copy)]
struct FighterGroup {
    design: i32,
    owner: i32,
    race: i32,
    class: i32,
    fleet: i32,
    count: i32,
}

/// The context of a setup: rules, the strategic input, the global ship-type table (`AddUjUrhajo` writes the
/// footprints into it), stream 6 and the loaded formation (`aktfelallas`).
struct Setup<'a, 'b> {
    rules: &'a SetupRules,
    inp: &'a SetupInput<'b>,
    types: &'a mut [ShipType],
    rng: &'a mut Streams,
    formation: Vec<FormationSlot>,
}

impl Setup<'_, '_> {
    fn design(&self, owner: i32, d: i32) -> Option<&crate::spacewar::Design> {
        self.inp.designs.get(owner as usize)?.get(d as usize)
    }

    /// `Hajok[ship].fleet`. A defence ship (`−k`) has no record: the exe reads the bytes before the array
    /// [undefined]; by default its own fleet is `−k`, so it only joins units holding the same ship.
    fn ship_fleet(&self, ship: i32) -> i32 {
        if ship < 0 {
            return match self.inp.defence_fleets {
                Some(f) if (-8..0).contains(&ship) => f[(-ship - 1) as usize],
                _ => ship,
            };
        }
        self.inp.ships.get(ship as usize).map_or(0, |s| s.fleet as i32)
    }

    fn type_record(&self, race: i32, t: i32) -> ShipType {
        let i = race * TYPES_PER_RACE + t;
        if 0 <= i { self.types.get(i as usize).copied().unwrap_or_default() } else { ShipType::default() }
    }

    fn scale(&self, race: i32, t: i32) -> f32 {
        self.type_record(race, t).scale
    }
}

/// `terv2tipus` (exe 0x582010): the ship type of a design from its hull tech (`+0x48`).
pub fn design_type(hull_tech: i32) -> i32 {
    let v = hull_tech;
    if (0x4d..=0x50).contains(&v) {
        return v - 0x45;
    }
    if v > 0x11 {
        return (0x11 - 5) / 2;
    }
    match v {
        0xc..=0xe => v - 8,
        _ => (v - 5) / 2,
    }
}

/// The footprint `AddUjUrhajo` writes into a ship-type record (f32): `((scale·1024)/cell + cell)·0.5 + 1` across,
/// `((scale·1024)/cell + cell_z)·0.5` up.
pub fn footprint(scale: f32, cell: f32, cell_z: f32) -> (i32, i32) {
    (trunc(((scale * 1024.0) / cell + cell) * 0.5 + 1.0), trunc(((scale * 1024.0) / cell + cell_z) * 0.5))
}

/// What a design slot's item does to a new unit's shield and pod flags (`AddUjUrhajo`): shields 0x31..=0x36
/// add 15/30/45/60/80/100 % of the ship's hit points (f64), the rest set pod flags.
pub fn pod_effect(id: u8, hp: i32, shield: i32, pods: u32) -> (i32, u32) {
    let bonus = |k: f64, flag: u32| (trunc64(hp as f64 * k + shield as f64), pods | flag);
    match id {
        0x31 => bonus(0.15, 1),
        0x32 => bonus(0.3, 2),
        0x33 => bonus(0.45, 4),
        0x34 => bonus(0.6, 8),
        0x35 => bonus(0.8, 0x10),
        0x36 => (trunc64(shield as f64 + hp as f64), pods | 0x800),
        0x25 => (shield, pods | pod::DAMAGE),
        0x27 => (shield, pods | pod::JAMMER),
        0x28 => (shield, pods | pod::DECOY),
        0x29 => (shield, pods | pod::DECOY2),
        0x2a => (shield, pods | pod::SCANNER),
        0x37 => (shield, pods | pod::CLOAK),
        _ => (shield, pods),
    }
}

/// `AddUjVadasz`'s split of `count` fighters into `groups` groups (the first `count mod groups` get one more).
pub fn group_sizes(count: i32, groups: i32) -> Vec<i32> {
    let base = if groups == 0 { 0 } else { count / groups };
    let (mut rest, mut r) = (count, base.wrapping_mul(groups));
    (0..groups.max(0))
        .map(|_| {
            let n = if r < rest { base + 1 } else { base };
            rest -= n;
            r -= base;
            n
        })
        .collect()
}

/// `tipus2felallashely` (exe 0x5820a0): the formation slot class of a ship type.
pub fn slot_class(t: i32) -> i32 {
    if t < 8 { t.min(3) } else { 11 }
}

/// `felallasforgat` (exe 0x572d30): turn the formation to face `dir` (x, y used).
pub fn rotate_formation(f: &mut [FormationSlot], dir: V4, grid: [i32; 3]) {
    let mut d = V4::new(dir.x, dir.y, 0.0);
    normalize(&mut d);
    let (hx, hy) = ((grid[0] / 2) as f32, (grid[1] / 2) as f32);
    let turn = |c: &mut [i32; 4]| {
        let fx = c[0] as f32 - hx;
        let fy = c[1] as f32 - hy;
        let x = fx * d.y + fy * d.x + hx;
        let y = (fy * d.y - fx * d.x) + hy;
        c[0] = trunc(x);
        c[1] = trunc(y);
    };
    for s in f {
        turn(&mut s.cell);
        for k in 0..(s.waypoint_count.max(0) as usize).min(4) {
            turn(&mut s.waypoints[k]);
        }
    }
}

impl Battle {
    /// A slot before its first battle (`SpwC::Init`): the fields `InitSpaceWar` never resets.
    pub fn slot() -> Battle {
        Battle { end_period: 100, planet_pos: [1200, 200, 1200], planet_size: 1, ..Default::default() }
    }

    /// `InitSpaceWar` (exe 0x5684e0): reset the slot for a new battle (stream 6: the asteroid draws).
    fn init_space_war(&mut self, asteroids_on: bool, rules: &SetupRules, rng: &mut Streams) {
        let mut b = Battle {
            // Not reset (spec §7.10): the slot's previous battle's values.
            ai_timer: self.ai_timer,
            aim_timer: self.aim_timer,
            winner: self.winner,
            end_period: self.end_period,
            planet_pos: self.planet_pos,
            planet_size: self.planet_size,
            story_retreat: self.story_retreat,
            ai_empire: self.ai_empire,
            search: std::mem::take(&mut self.search),
            cell: [3.0; 3],
            grid: [128, 128, 10],
            bounds: [-192.0, 192.0, -192.0, 192.0, -15.0, 15.0],
            width: 384.0,
            planet_owner: -1,
            pod_timer: 1,
            f2d84: true,
            ..Default::default()
        };
        b.sides = (0..EMPIRES)
            .map(|_| Side { rotation: [std::f32::consts::FRAC_PI_2, 0.0, -std::f32::consts::FRAC_PI_2], fighter_aggression: 50, ..Default::default() })
            .collect();
        b.sides[0].centre.z = b.cell[1] * -20.0;
        b.sides[1].centre.z = b.cell[1] * 20.0;
        // `Lovedekinit`: every pool empty (the exe's pools are global, the port's per battle).
        b.pools = rules.forms.iter().map(|f| Pool { shots: vec![Shot::default(); f.slots] }).collect();
        b.asteroids = if asteroids_on || roll(rng, 0, 1) != 0 {
            let a = roll(rng, 0, 2);
            let c = roll(rng, 0, 5);
            Some([a, c, roll(rng, 0, 7)])
        } else {
            None
        };
        *self = b;
    }

    /// `pos2racspos` for a new unit's cells.
    fn set_cells(&mut self, i: usize) {
        let (c, d, s) = (self.grid_of(self.units[i].pos), self.grid_of(self.units[i].dest), self.grid_of(self.units[i].step));
        let u = &mut self.units[i];
        u.cell[..3].copy_from_slice(&c);
        u.waypoints[0][..3].copy_from_slice(&d);
        u.step_cell[..3].copy_from_slice(&s);
    }

    /// `helykeres` (exe 0x573cd0): the formation slot for a ship of type `t`; it closes the slots around it.
    fn find_slot(&self, s: &mut Setup, t: i32, race: i32) -> i32 {
        let c = slot_class(t);
        let mut best_w = 12;
        let mut found = None;
        for (k, slot) in s.formation.iter().enumerate() {
            let w = slot.cell[3];
            let take = if c < 8 { w < 11 && c <= w && w < best_w } else { w == 11 && w < best_w };
            if take {
                best_w = w;
                found = Some(k);
            }
        }
        let Some(k) = found else { return -1 };
        let at = self.pos_of([s.formation[k].cell[0], s.formation[k].cell[1], s.formation[k].cell[2]]);
        let mine = s.scale(race, t) * 1024.0;
        for j in 0..s.formation.len() {
            let w = s.formation[j].cell[3];
            if w < 0 {
                continue;
            }
            let c = s.formation[j].cell;
            let limit = mine + s.scale(race, w) * 1024.0;
            if dist(at, self.pos_of([c[0], c[1], c[2]])) < limit {
                s.formation[j].cell[3] = -1;
            }
        }
        k as i32
    }

    /// `fegyverhozzaad` (exe 0x572650): `n` barrels of weapon `w` (a mount of `w` already there gains them).
    fn add_mount(&mut self, s: &mut Setup, u: usize, w: i32, n: i32) {
        self.add_weapon(&s.rules.weapons, s.rng, u, w, n);
    }

    /// [`Battle::add_mount`] with the weapon table and stream 6 (also the cheat order 0x75, spec §7.12).
    fn add_weapon(&mut self, weapons: &[WeaponRules], rng: &mut Streams, u: usize, w: i32, n: i32) {
        if let Some(m) = self.units[u].mounts.iter_mut().rev().find(|m| m.weapon == w) {
            m.count = m.count.wrapping_add(n);
            return;
        }
        let Some(r) = weapons.get(w as usize) else { return };
        let timer = reload_roll(rng, r.reload);
        let unit = &mut self.units[u];
        unit.mounts.push(Mount {
            weapon: w,
            kind: r.kind,
            count: n,
            timer,
            timer_set: timer,
            reload: r.reload,
            damage4: r.damage[4],
            range: r.range,
        });
        if r.kind != 2 {
            let range = r.range as f32;
            if range < unit.range_min || unit.range_min == 0.0 {
                unit.range_min = range;
            }
            if unit.range_max < range {
                unit.range_max = range;
            }
        }
    }

    /// `AddUjUrhajo(race, design, owner, ship, colour, −1, xp)` (exe 0x565800): a new unit, or the ship joins a
    /// unit of its fleet when no formation slot is left. Returns the unit (−1: no room; 0 or `ship`: see spec).
    fn add_unit(&mut self, s: &mut Setup, race: i32, design: i32, owner: i32, ship: i32, xp: i32) -> i32 {
        let (mut t, model, mut hp) = if design < SCENERY {
            let Some(d) = s.design(owner, design) else { return -1 };
            let (model, hp) = (d.hull_tech, d.hp.wrapping_mul(15));
            let t = design_type(model);
            let (c0, c2) = (self.cell[0], self.cell[2]);
            let i = race * TYPES_PER_RACE + t;
            if let Some(rec) = (0 <= i).then_some(()).and_then(|_| s.types.get_mut(i as usize)) {
                (rec.size_xy, rec.size_z) = footprint(rec.scale, c0, c2);
            }
            (t, model, hp)
        } else {
            // Scenery: the model is left uninitialised by the exe.
            (design - SCENERY, 0, 10000)
        };
        if t < 0 {
            t = 0;
        }
        let mut pos = V4::default();
        let mut slot = -1;
        let mut target = None;
        if owner != 0 {
            slot = self.find_slot(s, t, race);
            if slot == -1 {
                let fleet = s.ship_fleet(ship);
                let same_fleet = |u: &Unit| s.ship_fleet(u.ships.first().copied().unwrap_or(0)) == fleet;
                let any = self.units.iter().any(|u| {
                    u.alive && u.owner == owner && u.race == race && u.model == model && u.ship_count < 10000 && same_fleet(u)
                });
                if any {
                    let mut best = None;
                    let mut fewest = 10000;
                    for (k, u) in self.units.iter().enumerate() {
                        if u.alive && u.owner == owner && u.kind == t && u.ship_count < fewest && same_fleet(u) {
                            fewest = u.ship_count;
                            best = Some(k);
                        }
                    }
                    match best {
                        Some(k) => {
                            let u = &mut self.units[k];
                            if 9999 < u.ship_count {
                                return 0;
                            }
                            u.hp = u.hp.wrapping_add(hp);
                            u.experience = u.experience.wrapping_add(xp);
                            u.ships.truncate(u.ship_count.max(0) as usize);
                            u.ships.push(ship);
                            u.ship_count += 1;
                            target = Some((k, k as i32));
                        }
                        // Original bug: the pods and weapons go to the last unit examined, the shield bonus is
                        // computed from the ship index, and the ship index is returned.
                        None => {
                            hp = ship;
                            target = Some((self.units.len() - 1, ship));
                        }
                    }
                }
            } else {
                let c = s.formation[slot as usize].cell;
                pos = self.pos_of([c[0], c[1], c[2]]);
            }
        }
        let (u, ret) = match target {
            Some(x) => x,
            None => {
                let dead = self.units.iter().position(|u| !u.alive);
                let i = match (dead, self.units.len() >= UNIT_REUSE_FROM) {
                    (Some(i), _) => i,
                    (None, true) => return -1,
                    (None, false) => {
                        self.units.push(Unit::default());
                        self.units.len() - 1
                    }
                };
                let look = roll(s.rng, 0, 3);
                let rotation = self.sides.get(owner as usize).map_or([0.0; 3], |x| x.rotation);
                self.units[i] = Unit {
                    alive: true,
                    race,
                    kind: t,
                    design,
                    model,
                    owner,
                    hp,
                    ships: vec![ship],
                    ship_count: 1,
                    hp_share: 1,
                    waypoint_count: 1,
                    pos,
                    dest: pos,
                    step: pos,
                    heading: V4::new(0.0, 0.0, -1.0),
                    spin: rotation,
                    picked: -1,
                    id: i as i32,
                    experience: xp,
                    look,
                    weapon_mask: u32::MAX,
                    original_owner: owner,
                    ship_type: s.type_record(race, t),
                    ..Default::default()
                };
                self.set_cells(i);
                if slot != -1 {
                    let f = s.formation[slot as usize];
                    let u = &mut self.units[i];
                    u.waypoint_count = f.waypoint_count;
                    for k in 0..(f.waypoint_count.max(0) as usize).min(4) {
                        u.waypoints[k] = f.waypoints[k];
                    }
                }
                (i, i as i32)
            }
        };
        // The hull's built-in weapons (`alapfegyverek`, exe 0xE5E08C) are a BSS table nobody writes: none.
        if design < SCENERY {
            let Some(d) = s.design(owner, design).cloned() else { return ret };
            for k in 1..=d.slots.max(0) as usize {
                let (Some(&id), Some(&n)) = (d.raw.get(0x50 + k), d.raw.get(0x5d + k)) else { break };
                let n = n as i32;
                match id {
                    0x11..=0x20 => self.add_mount(s, u, id as i32 - 0x11, n),
                    0x41..=0x48 => self.add_mount(s, u, id as i32 - 0x34, n),
                    _ => {}
                }
                let unit = &mut self.units[u];
                (unit.shield, unit.pods) = pod_effect(id, hp, unit.shield, unit.pods);
            }
        }
        ret
    }

    /// `utkereselfer` (exe 0x614190): unit `i` stands clear of every other live unit.
    fn placement_free(&mut self, i: usize) -> bool {
        if self.search.grid.len() != GRID_CELLS + 4 {
            self.search.grid = vec![0; GRID_CELLS + 4];
        }
        self.search.grid[..GRID_CELLS].fill(CELL_FREE);
        let me = self.units[i].ship_type;
        for j in 0..self.units.len() {
            let u = &self.units[j];
            if !u.alive || j == i {
                continue;
            }
            let b = me.size_xy + u.ship_type.size_xy - 2;
            let bz = me.size_z - 2 + u.ship_type.size_z;
            let c = u.cell;
            self.search_box([c[0] - b, c[1] - b, c[2] - bz], [c[0] + b, c[1] + b, c[2] + bz]);
        }
        if !self.units[i].alive {
            return true;
        }
        let b = me.size_xy - 2 + me.size_xy;
        let bz = me.size_z - 2 + me.size_z;
        let c = self.units[i].cell;
        // `TeglaRajzol2` (0x613180): no blocked cell in the box.
        let lo = [(c[0] - b).clamp(0, 0x7f), (c[1] - b).clamp(0, 0x7f), (c[2] - bz).clamp(0, 7)];
        let hi = [(c[0] + b).clamp(0, 0x7f), (c[1] + b).clamp(0, 0x7f), (c[2] + bz).clamp(0, 7)];
        for z in lo[2]..=hi[2] {
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    if self.search.grid[(x as usize * GRID_Y + y as usize) * GRID_Z + z as usize] == CELL_BLOCKED {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// `setshippos(spin_y, cell, unit)` (exe 0x5802d0).
    fn set_ship_pos(&mut self, i: usize, spin_y: f32, cell: [i32; 3]) {
        let p = self.pos_of(cell);
        let u = &mut self.units[i];
        u.cell[..3].copy_from_slice(&cell);
        u.waypoints[0][..3].copy_from_slice(&cell);
        u.step_cell[..3].copy_from_slice(&cell);
        u.pos = V4 { w: u.pos.w, ..p };
        u.dest = V4 { w: u.dest.w, ..p };
        u.step = V4 { w: u.step.w, ..p };
        u.spin[1] = spin_y;
        let mut h = V4::new(-p.x, -p.y, -p.z);
        if h.x == 0.0 && h.y == 0.0 && h.z == 0.0 {
            h.z = -1.0;
        }
        normalize(&mut h);
        u.heading = V4 { w: u.heading.w, ..h };
    }

    /// `felallit(e)` (exe 0x572ef0): place side `e`'s waiting ships in the loaded formation.
    fn felallit(&mut self, s: &mut Setup, e: usize, list: &mut [Entry]) {
        for en in list.iter() {
            if en.unit != -1 {
                if let Some(u) = self.units.get_mut(en.unit as usize) {
                    u.alive = false;
                }
            }
        }
        // Face away from the empires already on the field (`gethajokcenter` per empire).
        let mut dirs = Vec::new();
        for o in 1..EMPIRES as i32 {
            let mine: Vec<&Unit> = self.units.iter().filter(|u| u.alive && u.owner == o).collect();
            if mine.is_empty() {
                continue;
            }
            let n = mine.len() as i32;
            let (sx, sy) = mine.iter().fold((0i32, 0i32), |a, u| (a.0.wrapping_add(u.cell[0]), a.1.wrapping_add(u.cell[1])));
            let mut d = V4::new((sx / n - self.grid[0] / 2) as f32, (sy / n - self.grid[1] / 2) as f32, 0.0);
            normalize(&mut d);
            dirs.push(d);
        }
        let mut dir = match dirs.len() {
            0 => V4::new(0.0, -1.0, 0.0),
            1 => V4::new(-dirs[0].x, -dirs[0].y, -dirs[0].z),
            2 => {
                let m = V4::new((dirs[1].x + dirs[0].x) * 0.5, (dirs[1].y + dirs[0].y) * 0.5, (dirs[1].z + dirs[0].z) * 0.5);
                if m.len() != 0.0 { V4::new(-m.x, -m.y, -m.z) } else { V4::new(dirs[0].y, -dirs[0].x, dirs[0].z) }
            }
            // Three or more: the exe leaves the direction uninitialised (stack) [undefined].
            _ => V4::new(0.0, -1.0, 0.0),
        };
        normalize(&mut dir);
        dir.x *= -1.0;
        dir.y *= -1.0;
        rotate_formation(&mut s.formation, dir, self.grid);
        for u in &self.units {
            if !u.alive {
                continue;
            }
            let mine = s.scale(u.race, u.kind) * 1024.0;
            for k in 0..s.formation.len() {
                let w = s.formation[k].cell[3];
                if w < 0 {
                    continue;
                }
                let c = s.formation[k].cell;
                if dist(u.pos, self.pos_of([c[0], c[1], c[2]])) < mine + s.scale(u.race, w) * 1024.0 {
                    s.formation[k].cell[3] = -1;
                }
            }
        }
        for en in list.iter_mut() {
            en.unit = self.add_unit(s, en.race, en.design, en.owner, en.ship, en.experience);
        }
        for i in 0..self.units.len() {
            let u = &mut self.units[i];
            if !u.alive || u.owner != e as i32 {
                continue;
            }
            let mut h = V4::new(-u.pos.x, -u.pos.y, -u.pos.z);
            if h.x == 0.0 && h.y == 0.0 && h.z == 0.0 {
                h.z = -1.0;
            }
            normalize(&mut h);
            u.heading = V4 { w: u.heading.w, ..h };
            if 1 < u.waypoint_count {
                self.next_waypoint(i);
            }
        }
    }

    /// `loadfelallas("felallas<file>")` (exe 0x578850) outside the editor: `SpaceWarFrissit2` calls it when the
    /// formation phase's pick (`+0x2d88`, file = pick + 1) differs from the loaded one (`+0x2d8c`). With ships in
    /// the human's list, `FelallasLoad` and `felallit` of the human's side: its units die and its ships get new
    /// ones in the new formation, turned to face the others (stream 6: each new unit's look and mount timers).
    /// `inp` needs only `ships` and `designs` (the strategic records); `types` is the global ship-type table.
    pub fn pick_formation(&mut self, file: usize, inp: &SetupInput, rules: &SetupRules, types: &mut [ShipType], rng: &mut Streams) {
        let Some(formation) = rules.formations.get(file) else { return };
        if self.human_list.is_empty() {
            return;
        }
        let mut list = std::mem::take(&mut self.human_list);
        let mut s = Setup { rules, inp, types, rng, formation: formation.clone() };
        self.felallit(&mut s, self.human as usize, &mut list);
        self.human_list = list;
    }

    /// `AddUjVadasz(race, class, owner, fleet, design, count, groups)` (exe 0x566420).
    #[allow(clippy::too_many_arguments)]
    fn add_fighters(&mut self, s: &mut Setup, race: i32, class: i32, owner: i32, fleet: i32, design: i32, count: i32, groups: i32) {
        let hp = s.design(owner, design).map_or(0, |d| d.attack);
        let centre = self.sides.get(owner as usize).map_or(V4::default(), |x| x.centre);
        for n in group_sizes(count, groups) {
            let mut off = || ((roll(s.rng, 200, 999) - 500) * 12 / 500) as f32;
            let x = off() + centre.x;
            let y = off() + centre.y;
            let z = off() + centre.z;
            let pos = V4::new(x, y, z);
            let dir = normalized(V4::new(-x, -y, -z));
            self.fighters.push(Fighter {
                alive: true,
                pos,
                dest: pos,
                vel: dir.scale(0.2),
                dir,
                design,
                owner,
                race,
                weapon: class,
                fleet,
                count: n,
                hp,
                ..Default::default()
            });
        }
    }

    /// `bolygofegyverhozzaad(tech, n, x, slot)` (exe 0x571d00): a planet gun (at most 50).
    fn add_gun(&mut self, s: &mut Setup, tech: i32, slot: i32) {
        if MAX_GUNS <= self.guns.len() {
            return;
        }
        let armour = match tech {
            0x79 => 1.3,
            0x7a => 1.5,
            0x7b => 1.8,
            _ => 1.0,
        };
        let weapon = match tech {
            0x71..=0x74 => tech - 0x71 + 20,
            _ => -1,
        };
        let mut g = Gun { weapon, armour, building: slot, active: true, target: -1, ..Default::default() };
        if 1 <= weapon {
            if let Some(w) = s.rules.weapons.get(weapon as usize) {
                g.reload = w.reload;
                g.timer = roll(s.rng, 0, w.reload - 1);
                g.timer_set = g.timer;
            }
        }
        let a = roll(s.rng, 0, 99) - 0x50;
        let b = roll(s.rng, 0, 99) - 0xb4;
        let mut v = V4::new(-100.0, a as f32, b as f32);
        normalize(&mut v);
        let k = (self.planet_size << 10) as f32;
        let p = self.planet_pos;
        g.pos = V4::new(p[0] as f32 + v.z * k, p[1] as f32 + v.y * k, p[2] as f32 + v.x * k);
        g.hits = if self.planet_owner == self.human { 1000 } else { 1 };
        self.guns.push(g);
    }

    /// `AddFleet` (exe 0x5646b0): the listed fleets (and on a new battle the planet's defences and the
    /// asteroids) join the battle.
    fn add_fleet(&mut self, s: &mut Setup) {
        let inp = s.inp;
        self.active = true;
        for side in &mut self.sides {
            side.centre = V4::default();
            side.fighter_count = 0;
        }
        let mut lists: Vec<Vec<Entry>> = vec![Vec::new(); EMPIRES];
        let new = self.participants.is_empty();
        self.planet = inp.planet;
        self.human = inp.human;
        self.ground_war = inp.ground_war;
        let owner_of = |f: i32| inp.fleet_records.get(f as usize).map_or(0, |r| r.owner as i32);
        let mut groups: Vec<FighterGroup> = Vec::new();
        for &f in &inp.fleets {
            if f == -1 {
                continue;
            }
            let rec = inp.fleet_records.get(f as usize).copied().unwrap_or_default();
            let o = rec.owner as i32;
            if let Some(side) = self.sides.get_mut(o as usize) {
                side.ai = !inp.human_controlled.get(o as usize).copied().unwrap_or(false);
            }
            self.participants.push(Participant { owner: o, status: 3, fleet: f, story_id: rec.story_id as i32, hp: [rec.hp; 2] });
            for v in inp.vehicles.iter().skip(1) {
                if v.design == crate::spacewar::NO_DESIGN || v.fleet as i32 != f {
                    continue;
                }
                let Some(d) = s.design(o, v.design as i32) else { continue };
                if !d.is_fighter() {
                    continue;
                }
                match groups.iter_mut().find(|g| g.fleet == v.fleet as i32 && g.design == v.design as i32) {
                    Some(g) => g.count = g.count.wrapping_add(v.count),
                    None => groups.push(FighterGroup {
                        design: v.design as i32,
                        owner: o,
                        race: d.pod_table as i32,
                        class: d.hull,
                        fleet: v.fleet as i32,
                        count: v.count,
                    }),
                }
            }
        }
        if self.planet == 0 {
            self.planet_owner = -1;
        } else {
            let pr = inp.planet_record.cloned().unwrap_or_default();
            self.planet_owner = pr.owner as i32;
            for (k, b) in pr.buildings.iter().enumerate() {
                let tech = s.rules.building_tech.get(b.kind as usize).copied().unwrap_or(0) as i32;
                if ((0x71..=0x74).contains(&tech) || (0x79..=0x7b).contains(&tech)) && b.finished && b.on {
                    self.add_gun(s, tech, k as i32 + 1);
                }
            }
            let o = self.planet_owner;
            for (k, &d) in pr.ships.iter().take(8).enumerate() {
                if let Some(l) = lists.get_mut(o as usize) {
                    l.push(Entry { race: o, design: d as i32, owner: o, ship: -(k as i32 + 1), unit: -1, experience: -1 });
                }
            }
        }
        for &f in &inp.fleets {
            if f == -1 {
                continue;
            }
            let o = owner_of(f);
            for (i, sh) in inp.ships.iter().enumerate().skip(1) {
                if sh.fleet as i32 != f || sh.design == crate::spacewar::NO_DESIGN {
                    continue;
                }
                let race = s.design(o, sh.design as i32).map_or(0, |d| d.pod_table as i32);
                lists[o as usize].push(Entry { race, design: sh.design as i32, owner: o, ship: i as i32, unit: -1, experience: sh.experience as i32 });
            }
            sort_side(s, &mut lists[o as usize]);
        }
        if new {
            if let Some(&f) = inp.fleets.first() {
                let r = inp.fleet_records.get(f as usize).copied().unwrap_or_default();
                self.map_pos = [r.x, r.y];
            }
            if let (true, Some(n)) = (inp.asteroids, self.asteroids) {
                for (k, &count) in n.iter().enumerate() {
                    for _ in 0..count {
                        let u = self.add_unit(s, 0, SCENERY + k as i32, 0, 0, -1);
                        if u < 0 {
                            continue;
                        }
                        let u = u as usize;
                        loop {
                            let c = [roll(s.rng, 0, self.grid[0]), roll(s.rng, 0, self.grid[1]), roll(s.rng, 0, self.grid[2])];
                            self.set_ship_pos(u, 0.0, c);
                            for a in 0..3 {
                                self.units[u].spin[a] = roll(s.rng, 0, 100) as f32 / 20.0;
                            }
                            if self.placement_free(u) {
                                break;
                            }
                        }
                    }
                }
            }
        }
        let mut fighterless = 0;
        for (e, list) in lists.iter_mut().enumerate() {
            if list.is_empty() {
                continue;
            }
            s.formation = s.rules.formations.get(if list.len() < 6 { 0 } else { 1 }).cloned().unwrap_or_default();
            self.felallit(s, e, list);
            if self.sides[e].fighter_count == 0 {
                fighterless += 1;
            }
        }
        for g in &groups {
            if let Some(side) = self.sides.get_mut(g.owner as usize) {
                side.fighter_count = side.fighter_count.wrapping_add(g.count);
            }
        }
        let mut k = fighterless;
        for (e, list) in lists.iter().enumerate() {
            if list.is_empty() && self.sides[e].fighter_count == 0 {
                continue;
            }
            let c = &mut self.sides[e].centre;
            match k {
                0 => (c.x, c.y, c.z) = (0.0, 0.0, -50.0),
                1 => (c.x, c.y, c.z) = (0.0, 0.0, 50.0),
                2 => (c.x, c.y, c.z) = (0.0, -50.0, 0.0),
                3 => (c.x, c.y, c.z) = (0.0, 50.0, 0.0),
                _ => {}
            }
            k += 1;
        }
        let total = groups.iter().fold(0i32, |a, g| a.wrapping_add(g.count));
        let mut scale = 1.0f32;
        if total != 0 {
            scale = 60.0 / total as f32;
            if 1.0 < scale {
                scale = 1.0;
            }
        }
        for g in &groups {
            let n = match trunc(g.count as f32 * scale) {
                0 => 1,
                n => n,
            };
            if g.count != 0 {
                self.add_fighters(s, g.race, g.class - 1, g.owner, g.fleet, g.design, g.count, n);
            }
        }
        self.formation_choice = lists.get(self.human as usize).is_some_and(|l| 6 <= l.len()) && !inp.multiplayer;
        // `AddFleet` frees the lists when it starts, so the human's is the one this call built.
        self.human_list = lists.get(self.human as usize).cloned().unwrap_or_default();
        self.f2d86 = false;
        self.recount_present();
    }
}

/// `AddFleet`'s two sorts of a side's waiting ships (spec §7.10 step 4).
fn sort_side(s: &Setup, list: &mut [Entry]) {
    let class = |e: &Entry| slot_class(design_type(s.design(e.owner, e.design).map_or(0, |d| d.hull_tech)));
    let key = |e: &Entry| e.race.wrapping_mul(1000).wrapping_add(s.design(e.owner, e.design).map_or(0, |d| d.hull_tech));
    let n = list.len();
    for i in 0..n {
        let mut best = class(&list[i]);
        let mut bi = i;
        for (j, e) in list.iter().enumerate().skip(i + 1) {
            let c = class(e);
            if best < c {
                best = c;
                bi = j;
            }
        }
        if bi != i {
            list.swap(i, bi);
        }
    }
    let mut i = 0;
    while i < n {
        let c = class(&list[i]);
        let mut end = i;
        while end < n && class(&list[end]) == c {
            end += 1;
        }
        // "The smallest key strictly above the previous one" (repeated keys stay where they are).
        let mut prev = 0;
        for k in i..end {
            let mut best = 100_000_000;
            let mut bi = None;
            for (j, e) in list.iter().enumerate().take(end).skip(k) {
                let v = key(e);
                if v < best && prev < v {
                    best = v;
                    bi = Some(j);
                }
            }
            let Some(j) = bi else { break };
            if j != k {
                list.swap(k, j);
            }
            prev = best;
        }
        i = end;
    }
}

/// `SpaceWar_NewBattle` (exe 0x563700): join the battle a listed fleet already fights in, else start one in the
/// first free slot; returns the slot (None: all ten busy). `types` is the global ship-type table.
pub fn new_battle(slots: &mut [Battle], inp: &mut SetupInput, rules: &SetupRules, types: &mut [ShipType], rng: &mut Streams) -> Option<usize> {
    let mut found = None;
    for (b, slot) in slots.iter().enumerate() {
        if !slot.active || found.is_some() {
            continue;
        }
        for p in &slot.participants {
            for f in inp.fleets.iter_mut() {
                if *f == p.fleet {
                    *f = -1;
                    found = Some(b);
                }
            }
        }
    }
    let b = match found {
        Some(b) => b,
        None => {
            let b = slots.iter().position(|x| !x.active)?;
            slots[b].init_space_war(inp.asteroids, rules, rng);
            b
        }
    };
    let battle = &mut slots[b];
    {
        let mut s = Setup { rules, inp, types, rng, formation: Vec::new() };
        battle.add_fleet(&mut s);
    }
    let en = inp.human;
    let mut flat = [0u8; EMPIRES * EMPIRES];
    for (a, row) in battle.hostile.iter().enumerate() {
        flat[a * EMPIRES..(a + 1) * EMPIRES].copy_from_slice(row);
    }
    fn clear(flat: &mut [u8], i: i32) {
        if (0..flat.len() as i32).contains(&i) {
            flat[i as usize] = 0;
        }
    }
    let n = EMPIRES as i32;
    for a in 1..n {
        for c in 1..n {
            flat[(a * n + c) as usize] = 1;
        }
        clear(&mut flat, a * n + a);
        if !inp.multiplayer {
            if inp.story_pact[a as usize] == 1 {
                clear(&mut flat, en * n + a);
                clear(&mut flat, a * n + en);
            }
            let z = inp.story_zachkin;
            if z != 0 {
                clear(&mut flat, z * n + 9);
                clear(&mut flat, 9 * n + z);
            }
        }
        battle.story_retreat[a as usize] = inp.story_retreat[a as usize];
    }
    for (a, row) in battle.hostile.iter_mut().enumerate() {
        row.copy_from_slice(&flat[a * EMPIRES..(a + 1) * EMPIRES]);
    }
    Some(b)
}

// ------------------------------------------------------------------------------------------------
// The end (spec §7.11)

/// What `SpaceWarVege` hands to the strategic game.
#[derive(Debug, Clone, PartialEq)]
pub struct BattleResult {
    /// `SpWarKiNyert` (exe 0xBAA1EC): the winner (0 = nobody).
    pub winner: i32,
    /// `SpWarVesztesVisszaVonult` (0xBAA1DE): always cleared here.
    pub loser_retreated: bool,
    /// `SpwFlottak` / `SpwFlottaNum` for `Story_SpaceBattleFinished` (0x5c9180), which then sets
    /// `SpWarYouWasInBattle` (= `human_took_part`), story event 3, clears `StoryAdat+0x88` and runs the
    /// campaign script (`Story_Check(1)`).
    pub participants: Vec<Participant>,
    pub human_took_part: bool,
}

impl Battle {
    /// The end check found the battle over on the battle screen (spec §7.11, the tick's step 4): the ground war
    /// starts when ground units wait, it hasn't started yet, the first participant's empire still has units
    /// and its fleet carries ground units (`ground_units(fleet)` = `Flottak[fleet].+0x5c`). Returns that fleet
    /// for `GroundWarIndit(fleet, planet)`; the caller also leaves the screen and calls `SetIdoLelassit(0)`.
    pub fn ground_war_due(&mut self, ground_units: impl Fn(i32) -> i32) -> Option<i32> {
        let p = self.participants.first()?;
        let alive = self.counts.get(p.owner as usize).is_some_and(|&c| 0 < c);
        if self.ground_war && !self.ground_war_started && alive && 0 < ground_units(p.fleet) {
            self.ground_war_started = true;
            return Some(p.fleet);
        }
        None
    }

    /// `SpaceWarVege` (exe 0x5706f0) with `EndSpaceWar`: the result, and the slot closes.
    pub fn finish(&mut self) -> BattleResult {
        let r = BattleResult {
            winner: self.winner,
            loser_retreated: false,
            human_took_part: self.participants.iter().any(|p| p.owner == self.human),
            participants: self.participants.clone(),
        };
        self.active = false;
        self.shown = false;
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strategic map that records what the battle did to it.
    #[derive(Default)]
    struct Log {
        deleted: Vec<i32>,
        fleets_deleted: Vec<i32>,
    }

    impl BattleWorld for Log {
        fn ship_fleet(&self, _ship: i32) -> i32 {
            1
        }
        fn ship_attack(&self, _owner: i32, _ship: i32) -> i32 {
            10
        }
        fn delete_ship(&mut self, ship: i32) {
            self.deleted.push(ship);
        }
        fn fleet_empty(&self, _fleet: i32) -> bool {
            self.deleted.len() == 2
        }
        fn delete_fleet(&mut self, fleet: i32) {
            self.fleets_deleted.push(fleet);
        }
        fn fleet_owner(&self, _fleet: i32) -> i32 {
            3
        }
        fn delete_fighters(&mut self, _fleet: i32, _design: i32, _n: i32) {}
        fn remove_planet_ship(&mut self, _planet: i32, _k: i32) {}
        fn damage_building(&mut self, _planet: i32, _slot: i32, _damage: i32) -> bool {
            false
        }
        fn retreat_fleet(&mut self, _fleet: i32) {}
    }

    fn rules() -> BattleRules {
        let w = WeaponRules {
            kind: 1,
            reload: 150,
            damage: [10; 12],
            hit: 80,
            spread: 0.2,
            speed: 5.0,
            accel: 1.0,
            duration: 20,
            range: 50,
            form: 0,
            aimed: true,
            ..Default::default()
        };
        BattleRules {
            weapons: vec![w; 40],
            forms: vec![FormRules { loaded: true, slots: 3, size: 1.0 }; FORMS],
            ..Default::default()
        }
    }

    fn battle(units: Vec<Unit>) -> Battle {
        let mut b = Battle {
            units,
            pools: vec![Pool { shots: vec![Shot::default(); 3] }; FORMS],
            sides: vec![Side::default(); EMPIRES],
            human: 6,
            planet_owner: -1,
            cell: [3.0; 3],
            grid: [128, 128, 10],
            bounds: [-192.0, 192.0, -192.0, 192.0, -15.0, 15.0],
            width: 384.0,
            end_period: 100,
            pod_timer: 10,
            ..Default::default()
        };
        for a in 1..EMPIRES {
            for c in 1..EMPIRES {
                b.hostile[a][c] = if a == c { 0 } else { 1 };
            }
        }
        b
    }

    fn ship(owner: i32, pos: V4) -> Unit {
        Unit {
            alive: true,
            owner,
            original_owner: owner,
            hp: 900,
            ships: vec![7],
            ship_count: 1,
            hp_share: 1,
            pos,
            heading: V4::new(0.0, 1.0, 0.0),
            picked: -1,
            weapon_mask: u32::MAX,
            ship_type: ShipType { scale: 0.005, top_speed: 0.4, accel: 0.02, turn: 0.1, size_xy: 2, size_z: 1 },
            mounts: vec![Mount { weapon: 0, kind: 1, count: 1, timer: -1, reload: 150, range: 50, ..Default::default() }],
            range_min: 50.0,
            range_max: 50.0,
            ..Default::default()
        }
    }

    #[test]
    fn distances_keep_the_exe_summation_order() {
        // (dy² + dx²) + dz²: rounding differs from (dx² + dy²) + dz² for these values.
        let a = V4::new(0.0, 0.0, 0.0);
        let b = V4::new(1.0e8, 1.0, 3.0);
        assert_eq!(dist2(a, b), (1.0f32 * 1.0 + 1.0e8 * 1.0e8) + 9.0);
        assert_eq!(normalize(&mut V4::new(1e-6, 0.0, 0.0)), 0.0);
        assert_eq!(normalized(V4::new(3.0, 4.0, 0.0)), V4::new(0.6, 0.8, 0.0));
        let a = angle(V4::new(1.0, 0.0, 0.0), V4::new(0.0, 1.0, 0.0));
        assert!((a - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(trunc(f32::NAN), i32::MIN);
        assert_eq!(trunc(-2.9), -2);
    }

    #[test]
    fn rotation_turns_the_heading_about_the_axis() {
        let m = axis_matrix(V4::new(0.0, 0.0, 1.0), std::f32::consts::FRAC_PI_2);
        let r = rotate(V4::new(1.0, 0.0, 0.0), &m);
        assert!(r.x.abs() < 1e-6 && (r.y - 1.0).abs() < 1e-6 && r.z.abs() < 1e-6);
    }

    #[test]
    fn shields_absorb_first_and_the_last_ship_dies_past_its_hit_points() {
        let mut b = battle(vec![ship(3, V4::new(0.0, 0.0, 0.0))]);
        b.units[0].shield = 100;
        let r = rules();
        let words = vec![vec![0; 46]; 40];
        let mut rng = Streams::default();
        let mut w = Log::default();
        let mut ev = Vec::new();
        let mut t = Tick { rules: &r, weapon_words: &words, rng: &mut rng, world: &mut w, events: &mut ev };
        b.take_damage(&mut t, 0, 0, 150);
        assert_eq!((b.units[0].shield_used, b.units[0].damage), (100, 50));
        b.take_damage(&mut t, 0, 0, 900);
        assert!(!b.units[0].alive);
        assert_eq!(w.deleted, vec![7]);
        // The owner's losses count the design's attack.
        assert_eq!(b.sides[3].lost, 10);
    }

    #[test]
    fn a_group_loses_ships_and_scales_its_damage() {
        let mut u = ship(3, V4::new(0.0, 0.0, 0.0));
        u.ships = vec![7, 8, 9];
        u.ship_count = 3;
        u.hp = 2700;
        u.mounts[0].count = 3;
        let mut b = battle(vec![u]);
        let r = rules();
        let words = vec![vec![0; 46]; 40];
        let mut rng = Streams::default();
        let mut w = Log::default();
        let mut ev = Vec::new();
        let mut t = Tick { rules: &r, weapon_words: &words, rng: &mut rng, world: &mut w, events: &mut ev };
        // One ship's "share" is all 2700 hit points (`+0x1b4` is 1): a 2700-point hit costs one ship.
        b.take_damage(&mut t, 0, 1, 2700);
        let x = &b.units[0];
        assert_eq!((x.ship_count, &x.ships[..2]), (2, &[7, 9][..]));
        assert_eq!(x.damage, 1800);
        assert_eq!(x.mounts[0].count, 2);
        assert_eq!(w.deleted, vec![8]);
    }

    #[test]
    fn shots_take_the_first_free_slot_but_never_the_last() {
        let mut b = battle(vec![ship(3, V4::new(0.0, 0.0, 0.0)), ship(6, V4::new(0.0, 30.0, 0.0))]);
        let r = rules();
        let words = vec![vec![0; 46]; 40];
        let mut rng = Streams::default();
        let mut w = Log::default();
        let mut ev = Vec::new();
        let mut t = Tick { rules: &r, weapon_words: &words, rng: &mut rng, world: &mut w, events: &mut ev };
        for _ in 0..3 {
            let p = b.units[0].pos;
            b.fire(&mut t, 3, p, shot_kind::SHIP, 0, 1, 0, 1);
        }
        let used: Vec<bool> = b.pools[0].shots.iter().map(|s| s.shooter.is_some()).collect();
        assert_eq!(used, vec![true, true, false]);
        let s = &b.pools[0].shots[0];
        assert_eq!((s.owner, s.target, s.speed), (3, 1, 5.0));
        // Aimed along +y.
        assert!(s.dir.y > 0.9);
    }

    #[test]
    fn the_human_story_bonus_tests_the_slots_previous_owner() {
        let mut b = battle(vec![ship(3, V4::new(0.0, 0.0, 0.0)), ship(6, V4::new(0.0, 30.0, 0.0))]);
        let mut r = rules();
        r.story_damage_bonus = 100;
        let words = vec![vec![0; 46]; 40];
        let mut rng = Streams::default();
        let mut w = Log::default();
        let mut ev = Vec::new();
        b.pools[0].shots[0].owner = 6;
        let mut t = Tick { rules: &r, weapon_words: &words, rng: &mut rng, world: &mut w, events: &mut ev };
        let p = b.units[0].pos;
        b.fire(&mut t, 3, p, shot_kind::SHIP, 0, 1, 0, 1);
        assert_eq!(b.pools[0].shots[0].mult, 2.0);
        assert_eq!(b.pools[0].shots[0].owner, 3);
    }

    #[test]
    fn the_path_search_steps_towards_the_goal_around_a_blocker() {
        let mut b = battle(vec![ship(3, V4::new(0.0, 0.0, 0.0)), ship(6, V4::new(0.0, 0.0, 0.0))]);
        b.units[1].cell = [70, 64, 5, 0];
        b.path_search([60, 64, 5], [80, 64, 5], 1, 0);
        let r = b.search.result;
        assert!(r != [60, 64, 5] && !b.search.arrived);
        // Not through the blocker's box (x 67..=73 around y 64).
        assert!(!(67..=73).contains(&r[0]) || (r[1] - 64).abs() > 3 || (r[2] - 5).abs() > 1);
        // In the open, the line-of-sight cut takes the goal cell itself (radius 1: the sphere is just it).
        let mut b = battle(vec![ship(3, V4::new(0.0, 0.0, 0.0))]);
        b.path_search([60, 64, 5], [80, 64, 5], 1, 0);
        assert_eq!(b.search.result, [80, 64, 5]);
    }

    #[test]
    fn the_special_pod_pass_runs_every_tenth_call() {
        let mut u = ship(3, V4::new(0.0, 0.0, 0.0));
        u.pods = pod::CLOAK;
        let mut b = battle(vec![u, ship(6, V4::new(0.0, 100.0, 0.0))]);
        for _ in 0..9 {
            b.special_pods();
        }
        assert!(!b.units[0].cloaked);
        b.special_pods();
        assert!(b.units[0].cloaked);
        assert_eq!(b.pod_timer, 10);
    }

    #[test]
    fn the_end_check_waits_for_its_period_and_counts_original_owners() {
        let mut taken = ship(6, V4::new(0.0, 0.0, 0.0));
        taken.original_owner = 3;
        let mut b = battle(vec![ship(3, V4::new(0.0, 0.0, 0.0)), taken]);
        b.end_timer = 1;
        let r = rules();
        assert!(!b.check_end(&r));
        // Both units count for empire 3: nobody left to fight.
        assert!(b.check_end(&r));
        assert_eq!((b.winner, b.counts[3], b.end_timer), (3, 2, 100));
    }

    // --------------------------------------------------------------------------------------------
    // Setup and end (spec §7.10, §7.11). Spot values from `spwc-setup-*` (the replay checks every field).

    #[test]
    fn design_types_and_slot_classes_match_the_traced_units() {
        // (design hull tech `+0x48`, the unit type the exe gave the ship).
        for (model, t) in [(77, 8), (78, 9), (9, 2), (13, 5), (7, 1), (5, 0), (6, 0)] {
            assert_eq!(design_type(model), t, "{model}");
        }
        assert_eq!((design_type(0x30), design_type(0xe), design_type(2)), (6, 6, -1));
        let c: Vec<i32> = (0..12).map(slot_class).collect();
        assert_eq!(c, [0, 1, 2, 3, 3, 3, 3, 3, 11, 11, 11, 11]);
    }

    #[test]
    fn footprints_match_the_traced_ship_types() {
        // (scale, the footprint the exe wrote into the record), cell size 3.
        for (scale, fp) in [(0.002f32, (2, 1)), (0.00225, (2, 1)), (0.0035, (3, 2)), (0.004, (3, 2)), (0.01, (4, 3)), (0.02, (5, 4))] {
            assert_eq!(footprint(scale, 3.0, 3.0), fp, "{scale}");
        }
    }

    #[test]
    fn shield_pods_add_the_traced_shares_of_the_hit_points() {
        // (ship hit points, slot items) → the shield and flags the exe gave the unit.
        let apply = |hp: i32, ids: &[u8]| ids.iter().fold((0, 0u32), |(s, p), &id| pod_effect(id, hp, s, p));
        assert_eq!(apply(900, &[0x31, 0x25, 0x27, 0x29]), (135, 0x161));
        assert_eq!(apply(900, &[0x31, 0x32, 0x2a]), (405, 0x203));
        assert_eq!(apply(225, &[0x34, 0x35]), (315, 0x18));
        assert_eq!(apply(375, &[0x33, 0x2a]), (168, 0x204));
        assert_eq!(apply(900, &[0x36]), (900, 0x800));
        assert_eq!(apply(900, &[0x11, 0x41]), (0, 0));
    }

    #[test]
    fn fighters_split_like_the_traced_groups() {
        assert_eq!(group_sizes(66, 31), [vec![3; 4], vec![2; 27]].concat());
        assert_eq!(group_sizes(37, 13), [vec![3; 11], vec![2; 2]].concat());
        assert_eq!(group_sizes(6, 4), vec![2, 2, 1, 1]);
        assert_eq!(group_sizes(5, 5), vec![1; 5]);
        assert!(group_sizes(7, 0).is_empty());
    }

    #[test]
    fn formations_turn_about_the_grid_centre() {
        let slot = |x, y| FormationSlot { cell: [x, y, 5, 3], waypoint_count: 1, waypoints: [[x, y, 5, 3]; 4] };
        let mut f = vec![slot(64, 55), slot(72, 54)];
        rotate_formation(&mut f, V4::new(0.0, 1.0, 0.0), [128, 128, 10]);
        assert_eq!((f[0].cell, f[1].cell), ([64, 55, 5, 3], [72, 54, 5, 3]));
        // A quarter turn: (x, y) → (y, 128 − x); waypoints beyond the count stay.
        rotate_formation(&mut f, V4::new(1.0, 0.0, 0.0), [128, 128, 10]);
        assert_eq!((f[1].cell, f[1].waypoints[0], f[1].waypoints[1]), ([54, 56, 5, 3], [54, 56, 5, 3], [72, 54, 5, 3]));
    }

    fn design(hp: i32, hull_tech: i32, race: i16, pods: &[(u8, u8)]) -> crate::spacewar::Design {
        let mut r = [0u8; 0x84];
        r[0x16..0x18].copy_from_slice(&1i16.to_le_bytes());
        r[0x2c..0x30].copy_from_slice(&7i32.to_le_bytes());
        r[0x30..0x34].copy_from_slice(&hp.to_le_bytes());
        r[0x40..0x42].copy_from_slice(&race.to_le_bytes());
        r[0x48..0x4c].copy_from_slice(&hull_tech.to_le_bytes());
        r[0x4c..0x50].copy_from_slice(&(pods.len() as i32).to_le_bytes());
        for (k, &(id, n)) in pods.iter().enumerate() {
            r[0x51 + k] = id;
            r[0x5e + k] = n;
        }
        crate::spacewar::Design::from_bytes(&r)
    }

    #[test]
    fn a_new_battle_places_both_sides_a_join_adds_to_it_and_the_end_reports() {
        let line = |y| (0..6).map(|k| FormationSlot { cell: [40 + 8 * k, y, 5, 3], waypoint_count: 1, waypoints: [[40 + 8 * k, y, 5, 3]; 4] }).collect::<Vec<_>>();
        let mut weapons = vec![WeaponRules::default(); 40];
        weapons[0] = WeaponRules { kind: 1, reload: 30, range: 50, ..Default::default() };
        let rules = SetupRules {
            weapons,
            forms: vec![FormRules { loaded: true, slots: 3, size: 1.0 }; FORMS],
            formations: vec![line(20), line(20)],
            building_tech: Vec::new(),
        };
        let mut types = vec![ShipType::default(); 240];
        for race in [3, 5, 6] {
            types[race * 12 + 2].scale = 0.002;
        }
        let mut designs = vec![vec![design(0, 0, 0, &[])]; EMPIRES];
        for (e, race) in [(3usize, 3i16), (5, 5), (6, 6)] {
            designs[e].push(design(60, 9, race, &[(0x11, 2), (0x31, 1)]));
        }
        let fleet = |owner| SetupFleet { owner, story_id: 0, hp: 180, x: 10.0, y: 20.0 };
        let fleets = [SetupFleet::default(), fleet(3), fleet(6), fleet(5)];
        let ship = |fleet| SetupShip { fleet, design: 1, experience: 4 };
        let ships = [SetupShip::default(), ship(1), ship(1), ship(1), ship(2), ship(2), ship(3)];
        let vehicles = [crate::spacewar::Vehicle { fleet: 0, design: 0x7fff, count: 0 }];
        let mut story_pact = [0; EMPIRES];
        story_pact[5] = 1;
        let input = |list: Vec<i32>| SetupInput {
            fleets: list,
            planet: 0,
            ground_war: false,
            human: 6,
            human_controlled: std::array::from_fn(|e| e == 6),
            multiplayer: false,
            asteroids: false,
            story_pact,
            story_zachkin: 0,
            story_retreat: [1; EMPIRES],
            fleet_records: &fleets,
            ships: &ships,
            vehicles: &vehicles,
            designs: &designs,
            planet_record: None,
            defence_fleets: None,
        };
        let mut slots: Vec<Battle> = (0..10).map(|_| Battle::slot()).collect();
        slots[0].active = true;
        slots[0].ai_timer = 7.0;
        let mut rng = Streams::default();
        let s6 = rng.state(STREAM);
        let mut inp = input(vec![1, 2]);
        assert_eq!(new_battle(&mut slots, &mut inp, &rules, &mut types, &mut rng), Some(1));
        let b = &slots[1];
        assert_eq!((b.ai_timer, b.end_period, b.human, b.planet_owner, b.active), (0.0, 100, 6, -1, true));
        assert_ne!(rng.state(STREAM), s6);
        assert_eq!(b.participants.iter().map(|p| (p.owner, p.status, p.fleet, p.hp)).collect::<Vec<_>>(), [(3, 3, 1, [180; 2]), (6, 3, 2, [180; 2])]);
        assert_eq!((b.hostile[3][6], b.hostile[6][3], b.hostile[3][3], b.hostile[0][3]), (1, 1, 0, 0));
        assert_eq!((b.sides[3].ai, b.sides[6].ai, b.story_retreat[4]), (true, false, 1));
        // One unit per ship, each in its own slot; the footprint was written into the type record.
        assert_eq!(b.units.len(), 5);
        let mut cells: Vec<[i32; 4]> = b.units.iter().map(|u| u.cell).collect();
        cells.dedup();
        assert_eq!(cells.len(), 5);
        assert_eq!((types[3 * 12 + 2].size_xy, types[3 * 12 + 2].size_z), (2, 1));
        for u in &b.units {
            assert_eq!((u.kind, u.hp, u.shield, u.pods, u.experience, u.ship_count), (2, 900, 135, 1, 4, 1));
            assert_eq!((u.mounts.len(), u.mounts[0].count, u.range_min, u.range_max), (1, 2, 50.0, 50.0));
            assert!((20..30).contains(&u.mounts[0].timer));
            assert!((u.heading.len() - 1.0).abs() < 1e-6);
        }
        assert_eq!(b.present.empires.iter().map(|x| x.0).collect::<Vec<_>>(), [3, 6]);
        // Fleet 3 (empire 5, in a story pact with the human) joins: fleet 2 is already fighting.
        let mut inp = input(vec![3, 2]);
        assert_eq!(new_battle(&mut slots, &mut inp, &rules, &mut types, &mut rng), Some(1));
        assert_eq!(inp.fleets, [3, -1]);
        let b = &mut slots[1];
        assert_eq!((b.units.len(), b.participants.len()), (6, 3));
        // The pact clears [5][6] and [6][5], but row 6 is rebuilt after row 5 [quirk].
        assert_eq!((b.hostile[5][6], b.hostile[6][5], b.hostile[5][3]), (0, 1, 1));
        b.winner = 6;
        let r = b.finish();
        assert_eq!((r.winner, r.human_took_part, r.participants.len(), r.loser_retreated, b.active), (6, true, 3, false, false));
    }
}
