//! `st_Rnd_include`, part 2b: escorts, pirates and battles (spec `re/rules/campaign.md` §8.13): a trader to escort
//! (101, 102), a trader chased by pirates (108), a pirate hunter's prey (124), a scientist's ship (131) and the space
//! monster (151).

use super::{add_money, answered, anim, campaign_id, message, param, param_int, promoted, relation, Pick};
use crate::begin::{attack_fleet, delete_fleet, fleet_distance, move_to_planet, show_fleet, stands};
use crate::brnd::nearest_fleet;
use crate::story::{adat, flag, last_fn, Abort, InfoLine, MessageText, Story, StoryEvent, StoryWorld};

/// The script's π (`3.141592`, f64).
#[allow(clippy::approx_constant)]
const SOURCE_PI: f64 = 3.141592;

/// `st_SpWarStart(id)`: a space battle starts (flag 1) with one of `spFlottak`'s two fleets having story id `id`
/// (0: any).
fn battle_starts(s: &Story, id: i32, w: &dyn StoryWorld) -> bool {
    if !s.flags[flag::SPACE_START] {
        return false;
    }
    let (a, b) = w.battle_fleets();
    id == 0 || w.fleet_id(a) == id || w.fleet_id(b) == id
}

/// The escorts' battle: every fleet slot 1..=`OsszHanyHajo` (the ship count: the source's slip) of the human's within
/// 10 of fleet `f` (`(int)(f32) sqrt` of f32 squares from `f`'s truncated position) joins the starting battle
/// (`st_AddFleetToSpWar`), the pirates (9) fight on the human's side (`StoryUrharcViszony[9] := 1`) and no one
/// retreats (`StoryAdat+0x88`, `+0x94 := 1`).
fn join_escort(s: &mut Story, f: i32, w: &mut dyn StoryWorld) {
    let en = w.human();
    let (fx, fy) = w.fleet_xy(f);
    let (ix, iy) = (fx as i32 as f32, fy as i32 as f32);
    for g in 1..=w.ship_count() {
        if w.fleet_owner(g) != en {
            continue;
        }
        let (gx, gy) = w.fleet_xy(g);
        let (dx, dy) = (ix - gx, iy - gy);
        // `cvttss2si`: the garbage of slots past the fleet count (NaN, huge) gives INT_MIN, which joins.
        let d = super::cvtt(((dy * dy + dx * dx) as f64).sqrt() as f32 as f64);
        if d < 10 {
            w.event(StoryEvent::JoinBattle(g));
            s.battle_pact[9] = 1;
            s.set_adat_i32(adat::BATTLE_88, 1);
            s.set_adat_i32(adat::NO_RETREAT_94, 1);
        }
    }
}

/// `GetNewKereskedoAzonosito()` (exe 0x5c2770): one above the highest story id 10101..10999 of a fleet of owner 9
/// among slots 0..`OsszHanyFlotta` − 1 (10100 at least).
fn new_trader_id(w: &dyn StoryWorld) -> i32 {
    let mut id = 10100;
    for f in 0..w.fleet_count() {
        let fid = w.fleet_id(f);
        if w.fleet_owner(f) == 9 && id < fid && fid < 11000 {
            id = fid;
        }
    }
    id + 1
}

/// `st_fSetBarat(id, on)` / `st_fSetVillogas(id, on)` / `st_fSetRadarMsg(id, on)`: fleet `id`'s (`< 0`: slot
/// `−id`) `+0x52` bit `bit` (0x200 friendly, 0x80 blinking, 0x40 radar contact reported). `StoryError` = no fleet.
pub(super) fn fleet_bit(s: &mut Story, id: i32, bit: u16, on: bool, w: &mut dyn StoryWorld) {
    let f = if id < 0 { id.wrapping_neg() } else { s.f_search(id, w) };
    if f >= 1 {
        let (set, clear) = if on { (bit, 0) } else { (0, bit) };
        w.event(StoryEvent::FleetFlags { fleet: f, set, clear });
    }
    s.error = (f < 1) as i32;
}

/// An escorted trader goes back to trading: fleet `f` gets a new trader id, loses its name, visibility, speed and
/// `+0x18` bit 0, and heads for a planet (`b`).
fn back_to_trade(f: i32, b: i32, w: &mut dyn StoryWorld) {
    let id = new_trader_id(&*w);
    w.event(StoryEvent::SetFleetId { fleet: f, id });
    w.event(StoryEvent::FleetFlags { fleet: f, set: 0, clear: 0x800 | 0x100 });
    w.event(StoryEvent::SetFleetSpeed { fleet: f, speed: 0.0 });
    w.event(StoryEvent::FleetState { fleet: f, set: 0, clear: 1 });
    w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: b as f32, y: 0.0 });
}

/// `st_fNewPos(name, owner, id, x, y, speed)` (exe 0x605010): `FlottaNew(name, owner, id, x, y, speed ≤ 1 ? 100·speed
/// : speed)` and `AutoAddship(f, id)`.
pub(super) fn fleet_at(s: &mut Story, owner: i32, id: i32, x: i32, y: i32, speed: f32, w: &mut dyn StoryWorld) -> i32 {
    let speed = if speed <= 1.0 { speed * 100.0 } else { speed };
    let f = w.new_fleet(&crate::story::FleetName::Text(id), owner, id, x as f32, y as f32, speed);
    s.auto_add_ship(f, id, w);
    f
}

/// `st_SpWarPartTulaj(EN)` as a bool.
fn human_in_battle(s: &Story, w: &dyn StoryWorld) -> bool {
    super::battle_owner_part(s, w.human()) != 0
}

// ---------------------------------------------------------------- 101, 102: a trader to escort

/// 101 (with fewer than 15 planets; else abort): a trader of the pirates' (9) asks for an escort from a planet of the
/// human's (id 1011) to a free planet 100..200 away (id 1012; message 0x3f3, the human's name and the planet).
/// Accepted (answer 2) → 60 days later the trader (fleet 1011, friendly) comes to planet 1011; refused or lapsed →
/// the end. At planet 1011: message 0x3fb (planet 1012), it leaves for 1012 and 15 days later a pirate (fleet 1012,
/// owner 10, from planet 1012) attacks it; the battle: the human's fleets within 10 of the trader join and the
/// pirates (9) side with the human. Trader lost → message 0x3f8, the pirate goes home (none → the end). At planet
/// 1012: one ship left → message 0x3f5, pirates +3, 3,500; more → 0x3f4, +7, 10,000; the pirate deleted, the trader
/// back to trading; the end. The pirate home → deleted, the end.
pub(super) fn escort_101(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const TRADER: i32 = 0x3f3;
    const PIRATE: i32 = 0x3f4;
    const LATER: i32 = 0x3fb;
    let en = w.human();
    if s.starting_now() {
        if w.planets_of(en) > 14 {
            return Err(Abort);
        }
        s.v_set(TRADER, 0);
        let p = s.b_search(en.wrapping_neg(), 0, w);
        s.b_add_id(p, TRADER);
        let q = s.planet_at_distance(100, 0, w)?;
        s.b_add_id(q, PIRATE);
        message(w, TRADER, 0x14);
        param(w, 1, MessageText::Empire(en));
        param(w, 2, MessageText::Planet(p));
    }
    if answered(s, TRADER, 2) {
        s.sz_start(LATER, 60);
    }
    if s.counter(LATER) {
        let p = s.b_search(TRADER, 0, w);
        s.f_new_ex(crate::story::FleetName::Text(0x3f3), 9, TRADER, 0.04, 1, p << 16, w);
        fleet_bit(s, TRADER, 0x200, true, w);
        move_to_planet(s, TRADER, TRADER, w);
    }
    if answered(s, TRADER, 3) || promoted(s, TRADER) {
        s.win(0, true, w);
    }
    if s.arrived_planet(TRADER, 0, w) {
        if !s.arrived_planet(0, TRADER, w) {
            if s.arrived_planet(0, PIRATE, w) {
                let f = s.f_search(TRADER, w);
                if w.fleet_ship_count(f) < 2 {
                    message(w, 0x3f5, 0);
                    relation(w, en, 9, 3);
                    add_money(w, 3500.0);
                } else {
                    message(w, 0x3f4, 0);
                    relation(w, en, 9, 7);
                    add_money(w, 10_000.0);
                }
                delete_fleet(s, PIRATE, w);
                let f = s.f_search(TRADER, w);
                if f != 0 {
                    fleet_bit(s, TRADER, 0x200, false, w);
                    let b = s.b_search(en.wrapping_neg(), 0, w);
                    back_to_trade(f, b, w);
                }
                s.win(0, true, w);
            }
        } else {
            message(w, LATER, 0);
            let q = s.b_search(PIRATE, 0, w);
            param(w, 1, MessageText::Planet(q));
            move_to_planet(s, TRADER, PIRATE, w);
            s.sz_start(TRADER, 15);
            show_fleet(s, TRADER, true, w);
        }
    }
    if s.counter(TRADER) {
        let q = s.b_search(PIRATE, 0, w);
        s.f_new_ex(crate::story::FleetName::Text(0x3f4), 10, PIRATE, 0.05, q.wrapping_neg(), 0, w);
        attack_fleet(s, PIRATE, TRADER, w);
    }
    if battle_starts(s, TRADER, &*w) && battle_starts(s, PIRATE, &*w) {
        let f = s.f_search(TRADER, w);
        if f == 0 {
            return Err(Abort);
        }
        join_escort(s, f, w);
    }
    if s.battle_part(TRADER) != 0 {
        if s.f_search(TRADER, w) == 0 {
            message(w, 0x3f8, 0);
            if !move_to_planet(s, PIRATE, PIRATE, w) {
                s.win(0, true, w);
            }
        } else {
            move_to_planet(s, TRADER, PIRATE, w);
        }
    }
    if s.battle_part(PIRATE) != 0 {
        if s.f_search(TRADER, w) == 0 && s.battle_part(PIRATE) == 0 {
            s.win(0, true, w);
        } else {
            move_to_planet(s, PIRATE, PIRATE, w);
        }
    }
    if s.arrived_planet(PIRATE, 0, w) {
        delete_fleet(s, PIRATE, w);
        s.win(0, true, w);
    }
    Ok(())
}

/// 102 (fewer than 15 planets): a trader asks for an escort from a planet of the human's (id 1021) to one of a random
/// empire's (id 1022; none → abort; message 0x3fd, the empire's name). Accepted → 60 days later the trader (1021,
/// friendly) comes; at planet 1021 it leaves for 1022 (message 0x405, shown) and with `rnd(1, 100) < 60` a pirate
/// (1022, owner 10: ships 3×4, 2×16) comes 4 days later from planet 1022. The battle as 101's; the trader lost →
/// pirates −14, message 0x402, the pirate sent away, the end. At planet 1022: both fleets deleted, message 0x3fe,
/// 10,000, pirates +8, the end. Refused or lapsed → the end (lapsed: pirates −2 too).
pub(super) fn escort_102(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const TRADER: i32 = 0x3fd;
    const DEST: i32 = 0x3fe;
    const LATER: i32 = 0x405;
    let en = w.human();
    if s.starting_now() {
        if w.planets_of(en) > 14 {
            return Err(Abort);
        }
        let e = crate::brnd::random_empire(w, 0, 0, 0)?;
        let p = s.b_search(en.wrapping_neg(), 0, w);
        s.b_add_id(p, TRADER);
        let q = s.b_search(e.wrapping_neg(), 0, w);
        s.b_add_id(q, DEST);
        if q == 0 {
            return Err(Abort);
        }
        message(w, TRADER, 0x14);
        param(w, 1, MessageText::Empire(e));
    }
    if answered(s, TRADER, 2) {
        s.sz_start(LATER, 60);
    }
    if s.counter(LATER) {
        let p = s.b_search(TRADER, 0, w);
        s.f_new_ex(crate::story::FleetName::Text(0x3fd), 9, TRADER, 0.04, 1, p << 16, w);
        fleet_bit(s, TRADER, 0x200, true, w);
        move_to_planet(s, TRADER, TRADER, w);
    }
    if answered(s, TRADER, 3) || promoted(s, TRADER) {
        s.win(0, true, w);
    }
    if s.arrived_planet(TRADER, 0, w) {
        let target = s.arrival.target;
        if target == s.b_search(TRADER, 0, w) {
            move_to_planet(s, TRADER, DEST, w);
            s.b_search(DEST, 0, w);
            if super::rnd(w, 1, 100) < 60 {
                s.sz_start(TRADER, 4);
            }
            message(w, LATER, 0);
            show_fleet(s, TRADER, true, w);
        } else if target == s.b_search(DEST, 0, w) {
            delete_fleet(s, TRADER, w);
            delete_fleet(s, DEST, w);
            message(w, DEST, 0);
            add_money(w, 10_000.0);
            relation(w, en, 9, 8);
            s.win(0, true, w);
        }
    }
    if s.counter(TRADER) {
        let q = s.b_search(DEST, 0, w);
        s.f_new_ex(crate::story::FleetName::Text(0x3fe), 10, DEST, 0.06, q.wrapping_neg(), 0, w);
        s.f_add_ship(DEST, 3, 4, w);
        s.f_add_ship(DEST, 2, 16, w);
        attack_fleet(s, DEST, TRADER, w);
    }
    if battle_starts(s, TRADER, &*w) && battle_starts(s, DEST, &*w) {
        let f = s.f_search(TRADER, w);
        if f == 0 {
            return Err(Abort);
        }
        join_escort(s, f, w);
    }
    if s.battle_part(TRADER) != 0 {
        if s.f_search(TRADER, w) == 0 {
            relation(w, en, 9, -14);
            message(w, 0x402, 0);
            s.send_away(DEST, w);
            s.win(0, true, w);
        } else {
            move_to_planet(s, TRADER, DEST, w);
        }
    }
    if s.arrived_planet(DEST, 0, w) {
        delete_fleet(s, DEST, w);
        s.win(0, true, w);
    }
    if promoted(s, TRADER) {
        relation(w, en, 9, -2);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 108: a chased trader

/// 108: a trader (fleet 1081, owner 9, ships 8×3, friendly, blinking, shown; message 0x439) leaves a planet of the
/// human's (id 1081) for a point 300 past a spot within ±20 of it; a pirate (1082, owner 10, ships 3×2 and 2×8,
/// placed `rnd(35, 55)` + `last_fn_r` off at angle `rnd(−25, 25)` + `last_fn_q` by `st_fNewPos`) chases it. The pirate
/// reaching the trader: both stop, the human's nearest fleet (`(int)` of the f32 squared distance under 50) follows the
/// pirate, 35 days later (counter 1089) the chase goes on (logic 1081/1082: `Story_SpaceBattleStarted`'s scripted
/// battle). Both in a finished battle: the trader alive and the human in it → message 0x43a, pirates +8, `rnd(0, 10) +
/// 4·level` thousands; else the pirate sent away, pirates −6. Only the pirate in it, gone, with the human: the trader
/// alive → message 0x43b, pirates +12, `rnd(0, 10) + 5·level` thousands; else pirates −2. Either way the trader goes
/// back to trading, the end. The pirate at a planet → deleted, pirates −2, the end.
pub(super) fn chase(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const TRADER: i32 = 0x439;
    const PIRATE: i32 = 0x43a;
    const LATER: i32 = 0x441;
    let en = w.human();
    if s.starting_now() {
        let p = s.b_search(en.wrapping_neg(), 0, w);
        s.b_add_id(p, TRADER);
        s.f_new_ex(crate::story::FleetName::Text(0x439), 9, TRADER, 0.03, 1, p << 16, w);
        s.f_add_ship(TRADER, 8, 3, w);
        fleet_bit(s, TRADER, 0x200, true, w);
        let f = s.f_search(TRADER, w);
        let (fx, fy) = w.fleet_xy(f);
        let (px, py) = w.planet_xy(p);
        let tx = super::rnd(w, -20, 20) as f32 + px;
        let ty = super::rnd(w, -20, 20) as f32 + py;
        let (dx, dy) = (fx - tx, fy - ty);
        let d = ((dy * dy + dx * dx) as f64).sqrt() as f32;
        let x = fx + (tx - fx) * 300.0 / d;
        let y = fy + (ty - fy) * 300.0 / d;
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 0, x, y });
        let r = s.last_fn[last_fn::R].wrapping_add(super::rnd(w, 35, 55));
        let q = super::rnd(w, -25, 25).wrapping_add(s.last_fn[last_fn::Q]);
        let a = q as f64 * SOURCE_PI / 180.0;
        let b = s.last_fn[last_fn::B];
        let nx = (w.planet_xy(b).0 as f64 + r as f64 * a.cos()) as i32;
        let ny = (py as f64 + r as f64 * a.sin()) as i32;
        fleet_at(s, 10, PIRATE, nx, ny, 0.05, w);
        s.f_add_ship(PIRATE, 3, 2, w);
        s.f_add_ship(PIRATE, 2, 8, w);
        attack_fleet(s, PIRATE, TRADER, w);
        message(w, TRADER, 0);
        s.l_set(TRADER, true);
        s.l_set(PIRATE, true);
        show_fleet(s, TRADER, true, w);
        fleet_bit(s, TRADER, 0x80, true, w);
    }
    let mut over = false;
    if s.arrived_fleet(PIRATE, TRADER) {
        s.l_set(TRADER, false);
        s.sz_start(LATER, 35);
        let f = s.f_search(TRADER, w);
        w.event(StoryEvent::StopFleet(f));
        let g = s.f_search(PIRATE, w);
        w.event(StoryEvent::StopFleet(g));
        let (gx, gy) = w.fleet_xy(g);
        let (mut best, mut best_d) = (0, 0);
        for h in 1..=w.fleet_count() {
            if w.fleet_owner(h) != en {
                continue;
            }
            let (hx, hy) = w.fleet_xy(h);
            let (dx, dy) = (gx - hx, gy - hy);
            let d = (dx * dx + dy * dy) as i32;
            if best == 0 || d < best_d {
                (best, best_d) = (h, d);
            }
        }
        if best_d < 50 {
            w.event(StoryEvent::MoveFleet { fleet: best, kind: 2, mode: 2, x: g as f32, y: 0.0 });
        }
    } else if s.battle_part(TRADER) != 0 && s.battle_part(PIRATE) != 0 {
        if s.f_search(TRADER, w) == 0 || !human_in_battle(s, &*w) {
            over = true;
            s.send_away(PIRATE, w);
            relation(w, en, 9, -6);
            s.win(0, true, w);
        } else {
            message(w, PIRATE, 0);
            relation(w, en, 9, 8);
            let k = super::rnd(w, 0, 10).wrapping_add(Story::story_level(&*w) as i32 * 4);
            add_money(w, k.wrapping_mul(1000) as f64);
            param_int(w, 1, k);
            over = true;
            s.win(0, true, w);
        }
    } else if s.battle_part(PIRATE) != 0 && s.f_search(PIRATE, w) == 0 && human_in_battle(s, &*w) {
        if s.f_search(TRADER, w) == 0 {
            relation(w, en, 9, -2);
        } else {
            relation(w, en, 9, 12);
            message(w, 0x43b, 0);
            let k = super::rnd(w, 0, 10).wrapping_add(Story::story_level(&*w) as i32 * 5);
            add_money(w, k.wrapping_mul(1000) as f64);
            param_int(w, 1, k);
        }
        over = true;
        s.win(0, true, w);
    }
    if s.daily() && s.l_is_true(TRADER) && stands(s, PIRATE, w) {
        attack_fleet(s, PIRATE, TRADER, w);
    }
    if over {
        let f = s.f_search(TRADER, w);
        if f > 0 {
            fleet_bit(s, TRADER, 0x80, false, w);
            fleet_bit(s, TRADER, 0x200, false, w);
            let b = s.b_search(en.wrapping_neg(), 0, w);
            back_to_trade(f, b, w);
        }
    }
    if s.counter(LATER) {
        s.l_set(PIRATE, false);
        attack_fleet(s, PIRATE, TRADER, w);
    }
    if s.arrived_planet(PIRATE, 0, w) {
        delete_fleet(s, PIRATE, w);
        relation(w, en, 9, -2);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 124: the hunter's prey

/// 124's pirate goes for the human's fleet nearest to it (`st_fGetNearest`); none → it is sent away, the end.
fn hunt(s: &mut Story, f: i32, w: &mut dyn StoryWorld) {
    let (fx, fy) = w.fleet_xy(f);
    let en = w.human();
    let n = nearest_fleet(fx as i32, fy as i32, en, &*w);
    if n == 0 {
        s.send_away(0x4d9, w);
        s.win(0, true, w);
    } else {
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 2, x: n as f32, y: 0.0 });
    }
}

/// 124: an empire at war with the human (`v(1241)`; none → the end without history) has a pirate (fleet 1241, owner
/// 10, ships 3×3, 2×5) waiting outside the human's radar `rnd(10, 15)` + 1 per try (up to 100 tries; none → abort)
/// off the point `st_fNewEx` placed a trader (fleet 1242, owner 9, ships 8×2, friendly, shown, blinking) at; both
/// report radar contact (message 0x4d9, a video, counter 1249 := 480 days). The counter → both hidden, the pirate
/// deleted, the end. The radar seeing either: the other's contact flag cleared, the trader hidden and not friendly,
/// the pirate hunts the human's nearest fleet (message 0x4da when the trader was seen, else 0x4db; the empire's
/// portrait and name), every day and after each battle again; a battle starting with it: `StoryAdat+0x88 := 1`.
/// Whenever it ended the trader (if any) goes back to trading towards a planet of the empire's (else the human's).
pub(super) fn prey(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const PIRATE: i32 = 0x4d9;
    const TRADER: i32 = 0x4da;
    const LIMIT: i32 = 0x4e1;
    let en = w.human();
    let mut over = false;
    if s.starting_now() {
        let e = super::relations::enemy_empire(0, w);
        if e == 0 {
            s.win(0, false, w);
        } else {
            s.v_set(PIRATE, e);
            let mut r = super::rnd(w, 10, 15);
            s.f_new_ex(crate::story::FleetName::Text(0x4da), 9, TRADER, 0.03, 1, 0, w);
            s.f_add_ship(TRADER, 8, 2, w);
            fleet_bit(s, TRADER, 0x200, true, w);
            let mut tries = 100;
            let (x, y) = loop {
                let a = s.last_fn[last_fn::Q] as f64 * SOURCE_PI / 180.0;
                let x = (s.last_fn[last_fn::X] as f64 + r as f64 * a.cos()) as i32;
                let y = (s.last_fn[last_fn::Y] as f64 + r as f64 * a.sin()) as i32;
                r = r.wrapping_add(1);
                tries -= 1;
                if tries == 0 || !w.in_radar_range(x, y, 3) {
                    break (x, y);
                }
            };
            if tries == 0 {
                return Err(Abort);
            }
            fleet_at(s, 10, PIRATE, x, y, 0.07, w);
            s.f_add_ship(PIRATE, 3, 3, w);
            s.f_add_ship(PIRATE, 2, 5, w);
            show_fleet(s, TRADER, true, w);
            fleet_bit(s, PIRATE, 0x40, true, w);
            fleet_bit(s, TRADER, 0x40, true, w);
            fleet_bit(s, TRADER, 0x80, true, w);
            message(w, PIRATE, 0x1e0);
            anim(w, PIRATE);
            s.sz_start(LIMIT, 0x1e0);
        }
    }
    if s.counter(LIMIT) {
        show_fleet(s, TRADER, false, w);
        show_fleet(s, PIRATE, false, w);
        fleet_bit(s, TRADER, 0x200, false, w);
        delete_fleet(s, PIRATE, w);
        over = true;
        s.win(0, true, w);
    }
    if radar_fleet(s, TRADER, w) || radar_fleet(s, PIRATE, w) {
        s.sz_stop(LIMIT);
        let trader_seen = radar_fleet(s, TRADER, w);
        fleet_bit(s, PIRATE + !trader_seen as i32, 0x40, false, w);
        show_fleet(s, TRADER, false, w);
        fleet_bit(s, TRADER, 0x200, false, w);
        over = true;
        let f = s.f_search(PIRATE, w);
        if f == 0 {
            return Err(Abort);
        }
        hunt(s, f, w);
        let trader_seen = radar_fleet(s, TRADER, w);
        message(w, TRADER | !trader_seen as i32, 0);
        let e = s.v_get(PIRATE);
        w.event(StoryEvent::MessageRace(e));
        let e = s.v_get(PIRATE);
        param(w, 1, MessageText::Empire(e));
        s.v_set(LIMIT, 1);
    }
    if s.daily() && s.v_get(LIMIT) != 0 {
        let f = s.f_search(PIRATE, w);
        if f == 0 {
            s.win(0, true, w);
        } else {
            hunt(s, f, w);
        }
    }
    if battle_starts(s, PIRATE, &*w) {
        s.set_adat_i32(adat::BATTLE_88, 1);
    }
    if s.battle_part(PIRATE) != 0 {
        let f = s.f_search(PIRATE, w);
        if f == 0 {
            s.win(0, true, w);
        } else {
            hunt(s, f, w);
        }
    }
    if over {
        let g = s.f_search(TRADER, w);
        if g != 0 {
            fleet_bit(s, TRADER, 0x80, false, w);
            fleet_bit(s, TRADER, 0x200, false, w);
            let e = s.v_get(PIRATE);
            let mut b = s.b_search(e.wrapping_neg(), 1, w);
            if b == 0 {
                b = s.b_search(en.wrapping_neg(), 0, w);
            }
            back_to_trade(g, b, w);
        }
    }
    Ok(())
}

/// `st_RangeF(f)`: the human's radar reached fleet `f` (flag 10).
fn radar_fleet(s: &mut Story, f: i32, w: &mut dyn StoryWorld) -> bool {
    s.flags[flag::RADAR_FLEET] && (f == 0 || s.radar.1 == s.f_search(f, w))
}

// ---------------------------------------------------------------- 131: a scientist

/// A planet of the human's with an intact (`+8 == 0`) building of kind 14..=16, else 0.
fn lab_planet(s: &mut Story, w: &mut dyn StoryWorld) -> i32 {
    let en = w.human();
    let p = s.b_search(en.wrapping_neg(), 0, w);
    let lab = (1..=w.building_count(p)).any(|k| {
        let (kind, _, b8) = w.building(p, k);
        (14..=16).contains(&kind) && b8 == 0
    });
    if lab {
        p
    } else {
        0
    }
}

/// `st_bTav(a, b)` (inlined): `(int)(f32) sqrt((f64)(f32)(dy² + dx²))` of the planets (the ELF skips the f32).
fn planet_distance(a: i32, b: i32, w: &dyn StoryWorld) -> i32 {
    crate::begin::planet_distance(w.planet_xy(a), w.planet_xy(b))
}

/// Tech `t`'s fake prerequisite 0xb8 (131's) goes.
fn clear_fake_prereq(t: i32, w: &mut dyn StoryWorld) {
    let pre = w.tech_prereqs(t);
    for (k, &q) in pre.iter().enumerate() {
        if q == 0xb8 {
            w.event(StoryEvent::TechPrereq { tech: t, slot: k as i32 + 1, value: 0 });
        }
    }
}

/// 131 (with tech 57): of 101 pairs of random planets of the human's with labs (kinds 14..=16) the farthest apart
/// two become 1311 and 1312 (shown); the tech being researched (none, or no pair → the end without history) gets a
/// fake prerequisite 0xb8 and status 2 (message 0x51f). Five days later a scientist (fleet 1311, owner 9, friendly,
/// shown) leaves 1311 for 1312 and a pirate (1312, owner 10) from near 1312 hunts it (message 0x522, an info line).
/// The scientist at 1312: a video, message 0x520, the prerequisite goes, the tech back to status 4 (researched by
/// none: `ResearchAktFejlesztett[EN] == 0`) or 3, both fleets sent away, the end. The scientist lost: message 0x521,
/// the prerequisite goes, the tech back to 3 with its research time 0, the end. Its battle as 101's.
pub(super) fn scientist(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const FROM: i32 = 0x51f;
    const TO: i32 = 0x520;
    const TECH: i32 = 0x527;
    let en = w.human();
    if s.starting_now() {
        if w.tech_status(57, en) == 5 {
            let (mut a, mut b) = (0, 0);
            for _ in 0..101 {
                let p = lab_planet(s, w);
                let q = lab_planet(s, w);
                if p != 0 && q != 0 && p != q && (a == 0 || planet_distance(p, q, &*w) > planet_distance(a, b, &*w)) {
                    (a, b) = (p, q);
                }
            }
            let t = w.researching(en);
            if a == 0 || t == 0 {
                s.win(0, false, w);
            } else {
                s.b_add_id(a, FROM);
                s.b_add_id(b, TO);
                s.b_show(FROM, -1, w);
                s.b_show(TO, -1, w);
                s.v_set(TECH, t);
                let pre = w.tech_prereqs(t);
                let k = (1..3).find(|&k| pre[k as usize - 1] == 0).unwrap_or(3);
                w.event(StoryEvent::TechPrereq { tech: t, slot: k, value: 0xb8 });
                s.set_tech_state_to(t, 2, en, w);
                message(w, FROM, 0);
                param(w, 1, MessageText::Planet(a));
                param(w, 2, MessageText::Planet(b));
                param(w, 3, MessageText::Tech(t));
                s.sz_start(FROM, 5);
            }
        } else {
            s.win(0, false, w);
        }
    }
    if s.counter(FROM) {
        let p = s.b_search(FROM, 0, w);
        s.f_new_ex(crate::story::FleetName::Text(0x51f), 9, FROM, 0.01, 2, p, w);
        move_to_planet(s, FROM, TO, w);
        fleet_bit(s, FROM, 0x200, true, w);
        show_fleet(s, FROM, true, w);
        let q = s.b_search(TO, 0, w);
        s.f_new_ex(crate::story::FleetName::Text(0x520), 10, TO, 0.06, 1, q << 16, w);
        attack_fleet(s, TO, FROM, w);
        message(w, 0x522, 0x14);
        let q = s.b_search(TO, 0, w);
        param(w, 1, MessageText::Planet(q));
        // The exe pushes `a2` first: `StoryError` is left by the search for `FROM`.
        let to = s.b_search(TO, 0, w);
        let from = s.b_search(FROM, 0, w);
        s.add_info_line(FROM, InfoLine::TwoPlanets([from, to]), -1, w);
    }
    if s.arrived_planet(FROM, TO, w) {
        message(w, TO, 0);
        anim(w, TO);
        let t = s.v_get(TECH);
        clear_fake_prereq(t, w);
        let state = if w.researching(en) == 0 { 4 } else { 3 };
        s.set_tech_state_to(t, state, en, w);
        s.send_away(FROM, w);
        show_fleet(s, FROM, false, w);
        s.remove_fleet_id(FROM, w);
        s.send_away(TO, w);
        s.win(0, true, w);
    }
    if battle_starts(s, FROM, &*w) && battle_starts(s, TO, &*w) {
        let f = s.f_search(FROM, w);
        if f == 0 {
            return Err(Abort);
        }
        join_escort(s, f, w);
    }
    if s.battle_part(FROM) != 0 && s.f_search(FROM, w) == 0 {
        message(w, 0x521, 0);
        s.send_away(TO, w);
        let t = s.v_get(TECH);
        clear_fake_prereq(t, w);
        s.set_tech_state_to(t, 3, en, w);
        w.event(StoryEvent::TechResearchTime { tech: t, value: 0 });
        s.win(0, true, w);
    }
    if s.daily() {
        let f = s.f_search(FROM, w);
        if f != 0 && stands(s, f.wrapping_neg(), w) {
            move_to_planet(s, FROM, TO, w);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 151: the space monster

/// 151's monster (fleet 1511 of owner 0, `st_fNewEx` mode 3, story-protected `+0x52 |= 0x4000`, `last_addship := 0`)
/// heads for a planet of the human's (id 1511) with 200 hit points (`v(1511)`). Within 150 of the human's planets
/// (daily, once): a video (not in campaign 1), it is shown, message 0x5e7 (campaign 3: 0x119f), an info line. Idle:
/// it goes for the planet again (attacking it unless free). At a free planet: deleted, the end. Special 0x12 (data 2
/// = 1: the fleet `SearchBolygohozKozeliFlotta(planet, 15, …)` finds there, 2: data 3) fights it: a fleet weaker than
/// its hit points (`+0x60`) is deleted and the monster loses that much (message 0x5e9 / 0x11a1 with the damage and
/// the percentage); else the fleet's ships (weight 1 while it has attack or nothing was picked yet, else 0) and its
/// non-ground units (weight their attack, or the first) are drawn one by one until the hit points are gone: a unit
/// goes when its attack takes them below (ships: unless the loss would be more than half its attack); 0 left → the
/// monster dies (a video in campaign 1, message 0x5e8 or 0x5ea / 0x11a2 with the ships and units lost), the end;
/// else message 0x5e9. Special 0x12 with data 2 = 1 while it lives: the planet's armed buildings (kinds 0x1c..0x1f,
/// weights 2..5, `SearchBuilding`) against its hit points: the human's planet gets message 0x5ef / 0x11a7 (untouched),
/// 0x5ed, 0x5ec or 0x5eb / 0x11a3 (with videos outside campaign 1); falling short, `pct = ((hp − total)·100 / 200)
/// / 2`: every intact building is either destroyed (`rnd(1, 100) ≤ pct`, not the colony hub, kind 1) or damaged to
/// `max(damage, hp·rnd(5, 95)/100)` (`rnd(1, 100) ≤ pct`), and the planet loses `pct` % of its `+0x28`; the
/// attacking fleet is deleted (special data 1), the end.
pub(super) fn monster(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x5e7;
    const SEEN: i32 = 0x5e8;
    let en = w.human();
    let campaign = w.campaign_type();
    if s.starting_now() {
        let p = s.b_search(en.wrapping_neg(), 0, w);
        s.b_add_id(p, ID);
        let f = s.f_new_ex(crate::story::FleetName::Text(0x5e7), 0, ID, 0.07, 3, 0, w);
        s.last_addship = 0;
        w.event(StoryEvent::FleetFlags { fleet: f, set: 0x4000, clear: 0 });
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 1, x: p as f32, y: 0.0 });
        s.v_set(ID, 200);
        s.v_set(SEEN, 1);
        s.v_set(0x5e9, 1);
    }
    if s.daily() && s.v_get(SEEN) != 0 {
        let f = s.f_search(ID, w);
        if fleet_distance(f, &*w) < 150 {
            if campaign != 1 {
                anim(w, ID);
            }
            show_fleet(s, ID, true, w);
            s.v_set(SEEN, 0);
            let id = campaign_id(&*w, 3, ID, 0x119f);
            message(w, id, 0);
            let p = s.b_search(ID, 0, w);
            param(w, 1, MessageText::Planet(p));
            s.add_info_line(ID, InfoLine::Blank, -1, w);
        }
    }
    if s.daily() && stands(s, ID, w) {
        let p = s.b_search(ID, 0, w);
        let f = s.f_search(ID, w);
        let p2 = s.b_search(ID, 0, w);
        let kind = if w.planet_owner(p) == 0 { 1 } else { 2 };
        w.event(StoryEvent::MoveFleet { fleet: f, kind, mode: 1, x: p2 as f32, y: 0.0 });
    }
    if s.arrived_planet(ID, ID, w) {
        let p = s.b_search(ID, 0, w);
        if w.planet_owner(p) == 0 {
            delete_fleet(s, ID, w);
            s.win(0, true, w);
        } else {
            let f = s.f_search(ID, w);
            let p = s.b_search(ID, 0, w);
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 1, x: p as f32, y: 0.0 });
        }
    }
    let mut attacker = 0;
    if s.special_is(0x12, 0, 1, 0) {
        let p = s.b_search(ID, 0, w);
        attacker = w.near_fleet(p, 15.0);
    }
    if s.special_is(0x12, 0, 2, 0) {
        attacker = s.special[3];
    }
    if attacker != 0 {
        let monster = s.f_search(ID, w);
        let hp = s.v_get(ID);
        if w.fleet_power(attacker) < hp {
            let left = hp - w.fleet_power(attacker);
            w.event(StoryEvent::DeleteFleet(attacker));
            s.v_set(ID, left);
            let id = campaign_id(&*w, 3, 0x5e9, 0x11a1);
            message(w, id, 0);
            param_int(w, 1, 200 - left);
            param_int(w, 2, (200 - left) * 100 / 200);
        } else {
            let owner = w.fleet_owner(attacker);
            let (mut ships, mut tanks) = (0, 0);
            let mut left = hp;
            let mut ship_list: Vec<(i32, i32)> = (0..=w.ship_count()).map(|k| w.ship(k)).collect();
            let mut tank_list: Vec<(i32, i32, i32)> = (0..=w.tank_count()).map(|k| w.tank(k)).collect();
            while left > 0 {
                let mut pick = Pick::default();
                let (mut chosen, mut is_tank) = (0usize, false);
                for (k, &(f, d)) in ship_list.iter().enumerate().skip(1) {
                    if d == 0x7fff || f != attacker {
                        continue;
                    }
                    let weight = if w.design_attack(owner, d) == 0 { (chosen == 0) as i32 } else { 1 };
                    if pick.weighted(weight, w) {
                        chosen = k;
                    }
                }
                for (k, &(place, d, _)) in tank_list.iter().enumerate().skip(1) {
                    if d == 0x7fff || w.design_kind(owner, d) == 2 || place != attacker {
                        continue;
                    }
                    if pick.weighted(w.design_attack(owner, d), w) || chosen == 0 {
                        is_tank = true;
                        chosen = k;
                    }
                }
                if chosen == 0 {
                    break;
                }
                if !is_tank {
                    let attack = w.design_attack(owner, ship_list[chosen].1);
                    left -= attack;
                    if -attack / 2 <= left {
                        w.event(StoryEvent::DeleteShip(chosen as i32));
                        ship_list[chosen].1 = 0x7fff;
                        ships += 1;
                    }
                } else {
                    let d = tank_list[chosen].1;
                    left -= w.design_attack(owner, d);
                    w.event(StoryEvent::DeleteTanks { place: attacker, design: d, count: 1 });
                    if let Some(t) = tank_list.iter_mut().skip(1).find(|t| t.0 == attacker && t.1 == d) {
                        t.2 -= 1;
                        if t.2 < 1 {
                            t.1 = 0x7fff;
                        }
                    }
                    tanks += 1;
                }
            }
            s.v_set(ID, left);
            if left < 1 {
                if campaign == 1 {
                    anim(w, ID);
                }
                w.event(StoryEvent::DeleteFleet(monster));
                if ships + tanks == 0 {
                    message(w, 0x5e8, 0);
                } else {
                    let id = campaign_id(&*w, 3, 0x5ea, 0x11a2);
            message(w, id, 0);
                    param_int(w, 1, ships);
                    param_int(w, 2, tanks);
                }
                s.win(0, true, w);
            } else {
                let id = campaign_id(&*w, 3, 0x5e9, 0x11a1);
            message(w, id, 0);
            }
        }
    }
    if s.special_is(0x12, 0, 1, 0) && s.f_search(ID, w) != 0 {
        const WEIGHT: [i32; 4] = [2, 3, 4, 5];
        let p = s.b_search(ID, 0, w);
        let mut total = 0i32;
        for (k, weight) in WEIGHT.iter().enumerate() {
            let n = (1..=w.building_count(p))
                .filter(|&b| {
                    let (kind, _, b8) = w.building(p, b);
                    kind == 0x1c + k as i32 && b8 == 0 && w.building_bytes(p, b).1 & 1 != 0
                })
                .count() as i32;
            total = total.wrapping_add(n.wrapping_mul(*weight));
        }
        let hp = s.v_get(ID);
        if w.planet_owner(p) == en {
            if total == 0 {
                if campaign != 1 {
                    anim(w, 0x5ef);
                }
                if hp == 200 {
                    let id = campaign_id(&*w, 3, 0x5ef, 0x11a7);
            message(w, id, 0);
                } else {
                    message(w, 0x5ed, 0);
                }
            } else if total < hp {
                if campaign != 1 {
                    anim(w, 0x5ee);
                }
                message(w, 0x5ec, 0);
            } else {
                if campaign != 1 {
                    anim(w, 0x5ee);
                }
                let id = campaign_id(&*w, 3, 0x5eb, 0x11a3);
            message(w, id, 0);
            }
        }
        if total < hp {
            let pct = (hp - total) * 100 / 200 / 2;
            let (mut destroyed, mut damaged) = (0, 0);
            for k in 1..=w.building_count(p) {
                let (kind, damage, b8) = w.building(p, k);
                if b8 != 0 || kind == 0 {
                    continue;
                }
                if kind == 1 || super::rnd(w, 1, 100) > pct {
                    if super::rnd(w, 1, 100) <= pct {
                        let hp_k = w.building_hp(kind);
                        let r = super::rnd(w, 5, 95);
                        let damage = if hp_k * r / 100 < damage { damage } else { hp_k * super::rnd(w, 5, 95) / 100 };
                        w.event(StoryEvent::DamageBuilding { planet: p, slot: k, damage });
                        damaged += 1;
                    }
                } else {
                    w.event(StoryEvent::DeleteBuilding { planet: p, slot: k });
                    destroyed += 1;
                }
            }
            let lost = pct * w.planet_value(p) / 100;
            w.event(StoryEvent::SetPlanetValue { planet: p, value: w.planet_value(p) - lost });
            param_int(w, 1, destroyed);
            param_int(w, 2, damaged);
            param_int(w, 3, lost);
        }
        let f = s.special[1];
        w.event(StoryEvent::DeleteFleet(f));
        s.win(0, true, w);
    }
    Ok(())
}
