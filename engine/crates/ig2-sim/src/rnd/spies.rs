//! `st_Rnd_include`, part 2a: a spy mission (127, spec `re/rules/campaign.md` §8.13).

use super::{message, param, read};
use crate::brnd::random_empire;
use crate::story::{flag, Abort, MessageText, Story, StoryEvent, StoryWorld};

/// The story spy missions: `KemKuldetesek[14 + k]` belongs to story `StoryKuldetes[k]` (12 of them).
const MISSIONS: i32 = 12;

impl Story {
    /// `st_EnableKemKuldetes(id)` (exe 0x5e6ac0) / `st_DisableKemKuldetes(id)` (0x5d4ab0): story `id`'s spy mission
    /// is offered (an undo entry `(5, id, StoryLastChecked)` withdraws it) or withdrawn; false when `id` has none.
    pub fn spy_mission(&mut self, id: i32, on: bool, w: &mut dyn StoryWorld) -> bool {
        w.event(StoryEvent::SpyMission { id, on });
        let found = (0..MISSIONS).any(|k| w.campaign_tables().story_mission(k) == id);
        if found && on {
            self.add_undo(5, id, self.last_checked);
        }
        found
    }
}

/// `st_KemSiker(id)`: spy news of kind 2 (a success) for story mission `id` (flag 15).
fn spy_success(s: &Story, id: i32) -> bool {
    s.flags[flag::SPY] && s.spy_kind == 2 && (id == 0 || s.spy_story == id)
}

/// 127: an empire (not 5) is to be spied on (message 0x4f7, its name). Read → story mission 1271 is offered for
/// 60 days (an info line). Its success clears the answer (`KemkedesValasz`). After 60 days the mission is withdrawn
/// and the human's spies on it at home (state 3) come back (state 1): none → the end; one of rank 2 or more → message
/// 0x4f8, it gains `rnd(800, 1200)` experience, the human meets the empire, sees three random planets of it and the
/// techs it has; one of a lower rank is lost (message 0x4f9, state 0); more than one → message 0x4fa.
pub(super) fn mission(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x4f7;
    let en = w.human();
    if s.starting_now() {
        let e = random_empire(w, 5, 0, 0)?;
        s.v_set(ID, e);
        message(w, ID, 0xa1);
        let e = s.v_get(ID);
        param(w, 1, MessageText::Empire(e));
    }
    if read(s, ID) {
        s.sz_start(ID, 60);
        s.spy_mission(ID, true, w);
        s.add_game_info(ID, 0, -1, w);
    }
    if spy_success(s, ID) {
        s.spy_answer = 0;
    }
    if s.counter(ID) {
        s.spy_mission(ID, false, w);
        let k = (0..MISSIONS).find(|&k| w.campaign_tables().story_mission(k) == ID).unwrap_or(MISSIONS);
        let mission = k + 14;
        let (mut n, mut last) = (0, 0);
        for slot in 1..=w.spy_count(en) {
            let spy = w.spy_record(en, slot);
            if spy.state == 3 && spy.mission == mission {
                n += 1;
                w.event(StoryEvent::SpyState { slot, state: 1 });
                last = slot;
            }
        }
        match n {
            0 => s.win(0, true, w),
            1 if w.spy_rank(en, last) < 2 => {
                message(w, 0x4f9, 0);
                let race = w.spy_record(en, last).race;
                w.event(StoryEvent::MessageRace(race));
                w.event(StoryEvent::SpyState { slot: last, state: 0 });
                s.win(0, true, w);
            }
            1 => {
                let e = s.v_get(ID);
                message(w, 0x4f8, 0);
                let race = w.spy_record(en, last).race;
                w.event(StoryEvent::MessageRace(race));
                w.event(StoryEvent::SpyState { slot: last, state: 1 });
                let add = super::rnd(w, 800, 1200);
                w.event(StoryEvent::SpyExperience { slot: last, add });
                if !w.knows(en, e) {
                    w.event(StoryEvent::DiscoverEmpire(e));
                }
                for _ in 0..3 {
                    let p = s.b_search(e.wrapping_neg(), 0, w);
                    if p != 0 {
                        w.event(StoryEvent::SetPlanetView { planet: p, view: 1 });
                    }
                }
                for t in 1..193 {
                    if w.tech_status(t, e) == 5 {
                        w.event(StoryEvent::TechSeen { tech: t, owner: e });
                    }
                }
                s.win(0, true, w);
            }
            _ => {
                message(w, 0x4fa, 0);
                s.win(0, true, w);
            }
        }
    }
    Ok(())
}
