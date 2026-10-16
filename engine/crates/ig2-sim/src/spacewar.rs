//! Space combat on the strategic side: design stats (`TervUjraSzamol`), fleet totals
//! (`HajoFlottahozHozzaszamol` / `TankFlottahozHozzaszamol`), battle routing (`SpaceWarIndit`), the
//! auto-resolve (`SimulateSpBattle`) with its deletions and the loser's retreat, and the starmap's
//! in-battle marking. Spec: `re/rules/space-combat.md`. **The exe is normative** (precision per its asm).
//!
//! The real-time tactical battle (`SpwC::*`) is specified (§7 of the spec) but not ported; its state
//! enters here only as the list of [`ActiveBattle`] units that mark fleets as in battle.

use crate::rng::Streams;

/// Auto-resolve rolls (`SimulateSpBattle`, `starmap.cpp` lines 0x1f09..0x1fb3).
pub const AUTO_BATTLE_STREAM: usize = 7;
/// `Hajok[i].design` / `Tankok[i].design` of a free slot.
pub const NO_DESIGN: i16 = 0x7fff;
/// Fleet `+0x52` bit 13: the fleet holds a ship of design slot 28 (`HajoFlottahozHozzaszamol`).
pub const FLAG_DESIGN28: u16 = 1 << 13;
const FLAG_IN_BATTLE: u16 = 1;
const FLAG_WORMHOLE: u16 = 1 << 10;
const FLAG_PROTECTED: u16 = 1 << 14;

// ------------------------------------------------------------------------------------------------
// Rules inputs

/// Hull record (`Alapok`, 36 bytes; spec §2.1): the fields design stats read.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Hull {
    /// `+0` i16: 0 none, 1 space (ships and fighters), 2 ground vehicle.
    pub class: i16,
    /// `+2..+7`: colony ship, terraformer, fighter, space base, movable base, satellite (copied raw).
    pub flags: [u8; 6],
    /// `+0xc`: the hull's own tech (its price, time, firepower and hit points are the base stats).
    pub tech: i32,
}

/// What `TervUjraSzamol` reads from the rules tables (all from the user's exe, `ig2-rules`).
#[derive(Debug, Clone, Default)]
pub struct DesignRules {
    pub hulls: Vec<Hull>,
    /// Live tech table `+20` price, `+24` time, `+32` hit points, `+37` (u8) cargo capacity.
    pub tech_price: Vec<i32>,
    pub tech_time: Vec<i32>,
    pub tech_hp: Vec<i32>,
    pub tech_capacity: Vec<u8>,
    /// `TalalmanyokrEro`.
    pub firepower: Vec<i32>,
    /// `HanyasFajPodTablazatatHasznalja`: index → 1-based pod table.
    pub race_pod_table: Vec<i32>,
    /// `PodFajok[table][hull][slot].pod_type` (14 × 27 × 12).
    pub pod_slot_types: Vec<Vec<Vec<i32>>>,
    /// `TankAlapSpeed` (f32 × 5): ground hulls, indexed by hull tech − 0x91.
    pub tank_speeds: Vec<f32>,
}

impl DesignRules {
    /// `GetGyartasAr(empire, tech)` with the empire's race: race 4/7 build tech 13 at ¾ price, race 5
    /// gets ×0.7000001 (a double) on everything outside techs 81..=144 and 13/14.
    pub fn price(&self, race: u8, tech: usize) -> i32 {
        if (race == 4 || race == 7) && tech == 13 {
            return (self.tech_price[13] as f64 * 0.75) as i32;
        }
        if race == 5 && !(81..=144).contains(&tech) && tech != 13 && tech != 14 {
            return (self.tech_price[tech] as f64 * f64::from_bits(0x3FE6_6666_9C16_4B9C)) as i32;
        }
        self.tech_price[tech]
    }

    /// `GetGyartasIdo(empire, tech)`: race 4 builds tech 13 in half the time.
    pub fn time(&self, race: u8, tech: usize) -> i32 {
        if race == 4 && tech == 13 {
            return self.tech_time[13] / 2;
        }
        self.tech_time[tech]
    }

    /// `GetMaxPodok(table_index, hull)`: slots whose pod type is > 0.
    pub fn slot_count(&self, table_index: i16, hull: usize) -> i32 {
        let table = self.race_pod_table.get(table_index.max(0) as usize).copied().unwrap_or(0) - 1;
        let Some(slots) = self.pod_slot_types.get(table.max(0) as usize).and_then(|t| t.get(hull)) else { return 0 };
        slots.iter().filter(|&&t| t > 0).count() as i32
    }
}

// ------------------------------------------------------------------------------------------------
// Designs

/// A design (`Tervek[empire][d]`, `Tervek0` defaults; 0x84 bytes; spec §2.3). `.scn` keys in brackets.
#[derive(Debug, Clone, PartialEq)]
pub struct Design {
    /// The record as read; bytes without a field below are written back unchanged.
    pub raw: [u8; 0x84],
    /// `+0x16` [tTipus] class from the hull: 0 none, 1 space, 2 ground.
    pub class: i16,
    /// `+0x20` [tGyartasIdo] build time, `+0x24` [tAr] price.
    pub build_time: i32,
    pub price: i32,
    /// `+0x28` [tMaxTank] vehicle capacity (u8, wraps).
    pub capacity: u8,
    /// `+0x2c` [tEro] attack (firepower sum), `+0x30` [tHP] hit points.
    pub attack: i32,
    pub hp: i32,
    /// `+0x34..+0x39` [tKolonizalo, tTerraformalo, tVadasz, tUrbazis, tUrbazisMozgathato, tSatellite].
    pub hull_flags: [u8; 6],
    /// `+0x3c` [tSebesseg].
    pub speed: f32,
    /// `+0x40` [tEredetiTulaj] index into `race_pod_table` (the designing empire).
    pub pod_table: i16,
    /// `+0x44` [tAlap] hull, `+0x48` [tAlapTalalmany] hull tech, `+0x4c` [tHanyPod] slot count.
    pub hull: i32,
    pub hull_tech: i32,
    pub slots: i32,
    /// `+0x51..+0x5c` [tPodTalalmany] tech per slot, `+0x5e..+0x69` [tPodDarab] count per slot.
    pub pods: [u8; 12],
    pub counts: [u8; 12],
    /// `+0x6a` [tRadar] radar tech level, `+0x6b` [tFleetCloaking] stealth.
    pub radar: u8,
    pub stealth: u8,
}

fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn rd_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}

impl Design {
    pub fn from_bytes(r: &[u8]) -> Self {
        let raw: [u8; 0x84] = r[..0x84].try_into().unwrap();
        Self {
            raw,
            class: rd_i16(r, 0x16),
            build_time: rd_i32(r, 0x20),
            price: rd_i32(r, 0x24),
            capacity: r[0x28],
            attack: rd_i32(r, 0x2c),
            hp: rd_i32(r, 0x30),
            hull_flags: r[0x34..0x3a].try_into().unwrap(),
            speed: f32::from_le_bytes(r[0x3c..0x40].try_into().unwrap()),
            pod_table: rd_i16(r, 0x40),
            hull: rd_i32(r, 0x44),
            hull_tech: rd_i32(r, 0x48),
            slots: rd_i32(r, 0x4c),
            pods: r[0x51..0x5d].try_into().unwrap(),
            counts: r[0x5e..0x6a].try_into().unwrap(),
            radar: r[0x6a],
            stealth: r[0x6b],
        }
    }

    pub fn to_bytes(&self) -> [u8; 0x84] {
        let mut r = self.raw;
        r[0x16..0x18].copy_from_slice(&self.class.to_le_bytes());
        r[0x20..0x24].copy_from_slice(&self.build_time.to_le_bytes());
        r[0x24..0x28].copy_from_slice(&self.price.to_le_bytes());
        r[0x28] = self.capacity;
        r[0x2c..0x30].copy_from_slice(&self.attack.to_le_bytes());
        r[0x30..0x34].copy_from_slice(&self.hp.to_le_bytes());
        r[0x34..0x3a].copy_from_slice(&self.hull_flags);
        r[0x3c..0x40].copy_from_slice(&self.speed.to_le_bytes());
        r[0x40..0x42].copy_from_slice(&self.pod_table.to_le_bytes());
        r[0x44..0x48].copy_from_slice(&self.hull.to_le_bytes());
        r[0x48..0x4c].copy_from_slice(&self.hull_tech.to_le_bytes());
        r[0x4c..0x50].copy_from_slice(&self.slots.to_le_bytes());
        r[0x51..0x5d].copy_from_slice(&self.pods);
        r[0x5e..0x6a].copy_from_slice(&self.counts);
        r[0x6a] = self.radar;
        r[0x6b] = self.stealth;
        r
    }

    /// Fighters ride in carriers' hangars (`+0x36`, from the hull's fighter flag).
    pub fn is_fighter(&self) -> bool {
        self.hull_flags[2] != 0
    }

    /// `TervUjraSzamol(empire, d, false, recompute_price)` (spec §2.4) for an empire of race `race`.
    /// Without `recompute_price` the pods' prices are added to the stored price (as the original does).
    pub fn recompute(&mut self, rules: &DesignRules, race: u8, recompute_price: bool) {
        let h = rules.hulls[self.hull as usize];
        let tech = h.tech as usize;
        self.class = h.class;
        self.hull_tech = h.tech;
        self.slots = rules.slot_count(self.pod_table, self.hull as usize);
        self.hull_flags = h.flags;
        self.build_time = rules.time(race, tech);
        if recompute_price {
            self.price = rules.price(race, tech);
        }
        self.attack = rules.firepower[tech];
        self.hp = rules.tech_hp[tech];
        self.capacity = rules.tech_capacity[tech];
        self.raw[0x70] = 0;
        self.speed = if self.class < 2 {
            f32::from_bits(0x3d75c28f) // 0.06
        } else {
            let s = rules.tank_speeds.get((self.hull_tech - 0x91) as usize).copied().unwrap_or(0.0);
            (s as f64 / 100.0) as f32
        };
        self.radar = 0;
        self.stealth = 0;
        let slots = self.slots.clamp(0, 12) as usize;
        for k in 0..slots {
            let t = self.pods[k] as usize;
            if t == 0 {
                continue;
            }
            let c = self.counts[k] as i32;
            self.build_time = self.build_time.wrapping_add(rules.time(race, t).wrapping_mul(c));
            self.price = self.price.wrapping_add(rules.price(race, t).wrapping_mul(c));
            self.capacity = self.capacity.wrapping_add(rules.tech_capacity[t].wrapping_mul(c as u8));
            if !(0x31..=0x36).contains(&t) && t != 0x25 {
                self.attack = self.attack.wrapping_add(c.wrapping_mul(rules.firepower[t]));
                self.hp = self.hp.wrapping_add(c.wrapping_mul(rules.tech_hp[t]));
            }
            if (0x39..=0x3d).contains(&t) {
                self.speed = ((t as i32 - 0x38) as f64 * 0.02 + 0.06) as f32;
            }
            if (0x21..=0x24).contains(&t) {
                self.radar = (t - 0x20) as u8;
            }
            if t == 0x38 {
                self.stealth = 1;
            }
        }
        // Shields (0x31..=0x36) add a share of the pre-shield hit points, 0x25 a quarter of the attack;
        // each step truncates (f64).
        let (base_attack, base_hp) = (self.attack as f64, self.hp as f64);
        for &t in &self.pods[..slots] {
            let share = match t {
                0x31 => 0.15,
                0x32 => 0.3,
                0x33 => 0.45,
                0x34 => 0.6,
                0x35 => 0.8,
                0x36 => {
                    self.hp = (self.hp as f64 + base_hp) as i32;
                    continue;
                }
                0x25 => {
                    self.attack = (base_attack * 0.25 + self.attack as f64) as i32;
                    continue;
                }
                _ => continue,
            };
            self.hp = (base_hp * share + self.hp as f64) as i32;
        }
        if race == 2 && self.class == 1 {
            self.speed = (self.speed as f64 + 0.02) as f32;
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Fleet totals (`+0x54..+0x78`)

/// The ship-derived sums of a fleet record (spec §3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FleetTotals {
    /// `+0x54` ships including fighters, `+0x58` fighters, `+0x5c` ground vehicles.
    pub ships: i32,
    pub fighters: i32,
    pub ground: i32,
    /// `+0x60` attack (ships and fighters), `+0x64` fighter attack, `+0x68` ground strength.
    pub attack: i32,
    pub fighter_attack: i32,
    pub ground_strength: i32,
    /// `+0x6c` hit points (ships and fighters), `+0x70` fighter hit points.
    pub hp: i32,
    pub fighter_hp: i32,
    /// `+0x74` (unknown; zeroed by `FlottakUjraSzamol`), `+0x78` vehicle capacity.
    pub field_74: i32,
    pub capacity: i32,
}

impl FleetTotals {
    pub fn from_record(r: &[u8]) -> Self {
        Self {
            ships: rd_i32(r, 0x54),
            fighters: rd_i32(r, 0x58),
            ground: rd_i32(r, 0x5c),
            attack: rd_i32(r, 0x60),
            fighter_attack: rd_i32(r, 0x64),
            ground_strength: rd_i32(r, 0x68),
            hp: rd_i32(r, 0x6c),
            fighter_hp: rd_i32(r, 0x70),
            field_74: rd_i32(r, 0x74),
            capacity: rd_i32(r, 0x78),
        }
    }

    /// `HajoFlottahozHozzaszamol` sums (`n` ships of design `d`; negative removes).
    pub fn add_ship(&mut self, d: &Design, n: i32) {
        self.ships += n;
        self.capacity += d.capacity as i32 * n;
        self.attack += d.attack * n;
        self.hp += d.hp * n;
    }

    /// `TankFlottahozHozzaszamol` for a fleet: ground vehicles add `(f32)sqrt(attack × hp)` per unit
    /// (f32 accumulate, truncated), anything else counts as fighters.
    pub fn add_vehicle(&mut self, d: &Design, n: i32) {
        if d.class == 2 {
            self.ground += n;
            let s = ((d.hp.wrapping_mul(d.attack)) as f32 as f64).sqrt() as f32;
            self.ground_strength = (s * n as f32 + self.ground_strength as f32) as i32;
        } else {
            self.ships += n;
            self.attack += d.attack * n;
            self.fighters += n;
            self.hp += d.hp * n;
            self.fighter_attack += d.attack * n;
            self.fighter_hp += d.hp * n;
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Routing (`SpaceWarIndit`)

/// One fleet in a starting battle, as `SpaceWarIndit` sees it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Contender {
    pub owner: u8,
    pub story_id: i16,
    /// Fleet `+0x60` and `+0x6c`.
    pub attack: i32,
    pub hp: i32,
}

/// Inputs of the battle routing decision (spec §4).
#[derive(Debug, Clone, Default)]
pub struct RouteInput {
    /// The attacker, then the defending fleet if any (`SpWarHanyFlotta` entries).
    pub fleets: Vec<Contender>,
    /// The owner of the battle planet (`spKozeliBolygo`; 0 if none) and its ion-gun strength.
    pub planet_owner: u8,
    pub ion_gun: i32,
    pub human_controlled: [bool; 19],
    pub multiplayer: bool,
    /// `MultiplayerBattleStatusz`.
    pub mp_battle_status: i32,
    /// Debug switches (exe 0xB77230: always simulate, 0xB77238: never lopsided).
    pub debug_always_simulate: bool,
    pub debug_never_lopsided: bool,
    /// `Story_SpWarSimulateTiltas`: a story forbids auto-resolving.
    pub story_simulate_ban: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// `SpaceWar_NewBattle`: the real-time battle (both fleets' orders are cleared first).
    Tactical,
    /// `SimulateSpBattle`. `lopsided`: the strength ratio was beyond 5:1 either way (message 0x0F/0x10
    /// instead of 0x11/0x12 for the human in SP).
    Simulate { lopsided: bool },
}

/// The strength ratio `sqrt(att₁·hp₁ + 0.1) / (sqrt(att₂·hp₂) + ion + 0.1)` with the exe's precision.
pub fn strength_ratio(fleets: &[Contender], ion_gun: i32) -> f32 {
    let ion = ion_gun as f32;
    let a = &fleets[0];
    let r1 = ((a.hp as f32 * a.attack as f32) as f64 + 0.1).sqrt();
    match fleets.get(1) {
        None => (r1 / (ion as f64 + 0.1)) as f32,
        Some(b) => {
            let d = ((b.hp as f32 * b.attack as f32) as f64).sqrt() as f32 + ion;
            (r1 / (d as f64 + 0.1)) as f32
        }
    }
}

/// `SpaceWarIndit`'s choice between the real-time battle and the auto-resolve (spec §4). Battles with
/// no human-controlled side, or lopsided ones (ratio > 5 or < 0.2), are auto-resolved in SP.
pub fn route(inp: &RouteInput) -> Route {
    let human = |e: u8| inp.human_controlled.get(e as usize).copied().unwrap_or(false);
    let po = if inp.planet_owner != 0 && human(inp.planet_owner) { inp.planet_owner } else { 0 };
    let mut any_human = inp.fleets.iter().any(|f| human(f.owner));
    let mut humans = inp.fleets.iter().filter(|f| human(f.owner) && (f.owner != po || po == 0)).count();
    if po != 0 {
        humans += 1;
    }
    if inp.planet_owner != 0 {
        any_human |= human(inp.planet_owner);
    }
    let mut lopsided = false;
    if inp.fleets.len() < 3 {
        let r = strength_ratio(&inp.fleets, inp.ion_gun);
        lopsided = !inp.debug_never_lopsided && (r > 5.0 || 0.2 > r as f64) && !inp.debug_always_simulate;
        if inp.fleets.iter().any(|f| (10_000..11_000).contains(&f.story_id)) {
            lopsided = false;
        }
    }
    let tactical = inp.fleets.len() > 2
        || (((!inp.debug_always_simulate && !lopsided) || inp.story_simulate_ban) && any_human);
    let mp_tactical = || (inp.mp_battle_status == 0 || inp.mp_battle_status == 1) && any_human && humans == 2;
    let tactical = if inp.multiplayer { mp_tactical() } else { tactical };
    if tactical { Route::Tactical } else { Route::Simulate { lopsided } }
}

// ------------------------------------------------------------------------------------------------
// Auto-resolve (`SimulateSpBattle`)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    /// A ship (`Hajok` index).
    Ship = 1,
    /// One fighter of a `Tankok` entry (class-1 vehicle design).
    Fighter = 2,
    /// A planet defence ship (planet `+0xfe + 2k`).
    PlanetShip = 3,
    /// A finished planet gun (building types 28..=31, slot index).
    PlanetGun = 4,
}

/// One combatant: `hp` is design hit points × 30 (the tactical battle uses × 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    pub id: usize,
    pub attack: i32,
    pub hp: i32,
    pub kind: UnitKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoOutcome {
    /// 1 = attacker side, 2 = defender side.
    pub winner: u8,
    /// Only fighters (or nothing) left facing only planet guns (or nothing): neither can hurt the other.
    pub stalemate: bool,
    /// Final unit states of both sides (hp < 1 = destroyed).
    pub sides: [Vec<Unit>; 2],
}

fn incompatible(a: UnitKind, b: UnitKind) -> bool {
    matches!((a, b), (UnitKind::PlanetGun, UnitKind::Fighter) | (UnitKind::Fighter, UnitKind::PlanetGun))
}

/// The battle loop of `SimulateSpBattle` (spec §5.2). Units fire in a fixed round-robin from a random
/// start; damage carries over to further targets; each side focuses one target until it dies.
pub fn auto_resolve(side1: Vec<Unit>, side2: Vec<Unit>, rng: &mut Streams) -> AutoOutcome {
    const S: usize = AUTO_BATTLE_STREAM;
    let mut u = [side1, side2];
    let n = [u[0].len() as i32, u[1].len() as i32];
    let count = |u: &[Unit], k: UnitKind| u.iter().filter(|x| x.kind == k).count() as i32;
    let mut alive = n;
    let mut fighters = [count(&u[0], UnitKind::Fighter), count(&u[1], UnitKind::Fighter)];
    let mut guns = [count(&u[0], UnitKind::PlanetGun), count(&u[1], UnitKind::PlanetGun)];
    rng.range(S, 0, 100);
    let mut e = rng.range(S, 1, n[0] + n[1]);
    let total: i64 = u.iter().flatten().map(|x| x.attack as i64).sum();
    let stale_now = |a: [i32; 2], f: [i32; 2], g: [i32; 2]| {
        (a[0] - f[0] < 1 && a[1] - g[1] < 1) || (a[1] - f[1] < 1 && a[0] - g[0] < 1)
    };
    let mut stalemate = stale_now(alive, fighters, guns);
    // Current target per side (1-based, 0 = pick a new one).
    let mut cur = [0usize; 2];
    let hp_of = |u: &[Vec<Unit>; 2], s: usize, i: usize| if i == 0 { 0 } else { u[s][i - 1].hp };
    if total > 0 {
        while alive[0] > 0 && alive[1] > 0 && !stalemate {
            let (s, i) = loop {
                e = e % (n[0] + n[1]) + 1;
                let (s, i) = if e > n[0] { (1, (e - n[0]) as usize) } else { (0, e as usize) };
                if u[s][i - 1].hp >= 1 {
                    break (s, i);
                }
            };
            let o = 1 - s;
            let kind = u[s][i - 1].kind;
            let mut dmg = u[s][i - 1].attack;
            loop {
                let t;
                if cur[o] == 0 {
                    let r = rng.range(S, 1, alive[o]);
                    let mut q = r;
                    loop {
                        let mut c = q;
                        let mut j = 1;
                        while j <= n[o] && c > 0 {
                            if u[o][j as usize - 1].hp > 0 {
                                c -= 1;
                            }
                            j += 1;
                        }
                        cur[o] = (j - 1) as usize;
                        q = q % alive[o] + 1;
                        if !(incompatible(kind, u[o][cur[o] - 1].kind) && q != r) {
                            break;
                        }
                    }
                    t = cur[o];
                    if incompatible(kind, u[o][t - 1].kind) {
                        dmg = 0;
                    }
                } else {
                    t = cur[o];
                }
                let target = &mut u[o][t - 1];
                let mut d = dmg.min(target.hp);
                if incompatible(kind, target.kind) {
                    d = 0;
                    cur[o] = 0;
                }
                target.hp -= d;
                dmg -= d;
                if target.hp < 1 {
                    alive[o] -= 1;
                    match target.kind {
                        UnitKind::Fighter => fighters[o] -= 1,
                        UnitKind::PlanetGun => guns[o] -= 1,
                        _ => {}
                    }
                }
                for (side, c) in cur.iter_mut().enumerate() {
                    if hp_of(&u, side, *c) < 1 {
                        *c = 0;
                    }
                }
                if alive[0] > 0 && alive[1] > 0 && stale_now(alive, fighters, guns) {
                    stalemate = true;
                }
                if !(dmg > 0 && alive[0] > 0 && alive[1] > 0 && !stalemate) {
                    break;
                }
            }
        }
    }
    let mut winner = if alive[0] > 0 { 1 } else { 2 };
    if stalemate {
        if alive[0] - fighters[0] < 1 && alive[1] - guns[1] < 1 {
            winner = 2;
        }
        if alive[1] - fighters[1] < 1 && alive[0] - guns[0] < 1 {
            winner = 1;
        }
    }
    AutoOutcome { winner, stalemate, sides: u }
}

// ------------------------------------------------------------------------------------------------
// The strategic state a battle touches

/// `Hajok` entry (6 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ship {
    pub fleet: i16,
    pub design: i16,
}

/// `Tankok` entry (8 bytes): `count` vehicles of one design; fleet ≤ 0 = held by planet −fleet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vehicle {
    pub fleet: i16,
    pub design: i16,
    pub count: i32,
}

/// The parts of a fleet record that battles read and write.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CombatFleet {
    pub owner: u8,
    pub flags: u16,
    pub x: f32,
    pub y: f32,
    pub totals: FleetTotals,
    pub colony_ships: i16,
    pub terraformers: i16,
}

impl CombatFleet {
    pub fn alive(&self) -> bool {
        self.owner != 0xff
    }
}

/// Defence buildings of the battle planet (`Epuletek[p]` slot → building type, finished).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefenceSlot {
    pub slot: usize,
    pub kind: u8,
    pub finished: bool,
}

/// Side effects for other systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatEvent {
    /// `EpuletDelete(planet, slot)`.
    BuildingDestroyed { planet: usize, slot: usize },
    /// `FlottaDelete` ran (`OsszHanySajatFlotta[owner] -= 1`; it can run twice for one fleet).
    FleetDeleted { fleet: usize, owner: u8 },
    /// Fleets following a deleted fleet lose their order (`FlottaMozgat(f, 0, …)`).
    FollowerStopped { fleet: usize },
    /// MP: the human's emptied fleet is reported over the network (`Multi_Send(0x1b)`).
    HumanFleetEmptied { fleet: usize },
    /// The planet lost vehicles it holds (fleet ≤ 0): the planet's own sums, not modelled here.
    PlanetVehiclesLost { planet: usize, design: i16, count: i32 },
}

/// Ships, vehicles, fleets and designs: all 1-based like the original (index 0 kept).
#[derive(Debug, Clone, Default)]
pub struct CombatWorld {
    pub fleets: Vec<CombatFleet>,
    pub ships: Vec<Ship>,
    pub vehicles: Vec<Vehicle>,
    /// `Tervek[empire]` (index 0 of each table unused).
    pub designs: Vec<Vec<Design>>,
    pub human_controlled: [bool; 19],
    pub colony_ship_pool: [i32; 19],
    pub terraformer_pool: [i32; 19],
    /// Fleets with an attack order on another fleet (`+0x40 > 0`, `+0x42 == 2`) and its target (`+0x44`).
    pub follow_targets: Vec<(usize, f32)>,
    pub events: Vec<CombatEvent>,
}

/// The battle planet's defences.
#[derive(Debug, Clone, Default)]
pub struct BattlePlanet {
    pub index: usize,
    pub owner: u8,
    /// `+0x100 + 2k` design ids (`+0x114` count).
    pub ships: Vec<i16>,
    pub buildings: Vec<DefenceSlot>,
}

/// Rule values the auto-resolve reads for planet guns.
#[derive(Debug, Clone, Default)]
pub struct GunRules {
    /// `EpuletekLeiro[type].+0` tech per building type.
    pub building_tech: Vec<u8>,
    pub firepower: Vec<i32>,
    pub tech_hp: Vec<i32>,
}

fn design_of(designs: &[Vec<Design>], owner: u8, d: i16) -> Option<&Design> {
    designs.get(owner as usize)?.get(d as usize)
}

impl CombatWorld {
    fn owner(&self, f: i16) -> u8 {
        self.fleets.get(f.max(0) as usize).map_or(0xff, |x| x.owner)
    }

    fn ship_totals(&mut self, f: i16, d: i16, n: i32) {
        let Some(fl) = self.fleets.get(f.max(0) as usize).copied() else { return };
        if f < 0 || !fl.alive() {
            return;
        }
        let Some(des) = design_of(&self.designs, fl.owner, d).cloned() else { return };
        let fl = &mut self.fleets[f as usize];
        fl.totals.add_ship(&des, n);
        if des.hull_flags[0] != 0 {
            fl.colony_ships = fl.colony_ships.wrapping_add(n as i16);
            self.colony_ship_pool[fl.owner as usize] += n;
        }
        if des.hull_flags[1] != 0 {
            fl.terraformers = fl.terraformers.wrapping_add(n as i16);
            self.terraformer_pool[fl.owner as usize] += n;
        }
        if d == 28 {
            if n > 0 {
                fl.flags |= FLAG_DESIGN28;
            } else if n < 0 {
                fl.flags &= !FLAG_DESIGN28;
            }
        }
    }

    /// `TankDelete(fleet, design, n)`: the lowest-index matching entry loses up to `n` vehicles.
    pub fn delete_vehicles(&mut self, fleet: i16, design: i16, n: i32) {
        let Some(i) = (1..self.vehicles.len()).find(|&i| self.vehicles[i].fleet == fleet && self.vehicles[i].design == design)
        else {
            return;
        };
        let n = n.min(self.vehicles[i].count);
        self.vehicles[i].count -= n;
        if self.vehicles[i].count < 1 {
            self.vehicles[i].design = NO_DESIGN;
        }
        if fleet < 1 {
            self.events.push(CombatEvent::PlanetVehiclesLost { planet: (-fleet) as usize, design, count: n });
            return;
        }
        let fl = self.fleets[fleet as usize];
        if !fl.alive() {
            return;
        }
        if let Some(des) = design_of(&self.designs, fl.owner, design).cloned() {
            self.fleets[fleet as usize].totals.add_vehicle(&des, -n);
        }
    }

    /// `HajoDelete(i)`: the ship goes, ground vehicles beyond the fleet's capacity go (every matching
    /// non-fighter entry loses up to the excess: the excess isn't reduced as entries are cut), and an
    /// emptied non-human fleet is deleted.
    pub fn delete_ship(&mut self, i: usize) {
        let Ship { fleet: f, design: d } = self.ships[i];
        if d == NO_DESIGN {
            return;
        }
        self.ships[i].design = NO_DESIGN;
        self.ship_totals(f, d, -1);
        let fu = f.max(0) as usize;
        let Some(fl) = self.fleets.get(fu).copied() else { return };
        let excess = fl.totals.ground - fl.totals.capacity;
        let owner = fl.owner;
        let mut k = 1;
        while k < self.vehicles.len() && excess > 0 {
            let v = self.vehicles[k];
            if v.design != NO_DESIGN && v.fleet == f {
                let fighter = design_of(&self.designs, owner, v.design).is_some_and(Design::is_fighter);
                if !fighter {
                    self.delete_vehicles(f, v.design, excess.min(v.count));
                }
            }
            k += 1;
        }
        let fl = self.fleets[fu];
        let special = fl.flags & (FLAG_WORMHOLE | FLAG_PROTECTED) != 0;
        if fl.totals.ships < 1 && !special && self.human_controlled.get(fl.owner as usize).copied().unwrap_or(false) {
            self.events.push(CombatEvent::HumanFleetEmptied { fleet: fu });
        }
        if fl.totals.ships < 1 && !special && !self.human_controlled.get(fl.owner as usize).copied().unwrap_or(false) {
            self.delete_fleet(fu);
        }
    }

    /// `FlottaDelete(f)` (wormholes excepted): followers stop, every ship and vehicle goes, the slot
    /// is freed. It re-enters itself through `HajoDelete` when the last ship goes.
    pub fn delete_fleet(&mut self, f: usize) {
        let fl = self.fleets[f];
        if !fl.alive() || fl.flags & FLAG_WORMHOLE != 0 {
            return;
        }
        for &(g, t) in &self.follow_targets {
            if t == f as f32 && self.fleets.get(g).is_some_and(CombatFleet::alive) {
                self.events.push(CombatEvent::FollowerStopped { fleet: g });
            }
        }
        for i in 1..self.ships.len() {
            if self.ships[i].fleet as usize == f && self.ships[i].fleet >= 0 && self.ships[i].design != NO_DESIGN {
                self.delete_ship(i);
            }
        }
        for i in 1..self.vehicles.len() {
            let v = self.vehicles[i];
            if v.fleet >= 0 && v.fleet as usize == f && v.design != NO_DESIGN {
                self.delete_vehicles(v.fleet, v.design, v.count);
            }
        }
        self.events.push(CombatEvent::FleetDeleted { fleet: f, owner: self.fleets[f].owner });
        self.fleets[f].owner = 0xff;
    }

    /// `FlottaArrebPakol(f)`: the loser backs off 6 units towards its empire's centre (f32).
    pub fn retreat(&mut self, f: usize, centre: (i32, i32)) {
        let fl = &mut self.fleets[f];
        if !fl.alive() {
            return;
        }
        let dy = centre.1 as f32 - fl.y;
        let dx = centre.0 as f32 - fl.x;
        let len = ((dy * dy + dx * dx) as f64).sqrt() as f32 + 0.001;
        fl.x += (dx * 6.0) / len;
        fl.y += (dy * 6.0) / len;
    }

    /// Units of one side in the original order: the fleet's ships, then its fighters (one unit per
    /// vehicle). `fleet` is compared literally (side 2 of a planet battle is fleet 0).
    fn side_units(&self, fleet: i16, owner: u8, out: &mut Vec<Unit>) {
        for (i, s) in self.ships.iter().enumerate().skip(1) {
            if s.design != NO_DESIGN && s.fleet == fleet {
                let d = design_of(&self.designs, owner, s.design).expect("ship design");
                out.push(Unit { id: i, attack: d.attack, hp: d.hp * 30, kind: UnitKind::Ship });
            }
        }
        for (i, v) in self.vehicles.iter().enumerate().skip(1) {
            if v.design == NO_DESIGN || v.fleet != fleet {
                continue;
            }
            let Some(d) = design_of(&self.designs, owner, v.design) else { continue };
            if d.class == 1 {
                for _ in 0..v.count.max(0) {
                    out.push(Unit { id: i, attack: d.attack, hp: d.hp * 30, kind: UnitKind::Fighter });
                }
            }
        }
    }

    /// `SimulateSpBattle(f1, f2, planet)` (spec §5): build both sides, fight, delete the destroyed,
    /// back the loser off, delete fleets without hit points. Returns the outcome; the winner's empire
    /// is `SpWarKiNyert`.
    pub fn simulate(&mut self, f1: usize, f2: usize, planet: Option<&mut BattlePlanet>, guns: &GunRules,
        centres: &[(i32, i32)], rng: &mut Streams) -> AutoOutcome {
        let o1 = self.owner(f1 as i16);
        let o2 = if f2 != 0 { self.owner(f2 as i16) } else { planet.as_ref().map_or(0, |p| p.owner) };
        let mut side = [Vec::new(), Vec::new()];
        self.side_units(f1 as i16, o1, &mut side[0]);
        self.side_units(f2 as i16, o2, &mut side[1]);
        let mut planet = planet;
        if let Some(p) = planet.as_deref() {
            for (k, &d) in p.ships.iter().enumerate() {
                let des = design_of(&self.designs, o2, d).expect("planet ship design");
                side[1].push(Unit { id: k + 1, attack: des.attack, hp: des.hp * 30, kind: UnitKind::PlanetShip });
            }
            for b in &p.buildings {
                if b.finished && (28..32).contains(&b.kind) {
                    let tech = guns.building_tech[b.kind as usize] as usize;
                    side[1].push(Unit { id: b.slot, attack: guns.firepower[tech], hp: guns.tech_hp[tech] * 30, kind: UnitKind::PlanetGun });
                }
            }
        }
        let [s1, s2] = side;
        let out = auto_resolve(s1, s2, rng);
        for units in &out.sides {
            for x in units.iter().filter(|x| x.hp < 1) {
                match x.kind {
                    UnitKind::Ship => self.delete_ship(x.id),
                    UnitKind::Fighter => {
                        let v = self.vehicles[x.id];
                        self.delete_vehicles(v.fleet, v.design, 1);
                    }
                    UnitKind::PlanetShip => {
                        if let Some(p) = planet.as_deref_mut() {
                            p.ships[x.id - 1] = 0;
                        }
                    }
                    UnitKind::PlanetGun => {
                        if let Some(p) = planet.as_deref() {
                            self.events.push(CombatEvent::BuildingDestroyed { planet: p.index, slot: x.id });
                        }
                    }
                }
            }
        }
        if let Some(p) = planet {
            p.ships.retain(|&d| d != 0);
        }
        let loser_fleet = if out.winner == 1 { f2 } else { f1 };
        let lo = self.fleets[loser_fleet].owner;
        if lo != 0xff {
            let c = centres.get(lo as usize).copied().unwrap_or((0, 0));
            self.retreat(loser_fleet, c);
        }
        if self.fleets[f1].totals.hp < 1 {
            self.delete_fleet(f1);
        }
        if f2 != 0 && self.fleets[f2].totals.hp < 1 {
            self.delete_fleet(f2);
        }
        out
    }
}

// ------------------------------------------------------------------------------------------------
// In-battle marking (`FRISSIT_Starmap`, after the radar block)

/// A unit of a real-time battle (`urhajotipus`, exe 600 bytes: `+0` alive, `+0x14` owner, `+0x28` ship
/// list, `+0x1b0` its length).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BattleUnit {
    pub alive: u8,
    /// 0 for scenery objects (designs ≥ 0x8000).
    pub owner: i32,
    pub ships: Vec<i32>,
}

/// An active battle slot (`spw[b]`, 10 slots; exe active byte 0xE436EE + b·0x2D9C).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActiveBattle {
    pub units: Vec<BattleUnit>,
}

/// The fleets the starmap marks as in battle this tick (spec §6): the fleet of every ship of every
/// live unit with an owner, in battle order. The fleet index is read from `Hajok` as is (a freed ship
/// keeps its fleet), so it can name slot 0 or a freed slot; the bit is ORed into that record anyway.
pub fn fleets_in_battle(battles: &[ActiveBattle], ships: &[Ship]) -> Vec<usize> {
    let mut out = Vec::new();
    for u in battles.iter().flat_map(|b| &b.units) {
        if u.alive == 0 || u.owner == 0 {
            continue;
        }
        for &s in u.ships.iter().filter(|&&s| s >= 0) {
            if let Some(sh) = ships.get(s as usize) {
                out.push(sh.fleet.max(0) as usize);
            }
        }
    }
    out
}

/// Apply the marking to `+0x52` flags (`flags[f] |= 1`).
pub fn mark_in_battle(flags: &mut [u16], in_battle: &[usize]) {
    for &f in in_battle {
        if let Some(x) = flags.get_mut(f) {
            *x |= FLAG_IN_BATTLE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(kind: UnitKind, attack: i32, hp: i32) -> Unit {
        Unit { id: 0, attack, hp, kind }
    }

    #[test]
    fn fighters_against_guns_is_a_stalemate() {
        let mut rng = Streams::default();
        let out = auto_resolve(vec![unit(UnitKind::Fighter, 10, 30)], vec![unit(UnitKind::PlanetGun, 50, 300)], &mut rng);
        assert!(out.stalemate);
        // Side 1 has only fighters and side 2 only guns: side 2 holds.
        assert_eq!(out.winner, 2);
        assert_eq!(out.sides[0][0].hp, 30);
    }

    #[test]
    fn damage_carries_over_and_rolls_stream_7() {
        let mut rng = Streams::default();
        let a = vec![unit(UnitKind::Ship, 100, 3000)];
        let b = vec![unit(UnitKind::Ship, 1, 30), unit(UnitKind::Ship, 1, 30)];
        let out = auto_resolve(a, b, &mut rng);
        assert_eq!(out.winner, 1);
        assert!(out.sides[1].iter().all(|u| u.hp < 1));
        assert_ne!(rng.state(AUTO_BATTLE_STREAM), 0);
        assert_eq!(rng.state(1), 0);
    }

    #[test]
    fn no_attack_no_fight() {
        let mut rng = Streams::default();
        let out = auto_resolve(vec![unit(UnitKind::Ship, 0, 30)], vec![unit(UnitKind::Ship, 0, 30)], &mut rng);
        assert_eq!(out.winner, 1);
        assert_eq!(out.sides[1][0].hp, 30);
    }

    #[test]
    fn routing() {
        let mut inp = RouteInput {
            fleets: vec![Contender { owner: 1, attack: 100, hp: 100, story_id: 0 }, Contender { owner: 2, attack: 100, hp: 100, story_id: 0 }],
            ..Default::default()
        };
        assert_eq!(route(&inp), Route::Simulate { lopsided: false }); // no human
        inp.human_controlled[1] = true;
        assert_eq!(route(&inp), Route::Tactical);
        inp.fleets[0].attack = 10_000;
        assert_eq!(route(&inp), Route::Simulate { lopsided: true });
        inp.story_simulate_ban = true;
        assert_eq!(route(&inp), Route::Tactical);
        inp.story_simulate_ban = false;
        inp.fleets[1].story_id = 10_005; // traders are never lopsided
        assert_eq!(route(&inp), Route::Tactical);
        inp.multiplayer = true; // MP: only two human sides fight in real time
        assert_eq!(route(&inp), Route::Simulate { lopsided: false });
    }

    #[test]
    fn marking_skips_dead_and_ownerless_units() {
        let ships = vec![Ship { fleet: 0, design: 0 }, Ship { fleet: 3, design: 1 }, Ship { fleet: 5, design: NO_DESIGN }];
        let b = ActiveBattle {
            units: vec![
                BattleUnit { alive: 2, owner: 1, ships: vec![1, 2, -1] },
                BattleUnit { alive: 0, owner: 1, ships: vec![1] },
                BattleUnit { alive: 1, owner: 0, ships: vec![2] },
            ],
        };
        let f = fleets_in_battle(&[b], &ships);
        assert_eq!(f, vec![3, 5]);
        let mut flags = vec![0u16; 6];
        mark_in_battle(&mut flags, &f);
        assert_eq!(flags, vec![0, 0, 0, 1, 0, 1]);
    }

    #[test]
    fn retreat_moves_six_units_towards_centre() {
        let mut w = CombatWorld { fleets: vec![CombatFleet::default(), CombatFleet { owner: 1, x: 10.0, y: 0.0, ..Default::default() }], ..Default::default() };
        w.retreat(1, (0, 0));
        assert!((w.fleets[1].x - 4.0).abs() < 1e-3);
    }
}
