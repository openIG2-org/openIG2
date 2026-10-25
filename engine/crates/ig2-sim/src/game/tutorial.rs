//! The tutorial's writes to the game (`re/rules/tutorial.md` §3, the *sim* commands): the plain state changes
//! `TutorialGetNextCommand` (exe 0x60c570) makes in place. The others (new fleets, vehicles and ships, the trade
//! market's destroyers, the diplomacy talk, messages, the clock, a save load) go to the systems that own them.

use super::{va, Game};
use crate::colony_side::EMPIRES;
use crate::save::{PLANET_SIZE, TECH_SIZE};
use crate::tutorial::Effect;

impl Game<'_> {
    /// `FRISSIT_Starmap`'s tutorial sighting (exe 0x595a77..0x595ae6, `re/rules/tutorial.md` §4): while event 1 is
    /// awaited, the planet's name := `GetString(2, 4, 6)` (`strcpy`: the bytes after the NUL stay), then
    /// `TutorialCheckEvent(1)`: `TutorialEvent` 0 and, when paused, `TutorialUnPause`'s flags (its input resets and
    /// the interpreter's own copy are the driver's: `StarmapEvent::TutorialPlanetSighted`).
    pub(crate) fn tutorial_sighting(&mut self, p: usize) {
        let m = &mut self.state.mem;
        if (m.u8(va::TUTORIAL_ACTIVE) == 0 && m.u8(va::TUTORIAL_PAUSED) == 0) || m.i32(va::TUTORIAL_EVENT) != 1 {
            return;
        }
        let name = cstr(&self.rules.messages.new_planet);
        let mut text = name.to_vec();
        text.push(0);
        m.write(va::PLANETS + (p * PLANET_SIZE) as u32, &text);
        self.tutorial_check_event(1);
    }

    /// `TutorialCheckEvent(id)` (exe 0x60bdf0): the awaited event `id` clears `TutorialEvent` and, when paused, sets
    /// `TutorialUnPause`'s flags (its input resets are the driver's).
    pub fn tutorial_check_event(&mut self, id: i32) {
        crate::save::tutorial_check_event(&mut self.state.mem, id);
    }

    /// The new fleets' planet: EN's last of 1..HanyBolygo−1 (strict), planet 0 with none (the exe; spec §8).
    fn tutorial_spot(&self, en: usize) -> (f32, f32) {
        let n = self.state.planet_count();
        let rec = |p: usize| va::PLANETS + (p * PLANET_SIZE) as u32;
        let p = (1..n).rev().find(|&p| self.state.mem.u8(rec(p) + 0x20) as usize == en).unwrap_or(0);
        (self.state.mem.f32(rec(p) + 0x18), self.state.mem.f32(rec(p) + 0x1c))
    }

    /// Carry out a tutorial command's effect on the game, if it is one of the in-place writes; false otherwise
    /// (the caller hands it to the UI or the owning system).
    pub fn tutorial_effect(&mut self, e: &Effect) -> bool {
        let en = self.state.mem.i32(va::EN).clamp(0, EMPIRES as i32 - 1) as usize;
        let planets = self.state.planet_count();
        let planet = |p: usize| va::PLANETS + (p * PLANET_SIZE) as u32;
        match e {
            // `MoneyHozzaadLog(EN, 0, n)`.
            Effect::GiveMoney(n) => {
                let income = self.ai_income(en);
                let mut t = self.state.treasuries();
                t[en].add(0, *n as f64, income);
                self.state.store_treasuries(&t);
            }
            // `Talalmanyok[t].state[EN]` (index unchecked in the exe).
            Effect::Invention { tech, state } => {
                if (0..crate::save::TECHS as i32).contains(tech) {
                    self.state.mem.set_u8(va::TECHS + (*tech as usize * TECH_SIZE) as u32 + 0x45 + en as u32, *state);
                }
            }
            // The first planet of that name: `explored[EN] = 1`.
            Effect::ShowPlanet(name) => {
                if let Some(p) = (1..=planets).find(|&p| cstr(&self.state.mem.read(planet(p), 0x18)) == name.as_slice()) {
                    self.state.mem.write(planet(p) + 0xd8 + 2 * en as u32, &1i16.to_le_bytes());
                }
            }
            // `FelfedezBirodalom(e, true)` first for the diplomacy form, then `BirodalomIsmeri[EN][e] = 1`.
            Effect::ExploreEmpire { empire, diplomacy } => {
                let o = *empire;
                if *diplomacy && (0..EMPIRES as i32).contains(&o) {
                    self.with_diplomacy(|d, w| d.discover(w, o as usize));
                }
                self.state.mem.set_u8((va::EMPIRE_KNOWS as i32 + o + en as i32 * 0x13) as u32, 1);
            }
            // Planet +0x6c (autobuild) = 0 on planets 1..HanyBolygo, every owner.
            Effect::DisableAutobuild => {
                for p in 1..=planets {
                    self.state.mem.set_u8(planet(p) + 0x6c, 0);
                }
            }
            // The merchants (owner 9): hold bit +0x18 bit 0; disabling also sets +0x16 = 0x29a.
            Effect::Traders { enabled } => {
                for f in self.state.fleets.iter_mut().skip(1).filter(|f| f[0x15] == 9) {
                    if *enabled {
                        f[0x18] &= !1;
                    } else {
                        f[0x18] |= 1;
                        f[0x16..0x18].copy_from_slice(&0x29au16.to_le_bytes());
                    }
                }
            }
            // `WAITFOREVENT`: planet +0x186 = 1 on EN's planets.
            Effect::MarkOwnedPlanets => {
                for p in 1..=planets {
                    if self.state.mem.u8(planet(p) + 0x20) as usize == en {
                        self.state.mem.set_u8(planet(p) + 0x186, 1);
                    }
                }
            }
            // `WAITFORBATTLE` (exe 0x60fd1f): `TankNew(f, 9, 2)` for every fleet 1..=OsszHanyFlotta of EN's with no
            // vehicle entry (`CountMyTanksInFleet` 0x60b6d0: the `Tankok` entries held by f, design ≠ 0x7fff, whose
            // EN design has +0x36 = 0).
            Effect::BattleVehicles => {
                let n = self.state.mem.i32(va::VEHICLE_COUNT).max(0) as usize;
                let raw = self.state.mem.read(va::VEHICLES, 8 * (n + 1));
                let designs = self.state.designs.get(en).cloned().unwrap_or_default();
                let held = |f: i32| {
                    raw.chunks_exact(8).skip(1).any(|c| {
                        let (holder, d) = (i16::from_le_bytes([c[0], c[1]]), i16::from_le_bytes([c[2], c[3]]));
                        holder as i32 == f && d != 0x7fff && designs.get(d.max(0) as usize).is_some_and(|r| r[0x36] == 0)
                    })
                };
                let own: Vec<i32> = (1..self.state.fleets.len() as i32).filter(|&f| self.state.fleets[f as usize][0x15] as usize == en).collect();
                for f in own {
                    if !held(f) {
                        self.new_vehicles(f, 9, 2);
                    }
                }
            }
            // `GIVE KILLER FLEET e` (exe 0x610dc8 area, re/rules/tutorial.md §3 61): for EN "Terminator" beside EN's last
            // planet of 1..HanyBolygo−1 (f32 ±20.0; planet 0 with none) with 0x19 vehicles of design 2, three ships of
            // design 3 and one of 6, and every planet of empire 6 explored by EN; for another empire "Unbeatable" at
            // ((6 − e)·20, 300) with a hundred ships of design 0x2b.
            Effect::KillerFleet(e) => {
                let e = (*e).clamp(0, EMPIRES as i32 - 1) as usize;
                if e == en {
                    let (x, y) = self.tutorial_spot(en);
                    let f = self.new_fleet(b"Terminator", en as u8, 0, x + 20.0, y - 20.0, 0.0) as i32;
                    self.new_vehicles(f, 2, 0x19);
                    for d in [3, 3, 3, 6] {
                        self.new_ship(f, d);
                    }
                    for p in 1..=planets {
                        if self.state.mem.u8(planet(p) + 0x20) == 6 {
                            self.state.mem.write(planet(p) + 0xd8 + 2 * en as u32, &1i16.to_le_bytes());
                        }
                    }
                } else {
                    let f = self.new_fleet(b"Unbeatable", e as u8, 0, ((6 - e as i32) * 0x14) as f32, 300.0, 0.0) as i32;
                    for _ in 0..100 {
                        self.new_ship(f, 0x2b);
                    }
                }
            }
            // `GIVE KOLONIZALO FLEET` (§3 62): "Colonization" beside EN's last planet with one ship of design 4.
            Effect::ColonizationFleet => {
                let (x, y) = self.tutorial_spot(en);
                let f = self.new_fleet(b"Colonization", en as u8, 0, x + 20.0, y - 20.0, 0.0) as i32;
                self.new_ship(f, 4);
            }
            // `ADD DESTROYERS TO TRADE`: `DestoryerATradereTutorialhoz()` four times (exe 0x6057f0).
            Effect::DestroyersToTrade => {
                for _ in 0..4 {
                    self.destroyer_for_sale();
                }
            }
            // `DISABLE AI` / `ENABLE AI`: the debug AI-off flag (exe 0xB77244).
            Effect::AiDisabled(off) => self.state.mem.set_i32(va::AI_OFF, *off as i32),
            // `SEND MESSAGE n`: `NewMessage(n, 0)`.
            Effect::SendMessage(n) => self.new_message(*n, 0),
            // `IRRITATEME` (exe 0x60c570): `DiplomaciaTalkAbout(6, EN, 4, −1)`, empire 6's insult waits in EN's contact
            // list (the screen doesn't open). Unused by main.tut.
            Effect::Irritate => self.with_diplomacy(|d, w| d.talk_about(w, 6, en, crate::diplomacy::talk::INSULT, -1)),
            _ => return false,
        }
        true
    }
}

fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}
