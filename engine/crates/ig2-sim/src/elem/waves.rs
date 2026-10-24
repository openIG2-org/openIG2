//! `st_Elem_include`, part 2f: 836 (empire 1's three waves against the human's strongest planet), 839 (empire 10's
//! hunter fleet round three free planets), 840 (the colonisation race) and 843 (the raiders' fly-past). Spec:
//! `re/rules/campaign/elem.md` §8.14, part 2f.

use super::message;
use crate::begin::{delete_fleet, move_to_planet, planet_distance};
use crate::brnd::nearest_fleet;
use crate::elem::spies::empire_distance;
use crate::rnd::cvtt;
use crate::story::{flag, last_fn, Abort, InfoLine, Story, StoryEvent, StoryWorld, STREAM};

fn move_fleet(w: &mut dyn StoryWorld, fleet: i32, kind: i32, mode: i32, x: f32, y: f32) {
    w.event(StoryEvent::MoveFleet { fleet, kind, mode, x, y });
}

/// `st_fGetNearest((int) fx, (int) fy, EN)` from fleet `f`'s position.
fn nearest_to(f: i32, w: &dyn StoryWorld) -> i32 {
    let (fx, fy) = w.fleet_xy(f);
    nearest_fleet(cvtt(fx as f64), cvtt(fy as f64), w.human(), w)
}

// ---------------------------------------------------------------- 836: empire 1's three waves

/// 836 (exe block 0x5def0a): three fleets of empire 1 attack planet 8351 (the human's strongest when it isn't the
/// human's); a won landing there leads, 30 days on, to every empire allied against the human.
pub(super) fn story_836(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        s.sz_start(8360, 600);
        let p = s.b_search(8351, 1, w);
        if p == 0 || w.planet_owner(p) != en {
            let p = s.strongest_planet(&*w);
            s.b_remove_id(8351);
            s.b_add_id(p, 8351);
        }
        if s.b_search(-1, 0, w) == 0 {
            s.win(0, true, w);
        } else {
            let b = s.b_search(-1, 0, w);
            s.f_new_ex(crate::story::FleetName::Text(8360), 1, 8360, 0.09, 2, b, w);
            s.attack_planet(8360, 8351, w);
        }
        let p = s.b_search(8351, 0, w);
        s.add_info_line(8361, InfoLine::Planet { planet: p, count: None }, -1, w);
    }
    if s.ground_part(0, 8351, w) && s.ground.attacker_won {
        // `st_RemoveFlottaAzonosito` (inlined) of 8350..=8354, not of the story's own fleets 8360..=8362.
        for id in 8350..8355 {
            if s.f_search(id, w) != 0 {
                s.remove_fleet_id(id, w);
            }
        }
        message(w, 8361);
        s.sz_start(8361, 30);
    }
    if s.counter(8361) {
        message(w, 8362);
        for i in 1..=8 {
            for j in 1..=8 {
                if i != j {
                    w.event(StoryEvent::MakeAllies { a: i, b: j });
                }
            }
        }
        for k in (1..=8).filter(|&k| k != en) {
            w.event(StoryEvent::DeclareWar { a: en, b: k });
        }
        s.win(0, true, w);
    }
    if s.counter(8360) {
        s.sz_start(8363, 400);
        if s.b_search(-1, 0, w) == 0 {
            s.win(0, true, w);
        } else {
            let b = s.b_search(-1, 0, w);
            s.f_new_ex(crate::story::FleetName::Text(8361), 1, 8361, 0.09, 2, b, w);
            s.attack_planet(8361, 8351, w);
        }
    }
    if s.counter(8363) {
        s.sz_start(8364, 1000);
        let b = s.b_search(-1, 0, w);
        if b == 0 {
            s.win(0, true, w);
        } else {
            s.f_new_ex(crate::story::FleetName::Text(8362), 1, 8362, 0.09, 2, b, w);
            s.attack_planet(8362, 8351, w);
        }
    }
    if s.counter(8364) {
        s.win(0, true, w);
    }
    if s.daily() {
        for id in 8360..8365 {
            let f = s.f_search(id, w);
            if f == 0 {
                continue;
            }
            if w.fleet_5c(f) > 0 {
                // `st_fAttackPlanet(−f, ·)` searches `−f` as an empire: a random fleet of empire `f` (stream 5).
                if s.fleet_idle(f.wrapping_neg(), w) {
                    s.attack_planet(f.wrapping_neg(), 8351, w);
                }
            } else {
                let g = nearest_to(f, &*w);
                if g != 0 {
                    move_fleet(w, f, 2, 2, g as f32, 0.0);
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 839: the hunter fleet

/// `StoryInit839_GenerateBolygok()` (exe 0x5c4df0): planets 8391..=8393 shown when all three are there, else three
/// free planets close to each other and to the human's.
fn init_839(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let ids = [8391, 8392, 8393].map(|id| s.b_search(id, 0, w));
    if ids.iter().all(|&p| p != 0) {
        for p in ids {
            w.event(StoryEvent::UnhidePlanet(p));
        }
        return Ok(());
    }
    let en = w.human();
    let (mut best, mut pick) = (1000, [0; 3]);
    for _ in 0..30 {
        let r = s.b_search_try(0, 0, w)?;
        let free = |q: i32, w: &dyn StoryWorld| w.planet_owner(q) == 0 && w.planet_visibility(q) >= 0;
        let nearest = |not: i32, w: &dyn StoryWorld| {
            let (mut d, mut q) = (0, 0);
            for p in 1..=w.planet_count() {
                if p != not && p != r && free(p, w) {
                    let e = planet_distance(w.planet_xy(r), w.planet_xy(p));
                    if d == 0 || e < d {
                        (q, d) = (p, e);
                    }
                }
            }
            q
        };
        let q1 = nearest(r, &*w);
        let q2 = nearest(q1, &*w);
        let sum = empire_distance(q2, en, &*w)
            .wrapping_add(empire_distance(q1, en, &*w))
            .wrapping_add(empire_distance(r, en, &*w));
        if sum < best {
            best = sum;
            pick = [r, q1, q2];
        }
    }
    for (p, id) in pick.into_iter().zip([8391, 8392, 8393]) {
        s.b_add_id(p, id);
    }
    Ok(())
}

/// 839 (exe block 0x5df708): empire 10's 1000-ship fleet 8390 cruises between planets 8391..=8393 and moves on when
/// one of them is taken; after 600 days it hunts the human's nearest fleet.
pub(super) fn story_839(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        init_839(s, w)?;
        let b = s.b_search(8391, 0, w);
        s.f_new_ex(crate::story::FleetName::Text(8390), 10, 8390, 0.08, 2, b, w);
        s.f_add_ship(8390, 3, 1000, w);
        move_to_planet(s, 8390, 8392, w);
        s.sz_start(8399, 600);
        s.sz_start(8398, 10);
        message(w, 8390);
    }
    if super::read(s, 8390) {
        s.add_info_line(8390, InfoLine::Blank, -1, w);
    }
    let mut moved = false;
    if s.counter(8398) || s.special_is(10, en, 0, 0) {
        let taken = (0..3).any(|k| {
            let p = s.b_search(8391 + k, 0, w);
            w.planet_owner(p) != 0
        });
        if taken {
            for id in 8391..=8393 {
                s.b_remove_id(id);
            }
            init_839(s, w)?;
            if s.v_get(8399) == 0 {
                s.sz_stop(8391);
                s.sz_stop(8392);
                moved = true;
            }
        }
        s.sz_start(8398, 5);
    }
    if s.arrived_planet(8390, 0, w) || moved {
        match w.rng().range(STREAM, 1, if moved { 3 } else { 5 }) {
            r @ 1..=3 => {
                move_to_planet(s, 8390, 8390 + r, w);
            }
            4 => s.sz_start(8391, 20),
            5 => s.sz_start(8392, 30),
            _ => {}
        }
    }
    if s.counter(8391) {
        s.f_add_ship(8390, 3, 1, w);
        for _ in 0..3 {
            s.f_add_ship(8390, 2, 1, w);
        }
        let b = w.rng().range(STREAM, 8391, 8393);
        move_to_planet(s, 8390, b, w);
    }
    if s.counter(8392) {
        let b = w.rng().range(STREAM, 8391, 8393);
        move_to_planet(s, 8390, b, w);
    }
    if s.battle_part(8390) != 0 && s.f_search(8390, w) == 0 {
        let x = w.rng().range(STREAM, 25, 32).wrapping_mul(1000);
        super::add_money(w, x, -1);
        message(w, 8391);
        w.event(StoryEvent::MessageParamInt { slot: 1, value: x });
        s.win(0, true, w);
    }
    if s.counter(8399) {
        s.v_set(8399, 1);
        let f = s.f_search(8390, w);
        if f == 0 {
            s.win(0, true, w);
        } else {
            let g = nearest_to(f, &*w);
            if g == 0 {
                s.send_away(f.wrapping_neg(), w);
                s.win(0, true, w);
            } else {
                move_fleet(w, f, 2, 2, g as f32, 0.0);
            }
        }
        s.sz_start(8399, 5);
    }
    Ok(())
}

// ---------------------------------------------------------------- 840: the colonisation race

/// `[4, 3, 2, 2][Nehezseg − 1]` (exe 0x85e500): the years 840 gives.
const YEARS: [i32; 4] = [4, 3, 2, 2];
/// The colonisations 840 wants.
const GOAL: i32 = 5;

/// 840 (exe block 0x5dfc10): colony ships cost half for a while; colonise five planets in time, else empire 17 takes
/// the human's two poorest planets and must be driven out.
pub(super) fn story_840(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    let years = YEARS[(w.difficulty() - 1).clamp(0, 3) as usize];
    if s.starting_now() {
        s.sz_start(8400, years * 360);
        s.v_set(8400, 0);
        s.add_game_info(8401, GOAL, -1, w);
        s.sz_start(8407, 30);
        message(w, 8400);
        w.event(StoryEvent::MessageParamInt { slot: 1, value: years });
    }
    if s.counter(8407) {
        w.event(StoryEvent::ColonyDiscount { halve: true });
        s.add_undo(11, 0, 840);
        message(w, 8402);
    }
    if s.v_get(8401) == 0 && s.special_is(10, en, 0, 0) {
        s.v_inc(8400);
        let n = s.v_get(8400);
        s.remove_game_info(if n == 1 { 8401 } else { 8402 }, w);
        if n < GOAL {
            s.add_game_info(8402, GOAL - n, -1, w);
        }
        if n == GOAL {
            s.sz_start(8401, 1);
            s.v_set(8401, 1);
        }
    }
    if s.counter(8401) {
        s.win(0, true, w);
    }
    if s.counter(8400) {
        s.sz_stop(8400);
        s.sz_stop(8401);
        s.v_set(8401, 1);
        let seen = |p: i32, w: &dyn StoryWorld| w.planet_owner(p) == en && w.planet_visibility(p) >= 0;
        let n = (1..=w.planet_count()).filter(|&p| seen(p, &*w)).count();
        if n < 3 {
            message(w, 8409);
            s.start_story(406, false);
        } else {
            // The ELF's `st_Counter(8401)` here is false (the counter that ended is 8400): always `M(8403)`.
            message(w, 8403);
            s.sz_start(8402, 30);
            s.sz_start(8409, 900);
            // The raw `RemoveGameInfo` (exe 0x540e50): the undo entry stays.
            let info = if s.v_get(8400) == 0 { 8401 } else { 8402 };
            w.event(StoryEvent::RemoveGameInfo(info));
            let a = w.story_design(17, 8401, 1);
            let b = w.story_design(17, 8402, 1);
            let difficulty = w.difficulty();
            for _ in 0..2 {
                // The human's poorest planet (`st_BolygoErtek` = `+0x28`; the first of equals).
                let mut p = 0;
                for q in 1..=w.planet_count() {
                    if seen(q, &*w) && (p == 0 || w.planet_value(p) > w.planet_value(q)) {
                        p = q;
                    }
                }
                s.set_planet_owner(p, 17, w);
                let n1 = Story::diff(difficulty, 1, 2, 2, 2);
                let n2 = Story::diff(difficulty, 0, 0, 1, 2);
                if n1 != 0 {
                    w.event(StoryEvent::NewTanks { fleet: p.wrapping_neg(), design: a, count: n1 });
                }
                if n2 != 0 {
                    w.event(StoryEvent::NewTanks { fleet: p.wrapping_neg(), design: b, count: n2 });
                }
            }
            s.v_set(8409, 1);
            s.add_info_line(8403, InfoLine::Blank, -1, w);
            // The discount's undo entry dropped (its type zeroed) and, when there was one, undone now.
            let mut found = false;
            for e in s.undo.iter_mut().filter(|e| e.0 == 11 && e.2 == 840) {
                e.0 = 0;
                found = true;
            }
            if found {
                w.event(StoryEvent::ColonyDiscount { halve: false });
            }
        }
    }
    if s.counter(8409) {
        message(w, 8409);
        s.start_story(406, false);
        s.win(0, true, w);
    }
    if s.counter(8402) {
        message(w, 8404);
    }
    let check = (s.ground_part(0, 0, w) && s.ground.attacker_won) || s.daily();
    if check && s.b_search(-17, 1, w) == 0 && s.v_get(8409) == 1 {
        s.v_set(8409, 2);
        message(w, 8405);
        s.sz_start(8408, 20);
        let f = s.f_new_ex(crate::story::FleetName::Text(8401), en, 0, 0.06, 1, 0, w);
        s.auto_add_ship(f, 8401, w);
        // To the point (planet index, 0): `last_fn_b` passed as an x coordinate.
        move_fleet(w, f, 1, 0, s.last_fn[last_fn::B] as f32, 0.0);
        s.remove_game_info(8403, w);
    }
    if s.counter(8408) {
        message(w, 8408);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 843: the raiders' fly-past

/// The raiders: fleet id and name text.
const RAIDERS: [(i32, i32); 3] = [(8430, 8340), (8431, 8341), (8432, 8342)];

/// One raider fleet of empire 10 next to planet 8044, sent 300 further out on the bearing it was placed on.
fn raider(s: &mut Story, id: i32, text: i32, w: &mut dyn StoryWorld) {
    let p = s.b_search(8044, 0, w);
    s.f_new_ex(crate::story::FleetName::Text(text), 10, id, 0.04, p.wrapping_neg(), 5, w);
    s.f_add_ship(id, 2, 5, w);
    s.f_add_ship(id, 3, 2, w);
    let f = s.f_search(id, w);
    // `(q · 3.141592) / 180.0` in f64, as the exe computes it (its constant, not π).
    #[allow(clippy::approx_constant)]
    let a = s.last_fn[last_fn::Q] as f64 * 3.141592 / 180.0;
    let (bx, by) = w.planet_xy(s.last_fn[last_fn::B]);
    let x = (a.cos() * 300.0 + bx as f64) as f32;
    let y = (a.sin() * 300.0 + by as f64) as f32;
    move_fleet(w, f, 1, 0, x, y);
}

/// 843 (exe block 0x5e079f): three raider fleets fly past planet 8044 and vanish when they reach their point.
pub(super) fn story_843(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.add_info_line(8431, InfoLine::Blank, -1, w);
        let (id, text) = RAIDERS[0];
        raider(s, id, text, w);
        s.sz_start(8435, 30);
        s.sz_start(8430, 300);
    }
    if s.counter(8435) {
        s.sz_start(8436, 20);
        let (id, text) = RAIDERS[1];
        raider(s, id, text, w);
    }
    if s.counter(8436) {
        let (id, text) = RAIDERS[2];
        raider(s, id, text, w);
    }
    if s.counter(8430) {
        message(w, 8431);
        s.win(0, true, w);
    }
    if s.flags[flag::SPACE_END] && RAIDERS.iter().all(|&(id, _)| s.f_search(id, w) == 0) {
        s.win(0, true, w);
    }
    for (id, _) in RAIDERS {
        if s.arrived_point(id) {
            delete_fleet(s, id, w);
        }
    }
    Ok(())
}
