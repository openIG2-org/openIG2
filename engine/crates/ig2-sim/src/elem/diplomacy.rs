//! `st_Elem_include`, part 2c: the diplomacy episodes 810 (an empire asks for protection), 857 (an alliance to buy
//! from the second strongest enemy), 858 (empire 5's planet taken: the others' non-aggression pacts) and 860 (empire
//! 1's tribute for three planets). Spec: `re/rules/campaign.md` §8.14, part 2c.

use super::{add_money, answered, param, promoted};
use crate::campaign::second_strongest_enemy;
use crate::rnd::relations::friendly_empire;
use crate::story::{flag, Abort, InfoLine, MessageText, Story, StoryEvent, StoryWorld, STREAM};

fn message(w: &mut dyn StoryWorld, id: i32, arg: i32) {
    w.event(StoryEvent::NewMessage { id, arg });
}

fn race(w: &mut dyn StoryWorld, e: i32) {
    w.event(StoryEvent::MessageRace(e));
}

/// `st_ShowAnim(_s(6, id))`: a video.
fn anim(w: &mut dyn StoryWorld, id: i32) {
    w.event(StoryEvent::Anim(id));
}

/// `st_Diplomacia(which, 0)` (exe 0x5d4a80): the diplomacy special `which` was answered (flag 13).
fn diplomacy(s: &Story, which: i32) -> bool {
    s.flags[flag::DIPLOMACY] && s.diplomacy.0 == which
}

/// `st_HaborubanAllE(a, b)`: level `Viszony[a][b].+0x25 < 3`.
fn at_war(w: &dyn StoryWorld, a: i32, b: i32) -> bool {
    w.relation_level(a, b) < 3
}

/// `CStoryWinException` unless empire `e` lives (`BirodalomEl[e]`).
fn alive(w: &dyn StoryWorld, e: i32) -> Result<(), Abort> {
    if w.empire_alive(e) { Ok(()) } else { Err(Abort) }
}

/// A ground battle ended on planet `bid` (0: any) and the battle's fleet (`Flottak[GrWarFlotta].+0x15`, read live)
/// is the human's.
fn human_landed(s: &mut Story, bid: i32, w: &mut dyn StoryWorld) -> bool {
    let en = w.human();
    s.ground_part(0, bid, w) && w.fleet_owner(s.ground.fleet) == en
}

// ---------------------------------------------------------------- 810: an empire asks for protection

/// 810 (exe block 0x5d95a1): an empire the human isn't at war with (`v(8109)`, chosen once) asks the human to
/// protect it; accepted, it watches the war with it (`v(6700)`'s second message) and ends in a war of the empire's
/// own after `C(6701)`.
pub(super) fn story_810(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let mut e = s.v_get(8109);
        if e == 0 {
            e = friendly_empire(5, 0, w);
            s.v_set(8109, e);
        }
        // `st_FelfedezBirodalom` inlined.
        if !w.knows(en, e) {
            w.event(StoryEvent::DiscoverEmpire(e));
        }
        message(w, 6601, 0);
        let e = s.v_get(8109);
        race(w, e);
        let e = s.v_get(8109);
        param(w, 1, MessageText::Empire(e));
        param(w, 2, MessageText::Player(en));
        param(w, 3, MessageText::Empire(en));
        let e = s.v_get(8109);
        s.protect_empire(e);
    }
    if answered(s, 6601, 2) || answered(s, 6800, 2) {
        let e = s.v_get(8109);
        s.add_info_line(8101, InfoLine::Empire(e), -1, w);
        s.v_set(8100, 1);
        let e = s.v_get(8109);
        s.set_adat_i32(0x24, e);
        s.sz_start(6011, 50);
        s.sz_start(6602, 360);
    }
    if answered(s, 6601, 3) || answered(s, 6800, 3) || promoted(s, 6601) || promoted(s, 6800) {
        s.v_set(8101, 1);
        s.win(0, true, w);
    }
    if s.counter(6011) && s.v_get(6700) == 1 {
        message(w, 6701, 100);
        race(w, en);
        s.sz_start(6011, 50);
        s.v_set(6701, 1);
        s.v_set(6700, 0);
    }
    if answered(s, 6701, 2) {
        s.sz_start(6701, 100);
    }
    if s.counter(6011) && s.v_get(6700) == 0 {
        let e = s.v_get(8109);
        if !at_war(&*w, en, e) {
            s.sz_start(6011, 50);
        } else {
            let id = if s.v_get(6701) == 0 { 6011 } else { 6711 };
            message(w, id, 0);
            race(w, e);
            param(w, 1, MessageText::Empire(e));
            if id == 6711 {
                s.v_set(8102, 1);
            }
            s.v_set(8100, 0);
            s.set_adat_i32(0x24, 0);
            s.win(0, true, w);
        }
    }
    if s.daily() {
        let e = s.v_get(8109);
        alive(&*w, e)?;
    }
    if s.counter(6602) {
        message(w, 6602, 0);
        let e = s.adat_i32(0x24);
        race(w, e);
        param(w, 1, MessageText::Empire(e));
        s.v_set(8100, 0);
        s.set_adat_i32(0x24, 0);
        s.remove_game_info(8101, w);
    }
    if answered(s, 6602, 2) || answered(s, 6802, 2) {
        s.win(0, true, w);
    }
    if answered(s, 6602, 3) || answered(s, 6802, 3) {
        s.win(0, true, w);
    }
    if promoted(s, 6602) || promoted(s, 6802) {
        message(w, 6603, 0);
        let e = s.v_get(8109);
        race(w, e);
        s.win(0, true, w);
    }
    if s.counter(6701) {
        let id = if w.rng().range(STREAM, 1, 2) == 1 { 6702 } else { 6703 };
        message(w, id, 60);
        let e = s.v_get(8109);
        param(w, 1, MessageText::Empire(e));
        race(w, en);
    }
    if answered(s, 6702, 2) || answered(s, 6703, 2) {
        s.sz_start(6705, 10);
    }
    if s.counter(6705) {
        message(w, 6709, 0);
        let e = s.v_get(8109);
        race(w, e);
        let e = s.v_get(8109);
        w.event(StoryEvent::DeclareWar { a: en, b: e });
        s.v_set(8102, 1);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 857: an alliance for 2,000,000

/// 857 (exe block 0x5e3f60): the second strongest enemy offers an alliance for 2,000,000 (diplomacy special 8571);
/// paid, the alliance ends it; its planets taken by the human end it too.
pub(super) fn story_857(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const PRICE: i32 = 2_000_000;
    let en = w.human();
    if s.starting_now() {
        let e = second_strongest_enemy(5, &*w);
        s.v_set(8589, (e != 0) as i32);
        if e == 0 {
            return Err(Abort);
        }
        s.v_set(8579, e);
        message(w, 8573, 0);
        race(w, e);
        w.event(StoryEvent::MessageParamInt { slot: 1, value: PRICE });
        param(w, 2, MessageText::Empire(e));
        s.sz_start(8570, 130);
        let d = s.v_get(8579);
        s.add_diplomacy(8571, d, -1, w);
        s.add_info_line(8571, InfoLine::Empire(e), -1, w);
    }
    if s.counter(8570) {
        // `MoneyMennyi` (x87) stored as f64 and compared with 2,000,000.0.
        if w.money(en) < PRICE as f64 {
            message(w, 8570, 0);
            let e = s.v_get(8579);
            param(w, 1, MessageText::Empire(e));
        }
        s.sz_start(8571, 100);
    }
    if diplomacy(s, 8571) {
        add_money(w, -PRICE, -1);
        anim(w, 8572);
        s.win(0, true, w);
    }
    if s.counter(8571) {
        let e = s.v_get(8579);
        if !w.empire_alive(e) {
            message(w, 8579, 0);
            s.sz_start(8572, 300);
            s.remove_game_info(8571, w);
        } else if w.relation_level(en, e) == 5 {
            anim(w, 8574);
            message(w, 8574, 0);
            s.win(0, true, w);
        } else {
            s.sz_start(8571, 3);
        }
    }
    if human_landed(s, 0, w) && s.ground.old_owner == s.v_get(8579) {
        let e = s.v_get(8579);
        if s.b_search(e.wrapping_neg(), 0, w) == 0 {
            message(w, 8572, 0);
            anim(w, 8572);
            s.win(0, true, w);
        }
    }
    if s.counter(8572) {
        message(w, 8571, 0);
        anim(w, 8571);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 858: empire 5's planet

/// 858 (exe block 0x5e4379): empire 5's least populated planet the human sees (id 8580, with a guard fleet from
/// another of its planets, id 8581); taken, every other live empire 1..=8 is offered a non-aggression pact
/// (diplomacy special 8581); empire 5 dead after that → `MEGNYERES`.
pub(super) fn story_858(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const E: i32 = 5;
    let en = w.human();
    if s.starting_now() {
        alive(&*w, E)?;
        if s.v_get(8589) == 0 {
            return Err(Abort);
        }
        // The planet of empire 5 the human sees with the fewest people (`st_BolygoErtek` = `+0x28`; strictly fewer
        // replaces).
        let mut best = 0;
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) == E && w.planet_visibility(p) >= 0 && (best == 0 || w.planet_value(best) > w.planet_value(p)) {
                best = p;
            }
        }
        if best == 0 {
            s.win(0, true, w);
        } else {
            s.b_add_id(best, 8580);
            let mut tries = 50;
            let q = loop {
                let q = s.b_search(-E, 0, w);
                tries -= 1;
                if tries == 0 || q != best {
                    break q;
                }
            };
            s.b_add_id(q, 8581);
            message(w, 8580, 0);
            param(w, 1, MessageText::Planet(best));
            s.add_info_line(8581, InfoLine::Planet { planet: best, count: None }, -1, w);
            s.b_show(8580, -1, w);
            s.f_new_ex(crate::story::FleetName::Text(8581), E, 8581, 0.09, 2, best, w);
        }
    }
    if human_landed(s, 8580, w) && s.ground.attacker_won && s.v_get(8582) == 0 {
        s.v_set(8582, 1);
        s.remove_game_info(8581, w);
        let p = s.b_search(8580, 0, w);
        w.event(StoryEvent::RemovePlanetInfo { planet: p, info: -8580 });
        message(w, 8581, 0);
        anim(w, 8581);
        let mut any = false;
        for e in 1..9 {
            if !w.empire_alive(e) || e == en || e == E {
                s.v_set(e + 8590, 0);
            } else {
                s.add_diplomacy(8581, e, -1, w);
                s.v_set(e + 8590, 1);
                any = true;
            }
        }
        s.v_set(8599, 1);
        if any {
            s.add_info_line(8582, InfoLine::Blank, -1, w);
        }
        s.sz_start(8580, 30);
    }
    if s.counter(8580) {
        s.v_set(8588, 1);
        if w.empire_alive(E) {
            anim(w, 8583);
            message(w, 8582, 0);
        }
    }
    if diplomacy(s, 8581) {
        let e = s.diplomacy.1;
        s.v_set(e.wrapping_add(8590), 0);
        w.event(StoryEvent::RemoveDiplomacySpecial { id: 8581, empire: e });
        let e = s.diplomacy.1;
        if w.relation_level(en, e) < 4 {
            w.event(StoryEvent::MakeNonAggression { a: en, b: e });
        }
    }
    if s.daily() && s.v_get(8599) != 0 {
        let (mut all, mut refused) = (true, 0);
        for e in 1..9 {
            if w.empire_alive(e) && e != en && e != E {
                if s.v_get(e + 8590) == 0 {
                    refused += 1;
                } else {
                    all = false;
                }
            }
        }
        if all {
            s.v_set(8599, 0);
            s.remove_game_info(8582, w);
            if refused != 0 {
                anim(w, 8582);
            }
        }
    }
    if s.daily() && s.v_get(8588) != 0 && !w.empire_alive(E) {
        message(w, 8589, 0);
        s.v_set(8588, 0);
        s.sz_start(8589, 1);
        s.v_set(4059, 1);
    }
    if s.counter(8589) {
        w.event(StoryEvent::Won);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 860: empire 1's tribute

/// 860 (exe block 0x5e4964): empire 1 asks for three of the human's planets nearest to it (ids 8600..=8602); the
/// answer 6..=9 marks them (info `200 + answer`) and opens diplomacy special 8601: answered → the end; else every
/// 30 days the tribute `StoryAdat+0x2c` pays 1.2 / 10 of it to the human, eleven times.
pub(super) fn story_860(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        alive(&*w, 1)?;
        alive(&*w, 5)?;
        s.protect_empire(1);
        message(w, 8600, 0);
        s.sz_start(8600, 80);
        if !w.knows(en, 1) {
            w.event(StoryEvent::DiscoverEmpire(1));
        }
        if at_war(&*w, en, 1) {
            w.event(StoryEvent::MakeNeutral { a: en, b: 1 });
        }
    }
    if s.counter(8600) {
        alive(&*w, 1)?;
        let Some(three) = s.three_nearest(1, &*w) else { return Err(Abort) };
        for (k, &p) in three.iter().enumerate() {
            s.b_add_id(p, 8600 + k as i32);
        }
        message(w, 8601, 100);
        for (k, &p) in three.iter().enumerate() {
            param(w, k as i32 + 1, MessageText::Planet(p));
        }
        param(w, 4, MessageText::Player(en));
        param(w, 5, MessageText::Empire(en));
    }
    if answered(s, 8601, 5) || promoted(s, 8601) {
        s.sz_start(8602, 300);
    }
    let mut answer = 0;
    for (r, a) in [(6, 1), (7, 2), (8, 3), (9, 4)] {
        if answered(s, 8601, r) {
            answer = a;
        }
    }
    if answer != 0 {
        s.v_set(8606, 1);
        s.sz_start(8601, 30);
        for k in 0..3 {
            let p = s.b_search(8600 + k, 0, w);
            if w.planet_owner(p) == en {
                s.b_add_info(p.wrapping_neg(), answer + 200, -1, w);
            }
        }
        s.set_adat_i32(0x2c, 0);
        s.add_diplomacy(8601, 1, -1, w);
    }
    if diplomacy(s, 8601) {
        s.sz_stop(8601);
        s.win(0, true, w);
    }
    if s.counter(8601) {
        alive(&*w, 1)?;
        let tribute = s.adat_i32(0x2c);
        let paid = (tribute as f64 * 1.2 / 10.0) as i32;
        let kept = tribute / 10;
        add_money(w, paid, -1);
        s.set_adat_i32(0x2c, 0);
        if s.v_get(8600) < 11 {
            w.event(StoryEvent::ScrollNumbers { text: 8601, a: kept, b: paid });
            s.v_inc(8600);
            s.sz_start(8601, 30);
        } else {
            message(w, 8604, 0);
            w.event(StoryEvent::MessageParamInt { slot: 1, value: kept });
            w.event(StoryEvent::MessageParamInt { slot: 2, value: paid });
            s.win(0, true, w);
        }
    }
    if s.counter(8602) {
        s.win(0, true, w);
    }
    Ok(())
}
