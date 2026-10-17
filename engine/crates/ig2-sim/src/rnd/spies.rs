//! `st_Rnd_include`: a spy mission (127, part 2a), spy recruits (138) and a colony to spy out (174, part 2c; spec
//! `re/rules/campaign.md` §8.13).

use super::{answered, message, param, read};
use crate::brnd::random_empire;
use crate::rng::stream::SPY as SPY_STREAM;
use crate::story::{flag, Abort, KemNews, MessageText, Story, StoryEvent, StoryWorld};

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
pub(crate) fn spy_success(s: &Story, id: i32) -> bool {
    s.flags[flag::SPY] && s.spy_kind == 2 && (id == 0 || s.spy_story == id)
}

/// `st_KemElkap(id)` (exe 0x5eb4b0): spy news of kind 1 (is the spy caught?) for story mission `id` (flag 15).
pub(crate) fn spy_caught(s: &Story, id: i32) -> bool {
    s.flags[flag::SPY] && s.spy_kind == 1 && (id == 0 || s.spy_story == id)
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

/// `st_IsKemKuldetesEnabled(id)` (exe 0x5eb430): story `id`'s spy mission is offered (`KemKuldetesek[14 + k].+0`).
pub(crate) fn mission_enabled(id: i32, w: &dyn StoryWorld) -> bool {
    (0..MISSIONS).find(|&k| w.campaign_tables().story_mission(k) == id).is_some_and(|k| w.spy_mission_on(14 + k))
}

/// `st_KemEsely(kind, a, lo, hi, true)` (exe 0x5eb4e0): the last story mission's spy (`Kemek[KemkedesTulaj]
/// [KemkedesAktKem]`, skills `+0x31..+0x33`) succeeds: kinds 1 and 2 `100·s₁`, 3 `70·s₃ + 30·s₁`, 4 `70·s₂ + 30·s₃`,
/// else 0; `(x·50/a)/10` clamped to `hi`, then `lo`; `rnd(0, 99)` below it.
pub(crate) fn spy_chance(s: &Story, kind: i32, a: i32, lo: i32, hi: i32, w: &mut dyn StoryWorld) -> i32 {
    let spy = w.spy_record(s.spy_owner, s.spy_slot);
    let (s1, s2, s3) = (spy.skill_31, spy.skill.0, spy.skill.1);
    let x = match kind {
        1 | 2 => s1 * 100,
        3 => s3 * 70 + s1 * 30,
        4 => s2 * 70 + s3 * 30,
        _ => 0,
    };
    let c = (x.wrapping_mul(50) / a / 10).min(hi).max(lo);
    (super::rnd(w, 0, 99) < c) as i32
}

/// The human's live race-14 spies (slots 0..=`OsszHanyKem`): 138's recruits.
fn recruits(w: &dyn StoryWorld) -> i32 {
    let en = w.human();
    (0..=w.spy_count(en))
        .filter(|&k| {
            let spy = w.spy_record(en, k);
            spy.race == 14 && spy.state > 0
        })
        .count() as i32
}

fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// 138: a planet of spies (the nearest unknown of 11001..11015, id 1381, message 0x565, shown, hidden from the
/// other empires) offers up to two recruits to a fleet of the human's arriving there; the first visit names the
/// planet and always offers two. Hired: `Spy_New` with 138's record, one of seven names not in use (30 tries),
/// `rnd(4: XP[2], XP[3])` experience and a spy report (kind 2). The planet taken by anyone ends it.
pub(super) fn recruits_138(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x565;
    const OPEN: i32 = 0x566;
    const HOW_MANY: i32 = 0x567;
    const FIRST: i32 = 0x568;
    let en = w.human();
    if s.starting_now() {
        let p = s.nearest_unknown(w)?;
        s.b_add_id(p, ID);
        message(w, ID, 0);
        param(w, 1, MessageText::Planet(p));
        s.b_show(ID, -1, w);
        s.v_set(ID, 1);
        for e in 1..19 {
            if e != en {
                w.event(StoryEvent::SetPlanetViewOf { planet: p, empire: e, view: -1 });
            }
        }
        s.v_set(OPEN, 0);
    }
    if s.daily() {
        let p = s.b_search(ID, 0, w);
        if w.planet_owner(p) != 0 {
            s.b_unhide(ID, w);
            s.win(0, true, w);
        }
    }
    if s.arrived_planet(0, ID, w) && w.fleet_owner(s.arrival.fleet) == en && s.v_get(OPEN) == 0 && s.sz_get(ID) == 0 {
        let mut fresh = false;
        if s.v_get(ID) != 0 {
            s.v_set(ID, 0);
            s.v_set(FIRST, 1);
            let p = s.b_search(ID, 0, w);
            w.event(StoryEvent::RemovePlanetInfo { planet: p, info: ID.wrapping_neg() });
            let p = s.b_search(ID, 0, w);
            w.event(StoryEvent::NamePlanet { planet: p, text: ID });
            fresh = true;
        }
        let k = (2 - recruits(&*w)).max(0);
        if k == 2 || fresh {
            message(w, OPEN, 0);
            s.v_set(OPEN, 1);
        } else if k == 1 {
            message(w, HOW_MANY, 0);
            s.v_set(OPEN, 1);
        } else {
            message(w, FIRST, 0);
            s.sz_start(ID, 7);
        }
        s.v_set(HOW_MANY, k);
    }
    if answered(s, OPEN, 3) || answered(s, HOW_MANY, 3) {
        s.v_set(OPEN, 0);
    }
    if answered(s, OPEN, 2) || answered(s, HOW_MANY, 2) {
        s.v_set(OPEN, 0);
        let mut k = 2 - recruits(&*w).min(2);
        if s.v_get(HOW_MANY) < k {
            k = s.v_get(HOW_MANY);
        }
        if s.v_get(FIRST) != 0 {
            s.v_set(FIRST, 0);
            k = 2;
        }
        for _ in 0..k {
            let mut name;
            let mut tries = 30;
            loop {
                name = super::rnd(w, 0, 6);
                let want = w.recruit_name(name);
                let taken = (0..=w.spy_count(en)).any(|j| {
                    let spy = w.spy_record(en, j);
                    spy.race == 14 && spy.state > 0 && cstr(&spy.name) == want.as_slice()
                });
                tries -= 1;
                if tries == 0 || !taken {
                    break;
                }
            }
            let slot = w.new_spy(en);
            if slot > 0 {
                // The record's rank (+0x40) is 2.
                let (lo, hi) = (w.spy_rank_xp(2), w.spy_rank_xp(3));
                let xp = w.rng().range(SPY_STREAM, lo, hi);
                w.event(StoryEvent::StoryRecruit { slot, name, xp });
                // KemJelentesAdd(2, slot, 0, …) under Story_check_locked: its Story_KemEsemeny sets the spy flag for
                // the rest of this check, its Story_Check returns at once.
                s.kem = KemNews { news: 2, spy: slot, ..KemNews::default() };
                if !w.multiplayer() {
                    s.flags[flag::SPY] = true;
                    s.spy_kind = 3;
                }
                w.event(StoryEvent::SpyReport { kind: 2, spy: slot });
            }
        }
    }
    Ok(())
}

/// 174: a planet of the human's (id 1741, message 0x6cd) is to be spied out: its story mission is offered; after 150
/// days (counter 1744) the planet goes to empire 0 (messages 0x6d1, then 0x6d2 90 days later). A success with
/// `st_KemEsely(4, 50, 20, 100)` ends it (0x6d0), so does losing the planet (0x6ce) or winning a ground battle on it
/// (0x6d3). The source's slip: `st_BolygoMoralValt(0x6cd, −4)` changes planet slot 1741.
pub(super) fn colony_174(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x6cd;
    let en = w.human();
    if s.starting_now() {
        let p = s.b_search(en.wrapping_neg(), 0, w);
        s.b_add_id(p, ID);
        message(w, ID, 0);
        w.event(StoryEvent::PlanetMorale { planet: ID, delta: 40 });
        s.spy_mission(ID, true, w);
        s.sz_start(ID, 30);
        s.sz_start(0x6ce, 60);
        s.sz_start(0x6cf, 90);
        s.sz_start(0x6d0, 150);
    }
    let p = s.b_search(ID, 0, w);
    if w.planet_owner(p) != en {
        message(w, 0x6ce, 0);
        s.win(0, true, w);
    }
    if spy_success(s, ID) {
        s.spy_answer = if mission_enabled(ID, &*w) { spy_chance(s, 4, 50, 20, 100, w) } else { 0 };
        if s.spy_answer != 0 {
            s.spy_mission(ID, false, w);
            message(w, 0x6d0, 0);
            s.win(0, true, w);
        }
    }
    if s.counter(0x6d0) {
        let p = s.b_search(ID, 0, w);
        s.set_planet_owner(p, 0, w);
        message(w, 0x6d1, 0);
        s.sz_start(0x6d1, 90);
    }
    if s.counter(0x6d1) {
        message(w, 0x6d2, 0);
        s.win(0, true, w);
    }
    if s.ground_part(0, ID, w) && w.fleet_owner(s.ground.fleet) == en && s.ground.attacker_won {
        message(w, 0x6d3, 0);
        s.win(0, true, w);
    }
    Ok(())
}
