//! What the story scripts do to the rest of the game (`StoryEvent`, `re/rules/new-game.md` §15), carried out the
//! moment a script asks, as the exe calls the system there: later reads of the same script see it.

use super::story_world::GameStory;
use super::{Game, GameEvent, TradeView, ViewMode};
use crate::colony_side::{status, EMPIRES};
use crate::research::{self, ResearchEvent};
use crate::messages::cstr;
use crate::save::{va, BUILDING_SIZE, PLANET_SIZE};
use crate::story::{StoryEvent, StoryWorld, DESIGN};

/// Story 23's fugitive is renamed `_s(5, 232)` when it is caught (exe 0x5d4000).
const FUGITIVE_NAME: i32 = 232;

fn rd_i16(b: &[u8], o: usize) -> i32 {
    i16::from_le_bytes([b[o], b[o + 1]]) as i32
}
fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
/// `strcpy` into a record field of `dst.len()` bytes (the terminator included; the bytes after it keep theirs).
fn strcpy(dst: &mut [u8], src: &[u8]) {
    let src = cstr(src);
    let n = src.len().min(dst.len().saturating_sub(1));
    dst[..n].copy_from_slice(&src[..n]);
    if let Some(t) = dst.get_mut(n) {
        *t = 0;
    }
}

impl GameStory<'_, '_> {
    /// Carry out what a script asked of the rest of the game, at once (the exe calls the system there).
    pub(super) fn carry_out(&mut self, ev: &StoryEvent) {
        match *ev {
            StoryEvent::StopFleet(f) => self.with_game(|g| g.stop_fleet(f)),
            // Fleets, ships and units (`FlottaMozgat`, `FlottaDelete`, `HajoNew`, `HajoDelete`, `TankNew`, `TankDelete`,
            // `FlottaOsszevon`, `FlottakUjraSzamol`): the ports the AI calls.
            StoryEvent::MoveFleet { fleet, kind, mode, x, y } if fleet > 0 => {
                self.units(|w| w.order(fleet as usize, kind as i16, mode as i16, x, y))
            }
            StoryEvent::DeleteFleet(f) if f > 0 => self.units(|w| w.delete_fleet(f as usize)),
            StoryEvent::NewShip { fleet, design } => self.with_game(|g| g.new_ship(fleet, design)),
            StoryEvent::NewTanks { fleet, design, count } => self.units(|w| w.new_vehicles(fleet, design as i16, count)),
            StoryEvent::DeleteShip(k) if k > 0 => self.units(|w| {
                if (k as usize) < w.spy.combat.ships.len() {
                    w.delete_ship(k as usize)
                }
            }),
            StoryEvent::DeleteTanks { place, design, count } => self.units(|w| w.delete_vehicles(place, design as i16, count)),
            StoryEvent::MergeFleets { into, from } if into > 0 && from > 0 => self.units(|w| w.merge(into as usize, from as usize)),
            StoryEvent::RecalcFleets => self.units(|w| w.recount()),
            StoryEvent::UpgradeShip { ship, design } => {
                if let Some(s) = usize::try_from(ship).ok().and_then(|k| self.game.state.ships.get_mut(k)) {
                    s[2..4].copy_from_slice(&(design as i16).to_le_bytes());
                }
            }
            StoryEvent::SetTankDesign { tank, design } if (0..=self.tank_count()).contains(&tank) => {
                self.game.state.mem.write(va::VEHICLES + 8 * tank as u32 + 2, &(design as i16).to_le_bytes());
            }
            // Fleet record fields the scripts write directly.
            StoryEvent::SetFleetSpeed { fleet, speed } => self.fleet_write(fleet, 0x3c, &speed.to_le_bytes()),
            StoryEvent::SetFleetId { fleet, id } => self.fleet_write(fleet, 0x16, &(id as i16).to_le_bytes()),
            StoryEvent::SetFleetPos { fleet, x, y } => {
                self.fleet_write(fleet, 0x30, &x.to_le_bytes());
                self.fleet_write(fleet, 0x34, &y.to_le_bytes());
            }
            StoryEvent::SetFleetOwner { fleet, owner } => self.fleet_write(fleet, 0x15, &[owner as u8]),
            StoryEvent::SetFleetVisible { fleet, on } => {
                self.fleet_flags_write(fleet, if on { crate::fleet::flag::ALWAYS_VISIBLE } else { 0 }, if on { 0 } else { crate::fleet::flag::ALWAYS_VISIBLE })
            }
            StoryEvent::FleetFlags { fleet, set, clear } => self.fleet_flags_write(fleet, set, clear),
            StoryEvent::FleetState { fleet, set, clear } => {
                if let Some(r) = self.fleet_mut(fleet) {
                    r[0x18] = (r[0x18] | set) & !clear;
                }
            }
            StoryEvent::SendAway(f) => {
                if let Some(r) = self.fleet_mut(f) {
                    r[0x18] |= 2;
                }
            }
            StoryEvent::NameFleet { fleet, text } => {
                let name = match text {
                    Some(t) => self.story_text(t),
                    None => self.game.fleet_name(),
                };
                self.fleet_write_name(fleet, &name);
            }
            StoryEvent::RenameFleet(f) => {
                let name = self.story_text(FUGITIVE_NAME);
                self.fleet_write_name(f, &name);
            }
            // Designs (`Tervek[e][d]`).
            StoryEvent::DeleteDesign { empire, design, fleet, mark } if empire >= 0 => {
                self.units(|w| w.delete_design(empire as usize, design, fleet, mark))
            }
            StoryEvent::RecalcDesign { empire, design } if empire >= 0 && design >= 0 => {
                self.with_game(|g| crate::trade::Market::recompute_design(&mut TradeView { game: g }, empire as usize, design as usize, true))
            }
            StoryEvent::DesignCostTimes5 { empire, design } => self.design_edit(empire, design, |r| {
                let v = rd_i32(r, 0x20).wrapping_mul(5);
                r[0x20..0x24].copy_from_slice(&v.to_le_bytes());
            }),
            StoryEvent::DesignPriceDiv { empire, design, div } if div != 0 => self.design_edit(empire, design, |r| {
                let v = rd_i32(r, 0x24) / div;
                r[0x24..0x28].copy_from_slice(&v.to_le_bytes());
            }),
            StoryEvent::DesignForSale { empire, design } => {
                let date = self.date();
                self.design_edit(empire, design, |r| {
                    r[0x6c..0x70].copy_from_slice(&3i32.to_le_bytes());
                    for (k, v) in date.iter().enumerate() {
                        r[0x74 + 4 * k..0x78 + 4 * k].copy_from_slice(&v.to_le_bytes());
                    }
                    let price = rd_i32(r, 0x24) / 3;
                    r[0x24..0x28].copy_from_slice(&price.to_le_bytes());
                });
                if let Some(r) = self.design(empire, design).copied() {
                    let mut t = self.game.state.trade();
                    t.offers.push(r);
                    self.game.state.store_trade(&t);
                }
            }
            StoryEvent::CopyDesign { empire, from, to } => {
                if let Some(r) = self.design(empire, from).copied() {
                    self.design_edit(empire, to, |x| *x = r);
                }
            }
            StoryEvent::SetDesignKey { empire, design, key } => self.design_edit(empire, design, |r| r[0x80..0x84].copy_from_slice(&key.to_le_bytes())),
            StoryEvent::DesignFlags { empire, design, b18, b19 } => self.design_edit(empire, design, |r| {
                r[0x18] = b18 as u8;
                r[0x19] = b19 as u8;
            }),
            StoryEvent::DesignByte { empire, design, offset, value } if (0..DESIGN as i32).contains(&offset) => {
                self.design_edit(empire, design, |r| r[offset as usize] = value as u8)
            }
            StoryEvent::NameDesign { empire, design, text } => {
                let name = match text {
                    Some(t) => self.story_text(t),
                    None => {
                        let tech = self.design_i32(empire, design, 0x48);
                        cstr(&self.mem().read(Self::tech_at(tech), 0x20)).to_vec()
                    }
                };
                self.design_edit(empire, design, |r| strcpy(&mut r[..0x16], &name));
            }
            StoryEvent::ColonyDiscount { halve } => self.colony_discount(halve),
            StoryEvent::Undo { kind: 11, .. } => self.colony_discount(false),
            StoryEvent::ResetDesigns => self.with_game(|g| g.reset_designs()),
            StoryEvent::Wormhole { x1, y1, x2, y2 } => self.made = self.with_game(|g| g.add_wormhole(x1, y1, x2, y2)),
            StoryEvent::RevealWormhole if self.made != 0 => {
                let w = self.made;
                self.with_game(|g| g.reveal_wormhole(w));
            }
            _ => self.carry_out_world(ev),
        }
    }

    /// The planets, buildings, techs, money, diplomacy and spies (`re/rules/new-game.md` §15).
    fn carry_out_world(&mut self, ev: &StoryEvent) {
        let en = self.g.en as i32;
        let view = |e: i32| 0xd8 + 2 * e as usize;
        match *ev {
            // Planet record fields the scripts write directly.
            StoryEvent::SetPlanetValue { planet, value } => self.planet_edit(planet, |r| r[0x28..0x2c].copy_from_slice(&value.to_le_bytes())),
            StoryEvent::SetPlanetFriendly { planet, friendly } => self.planet_edit(planet, |r| r[0xbc] = friendly as u8),
            StoryEvent::SetPlanetRichness { planet, value } => self.planet_edit(planet, |r| r[0x24..0x28].copy_from_slice(&value.to_le_bytes())),
            StoryEvent::SetPlanetXY { planet, x, y } => self.planet_edit(planet, |r| {
                r[0x18..0x1c].copy_from_slice(&x.to_le_bytes());
                r[0x1c..0x20].copy_from_slice(&y.to_le_bytes());
            }),
            StoryEvent::SetPlanetRadar { planet, level } => self.planet_edit(planet, |r| r[0x6e] = level as u8),
            StoryEvent::RadarNotice(p) => self.planet_edit(p, |r| r[0xbb] = 1),
            StoryEvent::PlanetRadarMsg { planet, on } => self.planet_edit(planet, |r| r[0xbb] = on as u8),
            StoryEvent::SetEpidemic { planet, value } => self.planet_edit(planet, |r| r[0xb8..0xba].copy_from_slice(&(value as i16).to_le_bytes())),
            StoryEvent::PlanetExtra { planet, on } => self.planet_edit(planet, |r| {
                r[0x186] = on as u8;
                r[0x187] = on as u8;
            }),
            StoryEvent::PlanetExtraFlag { planet, on } => self.planet_edit(planet, |r| r[0x187] = on as u8),
            StoryEvent::PlanetMorale { planet, delta } => self.planet_edit(planet, |r| {
                let v = rd_i32(r, 0x174).wrapping_add(delta);
                r[0x174..0x178].copy_from_slice(&v.to_le_bytes());
                r[0x178..0x17c].copy_from_slice(&6i32.to_le_bytes());
            }),
            StoryEvent::GlobalMorale(delta) => {
                let m = &mut self.game.state.mem;
                m.set_i32(va::GLOBAL_MORALE, m.i32(va::GLOBAL_MORALE).wrapping_add(delta));
                m.set_i32(va::GLOBAL_MORALE_SPEED, 6);
            }
            StoryEvent::SetPlanetView { planet, view: v } => {
                let o = view(en);
                self.planet_edit(planet, |r| r[o..o + 2].copy_from_slice(&(v as i16).to_le_bytes()))
            }
            StoryEvent::SetPlanetViewOf { planet, empire, view: v } if (0..19).contains(&empire) => {
                let o = view(empire);
                self.planet_edit(planet, |r| r[o..o + 2].copy_from_slice(&(v as i16).to_le_bytes()))
            }
            StoryEvent::ShowPlanet(p) => {
                let o = view(en);
                self.planet_edit(p, |r| {
                    if rd_i16(r, o) < 1 {
                        r[o..o + 2].copy_from_slice(&1i16.to_le_bytes());
                    }
                })
            }
            StoryEvent::UnhidePlanet(p) => self.planet_edit(p, |r| {
                for o in (0xda..0xda + 2 * 18).step_by(2) {
                    if rd_i16(r, o) < 0 {
                        r[o..o + 2].fill(0);
                    }
                }
            }),
            StoryEvent::RenamePlanet { planet, name } => self.planet_edit(planet, |r| strcpy(&mut r[..0x18], name.as_bytes())),
            StoryEvent::NamePlanet { planet, text } => {
                let name = self.story_text(text);
                self.planet_edit(planet, |r| strcpy(&mut r[..0x18], &name));
            }
            // Buildings.
            StoryEvent::DamageBuilding { planet, slot, damage } => self.building_edit(planet, slot, |b| b[4..8].copy_from_slice(&damage.to_le_bytes())),
            StoryEvent::BuildingByte { planet, slot, offset, value } if (0..0x20).contains(&offset) => {
                self.building_edit(planet, slot, |b| b[offset as usize] = value as u8)
            }
            // Techs (`Talalmanyok[t]`).
            StoryEvent::TechCheaper(t) => {
                let at = Self::tech_at(t);
                for o in [0x38, 0x58 + 4 * en as u32, 0x34] {
                    let m = &mut self.game.state.mem;
                    m.set_i32(at + o, m.i32(at + o) / 3);
                }
            }
            StoryEvent::TechTimes { tech, empire, halve, price } => {
                let at = Self::tech_at(tech);
                let mut fields = vec![0x38, 0x58 + 4 * empire.max(0) as u32];
                if price {
                    fields.push(0x34);
                }
                for o in fields {
                    let m = &mut self.game.state.mem;
                    let v = m.i32(at + o);
                    m.set_i32(at + o, if halve { v / 2 } else { v << 1 });
                }
            }
            StoryEvent::TechSeen { tech, owner } if (0..9).contains(&owner) => {
                self.game.state.mem.set_u8(Self::tech_at(tech) + 0xc0 + 9 * en as u32 + owner as u32, 1)
            }
            StoryEvent::TechPrereq { tech, slot, value } if (1..=3).contains(&slot) => {
                self.game.state.mem.set_u8(Self::tech_at(tech) + 0x2c + slot as u32, value as u8)
            }
            StoryEvent::TechResearchTime { tech, value } => self.game.state.mem.set_i32(Self::tech_at(tech) + 0x58 + 4 * en as u32, value),
            // Diplomacy (`Viszony[a][b]`, 0x26 bytes).
            StoryEvent::RelationChange { a, b, delta } if (0..19).contains(&a) && (0..19).contains(&b) => {
                let at = Self::relation_at(b, a) + 4;
                let m = &mut self.game.state.mem;
                let v = (m.i16(at) as i32).wrapping_add(delta) as i16;
                m.write(at, &v.to_le_bytes());
            }
            StoryEvent::TradeAgreement { a, b } => {
                for (x, y) in [(a, b), (b, a)] {
                    if (0..19).contains(&x) && (0..19).contains(&y) {
                        self.game.state.mem.set_u8(Self::relation_at(x, y) + 0x24, 1);
                    }
                }
            }
            // Spies (`Kemek[e][k]`, 0x4c bytes).
            StoryEvent::SpyTarget { slot, empire } => self.spy_edit(en, slot, |r| r[0x18] = empire as u8),
            StoryEvent::SpyState { slot, state } => self.spy_edit(en, slot, |r| r[0x10..0x12].copy_from_slice(&(state as i16).to_le_bytes())),
            StoryEvent::SpyExperience { slot, add } => self.spy_edit(en, slot, |r| {
                let v = rd_i32(r, 0x3c).wrapping_add(add);
                r[0x3c..0x40].copy_from_slice(&v.to_le_bytes());
            }),
            StoryEvent::KillSpy { empire, slot } => self.spy_edit(empire, slot, |r| r[0x10..0x12].fill(0)),
            // Undo actions of a finished story that reach the world (`campaign.md` §2.3).
            StoryEvent::Undo { kind: 1, value } => self.planet_edit(value, |r| r[0xba] = r[0xba].wrapping_sub(1)),
            StoryEvent::Undo { kind: 9, value } if (1..=self.planet_count()).contains(&value) => self.planet_edit(value, |r| r[0x6e] = 0),
            // Planets: owners, colonies, the planet info lines, buildings.
            StoryEvent::SetPlanetOwner { planet, owner, race } if planet > 0 => self.with_game(|g| {
                if g.planet_owner(planet as usize) == 0 {
                    g.story_colonise(planet as usize, owner.max(0) as usize, race as u8, true, false);
                } else {
                    g.capture(planet as usize, owner as u8);
                    if race != 0 {
                        g.planet_byte(planet as usize, 0x21, race as u8);
                    }
                }
            }),
            StoryEvent::Colonise { planet, owner } if planet > 0 => {
                self.with_game(|g| g.story_colonise(planet as usize, owner.max(0) as usize, owner as u8, false, false))
            }
            StoryEvent::ColoniseAs { planet, owner, race } if planet > 0 && self.planet_owner(planet) != owner => {
                let race = if race != 0 { race } else { owner };
                self.with_game(|g| g.story_colonise(planet as usize, owner.max(0) as usize, race as u8, false, false))
            }
            StoryEvent::DestroyPlanet(p) => self.with_game(|g| g.destroy_planet(p)),
            StoryEvent::RazePlanet(p) if p > 0 => self.with_game(|g| {
                let p = p as usize;
                for k in 1..=g.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32) {
                    if g.state.buildings.get(p).and_then(|b| b.get(k as usize)).is_some_and(|b| b[0] != 0) {
                        g.with_war(p, |w| w.delete_building(k));
                    }
                }
                g.capture(p, 0);
            }),
            StoryEvent::Volcano(p) if p > 0 => self.with_game(|g| g.volcano(p as usize)),
            StoryEvent::AddPlanetInfo { planet, info } => self.planet_info_add(planet, info, true),
            StoryEvent::AddPlanetInfoHidden { planet, info } => self.planet_info_add(planet, info, false),
            StoryEvent::RemovePlanetInfo { planet, info } => self.planet_info_remove(planet, info),
            StoryEvent::VirusCheck(p) => {
                if !self.planet_has_info(p, VIRUS_INFO) {
                    self.planet_info_add(p, VIRUS_INFO, true);
                }
            }
            StoryEvent::EpidemicOver => {
                for p in 1..=self.planet_count() {
                    if self.planet_has_info(p, EPIDEMIC_INFO) {
                        self.planet_info_remove(p, EPIDEMIC_INFO);
                    }
                }
            }
            StoryEvent::Undo { kind: 2, value } => self.planet_info_remove(value >> 16, -(value & 0xffff)),
            StoryEvent::Undo { kind: 6, value } => self.planet_info_remove(value >> 16, value as i16 as i32),
            StoryEvent::PlaceBuilding { planet, kind } if planet > 0 => self.with_game(|g| g.place(planet as usize, kind as u8, true, 0)),
            StoryEvent::PlaceStoryBuilding { planet, kind, level } if planet > 0 => {
                self.with_game(|g| g.place(planet as usize, kind as u8, true, level))
            }
            StoryEvent::PlaceBuildingUnbuilt { planet, kind } if planet > 0 => self.with_game(|g| g.place(planet as usize, kind as u8, false, 0)),
            StoryEvent::DeleteBuilding { planet, slot } if planet > 0 => self.with_game(|g| g.with_war(planet as usize, |w| w.delete_building(slot))),
            // The battlefields (`WF[p]`).
            StoryEvent::SelfDestruct { planet, value } | StoryEvent::SelfDestructTimer { planet, value } => {
                self.field_edit(planet, |f| f.self_destruct = value)
            }
            StoryEvent::MadUnit { planet, unit } => self.unit_edit(planet, unit, |u| u.flags |= 4),
            StoryEvent::CalmUnit { planet, unit } => self.unit_edit(planet, unit, |u| u.flags &= !(4 | 0x900)),
            StoryEvent::UnitDesign { planet, unit, design } => self.unit_edit(planet, unit, |u| u.design = design),
            StoryEvent::RiotUnit { planet, unit, fleet } if planet > 0 => self.with_game(|g| g.with_war(planet as usize, |w| w.riot(unit, fleet))),
            // Techs, money, trade.
            StoryEvent::SetTechState { tech, state, empire } => {
                let e = if empire < 0 { en } else { empire };
                self.with_game(|g| g.set_tech_state(tech, state, e))
            }
            StoryEvent::GiveTech(t) => self.with_game(|g| g.set_tech_state(t, 5, en)),
            StoryEvent::AddMoney { empire, amount } => {
                let e = if empire < 1 { en } else { empire };
                self.with_game(|g| g.add_money(e as usize, amount))
            }
            StoryEvent::TradePrices { cheap } => {
                let mut t = self.game.state.trade();
                for o in t.offers.iter_mut() {
                    let v = (rd_i32(o, 0x24) as f64 * if cheap { 0.8 } else { 1.25 }) as i32;
                    o[0x24..0x28].copy_from_slice(&v.to_le_bytes());
                }
                self.game.state.store_trade(&t);
            }
            StoryEvent::DestroyerTutorial => self.with_game(|g| g.destroyer_for_sale()),
            // Diplomacy.
            StoryEvent::DiscoverEmpire(e) if (0..19).contains(&e) => {
                let at = va::EMPIRE_KNOWS + (en * 19 + e) as u32;
                if self.mem().u8(at) == 0 {
                    self.game.state.mem.set_u8(at, 1);
                    self.with_game(|g| g.with_diplomacy(|d, w| d.discover(w, e as usize)));
                }
            }
            StoryEvent::DeclareWar { a, b } if (0..19).contains(&a) && (0..19).contains(&b) => {
                let at = Self::relation_at(b, a) + 4;
                let m = &mut self.game.state.mem;
                let v = (m.i16(at) as i32).wrapping_sub(10_000) as i16;
                m.write(at, &v.to_le_bytes());
                self.set_levels(a, b, 1);
            }
            StoryEvent::MakeNeutral { a, b } => self.peace(a, b, 3),
            StoryEvent::MakeNonAggression { a, b } => self.peace(a, b, 4),
            StoryEvent::MakeAllies { a, b } => self.peace(a, b, 5),
            StoryEvent::DiplomacySpecial { id, empire } => self.with_game(|g| g.add_diplomacy_special(id, empire)),
            StoryEvent::RemoveDiplomacySpecial { id, empire } => self.with_game(|g| g.remove_diplomacy_special(id, empire)),
            StoryEvent::Undo { kind: 3, value } => self.with_game(|g| g.remove_diplomacy_special(value & 0xffff, value >> 16)),
            // Spies.
            StoryEvent::SpyMission { id, on } => self.spy_mission(id, on),
            StoryEvent::Undo { kind: 5, value } => self.spy_mission(value, false),
            StoryEvent::SpyCaught { spy, empire, enemy } => {
                let mut x = [0; 9];
                (x[1], x[2]) = (empire, enemy);
                self.game.state.push_spy_report(&crate::spy::Report { kind: 3, a: spy, x, ..Default::default() });
            }
            StoryEvent::SpyReport { kind, spy } => {
                let mut x = [0; 9];
                x[1] = en;
                self.game.state.push_spy_report(&crate::spy::Report { kind: kind as u8, a: spy, x, ..Default::default() });
            }
            StoryEvent::StorySpy { slot, xp } => {
                let name = self.story_text(STORY_AGENT);
                self.spy_edit(en, slot, |r| {
                    r.copy_from_slice(&story_spy(xp));
                    strcpy(&mut r[..0x10], &name);
                });
            }
            StoryEvent::StoryRecruit { slot, name, xp } => {
                let name = self.recruit_name(name);
                self.spy_edit(en, slot, |r| {
                    r.copy_from_slice(&story_recruit(xp));
                    strcpy(&mut r[..0x10], &name);
                });
            }
            // The space battle starting (`st_AddFleetToSpWar`, exe 0x5c9cd3): entry `n + 1` of `spFlottak`, `n < 4`,
            // side 1 (a byte, exe 0x5c9cf2). `space_war` reads the list back.
            StoryEvent::JoinBattle(f) => {
                let m = &mut self.game.state.mem;
                let n = m.i32(super::battles::at::SP_COUNT);
                if n < 4 {
                    let at = super::battles::at::SP_FLEETS + 6 * (n as u32 + 1);
                    m.write(at, &(f as i16).to_le_bytes());
                    m.set_u8(at + 2, 1);
                    m.set_i32(super::battles::at::SP_COUNT, n + 1);
                }
            }
            // `st_AddGameInfo` / `RemoveGameInfo` (undo 4 too): the saved info lines.
            StoryEvent::GameInfo { id, value } => self.with_game(|g| g.add_game_info_number(id, value)),
            StoryEvent::InfoLine { id, line } => self.with_game(|g| g.add_game_info_line(id, line)),
            StoryEvent::RemoveGameInfo(id) | StoryEvent::Undo { kind: 4, value: id } => self.with_game(|g| g.remove_game_info(id)),
            StoryEvent::ShowWormhole(w) if w > 0 => {
                let at = va::WORMHOLES + 0x1c * w as u32;
                self.game.state.mem.set_u8(at + 8, 1);
                self.game.state.mem.set_u8(at + 0x14, 1);
            }
            _ => {}
        }
    }

    fn relation_at(a: i32, b: i32) -> u32 {
        va::RELATIONS + ((a as usize * crate::colony_side::EMPIRES + b as usize) * 0x26) as u32
    }
    fn planet_edit(&mut self, p: i32, f: impl FnOnce(&mut Vec<u8>)) {
        if !(0..=self.planet_count()).contains(&p) {
            return;
        }
        let mut r = self.planet(p);
        f(&mut r);
        self.game.state.mem.write(va::PLANETS + (p as usize * PLANET_SIZE) as u32, &r);
    }
    fn building_edit(&mut self, p: i32, k: i32, f: impl FnOnce(&mut [u8; crate::save::BUILDING_SIZE])) {
        let (Ok(p), Ok(k)) = (usize::try_from(p), usize::try_from(k)) else { return };
        if let Some(b) = self.game.state.buildings.get_mut(p).and_then(|t| t.get_mut(k)) {
            f(b);
        }
    }
    fn spy_edit(&mut self, e: i32, k: i32, f: impl FnOnce(&mut Vec<u8>)) {
        let Some(at) = Self::spy_at(e, k) else { return };
        let mut r = self.mem().read(at, crate::spy::RECORD);
        f(&mut r);
        self.game.state.mem.write(at, &r);
    }
    /// `BolygoInfoAdd(p, info, dup)` (exe 0x53beb0): the first free slot of 1..=5 (`+0x168 + 2i`) takes `info`
    /// unless `!dup` and the planet has it already; an info flagged in `infostr.txt` counts in `+0xba`.
    fn planet_info_add(&mut self, p: i32, info: i32, dup: bool) {
        if !dup && self.planet_has_info(p, info) {
            return;
        }
        let shown = self.game.info_string_flag(info);
        self.planet_edit(p, |r| {
            if let Some(i) = (1..=5).find(|&i| rd_i16(r, 0x168 + 2 * i) == 0) {
                r[0x168 + 2 * i..0x16a + 2 * i].copy_from_slice(&(info as i16).to_le_bytes());
                if shown {
                    r[0xba] = r[0xba].wrapping_add(1);
                }
            }
        });
    }
    /// `BolygoInfoRemove(p, info, _)` (exe 0x53c0b0): the first slot holding `info` is cleared (no compaction).
    fn planet_info_remove(&mut self, p: i32, info: i32) {
        let shown = self.game.info_string_flag(info);
        self.planet_edit(p, |r| {
            if let Some(i) = (1..=5).find(|&i| rd_i16(r, 0x168 + 2 * i) == info) {
                r[0x168 + 2 * i..0x16a + 2 * i].fill(0);
                if shown && r[0xba] != 0 {
                    r[0xba] -= 1;
                }
            }
        });
    }
    fn field_edit(&mut self, p: i32, f: impl FnOnce(&mut crate::ground::Field)) {
        let Some(p) = usize::try_from(p).ok().filter(|&p| p > 0) else { return };
        let mut field = self.game.state.take_ground_field(p);
        f(&mut field);
        self.game.state.store_ground_field(p, field);
    }
    fn unit_edit(&mut self, p: i32, u: i32, f: impl FnOnce(&mut crate::ground::Group)) {
        self.field_edit(p, |field| {
            if let Some(g) = usize::try_from(u).ok().and_then(|u| field.units.get_mut(u)) {
                f(g);
            }
        });
    }
    /// `Viszony[a][b]` and `[b][a]` levels (`+0x25`).
    fn set_levels(&mut self, a: i32, b: i32, level: u8) {
        for (x, y) in [(a, b), (b, a)] {
            if (0..19).contains(&x) && (0..19).contains(&y) {
                self.game.state.mem.set_u8(Self::relation_at(x, y) + 0x25, level);
            }
        }
    }
    /// `st_SetSemlegesViszony` & co. (exe 0x6028a0, 0x602860, 0x6028e0): `BirodalomBeketKot(a, b)`, then both levels.
    fn peace(&mut self, a: i32, b: i32, level: u8) {
        if !((0..19).contains(&a) && (0..19).contains(&b)) {
            return;
        }
        self.with_game(|g| g.with_diplomacy(|d, w| d.make_peace(w, a as usize, b as usize)));
        self.set_levels(a, b, level);
    }
    /// `st_EnableKemKuldetes(id)` / `st_DisableKemKuldetes(id)`: the story mission `14 + k` with `StoryKuldetes[k] == id`.
    fn spy_mission(&mut self, id: i32, on: bool) {
        if let Some(k) = (0..12).find(|&k| self.campaign_tables().story_mission(k) == id) {
            self.game.state.mem.set_u8(va::MISSIONS + 0x20 * (14 + k) as u32, on as u8);
        }
    }

    /// Run `f` on the AI's unit operations (the exe's `FlottaDelete`, `TankNew` & co.), then the battlefield side of
    /// the garrison hand-offs they raised.
    fn units(&mut self, f: impl FnOnce(&mut crate::ai::World)) {
        self.with_game(|g| {
            g.with_ai(|_, w, _| f(w));
            g.battlefield_hand_offs();
        })
    }
    fn fleet_mut(&mut self, f: i32) -> Option<&mut [u8; 0x80]> {
        self.game.state.fleets.get_mut(usize::try_from(f).ok().filter(|&f| f > 0)?)
    }
    fn fleet_write(&mut self, f: i32, o: usize, bytes: &[u8]) {
        if let Some(r) = self.fleet_mut(f) {
            r[o..o + bytes.len()].copy_from_slice(bytes);
        }
    }
    fn fleet_flags_write(&mut self, f: i32, set: u16, clear: u16) {
        if let Some(r) = self.fleet_mut(f) {
            let v = (u16::from_le_bytes([r[0x52], r[0x53]]) | set) & !clear;
            r[0x52..0x54].copy_from_slice(&v.to_le_bytes());
        }
    }
    fn fleet_write_name(&mut self, f: i32, name: &[u8]) {
        if let Some(r) = self.fleet_mut(f) {
            strcpy(&mut r[..0x15], name);
        }
    }
    fn design_edit(&mut self, e: i32, d: i32, f: impl FnOnce(&mut [u8; DESIGN])) {
        let (Ok(e), Ok(d)) = (usize::try_from(e), usize::try_from(d)) else { return };
        if let Some(r) = self.game.state.designs.get_mut(e).and_then(|t| t.get_mut(d)) {
            f(r);
        }
    }
    /// Story 840's colony discount and its undo (type 11; `campaign/elem.md` §840): the human's colony-ship designs
    /// 40..=`OsszHanyTerv[EN]` (byte `+0x34`) get `+0x24` and `+0x20` halved (C division) or doubled, and the
    /// human's planets building one (`+0x78 > 0`) the build time left `+0xa0`.
    fn colony_discount(&mut self, halve: bool) {
        let f = |x: i32| if halve { x / 2 } else { x.wrapping_add(x) };
        let en = self.g.en as i32;
        for d in 40..=self.design_count(en) {
            if self.design_colony(en, d) {
                self.design_edit(en, d, |r| {
                    for o in [0x24, 0x20] {
                        let v = f(rd_i32(r, o));
                        r[o..o + 4].copy_from_slice(&v.to_le_bytes());
                    }
                });
            }
        }
        for p in 1..=self.planet_count() {
            let mut r = self.planet(p);
            let d = rd_i16(&r, 0x78);
            if r[0x20] as i32 == en && d > 0 && self.design_colony(en, d) {
                let v = f(rd_i32(&r, 0xa0));
                r[0xa0..0xa4].copy_from_slice(&v.to_le_bytes());
                self.game.state.mem.write(va::PLANETS + (p as usize * PLANET_SIZE) as u32, &r);
            }
        }
    }

}

impl Game<'_> {
    /// `HajoNew(f, d)` (exe 0x53e800, `production-research.md` §5.1): one ship of the fleet owner's design.
    pub(super) fn new_ship(&mut self, f: i32, d: i32) {
        if f < 1 {
            return;
        }
        let mut yard = self.shipyard();
        if f as usize >= yard.fleets.len() {
            return;
        }
        let mut ev = Vec::new();
        yard.new_ship(f as usize, d as i16, &mut ev);
        self.store_shipyard(&yard, &ev);
        self.events.extend(ev.into_iter().map(GameEvent::Production));
    }

    /// `TankNew(f, d, n)` (exe 0x53ed80 area, `production-research.md` §5.1).
    pub(super) fn new_vehicles(&mut self, f: i32, d: i32, n: i32) {
        let mut yard = self.shipyard();
        let mut ev = Vec::new();
        yard.new_vehicles(f, d as i16, n, &mut ev);
        self.store_shipyard(&yard, &ev);
        self.events.extend(ev.into_iter().map(GameEvent::Production));
    }

    /// `FlottaMozgat(f, 0, 0, 0, 0)`: the fleet stops.
    pub(super) fn stop_fleet(&mut self, f: i32) {
        let g = self.globals();
        let ctx = self.starmap_context(&g);
        let mut map = self.state.starmap();
        let mut ev = Vec::new();
        map.order(f.max(0) as usize, 0, 0, 0.0, 0.0, &ctx, &mut ev);
        self.state.store_starmap(&map);
    }

    /// `FlottaNew(name, owner, id, x, y, speed)` (exe 0x53d400, `production-research.md` §5.1): the new fleet's slot.
    pub(super) fn new_fleet(&mut self, name: &[u8], owner: u8, id: i16, x: f32, y: f32, speed: f32) -> usize {
        let g = self.globals();
        let ctx = self.production_context(&g, g.speed);
        let mut yard = self.shipyard();
        let mut ev = Vec::new();
        let f = yard.new_fleet(name, owner, id, x, y, speed, &ctx, &mut ev);
        self.store_shipyard(&yard, &ev);
        self.events.extend(ev.into_iter().map(GameEvent::Production));
        f
    }

    /// `Spy_New(e)` (exe 0x58f830, `espionage.md` §6.1): the new spy's slot (−1 no room).
    pub(super) fn new_spy(&mut self, e: usize) -> i32 {
        let mut slot = 0;
        self.with_spies(|sp, w| slot = sp.new_spy(w, e));
        slot
    }
}

/// `Virus_CheckAdd`'s info line (the epidemic's switch, `StoryAdat+0x30`, is the story's).
const VIRUS_INFO: i32 = 100;
/// The info line `StoryAdat+0x30`'s clearing takes off every planet.
const EPIDEMIC_INFO: i32 = 0x65;
/// `_s(5, 3092)`: the story agent's name (`st_Fable_include` 310).
const STORY_AGENT: i32 = 3092;

/// The story agent's spy record (`st_Fable_include` 310, `campaign/fable.md`): state 1, race 1, `+0x16 := 1`, the
/// experience `xp`, rank 1.
fn story_spy(xp: i32) -> [u8; crate::spy::RECORD] {
    let mut r = [0u8; crate::spy::RECORD];
    let mut put = |o: usize, b: &[u8]| r[o..o + b.len()].copy_from_slice(b);
    put(0x10, &1i16.to_le_bytes());
    put(0x12, &[1]);
    put(0x14, &4i16.to_le_bytes());
    put(0x16, &1i16.to_le_bytes());
    put(0x1a, &4i16.to_le_bytes());
    put(0x1c, &[3]);
    put(0x24, &1000i32.to_le_bytes());
    put(0x30, &0x100i32.to_le_bytes());
    put(0x3c, &xp.to_le_bytes());
    put(0x40, &1i32.to_le_bytes());
    r
}

/// `st_Rnd` 138's recruit (`campaign/rnd.md`, its stack template; the bytes the stack held are 0 here): race 14,
/// skills 5, 2, 6, 1, `+0x16 := 1`, the experience `xp`, rank 2.
fn story_recruit(xp: i32) -> [u8; crate::spy::RECORD] {
    let mut r = [0u8; crate::spy::RECORD];
    let mut put = |o: usize, b: &[u8]| r[o..o + b.len()].copy_from_slice(b);
    put(0x10, &1i16.to_le_bytes());
    put(0x12, &[14]);
    put(0x14, &4i16.to_le_bytes());
    put(0x16, &1i16.to_le_bytes());
    put(0x1a, &4i16.to_le_bytes());
    put(0x1c, &[3]);
    put(0x24, &1000i32.to_le_bytes());
    put(0x31, &[5, 2, 6, 1]);
    put(0x3c, &xp.to_le_bytes());
    put(0x40, &2i32.to_le_bytes());
    r
}

impl Game<'_> {
    fn planet_byte(&mut self, p: usize, o: u32, v: u8) {
        self.state.mem.set_u8(va::PLANETS + (p * PLANET_SIZE) as u32 + o, v);
    }

    /// `InfoStrings[InfoString_GetIndex(info)]`'s flag (`II`): the info counts in planet `+0xba`.
    fn info_string_flag(&self, info: i32) -> bool {
        let s = &self.rules.story.info_strings;
        s.iter().position(|&(id, _)| id as i32 == info).is_some_and(|k| k > 0 && s[k].1)
    }

    /// `st_BolygoDestroy(p)` (exe 0x5d47b0): `BolygoElfoglal(p, 0)`, every empire's view of it `−1`, and
    /// `StarmapBolygoFelrobban(p)` (exe 0x5a7e10): a selected planet `p` gives way to the last planet the human sees
    /// (`AktPlanet`, classic view only; the rest is the UI's).
    fn destroy_planet(&mut self, p: i32) {
        if p < 1 {
            return;
        }
        let p = p as usize;
        self.capture(p, 0);
        let at = va::PLANETS + (p * PLANET_SIZE) as u32;
        self.state.mem.write(at + 0xda, &[0xff; 36]);
        if self.view != ViewMode::Classic {
            return;
        }
        let en = self.globals().en;
        let akt = self.state.mem.i32(va::AKT_PLANET);
        let mut pick = akt;
        for i in 1..=self.state.planet_count() {
            let r = self.planet_rec(i);
            if rd_i16(&r, 0xd8 + 2 * en) > 0 && i != p && (pick == p as i32 || r[0x20] as usize == en) {
                pick = i as i32;
            }
        }
        if akt == p as i32 {
            self.state.mem.set_i32(va::AKT_PLANET, pick);
        }
    }

    /// `Story_VulkanKitores(p)` (exe 0x5c95b0): the planet turns volcanic (`+0x22 := 4`, `+0x23 := 0x3f`); on an owned
    /// planet each building in slot order is deleted, the first colony hub dropped, the others built again only where
    /// `LocateSuitableLocation` keeps their cell (the old record, its position fields new), then a new colony hub.
    fn volcano(&mut self, p: usize) {
        let at = va::PLANETS + (p * PLANET_SIZE) as u32;
        self.state.mem.set_u8(at + 0x22, 4);
        self.state.mem.set_u8(at + 0x23, 0x3f);
        let owner = self.state.mem.u8(at + 0x20);
        if owner == 0 {
            return;
        }
        let g = self.globals();
        let econ = self.economy_context(&g);
        let mut hub_seen = false;
        let mut i = 1;
        while i <= self.state.mem.i32(va::BUILDING_SLOTS + 4 * p as u32) {
            let rec = self.state.buildings.get(p).and_then(|b| b.get(i as usize)).copied().unwrap_or([0; BUILDING_SIZE]);
            let kind = rec[0];
            if kind != 0 {
                let f = |o: usize| f32::from_le_bytes(rec[o..o + 4].try_into().unwrap()) as i32;
                let (x, y) = (f(0xc), f(0x10));
                self.with_war(p, |w| w.delete_building(i));
                if kind == 1 && !hub_seen {
                    hub_seen = true;
                } else {
                    let mut world = self.state.colony();
                    let cells = self.state.vehicle_cells(p);
                    if self.rules.placement.locate(&world.planets[p], kind, &cells, Some((x, y))) == Some((x, y)) {
                        let slot = world.new_building(p, kind, true, (x as f32, y as f32), 0, &self.rules.side, &econ);
                        self.state.store_colony(&world);
                        self.fill_fresh_buildings();
                        if let Some(b) = self.state.buildings.get_mut(p).and_then(|b| b.get_mut(slot)) {
                            let pos: [u8; 12] = b[0xc..0x18].try_into().unwrap();
                            *b = rec;
                            b[0xc..0x18].copy_from_slice(&pos);
                        }
                        self.with_war(p, |w| {
                            w.add_building_turrets(kind, owner as i32, slot as i32);
                            w.building_built(slot as i32);
                        });
                    }
                }
            }
            i += 1;
        }
        self.place(p, 1, true, 0);
    }

    /// `st_SetTalalmanyAllapot(t, state, e)` (exe 0x602920; the specials queue entry is the story's): a tech leaving
    /// state 4 stops the research; states above 2 clear the tech's negative prerequisites, state 4 starts it; 5 gives it
    /// (`TalalmanyKifejlesztve`, its research time from the race), the others write the status; then the recompute.
    fn set_tech_state(&mut self, t: i32, state: i32, e: i32) {
        if !(0..crate::save::TECHS as i32).contains(&t) || !(0..EMPIRES as i32).contains(&e) {
            return;
        }
        let (t, e) = (t as usize, e as usize);
        let tat = va::TECHS + (t * crate::save::TECH_SIZE) as u32;
        if state > 2 {
            for i in 1..=3 {
                if (self.state.mem.u8(tat + 0x2c + i) as i8) < 0 {
                    self.state.mem.set_u8(tat + 0x2c + i, 0);
                }
            }
        }
        let g = self.globals();
        let ctx = self.research_context(&g);
        let mut techs = self.state.tech();
        let mut res = self.state.research();
        if state != 4 && techs.status(t, e) == status::RESEARCHING {
            res.current[e] = 0;
        }
        if state == 4 {
            res.current[e] = t as i32;
        }
        let mut events = Vec::new();
        if state == 5 {
            let mult = self.rules.research.race_time.get(ctx.race[e] as usize).copied().unwrap_or(0.0);
            techs.set_research_time(t, e, (techs.base_research_time(t) as f32 * mult) as i32);
            research::developed(&mut techs, &mut res, e, t, &self.rules.research, &ctx, &mut events);
        } else {
            techs.set_status(t, e, state as u8);
        }
        techs.recompute(ctx.players, ctx.all_techs);
        self.state.store_tech(&techs);
        self.state.store_research(&res);
        for ev in &events {
            if let ResearchEvent::AutoDesign { empire, tech } = *ev {
                self.automatic_design(empire as usize, tech);
            }
        }
        self.events.extend(events.into_iter().map(GameEvent::Research));
    }

    /// `st_AddMoney` → `MoneyHozzaadOld(e, −1, x)` (exe 0x53fbd0): every pool by its share.
    fn add_money(&mut self, e: usize, x: f64) {
        let e = e.min(EMPIRES - 1);
        let income = self.ai_income(e);
        let mut t = self.state.treasuries();
        t[e].add(-1, x, income);
        self.state.store_treasuries(&t);
    }

    /// `DestoryerATradereTutorialhoz()` (exe 0x6057f0): empire 1's design 3 dated and offered at half price under tech
    /// 5's name in the first used-up market slot (`+0x16 < 1`), else a new one below 0x28.
    pub(super) fn destroyer_for_sale(&mut self) {
        let date = [va::EV, va::HONAP, va::NAP].map(|a| self.state.mem.i32(a));
        let Some(d) = self.state.designs.get_mut(1).and_then(|t| t.get_mut(3)) else { return };
        d[0x6c..0x70].copy_from_slice(&5i32.to_le_bytes());
        d[0x70] = 0;
        for (k, v) in date.iter().enumerate() {
            d[0x74 + 4 * k..0x78 + 4 * k].copy_from_slice(&v.to_le_bytes());
        }
        let mut r = *d;
        let mut t = self.state.trade();
        let slot = match t.offers.iter().position(|o| rd_i16(o, 0x16) < 1) {
            Some(i) => i,
            None if t.offers.len() < 0x28 => {
                t.offers.push([0; DESIGN]);
                t.offers.len() - 1
            }
            None => return,
        };
        let name = self.state.mem.read(va::TECHS + (5 * crate::save::TECH_SIZE) as u32, 0x20);
        strcpy(&mut r[..0x16], &name);
        let price = (rd_i32(&r, 0x24) as f64 * 0.5) as i32;
        r[0x24..0x28].copy_from_slice(&price.to_le_bytes());
        t.offers[slot] = r;
        self.state.store_trade(&t);
    }

    /// `Diplomacy_AddSpecial(id, e)` (exe 0x4bf4b0): a known special (`special.eng`) takes the first hole of
    /// `SpecialLista` (12-byte `{id, empire, index}`), else a new entry.
    fn add_diplomacy_special(&mut self, id: i32, e: i32) {
        let Some(k) = self.rules.story.diplomacy_specials.iter().position(|&s| s == id) else { return };
        let m = &mut self.state.mem;
        let n = m.i32(SPECIAL_COUNT).max(0);
        let j = (0..n).find(|&j| m.i32(SPECIAL_LIST + 12 * j as u32) == 0).unwrap_or_else(|| {
            m.set_i32(SPECIAL_COUNT, n + 1);
            n
        });
        m.set_i32s(SPECIAL_LIST + 12 * j as u32, &[id, e, k as i32]);
    }

    /// `Diplomacy_RemoveSpecial(id, e)` (exe 0x4c0010): the entry's id cleared, the trailing holes cut off the count.
    fn remove_diplomacy_special(&mut self, id: i32, e: i32) {
        let m = &mut self.state.mem;
        let mut n = m.i32(SPECIAL_COUNT).max(0);
        let at = |j: i32| SPECIAL_LIST + 12 * j as u32;
        let Some(j) = (0..n).find(|&j| m.i32(at(j)) == id && m.i32(at(j) + 4) == e) else { return };
        m.set_i32(at(j), 0);
        while n != 0 && m.i32(at(n - 1)) == 0 {
            n -= 1;
        }
        m.set_i32(SPECIAL_COUNT, n);
    }
}

/// `SpecialLista` (exe 0xA42DE8) and `SpecialListaMut` (0xA41DC4): the diplomacy specials offered (`diplomacy.md` §6.1).
const SPECIAL_LIST: u32 = 0xA42DE8;
const SPECIAL_COUNT: u32 = 0xA41DC4;

impl Game<'_> {
    /// Story 908 (`campaign.md` §8, ELF `Story_Check` 0x73cc0f..): empires 1..=18, designs 0..=0x28 := `Tervek0[k]` with
    /// the empire's race (`+0x40`); from 1 on `TervAutoDesign(e, k, 0)` and `TervUjraSzamol(e, k, 0, 1)`.
    pub(super) fn reset_designs(&mut self) {
        let templates = &self.rules.story.design_templates;
        if templates.is_empty() {
            return;
        }
        let races = self.state.races();
        for (e, &race) in races.iter().enumerate().skip(1) {
            let Some(list) = self.state.designs.get_mut(e) else { continue };
            for (k, t) in templates.iter().enumerate() {
                if let Some(d) = list.get_mut(k) {
                    *d = *t;
                    d[0x40..0x42].copy_from_slice(&(race as i16).to_le_bytes());
                }
            }
        }
        let n = templates.len();
        self.with_ai(|_, w, _| {
            for e in 1..EMPIRES {
                for k in 1..n {
                    w.auto_design(e, k, 0);
                    w.recompute_design(e, k);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameRules, StoryRules};
    use crate::save::{Memory, SaveState};

    fn game(rules: &GameRules) -> Game<'_> {
        let mut state = SaveState::from_parts(Memory::default(), Vec::new());
        state.mem.write(va::PLANETS, &vec![0; 3 * PLANET_SIZE]);
        state.mem.set_i32(va::PLANET_COUNT, 2);
        state.mem.write(SPECIAL_LIST, &[0; 12 * 4]);
        state.mem.set_i32(SPECIAL_COUNT, 0);
        state.mem.write(va::RELATIONS, &vec![0; 19 * 19 * 0x26]);
        Game::new(state, rules)
    }

    #[test]
    fn planet_info_lines_count_the_flagged_ones() {
        let rules = GameRules { story: StoryRules { info_strings: vec![(0, false), (100, true), (7, false)], ..Default::default() }, ..Default::default() };
        let mut g = game(&rules);
        let mut w = GameStory::new(&mut g);
        // `Virus_CheckAdd`: info 100 once, flagged: `+0xba` 1.
        w.event(StoryEvent::VirusCheck(2));
        w.event(StoryEvent::VirusCheck(2));
        // `BolygoInfoAdd(p, 7, false)` twice: no duplicate; `(p, 7, true)` duplicates.
        w.event(StoryEvent::AddPlanetInfoHidden { planet: 2, info: 7 });
        w.event(StoryEvent::AddPlanetInfoHidden { planet: 2, info: 7 });
        w.event(StoryEvent::AddPlanetInfo { planet: 2, info: 7 });
        let r = w.planet(2);
        let slots: Vec<i32> = (1..=5).map(|i| rd_i16(&r, 0x168 + 2 * i)).collect();
        assert_eq!((slots, r[0xba]), (vec![100, 7, 7, 0, 0], 1));
        // Removal clears the first match (no compaction) and the count of a flagged one.
        w.event(StoryEvent::RemovePlanetInfo { planet: 2, info: 100 });
        w.event(StoryEvent::RemovePlanetInfo { planet: 2, info: 7 });
        let r = w.planet(2);
        let slots: Vec<i32> = (1..=5).map(|i| rd_i16(&r, 0x168 + 2 * i)).collect();
        assert_eq!((slots, r[0xba]), (vec![0, 0, 7, 0, 0], 0));
    }

    #[test]
    fn diplomacy_specials_reuse_holes_and_trim_the_count() {
        let rules = GameRules { story: StoryRules { diplomacy_specials: vec![0x2179, 0x217b], ..Default::default() }, ..Default::default() };
        let mut g = game(&rules);
        let mut w = GameStory::new(&mut g);
        w.event(StoryEvent::DiplomacySpecial { id: 0x217b, empire: 2 });
        w.event(StoryEvent::DiplomacySpecial { id: 0x2179, empire: 3 });
        w.event(StoryEvent::DiplomacySpecial { id: 0x9999, empire: 3 });
        let list = |w: &GameStory| (w.mem().i32(SPECIAL_COUNT), w.mem().read(SPECIAL_LIST, 24));
        let entry = |id: i32, e: i32, k: i32| [id, e, k].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>();
        assert_eq!(list(&w), (2, [entry(0x217b, 2, 1), entry(0x2179, 3, 0)].concat()));
        // Undo type 3 (`a & 0xffff` the id, `a >> 16` the empire): the last entry goes, the count follows.
        w.event(StoryEvent::Undo { kind: 3, value: 0x3_2179 });
        assert_eq!(list(&w).0, 1);
        w.event(StoryEvent::RemoveDiplomacySpecial { id: 0x217b, empire: 2 });
        assert_eq!(list(&w).0, 0);
        // A hole is reused before the list grows; a removed entry keeps its empire and index.
        w.event(StoryEvent::DiplomacySpecial { id: 0x2179, empire: 4 });
        assert_eq!(list(&w), (1, [entry(0x2179, 4, 0), entry(0, 3, 0)].concat()));
    }

    #[test]
    fn joined_fleets_are_read_back_by_the_battle() {
        use crate::game::battles::at;
        let rules = GameRules::default();
        let mut g = game(&rules);
        // `SpaceWarIndit`'s two entries over 0xAA filler (the story writes the side as a byte: `+3` keeps its filler).
        g.state.mem.write(at::SP_FLEETS, &[0xAA; 6 * 5]);
        for (k, f) in [(1u32, 3i16), (2, 5)] {
            g.state.mem.write(at::SP_FLEETS + 6 * k, &f.to_le_bytes());
        }
        g.state.mem.set_i32(at::SP_COUNT, 2);
        let mut w = GameStory::new(&mut g);
        for f in [7, 8, 9] {
            w.event(StoryEvent::JoinBattle(f));
        }
        drop(w);
        // `n < 4`: the third join is dropped; entries 3 and 4 get side 1 in their `+2` byte only.
        assert_eq!(g.sp_fleets(), [3, 5, 7, 8]);
        assert_eq!(g.state.mem.read(at::SP_FLEETS + 6 * 3, 6), [7, 0, 1, 0xAA, 0xAA, 0xAA]);
    }

    #[test]
    fn war_and_relation_changes_write_both_ways() {
        let rules = GameRules::default();
        let mut g = game(&rules);
        let mut w = GameStory::new(&mut g);
        w.event(StoryEvent::RelationChange { a: 1, b: 2, delta: 300 });
        w.event(StoryEvent::DeclareWar { a: 1, b: 2 });
        assert_eq!(w.mem().i16(GameStory::relation_at(2, 1) + 4), 300 - 10_000);
        assert_eq!((w.relation_level(1, 2), w.relation_level(2, 1)), (1, 1));
        w.event(StoryEvent::TradeAgreement { a: 1, b: 40 });
        // `b` out of range: neither entry is written (story 9 passes an empire that may not exist).
        assert!(!w.trade_agreement(1, 2) && !w.trade_agreement(1, 18));
    }
}
