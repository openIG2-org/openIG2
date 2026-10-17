//! `st_Elem_include`, part 2b: the ground-war episodes 808 (an empire's planet to take: its defenders go mad when
//! the human lands, the buildings left decide the empire's thanks) and 854 (riots: the human's vehicles on a
//! crowded planet go over to empire 10 and the rest may go mad). Spec: `re/rules/campaign.md` §8.14, part 2b.

use super::{param, read};
use crate::multi::nearest_planet;
use crate::story::{flag, Abort, InfoLine, MessageText, Story, StoryEvent, StoryWorld, STREAM};

fn rnd(w: &mut dyn StoryWorld, lo: i32, hi: i32) -> i32 {
    w.rng().range(STREAM, lo, hi)
}

fn message(w: &mut dyn StoryWorld, id: i32) {
    w.event(StoryEvent::NewMessage { id, arg: 0 });
}

/// `NewMessage(id, 0)`, `SetMessageFajOverride(e)`, parameter 1 the empire's name (808's messages).
fn empire_message(w: &mut dyn StoryWorld, id: i32, e: i32) {
    message(w, id);
    w.event(StoryEvent::MessageRace(e));
    param(w, 1, MessageText::Empire(e));
}

// ---------------------------------------------------------------- 808

/// 808's building sums over the slots 1..=`HanyEpuletLefoglalva[p]`: `(total, special)`; the kinds 0xe..=0x10 add
/// their hit points (`EpuletekLeiro[kind].+8`) to both, the others their hit points less their damage (`+4`).
/// `purge` (the battle's start) deletes the kinds 0x1c..=0x1f instead of counting them.
fn building_sums(p: i32, purge: bool, w: &mut dyn StoryWorld) -> (i32, i32) {
    let (mut total, mut special) = (0i32, 0i32);
    let mut k = 1;
    while k <= w.building_count(p) {
        let (kind, damage, _) = w.building(p, k);
        if kind != 0 {
            if purge && (0x1c..=0x1f).contains(&kind) {
                w.event(StoryEvent::DeleteBuilding { planet: p, slot: k });
            } else {
                let hp = w.building_hp(kind);
                if (0xe..=0x10).contains(&kind) {
                    special = special.wrapping_add(hp);
                    total = total.wrapping_add(hp);
                } else {
                    total = total.wrapping_add(hp.wrapping_sub(damage));
                }
            }
        }
        k += 1;
    }
    (total, special)
}

/// 808: planet 8080 of empire `v(5208)` (a random one of its planets unless the id is given) is shown and marked,
/// its garrison replaced by the story's (`AutoAddtank`), the empire protected; daily its buildings 0x20..=0x23 go
/// while it isn't the human's. When the human's ground battle there starts, the buildings 0x1c..=0x1f go, the others
/// are summed, 10 more vehicles land after 153 days (`v(8080)`) and the defenders go mad by a coin each; when it
/// ends the planet is the empire's again, and what was left of the buildings brings thanks (relation +25, peace, 3
/// story vehicles for the human's nearest planet) or reproach (−2). After 682 days without a battle: −16.
pub(super) fn story_808(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const P: i32 = 8080;
    let en = w.human();
    if s.starting_now() {
        let q = s.b_search(P, 0, w);
        let e = s.v_get(5208);
        if q == 0 {
            let r = s.b_search_try(e.wrapping_neg(), 0, w)?;
            s.b_add_id(r, P);
        }
        s.b_show(P, -1, w);
        s.b_set_extra(P, true, w);
        s.sz_start(P, 153);
        s.sz_start(8081, 682);
        let p = s.b_search(P, 0, w);
        s.add_info_line(8081, InfoLine::EmpirePlanet { empire: e, planet: p }, -1, w);
        s.protect_empire(e);
        // The garrison of the planet the first search found (none: the records of place 0).
        let mut k = 1;
        while k <= w.tank_count() {
            let (place, design, count) = w.tank(k);
            if place == q.wrapping_neg() && design != 0x7fff {
                w.event(StoryEvent::DeleteTanks { place, design, count });
            }
            k += 1;
        }
        s.auto_add_tank(P, w);
    }
    if s.daily() {
        let p = s.b_search(P, 0, w);
        if p != 0 && w.planet_owner(p) != en {
            let mut k = 1;
            while k <= w.building_count(p) {
                if (0x20..=0x23).contains(&w.building(p, k).0) {
                    w.event(StoryEvent::DeleteBuilding { planet: p, slot: k });
                }
                k += 1;
            }
        }
    }
    if s.counter(P) {
        s.v_set(P, 1);
        if s.v_get(6400) == 1 {
            s.sz_start(8088, 20);
        }
    }
    if s.ground_start(0, P, w) {
        let p = s.b_search(P, 0, w);
        s.set_adat_i32(0x88, 1);
        s.set_adat_i32(0x40, p);
        let (total, special) = building_sums(p, true, w);
        s.v_set(8083, total);
        s.v_set(8084, special);
        if s.v_get(P) != 0 {
            w.event(StoryEvent::NewTanks { fleet: p.wrapping_neg(), design: 1, count: 10 });
        }
        for (k, (alive, owner)) in w.battlefield_units(p).into_iter().enumerate() {
            if alive && owner != en && rnd(w, 0, 1) != 0 {
                w.event(StoryEvent::MadUnit { planet: p, unit: k as i32 });
            }
        }
    }
    if s.ground_part(0, P, w) {
        let p = s.b_search(P, 0, w);
        for (k, (alive, _)) in w.battlefield_units(p).into_iter().enumerate() {
            if alive {
                w.event(StoryEvent::CalmUnit { planet: p, unit: k as i32 });
            }
        }
    }
    if s.ground_part(0, P, w) {
        s.set_adat_i32(0x40, 0);
    }
    let mut result = 0;
    if s.ground_part(0, P, w) {
        s.remove_game_info(8081, w);
        let p = s.b_search(P, 0, w);
        let e = s.v_get(5208);
        s.set_planet_owner_race(p, e, -1, w);
        s.set_adat_i32(0x40, 0);
        let (total, special) = building_sums(p, false, w);
        let (was, was_special) = (s.v_get(8083), s.v_get(8084));
        if special < 1 {
            let bar = was.wrapping_sub(was_special.wrapping_mul(2)).wrapping_mul(4) / 5;
            result = if total < 1 || total < bar { 2 } else { 1 };
        }
    }
    if s.counter(8081) {
        let e = s.v_get(5208);
        w.event(StoryEvent::RelationChange { a: en, b: e, delta: -1600 });
        if s.v_get(8089) != 0 {
            message(w, 8083);
            let e = s.v_get(5208);
            w.event(StoryEvent::MessageRace(e));
            let e = s.v_get(5208);
            param(w, 1, MessageText::Empire(e));
        }
        s.win(0, true, w);
    }
    if result != 0 {
        let e = s.v_get(5208);
        if result == 2 {
            w.event(StoryEvent::RelationChange { a: en, b: e, delta: -200 });
            empire_message(w, 8082, e);
            s.v_set(8530, 2);
            s.v_set(8082, 0);
        } else {
            w.event(StoryEvent::RelationChange { a: en, b: e, delta: 2500 });
            // `st_SetSemlegesViszony` inlined: `BirodalomBeketKot(EN, e)` and the levels 3 both ways.
            if w.relation_level(en, e) < 3 {
                w.event(StoryEvent::MakeNeutral { a: en, b: e });
            }
            empire_message(w, 8081, e);
            let b = s.b_search(P, 0, w);
            let (x, y) = w.planet_xy(b);
            let q = nearest_planet(x as i32, y as i32, en, &*w);
            if q != 0 {
                param(w, 2, MessageText::Planet(q));
                let d = w.story_design(w.planet_owner(q), 802, 3);
                w.event(StoryEvent::NewTanks { fleet: q.wrapping_neg(), design: d, count: 3 });
            } else {
                param(w, 2, MessageText::Literal("--"));
            }
            s.v_set(8530, 3);
            s.v_set(8082, 1);
        }
        s.sz_stop(8081);
        s.win(0, true, w);
    }
    if s.counter(8088) {
        message(w, 8088);
    }
    Ok(())
}

// ---------------------------------------------------------------- 854

/// 854's guards of planet `p`: intact (`+8 == 0`), working (`+0x19` bit 0) buildings of kind 6 (police) and 7.
fn guards(p: i32, w: &dyn StoryWorld) -> (i32, i32) {
    let (mut police, mut seven) = (0, 0);
    for k in 1..=w.building_count(p) {
        let (kind, _, hit) = w.building(p, k);
        if hit != 0 || w.building_bytes(p, k).1 & 1 == 0 {
            continue;
        }
        match kind {
            6 => police += 1,
            7 => seven += 1,
            _ => {}
        }
    }
    (police, seven)
}

/// 854's dummy fleet: `st_fNewEx("dummy", 10, 8549, 0.06, 3, 0)` with one ship of design 3, hidden (`+0x18 |= 1`).
fn dummy_fleet(s: &mut Story, w: &mut dyn StoryWorld) -> i32 {
    let f = s.f_new_ex("dummy", 10, 8549, 0.06, 3, 0, w);
    s.f_add_ship(8549, 3, 1, w);
    w.event(StoryEvent::FleetState { fleet: f, set: 1, clear: 0 });
    f
}

/// 854's riot on planet `p`: up to 2 vehicles (1 with fewer than 7 in the garrison) of the human's units go over to
/// empire 10, one unit at a time picked by reservoir draws (`rnd(5, 1, n) ≤ 1` for the `n`-th candidate: used, the
/// human's, not `+0x188 & 0x4020000`, fitting); each unit's design becomes a copy of it for empire 10 (one per
/// design). Some unit taken → the planet gets id 8540, is shown, the video once, the colony screen held and paused.
/// Returns the planet when something rioted, else 0.
fn riot(s: &mut Story, p: i32, w: &mut dyn StoryWorld) -> i32 {
    let en = w.human();
    let mut f = s.f_search(8549, w);
    if f == 0 {
        f = dummy_fleet(s, w);
    }
    let max = if w.planet_garrison(p) < 7 { 1 } else { 2 };
    let (mut taken, mut any, mut more) = (0, false, true);
    // The local `StoryVodorClass`: a human's design → empire 10's copy (`GetDual`: the first entry's value).
    let mut copies: Vec<(i32, i32)> = Vec::new();
    while taken < max && more {
        more = false;
        let units = w.battlefield_records(p);
        let (mut pick, mut n) = (-1i32, 0);
        for (k, u) in units.iter().enumerate() {
            if u.used() && u.owner == en && u.flags & 0x0402_0000 == 0 && u.count.wrapping_add(taken) <= max {
                n += 1;
                if rnd(w, 1, n) <= 1 {
                    pick = k as i32;
                }
            }
        }
        if pick < 0 {
            continue;
        }
        let u = units[pick as usize];
        any = true;
        more = true;
        taken = taken.wrapping_add(u.count);
        w.event(StoryEvent::RiotUnit { planet: p, unit: pick, fleet: f });
        let mut d = copies.iter().find(|c| c.0 == u.design).map_or(0, |c| c.1);
        if d == 0 {
            d = w.copy_design_new(10, en, u.design);
            copies.push((u.design, d));
        }
        w.event(StoryEvent::UnitDesign { planet: p, unit: pick, design: d });
    }
    if !any {
        return 0;
    }
    if s.b_search(8540, 1, w) == 0 {
        s.b_add_id(p, 8540);
    }
    s.b_show(8540, -1, w);
    if s.v_get(8548) == 0 {
        w.event(StoryEvent::Anim(8541));
    }
    s.v_set(8548, 1);
    w.event(StoryEvent::HoldGroundWar { planet: p });
    s.set_adat_i32(0xb4, 1);
    s.v_inc(8540);
    p
}

/// 854: every 80..180 days (first after 100) a planet of the human's it sees (not 8060's) with more than 4 vehicles
/// and too few police (kind 6, 5000 people each) and kind-7 buildings (7000 each) for its people riots (the one with
/// the most vehicles; once chosen it keeps id 8540); the third time ends it. A ground battle the human starts on its
/// own planet after a riot, or the riot itself, may drive the human's units mad (`rnd(1, 100)` above
/// `police·400000 / (people + 1)`, clamped to 25..=75); the battle's end calms them. Daily: enough guards end it.
pub(super) fn story_854(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const P: i32 = 8540;
    const F: i32 = 8549;
    let en = w.human();
    if s.starting_now() {
        s.sz_start(P, 100);
        s.v_set(P, 0);
        dummy_fleet(s, w);
        s.v_set(F, 0);
    }
    let mut rioted = 0;
    if s.counter(P) {
        let mut best = s.b_search(P, 1, w);
        let skip = s.b_search(8060, 1, w);
        if best == 0 {
            let mut most = 0;
            for q in 1..=w.planet_count() {
                if w.planet_owner(q) != en || w.planet_visibility(q) <= 0 || q == skip {
                    continue;
                }
                let (police, seven) = guards(q, &*w);
                let (n, people) = (w.planet_garrison(q), w.planet_value(q));
                if n > 4 && police.wrapping_mul(5000) < people && seven.wrapping_mul(7000) < people && (best == 0 || most < n) {
                    best = q;
                    most = n;
                }
            }
        }
        if s.v_get(P) == 3 {
            message(w, 8543);
            s.win(0, true, w);
        } else {
            let go = best != 0 && w.planet_garrison(best) > 4;
            let days = rnd(w, 80, 180);
            s.sz_start(P, days);
            if go {
                rioted = riot(s, best, w);
            }
        }
    }
    if s.flags[flag::GROUND_END] && s.ground.planet == s.v_get(F) {
        let p = s.ground.planet;
        s.set_adat_i32(0xb4, 0);
        s.v_set(F, 0);
        if s.v_get(P) == 1 {
            message(w, 8540);
        } else if s.v_get(P) == 2 {
            message(w, 8541);
        }
        param(w, 1, MessageText::Planet(p));
        for (k, (alive, owner)) in w.battlefield_units(p).into_iter().enumerate() {
            if alive && owner == en {
                w.event(StoryEvent::CalmUnit { planet: p, unit: k as i32 });
            }
        }
    }
    if read(s, P) {
        let q = s.b_search(P, 0, w);
        s.add_info_line(8541, InfoLine::Planet { planet: q, count: None }, -1, w);
    }
    let start = s.flags[flag::GROUND_START];
    let gp = s.ground.planet;
    if (start && w.planet_owner(gp) == en && s.v_get(8548) != 0) || rioted != 0 {
        let p = if start { gp } else { rioted };
        s.v_set(F, p);
        let police = guards(p, &*w).0;
        // A planet of −1 people would divide by zero (the exe faults); never seen.
        let chance = police.wrapping_mul(400_000).checked_div(w.planet_value(p).wrapping_add(1)).unwrap_or(0).clamp(25, 75);
        for (k, (alive, owner)) in w.battlefield_units(p).into_iter().enumerate() {
            if alive && owner == en && rnd(w, 1, 100) > chance {
                w.event(StoryEvent::MadUnit { planet: p, unit: k as i32 });
            }
        }
    }
    if s.daily() && s.v_get(8548) != 0 {
        let p = s.b_search(P, 0, w);
        if w.planet_owner(p) == en {
            let (police, seven) = guards(p, &*w);
            let people = w.planet_value(p);
            let end = if people < police.wrapping_mul(5000) {
                Some(8542)
            } else if people < seven.wrapping_mul(7000) {
                Some(8545)
            } else {
                None
            };
            if let Some(id) = end {
                message(w, id);
                param(w, 1, MessageText::Planet(p));
                s.win(0, true, w);
            }
        } else {
            s.win(0, true, w);
        }
    }
    if s.counter(F) {
        message(w, 8542);
        s.win(0, true, w);
    }
    Ok(())
}
