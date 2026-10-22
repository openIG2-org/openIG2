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
        m.set_i32(va::TUTORIAL_EVENT, 0);
        if m.u8(va::TUTORIAL_PAUSED) != 0 {
            m.set_u8(va::TUTORIAL_PAUSED, 0);
            m.set_u8(va::TUTORIAL_ACTIVE, 1);
        }
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
            _ => return false,
        }
        true
    }
}

fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}
