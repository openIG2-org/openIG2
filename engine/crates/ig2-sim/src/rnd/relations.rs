//! `st_Rnd_include`, part 2a: the stories about another empire (spec `re/rules/campaign.md` §8.13): a peace
//! offer (116), a trade treaty (117), a planet seized by pirates (145) and two planet swaps (147, 148).

use super::{answered, message, param, promoted, relation, Pick};
use crate::brnd::random_empire;
use crate::story::{flag, Abort, MessageText, Story, StoryEvent, StoryWorld};

/// `st_HaborubanAllE(a, b)`: `a` is at war with `b` (level `Viszony[a][b].+0x25 < 3`).
fn at_war(w: &dyn StoryWorld, a: i32, b: i32) -> bool {
    w.relation_level(a, b) < 3
}

/// `st_GetEllensegesBirodalom(x)` (exe 0x5ea640): a random live empire 1..=8 (not the human, not `x`) the human is
/// at war with (a reservoir pick, stream 5), 0 without one.
pub(super) fn enemy_empire(x: i32, w: &mut dyn StoryWorld) -> i32 {
    let en = w.human();
    let (mut pick, mut r) = (0, Pick::default());
    for e in 1..9 {
        if e != en && e != x && w.empire_alive(e) && at_war(&*w, en, e) && r.offer(w) {
            pick = e;
        }
    }
    pick
}

/// `st_GetNemEllensegesBirodalom(x, y)` (exe 0x5eaa60): a random live empire 1..=8 (not the human, `x` or `y`) the
/// human is not at war with, weighted 10 when the human knows it, else 1 (stream 5); 0 without one.
pub(crate) fn friendly_empire(x: i32, y: i32, w: &mut dyn StoryWorld) -> i32 {
    let en = w.human();
    let (mut pick, mut r) = (0, Pick::default());
    for e in 1..9 {
        if e != en && e != x && e != y && w.empire_alive(e) && !at_war(&*w, en, e) {
            let weight = if w.knows(en, e) { 10 } else { 1 };
            if r.weighted(weight, w) {
                pick = e;
            }
        }
    }
    pick
}

// ---------------------------------------------------------------- 116: a peace offer

/// 116: an empire 1..=8 at war with the human (not at cease-fire, level 2) offers neutrality (message 0x489, its
/// name; none → `WIN` without history). Accepted (answer 2) → both levels 3 (`st_SetSemlegesViszony`), the pirates
/// like it (+3); the war over otherwise takes the offer back; answer 3 ends it; message 0x425 promoted (the
/// source's slip for 0x489) → the pirates −2.
pub(super) fn peace(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const V: i32 = 0x489;
    let en = w.human();
    if s.starting_now() {
        let e = enemy_empire(5, w);
        if e == 0 || w.relation_level(en, e) == 2 {
            s.win(0, false, w);
        } else {
            s.v_set(V, e);
            message(w, V, 0x8c);
            param(w, 1, MessageText::Empire(en));
            param(w, 2, MessageText::Empire(e));
        }
    }
    if answered(s, V, 2) {
        let e = s.v_get(V);
        w.event(StoryEvent::MakeNeutral { a: en, b: e });
        relation(w, en, 9, 3);
        s.win(0, true, w);
    }
    if s.daily() {
        let e = s.v_get(V);
        if !at_war(&*w, en, e) {
            w.event(StoryEvent::RemoveMessage(V));
            s.win(0, true, w);
        }
    }
    if answered(s, V, 3) {
        s.win(0, true, w);
    }
    if promoted(s, 0x425) {
        relation(w, en, 9, -2);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 117: a trade treaty

/// 117: an empire the human knows, isn't at war with and has no trade agreement with offers one (message 0x493);
/// none fitting → abort. Accepted → the agreement both ways, it likes the human (+8), the pirates (+3); refused or
/// lapsed → the pirates −1; an agreement made meanwhile takes the offer back.
pub(super) fn trade(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const V: i32 = 0x493;
    let en = w.human();
    if s.starting_now() {
        let e = friendly_empire(5, 0, w);
        if e == 0 || !w.knows(en, e) || w.trade_agreement(en, e) {
            return Err(Abort);
        }
        s.v_set(V, e);
        message(w, V, 0x28);
        param(w, 1, MessageText::Empire(en));
        let e = s.v_get(V);
        param(w, 2, MessageText::Empire(e));
    }
    if s.daily() {
        let e = s.v_get(V);
        if w.trade_agreement(en, e) {
            w.event(StoryEvent::RemoveMessage(V));
            s.win(0, true, w);
        }
    }
    if answered(s, V, 2) {
        let e = s.v_get(V);
        w.event(StoryEvent::TradeAgreement { a: en, b: e });
        let e = s.v_get(V);
        relation(w, en, e, 8);
        relation(w, en, 9, 3);
        s.win(0, true, w);
    }
    if answered(s, V, 3) || promoted(s, V) {
        relation(w, en, 9, -1);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 145: a planet seized

/// 145: the pirates seize a random planet of a random empire (id 1451, owner 9); 30 days later the human may pay
/// for its rescue (message 0x5ab: answer 2 50,000 for a 7 in 10 chance, 3 10,000 for 2 in 10, 4 nothing for none;
/// promoted: the owner retakes it). A rescue that works gives the human the planet (message 0x5ac; the owner −10);
/// else the owner's fleet (id 1451) comes to retake it: arrived → the owner's again; beaten → the human's (message
/// 0x5ad; the owner −18).
pub(super) fn seized(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const P: i32 = 0x5ab;
    const E: i32 = 0x5b3;
    const TRY: i32 = 0x5ac;
    const FLEET: i32 = 0x5ad;
    let en = w.human();
    if s.starting_now() {
        let e = random_empire(w, 0, 0, 0)?;
        s.v_set(E, e);
        let p = s.b_search(e.wrapping_neg(), 0, w);
        s.b_add_id(p, P);
        s.set_planet_owner(p, 9, w);
        s.sz_start(P, 30);
    }
    if s.counter(P) {
        message(w, P, 0);
        let p = s.b_search(P, 0, w);
        param(w, 1, MessageText::Planet(p));
        let e = s.v_get(E);
        param(w, 2, MessageText::Empire(e));
    }
    if answered(s, P, 2) {
        super::add_money(w, -50_000.0);
        s.v_set(P, 7);
        s.sz_start(TRY, 15);
    }
    if answered(s, P, 3) {
        super::add_money(w, -10_000.0);
        s.v_set(P, 2);
        s.sz_start(TRY, 15);
    }
    if answered(s, P, 4) {
        s.v_set(P, 0);
        s.sz_start(TRY, 5);
    }
    if promoted(s, P) {
        s.sz_start(FLEET, 7);
    }
    if s.counter(TRY) {
        let r = super::rnd(w, 1, 10);
        if s.v_get(P) < r {
            s.sz_start(FLEET, 15);
        } else {
            let p = s.b_search(P, 0, w);
            s.set_planet_owner(p, en, w);
            let e = s.v_get(E);
            relation(w, en, e, -10);
            message(w, TRY, 0);
            let p = s.b_search(P, 0, w);
            param(w, 1, MessageText::Planet(p));
            param(w, 2, MessageText::Empire(en));
            s.win(0, true, w);
        }
    }
    if s.counter(FLEET) {
        let e = s.v_get(E);
        let e2 = s.v_get(E);
        let at = s.b_search(e2.wrapping_neg(), 0, w);
        s.f_new_ex(crate::story::FleetName::Text(0x5ab), e, P, 0.05, 2, at, w);
        super::move_to_planet(s, P, P, w);
    }
    if s.arrived_planet(P, 0, w) {
        let p = s.b_search(P, 0, w);
        let e = s.v_get(E);
        s.set_planet_owner(p, e, w);
        super::delete_fleet(s, P, w);
    }
    if s.battle_part(P) != 0 {
        if !s.battle_won(P) {
            let p = s.b_search(P, 0, w);
            s.set_planet_owner(p, en, w);
            message(w, FLEET, 0);
            let e = s.v_get(E);
            relation(w, en, e, -18);
        }
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 147, 148: planet swaps

/// The two planets of a random empire `e` (`v(var) := e`): `p` (id `a`) and, of up to 100 more picks, one that
/// differs (id `b`); `None` when every pick was `p`.
fn two_planets(s: &mut Story, var: i32, a: i32, b: i32, w: &mut dyn StoryWorld) -> Result<Option<(i32, i32, i32)>, Abort> {
    let e = random_empire(w, 0, 0, 0)?;
    s.v_set(var, e);
    let p = s.b_search(e.wrapping_neg(), 0, w);
    s.b_add_id(p, a);
    let mut tries = 100;
    let q = loop {
        let q = s.b_search(e.wrapping_neg(), 0, w);
        if q != p {
            break q;
        }
        tries -= 1;
        if tries == 0 {
            break q;
        }
    };
    if q == p {
        s.win(0, true, w);
        return Ok(None);
    }
    s.b_add_id(q, b);
    Ok(Some((e, p, q)))
}

/// `st_Diplomacia(which, 0)`: the diplomacy special `which` happened (flag 13).
fn diplomacy(s: &Story, which: i32) -> bool {
    s.flags[flag::DIPLOMACY] && s.diplomacy.0 == which
}

/// 147: an empire offers to swap two of its planets (ids 1471, 1482: the source's slip for 1472; message 0x5bf, or
/// 0x5c7 once 136's logic flag 1361 is set). Either answer 2 (v := 2) or 3 (v := 1) opens diplomacy special 0x5bf
/// with it for 25 days; the special answered → it likes the human (+10), v := 0. After 25 days: v 2 → its fleet
/// (id 1471) leaves planet 1471 for 1472 (which has no planet); v 1 → the end; v 0 → the pirates −6, the end. A
/// space battle of fleet 1471 with the human that 1481 (148's fleet) didn't win → message 0x5c0, the end.
pub(super) fn swap_147(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const P: i32 = 0x5bf;
    const E: i32 = 0x5c7;
    let en = w.human();
    if s.starting_now() {
        if let Some((e, p, q)) = two_planets(s, E, P, 0x5ca, w)? {
            let id = if s.l_is_true(0x551) { 0x5c7 } else { 0x5bf };
            message(w, id, 0);
            param(w, 1, MessageText::Empire(e));
            param(w, 2, MessageText::Empire(e));
            param(w, 3, MessageText::Planet(p));
            param(w, 4, MessageText::Planet(q));
        }
    }
    if answered(s, P, 2) || answered(s, E, 2) {
        s.sz_start(P, 25);
        s.v_set(P, 2);
        let e = s.v_get(E);
        s.add_diplomacy(P, e, -1, w);
    }
    if answered(s, P, 3) || answered(s, E, 3) {
        s.v_set(P, 1);
        s.sz_start(P, 25);
        let e = s.v_get(E);
        s.add_diplomacy(P, e, -1, w);
    }
    if promoted(s, P) || promoted(s, E) {
        s.win(0, true, w);
    }
    if s.counter(P) {
        if s.v_get(P) < 2 {
            if s.v_get(P) != 1 {
                relation(w, en, 10, -6);
            }
            s.win(0, true, w);
        } else {
            let e = s.v_get(E);
            let at = s.b_search(P, 0, w);
            s.f_new_ex(crate::story::FleetName::Text(0x5bf), e, P, 0.05, 2, at, w);
            super::move_to_planet(s, P, 0x5c0, w);
        }
    }
    if diplomacy(s, P) {
        let e = s.v_get(E);
        relation(w, en, e, 10);
        s.v_set(P, 0);
    }
    if s.battle_part(P) != 0 && super::battle_owner_part(s, en) != 0 && !s.battle_won(0x5c9) {
        message(w, 0x5c0, 0);
        s.win(0, true, w);
    }
    Ok(())
}

/// 148: the same offer for planets 1481 and 1482 (message 0x5c9, 20 days, special 0x5c9; v 0 or 1 → the end). A
/// space battle of fleet 1481 with the human: the human won → 100,000 (message 0x5ca), the end; lost → message
/// 0x5cc, the empire −15.
pub(super) fn swap_148(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const P: i32 = 0x5c9;
    const Q: i32 = 0x5ca;
    const E: i32 = 0x5d1;
    let en = w.human();
    if s.starting_now() {
        if let Some((e, p, q)) = two_planets(s, E, P, Q, w)? {
            message(w, P, 0xf);
            s.b_add_id(p, P);
            s.b_add_id(q, Q);
            param(w, 1, MessageText::Empire(e));
            param(w, 2, MessageText::Planet(p));
            param(w, 3, MessageText::Planet(q));
        }
    }
    if answered(s, P, 2) {
        s.v_set(P, 2);
        s.sz_start(P, 20);
        let e = s.v_get(E);
        s.add_diplomacy(P, e, -1, w);
    }
    if answered(s, P, 3) {
        s.v_set(P, 1);
        s.sz_start(P, 20);
        let e = s.v_get(E);
        s.add_diplomacy(P, e, -1, w);
    }
    if promoted(s, P) {
        s.win(0, true, w);
    }
    if s.counter(P) {
        if s.v_get(P) < 2 {
            s.v_get(P);
            s.win(0, true, w);
        } else {
            let e = s.v_get(E);
            let at = s.b_search(P, 0, w);
            s.f_new_ex(crate::story::FleetName::Text(0x5c9), e, P, 0.05, 2, at, w);
            super::move_to_planet(s, P, Q, w);
        }
    }
    if diplomacy(s, P) {
        let e = s.v_get(E);
        relation(w, en, e, 10);
        s.v_set(P, 0);
    }
    if s.battle_part(P) != 0 && super::battle_owner_part(s, en) != 0 {
        if !s.battle_won(P) {
            message(w, Q, 0);
            super::add_money(w, 100_000.0);
            s.win(0, true, w);
        } else {
            message(w, 0x5cc, 0);
            let e = s.v_get(E);
            relation(w, en, e, -15);
        }
    }
    Ok(())
}
