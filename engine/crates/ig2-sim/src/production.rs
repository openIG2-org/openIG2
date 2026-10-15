//! Planet production queues (the production block at the end of `FRISSIT_Starmap`, exe
//! 0x597322..0x597aac) and finished units (`Legyart` 0x59d740 with `SearchBolygohozKozeliFlotta`
//! 0x540f50, `FlottaNew` 0x53d400, `HajoNew` 0x53e800, `TankNew` 0x541750).
//! Spec: `re/rules/production-research.md` §3–§5.
//!
//! Not ported: the colony UI refresh flags and the ground-war placement of new garrison vehicles
//! (reported as [`ProductionEvent`]s); freed waypoint chains are left to the fleet port.

use crate::colony_side::{Treasury, EMPIRES};
use crate::fleet::{flag, order, Fleet, DELETED};
use crate::spacewar::{Design, FleetTotals, Ship, Vehicle, FLAG_DESIGN28, NO_DESIGN};

/// Queue lines: 0 ships, 1 vehicles.
pub const LINES: usize = 2;
/// Entries per line.
pub const SLOTS: usize = 4;
/// `Tankok` holds at most this many entries (`TankNew` drops units beyond it).
pub const MAX_VEHICLE_ENTRIES: usize = 1000;
/// Station slots in the planet record (`+0x100..+0x114`).
pub const STATION_SLOTS: usize = 10;
/// Search radius for a delivery fleet (Manhattan).
const DELIVERY_RANGE: f32 = 6.0;

/// The production queue (`+0x78..+0xb8`, 64 bytes; reset from `pGyartas0`). Bytes without a field
/// (`+0xaa`, `+0xab`, `+0xb5..`) are kept from `raw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queue {
    pub raw: [u8; 64],
    /// `+0x78 + 8L + 2k`: design (≤ 0 = empty).
    pub design: [[i16; SLOTS]; LINES],
    /// `+0x88 + 4L + k`: units to build.
    pub count: [[u8; SLOTS]; LINES],
    /// `+0x90 + 4L + k`: re-queue at the end when done (tested `!= 0`).
    pub repeat: [[u8; SLOTS]; LINES],
    /// `+0x98 + 4L + k`: UI byte, rotated with the queue.
    pub ui: [[u8; SLOTS]; LINES],
    /// `+0xa0 + 4L`: build time left on the head unit.
    pub left: [i32; LINES],
    /// `+0xa8 + L`: units of the head entry built so far.
    pub built: [u8; LINES],
    /// `+0xac`: rally planet for new ship fleets (0 = none).
    pub rally: u8,
    /// `+0xad + 4L + k`: deliver into a fleet (only line 1 slot 0 is read).
    pub into_fleet: [[u8; SLOTS]; LINES],
}

impl Queue {
    pub fn from_bytes(r: &[u8]) -> Self {
        let raw: [u8; 64] = r[..64].try_into().unwrap();
        let b = |o: usize| raw[o - 0x78];
        let i16_at = |o: usize| i16::from_le_bytes([b(o), b(o + 1)]);
        let i32_at = |o: usize| i32::from_le_bytes([b(o), b(o + 1), b(o + 2), b(o + 3)]);
        let grid = |base: usize| [0, 1].map(|l| [0, 1, 2, 3].map(|k| b(base + 4 * l + k)));
        Self {
            raw,
            design: [0, 1].map(|l| [0, 1, 2, 3].map(|k| i16_at(0x78 + 8 * l + 2 * k))),
            count: grid(0x88),
            repeat: grid(0x90),
            ui: grid(0x98),
            left: [i32_at(0xa0), i32_at(0xa4)],
            built: [b(0xa8), b(0xa9)],
            rally: b(0xac),
            into_fleet: grid(0xad),
        }
    }

    pub fn to_bytes(&self) -> [u8; 64] {
        let mut r = self.raw;
        let mut put = |o: usize, v: &[u8]| r[o - 0x78..o - 0x78 + v.len()].copy_from_slice(v);
        for l in 0..LINES {
            for k in 0..SLOTS {
                put(0x78 + 8 * l + 2 * k, &self.design[l][k].to_le_bytes());
                put(0x88 + 4 * l + k, &[self.count[l][k]]);
                put(0x90 + 4 * l + k, &[self.repeat[l][k]]);
                put(0x98 + 4 * l + k, &[self.ui[l][k]]);
                put(0xad + 4 * l + k, &[self.into_fleet[l][k]]);
            }
            put(0xa0 + 4 * l, &self.left[l].to_le_bytes());
            put(0xa8 + l, &[self.built[l]]);
        }
        put(0xac, &[self.rally]);
        r
    }

    /// Slots 1..=3 move down one; the UI bytes are rotated by swaps, the last design is cleared.
    fn shift(&mut self, l: usize) {
        for k in 0..SLOTS - 1 {
            self.design[l][k] = self.design[l][k + 1];
            self.count[l][k] = self.count[l][k + 1];
            self.repeat[l][k] = self.repeat[l][k + 1];
            self.into_fleet[l][k] = self.into_fleet[l][k + 1];
            self.ui[l].swap(k, k + 1);
        }
        self.design[l][SLOTS - 1] = 0;
    }

    /// The head entry is done (spec §3 step 3): advance, skip empty entries, re-queue a repeat.
    fn advance(&mut self, l: usize) {
        let (d, n, rep, into) = (self.design[l][0], self.count[l][0], self.repeat[l][0], self.into_fleet[l][0]);
        self.shift(l);
        self.built[l] = 0;
        while self.design[l][0] > 0 && self.count[l][0] == 0 {
            self.shift(l);
        }
        if rep != 0 {
            let mut k = SLOTS - 1;
            while k > 0 && self.design[l][k - 1] == 0 {
                k -= 1;
            }
            self.design[l][k] = d;
            self.count[l][k] = n;
            self.repeat[l][k] = rep;
            self.into_fleet[l][k] = into;
            self.ui[l].swap(k, SLOTS - 1);
        }
    }
}

/// The parts of a planet record production reads and writes.
#[derive(Debug, Clone, PartialEq)]
pub struct YardPlanet {
    pub owner: u8,
    pub x: f32,
    pub y: f32,
    /// `+0x64`: non-zero stops production.
    pub halted: u8,
    /// `+0x70 / +0x74`: capacity of the ship / vehicle line.
    pub capacity: [i32; LINES],
    pub queue: Queue,
    /// `+0x58 / +0x5c`: garrison vehicles and their strength.
    pub garrison: i32,
    pub garrison_strength: i32,
    /// `+0x100 + 2(k − 1)`: space stations `1..=station_count` (`+0x114`).
    pub stations: [i16; STATION_SLOTS],
    pub station_count: i32,
}

/// A fleet record with its unit sums; `fleet.ship_count` / `fleet.strength` mirror `totals.ships` /
/// `totals.attack` (`+0x54` / `+0x60`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct YardFleet {
    pub fleet: Fleet,
    pub totals: FleetTotals,
}

impl YardFleet {
    fn sync(&mut self) {
        self.fleet.ship_count = self.totals.ships;
        self.fleet.strength = self.totals.attack;
    }
}

/// Planets, fleets, ships, vehicles and designs; all 1-based like the original (index 0 kept).
#[derive(Debug, Clone, Default)]
pub struct Shipyard {
    pub planets: Vec<YardPlanet>,
    /// `Flottak[0..=OsszHanyFlotta]`.
    pub fleets: Vec<YardFleet>,
    /// `Hajok[0..=OsszHanyHajo]`.
    pub ships: Vec<Ship>,
    /// `Tankok[0..=OsszHanyTank]`.
    pub vehicles: Vec<Vehicle>,
    /// `Tervek[empire][d]`.
    pub designs: Vec<Vec<Design>>,
    /// `BolygoUtolsoKozeliFlotta[p]`: the planet's last delivery fleet.
    pub last_fleet: Vec<i32>,
    /// `OsszHanySajatFlotta[e]`: fleet name counter.
    pub fleet_numbers: [i32; EMPIRES],
    /// `OsszHanyKolonizalo` / `OsszHanyTerraformalo`.
    pub colony_ship_pool: [i32; EMPIRES],
    pub terraformer_pool: [i32; EMPIRES],
}

/// Global inputs of a production tick.
#[derive(Debug, Clone, Default)]
pub struct ProductionContext {
    /// `IdoSpeedSzorzo`.
    pub speed_mult: i32,
    /// `Nap` (1..=30).
    pub day: i32,
    pub multiplayer: bool,
    /// `PlayerKi[e] == 1`.
    pub human_controlled: [bool; EMPIRES],
    /// `NehezsegGyartasSzorzo[Nehezseg]`: AI production speed.
    pub difficulty_production: f32,
    /// `NehezsegPenzSzorzo[Nehezseg]`: AI income (`MoneyHozzaad`).
    pub difficulty_money: f32,
    /// `AlapFlottaNev`: the default fleet name new units' fleets get.
    pub default_fleet_name: Vec<u8>,
    /// String 17/5: a fleet created under this name is numbered (normally equal to the above).
    pub numbered_fleet_name: Vec<u8>,
}

/// Side effects for other systems and the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionEvent {
    /// A unit left the queue (`Legyart`); `fleet` is the delivery fleet search result.
    Built { planet: usize, line: usize, design: i16, fleet: usize },
    /// A new fleet (`FlottaNew`).
    FleetCreated { fleet: usize },
    /// A new ship slot (`HajoNew`; its `+4` byte is cleared).
    ShipCreated { ship: usize },
    /// Garrison vehicles enter the planet's ground-war map (`AddOwnTankToPlanet`, `RearrangeTankGroups`).
    GarrisonPlaced { planet: usize, design: i16, count: i32 },
    /// A colony ship or terraformer returns to its empire's pool on a new order.
    ColonyShipFreed { empire: u8 },
    TerraformerFreed { empire: u8 },
    /// A follower of a deleted fleet lost its order.
    FollowerStopped { fleet: usize },
    FleetDeleted { fleet: usize, owner: u8 },
}

/// The name up to its terminating NUL (C `strcmp` semantics).
fn cstr(name: &[u8]) -> &[u8] {
    &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())]
}

fn ai_income(ctx: &ProductionContext, e: usize) -> Option<f64> {
    if ctx.human_controlled[e] {
        return None;
    }
    let m = ctx.difficulty_money as f64;
    Some(if ctx.multiplayer { m * 0.5 } else { m })
}

impl Shipyard {
    fn design(&self, owner: usize, d: i16) -> Option<&Design> {
        self.designs.get(owner)?.get(usize::try_from(d).ok()?)
    }

    /// One production tick over all planets (spec §3). `money` = the empires' treasuries
    /// (`MoneyHatar[e][1]` is `reserve[1]`).
    pub fn tick(&mut self, money: &mut [Treasury; EMPIRES], ctx: &ProductionContext) -> Vec<ProductionEvent> {
        let mut ev = Vec::new();
        for m in money.iter_mut().skip(1) {
            m.reserve[1] = 0.0;
        }
        for p in 1..self.planets.len() {
            let o = self.planets[p].owner as usize;
            for l in 0..LINES {
                let d = self.planets[p].queue.design[l][0];
                if d <= 0 || self.planets[p].halted != 0 {
                    continue;
                }
                let Some(des) = self.design(o, d) else { continue }; // outside the table: undefined
                let (price, time) = (des.price as f64, des.build_time as f64);
                let rate = self.planets[p].capacity[l];
                let ai = !ctx.human_controlled.get(o).copied().unwrap_or(false);
                let mut s = rate.wrapping_mul(ctx.speed_mult) as f64 * 0.03;
                if ai {
                    s *= ctx.difficulty_production as f64;
                }
                if ctx.multiplayer && ai {
                    s *= 0.5;
                }
                let cost = s * price / time;
                let left = self.planets[p].queue.left[l] as f64;
                let month = (31 - ctx.day) as f64 * 50.0 * rate as f64 * 0.03;
                let cap = if left > month { month } else { left };
                money[o].reserve[1] += cap * price / time;
                if money[o].available(1) >= cost || cost <= 0.0 {
                    self.planets[p].queue.left[l] = (left - s) as i32;
                    money[o].add(1, -cost, ai_income(ctx, o));
                }
                if self.planets[p].queue.left[l] < 1 {
                    self.finish(p, l, d, ctx, &mut ev);
                }
            }
        }
        ev
    }

    /// A unit of the head entry is done (spec §3 steps 1–4).
    fn finish(&mut self, p: usize, l: usize, d: i16, ctx: &ProductionContext, ev: &mut Vec<ProductionEvent>) {
        let o = self.planets[p].owner as usize;
        let des = self.design(o, d).cloned().expect("checked by the caller");
        let near = self.delivery_fleet(p, DELIVERY_RANGE, true, true, l == 1, false, self.last_fleet[p]);
        self.last_fleet[p] = near as i32;
        let room = near > 0 && self.fleets[near].totals.capacity > self.fleets[near].totals.ground;
        let q = &self.planets[p].queue;
        let into = des.class == 1 || (room && q.into_fleet[1][0] != 0);
        let station = des.hull_flags[3] != 0;
        let rally = if station { 0 } else { q.rally as usize };
        ev.push(ProductionEvent::Built { planet: p, line: l, design: d, fleet: near });
        self.build(p, near, d, 1, rally, into, &des, false, ctx, ev);
        let q = &mut self.planets[p].queue;
        q.built[l] = q.built[l].wrapping_add(1);
        if q.built[l] >= q.count[l][0] {
            q.advance(l);
        }
        let head = self.planets[p].queue.design[l][0];
        if head > 0 {
            if let Some(t) = self.design(o, head).map(|x| x.build_time) {
                self.planets[p].queue.left[l] = t;
            }
        }
    }

    /// `SearchBolygohozKozeliFlotta(p, limit, own, idle, room, calm, last)` (spec §4).
    #[allow(clippy::too_many_arguments)]
    pub fn delivery_fleet(&self, p: usize, limit: f32, own: bool, idle: bool, room: bool, calm: bool, last: i32) -> usize {
        let pl = &self.planets[p];
        let qualifies = |g: usize| {
            let YardFleet { fleet: f, totals: t } = &self.fleets[g];
            f.owner != DELETED
                && f.owner != 0
                && (!own || pl.owner == f.owner)
                && (!idle || f.order == 0 || (f.order == order::MOVE && f.kind == order::PLANET && f.tx == p as f32))
                && (!room || t.ground < t.capacity)
                && f.flags & (flag::COLONISING | flag::TERRAFORMING) == 0
                && (!calm || f.flags & flag::IN_BATTLE == 0)
        };
        let dist = |g: usize| {
            let f = &self.fleets[g].fleet;
            (pl.x - f.x).abs() + (pl.y - f.y).abs()
        };
        if last > 0 && (last as usize) < self.fleets.len() && qualifies(last as usize) && dist(last as usize) < limit {
            return last as usize;
        }
        let (mut best, mut lim) = (0, limit);
        for g in 1..self.fleets.len() {
            if qualifies(g) && (best == 0 || self.fleets[g].fleet.order == 0) {
                let d = dist(g);
                if d < lim {
                    best = g;
                    lim = d;
                }
            }
        }
        best
    }

    /// `Legyart(o, p, f, d, n, rally, into, class, fighter, station, slow, single)` (spec §5), with the
    /// design's class and flags taken from `des`.
    #[allow(clippy::too_many_arguments)]
    pub fn build(&mut self, p: usize, f: usize, d: i16, n: i32, rally: usize, mut into: bool, des: &Design, single: bool,
        ctx: &ProductionContext, ev: &mut Vec<ProductionEvent>) {
        if des.class == -1 {
            return;
        }
        if des.class == 2 && into {
            let t = self.fleets.get(f).map(|x| x.totals).unwrap_or_default();
            if t.capacity <= t.ground {
                into = false;
            }
        }
        if des.hull_flags[3] != 0 {
            self.add_station(p, d);
            return;
        }
        if !into {
            self.new_vehicles(-(p as i32), d, n, ev);
            return;
        }
        let mut f = f;
        if rally > 0 || f == 0 {
            if des.class == 1 {
                let pl = &self.planets[p];
                let (owner, x, y) = (pl.owner, pl.x, pl.y);
                f = self.new_fleet(&ctx.default_fleet_name, owner, 0, x, y, 0.0, ctx, ev);
            }
            if f == 0 {
                return;
            }
        }
        if des.is_fighter() {
            self.new_vehicles(f as i32, d, if single { n } else { n * 5 }, ev);
        } else if des.class == 2 {
            self.new_vehicles(f as i32, d, n, ev);
        } else {
            for _ in 0..n {
                self.new_ship(f, d, ev);
            }
        }
        if rally > 0 && des.class == 1 {
            self.move_to_planet(f, rally, ctx, ev);
            self.fleets[f].fleet.flags |= flag::RADAR_IGNORED;
        }
        if self.fleets[f].totals.ships == 0 {
            self.delete_fleet(f, ctx, ev);
        }
    }

    /// Station `k = ++count` (unbounded in the original: station 11 lands on the count itself; later
    /// ones overwrite other planet fields and are dropped here).
    fn add_station(&mut self, p: usize, d: i16) {
        let pl = &mut self.planets[p];
        pl.station_count = pl.station_count.wrapping_add(1);
        let k = pl.station_count;
        if (1..=STATION_SLOTS as i32).contains(&k) {
            pl.stations[k as usize - 1] = d;
        } else if k == STATION_SLOTS as i32 + 1 {
            pl.station_count = (pl.station_count & !0xffff) | (d as u16 as i32);
        }
    }

    /// `FlottaNew(name, owner, id, x, y, speed)` (spec §5.1). Returns the slot.
    #[allow(clippy::too_many_arguments)]
    pub fn new_fleet(&mut self, name: &[u8], owner: u8, id: i16, x: f32, y: f32, speed: f32, ctx: &ProductionContext,
        ev: &mut Vec<ProductionEvent>) -> usize {
        let o = owner as usize;
        self.fleet_numbers[o] = self.fleet_numbers[o].wrapping_add(1);
        let first = self.fleet_numbers[o];
        let numbered = |n: i32| [cstr(name), b" ", n.to_string().as_bytes()].concat();
        let probe = numbered(first);
        let count = self.fleets.len().saturating_sub(1);
        let taken = (0..count).filter(|&g| self.fleets[g].fleet.owner == owner && cstr(&self.fleets[g].fleet.name) == probe).count();
        let full = numbered(first + taken as i32);
        let slot = (1..self.fleets.len()).rev().find(|&g| self.fleets[g].fleet.owner == DELETED).unwrap_or_else(|| {
            self.fleets.push(YardFleet::default());
            self.fleets.len() - 1
        });
        let rec = &mut self.fleets[slot];
        let text = if cstr(name) == cstr(&ctx.numbered_fleet_name) { full } else { cstr(name).to_vec() };
        for (i, &c) in text.iter().chain(std::iter::once(&0)).enumerate().take(rec.fleet.name.len()) {
            rec.fleet.name[i] = c;
        }
        let f = &mut rec.fleet;
        f.owner = owner;
        f.story_id = id;
        f.state &= !3;
        f.visible[1..].fill(0);
        if let Some(v) = f.visible.get_mut(o) {
            *v = 1;
        }
        if speed <= 1.0 {
            f.speed = ((if speed == 0.0 { 6.0 } else { speed }) as f64 / 100.0) as f32;
            f.speed_override = 0.0;
        } else {
            f.speed = (speed as f64 / 100.0) as f32;
            f.speed_override = f.speed;
        }
        (f.x, f.y) = (x, y);
        (f.order, f.kind, f.tx, f.ty) = (0, 0, 0.0, 0.0);
        (f.colony_ships, f.terraformers, f.first_waypoint) = (0, 0, 0);
        let named = cstr(&f.name) != cstr(&ctx.default_fleet_name);
        f.flags = (f.flags & 0x8000) | if named { flag::NAMED } else { 0 };
        f.radar = 0.0;
        rec.totals = FleetTotals::default();
        rec.sync();
        ev.push(ProductionEvent::FleetCreated { fleet: slot });
        slot
    }

    /// `HajoNew(f, d)`: the last free ship slot, else a new one; then the fleet sums.
    pub fn new_ship(&mut self, f: usize, d: i16, ev: &mut Vec<ProductionEvent>) {
        if f < 1 {
            return;
        }
        let slot = (1..self.ships.len()).rev().find(|&i| self.ships[i].design == NO_DESIGN).unwrap_or_else(|| {
            if self.ships.is_empty() {
                self.ships.push(Ship { fleet: 0, design: NO_DESIGN });
            }
            self.ships.push(Ship { fleet: 0, design: NO_DESIGN });
            self.ships.len() - 1
        });
        self.ships[slot] = Ship { fleet: f as i16, design: d };
        ev.push(ProductionEvent::ShipCreated { ship: slot });
        self.ship_sums(f, d, 1);
    }

    /// `HajoFlottahozHozzaszamol(f, d, n)` (`space-combat.md` §3).
    fn ship_sums(&mut self, f: usize, d: i16, n: i32) {
        let Some(rec) = self.fleets.get(f) else { return };
        let owner = rec.fleet.owner;
        if owner == DELETED {
            return;
        }
        let Some(des) = self.design(owner as usize, d).cloned() else { return };
        let o = owner as usize;
        let rec = &mut self.fleets[f];
        rec.totals.add_ship(&des, n);
        if des.hull_flags[0] != 0 {
            rec.fleet.colony_ships = rec.fleet.colony_ships.wrapping_add(n as i16);
            self.colony_ship_pool[o] += n;
        }
        if des.hull_flags[1] != 0 {
            rec.fleet.terraformers = rec.fleet.terraformers.wrapping_add(n as i16);
            self.terraformer_pool[o] += n;
        }
        if d == 28 {
            if n > 0 {
                rec.fleet.flags |= FLAG_DESIGN28;
            } else if n < 0 {
                rec.fleet.flags &= !FLAG_DESIGN28;
            }
        }
        rec.sync();
    }

    fn holder_owner(&self, f: i32) -> u8 {
        if f < 1 {
            self.planets.get((-f) as usize).map_or(DELETED, |p| p.owner)
        } else {
            self.fleets.get(f as usize).map_or(DELETED, |x| x.fleet.owner)
        }
    }

    /// `TankNew(f, d, n)` (spec §5.1): `f = −p` is planet `p`'s garrison.
    pub fn new_vehicles(&mut self, f: i32, d: i16, n: i32, ev: &mut Vec<ProductionEvent>) {
        let owner = self.holder_owner(f);
        let class = self.design(owner as usize, d).map_or(0, |x| x.class);
        if !(class > 1 || f > 0) {
            return;
        }
        let holder = f as i16;
        let live = (1..self.vehicles.len()).rev()
            .find(|&i| self.vehicles[i].design != NO_DESIGN && self.vehicles[i].design == d && self.vehicles[i].fleet == holder);
        let slot = match live.or_else(|| (1..self.vehicles.len()).rev().find(|&i| self.vehicles[i].design == NO_DESIGN)) {
            Some(i) => i,
            None => {
                if self.vehicles.len() > MAX_VEHICLE_ENTRIES {
                    return;
                }
                if self.vehicles.is_empty() {
                    self.vehicles.push(Vehicle { fleet: 0, design: NO_DESIGN, count: 0 });
                }
                self.vehicles.push(Vehicle { fleet: 0, design: NO_DESIGN, count: 0 });
                self.vehicles.len() - 1
            }
        };
        let v = &mut self.vehicles[slot];
        if v.design == NO_DESIGN {
            *v = Vehicle { fleet: holder, design: d, count: n };
        } else {
            v.count = v.count.wrapping_add(n);
        }
        self.vehicle_sums(f, d, n);
        if f < 0 {
            ev.push(ProductionEvent::GarrisonPlaced { planet: (-f) as usize, design: d, count: n });
        }
    }

    /// `TankFlottahozHozzaszamol(f, d, n)`: ground vehicles add to the holder's ground sums, anything
    /// else in a fleet counts as fighters.
    fn vehicle_sums(&mut self, f: i32, d: i16, n: i32) {
        let owner = self.holder_owner(f);
        if owner == DELETED {
            return;
        }
        let Some(des) = self.design(owner as usize, d).cloned() else { return };
        if f > 0 {
            let rec = &mut self.fleets[f as usize];
            rec.totals.add_vehicle(&des, n);
            rec.sync();
        } else if des.class == 2 {
            let pl = &mut self.planets[(-f) as usize];
            pl.garrison = pl.garrison.wrapping_add(n);
            let s = (des.hp.wrapping_mul(des.attack) as f32 as f64).sqrt() as f32;
            pl.garrison_strength = (s * n as f32 + pl.garrison_strength as f32) as i32;
        }
    }

    /// `FlottaMozgat(f, 1, 1, rally, 0)`: a move to the rally planet.
    fn move_to_planet(&mut self, f: usize, planet: usize, ctx: &ProductionContext, ev: &mut Vec<ProductionEvent>) {
        self.set_order(f, order::MOVE, order::PLANET, planet as f32, 0.0, ctx, ev);
    }

    /// `FlottaMozgat(f, ord, kind, x, y)` for plain orders (`fleet-movement.md` §2); waypoint nodes are
    /// left to the fleet port.
    #[allow(clippy::too_many_arguments)]
    fn set_order(&mut self, f: usize, ord: i16, kind: i16, x: f32, y: f32, ctx: &ProductionContext, ev: &mut Vec<ProductionEvent>) {
        let rec = &mut self.fleets[f];
        let fl = &mut rec.fleet;
        if fl.flags & flag::COLONISING != 0 {
            ev.push(ProductionEvent::ColonyShipFreed { empire: fl.owner });
        }
        if fl.flags & flag::TERRAFORMING != 0 {
            ev.push(ProductionEvent::TerraformerFreed { empire: fl.owner });
        }
        fl.flags &= !(flag::COLONISING | flag::TERRAFORMING);
        let human = ctx.human_controlled.get(fl.owner as usize).copied().unwrap_or(false);
        if human || fl.colony_ships as i32 + fl.terraformers as i32 == rec.totals.ships {
            fl.flags &= !flag::RADAR_IGNORED;
        }
        (fl.order, fl.kind, fl.tx, fl.ty) = (ord, kind, x, y);
        fl.first_waypoint = 0;
        fl.flags &= !flag::PATROL;
    }

    /// `FlottaDelete(f)` of an emptied delivery fleet: followers stop, its vehicles go, the slot is freed.
    fn delete_fleet(&mut self, f: usize, ctx: &ProductionContext, ev: &mut Vec<ProductionEvent>) {
        let fl = &self.fleets[f].fleet;
        if fl.owner == DELETED || fl.flags & flag::WORMHOLE != 0 {
            return;
        }
        for g in 1..self.fleets.len() {
            let x = &self.fleets[g].fleet;
            if x.owner != DELETED && x.order > 0 && x.kind == order::FLEET && x.tx == f as f32 {
                self.set_order(g, 0, 0, 0.0, 0.0, ctx, ev);
                ev.push(ProductionEvent::FollowerStopped { fleet: g });
            }
        }
        for i in 1..self.vehicles.len() {
            let v = self.vehicles[i];
            if v.fleet == f as i16 && v.design != NO_DESIGN {
                self.vehicles[i].count = 0;
                self.vehicles[i].design = NO_DESIGN;
                self.vehicle_sums(f as i32, v.design, -v.count);
            }
        }
        let owner = self.fleets[f].fleet.owner;
        self.fleet_numbers[owner as usize] -= 1;
        self.fleets[f].fleet.owner = DELETED;
        ev.push(ProductionEvent::FleetDeleted { fleet: f, owner });
    }
}
