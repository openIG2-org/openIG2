//! `st_Rnd_include`, part 2a: planets found (spec `re/rules/campaign.md` §8.13): a tech hidden on a planet (128,
//! 129), ruins (132), a race's settlers (133), a marked planet (134), a deserter (136), a far planet's tech (137)
//! and settlers asking for a planet (139).

use super::{add_money, answered, anim, message, param, promoted, relation};
use crate::begin::human_distance;
use crate::brnd::{can_give, random_empire, random_tech};
use crate::story::{flag, last_fn, Abort, MessageText, Story, StoryEvent, StoryWorld};

/// `(f64)(planet x + 300) + rnd(0, 99) / 100` (f32 sum): a coordinate as the messages show it.
fn coordinate(c: f32, r: i32) -> f64 {
    (c + 300.0) as f64 + r as f64 / 100.0
}

/// `rnd(0, 600) + rnd(0, 99) / 100`: a made-up third coordinate.
fn depth(w: &mut dyn StoryWorld) -> f64 {
    let a = super::rnd(w, 0, 600);
    let b = super::rnd(w, 0, 99);
    a as f64 + b as f64 / 100.0
}

/// Parameters 2..=4 of 128's and 129's messages: the planet's coordinates and a depth, one `%.2f` each.
fn coordinates(p: i32, w: &mut dyn StoryWorld) {
    let (x, y) = w.planet_xy(p);
    let r = super::rnd(w, 0, 99);
    param(w, 2, MessageText::Number(coordinate(x, r)));
    let r = super::rnd(w, 0, 99);
    param(w, 3, MessageText::Number(coordinate(y, r)));
    let z = depth(w);
    param(w, 4, MessageText::Number(z));
}

/// Parameter 2 of 132's and 133's messages: `"%.2f, %.2f, %.2f"` of the planet's coordinates and a depth (the exe
/// draws the depth first, then y's and x's hundredths).
fn position(p: i32, w: &mut dyn StoryWorld) {
    let z = depth(w);
    let ry = super::rnd(w, 0, 99);
    let rx = super::rnd(w, 0, 99);
    let (x, y) = w.planet_xy(p);
    param(w, 2, MessageText::Position([coordinate(x, rx), coordinate(y, ry), z]));
}

/// `st_ArrivedB(0, b)` by a fleet of the human's.
fn human_arrived(s: &mut Story, b: i32, w: &mut dyn StoryWorld) -> bool {
    s.arrived_planet(0, b, w) && w.fleet_owner(s.arrival.fleet) == w.human()
}

/// `st_RangeB(b)`: the human's radar reached planet `b` (flag 11).
fn radar_planet(s: &mut Story, b: i32, w: &mut dyn StoryWorld) -> bool {
    s.flags[flag::RADAR_PLANET] && (b == 0 || s.radar.0 == s.b_search(b, 0, w))
}

/// `st_RangeF(f)`: the human's radar reached fleet `f` (flag 10).
fn radar_fleet(s: &mut Story, f: i32, w: &mut dyn StoryWorld) -> bool {
    s.flags[flag::RADAR_FLEET] && (f == 0 || s.radar.1 == s.f_search(f, w))
}

/// Planet `id` has an owner.
fn owned(s: &mut Story, id: i32, w: &mut dyn StoryWorld) -> bool {
    let p = s.b_search(id, 0, w);
    w.planet_owner(p) != 0
}

/// `BolygoInfoRemove(b(id), −id, false)`: the planet's info line `−id` (`st_bShow`'s) goes.
fn remove_info(s: &mut Story, id: i32, w: &mut dyn StoryWorld) {
    let p = s.b_search(id, 0, w);
    w.event(StoryEvent::RemovePlanetInfo { planet: p, info: id.wrapping_neg() });
}

/// `v(var)` stays when the human can be given that tech (`st_TalalmanyAdhato`), else becomes a random one
/// (`st_GetRandomTalalmany(−1, 0)`).
fn givable(s: &mut Story, var: i32, w: &mut dyn StoryWorld) {
    let en = w.human();
    let t = s.v_get(var);
    if !can_give(&*w, t, en) {
        let t = random_tech(w, en, 0);
        s.v_set(var, t);
    }
}

// ---------------------------------------------------------------- 128, 129: a tech on a planet

/// 128: a free planet 100..150 from the human's (id 1281) hides a tech the human can be given (`v(1281)`; none →
/// the end). Two days later (and 90 days after answer 4) message 0x501 (0x507 the second time) gives its position;
/// answer 2 pays 10,000 to show the planet, 3 (or promoted) ends it. A fleet of the human's there → message 0x506
/// and 20 days later: 1 in 20 nothing (0x503), else a video (0x502) and 15 days later the tech (0x505, a coin
/// thrown away) unless `rnd(1, 100) < 8` (0x504). The planet taken by anyone ends it (the open message taken back).
pub(super) fn tech_planet(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x501;
    const AGAIN: i32 = 0x509;
    const OPEN: i32 = 0x502;
    const WAIT: i32 = 0x503;
    let en = w.human();
    if s.starting_now() {
        let p = s.planet_at_distance(100, 150, w)?;
        s.b_add_id(p, ID);
        let t = random_tech(w, en, 0);
        if t == 0 {
            s.win(0, true, w);
        } else {
            s.v_set(ID, t);
            s.sz_start(AGAIN, 2);
            s.v_set(AGAIN, 0);
        }
    }
    if s.counter(AGAIN) {
        let t = s.v_get(ID);
        if w.tech_status(t, en) < 5 {
            let id = if s.v_get(AGAIN) == 0 { ID } else { 0x507 };
            message(w, id, 0x28);
            let p = s.b_search(ID, 0, w);
            param(w, 1, MessageText::Planet(p));
            coordinates(p, w);
            s.v_set(OPEN, 1);
            s.v_set(WAIT, 1);
        } else {
            s.win(0, true, w);
        }
    }
    if s.daily() && s.v_get(WAIT) != 0 && owned(s, ID, w) {
        if s.v_get(OPEN) != 0 {
            w.event(StoryEvent::RemoveMessage(ID));
        }
        s.win(0, true, w);
    }
    if answered(s, ID, 2) {
        s.v_set(OPEN, 0);
        add_money(w, -10_000.0);
        s.b_show(ID, -1, w);
    }
    if answered(s, ID, 3) || promoted(s, ID) {
        s.win(0, true, w);
    }
    if answered(s, ID, 4) {
        s.sz_start(AGAIN, 90);
    }
    if s.arrived_planet(0, ID, w) && s.v_get(WAIT) != 0 && w.fleet_owner(s.arrival.fleet) == en {
        message(w, 0x506, 0);
        s.sz_start(ID, 20);
        s.v_set(WAIT, 0);
    }
    if s.counter(ID) {
        if super::rnd(w, 1, 20) == 1 {
            message(w, 0x503, 0);
            s.win(0, true, w);
        } else {
            message(w, 0x502, 0);
            anim(w, 0x502);
            s.sz_start(OPEN, 15);
        }
    }
    if s.counter(OPEN) {
        let r = super::rnd(w, 1, 100);
        givable(s, ID, w);
        if r < 8 || s.v_get(ID) == 0 {
            message(w, 0x504, 0);
        } else {
            message(w, 0x505, 0);
            super::rnd(w, 0, 2);
            let t = s.v_get(ID);
            s.set_tech_state(t, en, w);
            let t = s.v_get(ID);
            param(w, 1, MessageText::Tech(t));
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 129: the nearest unknown planet of 11001..=11015 (id 1291) hides a tech (`v(1291)`; none → the end); message
/// 0x50b with its position. Answer 2 pays 3,000 to show it and starts a 400-day limit; 3 (or promoted) ends it.
/// A fleet of the human's there (once) → message 0x510 and 20 days later: the limit run out → 0x50d, the end; else
/// a video (0x50c) and 15 days later a tech (0x50f; `v(1291)` was overwritten with 1 on the arrival, the source's
/// slip). The planet taken ends it.
pub(super) fn lost_tech(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x50b;
    const OPEN: i32 = 0x50d;
    const WAIT: i32 = 0x50e;
    const LIMIT: i32 = 0x50f;
    const ONCE: i32 = 0x512;
    let en = w.human();
    if s.starting_now() {
        let p = s.nearest_unknown(w)?;
        s.b_add_id(p, ID);
        let t = random_tech(w, en, 0);
        s.v_set(ID, t);
        s.v_set(ONCE, 0);
        if t == 0 {
            s.win(0, true, w);
        } else {
            message(w, ID, 0x14);
            param(w, 1, MessageText::Planet(p));
            coordinates(p, w);
            s.v_set(OPEN, 1);
            s.v_set(WAIT, 1);
            s.v_set(LIMIT, 0);
        }
    }
    if answered(s, ID, 2) {
        add_money(w, -3000.0);
        s.b_show(ID, -1, w);
        s.sz_start(LIMIT, 400);
        s.v_set(OPEN, 0);
    }
    if answered(s, ID, 3) || promoted(s, ID) {
        s.win(0, true, w);
    }
    if s.daily() && s.v_get(WAIT) != 0 && owned(s, ID, w) {
        if s.v_get(OPEN) != 0 {
            w.event(StoryEvent::RemoveMessage(ID));
        }
        s.win(0, true, w);
    }
    if s.counter(LIMIT) {
        s.v_set(LIMIT, 1);
    }
    if human_arrived(s, ID, w) && s.v_get(ONCE) == 0 {
        s.v_set(ONCE, 1);
        remove_info(s, ID, w);
        s.v_set(ID, 1);
        message(w, 0x510, 0);
        s.sz_start(ID, 20);
        s.v_set(WAIT, 0);
        super::move_to_planet(s, 0x50c, 0x50c, w);
        s.remove_fleet_id(0x50c, w);
    }
    if s.counter(ID) {
        super::rnd(w, 1, 20);
        if s.v_is_equal(LIMIT, 1) {
            message(w, OPEN, 0);
            s.win(0, true, w);
        } else {
            message(w, 0x50c, 0);
            anim(w, 0x50c);
            s.sz_start(0x50c, 15);
        }
    }
    if s.counter(0x50c) {
        message(w, LIMIT, 0);
        givable(s, ID, w);
        let t = s.v_get(ID);
        if t == 0 {
            param(w, 1, MessageText::Text(0x50f));
        } else {
            s.set_tech_state(t, en, w);
            param(w, 1, MessageText::Tech(t));
        }
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 132, 133, 134: unknown planets

/// 132: ruins on the nearest unknown planet (id 1321, its story buildings), message 0x529 with its position.
/// Answer 2 pays 1,000 to show it (the radar notices it); 3 (or promoted) ends it. Seen by the radar or reached by a
/// fleet of the human's: still free → message 0x52a; the end either way. Taken by anyone (daily) → the end.
pub(super) fn ruins(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x529;
    const OPEN: i32 = 0x531;
    if s.starting_now() {
        let p = s.nearest_unknown(w)?;
        s.b_add_id(p, ID);
        s.auto_add_building(ID, w);
        message(w, ID, 0x1e);
        param(w, 1, MessageText::Planet(p));
        position(p, w);
        s.v_set(ID, 1);
    }
    if answered(s, ID, 2) {
        add_money(w, -1000.0);
        s.b_show(ID, -1, w);
        s.radar_notice(ID, w);
        s.v_set(OPEN, 0);
    }
    if answered(s, ID, 3) || promoted(s, ID) {
        s.win(0, true, w);
    }
    if s.daily() && owned(s, ID, w) {
        if s.v_get(OPEN) != 0 {
            w.event(StoryEvent::RemoveMessage(ID));
        }
        s.win(0, true, w);
    }
    if radar_planet(s, ID, w) || human_arrived(s, ID, w) {
        if !owned(s, ID, w) {
            message(w, 0x52a, 0);
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 133: the nearest unknown planet (id 1331, its story buildings) is wanted by two random empires (`v(1331)`,
/// `v(1332)`; message 0x533 with its position). Answer 2 pays 3,000: it is shown and after `rnd(260, 318)` /
/// `rnd(240, 344)` days each empire sends settlers (fleets 1332 / 1333 from one of its planets, ids 1332 / 1333:
/// ships 4×1 and 3×3 (3×4), tanks 1×3 (1×4)); answer 3 pays 6,000 to show it only; 4 (or promoted) ends it. Settlers
/// arriving colonise it (`st_Colonise`) while it is free, else go home. Seen or reached by the human (once): free →
/// message 0x534, else not the human's → 0x535; its info line goes. Taken (daily) or colonised by the human (special
/// 10) → the settlers go home and lose their ids, the end.
pub(super) fn race(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x533;
    const SECOND: i32 = 0x534;
    const THIRD: i32 = 0x535;
    const SEEN: i32 = 0x53a;
    const OPEN: i32 = 0x53b;
    let en = w.human();
    if s.starting_now() {
        let p = s.nearest_unknown(w)?;
        s.b_add_id(p, ID);
        let a = random_empire(w, 0, 0, 0)?;
        let b = random_empire(w, a, 0, 0)?;
        s.v_set(ID, a);
        s.v_set(SECOND, b);
        message(w, ID, 0x1e);
        param(w, 1, MessageText::Planet(p));
        position(p, w);
        s.auto_add_building(ID, w);
        s.v_set(OPEN, 1);
        s.v_set(SEEN, 0);
    }
    if answered(s, ID, 2) {
        add_money(w, -3000.0);
        s.b_show(ID, -1, w);
        s.radar_notice(ID, w);
        let a = super::rnd(w, 260, 318);
        let b = super::rnd(w, 240, 344);
        s.sz_start(ID, a);
        s.sz_start(SECOND, b);
        s.v_set(OPEN, 0);
    }
    if answered(s, ID, 3) {
        add_money(w, -6000.0);
        s.b_show(ID, -1, w);
        s.radar_notice(ID, w);
        s.sz_start(OPEN, 20);
        s.v_set(OPEN, 0);
    }
    if answered(s, ID, 4) || promoted(s, ID) {
        s.win(0, true, w);
    }
    for (counter, var, fleet, ships) in [(ID, ID, SECOND, 3), (SECOND, SECOND, THIRD, 4)] {
        if s.counter(counter) {
            let e = s.v_get(var);
            let p = s.b_search(e.wrapping_neg(), 0, w);
            s.b_add_id(p, fleet);
            let at = s.b_search(fleet, 0, w);
            s.f_new_ex(crate::story::FleetName::Text(fleet), e, fleet, 0.05, at.wrapping_neg(), 0, w);
            s.f_add_ship(fleet, 4, 1, w);
            s.f_add_ship(fleet, 3, ships, w);
            s.f_add_tank(fleet, 1, ships, w);
            super::move_to_planet(s, fleet, ID, w);
        }
    }
    if s.arrived_planet(SECOND, 0, w) || s.arrived_planet(THIRD, 0, w) {
        let third = s.arrived_planet(THIRD, 0, w);
        let e = if third { s.v_get(SECOND) } else { s.v_get(ID) };
        let fleet = SECOND | third as i32;
        let p = s.b_search(ID, 0, w);
        if w.planet_owner(p) == 0 {
            if w.planet_owner(p) != e {
                w.event(StoryEvent::Colonise { planet: p, owner: e });
            }
            super::delete_fleet(s, fleet, w);
        } else {
            super::move_to_planet(s, fleet, fleet, w);
        }
    }
    if s.daily() && owned(s, ID, w) {
        settlers_home(s, w);
        if s.v_get(OPEN) == 1 {
            w.event(StoryEvent::RemoveMessage(ID));
        }
        s.win(0, true, w);
    }
    if (radar_planet(s, ID, w) || human_arrived(s, ID, w)) && s.v_get(SEEN) == 0 {
        let p = s.b_search(ID, 0, w);
        let owner = w.planet_owner(p);
        if owner == 0 {
            message(w, SECOND, 0);
        } else if owner != en {
            message(w, THIRD, 0);
        }
        remove_info(s, ID, w);
        s.v_set(SEEN, 1);
    }
    if s.special_is(10, en, 0, 0) {
        let d2 = s.special[2];
        if d2 == s.b_search(ID, 0, w) {
            settlers_home(s, w);
            s.win(0, true, w);
        }
    }
    Ok(())
}

/// 133's settlers go home (to their planets 1332 / 1333) and lose their ids.
fn settlers_home(s: &mut Story, w: &mut dyn StoryWorld) {
    for id in [0x534, 0x535] {
        if s.f_search(id, w) != 0 {
            super::move_to_planet(s, id, id, w);
            s.remove_fleet_id(id, w);
        }
    }
}

/// 134: the nearest unknown planet (id 1341, its story buildings) is marked and shown (message 0x53d). A fleet of
/// the human's there: still free → message 0x53e; the end either way; taken (daily) → the end.
pub(super) fn marked(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x53d;
    if s.starting_now() {
        let p = s.nearest_unknown(w)?;
        s.b_add_id(p, ID);
        s.auto_add_building(ID, w);
        message(w, ID, 0);
        param(w, 1, MessageText::Planet(p));
        s.b_show(ID, -1, w);
    }
    if s.daily() && owned(s, ID, w) {
        s.win(0, true, w);
    }
    if human_arrived(s, ID, w) {
        if !owned(s, ID, w) {
            message(w, 0x53e, 0);
        }
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 136: a deserter

/// `make_point_in_dist(&x, &y, from, to, d)` (inlined): the point `d` from `from` towards `to` (f32; the distance
/// through f64 `sqrt`).
fn towards(from: (f32, f32), to: (f32, f32), d: f32) -> (f32, f32) {
    let (dx, dy) = (from.0 - to.0, from.1 - to.1);
    let len = ((dy * dy + dx * dx) as f64).sqrt() as f32;
    ((to.0 - from.0) * d / len + from.0, (to.1 - from.1) * d / len + from.1)
}

/// 136: a deserter (fleet 1362 of owner 10, from `st_fNewEx` mode 1) waits 25 off a planet near the human (id 1362)
/// for 600 days, its radar contact reported (`+0x52` bit 0x40); the nearest unknown planet is 1361. Seen by the
/// radar: message 0x551 once (`v(1360)`), else 30 days (counter 1361). Answer 2 → planet 1361 shown and the
/// deserter goes near it (±10, the exe draws y first); 3 or promoted (owner 10 −1) → it leaves, the end. A fleet of
/// the human's at planet 1361 → its info line goes, 14 days later (counter 1362) 1 in 10 a trap (message 0x553),
/// else a tech and message 0x552 or 0x553 (a coin). A battle of the deserter with the human: owner 10 −5 or −10
/// and messages 0x556/0x558/0x559 by `v(1366)` and who won. The answers to 0x552/0x553 move the pirates' relation
/// (−7..+9) and pay 25,000..50,000; 0x552 answer 4 sets logic flag 1361 (147's other message).
pub(super) fn deserter(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const PLANET: i32 = 0x551;
    const FLEET: i32 = 0x552;
    const OFFER: i32 = 0x553;
    const LIMIT: i32 = 0x554;
    const SEEN: i32 = 0x550;
    const STATE: i32 = 0x556;
    let en = w.human();
    if s.starting_now() {
        let p = s.nearest_unknown(w)?;
        s.b_add_id(p, PLANET);
        s.f_new_ex(crate::story::FleetName::Text(0x552), 10, FLEET, 0.07, 1, 0, w);
        let b = s.last_fn[last_fn::B];
        s.b_add_id(b, FLEET);
        let at = (s.last_fn[last_fn::X] as f32, s.last_fn[last_fn::Y] as f32);
        let (x, y) = towards(w.planet_xy(b), at, 25.0);
        let f = s.f_search(FLEET, w);
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 0, x, y });
        s.fleet_radar_msg(FLEET, w);
        s.sz_start(LIMIT, 600);
    }
    if s.counter(LIMIT) {
        s.send_away(FLEET, w);
        s.win(0, true, w);
    }
    if radar_fleet(s, FLEET, w) {
        s.sz_stop(LIMIT);
        if s.v_is_equal(SEEN, 1) {
            s.sz_start(PLANET, 30);
        } else {
            message(w, PLANET, 0x78);
            let p = s.b_search(PLANET, 0, w);
            param(w, 1, MessageText::Planet(p));
            s.v_set(SEEN, 1);
        }
    }
    if answered(s, PLANET, 2) {
        s.b_show(PLANET, -1, w);
        let p = s.b_search(PLANET, 0, w);
        let ry = super::rnd(w, -10, 10);
        let rx = super::rnd(w, -10, 10);
        let f = s.f_search(FLEET, w);
        let (px, py) = w.planet_xy(p);
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 0, x: px + rx as f32, y: py + ry as f32 });
    }
    if answered(s, PLANET, 3) {
        s.send_away(FLEET, w);
        s.win(0, true, w);
    }
    if promoted(s, PLANET) {
        relation(w, en, 10, -1);
        s.send_away(FLEET, w);
        s.win(0, true, w);
    }
    if human_arrived(s, PLANET, w) {
        s.sz_stop(PLANET);
        s.sz_start(FLEET, 14);
        remove_info(s, PLANET, w);
    }
    if s.battle_part(FLEET) != 0 && super::battle_owner_part(s, en) != 0 {
        let won = s.battle_won(FLEET);
        if !s.v_greater(STATE, 1) {
            relation(w, en, 10, -5);
            if won {
                message(w, 0x558, 0);
                s.send_away(FLEET, w);
            } else {
                message(w, 0x559, 0);
            }
        } else if won {
            relation(w, en, 10, -10);
            add_money(w, 5000.0);
            message(w, 0x556, 0);
        } else {
            relation(w, en, 10, -5);
            message(w, 0x558, 0);
            s.send_away(FLEET, w);
        }
        s.win(0, true, w);
    }
    if s.counter(PLANET) {
        message(w, 0x557, 0);
        s.send_away(FLEET, w);
        s.win(0, true, w);
    }
    if s.counter(FLEET) {
        if super::rnd(w, 1, 10) == 1 {
            message(w, OFFER, 0);
            s.send_away(FLEET, w);
            s.win(0, true, w);
        } else {
            let t = random_tech(w, en, 0);
            s.set_tech_state(t, en, w);
            let coin = super::rnd(w, 0, 1);
            message(w, FLEET | (coin == 0) as i32, 0);
        }
    }
    if answered(s, FLEET, 2) || answered(s, FLEET, 3) {
        s.v_set(STATE, 2);
        relation(w, en, 10, -7);
        s.send_away(FLEET, w);
        s.win(0, true, w);
    }
    if answered(s, FLEET, 4) {
        relation(w, en, 10, 9);
        s.send_away(FLEET, w);
        s.l_set(PLANET, true);
        s.win(0, true, w);
    }
    for (part, x, money) in [(2, -7, 25_000.0), (5, -10, 30_000.0), (6, -17, 50_000.0)] {
        if answered(s, OFFER, part) {
            relation(w, en, 10, x);
            add_money(w, money);
            s.send_away(FLEET, w);
            s.win(0, true, w);
        }
    }
    if answered(s, OFFER, 4) {
        relation(w, en, 10, 9);
        s.send_away(FLEET, w);
        s.win(0, true, w);
    }
    Ok(())
}

impl Story {
    /// `st_fSetRadarMsg(id, true)` (inlined): fleet `id`'s radar contact is reported (`+0x52` bit 0x40).
    fn fleet_radar_msg(&mut self, id: i32, w: &mut dyn StoryWorld) {
        let f = if id < 0 { id.wrapping_neg() } else { self.f_search(id, w) };
        if f >= 1 {
            w.event(StoryEvent::FleetFlags { fleet: f, set: 0x40, clear: 0 });
        }
        self.error = (f < 1) as i32;
    }

    /// `st_bNearOutRR(lo, mode)` (exe 0x603b60): a free planet the human sees (mode 1: with its view 0) outside its
    /// radar (`st_IsInRR(x, y, 3)`), the nearest under 150 from its planets, or once one is within `lo + 100` a
    /// pick weighted by `|lo − d|` among those; aborts when there is none.
    pub fn near_out_of_radar(&mut self, lo: i32, mode: i32, w: &mut dyn StoryWorld) -> Result<i32, Abort> {
        let (mut best, mut pick, mut r) = (150, 0, super::Pick::default());
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) != 0 || w.planet_visibility(p) < 0 || (mode != 0 && w.planet_visibility(p) != 0) {
                continue;
            }
            let (x, y) = w.planet_xy(p);
            if w.in_radar_range(x as i32, y as i32, 3) {
                continue;
            }
            let d = human_distance(p, &*w);
            if !(d < best || d < lo + 100) {
                continue;
            }
            if best <= lo + 100 && !r.weighted((lo - d).wrapping_abs(), w) {
                continue;
            }
            best = d;
            pick = p;
        }
        if pick == 0 {
            Err(Abort)
        } else {
            Ok(pick)
        }
    }
}

// ---------------------------------------------------------------- 137, 139

/// 137: a free planet outside the radar with the human's view 0 (id 1371) is shown and noticed (message 0x55b;
/// campaign 2: 0x14fb, 3: 0x1113). Taken (daily) → the end; seen by the radar while owned: not the human's →
/// message 0x55f, the end. A fleet of the human's there: a video and a tech (message 0x55c) unless there is none to
/// give (`rnd(1, 10)`, again `rnd(1, 5)` when it was above 5 without a tech; both unused); the end.
pub(super) fn far_tech(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x55b;
    let en = w.human();
    if s.starting_now() {
        let p = s.near_out_of_radar(0, 1, w)?;
        s.b_add_id(p, ID);
        let id = match w.campaign_type() {
            3 => 0x1113,
            2 => 0x14fb,
            _ => ID,
        };
        message(w, id, 0);
        param(w, 1, MessageText::Planet(p));
        s.b_show(ID, -1, w);
        s.radar_notice(ID, w);
    }
    if s.daily() && owned(s, ID, w) {
        s.win(0, true, w);
    }
    if radar_planet(s, ID, w) {
        let p = s.b_search(ID, 0, w);
        let owner = w.planet_owner(p);
        if owner != 0 {
            if owner != en {
                message(w, 0x55f, 0);
            }
            s.win(0, true, w);
        }
    }
    if human_arrived(s, ID, w) {
        let r = super::rnd(w, 1, 10);
        let t = random_tech(w, en, 0);
        if t == 0 && r > 5 {
            super::rnd(w, 1, 5);
        }
        if t != 0 {
            anim(w, 0x55c);
            s.set_tech_state(t, en, w);
            message(w, 0x55c, 0);
            param(w, 1, MessageText::Tech(t));
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 139: settlers (the story buildings of id 1391) on a free planet: the nearest free one outside the radar is
/// searched (none → abort) but a random free planet the human sees is taken (`st_bSearch(0, 0)`, the source's
/// slip), noticed by the radar. Seen while free: the human gets it (message 0x56f; with `v(1395) == 1`, which
/// nothing sets: 3 in 10 lost, message 0x576, else 0x575 and the planet); the end either way. Taken (daily or a
/// colonisation) → the end. Message 0x570 (never sent): answer 2 → 26 days later the planet (0x572).
pub(super) fn settlers(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x56f;
    const ASK: i32 = 0x570;
    let en = w.human();
    if s.starting_now() {
        let (mut best, mut pick) = (10_000, 0);
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) != 0 || w.planet_visibility(p) < 0 {
                continue;
            }
            let (x, y) = w.planet_xy(p);
            if w.in_radar_range(x as i32, y as i32, 3) {
                continue;
            }
            let d = human_distance(p, &*w);
            if d < best {
                (pick, best) = (p, d);
            }
        }
        if pick == 0 {
            return Err(Abort);
        }
        let p = s.b_search_try(0, 0, w)?;
        s.b_add_id(p, ID);
        s.auto_add_building(ID, w);
        s.radar_notice(ID, w);
        s.v_set(ID, 0);
    }
    if radar_planet(s, ID, w) {
        if owned(s, ID, w) {
            s.win(0, true, w);
        } else if s.v_is_equal(0x573, 1) {
            s.sz_stop(0x575);
            s.sz_stop(0x573);
            s.v_set(0x573, 0);
            if super::rnd(w, 1, 10) < 4 {
                message(w, 0x576, 0);
                let p = s.b_search(ID, 0, w);
                param(w, 1, MessageText::Planet(p));
                s.win(0, true, w);
            } else {
                message(w, 0x575, 0);
                let p = s.b_search(ID, 0, w);
                param(w, 1, MessageText::Planet(p));
                let p = s.b_search(ID, 0, w);
                s.set_planet_owner(p, en, w);
                s.win(0, true, w);
            }
        } else {
            message(w, ID, 0);
            let p = s.b_search(ID, 0, w);
            s.set_planet_owner(p, en, w);
            s.win(0, true, w);
            let p = s.b_search(ID, 0, w);
            param(w, 1, MessageText::Planet(p));
        }
    }
    if (s.daily() || s.special_is(10, 0, 0, 0)) && owned(s, ID, w) {
        if s.v_get(ID) != 0 {
            w.event(StoryEvent::RemoveMessage(ASK));
        }
        s.win(0, true, w);
    }
    if answered(s, ASK, 2) {
        s.v_set(ID, 0);
        s.sz_start(ID, 26);
    }
    if answered(s, ASK, 3) || answered(s, ASK, 4) || promoted(s, ASK) {
        s.win(0, true, w);
    }
    if s.counter(ID) {
        message(w, 0x572, 0);
        let p = s.b_search(ID, 0, w);
        s.set_planet_owner(p, en, w);
        s.win(0, true, w);
    }
    Ok(())
}
