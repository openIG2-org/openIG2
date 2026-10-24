//! Fleet orders, waypoints, speed and movement on the starmap (`FRISSIT_Starmap`, exe 0x594ae0).
//! Spec: `re/rules/fleet-movement.md`.
//!
//! The refresh tick's speed, radar and visibility pass (spec §3, §6) lives in [`crate::radar`]. Planet
//! production (§7) lives in [`crate::production`]. Arrival side effects owned by other systems (battles, colonisation,
//! terraforming, ship transfer on merges, story hooks, money) are reported as [`StarmapEvent`]s.

use crate::radar::{RadarRules, RadarTables, ShipDesign};
use crate::rng::Streams;
use crate::time::slowed_down;

/// Trader rolls (spec §4.4).
pub const TRADER_STREAM: usize = 14;
/// The trader empire.
pub const TRADERS: u8 = 9;
/// Owner value of a free fleet slot.
pub const DELETED: u8 = 0xff;

/// Fleet flags (`+0x52`).
pub mod flag {
    pub const IN_BATTLE: u16 = 1 << 0;
    pub const PATROL: u16 = 1 << 1;
    pub const VIRUS: u16 = 1 << 2;
    pub const COLONISING: u16 = 1 << 3;
    pub const TERRAFORMING: u16 = 1 << 4;
    pub const RADAR_IGNORED: u16 = 1 << 5;
    pub const ALWAYS_VISIBLE: u16 = 1 << 8;
    /// The fleet has a name other than the default (`FlottaNevLatszik`).
    pub const NAMED: u16 = 1 << 11;
    pub const WORMHOLE: u16 = 1 << 10;
    pub const STEALTH: u16 = 1 << 12;
    pub const PROTECTED: u16 = 1 << 14;
}

/// Orders (`+0x40`) and target kinds (`+0x42`).
pub mod order {
    pub const NONE: i16 = 0;
    pub const MOVE: i16 = 1;
    pub const ATTACK: i16 = 2;
    /// `FlottaMozgat` arguments only: append a waypoint (14: and patrol).
    pub const ADD_WAYPOINT: i16 = 13;
    pub const ADD_PATROL_WAYPOINT: i16 = 14;
    pub const POINT: i16 = 0;
    pub const PLANET: i16 = 1;
    pub const FLEET: i16 = 2;
}

/// The movement-relevant part of a fleet record (`FlottaTipus`, 0x80 bytes, spec §1.1).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fleet {
    /// `+0x00`: NUL-terminated name.
    pub name: [u8; 21],
    pub owner: u8,
    pub story_id: i16,
    /// `+0x18`: bit0 hidden (traders: has traded), bit1 delete on arrival.
    pub state: u8,
    /// `+0x19 + e`.
    pub visible: [u8; 19],
    /// `+0x2c..+0x2d`: not read; `FlottaNew`'s dword clear at `+0x2a` zeroes them.
    pub field_2c: [u8; 2],
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub speed_override: f32,
    pub order: i16,
    pub kind: i16,
    pub tx: f32,
    pub ty: f32,
    pub colony_ships: i16,
    pub terraformers: i16,
    pub first_waypoint: i16,
    pub flags: u16,
    pub ship_count: i32,
    pub strength: i32,
    /// `+0x68`: ground strength of the vehicles aboard (`space-combat.md` §3).
    pub ground_strength: i32,
    pub radar: f32,
}

impl Fleet {
    /// Decode a 0x80-byte `FlottaTipus` record (spec §1.1).
    pub fn from_record(r: &[u8]) -> Self {
        let i16_at = |o: usize| i16::from_le_bytes([r[o], r[o + 1]]);
        let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let f32_at = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        Self {
            name: r[..21].try_into().unwrap(),
            owner: r[0x15],
            story_id: i16_at(0x16),
            state: r[0x18],
            visible: r[0x19..0x19 + 19].try_into().unwrap(),
            field_2c: [r[0x2c], r[0x2d]],
            x: f32_at(0x30),
            y: f32_at(0x34),
            speed: f32_at(0x38),
            speed_override: f32_at(0x3c),
            order: i16_at(0x40),
            kind: i16_at(0x42),
            tx: f32_at(0x44),
            ty: f32_at(0x48),
            colony_ships: i16_at(0x4c),
            terraformers: i16_at(0x4e),
            first_waypoint: i16_at(0x50),
            flags: u16::from_le_bytes([r[0x52], r[0x53]]),
            ship_count: i32_at(0x54),
            strength: i32_at(0x60),
            ground_strength: i32_at(0x68),
            radar: f32_at(0x7c),
        }
    }

    pub fn alive(&self) -> bool {
        self.owner != DELETED
    }
    fn is_trader(&self) -> bool {
        (10_000..11_000).contains(&self.story_id)
    }
    /// `+0x44` as an index (C float → int truncation).
    fn target_index(&self) -> usize {
        (self.tx as i32).max(0) as usize
    }
}

/// One waypoint node (12 bytes): `next` 0 = last, −1 = free.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Waypoint {
    pub x: f32,
    pub y: f32,
    pub next: i16,
}

/// A wormhole (0x1c bytes): end A at (`ax`, `ay`) is fleet `fleet_a`, end B likewise.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Wormhole {
    pub ax: f32,
    pub ay: f32,
    pub revealed_a: bool,
    pub fleet_a: i16,
    pub bx: f32,
    pub by: f32,
    pub revealed_b: bool,
    pub fleet_b: i16,
    /// `+0x18..+0x1a`: the ends' tint (`AddWormhole`).
    pub tint: [u8; 3],
}

/// What the starmap needs to know about a planet.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StarPlanet {
    pub x: f32,
    pub y: f32,
    pub owner: u8,
    pub epidemic: i16,
    /// Finished, working buildings (`SearchBuilding(p, type, true)`).
    pub trade_centres: i32,
    pub banks: i32,
    pub trade_ports: i32,
    /// `+0x6e`: 0 none, 1 colony hub (recomputed), >= 2 radar station (scenario/story).
    pub radar_level: u8,
    /// A finished colony hub (building type 1, `+8 == 0`), working or not.
    pub has_hub: bool,
    /// `+0xd8 + 2e`: seen by empire `e` (never cleared by the radar pass).
    pub visible: [i16; 19],
    /// `+0xbb`: tell the story when the human first sees this planet.
    pub notice: u8,
}

/// Fleets, waypoints, wormholes and planets; all 1-based like the original (index 0 unused).
#[derive(Debug, Clone, Default)]
pub struct Starmap {
    pub fleets: Vec<Fleet>,
    pub waypoints: Vec<Waypoint>,
    pub wormholes: Vec<Wormhole>,
    pub planets: Vec<StarPlanet>,
    /// Per-empire radar tables (`GetENRadarKifejlesztve`, spec §6.2).
    pub radar: RadarTables,
    /// `BirodalomIsmeri[e][o]`: empire `e` has met empire `o`.
    pub met: [[u8; 19]; 19],
    /// `OsszHanyKolonizalo` / `OsszHanyTerraformalo`: colony ships/terraformers aboard fleets that
    /// aren't using them (recounted on refresh ticks; index 0 is never cleared).
    pub colony_ship_pool: [i32; 19],
    pub terraformer_pool: [i32; 19],
}

/// Global inputs of a starmap tick.
#[derive(Debug, Clone, Default)]
pub struct StarmapContext {
    /// `Ido2`: the sim tick counter (speed refresh when `% 10 == 1`).
    pub tick: i32,
    /// `IdoSpeedSzorzo`; the tick leaves the clock's value afterwards here (spec §6.3 step 6: the
    /// "enemy approaching" slow-down acts before the movement loop).
    pub speed_mult: i32,
    pub multiplayer: bool,
    /// `nemlehetidotallitani` (exe 0xBAD0D4): scripted, the speed can't change (SP).
    pub speed_locked: bool,
    /// `ProgramSebesseg` (exe 0xBAE54C): the MP game speed 1..=4.
    pub program_speed: i32,
    /// Out: `SetIdoLelassit` ran (the caller writes `IdoSpeed` and the battle multiplier, [`crate::time::slow_down`]).
    pub slowed: bool,
    pub human_empire: u8,
    pub human_controlled: [bool; 19],
    /// `BirodalomEl[e]`.
    pub alive: [bool; 19],
    pub empire_race: [u8; 19],
    /// `DEBUGX`, `Story_Minden_Flotta_Latszik`, the debug ×100 speed switch.
    pub debug: bool,
    pub all_fleets_visible: bool,
    pub debug_speed: bool,
    /// `Viszony[a][b]`: (`+0x24` acknowledged, `+0x25` level).
    pub relations: Vec<[(u8, u8); 19]>,
    /// `AlapFlottaNev`: the localised default fleet name (string 17/5/46, "Fleet" in English).
    pub default_fleet_name: Vec<u8>,
    /// `ProgramPlayers` (empires that discover others) and `ProgramPlayers2` (empires in the
    /// visibility and alliance loops).
    pub program_players: usize,
    pub program_players2: usize,
    /// Highest developed radar tech per empire, 1..=4 for techs 0x21..=0x24 (0 = none).
    pub radar_tech: [u8; 19],
    /// `StoryAdat+0x28`: the tracked story fleet (always seen by the human, no supply limit).
    pub story_tracked_fleet: i32,
    /// `StoryAdat+0x38`: the human's radar tech counts one level higher (SP).
    pub story_radar_bonus: bool,
    /// `StoryAdat+0x48`: fleet radar isn't limited by supply.
    pub story_no_supply: bool,
    /// `StoryAdat+0x50`: a planet that anchors every empire's supply.
    pub story_supply_planet: i32,
    /// Tutorial active or paused with event 1 pending (a sighted planet gets named).
    pub tutorial_sighting: bool,
    /// `RadarHatotav` / `RadarHatotav2Init` from the user's exe.
    pub radar_rules: RadarRules,
    /// Fleets whose ships are in an active space war (get the in-battle bit every tick, spec §6.3 step 8):
    /// [`crate::spacewar::fleets_in_battle`] of the active battles (`space-combat.md` §6).
    pub in_battle: Vec<usize>,
    /// The empires whose machine sends the original's `Multi_Send(.., EN, ..)` (classic: the local player).
    pub sends: [bool; 19],
    /// The empires whose own machine adds the local human's sightings to the visibility pass (spec §6.4: the
    /// tracked story fleet, revealed wormhole ends, "enemy approaching"): canonical MP every human; none set =
    /// `human_empire` alone (classic).
    pub viewers: [bool; 19],
    /// The two fleets the starmap's transfer panel shows (`AktKep == 2` and `StarmapTransferMode`:
    /// `StarmapTransferFlotta1/2`); their emptiness isn't reported.
    pub transfer: Option<(i32, i32)>,
}

impl StarmapContext {
    /// `Viszony[a][b].+0x25` (5 = alliance); 0 outside the matrix.
    pub fn relation_level(&self, a: usize, b: usize) -> u8 {
        self.relations.get(a).and_then(|r| r.get(b)).map_or(0, |r| r.1)
    }

    /// [`StarmapContext::viewers`] in empire order.
    pub fn viewer_list(&self) -> Vec<usize> {
        let v: Vec<usize> = (0..19).filter(|&e| self.viewers[e]).collect();
        if v.is_empty() { vec![self.human_empire as usize % 19] } else { v }
    }

    /// `e` is one of [`StarmapContext::viewers`].
    pub fn is_viewer(&self, e: usize) -> bool {
        if self.viewers.iter().any(|&v| v) { self.viewers.get(e).copied().unwrap_or(false) } else { e == self.human_empire as usize }
    }

    /// `SetIdoLelassit` ([`crate::time::slowed_down`]).
    pub fn slow_down(&mut self) {
        self.speed_mult = slowed_down(self.speed_mult, self.multiplayer, self.speed_locked, self.program_speed);
        self.slowed = true;
    }
}

/// The name up to its terminating NUL (C `strcmp` semantics).
fn cstr(name: &[u8]) -> &[u8] {
    &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())]
}

/// Systems a fleet arrival hands over to **inside** the movement loop (spec §4.3 cases 1–2): their effects
/// are seen by the fleets moved later in the same tick. `()` does nothing (the planet is left as it was;
/// the events still report the arrival): [`crate::colony_side::ColonyArrivals`] is the real one.
pub trait Arrivals {
    /// Colonisation of unowned planet `planet` by `owner`; update the starmap's view `star` to match.
    fn colonise(&mut self, _planet: usize, _owner: u8, _star: &mut StarPlanet, _rng: &mut Streams) {}
    /// Terraforming of unowned planet `planet` by `owner`; returns the new surface (0 = not modelled).
    fn terraform(&mut self, _planet: usize, _owner: u8) -> u8 {
        0
    }
    /// A trader's income at the nearest planet (spec §4.3): `MoneyHozzaadLog(empire, 0, (f64)amount)` and
    /// `TradeIncomeThisYear[empire] += amount`, also when `amount` is 0.
    fn trader_paid(&mut self, _empire: u8, _amount: i32) {}
    /// `FlottaOsszevon(into, from)`'s units (spec §4.3): every ship of `from` moves to `into` (`HajoAtpakol`), then
    /// every vehicle entry (`TankAtpakol`); the records' totals (`+0x54`/`+0x60`/`+0x68`) and colony-ship /
    /// terraformer counts follow, and so do the map's pools. Called after the followers are retargeted, before
    /// `from` is freed (later fleets of the tick see the new strengths).
    fn merge_units(&mut self, _into: usize, _from: usize, _map: &mut Starmap) {}
    /// `FlottaDelete(f)`'s units: every ship (`HajoDelete`) and vehicle entry goes and `OsszHanySajatFlotta[owner]`
    /// drops. Called after the followers stop, before the slot is freed.
    fn delete_units(&mut self, _f: usize, _map: &mut Starmap) {}
}

impl Arrivals for () {}

/// A movement step paused by a battle ([`Starmap::tick_until_battle`]): the fleet and the position its step
/// reached (written back when the step ends).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paused {
    pub fleet: usize,
    pub pos: (f32, f32),
}

/// Effects of a tick that other systems carry out, or that the UI reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StarmapEvent {
    /// A colony ship or terraformer returns to its empire's pool (`OsszHanyKolonizalo`/…`Terraformalo`).
    ColonyShipFreed { empire: u8 },
    /// `Multi_Send(0x1b, EN, f)`: the sending player's fleet `f` has no ships (handled before the next tick:
    /// `Game::fleet_emptied`).
    FleetEmptied { fleet: usize },
    TerraformerFreed { empire: u8 },
    /// The tracked fleet vanished from radar; the order became a move to its last position.
    ContactLost { fleet: usize },
    /// MP: a human's fleet lost contact (reported for every human, [`StarmapContext::viewers`]); its order stays,
    /// the change goes over the network instead.
    ContactLostNetwork { fleet: usize },
    Deleted { fleet: usize },
    /// `Story_FleetArrivedSave(f)` on arrival, with what it stores (the record at that moment): the order's target
    /// type `+0x42`, `(int) +0x44`, the story ids `+0x16` of the fleet and (type 2) of the target fleet.
    Arrived { fleet: usize, kind: i16, target: i32, fleet_id: i16, target_fleet_id: i16 },
    /// `Story_FleetArrivedSave(f)` again after arriving at a followed fleet (and merging into it if it is an own one).
    ArrivalResaved { fleet: usize, kind: i16, target: i32, fleet_id: i16, target_fleet_id: i16 },
    /// Colonisation (carried out by [`Arrivals::colonise`]; story special 10 follows).
    Colonise { fleet: usize, planet: usize },
    /// Terraforming to `surface` ([`Arrivals::terraform`]; star sprites rebuilt, story special 0x18).
    Terraform { fleet: usize, planet: usize, surface: u8 },
    /// Scripted fleet reached its goal (story event 0x10).
    ScriptedArrival { fleet: usize },
    /// `FlottaOsszevon(into, from)` (the units moved by [`Arrivals::merge_units`]).
    Merge { into: usize, from: usize },
    /// Virus bookkeeping at a planet (`Virus_CheckTerjedes`, stream 8): may clear the carrier flag.
    VirusSpread { fleet: usize, planet: usize },
    Wormhole { fleet: usize, wormhole: usize },
    /// Battles: the rest of the tick's fleets don't move.
    SpaceBattle { fleet: usize, target: usize },
    PlanetAttack { fleet: usize, planet: usize },
    /// Attack on a story-protected target (story event 0x12).
    ProtectedTarget { fleet: usize },
    /// `Story_Special(data)` from an arrival (spec §4.3), at its place in the loop: colonisation 10(owner, planet),
    /// terraforming 0x18(owner, planet), scripted 0x10(story id, fleet), wormhole 0x14(owner, 1, fleet,
    /// wormholes), protected target 0x12(target fleet, 2, fleet) / 0x12(fleet, 1, planet).
    StorySpecial([i32; 8]),
    /// A trader was paid at `planet` (booked through [`Arrivals::trader_paid`]).
    TraderPaid { fleet: usize, planet: usize, empire: u8, amount: i32 },
    /// `BirodalomIsmeri[by][empire]` became set; for the human also story event 0x0B and
    /// `FelfedezBirodalom(empire)` (diplomacy list, "new empire" message).
    EmpireMet { by: u8, empire: u8 },
    /// `Story_ArrivedToRadarRangeFleet`: a fleet flagged for notice came into the human's view.
    FleetSighted { fleet: usize },
    /// `Story_ArrivedToRadarRangeBolygo`.
    PlanetSighted { planet: usize },
    /// A fleet newly seen by human `to` is attacking a planet or fleet of `to`'s: message 0x0E and `SetIdoLelassit(1)`.
    EnemyApproaching { fleet: usize, to: u8 },
    /// Tutorial event 1: the planet gets its name (string 2/4/6) and `TutorialCheckEvent(1)`.
    TutorialPlanetSighted { planet: usize },
}

/// `speed × 0.3` with the exe's double constant (spec §4.2).
fn step_of(speed: f32) -> f32 {
    (speed as f64 * 0.3) as f32
}

fn len(dx: f32, dy: f32) -> f32 {
    ((dy * dy + dx * dx) as f64).sqrt() as f32
}

impl Starmap {
    fn free_waypoint(&mut self) -> usize {
        if let Some(i) = (1..self.waypoints.len()).find(|&i| self.waypoints[i].next == -1) {
            return i;
        }
        if self.waypoints.is_empty() {
            self.waypoints.push(Waypoint::default());
        }
        self.waypoints.push(Waypoint { next: -1, ..Default::default() });
        self.waypoints.len() - 1
    }

    fn clear_chain(&mut self, mut node: usize) {
        while node > 0 && self.waypoints[node].next >= 0 {
            let next = self.waypoints[node].next;
            self.waypoints[node].next = -1;
            node = next as usize;
        }
    }

    /// `FlottaRemoveAllWaypoint`.
    pub fn remove_all_waypoints(&mut self, f: usize) {
        let first = self.fleets[f].first_waypoint;
        if first != 0 {
            self.fleets[f].first_waypoint = 0;
            self.fleets[f].flags &= !flag::PATROL;
            self.clear_chain(first as usize);
        }
    }

    /// `FlottaAddWaypoint`.
    pub fn add_waypoint(&mut self, f: usize, x: f32, y: f32, ctx: &StarmapContext, events: &mut Vec<StarmapEvent>) {
        if self.fleets[f].order == order::NONE {
            self.order(f, order::MOVE, order::POINT, x, y, ctx, events);
            return;
        }
        let n = if self.fleets[f].first_waypoint == 0 {
            let n = self.free_waypoint();
            self.fleets[f].first_waypoint = n as i16;
            n
        } else {
            let mut last = self.fleets[f].first_waypoint as usize;
            while self.waypoints[last].next > 0 {
                last = self.waypoints[last].next as usize;
            }
            let n = self.free_waypoint();
            self.waypoints[last].next = n as i16;
            n
        };
        self.waypoints[n] = Waypoint { x, y, next: 0 };
    }

    /// `FlottaRemoveNextWaypoint`: pop the head; on patrol re-append it holding the fleet's position.
    fn remove_next_waypoint(&mut self, f: usize) {
        let head = self.fleets[f].first_waypoint as usize;
        if head == 0 {
            return;
        }
        let next = self.waypoints[head].next;
        self.fleets[f].first_waypoint = next;
        if self.fleets[f].flags & flag::PATROL == 0 {
            self.waypoints[head].next = -1;
            return;
        }
        // With a single patrol waypoint the node is orphaned (original behaviour).
        self.waypoints[head] = Waypoint { x: self.fleets[f].x, y: self.fleets[f].y, next: 0 };
        if next != 0 {
            let mut last = next as usize;
            while self.waypoints[last].next != 0 {
                last = self.waypoints[last].next as usize;
            }
            self.waypoints[last].next = head as i16;
        }
    }

    /// `FlottaTovabbMozgat`: next waypoint, or stop.
    fn continue_route(&mut self, f: usize, ctx: &StarmapContext, events: &mut Vec<StarmapEvent>) {
        let fl = &self.fleets[f];
        if fl.first_waypoint == 0 || fl.order != order::MOVE || fl.kind != order::POINT {
            self.order(f, order::NONE, order::POINT, 0.0, 0.0, ctx, events);
            return;
        }
        let w = self.waypoints[fl.first_waypoint as usize];
        self.fleets[f].tx = w.x;
        self.fleets[f].ty = w.y;
        self.remove_next_waypoint(f);
    }

    /// `FlottaMozgat(f, order, kind, x, y)` (spec §2).
    #[allow(clippy::too_many_arguments)]
    pub fn order(&mut self, f: usize, mut ord: i16, kind: i16, x: f32, y: f32, ctx: &StarmapContext, events: &mut Vec<StarmapEvent>) {
        if ord == order::ADD_WAYPOINT || ord == order::ADD_PATROL_WAYPOINT {
            let fl = &self.fleets[f];
            if fl.order == order::MOVE && fl.kind == order::POINT {
                self.add_waypoint(f, x, y, ctx, events);
                if ord == order::ADD_PATROL_WAYPOINT {
                    self.fleets[f].flags |= flag::PATROL;
                }
                return;
            }
            ord = order::MOVE;
        }
        let fl = &mut self.fleets[f];
        if fl.flags & flag::COLONISING != 0 {
            events.push(StarmapEvent::ColonyShipFreed { empire: fl.owner });
        }
        if fl.flags & flag::TERRAFORMING != 0 {
            events.push(StarmapEvent::TerraformerFreed { empire: fl.owner });
        }
        fl.flags &= !(flag::COLONISING | flag::TERRAFORMING);
        let human = ctx.human_controlled.get(fl.owner as usize).copied().unwrap_or(false);
        if human || fl.colony_ships as i32 + fl.terraformers as i32 == fl.ship_count {
            fl.flags &= !flag::RADAR_IGNORED;
        }
        (fl.order, fl.kind, fl.tx, fl.ty) = (ord, kind, x, y);
        self.remove_all_waypoints(f);
    }

    /// The bit-5 rule shared by `FlottaMozgat` and `FlottaOsszevon`.
    fn settle_radar_flag(&mut self, f: usize, ctx: &StarmapContext) {
        let fl = &mut self.fleets[f];
        if ctx.human_controlled.get(fl.owner as usize).copied().unwrap_or(false)
            || fl.colony_ships as i32 + fl.terraformers as i32 == fl.ship_count
        {
            fl.flags &= !flag::RADAR_IGNORED;
        }
    }

    /// `FlottaDelete`: followers stop, the slot is freed (waypoints aren't returned to the pool).
    fn delete(&mut self, f: usize, ctx: &StarmapContext, arrivals: &mut dyn Arrivals, ev: &mut Vec<StarmapEvent>) {
        if !self.fleets[f].alive() || self.fleets[f].flags & flag::WORMHOLE != 0 {
            return;
        }
        for g in 1..self.fleets.len() {
            let fl = &self.fleets[g];
            if fl.alive() && fl.order > 0 && fl.kind == order::FLEET && fl.tx == f as f32 {
                self.order(g, order::NONE, order::POINT, 0.0, 0.0, ctx, ev);
            }
        }
        arrivals.delete_units(f, self);
        self.fleets[f].owner = DELETED;
        ev.push(StarmapEvent::Deleted { fleet: f });
    }

    /// `FlottaOsszevon(into, from)`: the records here, the units through [`Arrivals::merge_units`].
    fn merge(&mut self, into: usize, from: usize, ctx: &StarmapContext, arrivals: &mut dyn Arrivals, ev: &mut Vec<StarmapEvent>) {
        if into == from || into == 0 || from == 0 || into >= self.fleets.len() || from >= self.fleets.len() {
            return;
        }
        if self.fleets[into].radar < self.fleets[from].radar {
            self.fleets[into].radar = self.fleets[from].radar;
        }
        let default = cstr(&ctx.default_fleet_name);
        if cstr(&self.fleets[into].name) == default {
            // `strcpy`: up to and including the NUL; the bytes after it stay.
            let src = self.fleets[from].name;
            let len = (cstr(&src).len() + 1).min(src.len());
            self.fleets[into].name[..len].copy_from_slice(&src[..len]);
        }
        if self.fleets[into].story_id == 0 {
            self.fleets[into].story_id = self.fleets[from].story_id;
        }
        let named = cstr(&self.fleets[into].name) != default;
        self.fleets[into].flags = (self.fleets[into].flags & !flag::NAMED) | if named { flag::NAMED } else { 0 };
        for g in 1..self.fleets.len() {
            let fl = &self.fleets[g];
            if g != into && g != from && fl.alive() && fl.order > 0 && fl.kind == order::FLEET && fl.tx == from as f32 {
                let keep = fl.flags & flag::RADAR_IGNORED;
                let ord = fl.order;
                self.order(g, ord, order::FLEET, into as f32, 0.0, ctx, ev);
                self.fleets[g].flags = (self.fleets[g].flags & !flag::RADAR_IGNORED) | keep;
            }
        }
        arrivals.merge_units(into, from, self);
        self.fleets[from].owner = DELETED;
        self.settle_radar_flag(into, ctx);
        ev.push(StarmapEvent::Merge { into, from });
    }

    /// `SearchFlottahozKozeliBolygo(f, limit, false, false)`: nearest planet by Manhattan distance.
    pub fn nearest_planet(&self, f: usize, limit: f32) -> usize {
        let fl = &self.fleets[f];
        let mut best = (0, limit);
        if fl.owner != 0 {
            for (i, p) in self.planets.iter().enumerate().skip(1) {
                let d = (fl.y - p.y).abs() + (fl.x - p.x).abs();
                if d < best.1 {
                    best = (i, d);
                }
            }
        }
        best.0
    }

    /// Spec §4.1: keep orders consistent with what the owner can see and own.
    fn upkeep(&mut self, f: usize, ctx: &StarmapContext, arrivals: &mut dyn Arrivals, ev: &mut Vec<StarmapEvent>) {
        let fl = self.fleets[f].clone();
        let t = fl.target_index();
        if fl.order > 0 && fl.kind == order::FLEET {
            if !self.fleets[t].alive() {
                self.order(f, order::NONE, order::POINT, 0.0, 0.0, ctx, ev);
            }
            let tg = self.fleets[t].clone();
            let owner = fl.owner as usize;
            let unseen = (tg.visible.get(owner).copied().unwrap_or(0) == 0
                && tg.flags & flag::ALWAYS_VISIBLE == 0
                && !ctx.all_fleets_visible)
                || tg.state & 1 != 0;
            if !ctx.multiplayer {
                if fl.owner < 9 && unseen && !ctx.debug {
                    self.order(f, order::MOVE, order::POINT, tg.x, tg.y, ctx, ev);
                    if fl.owner == ctx.human_empire {
                        ev.push(StarmapEvent::ContactLost { fleet: f });
                    }
                }
            } else {
                if ctx.is_viewer(owner) && unseen {
                    ev.push(StarmapEvent::ContactLostNetwork { fleet: f });
                }
                if !ctx.human_controlled[owner] && unseen {
                    self.order(f, order::MOVE, order::POINT, tg.x, tg.y, ctx, ev);
                }
            }
        }
        let fl = &self.fleets[f];
        let planet_owner = self.planets.get(t).map_or(0, |p| p.owner);
        if fl.order == order::ATTACK && fl.kind == order::PLANET && planet_owner == fl.owner {
            self.order(f, order::NONE, order::POINT, 0.0, 0.0, ctx, ev);
        }
        for bit in [flag::COLONISING, flag::TERRAFORMING] {
            let fl = &self.fleets[f];
            if fl.flags & bit != 0 && fl.order == order::MOVE && fl.kind == order::PLANET && planet_owner != 0 {
                self.order(f, order::NONE, order::POINT, 0.0, 0.0, ctx, ev);
            }
        }
        let owner = self.fleets[f].owner;
        if (1..9).contains(&owner) && !ctx.alive[owner as usize] {
            self.delete(f, ctx, arrivals, ev);
        }
    }

    /// Where a fleet's order points (spec §4.2): `cur` (the target itself) and `aim` (with lead).
    fn aim(&self, fl: &Fleet, step: f32) -> ((f32, f32), (f32, f32)) {
        let t = fl.target_index();
        match fl.kind {
            order::PLANET => {
                let p = &self.planets[t];
                let c = (p.x, p.y + 2.0);
                (c, c)
            }
            order::FLEET => {
                let tg = &self.fleets[t];
                let cur = (tg.x, tg.y);
                let mut aim = cur;
                if tg.order != 0 && tg.speed > 0.0 {
                    let ti = tg.target_index();
                    let dest = match tg.kind {
                        order::POINT => (tg.tx, tg.ty),
                        order::PLANET => (self.planets[ti].x, self.planets[ti].y),
                        _ => (self.fleets[ti].x, self.fleets[ti].y),
                    };
                    let dd = len(dest.0 - cur.0, dest.1 - cur.1);
                    let ts = step_of(tg.speed);
                    let me = len(cur.0 - fl.x, cur.1 - fl.y);
                    let mut k = (me / step) * 0.5;
                    if dd / ts <= k {
                        k = dd / ts;
                    }
                    if dd as f64 > 0.0001 {
                        aim = ((k * ts * (dest.0 - cur.0)) / dd + cur.0, (k * ts * (dest.1 - cur.1)) / dd + cur.1);
                    }
                }
                (cur, aim)
            }
            _ => ((fl.tx, fl.ty), (fl.tx, fl.ty)),
        }
    }

    /// What `Story_FleetArrivedSave(f)` reads: `+0x42`, `(int) +0x44`, `+0x16`, and for a fleet target its `+0x16`.
    fn arrival_record(&self, f: usize) -> (i16, i32, i16, i16) {
        let fl = &self.fleets[f];
        let target = fl.tx as i32;
        let target_id = if fl.kind == order::FLEET { self.fleets.get(target.max(0) as usize).map_or(0, |g| g.story_id) } else { 0 };
        (fl.kind, target, fl.story_id, target_id)
    }

    /// Spec §4.3. Returns true if a battle started.
    fn arrive(&mut self, f: usize, cur: (f32, f32), ctx: &StarmapContext, rng: &mut Streams, arrivals: &mut dyn Arrivals,
        ev: &mut Vec<StarmapEvent>) -> Option<(f32, f32)> {
        let mut pos = cur;
        let (kind, target, fleet_id, target_fleet_id) = self.arrival_record(f);
        ev.push(StarmapEvent::Arrived { fleet: f, kind, target, fleet_id, target_fleet_id });
        let fl = self.fleets[f].clone();
        let t = fl.target_index();
        let to_unowned_planet =
            fl.order == order::MOVE && fl.kind == order::PLANET && self.planets.get(t).is_some_and(|p| p.owner == 0);
        if fl.flags & flag::COLONISING != 0 {
            if to_unowned_planet {
                ev.push(StarmapEvent::Colonise { fleet: f, planet: t });
                arrivals.colonise(t, fl.owner, &mut self.planets[t], rng);
                ev.push(StarmapEvent::StorySpecial([10, fl.owner as i32, t as i32, 0, 0, 0, 0, 0]));
                self.delete(f, ctx, arrivals, ev);
            }
        } else if fl.flags & flag::TERRAFORMING != 0 {
            if to_unowned_planet {
                let surface = arrivals.terraform(t, fl.owner);
                ev.push(StarmapEvent::Terraform { fleet: f, planet: t, surface });
                ev.push(StarmapEvent::StorySpecial([0x18, fl.owner as i32, t as i32, 0, 0, 0, 0, 0]));
                self.delete(f, ctx, arrivals, ev);
            }
        } else if fl.state & 2 != 0 {
            ev.push(StarmapEvent::ScriptedArrival { fleet: f });
            ev.push(StarmapEvent::StorySpecial([0x10, fl.story_id as i32, f as i32, 0, 0, 0, 0, 0]));
            self.delete(f, ctx, arrivals, ev);
        } else {
            if fl.order == order::MOVE && fl.kind == order::PLANET {
                let p = self.planets[t];
                if fl.flags & flag::VIRUS == 0 {
                    if p.epidemic > 0 {
                        self.fleets[f].flags |= flag::VIRUS;
                    }
                } else if p.owner == fl.owner || (p.owner != 0 && fl.owner == TRADERS) {
                    ev.push(StarmapEvent::VirusSpread { fleet: f, planet: t });
                }
                let mut best = (0usize, -1i32);
                if fl.owner != 9 && fl.owner != 10 {
                    for g in 1..self.fleets.len() {
                        let o = &self.fleets[g];
                        if g != f
                            && o.owner == fl.owner
                            && o.order == 0
                            && o.flags & (flag::COLONISING | flag::TERRAFORMING) == 0
                            && o.state & 1 == 0
                            && (fl.x - o.x).abs() + (fl.y - o.y).abs() < 10.0
                            && best.1 < o.strength
                        {
                            best = (g, o.strength);
                        }
                    }
                }
                if best.0 > 0 {
                    self.merge(best.0, f, ctx, arrivals, ev);
                }
            }
            let fl = self.fleets[f].clone();
            if fl.alive() && (fl.order == order::MOVE || fl.order == order::ATTACK) && fl.kind == order::FLEET
                && self.fleets[t].flags & flag::WORMHOLE != 0
            {
                for (w, h) in self.wormholes.iter_mut().enumerate().skip(1) {
                    for (here, other, to) in [(h.fleet_a, h.fleet_b, (h.bx, h.by)), (h.fleet_b, h.fleet_a, (h.ax, h.ay))] {
                        if fl.tx == here as i32 as f32 {
                            h.revealed_a = true;
                            h.revealed_b = true;
                            if let Some(o) = self.fleets.get_mut(other as usize) {
                                o.visible[fl.owner as usize] = 1;
                            }
                            self.fleets[f].radar = 0.0;
                            pos = to;
                            ev.push(StarmapEvent::Wormhole { fleet: f, wormhole: w });
                        }
                    }
                }
                // The last argument is the loop's last index, OsszHanyWormhole (exe 0x596c71: a local copied before
                // each increment); with no wormhole that local is never written: uninitialised, 0 here [original quirk].
                let last = self.wormholes.len().saturating_sub(1) as i32;
                ev.push(StarmapEvent::StorySpecial([0x14, fl.owner as i32, 1, f as i32, last, 0, 0, 0]));
            }
            let fl = self.fleets[f].clone();
            if fl.alive() && fl.order == order::ATTACK && fl.kind == order::FLEET {
                let tf = self.fleets[t].flags;
                if tf & flag::PROTECTED != 0 {
                    ev.push(StarmapEvent::ProtectedTarget { fleet: f });
                    ev.push(StarmapEvent::StorySpecial([0x12, t as i32, 2, f as i32, 0, 0, 0, 0]));
                } else if tf & flag::WORMHOLE == 0 {
                    ev.push(StarmapEvent::SpaceBattle { fleet: f, target: t });
                    return Some(pos);
                }
            }
            if fl.alive() && fl.order == order::ATTACK && fl.kind == order::PLANET {
                if fl.flags & flag::PROTECTED != 0 {
                    ev.push(StarmapEvent::ProtectedTarget { fleet: f });
                    ev.push(StarmapEvent::StorySpecial([0x12, f as i32, 1, t as i32, 0, 0, 0, 0]));
                } else {
                    ev.push(StarmapEvent::PlanetAttack { fleet: f, planet: t });
                    return Some(pos);
                }
            }
            self.arrival_tail(f, &mut pos, ctx, arrivals, ev);
        }
        self.fleets[f].x = pos.0;
        self.fleets[f].y = pos.1;
        None
    }

    /// The rest of an arrival (spec §4.3), also after a battle: following an own fleet merges into it, then the
    /// route continues or a trader is paid (the position the step ends at goes to `pos`).
    fn arrival_tail(&mut self, f: usize, pos: &mut (f32, f32), ctx: &StarmapContext, arrivals: &mut dyn Arrivals,
        ev: &mut Vec<StarmapEvent>) {
        {
            let t = self.fleets[f].target_index();
            let fl = self.fleets[f].clone();
            // Following an own fleet ends in a merge (story arrival save happens either way).
            if fl.alive() && fl.order == order::MOVE && fl.kind == order::FLEET
                && self.fleets[t].flags & (flag::COLONISING | flag::TERRAFORMING) == 0
            {
                if self.fleets[t].owner == fl.owner {
                    self.merge(t, f, ctx, arrivals, ev);
                }
                let (kind, target, fleet_id, target_fleet_id) = self.arrival_record(f);
                ev.push(StarmapEvent::ArrivalResaved { fleet: f, kind, target, fleet_id, target_fleet_id });
            }
            let fl = self.fleets[f].clone();
            if fl.alive() {
                if !fl.is_trader() {
                    let following = fl.order == order::MOVE && fl.kind == order::FLEET
                        && self.fleets[fl.target_index()].flags & flag::WORMHOLE == 0;
                    if !following {
                        self.continue_route(f, ctx, ev);
                    }
                } else {
                    if fl.state & 1 == 0 {
                        let n = self.nearest_planet(f, 1e6);
                        let p = self.planets[n];
                        let race3 = ctx.empire_race.get(p.owner as usize) == Some(&3);
                        let amount = ((p.banks.min(1) as f64 * 0.5 + 1.0) * (p.trade_centres * 10000) as f64
                            * if race3 { 2.0 } else { 1.0 }) as i32;
                        arrivals.trader_paid(p.owner, amount);
                        ev.push(StarmapEvent::TraderPaid { fleet: f, planet: n, empire: p.owner, amount });
                    }
                    self.fleets[f].state |= 1;
                    *pos = (fl.x, fl.y);
                    self.order(f, order::NONE, order::POINT, 0.0, 0.0, ctx, ev);
                }
            }
        }
    }

    /// The movement loop's step after a battle paused it ([`Starmap::tick_until_battle`]): the rest of the
    /// arrival, the position the step computed (written back over a retreat the battle made, as in the exe), then
    /// the end of the fleet's step. The loop doesn't go on: the battle ended the tick's movement.
    pub fn resume(&mut self, p: Paused, ctx: &StarmapContext, rng: &mut Streams, arrivals: &mut dyn Arrivals) -> Vec<StarmapEvent> {
        let mut ev = Vec::new();
        let mut pos = p.pos;
        self.arrival_tail(p.fleet, &mut pos, ctx, arrivals, &mut ev);
        self.fleets[p.fleet].x = pos.0;
        self.fleets[p.fleet].y = pos.1;
        self.step_end(p.fleet, ctx, rng, arrivals, &mut ev);
        ev
    }

    /// The end of a fleet's step: an idle trader picks a destination; a fleet without ships (`+0x54 < 1`, not a
    /// wormhole or protected one) of the sending player is reported (`Multi_Send(0x1b, EN, f)`, unless the transfer
    /// panel holds it), an AI's goes.
    fn step_end(&mut self, f: usize, ctx: &StarmapContext, rng: &mut Streams, arrivals: &mut dyn Arrivals, ev: &mut Vec<StarmapEvent>) {
        self.trader_destination(f, ctx, rng, ev);
        let fl = &self.fleets[f];
        if fl.alive()
            && fl.ship_count < 1
            && fl.flags & (flag::WORMHOLE | flag::PROTECTED) == 0
            && ctx.sends.get(fl.owner as usize).copied().unwrap_or(false)
            && !ctx.transfer.is_some_and(|(a, b)| f as i32 == a || f as i32 == b)
        {
            ev.push(StarmapEvent::FleetEmptied { fleet: f });
        }
        if fl.alive() && fl.ship_count < 1 && fl.flags & (flag::WORMHOLE | flag::PROTECTED) == 0
            && !ctx.human_controlled[fl.owner as usize]
        {
            self.delete(f, ctx, arrivals, ev);
        }
    }

    /// Spec §4.4: an idle trader picks its next planet (stream 14).
    fn trader_destination(&mut self, f: usize, ctx: &StarmapContext, rng: &mut Streams, ev: &mut Vec<StarmapEvent>) {
        let fl = &self.fleets[f];
        if fl.order != 0 || !fl.is_trader() || fl.owner != TRADERS {
            return;
        }
        if fl.state & 1 != 0 && rng.range(TRADER_STREAM, 0, 100) != 0 {
            return;
        }
        let n = self.nearest_planet(f, 1e6);
        self.fleets[f].state &= !1;
        let (fx, fy) = (self.fleets[f].x, self.fleets[f].y);
        let mut pick = 0;
        rng.choice_begin();
        for (p, pl) in self.planets.iter().enumerate().skip(1) {
            if pl.owner == 0 || pl.owner >= 9 || p == n {
                continue;
            }
            let b = pl.owner as usize;
            let near_owner = self.planets[n].owner as usize;
            let a = if near_owner == 0 { b } else { near_owner };
            let (mut ack, mut level) = (1, 1);
            if a <= 8 && b <= 8 {
                let (k, l) = ctx.relations[a][b];
                ack = (k != 0) as i32 + 1;
                level = l as i32;
            }
            let d = (((fy - pl.y) * (fy - pl.y) + (fx - pl.x) * (fx - pl.x)) as f64).sqrt() as i32;
            let w = (900 - d) * if pl.trade_ports > 0 { 3 } else { 1 } * ack * level * 3;
            if rng.choice_offer(TRADER_STREAM, w) {
                pick = p;
            }
        }
        if pick > 0 {
            self.order(f, order::MOVE, order::PLANET, pick as f32, 0.0, ctx, ev);
        }
    }

    /// One `FRISSIT_Starmap` tick's fleet part (spec §3, §4, §6): radar tables every tick; speeds,
    /// radar and visibility from `ships` on ticks with `tick % 10 == 1`; then the movement loop. An
    /// "enemy approaching" sighting slows the clock at once (`ctx.speed_mult`), so the same tick's
    /// movement already runs at the new speed. Colonisation and terraforming arrivals go to `arrivals`
    /// mid-loop.
    pub fn tick(&mut self, ships: &[ShipDesign], ctx: &mut StarmapContext, rng: &mut Streams, arrivals: &mut dyn Arrivals)
        -> Vec<StarmapEvent> {
        let (mut ev, paused) = self.tick_until_battle(ships, ctx, rng, arrivals);
        if let Some(p) = paused {
            ev.extend(self.resume(p, ctx, rng, arrivals));
        }
        ev
    }

    /// [`Starmap::tick`] up to a battle: an arrival that starts one (`SpaceBattle`, `PlanetAttack`, the last event)
    /// pauses the movement loop, so the caller can run `SpaceWarIndit`/`GroundWarIndit` on the whole game where the
    /// exe calls them, then [`Starmap::resume`] the step (no later fleet moves in this tick).
    pub fn tick_until_battle(&mut self, ships: &[ShipDesign], ctx: &mut StarmapContext, rng: &mut Streams,
        arrivals: &mut dyn Arrivals) -> (Vec<StarmapEvent>, Option<Paused>) {
        let mut ev = Vec::new();
        self.radar.refresh(&ctx.radar_rules, ctx);
        if ctx.tick % 10 == 1 {
            self.refresh(ships, ctx, &mut ev);
            // `SetIdoLelassit(1)` inside the visibility pass (exe 0x595570); nothing else in the
            // refresh reads the speed, the movement step does (0x596424).
            if ev.iter().any(|e| matches!(e, StarmapEvent::EnemyApproaching { .. })) {
                ctx.slow_down();
            }
        }
        let ctx = &*ctx;
        for &f in &ctx.in_battle {
            if let Some(fl) = self.fleets.get_mut(f) {
                fl.flags |= flag::IN_BATTLE;
            }
        }
        for f in 1..self.fleets.len() {
            if !self.fleets[f].alive() {
                continue;
            }
            self.upkeep(f, ctx, arrivals, &mut ev);
            let fl = self.fleets[f].clone();
            if fl.order > 0 && fl.speed > 0.0 {
                if fl.flags & flag::IN_BATTLE == 0 {
                    let mut step = step_of(fl.speed);
                    if fl.kind == order::FLEET {
                        step = (step as f64 + 0.003) as f32;
                    }
                    let (cur, mut aim) = self.aim(&fl, step);
                    let d = len(cur.0 - fl.x, cur.1 - fl.y);
                    let mut da = len(aim.0 - fl.x, aim.1 - fl.y);
                    if da == 0.0 {
                        aim = cur;
                        da = d;
                    }
                    let reach = ctx.speed_mult as f32 * step;
                    if d <= reach || (fl.kind == order::FLEET && d <= 1.0) {
                        if let Some(pos) = self.arrive(f, cur, ctx, rng, arrivals, &mut ev) {
                            return (ev, Some(Paused { fleet: f, pos }));
                        }
                    } else {
                        let g = &mut self.fleets[f];
                        g.x = ((aim.0 - fl.x) * reach) / da + fl.x;
                        g.y = ((aim.1 - fl.y) * reach) / da + fl.y;
                    }
                } else {
                    self.order(f, order::NONE, order::POINT, 0.0, 0.0, ctx, &mut ev);
                }
            }
            self.step_end(f, ctx, rng, arrivals, &mut ev);
        }
        (ev, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(n: usize) -> Starmap {
        let mut fleets = vec![Fleet { owner: DELETED, ..Default::default() }];
        for i in 0..n {
            fleets.push(Fleet { owner: 1, x: 10.0 * i as f32, speed: 1.0, ship_count: 1, ..Default::default() });
        }
        Starmap { fleets, waypoints: vec![Waypoint::default()], ..Default::default() }
    }

    fn ctx() -> StarmapContext {
        StarmapContext { tick: 2, speed_mult: 1, alive: [true; 19], human_controlled: [true; 19], ..Default::default() }
    }

    #[test]
    fn a_colonisation_is_seen_by_later_fleets_of_the_same_tick() {
        struct Take(Vec<usize>);
        impl Arrivals for Take {
            fn colonise(&mut self, p: usize, owner: u8, star: &mut StarPlanet, _: &mut Streams) {
                self.0.push(p);
                star.owner = owner;
            }
        }
        let mut m = map(2);
        m.planets = vec![StarPlanet::default(), StarPlanet { x: 0.0, y: 0.0, ..Default::default() }];
        for f in 1..=2 {
            // Both colonists sit on the planet's approach point (x, y + 2).
            m.fleets[f] = Fleet { x: 0.0, y: 2.0, flags: flag::COLONISING, order: order::MOVE, kind: order::PLANET, tx: 1.0, ..m.fleets[f].clone() };
        }
        let (mut c, mut rng, mut take) = (ctx(), Streams::default(), Take(Vec::new()));
        let ev = m.tick(&[], &mut c, &mut rng, &mut take);
        assert_eq!(take.0, [1], "only the first colonist takes the planet");
        assert_eq!(ev.iter().filter(|e| matches!(e, StarmapEvent::Colonise { .. })).count(), 1);
        assert!(!m.fleets[1].alive(), "the colonist is used up");
        assert!(m.fleets[2].alive() && m.fleets[2].order == order::NONE, "the second one's target became owned: order cleared");
        assert!(ev.contains(&StarmapEvent::ColonyShipFreed { empire: 1 }), "and its colony ship returns to the pool");
    }

    fn ship(fleet: usize, speed: f32, design_type: i32) -> ShipDesign {
        ShipDesign { fleet, speed, design_type, radar: 0, stealth: 0 }
    }

    #[test]
    fn waypoints_queue_after_a_point_move_and_are_followed_in_order() {
        let (mut m, c, mut ev) = (map(1), ctx(), Vec::new());
        m.order(1, order::ADD_WAYPOINT, 0, 5.0, 0.0, &c, &mut ev); // idle: becomes a plain move
        assert_eq!((m.fleets[1].order, m.fleets[1].tx, m.fleets[1].first_waypoint), (order::MOVE, 5.0, 0));
        m.order(1, order::ADD_WAYPOINT, 0, 5.0, 5.0, &c, &mut ev);
        m.order(1, order::ADD_WAYPOINT, 0, 0.0, 5.0, &c, &mut ev);
        let first = m.fleets[1].first_waypoint as usize;
        assert_eq!((m.waypoints[first].x, m.waypoints[first].y), (5.0, 5.0));
        m.continue_route(1, &c, &mut ev);
        assert_eq!((m.fleets[1].tx, m.fleets[1].ty), (5.0, 5.0));
        assert_eq!(m.waypoints[first].next, -1, "a visited node is freed");
        m.continue_route(1, &c, &mut ev);
        m.continue_route(1, &c, &mut ev);
        assert_eq!(m.fleets[1].order, order::NONE, "route finished");
    }

    #[test]
    fn patrol_reappends_each_leg_with_the_fleets_position() {
        let (mut m, c, mut ev) = (map(1), ctx(), Vec::new());
        m.order(1, order::MOVE, order::POINT, 5.0, 0.0, &c, &mut ev);
        m.order(1, order::ADD_PATROL_WAYPOINT, 0, 5.0, 5.0, &c, &mut ev);
        m.order(1, order::ADD_PATROL_WAYPOINT, 0, 0.0, 5.0, &c, &mut ev);
        m.fleets[1].x = 5.0; // arrived at the first point
        m.continue_route(1, &c, &mut ev);
        // The popped node now holds (5, 0) at the tail: the route loops forever.
        let mut node = m.fleets[1].first_waypoint as usize;
        let mut tail = node;
        while node != 0 {
            tail = node;
            node = m.waypoints[node].next as usize;
        }
        assert_eq!((m.waypoints[tail].x, m.waypoints[tail].y), (5.0, 0.0));
        assert_eq!(m.fleets[1].order, order::MOVE);
    }

    #[test]
    fn moving_and_arriving_at_a_point() {
        let (mut m, mut c, mut rng) = (map(1), ctx(), Streams::default());
        let mut ev = Vec::new();
        m.order(1, order::MOVE, order::POINT, 1.0, 0.0, &c, &mut ev);
        m.tick(&[], &mut c, &mut rng, &mut ());
        assert_eq!(m.fleets[1].x, (1.0f64 * 0.3) as f32, "one step of speed × 0.3");
        for _ in 0..3 {
            m.tick(&[], &mut c, &mut rng, &mut ());
        }
        assert_eq!((m.fleets[1].x, m.fleets[1].order), (1.0, order::NONE));
    }

    /// Moves the ship counts over, as the game's hook does with the units.
    struct Units(Vec<(usize, usize)>);
    impl Arrivals for Units {
        fn merge_units(&mut self, into: usize, from: usize, map: &mut Starmap) {
            assert!(map.fleets[from].alive(), "the units move before the slot is freed");
            map.fleets[into].ship_count += std::mem::take(&mut map.fleets[from].ship_count);
            self.0.push((into, from));
        }
    }

    #[test]
    fn a_merge_moves_the_units_before_the_radar_flag_rule() {
        let (mut m, mut c, mut ev) = (map(2), ctx(), Vec::new());
        c.human_controlled = [false; 19];
        // The destination holds one colony ship: after the merge it has two ships, so bit 5 stays.
        m.fleets[1].colony_ships = 1;
        m.fleets[1].flags |= flag::RADAR_IGNORED;
        let mut units = Units(Vec::new());
        m.merge(1, 2, &c, &mut units, &mut ev);
        assert_eq!(units.0, [(1, 2)]);
        assert!(!m.fleets[2].alive());
        assert_eq!(m.fleets[1].ship_count, 2);
        assert_ne!(m.fleets[1].flags & flag::RADAR_IGNORED, 0);
    }

    #[test]
    fn deleting_a_fleet_stops_its_followers() {
        let (mut m, c, mut ev) = (map(2), ctx(), Vec::new());
        m.order(2, order::MOVE, order::FLEET, 1.0, 0.0, &c, &mut ev);
        m.delete(1, &c, &mut (), &mut ev);
        assert!(!m.fleets[1].alive());
        assert_eq!(m.fleets[2].order, order::NONE);
    }

    #[test]
    fn speed_is_the_slowest_ship_unless_overridden() {
        let mut m = map(2);
        m.fleets[2].speed_override = 3.0;
        let ships = [ship(1, 2.0, 0), ship(1, 4.0, 80) /* slow class: 1.0 */, ship(2, 2.0, 0)];
        let (c, mut ev) = (StarmapContext { tick: 1, ..ctx() }, Vec::new());
        m.refresh(&ships, &c, &mut ev);
        assert_eq!((m.fleets[1].speed, m.fleets[2].speed), (1.0, 3.0));
        m.refresh(&[], &c, &mut ev);
        assert_eq!(m.fleets[1].speed, 0.06, "no ships: crawl speed");
    }
}
