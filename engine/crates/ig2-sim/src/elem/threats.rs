//! `st_Elem_include`, part 2e: 814 (empire 10's doomsday base), 815 (empire 12's forbidden zone), 821 (the pirates'
//! convoy hunt), 832 (empire 13's deadline) and 834 (empire 1's siege of planet 8340). Spec: `re/rules/campaign.md`
//! §8.14, part 2e.

use super::{add_money, answered, param, promoted, read};
use crate::begin::{delete_fleet, move_to_planet};
use crate::brnd::nearest_fleet;
use crate::rnd::cvtt;
use crate::story::{adat, flag, Abort, InfoLine, MessageText, Notice, Story, StoryEvent, StoryWorld, STREAM};

fn message(w: &mut dyn StoryWorld, id: i32) {
    w.event(StoryEvent::NewMessage { id, arg: 0 });
}

/// `st_ViszonyValt(EN, e, x)`: `Viszony[e][EN].+4 += 100·x`.
fn relation(w: &mut dyn StoryWorld, e: i32, x: i32) {
    let a = w.human();
    w.event(StoryEvent::RelationChange { a, b: e, delta: 100 * x });
}

fn move_fleet(w: &mut dyn StoryWorld, fleet: i32, kind: i32, mode: i32, x: f32, y: f32) {
    w.event(StoryEvent::MoveFleet { fleet, kind, mode, x, y });
}

/// `(f32) sqrt((f64)(dy² + dx²))` of f32 coordinates.
pub(super) fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    ((dy * dy + dx * dx) as f64).sqrt() as f32
}

// ---------------------------------------------------------------- 814: the doomsday base

/// 814 (exe block 0x5d9d93): empire 10 holds planet 8140 and its doomsday device (building `'+'`); a ground battle
/// there arms the battlefield's self-destruct countdown. The message's answers buy the pirates (empire 9) off.
pub(super) fn story_814(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let p = s.b_search(8140, 0, w);
        s.set_planet_owner(p, 10, w);
        s.sz_start(8140, 720);
        s.auto_add_tank(8140, w);
        s.auto_add_building(8140, w);
        s.add_info_line(8141, InfoLine::Planet { planet: p, count: None }, -1, w);
        s.b_show(8140, -1, w);
    }
    if s.counter(8140) {
        s.win(0, true, w);
    }
    if s.ground_start(0, 8140, w) {
        let p = s.b_search(8140, 0, w);
        s.set_adat_i32(0x68, p);
        w.event(StoryEvent::SelfDestructTimer { planet: p, value: 3000 });
    }
    if s.ground_part(0, 8140, w) {
        let p = s.b_search(8140, 0, w);
        w.event(StoryEvent::SelfDestructTimer { planet: p, value: -1 });
        let foreign = w.planet_owner(p) != en;
        let gone = !(1..=w.building_count(p)).any(|k| {
            let (kind, _, built) = w.building(p, k);
            kind == 0x2b && built as i16 == 0
        });
        if gone {
            message(w, 8141);
            s.win(0, true, w);
        } else if foreign {
            message(w, 8142);
        } else {
            w.event(StoryEvent::RemovePlanetInfo { planet: p, info: -8140 });
            message(w, 8140);
            s.sz_stop(8140);
            s.remove_game_info(8141, w);
        }
    }
    if s.flags[flag::DISABLED] && s.message.0 == 8140 && s.adat_i32(0x58) != 0 {
        w.event(StoryEvent::DisablePart(2));
    }
    if answered(s, 8140, 2) {
        s.set_adat_i32(0x5c, 1);
        relation(w, 9, 100);
        s.set_adat_i32(0x58, 1);
        s.win(0, true, w);
    }
    if answered(s, 8140, 3) || promoted(s, 8140) {
        s.trade_cheap(true, w);
        relation(w, 9, 80);
    }
    if answered(s, 8140, 4) {
        add_money(w, 500_000, -1);
        relation(w, 9, 90);
        s.add_info_line(8140, InfoLine::Blank, -1, w);
        s.sz_start(8141, 30);
    }
    if s.counter(8141) {
        let x = w.rng().range(STREAM, 100, 200) * 100;
        w.event(StoryEvent::ScrollNumber { text: 8140, a: x });
        add_money(w, x, -1);
        s.sz_start(8141, 30);
    }
    Ok(())
}

// ---------------------------------------------------------------- 815: the forbidden zone

/// `StoryAdat` offsets of 815: the zone's switch (a byte), the "entered" flag, the triangle's x's and y's (f32).
const ZONE_ON: usize = 0;
const ZONE_ENTERED: usize = 4;
const ZONE_X: usize = 0x08;
const ZONE_Y: usize = 0x14;

/// `benne(ax, ay, bx, by, cx, cy, px, py)` (exe 0x44f8c0): `(px, py)` lies inside the triangle (f32; every edge's
/// cross product on the same side as the triangle's own, none on an edge; a degenerate triangle holds nothing).
pub fn in_triangle(a: (f32, f32), b: (f32, f32), c: (f32, f32), p: (f32, f32)) -> bool {
    let s = (c.1 - b.1) * (a.0 - b.0) - (c.0 - b.0) * (a.1 - b.1);
    if s == 0.0 {
        return false;
    }
    let side = s > 0.0;
    let t1 = (p.1 - b.1) * (a.0 - b.0) - (p.0 - b.0) * (a.1 - b.1);
    if (t1 > 0.0) != side {
        return false;
    }
    let t2 = (a.0 - p.0) * (c.1 - p.1) - (a.1 - p.1) * (c.0 - p.0);
    if (t2 > 0.0) != side {
        return false;
    }
    let t3 = (p.0 - b.0) * (c.1 - b.1) - (p.1 - b.1) * (c.0 - b.0);
    (t3 > 0.0) == side
}

/// 815's triangle: the three planets' corners pulled a fifth towards their centre, `(f32)((f64)(8·x_k + 2·c) / 10)`
/// with `c = (f32)((f64)(x₀ + x₁ + x₂) / 3)` (the sums in f32).
pub fn zone_corners(xs: [f32; 3]) -> [f32; 3] {
    let c = ((xs[0] + xs[1] + xs[2]) as f64 / 3.0) as f32 * 2.0;
    xs.map(|x| ((x * 8.0 + c) as f64 / 10.0) as f32)
}

fn adat_f32(s: &Story, off: usize) -> f32 {
    f32::from_bits(s.adat_i32(off) as u32)
}

/// 815 (exe block 0x5da289): empire 12's planets 8150..=8152 are hidden; any fleet of the human's inside their
/// triangle is destroyed (two warnings, then tech 177 to find them); with the tech they show up and the story ends
/// when one of them is invaded.
pub(super) fn story_815(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        s.add_info_line(8151, InfoLine::Blank, -1, w);
        s.adat[ZONE_ON] = 1;
        s.v_set(8158, 1);
        s.set_adat_i32(ZONE_ENTERED, (w.campaign_type() == 3) as i32);
        let ps = [8150, 8151, 8152].map(|id| s.b_search(id, 1, w));
        for p in ps {
            s.set_planet_owner(p, 12, w);
            w.event(StoryEvent::SetPlanetView { planet: p, view: -1 });
        }
        for id in [8150, 8151, 8152] {
            s.auto_add_tank(id, w);
        }
        let xy = ps.map(|p| w.planet_xy(p));
        let xs = zone_corners(xy.map(|q| q.0));
        let ys = zone_corners(xy.map(|q| q.1));
        for k in 0..3 {
            s.set_adat_i32(ZONE_X + 4 * k, xs[k].to_bits() as i32);
            s.set_adat_i32(ZONE_Y + 4 * k, ys[k].to_bits() as i32);
        }
    }
    if s.daily() && s.adat[ZONE_ON] != 0 && s.v_get(8158) != 0 {
        let corner = |k: usize| (adat_f32(s, ZONE_X + 4 * k), adat_f32(s, ZONE_Y + 4 * k));
        let (a, b, c) = (corner(0), corner(1), corner(2));
        // The count is re-read every pass (`FlottaDelete` may shrink it).
        let mut f = 0;
        while f < w.fleet_count() {
            f += 1;
            let owner = w.fleet_owner(f);
            if owner == 0xff || !in_triangle(a, b, c, w.fleet_xy(f)) {
                continue;
            }
            if owner == en && s.v_get(8159) != 1 {
                let warning = if s.v_get(8150) == 1 { 8150 } else { 8151 };
                s.sz_start(warning, 1);
                s.v_set(8159, 1);
                s.set_adat_i32(ZONE_ENTERED, 1);
            }
            if owner == en {
                w.event(StoryEvent::DeleteFleet(f));
            }
        }
    }
    if s.counter(8151) {
        message(w, 8151);
        s.v_set(8150, 1);
        s.v_set(8159, 0);
    }
    if s.counter(8150) {
        message(w, 8152);
        s.set_tech_state_to(177, 3, en, w);
        param(w, 1, MessageText::Tech(177));
        s.v_set(8152, 1);
        s.remove_game_info(8151, w);
        s.add_info_line(8152, InfoLine::Blank, -1, w);
    }
    if s.daily() && s.v_get(8152) != 0 && w.tech_status(177, en) == 5 {
        s.remove_game_info(8152, w);
        s.v_set(8152, 0);
        message(w, 8153);
        for id in [8150, 8151, 8152] {
            s.b_unhide(id, w);
        }
        for id in [8150, 8151, 8152] {
            let p = s.b_search(id, 0, w);
            w.event(StoryEvent::SetPlanetView { planet: p, view: 1 });
        }
        s.v_set(8158, 0);
        s.finished.add(815, 2);
        if w.campaign_type() != 3 {
            s.sz_start(8159, 30);
        }
    }
    if s.counter(8159) {
        message(w, 5920);
        add_money(w, 100_000, -1);
    }
    let invaded = s.ground_part(0, 8150, w) || s.ground_part(0, 8151, w) || s.ground_part(0, 8152, w);
    if invaded {
        let lost = [8150, 8151, 8152].into_iter().any(|id| {
            let p = s.b_search(id, 0, w);
            w.planet_owner(p) != 12
        });
        if lost {
            s.adat[ZONE_ON] = 0;
            s.win(0, true, w);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 821: the convoy

/// The convoy (empire 9), the pirates hunting it and the pirates going home.
const CONVOY: i32 = 8207;
const PIRATES: i32 = 8202;
const HOMEWARD: i32 = 8203;
/// The pirates' planet (820's).
const BASE: i32 = 8200;

/// The convoy's next stop: up to 99 draws of a planet of the human's other than `p` (`v(8217)`, 0 when none).
fn next_stop(s: &mut Story, p: i32, w: &mut dyn StoryWorld) {
    let en = w.human();
    let mut n = 100;
    let q = loop {
        n -= 1;
        if n == 0 {
            break 0;
        }
        let q = s.b_search(en.wrapping_neg(), 0, w);
        if q != p {
            break q;
        }
    };
    if q != 0 {
        let f = s.f_search(CONVOY, w);
        move_fleet(w, f, 1, 1, q as f32, 0.0);
    }
    s.v_set(8217, q);
}

/// 821 (exe block 0x5db408): empire 9's convoy 8207 travels between the human's planets; the pirates of 820's
/// planet 8200 hunt it with fleet 8202 (stronger after each kill) and the human's fleets near it join the battle.
/// A lost convoy costs the human; the base taken ends the hunt.
pub(super) fn story_821(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    let mut p = 0;
    if s.starting_now() {
        s.sz_start(8219, 10);
        s.v_set(8201, 0);
        s.v_set(8211, 0);
        s.v_set(8219, 1);
        s.v_set(8218, 0);
    }
    if s.counter(8219) {
        if w.planets_of(en) < 2 {
            s.sz_start(8219, 10);
        } else {
            let t = w.rng().range(STREAM, 150, 200);
            s.sz_start(BASE, t);
            p = s.b_search(en.wrapping_neg(), 0, w);
            s.f_new_ex(&String::from_utf8_lossy(&w.story_text(8206)), 9, CONVOY, 0.06, 2, p, w);
            s.f_add_ship(CONVOY, 8, 1, w);
            // `st_fSetBarat(8207, 1)` and `st_fShow(8207)` inlined: `+0x52` bits 0x200 and 0x100.
            let f = s.f_search(CONVOY, w);
            if f >= 1 {
                w.event(StoryEvent::FleetFlags { fleet: f, set: 0x200, clear: 0 });
            }
            s.error = (f < 1) as i32;
            let f = s.f_search(CONVOY, w);
            if f >= 1 {
                w.event(StoryEvent::SetFleetVisible { fleet: f, on: true });
            }
            s.error = (f < 1) as i32;
            s.v_set(8219, 0);
        }
    }
    if s.arrived_planet(CONVOY, 0, w) {
        if s.v_get(8218) == 0 {
            p = s.arrival.target;
        } else {
            delete_fleet(s, CONVOY, w);
            s.win(0, true, w);
        }
    }
    if p != 0 && w.planets_of(en) > 1 {
        next_stop(s, p, w);
    }
    if s.daily() {
        if s.f_search(CONVOY, w) == 0 && s.v_get(8219) == 0 {
            if s.v_get(8218) != 0 {
                s.win(0, true, w);
            } else {
                s.sz_start(8219, 90);
                s.v_set(8219, 1);
            }
        } else if s.fleet_idle(CONVOY, w) {
            if s.v_get(8217) == 0 && w.planets_of(en) > 1 {
                let q = s.b_search(en.wrapping_neg(), 0, w);
                s.v_set(8217, q);
            }
            if s.v_get(8217) != 0 {
                let f = s.f_search(CONVOY, w);
                let q = s.v_get(8217);
                move_fleet(w, f, 1, 1, q as f32, 0.0);
            }
        } else {
            // Moving, or gone (slot 0's record).
            let f = s.f_search(CONVOY, w);
            if w.fleet_order(f) == 1 && w.fleet_command(f) == 1 && w.planet_owner(cvtt(w.fleet_target(f) as f64)) != en {
                let q = s.b_search(en.wrapping_neg(), 0, w);
                let f = s.f_search(CONVOY, w);
                move_fleet(w, f, 1, 1, q as f32, 0.0);
                s.v_set(8217, q);
            }
        }
        let b = s.b_search(BASE, 0, w);
        if w.planet_owner(b) != 10 {
            if s.v_get(8218) == 0 {
                s.send_away(PIRATES, w);
            }
            s.v_set(8218, 1);
        }
        if s.fleet_idle(PIRATES, w) {
            if s.f_search(CONVOY, w) == 0 {
                let f = s.f_search(PIRATES, w);
                w.event(StoryEvent::SetFleetId { fleet: f, id: HOMEWARD });
            } else {
                s.attack_fleet(PIRATES, CONVOY, w);
            }
        }
        if s.fleet_idle(HOMEWARD, w) {
            let b = s.b_search(BASE, 0, w);
            if w.planet_owner(b) == 10 {
                move_to_planet(s, HOMEWARD, BASE, w);
            } else {
                delete_fleet(s, HOMEWARD, w);
            }
        }
    }
    if s.arrived_planet(HOMEWARD, BASE, w) {
        delete_fleet(s, HOMEWARD, w);
    }
    if s.counter(BASE) && s.v_get(8201) < 3 {
        let b = s.b_search(BASE, 0, w);
        if w.planet_owner(b) == 10 {
            let b = s.b_search(BASE, 0, w);
            s.f_new_ex(&String::from_utf8_lossy(&w.story_text(8202)), 10, PIRATES, 0.08, 2, b, w);
            let n = s.v_get(8211).wrapping_mul(4).wrapping_add(3);
            s.f_add_ship(PIRATES, 3, n, w);
            s.v_set(8211, 0);
            s.f_add_ship(PIRATES, 2, 20, w);
            s.attack_fleet(PIRATES, CONVOY, w);
        }
        s.v_inc(8201);
    }
    if s.flags[flag::SPACE_START] {
        let (a, b) = w.battle_fleets();
        let (ia, ib) = (w.fleet_id(a), w.fleet_id(b));
        if (ia == PIRATES || ib == PIRATES) && (ia == CONVOY || ib == CONVOY) {
            let f = s.f_search(CONVOY, w);
            if f == 0 {
                return Err(Abort);
            }
            let (fx, fy) = w.fleet_xy(f);
            let (x, y) = (cvtt(fx as f64) as f32, cvtt(fy as f64) as f32);
            // The fleet slots up to the ship count (a slip kept: the stale records past the fleets too).
            for k in 1..=w.ship_count() {
                if w.fleet_owner(k) != en {
                    continue;
                }
                let (kx, ky) = w.fleet_xy(k);
                let (dx, dy) = (x - kx, y - ky);
                if cvtt(((dy * dy + dx * dx) as f64).sqrt() as f32 as f64) < 10 {
                    w.event(StoryEvent::JoinBattle(k));
                    s.battle_pact[9] = 1;
                    s.set_adat_i32(adat::BATTLE_88, 1);
                    s.set_adat_i32(adat::NO_RETREAT_94, 1);
                    s.set_adat_i32(adat::NO_RETREAT_98, 1);
                }
            }
        }
    }
    if s.battle_part(CONVOY) != 0 {
        if s.f_search(CONVOY, w) == 0 {
            message(w, 8201);
            let x = (w.rng().range(STREAM, 2, 10) + 20) * 1000;
            w.event(StoryEvent::MessageParamInt { slot: 1, value: x });
            s.sz_start(8219, 90);
            s.v_set(8219, 1);
            s.v_inc(8211);
            add_money(w, x.wrapping_neg(), -1);
        } else {
            let f = s.f_search(PIRATES, w);
            if f != 0 {
                w.event(StoryEvent::SetFleetId { fleet: f, id: HOMEWARD });
                move_to_planet(s, HOMEWARD, BASE, w);
            }
            s.sz_start(BASE, 120);
            if s.v_get(8216) == 0 {
                message(w, 8202);
                s.v_set(8216, 1);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 832: empire 13's deadline

/// 832 (exe block 0x5dde11): empire 13 takes planets 8320..=8322, each guarded by a fleet that chases the human's
/// fleets nearby or drifts round its planet, and retakes it when lost. Empire 13 wiped out ends the story; the
/// deadline (counter 8321) kills the human.
pub(super) fn story_832(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        for k in 0..3 {
            let p = s.b_search(8320 + k, 0, w);
            s.set_planet_owner(p, 13, w);
            s.b_show(8320 + k, -1, w);
        }
        for id in 8320..=8322 {
            s.auto_add_tank(id, w);
            s.auto_add_building(id, w);
        }
        let t = Story::diff(w.difficulty(), 3000, 2600, 2200, 1900);
        s.sz_start(8321, t);
        message(w, 8320);
        for id in 8320..=8322 {
            let p = s.b_search(id, 0, w);
            s.f_new_ex(&String::from_utf8_lossy(&w.story_text(id)), 13, id, 0.09, 2, p, w);
        }
    }
    if s.daily() {
        for id in 8320..=8322 {
            let f = s.f_search(id, w);
            let p = s.b_search(id, 0, w);
            if f == 0 {
                continue;
            }
            let (fx, fy) = w.fleet_xy(f);
            if w.planet_owner(p) == 13 {
                let g = nearest_fleet(cvtt(fx as f64), cvtt(fy as f64), en, &*w);
                if g != 0 && dist((fx, fy), w.fleet_xy(g)) < 20.0 {
                    move_fleet(w, f, 2, 2, g as f32, 0.0);
                } else if w.rng().range(STREAM, 1, 30) == 1 {
                    let r = w.rng().range(STREAM, 5, 20) as f64;
                    let a = w.rng().range(STREAM, 1, 360) as f64 * std::f64::consts::PI / 180.0;
                    let (px, py) = w.planet_xy(p);
                    let y = (a.sin() * r + py as f64) as f32;
                    let x = (a.cos() * r + px as f64) as f32;
                    move_fleet(w, f, 1, 0, x, y);
                }
            } else if w.fleet_5c(f) == 0 {
                let g = nearest_fleet(cvtt(fx as f64), cvtt(fy as f64), en, &*w);
                if g != 0 {
                    move_fleet(w, f, 2, 2, g as f32, 0.0);
                }
            } else {
                move_fleet(w, f, 2, 1, p as f32, 0.0);
            }
        }
    }
    if read(s, 8320) {
        s.add_info_line(8321, InfoLine::Blank, -1, w);
    }
    let mut over = false;
    if s.ground_part(0, 0, w) && s.ground.old_owner == 13 && s.b_search(-13, 1, w) == 0 {
        s.sz_start(8327, 100);
        over = true;
    }
    if s.counter(8327) && s.b_search(-13, 1, w) == 0 {
        loop {
            let f = s.f_search(-13, w);
            if f == 0 {
                break;
            }
            w.event(StoryEvent::DeleteFleet(f));
        }
        over = true;
    }
    if (over || s.battle_part(0) != 0) && s.b_search(-13, 1, w) == 0 && s.f_search(-13, w) == 0 {
        s.win(0, true, w);
    }
    if s.counter(8321) {
        message(w, 8321);
        s.sz_start(8329, 3);
    }
    if s.counter(8329) {
        w.event(StoryEvent::EmpireDies(en));
    }
    Ok(())
}

// ---------------------------------------------------------------- 834: the siege

/// 834 (exe block 0x5de47e): empire 1 sends five fleets one after another against planet 8340; each that arrives
/// attacks the human's fleet next to it, and with none there the planet falls (story 406). Colonising 8340 frees
/// planet 8560; all five fleets destroyed ends the siege.
pub(super) fn story_834(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        if s.b_search(-1, 0, w) == 0 {
            s.win(0, true, w);
        } else {
            let p = s.b_search(8340, 1, w);
            s.b_set_extra(p.wrapping_neg(), true, w);
            s.b_show(8340, -1, w);
            w.event(StoryEvent::SetPlanetRadar { planet: p, level: 5 });
            s.add_undo(9, p, 834);
            for (id, t) in [(8340, 240), (8341, 400), (8342, 700), (8343, 750), (8344, 755)] {
                s.sz_start(id, t);
            }
            message(w, 8340);
        }
    }
    if s.flags[flag::SPECIAL] && s.special[0] == 10 && s.special[2] == s.b_search(8340, 0, w) {
        let p = s.b_search(8560, 0, w);
        s.set_planet_owner(p, 0, w);
        if s.special[1] == en {
            w.event(StoryEvent::Notice(Notice::StoryText(8560)));
        }
    }
    if read(s, 8340) {
        s.add_info_line(8341, InfoLine::Blank, -1, w);
    }
    for id in 8340..=8344 {
        if !s.counter(id) {
            continue;
        }
        if s.b_search(-1, 0, w) == 0 {
            s.win(0, true, w);
        } else {
            s.f_new_ex(&String::from_utf8_lossy(&w.story_text(id)), 1, id, 0.09, 2, -1, w);
            move_to_planet(s, id, 8340, w);
            if id == 8344 {
                s.v_set(8349, 1);
            }
        }
    }
    if s.daily() {
        for id in 8340..=8344 {
            let f = s.f_search(id, w);
            if f != 0 && crate::begin::stands(s, f.wrapping_neg(), w) {
                move_to_planet(s, f.wrapping_neg(), 8340, w);
            }
        }
    }
    let arrived = (8340..=8344).any(|id| s.arrived_planet(id, 8340, w));
    if arrived {
        let mut q = -1;
        loop {
            let p = s.b_search(8340, 0, w);
            let prev = q;
            q = w.near_fleet_ex(p, 20.0, false, false, false, false, prev + 1);
            if !(q != 0 && prev < q && w.fleet_owner(q) != en) {
                break;
            }
        }
        if q == 0 {
            message(w, 8341);
            s.start_story(406, false);
            s.win(0, true, w);
        } else {
            move_fleet(w, s.arrival.fleet, 2, 2, q as f32, 0.0);
        }
    }
    if s.flags[flag::SPACE_END] && s.v_get(8349) != 0 && (8340..=8344).all(|id| s.f_search(id, w) == 0) {
        message(w, 8342);
        s.win(0, true, w);
    }
    Ok(())
}
