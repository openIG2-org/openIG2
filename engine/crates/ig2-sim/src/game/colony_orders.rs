//! The colony screen's lockstep messages (`MULTIGET_Colony` exe 0x48a270, ELF 0x4ec2e0; re/screens/colony.md §9):
//! the building orders the colony screen sends with `Multi_Send(type, EN, …)`, the shown-planet notice and the
//! self-destruct switch. The screen's own bookkeeping in the handler (`NemSzabadEpiteni` 0x996474, the placing
//! cursor `ColonyRemoveBuilding`/`ColonyBuildingPlace` 0x99643b/0x99643a, `AktEpulet` 0xB9C074 on the issuer's
//! screen, the speaker's lines) is the client's: it gets [`ColonyOrderEvent`]s.
//!
//! Not here: the ground war's unit orders 0x22 move, 0x23 waypoint, 0x24 target, 0x25 stop, 0x12d detonate, 0x12f
//! landing zone, 0x130 air strike, 0x134 a unit's hold flag (another task).

use super::*;

/// One colony message's payload (the handler's `Multi_Get` values after the issuer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColonyOrder {
    /// 0x1f: build `kind` on `planet` from grid cell `(x, y)` (`EpuletNewSearchLocation(p, kind, false, 0, x, y)`:
    /// the nearest free spot), paying `GetGyartasAr(e, EpuletekLeiro[kind].tech)` first (unless the research
    /// cheat 0xB7727C is on); refused without the money.
    Build { planet: i32, kind: i32, x: i32, y: i32 },
    /// 0x20: demolish building `slot`, refunding half its price (`GetGyartasAr / 2`, int).
    Demolish { planet: i32, slot: i32 },
    /// 0x21: switch building `slot` on or off (`+0x19` bit 1).
    Switch { planet: i32, slot: i32 },
    /// 0x26: the issuer's colony screen shows `planet` (`KiMelyikBolygonVan`, exe 0x9ABAE4 + 4e, unsaved).
    Watch { planet: i32 },
    /// 0x27: `planet`'s self-destruct starts (`WF+0x334 = 399`, ground-combat.md §11).
    SelfDestruct { planet: i32 },
    /// 300: it is called off (`WF+0x334 = −1`).
    SelfDestructOff { planet: i32 },
    /// 0x132: building `slot`'s priority flag (`+0x19` bit 5, the panel's button 29).
    Priority { planet: i32, slot: i32 },
    /// 0x133: repair building `slot` at once for `EpuletJavitasAr`; refused without the money.
    Repair { planet: i32, slot: i32 },
}

impl ColonyOrder {
    /// The order a `MULTIGET_Colony` message carries: its type and the values after the issuer, in `Multi_Get`
    /// order.
    pub fn from_message(kind: i32, v: &[i32]) -> Option<Self> {
        let at = |i: usize| v.get(i).copied();
        Some(match kind {
            0x1f => Self::Build { planet: at(0)?, kind: at(1)?, x: at(2)?, y: at(3)? },
            0x20 => Self::Demolish { planet: at(0)?, slot: at(1)? },
            0x21 => Self::Switch { planet: at(0)?, slot: at(1)? },
            0x26 => Self::Watch { planet: at(0)? },
            0x27 => Self::SelfDestruct { planet: at(0)? },
            300 => Self::SelfDestructOff { planet: at(0)? },
            0x132 => Self::Priority { planet: at(0)?, slot: at(1)? },
            0x133 => Self::Repair { planet: at(0)?, slot: at(1)? },
            _ => return None,
        })
    }

    /// The message type and values ([`ColonyOrder::from_message`]'s inverse).
    pub fn to_message(self) -> (i32, Vec<i32>) {
        match self {
            Self::Build { planet, kind, x, y } => (0x1f, vec![planet, kind, x, y]),
            Self::Demolish { planet, slot } => (0x20, vec![planet, slot]),
            Self::Switch { planet, slot } => (0x21, vec![planet, slot]),
            Self::Watch { planet } => (0x26, vec![planet]),
            Self::SelfDestruct { planet } => (0x27, vec![planet]),
            Self::SelfDestructOff { planet } => (300, vec![planet]),
            Self::Priority { planet, slot } => (0x132, vec![planet, slot]),
            Self::Repair { planet, slot } => (0x133, vec![planet, slot]),
        }
    }
}

/// What a colony order did that the issuer's screen reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColonyOrderEvent {
    /// 0x1f done: the building's slot (0: no room, the money is gone anyway [original behaviour]). The issuer's
    /// colony screen on that planet selects it (`AktEpulet`).
    Built { empire: u8, planet: usize, slot: usize },
    /// 0x1f refused: not enough money (`ScrollHibaUzenet(GetString(3, 2), tech name)`; the screen drops the
    /// building it was placing).
    BuildRefused { empire: u8, planet: usize, kind: u8 },
    /// 0x133 refused: not enough money (the same message).
    RepairRefused { empire: u8, planet: usize, slot: usize },
    /// 0x20: the building went (`EpuletDelete`; the screen on that planet drops its selection).
    Demolished { empire: u8, planet: usize, slot: usize },
}

/// Exe globals the orders read or write.
mod at {
    /// `KiMelyikBolygonVan` − 4 (`(&0x9ABAE4)[e]`: empire 0 writes `Colony_elozo_foldrenges` [original quirk]).
    pub const WATCHING: u32 = 0x9ABAE4;
}

/// `EpuletJavitasAr(p, k)` (ELF 0x4bb5c0, inlined in the exe at 0x48ac22): `(int)((f32)price · (f32)damage /
/// (f32)max_hp)` with the tech's own price (`Talalmanyok[t]+0x14`, no race discount); f32 throughout.
pub fn repair_price(price: i32, damage: i32, max_hp: i32) -> i32 {
    (price as f32 * damage as f32 / max_hp as f32) as i32
}

impl Game<'_> {
    /// `MULTIGET_Colony`'s handling of one message from empire `issuer`. A planet or building slot the original
    /// would read out of its arrays' bounds is refused.
    pub fn colony_order(&mut self, issuer: u8, o: ColonyOrder) {
        let e = issuer as usize % EMPIRES;
        let planets = self.state.planet_count();
        let planet = |p: i32| usize::try_from(p).ok().filter(|&p| (1..=planets).contains(&p));
        let slots = |g: &Self, p: usize| g.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32).max(0) as usize;
        let slot = |g: &Self, p: usize, k: i32| usize::try_from(k).ok().filter(|&k| k >= 1 && k <= slots(g, p));
        match o {
            ColonyOrder::Build { planet: p, kind, x, y } => {
                let (Some(p), Ok(kind)) = (planet(p), u8::try_from(kind)) else { return };
                if kind as usize >= self.rules.side.colony.buildings.len() {
                    return;
                }
                self.build(issuer, p, kind, (x, y));
            }
            ColonyOrder::Demolish { planet: p, slot: k } => {
                let Some(p) = planet(p) else { return };
                let Some(k) = slot(self, p, k) else { return };
                let mut c = self.state.colony();
                let kind = c.planets[p].buildings[k - 1].kind;
                let tech = self.rules.side.colony.buildings[kind as usize].tech as usize;
                let race = self.globals().race[e];
                let x = crate::colony_side::price(&c.tech, race, tech) / 2;
                let income = self.ai_income(e);
                c.money[e].add(0, x as f64, income);
                self.state.store_colony(&c);
                self.with_war(p, |w| w.delete_building(k as i32));
                self.events.push(GameEvent::ColonyOrder(ColonyOrderEvent::Demolished { empire: issuer, planet: p, slot: k }));
            }
            ColonyOrder::Switch { planet: p, slot: k } | ColonyOrder::Priority { planet: p, slot: k } => {
                let Some(p) = planet(p) else { return };
                let Some(k) = slot(self, p, k) else { return };
                let bit = if matches!(o, ColonyOrder::Switch { .. }) { 2 } else { 0x20 };
                let mut c = self.state.colony();
                c.planets[p].buildings[k - 1].flags ^= bit;
                self.state.store_colony(&c);
            }
            ColonyOrder::Watch { planet: p } => self.state.mem.set_i32(at::WATCHING + 4 * issuer as u32, p),
            ColonyOrder::SelfDestruct { planet: p } | ColonyOrder::SelfDestructOff { planet: p } => {
                let Some(p) = planet(p) else { return };
                let v = if matches!(o, ColonyOrder::SelfDestruct { .. }) { 399 } else { -1 };
                self.with_war(p, |w| w.field.self_destruct = v);
            }
            ColonyOrder::Repair { planet: p, slot: k } => {
                let Some(p) = planet(p) else { return };
                let Some(k) = slot(self, p, k) else { return };
                let mut c = self.state.colony();
                let b = c.planets[p].buildings[k - 1];
                if b.kind == 0 {
                    return;
                }
                let desc = self.rules.side.colony.buildings[b.kind as usize];
                let cost = repair_price(c.tech.price(desc.tech as usize), b.damage, desc.max_hp);
                if c.money[e].available(0) < cost as f64 {
                    self.events.push(GameEvent::ColonyOrder(ColonyOrderEvent::RepairRefused { empire: issuer, planet: p, slot: k }));
                    return;
                }
                let income = self.ai_income(e);
                c.money[e].add(0, -cost as f64, income);
                self.state.store_colony(&c);
                // Six `EpuletDamage(p, k, −damage, false, 10000)`: the first repairs it all, the rest pass 0 to the
                // turrets.
                self.with_war(p, |w| {
                    for _ in 0..6 {
                        let d = w.planet.buildings.get(k).map_or(0, |b| b.damage);
                        w.building_damage(k as i32, d.wrapping_neg(), false, 10000);
                    }
                });
            }
        }
    }

    /// 0x1f (exe 0x48a306).
    fn build(&mut self, issuer: u8, p: usize, kind: u8, at: (i32, i32)) {
        let e = issuer as usize % EMPIRES;
        let g = self.globals();
        let mut c = self.state.colony();
        let tech = self.rules.side.colony.buildings[kind as usize].tech as usize;
        let price = crate::colony_side::price(&c.tech, g.race[e], tech);
        if self.state.mem.i32(va::RESEARCH_CHEAT) == 0 {
            if c.money[e].available(0) < price as f64 {
                self.events.push(GameEvent::ColonyOrder(ColonyOrderEvent::BuildRefused { empire: issuer, planet: p, kind }));
                return;
            }
            let income = self.ai_income(e);
            c.money[e].add(0, -price as f64, income);
        }
        let cells = self.state.vehicle_cells(p);
        let mut slot = 0;
        if let Some((x, y)) = self.rules.placement.locate(&c.planets[p], kind, &cells, Some(at)) {
            let econ = self.economy_context(&g);
            slot = c.new_building(p, kind, false, (x as f32, y as f32), 0, &self.rules.side, &econ);
        }
        self.state.store_colony(&c);
        // A grown block's records hold the heap's bytes (`OldReallocate` by 20).
        self.fill_fresh_buildings();
        if slot > 0 {
            let owner = self.planet_owner(p) as i32;
            self.with_war(p, |w| w.add_building_turrets(kind, owner, slot as i32));
        }
        self.events.push(GameEvent::ColonyOrder(ColonyOrderEvent::Built { empire: issuer, planet: p, slot }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip() {
        for o in [
            ColonyOrder::Build { planet: 42, kind: 7, x: 100, y: -1 },
            ColonyOrder::Demolish { planet: 1, slot: 3 },
            ColonyOrder::SelfDestructOff { planet: 9 },
            ColonyOrder::Repair { planet: 2, slot: 1 },
        ] {
            let (t, v) = o.to_message();
            assert_eq!(ColonyOrder::from_message(t, &v), Some(o));
        }
        assert_eq!(ColonyOrder::from_message(0x22, &[1, 2, 3, 4]), None);
    }

    #[test]
    fn repair_price_is_f32() {
        // 3000 · 77 / 450 = 513.33 → 513.
        assert_eq!(repair_price(3000, 77, 450), 513);
        assert_eq!(repair_price(1000, 0, 450), 0);
    }
}
