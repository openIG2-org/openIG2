//! The battles the strategic tick starts (`re/rules/info-messages.md` §3.2, `space-combat.md` §4–§5,
//! `ground-combat.md` §2–§6): the starmap's movement loop pauses at an arrival that attacks
//! ([`crate::fleet::Starmap::tick_until_battle`]) and the game runs `SpaceWarIndit` / `GroundWarIndit` there, on the
//! whole state, before the step resumes. Auto-resolved battles are carried out (`SimulateSpBattle`, `WF::Simulate`);
//! a battle the routing sends to the real-time screen is reported and left alone (the screens are the UI's).

use super::story_chain;
use super::*;
use crate::ground::{self, GroundContext, GroundEvent, GroundPlanet, GroundRoute, War};
use crate::save::building_from;
use crate::spacewar::{self, BattlePlanet, CombatEvent, Contender, DefenceSlot, Route};
use crate::story::Participant;

/// Exe globals of the battle routing (unsaved: the replay dumps them).
pub(super) mod at {
    /// `spFlottak` (6-byte entries from index 1: `+0` fleet, `+2` side), `SpWarHanyFlotta`, `spKozeliBolygo`,
    /// `SpWarKiNyert`, the planet-battle flag.
    pub const SP_FLEETS: u32 = 0xBAA1C0;
    pub const SP_COUNT: u32 = 0xBAA1E8;
    pub const SP_PLANET: u32 = 0xBAA1E4;
    pub const SP_WINNER: u32 = 0xBAA1EC;
    pub const SP_PLANET_FLAG: u32 = 0xB9C027;
    /// Debug switches: 0xB77230 always simulate (no lopsided), 0xB77234 ground always simulate, 0xB77238 space
    /// never lopsided, 0xB7723C ground never lopsided.
    pub const DEBUG_ALWAYS_SIMULATE: u32 = 0xB77230;
    pub const DEBUG_GROUND_SIMULATE: u32 = 0xB77234;
    pub const DEBUG_NEVER_LOPSIDED: u32 = 0xB77238;
    pub const DEBUG_GROUND_NO_LOPSIDED: u32 = 0xB7723C;
    /// `MultiplayerBattleStatusz`.
    pub const MP_BATTLE_STATUS: u32 = 0xBAE558;
    /// `EpuletekLeiro`'s tech byte is `rules.side.colony.buildings[kind].tech`.
    pub const BATTLES_WON: u32 = crate::save::va::BATTLES_WON;
}

/// `StoryAdat` offsets the battles read: `+0x88` the attack isn't an incident, `+0x40` the story planet, `+0x70`
/// / `+0x74` the human's ground damage bonus.
const ADAT_NO_INCIDENT: usize = 0x88;
const ADAT_STORY_PLANET: usize = 0x40;

/// What a battle did (the messages to the human are posted from these, `info-messages.md` §3.2).
#[derive(Debug, Clone, PartialEq)]
pub enum BattleEvent {
    /// `SimulateSpBattle(attacker, defender, planet)` ran (`planet` = `spKozeliBolygo`); `winner` = `SpWarKiNyert`.
    /// `human`: `EN` owns a participant or the battle planet, so `SpaceWarIndit` posts 0x0F/0x10 (lopsided, SP) or
    /// 0x11/0x12 (won/lost).
    Space { attacker: usize, defender: usize, planet: usize, winner: u8, lopsided_sp: bool, human: bool },
    /// The routing chose the real-time battle (`SpaceWar_NewBattle`): not run here.
    SpaceRealTime { attacker: usize, defender: usize, planet: usize },
    /// `Story_SpWarTiltas`: a story forbade the battle.
    SpaceForbidden { attacker: usize, defender: usize },
    /// `WF::Simulate` ran on `planet` (owned by `owner` then); `won`: the attacking fleet carries ground vehicles
    /// again (`GroundWarIndit` §6.3). `story_planet` = `StoryAdat+0x40` (no 0x0C message for it).
    Ground { fleet: usize, planet: usize, attacker: u8, owner: u8, lopsided: bool, won: bool, story_planet: i32 },
    /// The routing chose the real-time ground war (`StartGrWar`): not run here.
    GroundRealTime { fleet: usize, planet: usize },
    /// A story planet (`BolygoInfoIs(p, −0x21ac)`): `Grwar8020hoz` (campaign), not ported.
    GroundScripted { fleet: usize, planet: usize },
    /// The ground war's side effects (`EpuletDelete`, mines, abandonment).
    GroundSide { planet: usize, event: GroundEvent },
}

impl BattleEvent {
    /// The scrolling messages `SpaceWarIndit` / `GroundWarIndit` and a real-time war's `GroundWarFinished` post for `en`: `(id, planet for %1, important)`
    /// (`info-messages.md` §3.2; exe 0x5a7ad5 and 0x59c4d0..0x59c5cb).
    pub fn messages(&self, en: u8) -> Vec<(i32, Option<usize>, bool)> {
        match *self {
            BattleEvent::Space { winner, lopsided_sp, human: true, .. } => {
                let id = match (lopsided_sp, winner == en) {
                    (true, true) => 0x0f,
                    (true, false) => 0x10,
                    (false, true) => 0x11,
                    (false, false) => 0x12,
                };
                vec![(id, None, false)]
            }
            BattleEvent::Ground { planet, attacker, owner, lopsided, won, story_planet, .. } => {
                let mut v = Vec::new();
                let (to_owner, to_attacker) = match (lopsided, won) {
                    (true, false) => (Some(0x18), Some(0x19)),
                    (true, true) => (Some(0x17), Some(0x16)),
                    (false, true) => (Some(0x0b), (planet as i32 != story_planet).then_some(0x0c)),
                    (false, false) => (None, None),
                };
                if let Some(id) = to_owner.filter(|_| owner == en) {
                    v.push((id, Some(planet), true));
                }
                if let Some(id) = to_attacker.filter(|_| attacker == en) {
                    v.push((id, Some(planet), true));
                }
                v
            }
            // A real-time war's end (`GroundWarFinished(p, s, false)`, exe 0x47c982 / 0x47c9ad).
            BattleEvent::GroundSide { planet, event: GroundEvent::Taken { from, to, story_planet } } => {
                let mut v = Vec::new();
                if from == en {
                    v.push((0x0b, Some(planet), true));
                }
                if to == en && planet as i32 != story_planet {
                    v.push((0x0c, Some(planet), true));
                }
                v
            }
            _ => Vec::new(),
        }
    }
}

/// The defence-ship list after a battle (`SimulateSpBattle` exe 0x5a6a3f..0x5a6a8f): `slots[1..=n]` with the
/// destroyed ones zeroed close up in place; the entries past the new end keep what they held (the old last one
/// stays). Returns the new count.
pub(crate) fn compact_planet_ships(slots: &mut [i16], mut n: i32) -> i32 {
    let mut i = 1;
    while i <= n {
        if slots.get(i as usize).copied().unwrap_or(0) == 0 {
            n -= 1;
            if n < i {
                break;
            }
            for j in i..=n {
                let next = slots.get(j as usize + 1).copied().unwrap_or(0);
                if let Some(s) = slots.get_mut(j as usize) {
                    *s = next;
                }
            }
        } else {
            i += 1;
        }
    }
    n
}

impl Game<'_> {
    fn fleet_owner(&self, f: usize) -> u8 {
        self.state.fleets.get(f).map_or(DELETED, |r| r[0x15])
    }

    /// `spFlottak[1..=SpWarHanyFlotta]`'s fleets (`+0` i16; a negative one reads as no fleet).
    pub(super) fn sp_fleets(&self) -> Vec<usize> {
        let m = &self.state.mem;
        (1..=m.i32(at::SP_COUNT).max(0) as u32).map(|k| usize::try_from(m.i16(at::SP_FLEETS + 6 * k)).unwrap_or(usize::MAX)).collect()
    }

    fn fleet_i32(&self, f: usize, o: usize) -> i32 {
        self.state.fleets.get(f).map_or(0, |r| i32::from_le_bytes(r[o..o + 4].try_into().unwrap()))
    }

    pub(super) fn planet_rec(&self, p: usize) -> Vec<u8> {
        self.state.mem.read(va::PLANETS + (p * PLANET_SIZE) as u32, PLANET_SIZE)
    }

    pub(super) fn planet_owner(&self, p: usize) -> u8 {
        self.state.mem.u8(va::PLANETS + (p * PLANET_SIZE) as u32 + 0x20)
    }

    fn set_adat_i32(&mut self, off: usize, v: i32) {
        self.state.mem.set_i32(va::STORY_ADAT + off as u32, v);
    }

    /// `KemkedesTervLekezel(spy, owner, d)` (exe 0x53f060): `spy` has seen design `d` of `owner`: its hull tech
    /// (`+0x48`) and the techs of its pods (`+0x51..`, `+0x4c` of them) in the seen matrix.
    fn see_design(tech: &mut crate::colony_side::TechTable, designs: &[Vec<spacewar::Design>], spy: usize, owner: usize, d: i16) {
        let Some(des) = designs.get(owner).and_then(|t| t.get(d.max(0) as usize)) else { return };
        let rd = |o: usize| i32::from_le_bytes(des.raw[o..o + 4].try_into().unwrap());
        tech.mark_seen(rd(0x48).max(0) as usize, spy, owner);
        for k in 1..=rd(0x4c).clamp(0, 0x30) as usize {
            tech.mark_seen(des.raw[0x50 + k] as usize, spy, owner);
        }
    }

    /// The arrival `f → planet p` with an attack order (`fleet-movement.md` §4.3, exe 0x596d61..0x596e8e): a story
    /// planet (events −0x1F90 / −0x2031) or an undefended one goes straight to the ground war, else the space battle
    /// against the planet's nearest idle fleet within 15 (`SearchBolygohozKozeliFlotta(p, 15, own, idle)`).
    pub(super) fn planet_attack(&mut self, f: usize, p: usize) {
        let r = self.planet_rec(p);
        let events: Vec<i16> = (0..5).map(|k| i16::from_le_bytes([r[0x16a + 2 * k], r[0x16b + 2 * k]])).collect();
        if events.contains(&-0x1f90) || events.contains(&-0x2031) {
            self.ground_war(f, p);
            return;
        }
        let defender = self.shipyard().delivery_fleet(p, 15.0, true, true, false, false, 0);
        let ships = i32::from_le_bytes(r[0x114..0x118].try_into().unwrap());
        let guns: i32 = [0x25u8, 0x26, 0x24, 0x1f, 0x1e, 0x1d, 0x1c]
            .iter()
            .map(|&k| self.state.buildings.get(p).map_or(0, |b| {
                b.iter().skip(1).filter(|x| x[0] == k && u16::from_le_bytes([x[8], x[9]]) == 0 && x[0x19] & crate::colony::flag::WORKING != 0).count() as i32
            }))
            .sum();
        if defender != 0 || ships != 0 || guns != 0 {
            self.space_war(f, defender, p);
        } else {
            self.ground_war(f, p);
        }
    }

    /// `SpaceWarIndit(f1, f2, planet)` (exe 0x5a7670, `space-combat.md` §4).
    pub(super) fn space_war(&mut self, f1: usize, f2: usize, planet: usize) {
        if f2 == 0 && planet == 0 {
            return;
        }
        let g = self.globals();
        let o1 = self.fleet_owner(f1);
        let o2 = if f2 == 0 { self.planet_owner(planet) } else { self.fleet_owner(f2) };
        let count = if f2 == 0 { 1 } else { 2 };
        {
            let m = &mut self.state.mem;
            m.set_i32(at::SP_COUNT, count);
            m.write(at::SP_FLEETS + 6, &(f1 as i16).to_le_bytes());
            m.write(at::SP_FLEETS + 8, &1i16.to_le_bytes());
            m.write(at::SP_FLEETS + 12, &(f2 as i16).to_le_bytes());
            m.write(at::SP_FLEETS + 14, &2i16.to_le_bytes());
        }
        // The battle planet: the one attacked, else the nearest within 20 if either side owns it.
        let bp = if planet == 0 {
            self.state.mem.set_u8(at::SP_PLANET_FLAG, 0);
            let q = self.state.starmap().nearest_planet(f1, 20.0);
            let qo = self.planet_owner(q);
            if o1 != qo && self.fleet_owner(f2) != qo { 0 } else { q }
        } else {
            self.state.mem.set_u8(at::SP_PLANET_FLAG, 1);
            planet
        };
        self.state.mem.set_i32(at::SP_PLANET, bp as i32);
        // `KemkedesUrharcLekezel` (exe 0x53f0c0): every participant's ships and fighters are seen by every
        // participant's owner (itself included).
        let fleets: Vec<usize> = if f2 == 0 { vec![f1] } else { vec![f1, f2] };
        let combat = self.state.combat();
        let mut tech = self.state.tech();
        for &a in &fleets {
            for &b in &fleets {
                let (oa, ob) = (self.fleet_owner(a) as usize, self.fleet_owner(b) as usize);
                if a == 0 || oa >= 9 || ob >= 9 {
                    continue;
                }
                for s in combat.ships.iter().skip(1).filter(|s| s.fleet as usize == a && s.design != spacewar::NO_DESIGN) {
                    Self::see_design(&mut tech, &combat.designs, ob, oa, s.design);
                }
                for v in combat.vehicles.iter().skip(1).filter(|v| v.fleet as usize == a && v.design != spacewar::NO_DESIGN) {
                    if combat.designs.get(oa).and_then(|t| t.get(v.design as usize)).is_some_and(|d| d.class == 1) {
                        Self::see_design(&mut tech, &combat.designs, ob, oa, v.design);
                    }
                }
            }
        }
        self.state.store_tech(&tech);
        let (mut forbidden, mut simulate_ban) = (false, false);
        self.with_story(|st, w| {
            st.space_battle_started(f1 as i32, f2 as i32, &story_chain(), w);
            (forbidden, simulate_ban) = (st.space_forbidden, st.space_simulate_forbidden);
        });
        if adat_i32(&self.adat(), ADAT_NO_INCIDENT) == 0 || g.multiplayer {
            self.with_diplomacy(|d, w| d.war(w, o1 as usize, o2 as usize, false, 0));
        }
        self.set_adat_i32(ADAT_NO_INCIDENT, 0);
        if forbidden {
            self.events.push(GameEvent::Battle(BattleEvent::SpaceForbidden { attacker: f1, defender: f2 }));
            return;
        }
        // From here on the exe reads the fleets back from `spFlottak[1..=SpWarHanyFlotta]` (exe 0x5a77f2, 0x5a7b19,
        // `SpaceWar_NewBattle`): the story's `st_AddFleetToSpWar` may have added up to two (the route then goes
        // real-time); the simulation keeps `f1`/`f2`.
        let fleets = self.sp_fleets();
        let m = &self.state.mem;
        let contender = |f: usize| Contender {
            owner: self.fleet_owner(f),
            story_id: self.state.fleets.get(f).map_or(0, |r| i16::from_le_bytes([r[0x16], r[0x17]])),
            attack: self.fleet_i32(f, 0x60),
            hp: self.fleet_i32(f, 0x6c),
        };
        let input = spacewar::RouteInput {
            fleets: fleets.iter().map(|&f| contender(f)).collect(),
            planet_owner: if bp > 0 { self.planet_owner(bp) } else { 0 },
            ion_gun: if bp > 0 { m.i32(va::ION_GUN + 4 * bp as u32) } else { 0 },
            human_controlled: g.human,
            multiplayer: g.multiplayer,
            mp_battle_status: m.i32(at::MP_BATTLE_STATUS),
            debug_always_simulate: m.i32(at::DEBUG_ALWAYS_SIMULATE) != 0,
            debug_never_lopsided: m.i32(at::DEBUG_NEVER_LOPSIDED) != 0,
            story_simulate_ban: simulate_ban,
        };
        let en = g.en as u8;
        let human = fleets.iter().any(|&f| self.fleet_owner(f) == en) || (bp > 0 && self.planet_owner(bp) == en);
        let Route::Simulate { lopsided } = spacewar::route(&input) else {
            self.events.push(GameEvent::Battle(BattleEvent::SpaceRealTime { attacker: f1, defender: f2, planet: bp }));
            if g.canonical || self.sp_battles.live {
                self.space_real_time(&fleets, bp);
            }
            return;
        };
        let winner = self.simulate_space(f1, f2, bp);
        let won = at::BATTLES_WON + 4 * winner as u32;
        self.state.mem.set_i32(won, self.state.mem.i32(won).wrapping_add(1));
        self.events.push(GameEvent::Battle(BattleEvent::Space {
            attacker: f1,
            defender: f2,
            planet: bp,
            winner,
            lopsided_sp: lopsided && !g.multiplayer,
            human,
        }));
        if self.fleet_i32(f1, 0x54) > 0 && self.fleet_i32(f1, 0x5c) > 0 && planet > 0 {
            self.ground_war(f1, planet);
        }
    }

    /// `SimulateSpBattle(f1, f2, planet)` (exe 0x5a5e60, `space-combat.md` §5) on the state, with its story
    /// bookkeeping (`spwsflotta`, `Story_SpaceBattleFinished(list, 2, false)`). Returns `SpWarKiNyert`.
    fn simulate_space(&mut self, f1: usize, f2: usize, bp: usize) -> u8 {
        let g = self.globals();
        let o1 = self.fleet_owner(f1);
        let o2 = if f2 == 0 { self.planet_owner(bp) } else { self.fleet_owner(f2) };
        let id = |f: usize| self.state.fleets.get(f).map_or(0, |r| i16::from_le_bytes([r[0x16], r[0x17]]) as i32);
        let list = [
            Participant { owner: o1 as i32, slot_id: f1 as i32, fleet_id: id(f1) },
            Participant { owner: o2 as i32, slot_id: f2 as i32, fleet_id: if f2 == 0 { 0 } else { id(f2) } },
        ];
        let mut combat = self.state.combat();
        let map = self.state.starmap();
        combat.follow_targets = (1..map.fleets.len())
            .filter(|&x| map.fleets[x].alive() && map.fleets[x].order > 0 && map.fleets[x].kind == order::FLEET)
            .map(|x| (x, map.fleets[x].tx))
            .collect();
        let mut planet = (bp > 0).then(|| {
            let r = self.planet_rec(bp);
            let n = i32::from_le_bytes(r[0x114..0x118].try_into().unwrap()).clamp(0, 10) as usize;
            BattlePlanet {
                index: bp,
                owner: r[0x20],
                ships: (1..=n).map(|k| i16::from_le_bytes([r[0xfe + 2 * k], r[0xff + 2 * k]])).collect(),
                buildings: self.state.buildings.get(bp).map_or(Vec::new(), |b| {
                    b.iter().enumerate().skip(1).map(|(slot, x)| DefenceSlot { slot, kind: x[0], finished: u16::from_le_bytes([x[8], x[9]]) == 0 }).collect()
                }),
            }
        });
        let centres: Vec<(i32, i32)> =
            (0..EMPIRES as u32).map(|e| (self.state.mem.i32(va::AI_CENTRE_X + 4 * e), self.state.mem.i32(va::AI_CENTRE_Y + 4 * e))).collect();
        // `CTF_value` (exe 0xF261EC): the attack (`+0x60`) each side lost, from before the battle.
        let attack = [self.fleet_i32(f1, 0x60), if f2 == 0 { 0 } else { self.fleet_i32(f2, 0x60) }];
        let mut rng = self.state.rng();
        let out = combat.simulate(f1, f2, planet.as_mut(), &self.rules.guns, &centres, &mut rng);
        self.state.store_rng(&rng);
        let winner = if out.winner == 1 { o1 } else { o2 };
        self.state.mem.set_i32(at::SP_WINNER, winner as i32);
        let fleet_events = std::mem::take(&mut combat.events);
        self.state.store_combat(&combat);
        if planet.is_some() {
            // Destroyed defence ships: slot `k` (`+0xfe + 2k`) := 0, then the list closes up in place, the old
            // last entry staying behind the new end (exe 0x5a69e3..0x5a6a8f).
            let at = va::PLANETS + (bp * PLANET_SIZE) as u32;
            let mut slots: Vec<i16> = (0..=10).map(|k| self.state.mem.i16(at + 0xfe + 2 * k)).collect();
            for u in out.sides[1].iter().filter(|u| u.hp < 1 && u.kind == spacewar::UnitKind::PlanetShip) {
                if let Some(s) = slots.get_mut(u.id) {
                    *s = 0;
                }
            }
            let n = compact_planet_ships(&mut slots, self.state.mem.i32(at + 0x114));
            self.state.mem.set_i32(at + 0x114, n);
            for (k, d) in slots.iter().enumerate().skip(1) {
                self.state.mem.write(at + 0xfe + 2 * k as u32, &d.to_le_bytes());
            }
        }
        // exe 0x5a7430..0x5a7469: `[1]` and `[2]` (`[0]` is never written); a side without ships lost all of it.
        for (k, (f, a)) in [(f1, attack[0]), (f2, attack[1])].into_iter().enumerate() {
            let lost = if f2 == 0 && k == 1 { 0 } else if self.fleet_i32(f, 0x54) != 0 { a.wrapping_sub(self.fleet_i32(f, 0x60)) } else { a };
            self.state.mem.set_i32(va::CTF_VALUE + 4 * (k as u32 + 1), lost);
        }
        self.with_story(|st, w| st.space_battle_finished(&list, false, &story_chain(), w));
        self.combat_aftermath(fleet_events, g.sends);
        winner
    }

    /// The deletions' consequences outside the combat view: followers stop (`FlottaMozgat(g, 0, …)`),
    /// `OsszHanySajatFlotta[owner] −= 1` per `FlottaDelete` (twice on a re-entry, the second time for owner 0xFF,
    /// whose counter is `Tankok[96].count`), the human's emptied fleet goes over the network (message 0x1b; in
    /// canonical mode every human's, `Game::view`),
    /// destroyed planet guns are `EpuletDelete`d.
    pub(super) fn combat_aftermath(&mut self, events: Vec<CombatEvent>, sends: [bool; EMPIRES]) {
        for e in events {
            match e {
                CombatEvent::FollowerStopped { fleet } => {
                    let g = self.globals();
                    let ctx = self.starmap_context(&g);
                    let mut map = self.state.starmap();
                    let mut ev = Vec::new();
                    map.order(fleet, 0, 0, 0.0, 0.0, &ctx, &mut ev);
                    for x in &ev {
                        match *x {
                            StarmapEvent::ColonyShipFreed { empire } => map.colony_ship_pool[empire as usize % EMPIRES] += 1,
                            StarmapEvent::TerraformerFreed { empire } => map.terraformer_pool[empire as usize % EMPIRES] += 1,
                            _ => {}
                        }
                    }
                    self.state.store_starmap(&map);
                    self.events.extend(ev.into_iter().map(GameEvent::Starmap));
                }
                CombatEvent::FleetDeleted { owner, .. } => {
                    let at = va::FLEET_NUMBERS + 4 * owner as u32;
                    self.state.mem.set_i32(at, self.state.mem.i32(at).wrapping_sub(1));
                }
                CombatEvent::HumanFleetEmptied { fleet } => {
                    if sends[self.fleet_owner(fleet) as usize % EMPIRES] {
                        self.events.push(GameEvent::FleetEmptied { fleet });
                    }
                }
                CombatEvent::BuildingDestroyed { planet, slot } => {
                    self.with_war(planet, |w| w.delete_building(slot as i32));
                }
                CombatEvent::PlanetVehiclesLost { .. } => {}
            }
        }
    }

    /// `GroundWarIndit(f, p)` (exe 0x59c2d0, `ground-combat.md` §2, §6.3–§6.4).
    pub(super) fn ground_war(&mut self, f: usize, p: usize) {
        if self.fleet_i32(f, 0x5c) < 1 {
            return;
        }
        let g = self.globals();
        let r = self.planet_rec(p);
        if (0..5).any(|k| i16::from_le_bytes([r[0x16a + 2 * k], r[0x16b + 2 * k]]) == -0x21ac) {
            self.events.push(GameEvent::Battle(BattleEvent::GroundScripted { fleet: f, planet: p }));
            return;
        }
        let (fo, po) = (self.fleet_owner(f), r[0x20]);
        // `KemkedesFoldiharcLekezel` (exe 0x53ef00): the planet's owner sees the attacker's ground designs, the
        // attacker the garrison's and every building on the planet.
        if (po as usize) < 9 && (fo as usize) < 9 {
            let combat = self.state.combat();
            let mut tech = self.state.tech();
            let class2 = |e: u8, d: i16| combat.designs.get(e as usize).and_then(|t| t.get(d as usize)).is_some_and(|x| x.class == 2);
            for v in combat.vehicles.iter().skip(1).filter(|v| v.fleet as usize == f && v.design != spacewar::NO_DESIGN) {
                if class2(fo, v.design) {
                    Self::see_design(&mut tech, &combat.designs, po as usize, fo as usize, v.design);
                }
            }
            for v in combat.vehicles.iter().skip(1).filter(|v| v.fleet as i32 == -(p as i32) && v.design != spacewar::NO_DESIGN) {
                if class2(po, v.design) {
                    Self::see_design(&mut tech, &combat.designs, fo as usize, po as usize, v.design);
                }
            }
            let slots = self.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32).max(0) as usize;
            for b in self.state.buildings.get(p).map_or(&[][..], |b| &b[..]).iter().skip(1).take(slots) {
                if b[0] != 0 {
                    let t = self.rules.side.colony.buildings.get(b[0] as usize).map_or(0, |k| k.tech as usize);
                    tech.mark_seen(t, fo as usize, po as usize);
                }
            }
            self.state.store_tech(&tech);
        }
        let m = &self.state.mem;
        let mut input = ground::RouteInput {
            fleet_ground: self.fleet_i32(f, 0x5c),
            fleet_strength: self.fleet_i32(f, 0x68),
            garrison_strength: i32::from_le_bytes(r[0x5c..0x60].try_into().unwrap()),
            fortress: m.i32(va::FORTRESS + 4 * p as u32),
            attacker_human: g.human[fo as usize % EMPIRES],
            owner_human: g.human[po as usize % EMPIRES],
            multiplayer: g.multiplayer,
            mp_battle_status: m.i32(at::MP_BATTLE_STATUS),
            debug_no_lopsided: m.i32(at::DEBUG_GROUND_NO_LOPSIDED) != 0,
            debug_lopsided_off: m.i32(at::DEBUG_ALWAYS_SIMULATE) != 0,
            debug_always_simulate: m.i32(at::DEBUG_GROUND_SIMULATE) != 0,
            story_ban: false,
        };
        let mut ban = false;
        self.with_story(|st, w| ban = st.ground_battle_simulate_check(p as i32, w));
        input.story_ban = ban;
        let lopsided = ground::lopsided(&input);
        let simulate = matches!(ground::route(&input), GroundRoute::Simulate { .. });
        self.with_war(p, |w| w.land(f as i32, simulate));
        self.with_story(|st, w| st.ground_battle_started(f as i32, p as i32, po as i32, &story_chain(), w));
        if adat_i32(&self.adat(), ADAT_NO_INCIDENT) == 0 || g.multiplayer {
            self.with_diplomacy(|d, w| d.war(w, fo as usize, po as usize, false, 0));
        }
        self.set_adat_i32(ADAT_NO_INCIDENT, 0);
        if !simulate {
            self.events.push(GameEvent::Battle(BattleEvent::GroundRealTime { fleet: f, planet: p }));
            if g.canonical {
                self.ground_real_time(p);
            }
            self.ground_battle_status();
            return;
        }
        self.with_war(p, |w| w.simulate());
        let won = self.fleet_i32(f, 0x5c) > 0;
        self.with_story(|st, w| st.ground_battle_finished_save(f as i32, p as i32, po as i32, won, &*w));
        let story_planet = adat_i32(&self.adat(), ADAT_STORY_PLANET);
        self.events.push(GameEvent::Battle(BattleEvent::Ground { fleet: f, planet: p, attacker: fo, owner: po, lopsided, won, story_planet }));
        if won {
            self.capture(p, fo);
            let won = at::BATTLES_WON + 4 * fo as u32;
            self.state.mem.set_i32(won, self.state.mem.i32(won).wrapping_add(1));
        }
        self.ground_battle_status();
    }

    /// `BolygoElfoglal(p, owner)` (`colony-side.md` §4) with its follow-ups.
    pub(super) fn capture(&mut self, p: usize, owner: u8) {
        let g = self.globals();
        let ctx = self.economy_context(&g);
        let mut w = self.state.colony();
        let mut events = Vec::new();
        w.transfer(p, owner, &self.rules.side, &ctx, &mut events);
        self.state.store_colony(&w);
        for e in &events {
            if let SideEvent::RecallAttackers { planet } = *e {
                self.recall_attackers(planet);
            }
        }
        self.events.extend(events.into_iter().map(GameEvent::Side));
        self.battlefield_hand_offs();
    }

    /// The battlefield side of the unit hand-offs raised since the last call, in event order (the exe does it inside
    /// the calls that raise them; nothing in between reads a battlefield): garrison vehicles entering a planet's
    /// battlefield (`TankNew(−p, d, n)`: `AddOwnTankToPlanet` × n, then `RearrangeTankGroups(owner)`) or leaving it
    /// (`RemoveTankFromPlanet` × n; `TankAtpakol` rearranges after), and a captured planet's garrison and turrets
    /// (`BolygoElfoglal`).
    pub(super) fn battlefield_hand_offs(&mut self) {
        while self.hand_offs < self.events.len() {
            let e = self.events[self.hand_offs].clone();
            self.hand_offs += 1;
            let owner = |g: &Self, p: usize| g.planet_owner(p) as i32;
            match e {
                GameEvent::Production(ProductionEvent::GarrisonPlaced { planet, design, count })
                | GameEvent::Ai(ai::AiEvent::GarrisonMap { planet, design, count })
                    if count > 0 =>
                {
                    let o = owner(self, planet);
                    self.with_war(planet, |w| {
                        for _ in 0..count {
                            w.add_own_tank(design as i32);
                        }
                        w.rearrange(o);
                    });
                }
                GameEvent::Ai(ai::AiEvent::GarrisonMap { planet, design, count }) => {
                    let o = owner(self, planet);
                    self.with_war(planet, |w| {
                        for _ in 0..count.wrapping_neg() {
                            w.remove_tank(design as i32, o);
                        }
                    });
                }
                GameEvent::Ai(ai::AiEvent::GarrisonRearranged { planet }) => {
                    let o = owner(self, planet);
                    self.with_war(planet, |w| w.rearrange(o));
                }
                // `EpuletNew` → `AddBuildingTurrets(kind, 100, owner, slot, p)`; `EpuletFelepult` → `BuildingBuilt(slot)`
                // (ground-combat.md §11.3).
                GameEvent::Side(SideEvent::AutobuildPlaced { planet, kind, slot, .. }) => {
                    let o = owner(self, planet);
                    self.with_war(planet, |w| w.add_building_turrets(kind, o, slot as i32));
                }
                GameEvent::Side(SideEvent::Colony { planet, event: crate::colony::ColonyEvent::BuildingFinished { slot } }) => {
                    self.with_war(planet, |w| w.building_built(slot as i32 + 1));
                }
                GameEvent::Side(SideEvent::GarrisonLost { planet, old_owner, new_owner })
                | GameEvent::Spy(SpyEvent::Colony(SideEvent::GarrisonLost { planet, old_owner, new_owner })) => {
                    self.garrison_lost(planet, old_owner, new_owner);
                }
                _ => {}
            }
        }
    }

    /// `BolygoElfoglal(p, new)`'s garrison and battlefield part (exe 0x53b812..0x53b8ff), on the old owner's terms:
    /// every garrison entry (`Tankok` with fleet −p, in order) is `TankDelete`d and its vehicles leave the battlefield
    /// one by one (`RemoveTankFromPlanet(d, p, old)`); then the turrets change hands ([`ground::War::captured`]).
    fn garrison_lost(&mut self, p: usize, old: u8, new: u8) {
        let garrison: Vec<(i32, i32)> = self.state.combat().vehicles.iter().skip(1)
            .filter(|v| v.fleet as i32 == -(p as i32) && v.design != spacewar::NO_DESIGN)
            .map(|v| (v.design as i32, v.count))
            .collect();
        self.with_war(p, |w| {
            let now = w.planet.owner;
            w.planet.owner = old;
            for &(d, n) in &garrison {
                w.tank_delete(-(p as i32), d, n);
                for _ in 0..n {
                    w.remove_tank(d, old as i32);
                }
            }
            w.planet.owner = now;
            w.captured(new as i32);
        });
    }

    /// `GroundWarSetBattleStatus` (exe 0x47caa0, `ground-combat.md` §6.4): every planet's war flag `+0x64` from
    /// its field's status, the attackers' fleets marked in battle while it is contested.
    pub(super) fn ground_battle_status(&mut self) {
        let all: Vec<usize> = (1..=self.state.planet_count()).collect();
        self.with_wars(&all, |w| w.battle_status());
    }

    /// `GroundWarSetBattleStatus` at the head of `FRISSIT_Starmap` (exe 0x595d61: after the radar refresh and the
    /// real-time battles' marks, before the movement loop): every planet's war flag and status cache. Returns the
    /// attacking fleets it marks in battle, for the starmap to mark with the space battles' (the refresh clears the
    /// mark, so it must come after).
    pub(super) fn starmap_ground_marks(&mut self) -> Vec<usize> {
        self.battlefield_hand_offs();
        let mut combat = self.state.combat();
        let flags: Vec<u16> = combat.fleets.iter().map(|f| f.flags).collect();
        for f in combat.fleets.iter_mut() {
            f.flags &= !1;
        }
        self.state.store_combat(&combat);
        self.ground_battle_status();
        let mut combat = self.state.combat();
        let marked = (0..combat.fleets.len()).filter(|&f| combat.fleets[f].flags & 1 != 0).collect();
        for (f, x) in combat.fleets.iter_mut().zip(flags) {
            f.flags = x;
        }
        self.state.store_combat(&combat);
        marked
    }

    /// The ground war on planet `p` (its battlefield from the state), with the planet, the units, the streams and
    /// libc; everything goes back to the state afterwards.
    pub(super) fn with_war(&mut self, p: usize, f: impl FnOnce(&mut War)) {
        let mut f = Some(f);
        self.with_wars(&[p], |w| (f.take().unwrap())(w));
    }

    /// [`Game::with_war`] for several planets in turn, with one view of the units, streams and libc.
    pub(super) fn with_wars(&mut self, planets: &[usize], mut f: impl FnMut(&mut War)) {
        let g = self.globals();
        let adat = self.adat();
        let mut world = self.state.combat();
        let mut rng = self.state.rng();
        let mut libc = self.libc;
        let mut raised = Vec::new();
        for &p in planets {
            let mut field = self.state.take_ground_field(p);
            let r = self.planet_rec(p);
            // The used slots (`Epuletek[p][1..=slots]`, exe 0xBA9840): the rest of the block is unused (garbage).
            let slots = self.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32).max(0) as usize;
            let mut blocks = self.state.buildings.get(p).cloned().unwrap_or_default();
            blocks.truncate(slots + 1);
            let m = &self.state.mem;
            let mut planet = GroundPlanet {
                index: p as i32,
                owner: r[0x20],
                garrison: i32::from_le_bytes(r[0x58..0x5c].try_into().unwrap()),
                garrison_strength: i32::from_le_bytes(r[0x5c..0x60].try_into().unwrap()),
                in_war: r[0x64],
                buildings: blocks.iter().map(|b| building_from(b)).collect(),
                building_count: m.i32(va::BUILDINGS_OCCUPIED + 4 * p as u32),
                building_z: blocks.iter().map(|b| f32::from_le_bytes(b[0x14..0x18].try_into().unwrap())).collect(),
            };
            let ctx = GroundContext {
                human: g.en as i32,
                // The canonical view runs the wars by the original's MP rules (`multiplayer.md` §7).
                multiplayer: g.multiplayer || g.canonical,
                mp_war: m.i32(super::realtime::at::GROUND_RUNNING) != 0,
                story_planet: adat_i32(&adat, ADAT_STORY_PLANET),
                story_bonus: adat_i32(&adat, 0x70).wrapping_add(adat_i32(&adat, 0x74)),
                players: g.pp2 as i32,
                prev_planet_owner: m.u8(va::PLANETS + ((p - 1) * PLANET_SIZE) as u32 + 0x20),
                on_screen: false,
                player_ki: m.read(va::PLAYER_KI, EMPIRES).try_into().unwrap(),
            };
            let mut draw = || libc.rand();
            let mut place = |_: Option<(i32, i32)>| (true, (100, 100));
            let map = self.rules.ground_maps.get(r[0x23] as usize);
            // `SetTankPositionAndMatrix` reads the heights of the planet on the colony screen [original quirk].
            let shown = self.planet_rec(m.i32(va::AKT_PLANET).max(0) as usize)[0x23] as usize;
            let view = self.rules.ground_maps.get(shown);
            let muted = m.u8(super::realtime::at::VOICE_MUTE) != 0;
            let (events, muted_after) = {
                let mut war = War::new(&mut field, &mut planet, &mut world, &self.rules.ground, &ctx, &mut rng, &mut draw, &mut place);
                war.map = map;
                war.view_map = view;
                war.muted = muted;
                f(&mut war);
                (war.events, war.muted)
            };
            let at = va::PLANETS + (p * PLANET_SIZE) as u32;
            let m = &mut self.state.mem;
            if muted_after != muted {
                m.set_u8(super::realtime::at::VOICE_MUTE, muted_after as u8);
            }
            m.set_i32(at + 0x58, planet.garrison);
            m.set_i32(at + 0x5c, planet.garrison_strength);
            m.set_u8(at + 0x64, planet.in_war);
            m.set_i32(va::BUILDINGS_OCCUPIED + 4 * p as u32, planet.building_count);
            if let Some(blocks) = self.state.buildings.get_mut(p) {
                for (rec, b) in blocks.iter_mut().zip(&planet.buildings) {
                    rec[0] = b.kind;
                    rec[4..8].copy_from_slice(&b.damage.to_le_bytes());
                }
            }
            for slot in std::mem::take(&mut field.tail.created) {
                let raw = match self.uninit.as_mut() {
                    Some(f) => f(Fresh::GroundUnit { planet: p, slot }),
                    None => Vec::new(),
                };
                if let Some(u) = field.units.get_mut(slot) {
                    u.fill_uninit(&raw);
                }
            }
            // A grown group's member arrays may have moved: only the saved origin pointer is taken.
            let mut grown = std::mem::take(&mut field.tail.grown);
            grown.sort_unstable();
            grown.dedup();
            for slot in grown {
                let raw = match self.uninit.as_mut() {
                    Some(f) => f(Fresh::GroundUnit { planet: p, slot }),
                    None => Vec::new(),
                };
                let ptr: Vec<u8> = raw.chunks_exact(8).filter(|c| c[..4] == 0x19cu32.to_le_bytes()).flatten().copied().collect();
                if let Some(u) = field.units.get_mut(slot) {
                    u.fill_uninit(&ptr);
                }
            }
            self.state.store_ground_field(p, field);
            raised.extend(events.into_iter().map(|e| (p, e)));
        }
        self.libc = libc;
        self.state.store_rng(&rng);
        self.state.store_combat(&world);
        for (p, e) in raised {
            // `GroundWarFinished(p, s, false)` from the real-time war (canonical MP only: the classic game runs it
            // simulated): `BolygoElfoglal(p, s)`, `BattlesWon[s] += 1`, before the abandonment test.
            if let GroundEvent::Captured { owner } = e {
                if g.canonical {
                    self.capture(p, owner);
                    let won = at::BATTLES_WON + 4 * owner as u32;
                    self.state.mem.set_i32(won, self.state.mem.i32(won).wrapping_add(1));
                }
            }
            if e == GroundEvent::Abandoned {
                self.capture(p, 0);
            }
            // `GroundWarFinished`'s step 4: `TutorialCheckEvent(0x21)` when EN won.
            if e == (GroundEvent::Finished { winner: g.en as i32 }) {
                self.tutorial_check_event(0x21);
            }
            self.events.push(GameEvent::Battle(BattleEvent::GroundSide { planet: p, event: e }));
        }
    }

    /// `FRISSIT_Epuletek` (exe 0x4746f0, every tick before `FRISSIT_Starmap`): on every owned planet not at war
    /// (`+0x64`), each damaged building is repaired by `min(damage, BolygoMennyiTuzolto[p]) × IdoSpeedSzorzo`
    /// (`EpuletDamage(p, slot, −x, false, 10000)`: its turrets heal too); a planet that repaired something, or one
    /// at war, gets `FRISSIT_Planet(false, p)` (`colony::refresh`).
    pub(super) fn buildings_tick(&mut self) {
        let speed = self.state.mem.i32(va::IDO_SPEED);
        for p in 1..=self.state.planet_count() {
            if self.planet_owner(p) == 0 {
                continue;
            }
            let at_war = self.planet_rec(p)[0x64] != 0;
            let mut refresh = at_war;
            if !at_war {
                let slots = self.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32).max(0) as usize;
                let brigade = self.state.mem.i32(va::FIRE_BRIGADE + 4 * p as u32);
                let repairs: Vec<(usize, i32)> = self.state.buildings.get(p).map_or(Vec::new(), |b| {
                    b.iter().enumerate().skip(1).take(slots).filter_map(|(slot, r)| {
                        let damage = i32::from_le_bytes(r[4..8].try_into().unwrap());
                        (r[0] != 0 && damage > 0).then_some((slot, damage))
                    }).collect()
                });
                for &(slot, damage) in &repairs {
                    let x = damage.min(brigade).wrapping_mul(speed);
                    self.with_war(p, |w| w.building_damage(slot as i32, x.wrapping_neg(), false, 10000));
                }
                refresh = !repairs.is_empty();
            }
            if refresh {
                let g = self.globals();
                let ctx = self.economy_context(&g);
                let mut w = self.state.colony();
                w.occupied[p] = crate::colony::refresh(&mut w.planets[p], &self.rules.side.colony, &ctx);
                self.state.store_colony(&w);
            }
        }
    }

    /// `Story_SpaceBattleFinishedCheck`, `Story_FleetArrivedCheck`, `Story_GroundBattleFinishedCheck` (exe
    /// 0x5c9200, 0x5c8c90, 0x5c8d80): the end of an arrival's step.
    pub(super) fn arrival_checks(&mut self) {
        self.with_story(|st, w| st.space_battle_finished_check(&story_chain(), w));
        self.with_story(|st, w| st.fleet_arrived_check(&story_chain(), w));
        let p = self.state.story().ground.planet.max(0) as usize;
        let buildings = self.state.mem.i32(va::BUILDINGS_OCCUPIED + 4 * p as u32);
        let owner = self.planet_owner(p) as i32;
        self.with_story(|st, w| st.ground_battle_finished_check(buildings, owner, &story_chain(), w));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destroyed_defence_ships_close_up_in_place() {
        // Slot 2 of [46, 41] destroyed: the count drops, slot 2 stays zero (the replay's planet 43).
        let mut s = vec![0, 46, 0];
        assert_eq!(compact_planet_ships(&mut s, 2), 1);
        assert_eq!(s, [0, 46, 0]);
        // Slot 1 of three: the rest moves down, the old last entry stays behind the end.
        let mut s = vec![0, 0, 7, 8];
        assert_eq!(compact_planet_ships(&mut s, 3), 2);
        assert_eq!(s, [0, 7, 8, 8]);
        // Two in a row are both caught (the index doesn't move after a shift).
        let mut s = vec![0, 0, 0, 9];
        assert_eq!(compact_planet_ships(&mut s, 3), 1);
        assert_eq!(s, [0, 9, 9, 9]);
    }

    #[test]
    fn battle_messages_follow_the_exe() {
        let space = |winner, lopsided_sp, human| BattleEvent::Space { attacker: 1, defender: 2, planet: 0, winner, lopsided_sp, human };
        assert_eq!(space(6, true, true).messages(6), [(0x0f, None, false)]);
        assert_eq!(space(3, true, true).messages(6), [(0x10, None, false)]);
        assert_eq!(space(6, false, true).messages(6), [(0x11, None, false)]);
        assert_eq!(space(3, false, true).messages(6), [(0x12, None, false)]);
        assert!(space(3, false, false).messages(6).is_empty());
        let ground = |attacker, owner, lopsided, won, story_planet| BattleEvent::Ground { fleet: 1, planet: 5, attacker, owner, lopsided, won, story_planet };
        assert_eq!(ground(6, 3, true, true, 0).messages(6), [(0x16, Some(5), true)]);
        assert_eq!(ground(3, 6, true, true, 0).messages(6), [(0x17, Some(5), true)]);
        assert_eq!(ground(3, 6, true, false, 0).messages(6), [(0x18, Some(5), true)]);
        assert_eq!(ground(6, 3, true, false, 0).messages(6), [(0x19, Some(5), true)]);
        assert_eq!(ground(3, 6, false, true, 0).messages(6), [(0x0b, Some(5), true)]);
        assert_eq!(ground(6, 3, false, true, 0).messages(6), [(0x0c, Some(5), true)]);
        // No 0x0C for the story planet, nothing for a lost even war.
        assert!(ground(6, 3, false, true, 5).messages(6).is_empty());
        assert!(ground(6, 3, false, false, 0).messages(6).is_empty());
        // A real-time war's end: the old owner's 0x0B, the winner's 0x0C (not for the story planet).
        let taken = |from, to, story_planet| BattleEvent::GroundSide { planet: 5, event: GroundEvent::Taken { from, to, story_planet } };
        assert_eq!(taken(6, 3, 0).messages(6), [(0x0b, Some(5), true)]);
        assert_eq!(taken(3, 6, 0).messages(6), [(0x0c, Some(5), true)]);
        assert!(taken(3, 6, 5).messages(6).is_empty());
        assert!(taken(3, 4, 0).messages(6).is_empty());
    }
}
