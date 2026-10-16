//! An empire's death and the victory check (`re/rules/diplomacy.md` §12): `BirodalomMeghal` (exe 0x53b630, ELF
//! 0x61e720 with `MEGHALAS` 0x61e800 inlined) and `MegnyeresEllenoriz` (exe 0x53f8c0).

use super::{adat_i32, va, Game, GameEvent};
use crate::colony_side::EMPIRES;
use crate::diplomacy::Relation;
use crate::story::StoryEvent;

/// `MegnyeresEllenoriz`'s story: the last enemy is much weaker (`StartStory(0x195, false)`).
pub const LAST_ENEMY_STORY: i32 = 0x195;

/// What the death and the victory check hand to the UI (`ExitToMainScreen`, the end animations, `AktKepKell =
/// 0xF`): the sim goes on, the front end ends the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameOver {
    /// `MEGHALAS`: the human's empire died (`misc/end_gameover.bik` after the race's or the generic animation).
    Lost,
    /// `MEGNYERES(true)`: no enemy left (`misc/end_victory.bik`, or `misc/end_campaign_win.bik` in a campaign).
    Won,
}

impl Game<'_> {
    /// The deaths `FRISSIT_Viszony` reported since event `from`, carried out in order (each inside the exe's
    /// death check, so an empire dying later the same day is still alive to the earlier one's victory check).
    pub(super) fn empire_deaths(&mut self, from: usize) {
        let dead: Vec<u8> = self.events[from..]
            .iter()
            .filter_map(|e| match e {
                GameEvent::Diplomacy(crate::diplomacy::DiploEvent::EmpireDied { empire }) => Some(*empire),
                _ => None,
            })
            .collect();
        for (i, &e) in dead.iter().enumerate() {
            self.empire_dies(e, &dead[i + 1..]);
        }
    }

    /// The deaths a story asked for (`DIE()` = `BirodalomMeghal(EN)`), after the story call.
    pub(super) fn story_deaths(&mut self, events: &[StoryEvent]) {
        for e in events {
            if let StoryEvent::EmpireDies(x) = *e {
                if (0..EMPIRES as i32).contains(&x) {
                    self.empire_dies(x as u8, &[]);
                }
            }
        }
    }

    /// `BirodalomMeghal(e)`: the scrolling line's "empire e has died" (id 3, `e < 9`, `e ≠ EN`) is posted from the
    /// event at the tick's end ([`Game::post_messages`]); every fleet of `e` is deleted (`FlottaDelete`, slots
    /// 1..=count in order); `BirodalomMeghalDiplomacia(e)` resets the diplomacy screen if it shows `e` (UI);
    /// `BirodalomEl[e] = 0`; the human's death ends the game ([`GameOver::Lost`]); then `MegnyeresEllenoriz`.
    /// `later`: empires that die after `e` in the same pass and are still alive here.
    pub fn empire_dies(&mut self, e: u8, later: &[u8]) {
        self.with_ai(|_, w, _| {
            for f in 1..w.starmap.fleets.len() {
                if w.starmap.fleets[f].owner == e {
                    w.delete_fleet(f);
                }
            }
        });
        self.state.mem.set_u8(va::EMPIRE_ALIVE + e as u32, 0);
        if e as usize == self.globals().en {
            self.events.push(GameEvent::GameOver(GameOver::Lost));
        }
        self.victory_check(later);
    }

    /// `MegnyeresEllenoriz` (every tick after `FRISSIT_Starmap`, and at the end of every death): no other empire
    /// alive, the human alive and the screen below 0xE → won. SP campaign only: no living non-ally → won; one
    /// empire left, not allied, `StoryAdat+0xc8 == 0`, and the human more than twice its power
    /// (`BirodalomEro`) and planets (`BirodalomHanyBolygo`) → `StoryAdat+0xcc = it`, story 0x195 starts.
    /// `GameStarted` (the front end's "a game runs") is taken as set.
    pub(super) fn victory_check(&mut self, later: &[u8]) {
        let g = self.globals();
        let m = &self.state.mem;
        let alive = |e: usize| g.alive[e] || later.contains(&(e as u8));
        let level = |e: usize| m.u8(va::RELATIONS + ((g.en * EMPIRES + e) * Relation::SIZE) as u32 + 0x25);
        let others: Vec<usize> = (1..=g.pp.min(EMPIRES - 1)).filter(|&e| alive(e) && e != g.en).collect();
        let enemies = others.iter().filter(|&&e| level(e) != 5).count();
        let human = alive(g.en);
        let campaign = m.i32(va::CAMPAIGN_TYPE) > 0;
        if others.is_empty() && human && m.i32(va::AKT_KEP) < 0xe {
            self.events.push(GameEvent::GameOver(GameOver::Won));
        }
        if g.multiplayer || !campaign {
            return;
        }
        if human && enemies == 0 {
            self.events.push(GameEvent::GameOver(GameOver::Won));
        }
        if adat_i32(&self.adat(), 0xc8) == 0 && human && others.len() == 1 && enemies > 0 {
            let last = others[0];
            let power: [f32; EMPIRES] = m.f32s(va::EMPIRE_POWER);
            let planets: [i32; EMPIRES] = m.i32s(va::EMPIRE_PLANETS);
            if power[last] * 2.0 < power[g.en] && planets[last] * 2 < planets[g.en] {
                self.state.mem.set_i32(va::STORY_ADAT + 0xcc, last as i32);
                self.with_story(|st, _| {
                    st.start_story(LAST_ENEMY_STORY, false);
                });
            }
        }
    }
}
