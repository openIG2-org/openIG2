//! The real-time battles in canonical multiplayer (`re/rules/multiplayer.md` §7, `space-combat.md` §7.12–§7.13,
//! `ground-combat.md` §11.17): the original's MP flow on the sim clock, with the humans' input as lockstep orders.
//!
//! - A space battle routed to real time is set up here (`SpaceWar_NewBattle`), every human is on its screen, and
//!   while it runs (`SpWar_MultiCsata_folyamatban`, exe 0xBAD114) the strategic systems stop (`Fake_FRISSITES`'s
//!   battle branch). Every active slot ticks `IdoSpeedSzorzoBattle` times per sim tick (`FRISSIT_SpaceWar` runs
//!   them all in MP). The humans' orders are [`Game::space_order`] (`MULTIGET_SpaceWar`). The end check closes the
//!   battle for everybody; each human leaving the screen sends 0x44d ([`Game::space_battle_left`]) and when all
//!   have, `SpaceWarVege` ends it ([`Game::space_battles_settle`], the handler's tail).
//! - A ground war routed to real time (`GrWar_MultiCsata_folyamatban` 0xBAD13C, planet 0xBAD140) runs `WF::MI` and
//!   `WF::Refresh` for its planet every tick whatever anybody's screen shows; `WF::Refresh` ends it as soon as one
//!   side holds the field (MP: no countdown); the dropships fly once per tick (the 100 Hz UI's every fifth pass).
//!
//! None of this runs in [`ViewMode::Classic`]: single player and the trace replays are unchanged, unless the client
//! asks for single player's own real-time battles ([`SpBattles::live`]): then a real-time route starts the battle as
//! `SpaceWarIndit` does in SP (screen 0xB, speed 1, paused), `FRISSIT_SpaceWar` runs the slots while the battle
//! screen is up (`AktKep == 0xB`), the human's orders arrive as in MP (`MULTIGET_SpaceWar` runs in SP too), and
//! leaving a finished battle's screen ends it (`SpaceWar_Kilep` → `SpaceWarVege`).

use super::*;
use crate::ground::{Clock, GroundEvent, GroundOrder, Start};
use crate::spacebattle::{
    self, Battle, BattleRules, BattleWorld, Order, SetupBuilding, SetupFleet, SetupInput, SetupPlanet, SetupShip, ShipType, Tick,
};
use crate::spacewar::CombatWorld;
use crate::story::Participant;

/// Exe globals of the MP battles.
pub(super) mod at {
    /// `SpWar_MultiCsata_folyamatban` (= `save::va::MP_SPACE_BATTLE`), `…_vegetert[e]` (i32 per empire).
    pub const SPACE_RUNNING: u32 = crate::save::va::MP_SPACE_BATTLE;
    pub const SPACE_LEFT: u32 = 0xBAD118;
    /// `GrWar_MultiCsata_folyamatban`, `GrWar_MultiCsata_bolygo`.
    pub const GROUND_RUNNING: u32 = 0xBAD13C;
    pub const GROUND_PLANET: u32 = 0xBAD140;
    /// `SpWarKiNyert`, `SpWarVesztesVisszaVonult`.
    pub const SP_WINNER: u32 = 0xBAA1EC;
    pub const SP_LOSER_RETREATED: u32 = 0xBAA1DE;
    /// `IdoPause`, `asteroidak`, the story's battle arrays (`StoryUrharcViszony`, `…ZachkinBarkacs`,
    /// `…Visszavonulhate`).
    pub const PAUSE: u32 = 0xBAE264;
    pub const ASTEROIDS: u32 = 0xE72DD2;
    pub const STORY_PACT: u32 = 0xF26150;
    pub const STORY_ZACHKIN: u32 = 0xF2619C;
    pub const STORY_RETREAT: u32 = 0xF261A0;
    /// `IdoMultiPlayerBattle`.
    pub const MP_BATTLE_TICKS: u32 = 0xBAE054;
}

/// The battle slots (`spw[10]`, exe 0xE417C0) and what goes with them. Synced state (in the snapshot).
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RealTime {
    /// Empty until the first battle, then ten slots.
    pub slots: Vec<Battle>,
    /// `tipusnevek` (exe 0x91F488): the setup rewrites the footprints.
    pub types: Vec<ShipType>,
    /// `AktSpaceWar` (exe 0xE41788): the slot `SpaceWar_NewBattle` used last (`SpaceWarVege` ends it).
    pub current: usize,
}

/// The single-player battle screen (ours, classic view; per machine, not in the state).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpBattles {
    /// The client shows real-time battles. Off (the replays, the tools): a real-time route is only reported
    /// ([`super::BattleEvent::SpaceRealTime`]).
    pub live: bool,
    /// `AktKep == 0xB`: the battle screen is up, so `FRISSIT_SpaceWar` runs the slots (`space-combat.md` §7.1).
    pub on_screen: bool,
    /// `spacewarelfade` (exe 0xE72E14): the end check waits while it is strictly between 0 and 1.
    pub fade: f32,
}

/// What a real-time battle did (canonical multiplayer, or single player's own with [`SpBattles::live`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealTimeEvent {
    /// `SpaceWar_NewBattle` put the fight into `slot` (joined when it was already running).
    SpaceStarted { slot: usize },
    /// The end check found the battle in `slot` over: the screens leave it; `winner` (+0xc48).
    SpaceOver { slot: usize, winner: i32 },
    /// Every human has left the screen: `SpaceWarVege`.
    SpaceEnded { slot: usize, winner: i32 },
    /// `StartGrWar(p)`: the war on `planet` runs (or ended at once: `holder` = the side holding the field).
    GroundStarted { planet: usize, holder: Option<i32> },
    /// `GroundWarFinished(p, s, false)` from `WF::Refresh`.
    GroundEnded { planet: usize, holder: i32 },
}

/// The strategic records a battle tick touches (`BattleWorld` on the state).
struct StateWorld<'a> {
    combat: CombatWorld,
    /// The battle planet's record and building blocks (copies, written back after the tick).
    planet: Option<Vec<u8>>,
    buildings: Vec<[u8; crate::save::BUILDING_SIZE]>,
    building_hp: &'a [i32],
    centres: Vec<(i32, i32)>,
    /// Building slots destroyed this tick (`EpuletDelete` after the tick).
    destroyed: Vec<usize>,
}

impl BattleWorld for StateWorld<'_> {
    fn ship_fleet(&self, ship: i32) -> i32 {
        self.combat.ships.get(ship.max(0) as usize).map_or(0, |s| s.fleet as i32)
    }
    fn ship_attack(&self, owner: i32, ship: i32) -> i32 {
        let d = self.combat.ships.get(ship.max(0) as usize).map_or(-1, |s| s.design);
        self.combat.designs.get(owner.max(0) as usize).and_then(|t| t.get(d.max(0) as usize)).map_or(0, |d| d.attack)
    }
    fn delete_ship(&mut self, ship: i32) {
        if (ship as usize) < self.combat.ships.len() {
            self.combat.delete_ship(ship as usize);
        }
    }
    fn fleet_empty(&self, fleet: i32) -> bool {
        self.combat.fleets.get(fleet.max(0) as usize).is_some_and(|f| f.totals.ships == 0 && f.totals.fighters == 0)
    }
    fn delete_fleet(&mut self, fleet: i32) {
        if (fleet as usize) < self.combat.fleets.len() {
            self.combat.delete_fleet(fleet as usize);
        }
    }
    fn fleet_owner(&self, fleet: i32) -> i32 {
        self.combat.fleets.get(fleet.max(0) as usize).map_or(0xff, |f| f.owner as i32)
    }
    fn delete_fighters(&mut self, fleet: i32, design: i32, n: i32) {
        self.combat.delete_vehicles(fleet as i16, design as i16, n);
    }
    fn remove_planet_ship(&mut self, _planet: i32, k: i32) {
        let Some(p) = self.planet.as_mut() else { return };
        let rd = |p: &[u8], o: usize| i32::from_le_bytes(p[o..o + 4].try_into().unwrap());
        let n = rd(p, 0x114);
        let mut j = k;
        while j < n && (0x100 + 2 * j as usize) < 0x114 {
            let o = 0xfe + 2 * (j as usize + 1);
            let v = [p[o], p[o + 1]];
            p[0xfe + 2 * j as usize..0x100 + 2 * j as usize].copy_from_slice(&v);
            j += 1;
        }
        p[0x114..0x118].copy_from_slice(&(n - 1).to_le_bytes());
    }
    /// Exe 0x474160: the damage adds up while below the type's hit points; at the limit `EpuletDelete` (carried out
    /// after the tick); the gun is gone when the slot's (now cleared) type's hit points ≤ its damage.
    fn damage_building(&mut self, _planet: i32, slot: i32, damage: i32) -> bool {
        let Some(b) = self.buildings.get_mut(slot.max(0) as usize) else { return false };
        let hp = |t: u8| self.building_hp.get(t as usize).copied().unwrap_or(0);
        let cur = i32::from_le_bytes(b[4..8].try_into().unwrap());
        if cur < hp(b[0]) {
            let d = damage.wrapping_add(cur);
            b[4..8].copy_from_slice(&d.to_le_bytes());
            if hp(b[0]) <= d {
                b[0] = 0;
                self.destroyed.push(slot as usize);
            }
        }
        hp(b[0]) <= i32::from_le_bytes(b[4..8].try_into().unwrap())
    }
    fn retreat_fleet(&mut self, fleet: i32) {
        let Some(o) = self.combat.fleets.get(fleet.max(0) as usize).map(|f| f.owner as usize) else { return };
        if o != 0xff {
            let c = self.centres.get(o).copied().unwrap_or((0, 0));
            self.combat.retreat(fleet as usize, c);
        }
    }
}

impl Game<'_> {
    /// Canonical multiplayer with a real-time battle on: the strategic systems wait (`Fake_FRISSITES`).
    pub fn mp_battle_running(&self) -> bool {
        self.view != ViewMode::Classic
            && (self.state.mem.i32(at::SPACE_RUNNING) != 0 || self.state.mem.i32(at::GROUND_RUNNING) != 0)
    }

    /// `Fake_FRISSITES`'s branch while an MP battle runs: no clock step, no strategic systems; only
    /// `GroundWarSetBattleStatus` (exe 0x47caa0), `IdoMultiPlayerBattle`, `FRISSIT_Grwar` unless a space battle
    /// runs, and `FRISSIT_SpaceWar` (`MULTISEND_SpWarSynch` only keeps the original's resync alive).
    pub(super) fn battle_tick(&mut self) {
        let first_event = self.events.len();
        self.ground_battle_status();
        let m = &mut self.state.mem;
        m.set_i32(at::MP_BATTLE_TICKS, m.i32(at::MP_BATTLE_TICKS).wrapping_add(1));
        self.battlefield_hand_offs();
        if self.state.mem.i32(at::SPACE_RUNNING) == 0 {
            self.grwar();
        }
        self.space_tick();
        self.post_messages(first_event);
    }

    // -------------------------------------------------------------------------------------------- space

    fn slots(&mut self) -> &mut Vec<Battle> {
        if self.realtime.slots.is_empty() {
            self.realtime.slots = (0..10).map(|_| Battle::slot()).collect();
        }
        &mut self.realtime.slots
    }

    /// `SpaceWarIndit`'s real-time branch (exe 0x5a7670) in canonical MP: every listed fleet's order is cleared
    /// (`FlottaMozgat(f, 0, …)`), `SpaceWar_NewBattle`, and every human goes to the battle screen: the battle is
    /// shown and the strategic game waits for it (`SpWar_MultiCsata_folyamatban`, the humans' `vegetert` cleared).
    pub(super) fn space_real_time(&mut self, fleets: &[usize], bp: usize) {
        let g = self.globals();
        {
            let ctx = self.starmap_context(&g);
            let mut map = self.state.starmap();
            let mut ev = Vec::new();
            for &f in fleets {
                map.order(f, 0, 0, 0.0, 0.0, &ctx, &mut ev);
            }
            self.state.store_starmap(&map);
            self.events.extend(ev.into_iter().map(GameEvent::Starmap));
        }
        let m = &self.state.mem;
        let fleet_records: Vec<SetupFleet> = self
            .state
            .fleets
            .iter()
            .map(|r| SetupFleet {
                owner: r[0x15],
                story_id: i16::from_le_bytes([r[0x16], r[0x17]]),
                hp: i32::from_le_bytes(r[0x6c..0x70].try_into().unwrap()).wrapping_add(i32::from_le_bytes(r[0x70..0x74].try_into().unwrap())),
                x: f32::from_le_bytes(r[0x30..0x34].try_into().unwrap()),
                y: f32::from_le_bytes(r[0x34..0x38].try_into().unwrap()),
            })
            .collect();
        let ships: Vec<SetupShip> = self
            .state
            .ships
            .iter()
            .map(|c| SetupShip { fleet: i16::from_le_bytes([c[0], c[1]]), design: i16::from_le_bytes([c[2], c[3]]), experience: c[4] })
            .collect();
        let combat = self.state.combat();
        let planet_record = (bp > 0).then(|| {
            let r = self.planet_rec(bp);
            let n = i32::from_le_bytes(r[0x114..0x118].try_into().unwrap()).clamp(0, 10) as usize;
            let slots = m.i32(va::BUILDING_SLOTS + 4 * bp as u32).max(0) as usize;
            let buildings = self.state.buildings.get(bp).map_or(Vec::new(), |b| {
                b.iter()
                    .skip(1)
                    .take(slots)
                    .map(|c| SetupBuilding { kind: c[0], finished: i16::from_le_bytes([c[8], c[9]]) == 0, on: c[0x19] & 1 != 0 })
                    .collect()
            });
            SetupPlanet { owner: r[0x20], ships: (1..=n).map(|k| i16::from_le_bytes([r[0xfe + 2 * k], r[0xff + 2 * k]])).collect(), buildings }
        });
        let mut inp = SetupInput {
            fleets: fleets.iter().map(|&f| f as i32).collect(),
            planet: bp as i32,
            ground_war: m.u8(super::battles::at::SP_PLANET_FLAG) != 0,
            human: g.en as i32,
            human_controlled: g.human,
            // The canonical view runs the battles by the original's MP rules (`multiplayer.md` §7).
            multiplayer: g.canonical || g.multiplayer,
            asteroids: m.u8(at::ASTEROIDS) != 0,
            story_pact: std::array::from_fn(|k| m.i32(at::STORY_PACT + 4 * k as u32)),
            story_zachkin: m.i32(at::STORY_ZACHKIN),
            story_retreat: std::array::from_fn(|k| m.i32(at::STORY_RETREAT + 4 * k as u32)),
            fleet_records: &fleet_records,
            ships: &ships,
            vehicles: &combat.vehicles,
            designs: &combat.designs,
            planet_record: planet_record.as_ref(),
            defence_fleets: None,
        };
        let rules = self.rules.space.setup_rules();
        if self.realtime.types.is_empty() {
            self.realtime.types = self.rules.space.ship_types.clone();
        }
        let mut rng = self.state.rng();
        self.slots();
        let RealTime { slots, types, .. } = &mut self.realtime;
        let got = spacebattle::new_battle(slots, &mut inp, &rules, types, &mut rng);
        self.state.store_rng(&rng);
        let Some(slot) = got else { return };
        self.realtime.current = slot;
        self.realtime.slots[slot].shown = true;
        if !g.canonical {
            // SP: `AktKepKell = 0xB`, `SetIdoSpeed(1, false)`, `SetIdoPause(true, false)` (spec §4 step 6).
            self.set_speed(1, false);
            self.set_pause(true, false);
            self.events.push(GameEvent::RealTime(RealTimeEvent::SpaceStarted { slot }));
            return;
        }
        let m = &mut self.state.mem;
        m.set_i32(at::SPACE_RUNNING, 1);
        for e in 1..9u32 {
            m.set_i32(at::SPACE_LEFT + 4 * e, 0);
        }
        self.events.push(GameEvent::RealTime(RealTimeEvent::SpaceStarted { slot }));
    }

    /// The tick's switches (`BattleRules`) from the state.
    fn battle_rules(&self) -> BattleRules {
        let m = &self.state.mem;
        let adat = |o: u32| m.i32(va::STORY_ADAT + o);
        let canonical = self.view != ViewMode::Classic;
        BattleRules {
            multiplayer: canonical || m.u8(va::MULTIPLAYER) != 0,
            story_hit_bonus: adat(0x78).wrapping_add(adat(0x7c)),
            story_damage_bonus: adat(0x80).wrapping_add(adat(0x84)),
            paused: m.u8(at::PAUSE) != 0,
            no_ai: m.i32(va::AI_OFF) != 0,
            story_no_retreat_check: adat(0x94) != 0,
            story_no_retreat: adat(0x98) != 0,
            story_retreat_lock_a: m.u8(va::TUTORIAL_ACTIVE) != 0,
            story_retreat_lock_b: m.u8(va::TUTORIAL_PAUSED) != 0,
            // No screen fades in the canonical view.
            fade: if canonical { 0.0 } else { self.sp_battles.fade },
            ..self.rules.space.tick_rules()
        }
    }

    /// Run `f` on battle slot `slot` with a tick context over the state; the strategic side effects are stored back
    /// (ships, fleets, the battle planet, destroyed buildings, the follow-ups of deleted fleets).
    fn with_battle<T>(&mut self, slot: usize, f: impl FnOnce(&mut Battle, &mut Tick) -> T) -> T {
        let g = self.globals();
        let rules = self.battle_rules();
        let bp = self.realtime.slots[slot].planet.max(0) as usize;
        let mut world = StateWorld {
            combat: self.state.combat(),
            planet: (bp > 0).then(|| self.planet_rec(bp)),
            buildings: if bp > 0 { self.state.buildings.get(bp).cloned().unwrap_or_default() } else { Vec::new() },
            building_hp: &self.rules.space.building_hp,
            centres: (0..EMPIRES as u32)
                .map(|e| (self.state.mem.i32(va::AI_CENTRE_X + 4 * e), self.state.mem.i32(va::AI_CENTRE_Y + 4 * e)))
                .collect(),
            destroyed: Vec::new(),
        };
        let map = self.state.starmap();
        world.combat.follow_targets = (1..map.fleets.len())
            .filter(|&x| map.fleets[x].alive() && map.fleets[x].order > 0 && map.fleets[x].kind == order::FLEET)
            .map(|x| (x, map.fleets[x].tx))
            .collect();
        let mut rng = self.state.rng();
        let mut events = Vec::new();
        let out = {
            let mut t = Tick { rules: &rules, weapon_words: &self.rules.space.weapon_words, rng: &mut rng, world: &mut world, events: &mut events };
            f(&mut self.realtime.slots[slot], &mut t)
        };
        self.state.store_rng(&rng);
        let StateWorld { mut combat, planet, mut buildings, destroyed, .. } = world;
        let fleet_events = std::mem::take(&mut combat.events);
        self.state.store_combat(&combat);
        if let Some(r) = planet {
            let at = va::PLANETS + (bp * PLANET_SIZE) as u32;
            self.state.mem.write(at + 0xfe, &r[0xfe..0x118]);
        }
        if bp > 0 {
            // The destroyed slots keep their type until `EpuletDelete` clears it with the rest.
            let old = self.state.buildings.get(bp).cloned().unwrap_or_default();
            for &s in &destroyed {
                if let (Some(b), Some(o)) = (buildings.get_mut(s), old.get(s)) {
                    b[0] = o[0];
                }
            }
            if let Some(dst) = self.state.buildings.get_mut(bp) {
                for (d, b) in dst.iter_mut().zip(&buildings) {
                    d[4..8].copy_from_slice(&b[4..8]);
                }
            }
            for s in destroyed {
                self.with_war(bp, |w| w.delete_building(s as i32));
            }
        }
        self.combat_aftermath(fleet_events, g.sends);
        out
    }

    /// `FRISSIT_SpaceWar` in MP (exe 0x55e7f0): every active slot runs `IdoSpeedSzorzoBattle` ticks; the end check
    /// that finds a battle over closes it for every screen (`+0x1fba`, then `Starmapvisszaallit`'s `+0x1fb8`),
    /// returns the time to the program speed (`SetIdoLelassit(0)`) and starts the ground war of a won planet battle.
    pub(super) fn space_tick(&mut self) {
        let sp = self.sp_battles;
        if (self.view == ViewMode::Classic && !(sp.live && sp.on_screen)) || self.realtime.slots.is_empty() {
            return;
        }
        let speed = self.state.mem.i32(va::BATTLE_SPEED);
        for slot in 0..self.realtime.slots.len() {
            for _ in 0..speed {
                if !self.realtime.slots[slot].active {
                    break;
                }
                if self.with_battle(slot, |b, t| b.step(t)) {
                    self.space_over(slot);
                }
            }
        }
    }

    /// The tick's step 4 on the (shown) battle (spec §7.11).
    fn space_over(&mut self, slot: usize) {
        let g = self.globals();
        let b = &mut self.realtime.slots[slot];
        b.ended = true;
        let winner = b.winner;
        let mem = &self.state.mem;
        let fleets = &self.state.fleets;
        let due = b.ground_war_due(|f| fleets.get(f.max(0) as usize).map_or(0, |r| i32::from_le_bytes(r[0x5c..0x60].try_into().unwrap())));
        let planet = b.planet.max(0) as usize;
        let speed = crate::time::slowed_down(mem.i32(va::IDO_SPEED), g.multiplayer, g.speed_locked, g.program_speed);
        self.state.mem.set_i32(va::IDO_SPEED, speed);
        self.events.push(GameEvent::RealTime(RealTimeEvent::SpaceOver { slot, winner }));
        if let Some(f) = due {
            self.ground_war(f as usize, planet);
        }
    }

    /// An order from the battle screen of empire `issuer` (`MULTIGET_Spacewar`, spec §7.12); refused when the slot
    /// isn't running or names something that isn't there. Canonical MP also refuses orders for another empire's
    /// units and guns (the screen only sends the player's own).
    pub fn space_order(&mut self, issuer: u8, slot: usize, o: Order) -> bool {
        if self.view == ViewMode::Classic && !(self.sp_battles.live && issuer as usize == self.globals().en) {
            return false;
        }
        if let Order::ArmAll = o {
            let mut rng = self.state.rng();
            let RealTime { slots, .. } = &mut self.realtime;
            for b in slots.iter_mut() {
                b.arm_all(&self.rules.space.weapons, &mut rng, issuer as i32);
            }
            self.state.store_rng(&rng);
            return true;
        }
        let Some(b) = self.realtime.slots.get(slot) else { return false };
        if !b.active || !b.order_valid(&o) {
            return false;
        }
        let own_unit = |u: usize| b.units[u].owner == issuer as i32;
        let mine = match o {
            Order::Move { unit, .. } | Order::Attack { unit, .. } | Order::AttackGun { unit, .. } | Order::Explode { unit }
            | Order::DirectFire { unit, .. } | Order::Weapons { unit, .. } => own_unit(unit),
            Order::GunTarget { .. } => b.planet_owner == issuer as i32,
            Order::FighterStance { side, .. } => side == issuer as usize,
            Order::Retreat | Order::ArmAll => true,
        };
        if !mine {
            return false;
        }
        // The screen only sends points inside the arena (`Battle::screen_point`); a peer's packet gets the same clamp.
        let o = match o {
            Order::Move { unit, x, y, z } => {
                let (x, y, z) = b.screen_point(x, y, z);
                Order::Move { unit, x, y, z }
            }
            o => o,
        };
        self.with_battle(slot, |b, t| b.order(t, issuer as i32, o));
        true
    }

    /// Message 0x44d: `issuer` has left the battle screen (`SpWar_MultiCsata_vegetert[issuer] = 1`).
    pub fn space_battle_left(&mut self, issuer: u8) {
        if self.view == ViewMode::Classic {
            // SP `SpaceWar_Kilep`: a finished battle ends at once (`SpaceWarVege`); one still running stays.
            let slot = self.realtime.current;
            if self.sp_battles.live && self.realtime.slots.get(slot).is_some_and(|b| b.active && b.ended) {
                self.space_battle_end(slot);
            }
            return;
        }
        if self.view != ViewMode::Classic && (1..EMPIRES).contains(&(issuer as usize)) {
            self.state.mem.set_i32(at::SPACE_LEFT + 4 * issuer as u32, 1);
        }
    }

    /// Ours (ADR 0011 §1): the start state of a lockstep session whose humans are `humans` (empires 1..=8):
    /// `MultiPlayerMode` = 1, `PlayerKi` = 1 for exactly those (a former human not among them: 0, AI), and the
    /// canonical view of `en`. The original's MP routing then sends only human-vs-human battles to real time.
    pub fn start_multiplayer(&mut self, en: u8, humans: &[u8]) {
        let m = &mut self.state.mem;
        m.set_u8(va::MULTIPLAYER, 1);
        for e in 1..9u8 {
            let k = va::PLAYER_KI + e as u32;
            if humans.contains(&e) {
                m.set_u8(k, 1);
            } else if m.u8(k) == 1 {
                m.set_u8(k, 0);
            }
        }
        self.canonical_view(en);
    }

    /// A real-time battle runs (either MP flag): the strategic clock waits, and a dropped human is worth waiting for
    /// (ADR 0011 §4).
    pub fn realtime_running(&self) -> bool {
        let m = &self.state.mem;
        self.view != ViewMode::Classic && (m.i32(at::SPACE_RUNNING) != 0 || m.i32(at::GROUND_RUNNING) != 0)
    }

    /// Ours (ADR 0011 §5): `empire`'s player is away (dropped past the grace period). It counts as having left the
    /// running space battle's screen; its units fight on without orders.
    pub fn player_away(&mut self, empire: u8) {
        if self.state.mem.i32(at::SPACE_RUNNING) != 0 {
            self.space_battle_left(empire);
        }
    }

    /// Ours (ADR 0011 §5): `empire`'s player is back: on the running space battle's screen again.
    pub fn player_back(&mut self, empire: u8) {
        if self.view != ViewMode::Classic && self.state.mem.i32(at::SPACE_RUNNING) != 0 && (1..9).contains(&empire) {
            self.state.mem.set_i32(at::SPACE_LEFT + 4 * empire as u32, 0);
        }
    }

    /// Ours (ADR 0011 §2): a ground-war order in the canonical view counts only for the flagged war's planet, for a
    /// live unit of the issuer's own, and (retreat, air strike) from a participant: the planet's owner or an empire
    /// with a live unit on the field. The exe takes any unit index a machine sends.
    pub(super) fn ground_order_allowed(&self, issuer: u8, p: usize, o: GroundOrder) -> bool {
        let m = &self.state.mem;
        if m.i32(at::GROUND_RUNNING) == 0 || m.i32(at::GROUND_PLANET) != p as i32 {
            return false;
        }
        let Some(f) = self.state.ground_field(p) else { return false };
        let mine = |i: usize| f.alive.get(i).copied().unwrap_or(false) && f.units.get(i).is_some_and(|u| u.owner == issuer as i32);
        match o {
            GroundOrder::Move { unit, .. }
            | GroundOrder::Waypoint { unit, .. }
            | GroundOrder::Target { unit, .. }
            | GroundOrder::Stop { unit }
            | GroundOrder::Detonate { unit, .. }
            | GroundOrder::Mines { unit, .. } => usize::try_from(unit).is_ok_and(mine),
            GroundOrder::Retreat | GroundOrder::AirStrike { .. } => self.planet_rec(p)[0x20] == issuer || (0..f.units.len()).any(mine),
        }
    }

    /// `MULTIGET_SpaceWar`'s tail at the tick boundary: while the battle runs, it goes on as long as a human
    /// (`PlayerKi == 1`, empires 1..=8) hasn't left the screen; otherwise `SpaceWarVege` on `AktSpaceWar`.
    pub fn space_battles_settle(&mut self) {
        if self.view == ViewMode::Classic || self.state.mem.i32(at::SPACE_RUNNING) == 0 {
            return;
        }
        let humans = self.state.human_controlled();
        let waiting = (1..9).any(|e| humans[e] && self.state.mem.i32(at::SPACE_LEFT + 4 * e as u32) == 0);
        if waiting {
            return;
        }
        self.state.mem.set_i32(at::SPACE_RUNNING, 0);
        let slot = self.realtime.current;
        if self.realtime.slots.get(slot).is_some_and(|b| b.active) {
            self.space_battle_end(slot);
        }
    }

    /// `SpaceWarVege` (exe 0x5706f0): `SpWarKiNyert`, `SpWarVesztesVisszaVonult = 0`, the story's
    /// `Story_SpaceBattleFinished(participants, n, 1)`; the slot closes.
    fn space_battle_end(&mut self, slot: usize) {
        let r = self.realtime.slots[slot].finish();
        let m = &mut self.state.mem;
        m.set_i32(at::SP_WINNER, r.winner);
        m.set_u8(at::SP_LOSER_RETREATED, 0);
        let list: Vec<Participant> =
            r.participants.iter().map(|p| Participant { owner: p.owner, slot_id: p.fleet, fleet_id: p.story_id }).collect();
        self.with_story(|st, w| st.space_battle_finished(&list, true, &ScenarioStories, w));
        self.events.push(GameEvent::RealTime(RealTimeEvent::SpaceEnded { slot, winner: r.winner }));
    }

    // -------------------------------------------------------------------------------------------- ground

    /// `GroundWarIndit`'s real-time branch in canonical MP (exe 0x59c2d0): `GrWar_MultiCsata_bolygo := p`, the
    /// in-progress flag, `StartGrWar(p)` (which may end the war at once).
    pub(super) fn ground_real_time(&mut self, p: usize) {
        let m = &mut self.state.mem;
        m.set_i32(at::GROUND_PLANET, p as i32);
        m.set_i32(at::GROUND_RUNNING, 1);
        let mut started = Start::Battle;
        self.with_war(p, |w| started = w.start());
        let holder = match started {
            Start::Battle => None,
            Start::Finished(s) => Some(s),
        };
        self.events.push(GameEvent::RealTime(RealTimeEvent::GroundStarted { planet: p, holder }));
        if let Some(s) = holder {
            self.ground_finished(p, s);
        }
    }

    /// The rest of `GroundWarFinished(p, s, false)` (the field's part ran in [`ground::War::finish`], the capture in
    /// [`Game::with_wars`]): `Story_GroundBattleFinishedCheck`, the MP flag clears.
    fn ground_finished(&mut self, p: usize, s: i32) {
        let buildings = self.state.mem.i32(va::BUILDINGS_OCCUPIED + 4 * p as u32);
        let owner = self.planet_owner(p) as i32;
        self.with_story(|st, w| st.ground_battle_finished_check(buildings, owner, &ScenarioStories, w));
        self.state.mem.set_i32(at::GROUND_RUNNING, 0);
        self.events.push(GameEvent::RealTime(RealTimeEvent::GroundEnded { planet: p, holder: s }));
    }

    /// `FRISSIT_Grwar`'s MP part (exe 0x4747c0) in the canonical view: `WF::MI` and `WF::Refresh` of the war's
    /// planet, every tick; `WF::Refresh` ends the war as soon as `GetStatus` names a holder.
    pub(super) fn grwar_mp(&mut self) {
        if self.state.mem.i32(at::GROUND_RUNNING) == 0 {
            return;
        }
        let p = self.state.mem.i32(at::GROUND_PLANET).max(0) as usize;
        if p == 0 || p > self.state.planet_count() {
            return;
        }
        let clock = Clock { speed: self.state.mem.i32(va::BATTLE_SPEED), paused: self.state.mem.u8(at::PAUSE) != 0 };
        let watching = self.watching();
        let mut ended = None;
        self.with_war(p, |w| {
            w.mi(clock.speed);
            w.refresh(clock, &watching);
            ended = w.events.iter().find_map(|e| if let GroundEvent::Ended { holder } = *e { Some(holder) } else { None });
        });
        if let Some(s) = ended {
            self.ground_finished(p, s);
        }
    }

    /// `KiMelyikBolygonVan[k]` (exe 0x9ABAE8): the planet on empire k + 1's colony screen; none in the canonical
    /// view.
    fn watching(&self) -> [i32; 18] {
        [0; 18]
    }

    /// The dropships of every battlefield (`WF::Refresh2`'s pass, exe 0x48ce10), once per tick in canonical MP:
    /// the original's 100 Hz UI tick runs it on every fifth pass, 20 times a second like the sim.
    pub(super) fn dropships_mp(&mut self) {
        let planets: Vec<usize> = (1..=self.state.planet_count()).filter(|&p| self.state.ground_field(p).is_some()).collect();
        self.with_wars(&planets, |w| w.dropships());
    }
}
