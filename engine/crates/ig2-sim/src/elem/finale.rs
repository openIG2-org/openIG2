//! `st_Elem_include`, part 2g: 844 (the envoy and the raid on an empire's planets), 856, 862 and 863. Spec:
//! `re/rules/campaign/elem.md` §8.14, part 2g.

use super::spies::{battle_starts_with, empire_distance};
use super::threats::dist;
use super::{answered, message, read};
use crate::begin::{delete_fleet, move_to_planet};
use crate::brnd::{nearest_fleet, random_empire};
use crate::multi::{empire_centre, nearest_planet};
use crate::rnd::cvtt;
use crate::story::{flag, Abort, InfoLine, MessageText, Notice, Story, StoryEvent, StoryWorld, STREAM};

fn move_fleet(w: &mut dyn StoryWorld, fleet: i32, kind: i32, mode: i32, x: f32, y: f32) {
    w.event(StoryEvent::MoveFleet { fleet, kind, mode, x, y });
}

/// `st_Letilt(b, 0)` (exe 0x5eb6b0): message `b` asks which of its answers to disable (flag 8).
fn disabling(s: &Story, b: i32) -> bool {
    s.flags[flag::DISABLED] && s.message.0 == b
}

fn param(w: &mut dyn StoryWorld, slot: i32, text: MessageText) {
    w.event(StoryEvent::MessageParam { slot, text });
}

// ---------------------------------------------------------------- 844: the envoy and the raid

/// 844's report: `M(id)` with the two empires' names, the envoy gone, the raid 90 days on.
fn envoy_report(s: &mut Story, id: i32, w: &mut dyn StoryWorld) {
    message(w, id);
    let a = s.v_get(8449);
    param(w, 1, MessageText::Empire(a));
    let b = s.v_get(8448);
    param(w, 2, MessageText::Empire(b));
    delete_fleet(s, 8443, w);
    s.sz_start(8441, 90);
}

/// 844 (exe block 0x5e0d66): an envoy of empire 10 crosses the human's realm; 90 days after it arrives (or loses a
/// battle) empire 9 raids empire `v(8448)`'s planets from empire `v(8449)`'s.
pub(super) fn story_844(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let a = random_empire(w, 0, 0, 0)?;
        s.v_set(8449, a);
        let b = random_empire(w, a, 0, 0)?;
        s.v_set(8448, b);
        message(w, 8441);
        s.f_new_ex(crate::story::FleetName::Text(8443), 10, 8443, 8.0, 1, 0, w);
        s.f_add_ship(8443, 3, 3, w);
        s.f_add_ship(8443, 2, 10, w);
        let (cx, cy) = empire_centre(w.human(), &*w);
        let f = s.f_search(8443, w);
        let (fx, fy) = w.fleet_xy(f);
        // The point mirrored through the human's centre (f32).
        move_fleet(w, f, 1, 0, cx * 2.0 - fx, cy * 2.0 - fy);
    }
    if s.arrived_point(8443) {
        envoy_report(s, 8442, w);
    }
    if battle_starts_with(s, 8443, &*w) {
        s.set_adat_i32(0x94, 0);
    }
    if s.battle_lost(8443) {
        envoy_report(s, 8443, w);
    }
    if s.counter(8441) {
        let b = s.v_get(8448);
        if !w.empire_alive(b) {
            return Err(Abort);
        }
        let a = s.v_get(8449);
        if !w.empire_alive(a) {
            return Err(Abort);
        }
        let a = s.v_get(8449);
        let p = s.b_search(-a, 0, w);
        let f = s.f_new_ex(crate::story::FleetName::Text(8441), 9, 8441, 6.0, 2, p, w);
        let b = s.v_get(8448);
        let q = s.b_search(-b, 0, w);
        s.b_add_id(q, 8441);
        move_fleet(w, f, 1, 1, q as f32, 0.0);
        s.v_set(8441, 1);
    }
    if battle_starts_with(s, 8441, &*w) {
        s.set_adat_i32(0x94, 1);
        s.set_adat_i32(0x98, 1);
    }
    if s.battle_lost(8441) {
        message(w, 8444);
        w.event(StoryEvent::AddMoney { empire: w.human(), amount: 100_000.0 });
        s.win(0, true, w);
    }
    if s.daily() && s.v_get(8441) != 0 {
        let f = s.f_search(8441, w);
        if f == 0 {
            s.win(0, true, w);
        } else {
            let b = s.v_get(8448);
            if w.planets_of(b) < 1 {
                delete_fleet(s, -f, w);
                s.win(0, true, w);
            } else if w.fleet_command(f) != 1
                || w.fleet_order(f) != 1
                || w.planet_owner(cvtt(w.fleet_target(f) as f64)) != s.v_get(8448)
            {
                let mut q = s.b_search(8441, 0, w);
                if w.planet_owner(q) != s.v_get(8448) {
                    s.b_remove_id(8441);
                    let b = s.v_get(8448);
                    q = s.b_search(-b, 0, w);
                    s.b_add_id(q, 8441);
                }
                move_fleet(w, f, 1, 1, q as f32, 0.0);
            }
        }
    }
    if s.arrived_planet(8441, 0, w) {
        delete_fleet(s, 8441, w);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 856: the toll of planet 8560

/// 856's fighter losses: the chance in 100 of losing one fighter of class 1..=4 (exe 0x85e5b0).
const LOSS: [i32; 4] = [92, 80, 70, 50];

/// The held fleet (story id 8562) goes free: `+0x18` bit 0 off, `+0x16 := 0`.
fn release(w: &mut dyn StoryWorld, f: i32) {
    w.event(StoryEvent::FleetState { fleet: f, set: 0, clear: 1 });
    w.event(StoryEvent::SetFleetId { fleet: f, id: 0 });
}

/// The held fleet (`st_fSearch(8562)`; none aborts), released, `v(8562) := 0`.
fn take_held(s: &mut Story, w: &mut dyn StoryWorld) -> Result<i32, Abort> {
    let f = s.f_search(8562, w);
    if f == 0 {
        return Err(Abort);
    }
    release(w, f);
    s.v_set(8562, 0);
    Ok(f)
}

/// Fleet `f`'s fighter records (`Tankok`, design `+0x36 ≠ 0`): each loses `rnd(5, 0, 99) < LOSS[class − 1]` of at
/// most `pool` of its fighters (negative: all; the pool shrinks by the fighters drawn for); the lost ones go
/// (`TankDelete`, called for 0 too). The fighters lost.
fn lose_fighters(f: i32, mut pool: i32, w: &mut dyn StoryWorld) -> i32 {
    let mut total = 0;
    let mut k = 1;
    while k <= w.tank_count() {
        let (place, design, count) = w.tank(k);
        let owner = w.fleet_owner(f);
        if design != 0x7fff && place == f && w.design_36(owner, design) != 0 {
            // A class outside 1..=4 reads the stack next to the exe's table (never seen).
            let q = LOSS.get((w.design_44(owner, design) - 1) as usize).copied().unwrap_or(0);
            let mut n = count;
            if pool >= 0 {
                n = count.min(pool);
                pool -= n;
            }
            let mut lost = 0;
            for _ in 0..n.max(0) {
                if w.rng().range(STREAM, 0, 99) < q {
                    lost += 1;
                }
            }
            w.event(StoryEvent::DeleteTanks { place: f, design, count: lost });
            total += lost;
        }
        k += 1;
    }
    total
}

/// Fleet `f`'s ships (`Hajok`): each draws `rnd(5, 0, 99)` and, under 65 while fewer than `most` went, goes
/// (`HajoDelete`). The ships lost.
fn lose_ships(f: i32, most: i32, w: &mut dyn StoryWorld) -> i32 {
    let mut lost = 0;
    let mut k = 1;
    while k <= w.ship_count() {
        let (fleet, design) = w.ship(k);
        if design != 0x7fff && fleet == f && w.rng().range(STREAM, 0, 99) < 65 && lost < most {
            w.event(StoryEvent::DeleteShip(k));
            lost += 1;
        }
        k += 1;
    }
    lost
}

/// The end of a toll paid in units: no fighters left (`+0x58`, not recounted) → `M(8559)`; else the video, `M(id)` with the counts, one of
/// the two story designs for the human (`rnd(5, 0, 1)`: 0 → 8562) and `WIN`.
fn toll_end(s: &mut Story, f: i32, id: i32, counts: &[i32], w: &mut dyn StoryWorld) {
    if w.fleet_fighters(f) == 0 {
        message(w, 8559);
        return;
    }
    w.event(StoryEvent::Anim(8566));
    message(w, id);
    for (slot, &value) in (1..).zip(counts) {
        w.event(StoryEvent::MessageParamInt { slot, value });
    }
    let key = if w.rng().range(STREAM, 0, 1) == 0 { 8562 } else { 8561 };
    let en = w.human();
    w.story_design(en, key, 3);
    s.win(0, true, w);
}

/// 856 (exe block 0x5e30e2): the toll of planet 8560. The human pays (or refuses); 1200 days on, empire 5's raider
/// comes and hunts the human's fleets near the planet; a fleet of the human's arriving there is held until it pays in
/// fighters or ships.
pub(super) fn story_856(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        s.sz_start(8560, 130);
        message(w, 8560);
        s.b_set_extra(8560, true, w);
    }
    if s.special_is(10, 0, 0, 0) && s.special[2] == s.b_search(8560, 0, w) {
        let p = s.b_search(8560, 0, w);
        s.set_planet_owner(p, 0, w);
        if s.special[1] == en {
            w.event(StoryEvent::Notice(Notice::StoryText(8560)));
        }
    }
    if s.counter(8560) {
        message(w, 8561);
        w.event(StoryEvent::Anim(8561));
        s.sz_start(8561, 330);
    }
    if s.counter(8561) {
        message(w, 8562);
    }
    // `st_oldLetilt(8562, 0)` (exe 0x48fce0) returns 0: its money tests never run.
    let mut paid = false;
    for (answer, price) in [(2, 50_000), (3, 30_000)] {
        if answered(s, 8562, answer) {
            w.event(StoryEvent::AddMoney { empire: en, amount: -price as f64 });
            paid = true;
            s.v_set(8560, 1);
            s.sz_start(8569, 1200);
        }
    }
    let refused = answered(s, 8562, 4);
    if refused {
        s.sz_start(8569, 1200);
    }
    if refused || paid {
        let p = s.b_search(8560, 1, w);
        s.b_show(8560, -1, w);
        s.add_info_line(8561, InfoLine::Planet { planet: p, count: None }, -1, w);
    }
    if s.counter(8569) && w.empire_alive(5) {
        s.f_new_ex(crate::story::FleetName::Text(8561), 5, 8561, 0.08, 2, 0, w);
        let p = s.b_search(8560, 0, w);
        let a = w.rng().range(STREAM, -10, 10);
        let b = w.rng().range(STREAM, -10, 10);
        let f = s.f_search(8561, w);
        let (px, py) = w.planet_xy(p);
        // The first draw goes to y, the second to x (the ELF has them the other way round).
        move_fleet(w, f, 1, 0, b as f32 + px, a as f32 + py);
        message(w, 8569);
    }
    if s.arrived_point(8561) && s.v_get(8561) == 0 {
        s.v_set(8561, 1);
    }
    if s.daily() && s.v_get(8561) != 0 {
        let f = s.f_search(8561, w);
        if f == 0 {
            s.v_set(8561, 0);
        } else {
            let p = s.b_search(8560, 0, w);
            let n = w.fleet_count();
            if n >= 1 {
                let mut near = 1000;
                for g in 1..=n {
                    if w.fleet_owner(g) == en {
                        near = near.min(cvtt(dist(w.fleet_xy(g), w.planet_xy(p)) as f64));
                    }
                }
                // The target is the loop's end: the slot past the last fleet.
                if near < 40 {
                    move_fleet(w, f, 2, 2, (n + 1) as f32, 0.0);
                }
            }
        }
    }
    if s.arrived_planet(0, 8560, w) {
        let f = s.arrival.fleet;
        if w.fleet_owner(f) == en && s.v_get(8562) == 0 {
            w.event(StoryEvent::SetFleetId { fleet: f, id: 8562 });
            w.event(StoryEvent::FleetState { fleet: f, set: 1, clear: 0 });
            let id = if s.v_get(8560) != 1 {
                8567
            } else if w.fleet_fighters(f) != 0 {
                8563
            } else {
                8564
            };
            message(w, id);
            s.v_set(8562, 1);
        }
    }
    if disabling(s, 8563) {
        let f = s.f_search(8562, w);
        if f != 0 {
            if w.fleet_fighters(f) < 5 {
                w.event(StoryEvent::DisablePart(5));
            }
            if w.fleet_fighters(f) < 20 {
                w.event(StoryEvent::DisablePart(6));
            }
        }
    }
    for (answer, level) in [(5, 1), (6, 2), (7, 3)] {
        if answered(s, 8563, answer) {
            s.v_set(8563, level);
            s.sz_start(8563, 15);
        }
    }
    if answered(s, 8563, 3) {
        s.sz_start(8565, 15);
    }
    if answered(s, 8563, 4) || answered(s, 8564, 2) || answered(s, 8567, 3) {
        s.v_set(8562, 0);
        let f = s.f_search(8562, w);
        if f != 0 {
            release(w, f);
        }
    }
    if answered(s, 8564, 3) {
        s.sz_start(8564, 15);
    }
    if answered(s, 8567, 2) {
        let f = s.f_search(8562, w);
        if f != 0 {
            let c = if w.fleet_fighters(f) != 0 { 8565 } else { 8564 };
            s.sz_start(c, 15);
        }
    }
    if s.counter(8563) {
        let f = take_held(s, w)?;
        // v(8563) is read a second time unless it is 1.
        let pool = if s.v_get(8563) == 1 {
            5
        } else if s.v_get(8563) == 2 {
            20
        } else {
            -1
        };
        let lost = lose_fighters(f, pool, w);
        toll_end(s, f, 8566, &[lost], w);
    }
    if s.counter(8564) {
        let f = take_held(s, w)?;
        let most = w.rng().range(STREAM, 5, 10);
        let lost = lose_ships(f, most, w);
        message(w, 8565);
        w.event(StoryEvent::MessageParamInt { slot: 1, value: lost });
    }
    if s.counter(8565) {
        let f = take_held(s, w)?;
        let most = w.rng().range(STREAM, 5, 10);
        let ships = lose_ships(f, most, w);
        let fighters = lose_fighters(f, -1, w);
        toll_end(s, f, 8568, &[fighters, ships], w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 862: empire 1 takes empire 5's planet

/// `distsq(x1, y1, x2, y2)` (exe 0x5c9bc0): `dy² + dx²` in f32.
fn dist_sq(a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    dy * dy + dx * dx
}

/// `make_point_in_dist(&x, &y, x0, y0, x1, y1, d)` (exe 0x5c9bf0): the point `d` from `(x0, y0)` towards `(x1, y1)`
/// (f32, the length `(f32) sqrt((f64)(dy² + dx²))`).
fn point_in_dist(a: (f32, f32), b: (f32, f32), d: f32) -> (f32, f32) {
    let (x0, y0, x1, y1) = (a.0, a.1, b.0, b.1);
    let len = (((y0 - y1) * (y0 - y1) + (x0 - x1) * (x0 - x1)) as f64).sqrt() as f32;
    (((x1 - x0) * d) / len + x0, ((y1 - y0) * d) / len + y0)
}

/// `st_fNewPos(_s(5, id), owner, id, x, y, speed)` (exe 0x605010): `FlottaNew` with `speed ≤ 1 ? 100·speed : speed`,
/// then `AutoAddship(f, id)`.
fn fleet_at(s: &mut Story, owner: i32, id: i32, x: i32, y: i32, speed: f32, w: &mut dyn StoryWorld) {
    let speed = if speed <= 1.0 { speed * 100.0 } else { speed };
    let f = w.new_fleet(&crate::story::FleetName::Text(id), owner, id, x as f32, y as f32, speed);
    s.auto_add_ship(f, id, w);
}

/// 862 (exe block 0x5e5212): empire 1 at war with empire 5 and allied with the human; its escort (8623) takes empire
/// 5's poorest planet (8620) while empire 5's raiders (8621) guard it, then hunt the human's fleets.
pub(super) fn story_862(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        if !w.empire_alive(1) || !w.empire_alive(5) {
            return Err(Abort);
        }
        s.set_adat_i32(0xbc, 0);
        s.set_adat_i32(0xb8, 0);
        w.event(StoryEvent::DeclareWar { a: 1, b: 5 });
        s.v_set(8651, 1);
        // Empire 5's planet the human sees with the lowest value (`st_BolygoErtek`; the first of equals).
        let mut p = 0;
        for q in 1..=w.planet_count() {
            if w.planet_owner(q) == 5 && w.planet_visibility(q) >= 0 && (p == 0 || w.planet_value(p) > w.planet_value(q)) {
                p = q;
            }
        }
        if p == 0 {
            return Err(Abort);
        }
        s.b_add_id(p, 8620);
        s.b_show(8620, -1, w);
        s.f_new_ex(crate::story::FleetName::Text(8621), 5, 8621, 0.12, -p, 0, w);
        let d = w.story_design(5, 8621, 3);
        s.f_add_ship(8621, d, 50, w);
        message(w, 8620);
        param(w, 1, MessageText::Planet(p));
        s.b_set_extra(8620, true, w);
        w.event(StoryEvent::MakeAllies { a: en, b: 1 });
    }
    if read(s, 8620) {
        let p = s.b_search(8620, 0, w);
        s.add_info_line(8621, InfoLine::Planet { planet: p, count: None }, -1, w);
        s.v_set(8628, 1);
    }
    if s.daily() {
        if s.b_search(-1, 1, w) == 0 && s.f_search(-1, w) == 0 && s.v_get(8621) == 0 {
            s.v_set(8621, 1);
            s.remove_fleet_id(8621, w);
            s.win(0, true, w);
        } else if s.b_search(-5, 0, w) == 0 && s.f_search(-5, w) == 0 {
            s.win(0, true, w);
        } else {
            let f = s.f_search(8621, w);
            let p = s.b_search(8620, 0, w);
            if f == 0 && s.v_get(8628) != 0 {
                s.remove_game_info(8621, w);
                s.v_set(8628, 0);
            }
            if (f == 0 || dist(w.fleet_xy(f), w.planet_xy(p)) > 100.0) && s.v_get(8621) < 1 {
                s.v_set(8621, 2);
                let from = if f == 0 { (0.0, 0.0) } else { w.fleet_xy(f) };
                // 50 beyond the planet, away from the raiders.
                let (x, y) = point_in_dist(w.planet_xy(p), from, -50.0);
                fleet_at(s, 1, 8623, cvtt(x as f64), cvtt(y as f64), 0.07, w);
                let g = s.f_search(8623, w);
                if g >= 1 {
                    w.event(StoryEvent::FleetFlags { fleet: g, set: 0x200, clear: 0 });
                }
                s.error = (g < 1) as i32;
                s.f_add_ship(8623, 3, 20, w);
                s.f_add_tank(8623, 1, 20, w);
                move_to_planet(s, 8623, 8620, w);
                s.attack_fleet(8621, 8623, w);
            }
            if f == 0 || s.v_get(8621) > 1 {
                if s.v_get(8621) == 3 {
                    if f == 0 {
                        s.win(0, true, w);
                    } else {
                        let (fx, fy) = w.fleet_xy(f);
                        let g = nearest_fleet(cvtt(fx as f64), cvtt(fy as f64), en, &*w);
                        if g != 0 {
                            move_fleet(w, f, 2, 2, g as f32, 0.0);
                        }
                    }
                }
            } else {
                let (mut best, mut near) = (0x640, 0);
                for g in 1..=w.fleet_count() {
                    if w.fleet_owner(g) == en {
                        let d = cvtt(dist_sq(w.fleet_xy(f), w.fleet_xy(g)) as f64);
                        if d < best {
                            near = g;
                            best = d;
                        }
                    }
                }
                if near == 0 {
                    let (x, y) = point_in_dist(w.planet_xy(p), w.fleet_xy(f), 30.0);
                    move_fleet(w, f, 1, 0, x, y);
                } else {
                    move_fleet(w, f, 2, 2, near as f32, 0.0);
                }
            }
        }
    }
    if s.arrived_planet(8623, 8620, w) {
        delete_fleet(s, 8623, w);
        s.v_set(8621, 3);
        let p = s.b_search(8620, 0, w);
        s.set_planet_owner(p, 1, w);
        s.b_set_extra(8620, false, w);
        let p = s.b_search(8620, 0, w);
        w.event(StoryEvent::RemovePlanetInfo { planet: p, info: -8620 });
        let f = s.f_search(8621, w);
        if f != 0 {
            w.event(StoryEvent::SetFleetSpeed { fleet: f, speed: 0.09 });
            let from = w.story_design(5, 8621, 3);
            let to = w.story_design(5, 8622, 3);
            for k in 0..w.ship_count() {
                if w.ship(k) == (f, from) {
                    w.event(StoryEvent::UpgradeShip { ship: k, design: to as i16 as i32 });
                }
            }
            w.event(StoryEvent::RecalcFleets);
        }
        message(w, 8621);
        s.remove_game_info(8621, w);
    }
    if s.ground_start(0, 8620, w) {
        s.ground_forbidden = true;
        w.event(StoryEvent::DeleteFleet(s.ground.fleet));
        if s.v_get(8622) == 0 {
            s.v_set(8622, 1);
            message(w, 8624);
            // `SetParamStr(8624, …)`: a slot past 5, nothing is written.
            let p = s.b_search(8622, 0, w);
            param(w, 8624, MessageText::Planet(p));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 863: empire 5's end

/// `st_bGet4Brutal()` (exe 0x6035a0): with at least three planets of the human's, the four planets of empire 5 the
/// human sees (1..`HanyBolygo` − 1) nearest to the human's (`st_bDistFromBir(p, EN)`), kept in a static that is never
/// cleared; `None` while one of its four is 0.
fn brutal4(s: &mut Story, w: &dyn StoryWorld) -> Option<[i32; 4]> {
    let en = w.human();
    if w.planets_of(en) < 3 {
        return None;
    }
    let mut l = [0i32; 4];
    for p in 1..w.planet_count() {
        if w.planet_owner(p) != 5 || w.planet_visibility(p) < 0 {
            continue;
        }
        let d = empire_distance(p, en, w);
        // The last empty slot, else the first of the farthest.
        let mut k = 0;
        for j in 1..4 {
            if l[j] == 0 || (l[k] != 0 && l[k] < l[j]) {
                k = j;
            }
        }
        if l[k] == 0 || d < l[k] {
            l[k] = d;
            s.brutal4[k] = p;
        }
    }
    s.brutal4.iter().all(|&p| p != 0).then_some(s.brutal4)
}

/// 863's target: `p` gets id `id`, its extra flag (undone with the story) and a message with its name.
fn mark_target(s: &mut Story, p: i32, w: &mut dyn StoryWorld) {
    s.b_add_id(p, 8633);
    s.b_set_extra(-p, true, w);
    s.add_undo(8, p, 863);
    message(w, 8633);
    param(w, 1, MessageText::Planet(p));
    s.sz_start(8633, 20);
}

/// Empire 5's planet nearest to the human's centre (`st_bGetNearest`; none aborts).
fn nearest_of_5(w: &dyn StoryWorld) -> Result<i32, Abort> {
    let (x, y) = empire_centre(w.human(), w);
    match nearest_planet(cvtt(x as f64), cvtt(y as f64), 5, w) {
        0 => Err(Abort),
        p => Ok(p),
    }
}

/// 863 (exe block 0x5e5b4f): empire 5's end. A year on, the human pays empire 1 500,000 (or can't); empire 5's
/// planets nearest to the human are marked for conquest wave by wave; empire 5 dead wins the campaign.
pub(super) fn story_863(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        if !w.empire_alive(1) || !w.empire_alive(5) {
            return Err(Abort);
        }
        s.v_set(8639, 1);
        if s.b_search(-5, 1, w) == 0 {
            s.win(0, true, w);
        } else {
            // Empire 1's planet the human sees nearest to empire 5's (the first of equals).
            let (mut p, mut best) = (0, 0);
            for q in 1..=w.planet_count() {
                if w.planet_owner(q) == 1 && w.planet_visibility(q) >= 0 {
                    let d = empire_distance(q, 5, &*w);
                    if p == 0 || d < best {
                        p = q;
                        best = d;
                    }
                }
            }
            if p == 0 {
                s.win(0, true, w);
            } else {
                s.b_add_id(p, 8631);
                message(w, 8630);
                s.sz_start(8630, 360);
                let [y, m, d] = w.date();
                s.add_info_line(8631, InfoLine::Date([y + 1, m, d]), -1, w);
            }
        }
    }
    if s.counter(8630) {
        s.remove_game_info(8631, w);
        if w.money(en) <= 500_000.0 {
            message(w, 8632);
            s.sz_start(8632, 40);
        } else {
            message(w, 8631);
            s.sz_start(8631, 80);
            w.event(StoryEvent::AddMoney { empire: en, amount: -500_000.0 });
            s.f_new_ex(crate::story::FleetName::Text(8638), 1, 8631, 0.09, 2, 0, w);
            s.remove_fleet_id(8631, w);
        }
    }
    if s.counter(8631) {
        let n = w.planets_of(5);
        if n < 3 {
            if n < 1 {
                s.win(0, true, w);
            } else {
                s.sz_start(8633, 20);
            }
        } else {
            let p = nearest_of_5(&*w)?;
            mark_target(s, p, w);
            s.add_info_line(8632, InfoLine::Planet { planet: p, count: None }, -1, w);
        }
    }
    if s.counter(8632) {
        let n = w.planets_of(5);
        if n < 6 {
            if n < 3 {
                s.sz_start(8632, 20);
            } else {
                let p = nearest_of_5(&*w)?;
                mark_target(s, p, w);
                s.v_set(8633, 0);
                s.add_info_line(8632, InfoLine::Planet { planet: p, count: None }, -1, w);
            }
        } else {
            let q = brutal4(s, &*w).ok_or(Abort)?;
            message(w, 8634);
            for (k, &p) in (0..).zip(&q) {
                param(w, k + 2, MessageText::Planet(p));
                // The arguments the other way round: planet 8632 + k (its low byte) gets the id `p`.
                s.b_add_id(8632 + k, p);
                s.b_set_extra(-p, true, w);
                s.add_undo(8, p, 863);
            }
            s.add_info_line(8633, InfoLine::Planets(q), -1, w);
            s.sz_start(8634, 20);
            s.v_set(8634, 1);
        }
    }
    if s.counter(8633) || (s.ground_part(0, 0, w) && s.sz_get(8633) != 0) {
        let p = s.b_search(8633, 0, w);
        if w.planet_owner(p) == 5 {
            s.sz_start(8633, 20);
        } else {
            message(w, 8635);
            s.sz_start(8635, 20);
            s.v_set(8633, 0);
            s.remove_game_info(8632, w);
        }
    }
    if s.counter(8634) || (s.ground_part(0, 0, w) && s.sz_get(8634) != 0) {
        let mut held = false;
        for id in 8632..=8635 {
            let p = s.b_search(id, 0, w);
            if w.planet_owner(p) == 5 {
                held = true;
                break;
            }
        }
        if held {
            s.sz_start(8634, 20);
        } else {
            if !w.empire_alive(5) {
                s.sz_start(8635, 1);
            } else {
                message(w, 8635);
                s.remove_game_info(8633, w);
                s.sz_start(8635, 20);
            }
            s.v_set(8634, 0);
        }
    }
    if s.daily() && s.v_get(8639) != 0 && !w.empire_alive(5) {
        message(w, 8636);
        s.sz_start(8636, 10);
        s.v_set(8639, 0);
    }
    if s.counter(8636) {
        message(w, 8637);
        s.sz_start(8638, 2);
    }
    if s.counter(8638) {
        w.event(StoryEvent::AnimFile("politikus_win.bik"));
        w.event(StoryEvent::AnimFile("misc/end_campaign_win.bik"));
        w.event(StoryEvent::CampaignOver);
        s.win(0, true, w);
    }
    Ok(())
}
