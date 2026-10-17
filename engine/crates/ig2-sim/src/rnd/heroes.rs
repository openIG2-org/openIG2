//! `st_Rnd_include`, part 2c: the heroes 176..=192 (spec `re/rules/campaign.md` §8.13). Most offer a fleet for
//! hire (`st_fNewEx` with the story's id, its ship found by `st_hSearch` through the story design key), a yearly
//! fee, battle bonuses while it fights, and on leaving or when its design goes (special 0x19) take their ships
//! along (`st_HajoKigyujt`); the rest sell intelligence, morale or bonuses for a monthly fee.

use super::{add_money, anim, answered, message, param, promoted, read, relation};
use super::battles::{fleet_at, fleet_bit};
use crate::begin::{delete_fleet, move_to_planet, stands};
use crate::brnd::{fleet_strength, random_empire, random_tech, ship_fleet};
use crate::multi::{empire_centre, nearest_planet};
use crate::story::{flag, last_fn, Abort, MessageText, Story, StoryEvent, StoryWorld};

fn add_adat(s: &mut Story, off: usize, x: i32) {
    let v = s.adat_i32(off).wrapping_add(x);
    s.set_adat_i32(off, v);
}

/// `GetRndBirodalom(0, 0, 0)` with its name as parameter `slot` of the last message.
fn empire_param(w: &mut dyn StoryWorld, slot: i32, e: i32) {
    param(w, slot, MessageText::Empire(e));
}

/// `GetIsmertBirodalom(a, b, c)` (exe 0x5c2720): `GetRndBirodalom(a, b, c)`; one the human doesn't know aborts.
fn known_empire(w: &mut dyn StoryWorld, a: i32, b: i32, c: i32) -> Result<i32, Abort> {
    let e = random_empire(w, a, b, c)?;
    let en = w.human();
    if w.knows(en, e) { Ok(e) } else { Err(Abort) }
}

/// `SetMessageFajOverride(e)`: the last message shows empire `e`'s race.
fn race(w: &mut dyn StoryWorld, e: i32) {
    w.event(StoryEvent::MessageRace(e));
}

/// `st_SpWarStartID(f)` (exe 0x602e30): a space battle starts with fleet slot `f` (0: any) among the two.
fn battle_with(s: &Story, w: &dyn StoryWorld, f: i32) -> bool {
    let (a, b) = w.battle_fleets();
    s.flags[flag::SPACE_START] && (f == 0 || a == f || b == f)
}

/// `st_SpWarStartTulaj(e)` (exe 0x602e70): one of the starting battle's two fleets is `e`'s.
fn battle_owner(s: &Story, w: &dyn StoryWorld, e: i32) -> bool {
    let (a, b) = w.battle_fleets();
    s.flags[flag::SPACE_START] && (w.fleet_owner(a) == e || w.fleet_owner(b) == e)
}

/// `st_SpWarPartID(f)` (exe 0x602cf0): fleet slot `f` took part in the finished battle (the first two entries, the
/// third with more than two).
fn battle_part_slot(s: &Story, f: i32) -> bool {
    if !s.flags[flag::SPACE_END] {
        return false;
    }
    let n = if s.battle.len() > 2 { 3 } else { 2 };
    (0..n).any(|i| s.battle.get(i).map_or(0, |p| p.slot_id) == f)
}

/// Special 0x19 (a design deleted): its story key `/ 10` is `story`.
fn design_gone(s: &Story, w: &dyn StoryWorld, story: i32) -> bool {
    s.special_is(0x19, 0, 0, 0) && w.design_key(w.fleet_owner(s.special[1]), s.special[2]) / 10 == story
}

/// The fleet for hire: `st_fNewEx(_s(5, id), owner, id, 0.0, 1, 0)`.
fn hire(s: &mut Story, owner: i32, id: i32, w: &mut dyn StoryWorld) -> i32 {
    s.f_new_ex("", owner, id, 0.0, 1, 0, w)
}

/// `FlottaMozgat(f, 1, 1, last_fn_b, 0)`: to the planet `st_fNewEx` placed it by.
fn to_last_planet(s: &Story, f: i32, w: &mut dyn StoryWorld) {
    let b = s.last_fn[last_fn::B];
    w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: b as f32, y: 0.0 });
}

/// The yearly fee of a hired hero (counter `key`): its ship gone → the end; else message `msg` (`NewMessage(msg,
/// arg)`; or paid at once with `v(paid)`), the next in 360 days.
fn fee(s: &mut Story, w: &mut dyn StoryWorld, key: i32, paid: i32, (msg, arg): (i32, i32), price: i32) {
    if !s.counter(key) {
        return;
    }
    let en = w.human();
    if ship_fleet(&*w, en, key) == 0 {
        s.win(0, true, w);
    } else {
        if s.v_get(paid) == 0 {
            message(w, msg, arg);
        } else {
            add_money(w, -price as f64);
        }
        s.sz_start(key, 0x168);
    }
}

/// Answers 2 and 4 of the fee's message pay it (4: from now on at once, `v(paid) := 1`).
fn fee_paid(s: &mut Story, w: &mut dyn StoryWorld, msg: i32, paid: i32, price: i32) {
    if answered(s, msg, 2) || answered(s, msg, 4) {
        if answered(s, msg, 4) {
            s.v_set(paid, 1);
        }
        add_money(w, -price as f64);
    }
}

impl Story {
    /// `st_HajoKigyujt(story, owner)` (exe 0x5eb040): the ships (then the units) whose design's story key `/ 10` is
    /// `story` and whose fleet isn't `owner`'s move to one new fleet of `owner` (`st_fNewEx("", owner, 0, 0, −1, 0)`,
    /// placed on the first such fleet): each is deleted and made anew of `owner`'s design for the key
    /// (`st_GetTerv(owner, key, 3)`). The loops read the counts afresh. `last_hajo_kigyujt_flotta` := the fleet
    /// of the last ship taken. Returns the new fleet (0: none).
    pub fn collect_ships(&mut self, story: i32, owner: i32, w: &mut dyn StoryWorld) -> i32 {
        let mut nf = 0;
        let mut k = 1;
        while k <= w.ship_count() {
            let (f, d) = w.ship(k);
            if d != 0x7fff {
                let o = w.fleet_owner(f);
                let key = w.design_key(o, d);
                if key / 10 == story && o != owner {
                    self.last_collect_fleet = f;
                    if nf == 0 {
                        nf = self.f_new_ex("", owner, 0, 0.0, -1, 0, w);
                        let (x, y) = w.fleet_xy(f);
                        w.event(StoryEvent::SetFleetPos { fleet: nf, x, y });
                    }
                    w.event(StoryEvent::DeleteShip(k));
                    let design = w.story_design(owner, key, 3);
                    w.event(StoryEvent::NewShip { fleet: nf, design });
                }
            }
            k += 1;
        }
        let mut k = 1;
        while k <= w.tank_count() {
            let (place, d, count) = w.tank(k);
            if d != 0x7fff && place > 0 {
                let o = w.fleet_owner(place);
                let key = w.design_key(o, d);
                if key / 10 == story && o != owner {
                    if nf == 0 {
                        nf = self.f_new_ex("", owner, 0, 0.0, -1, 0, w);
                        let (x, y) = w.fleet_xy(place);
                        w.event(StoryEvent::SetFleetPos { fleet: nf, x, y });
                    }
                    w.event(StoryEvent::DeleteTanks { place, design: d, count });
                    let design = w.story_design(owner, key, 3);
                    w.event(StoryEvent::NewTanks { fleet: nf, design, count });
                }
            }
            k += 1;
        }
        nf
    }

    /// A hero leaves: `st_HajoKigyujt(story, owner)` (none → the abort), the new fleet named `_s(5, id)`; the
    /// fleet it came from, still named so, loses its name (`+0x52 &= ~0x800`, `AlapFlottaNev`); the new fleet is
    /// sent away (`st_fEliminate`).
    fn hero_leaves(&mut self, story: i32, owner: i32, id: i32, w: &mut dyn StoryWorld) -> Result<(), Abort> {
        let f = self.collect_ships(story, owner, w);
        if f == 0 {
            return Err(Abort);
        }
        w.event(StoryEvent::NameFleet { fleet: f, text: Some(id) });
        let last = self.last_collect_fleet;
        if last != 0 && w.fleet_named(last, id) {
            w.event(StoryEvent::FleetFlags { fleet: last, set: 0, clear: 0x800 });
            w.event(StoryEvent::NameFleet { fleet: last, text: None });
        }
        self.send_away(f.wrapping_neg(), w);
        Ok(())
    }
}

// ---------------------------------------------------------------- 176, 179, 181, 182: hired fleets

/// 176: after `rnd(1700, 2200)` days an empire's fleet is for hire (message 0x6e1, its name): 3,000 and a yearly
/// 3,000 (message 0x6e2; answer 4 pays from then on); in a battle it starts `StoryAdat+0x84 += 14`.
pub(super) fn hero_176(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x6e1;
    const FEE: i32 = 0x6e2;
    const PAID: i32 = 0x6e8;
    const PRICE: i32 = 3000;
    let en = w.human();
    let mut leaves = false;
    if s.starting_now() {
        s.v_set(PAID, 0);
        let days = super::rnd(w, 0x6a4, 0x898);
        s.sz_start(0x6e9, days);
        s.v_set(ID, 0);
        s.add_undo(10, FEE, 0xb0);
    }
    if s.counter(0x6e9) {
        message(w, ID, 0x3c);
        let e = random_empire(w, 0, 0, 0)?;
        empire_param(w, 1, e);
    }
    if answered(s, ID, 2) {
        add_money(w, -PRICE as f64);
        hire(s, en, ID, w);
        s.v_set(ID, 1);
        let f = s.f_search(ID, w);
        to_last_planet(s, f, w);
        s.sz_start(ID, 0x168);
        s.remove_fleet_id(ID, w);
    }
    if answered(s, ID, 3) || promoted(s, ID) {
        s.win(0, true, w);
    }
    fee(s, w, ID, PAID, (FEE, 0x3c), PRICE);
    fee_paid(s, w, FEE, PAID, PRICE);
    if answered(s, FEE, 3) {
        leaves = true;
        s.sz_stop(ID);
        s.win(0, true, w);
    }
    if promoted(s, FEE) {
        leaves = true;
        s.sz_stop(ID);
        message(w, 0x6e3, 0);
        s.win(0, true, w);
    }
    if s.flags[flag::SPACE_START] && s.v_get(ID) != 0 {
        let f = ship_fleet(&*w, en, ID);
        if f == 0 {
            s.win(0, true, w);
        } else if battle_with(s, &*w, f) {
            add_adat(s, 0x84, 14);
        }
    }
    if design_gone(s, &*w, 0xb0) {
        leaves = true;
        s.win(0, true, w);
    }
    if leaves {
        s.hero_leaves(0xb0, 10, ID, w)?;
    }
    Ok(())
}

/// 179: with tech 78 and an attack above 4,500 (else `rnd(490, 1200)` days later) a fleet is for hire (message
/// 0x6ff): 17,000 and a yearly 17,000 (message 0x700); refused, it joins a random empire; in a battle it starts
/// `StoryAdat+0x84 += 13`, `+0x90 += 18`.
pub(super) fn hero_179(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x6ff;
    const FEE: i32 = 0x700;
    const PAID: i32 = 0x706;
    const PRICE: i32 = 17000;
    let en = w.human();
    let mut leaves = false;
    if s.starting_now() {
        s.v_set(ID, 0);
        s.v_set(PAID, 0);
        s.sz_start(0x707, 1);
        s.add_undo(10, FEE, 0xb3);
    }
    if s.counter(0x707) {
        if w.tech_status(78, en) == 5 && fleet_strength(&*w) / 3 > 1500 {
            message(w, ID, 0x3c);
        } else {
            let days = super::rnd(w, 0x1ea, 0x4b0);
            s.sz_start(0x707, days);
        }
    }
    if answered(s, ID, 2) {
        add_money(w, -PRICE as f64);
        hire(s, en, ID, w);
        s.v_set(ID, 1);
        let f = s.f_search(ID, w);
        to_last_planet(s, f, w);
        s.remove_fleet_id(ID, w);
        s.sz_start(ID, 0x168);
    }
    if answered(s, ID, 3) {
        s.win(0, true, w);
    }
    if answered(s, ID, 4) || promoted(s, ID) {
        let e = random_empire(w, 0, 0, 0)?;
        s.f_new_ex("", e, ID, 0.0, 2, 0, w);
        s.remove_fleet_id(ID, w);
        s.win(0, true, w);
    }
    fee(s, w, ID, PAID, (FEE, 0), PRICE);
    fee_paid(s, w, FEE, PAID, PRICE);
    if answered(s, FEE, 3) || promoted(s, FEE) {
        leaves = true;
        s.win(0, true, w);
    }
    if s.flags[flag::SPACE_START] && s.v_get(ID) != 0 {
        let f = ship_fleet(&*w, en, ID);
        if f == 0 {
            s.win(0, true, w);
        } else if battle_with(s, &*w, f) {
            add_adat(s, 0x84, 13);
            add_adat(s, 0x90, 18);
        }
    }
    if design_gone(s, &*w, 0xb3) {
        leaves = true;
        s.win(0, true, w);
    }
    if leaves {
        s.hero_leaves(0xb3, 10, ID, w)?;
    }
    Ok(())
}

/// 181: with tech 9, more than 50,000 and an attack of 9,003 or more (else 570 days later) an empire's fleet is for
/// hire (message 0x713, its name): 25,000 and a yearly 25,000 (message 0x714); in a battle it starts
/// `StoryAdat+0x7c += 18`, `+0x90 += 20`; leaving, its ships go to that empire.
pub(super) fn hero_181(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x713;
    const FEE: i32 = 0x714;
    const PAID: i32 = 0x71a;
    const HIRED: i32 = 0x71b;
    const PRICE: i32 = 25000;
    let en = w.human();
    let mut leaves = false;
    if s.starting_now() {
        s.v_set(HIRED, 0);
        s.v_set(PAID, 0);
        s.add_undo(10, FEE, 0xb5);
        s.sz_start(HIRED, 1);
    }
    if s.counter(HIRED) {
        if w.tech_status(9, en) != 5 || w.money(en) <= 50000.0 || fleet_strength(&*w) / 3 < 0xbb9 {
            s.sz_start(HIRED, 0x23a);
        } else {
            let e = random_empire(w, 0, 0, 0)?;
            message(w, ID, 0x3c);
            s.v_set(ID, e);
            empire_param(w, 1, e);
        }
    }
    if answered(s, ID, 2) || answered(s, ID, 5) {
        let f = hire(s, en, ID, w);
        s.remove_fleet_id(ID, w);
        to_last_planet(s, f, w);
        add_money(w, -PRICE as f64);
        s.sz_start(ID, 0x168);
        s.sz_start(0x715, 100);
        s.v_set(HIRED, 1);
    }
    if answered(s, ID, 4) || answered(s, ID, 6) || promoted(s, ID) {
        s.win(0, true, w);
    }
    if (s.battle_part(0) != 0 || s.daily()) && s.v_get(HIRED) != 0 && ship_fleet(&*w, en, ID) == 0 {
        w.event(StoryEvent::RemoveMessage(FEE));
        w.event(StoryEvent::RemoveMessage(0x716));
        s.win(0, true, w);
    }
    fee(s, w, ID, PAID, (FEE, 0x3c), PRICE);
    fee_paid(s, w, FEE, PAID, PRICE);
    if answered(s, FEE, 3) {
        leaves = true;
        s.sz_stop(ID);
        s.win(0, true, w);
    }
    if promoted(s, FEE) {
        message(w, 0x715, 0);
        s.sz_stop(ID);
        leaves = true;
        s.win(0, true, w);
    }
    s.counter(0x715);
    if answered(s, 0x716, 2) || promoted(s, 0x716) {
        let e = s.v_get(ID);
        relation(w, en, e, -10);
    }
    if answered(s, 0x716, 3) {
        leaves = true;
        s.win(0, true, w);
    }
    if s.flags[flag::SPACE_START] && s.v_get(HIRED) != 0 {
        let f = ship_fleet(&*w, en, ID);
        if f != 0 && battle_with(s, &*w, f) {
            add_adat(s, 0x7c, 18);
            add_adat(s, 0x90, 20);
        }
    }
    if design_gone(s, &*w, 0xb5) {
        leaves = true;
        s.win(0, true, w);
    }
    if leaves {
        let e = s.v_get(ID);
        s.hero_leaves(0xb5, e, ID, w)?;
    }
    Ok(())
}

/// 182: with tech 10 and an attack above 6,000 (else 590 days later) a fleet is for hire (message 0x71d): 28,000 and
/// a yearly 28,000 (message 0x71e). The battle bonus looks for 181's ship (the source's slip): `StoryAdat+0x7c
/// += 28`, `+0x90 += 22`.
pub(super) fn hero_182(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x71d;
    const FEE: i32 = 0x71e;
    const PAID: i32 = 0x724;
    const PRICE: i32 = 28000;
    let en = w.human();
    let mut leaves = false;
    if s.starting_now() {
        s.v_set(ID, 0);
        s.v_set(PAID, 0);
        s.sz_start(0x725, 1);
        s.add_undo(10, FEE, 0xb6);
    }
    if s.counter(0x725) {
        if w.tech_status(10, en) == 5 && fleet_strength(&*w) / 3 > 2000 {
            message(w, ID, 0x3c);
        } else {
            s.sz_start(0x725, 0x24e);
        }
    }
    if answered(s, ID, 2) {
        let f = hire(s, en, ID, w);
        to_last_planet(s, f, w);
        s.remove_fleet_id(ID, w);
        add_money(w, -PRICE as f64);
        s.sz_start(ID, 0x168);
        s.v_set(ID, 1);
    }
    if s.daily() && s.v_get(ID) != 0 && ship_fleet(&*w, en, ID) == 0 {
        s.win(0, true, w);
    }
    if answered(s, ID, 3) || promoted(s, ID) {
        s.win(0, true, w);
    }
    fee(s, w, ID, PAID, (FEE, 0x3c), PRICE);
    fee_paid(s, w, FEE, PAID, PRICE);
    if answered(s, FEE, 3) {
        leaves = true;
        s.sz_stop(ID);
        s.win(0, true, w);
    }
    if promoted(s, FEE) {
        leaves = true;
        message(w, 0x71f, 0);
        s.sz_stop(ID);
        s.win(0, true, w);
    }
    if s.flags[flag::SPACE_START] && s.v_get(ID) != 0 {
        let f = ship_fleet(&*w, en, 0x713);
        if f != 0 && battle_with(s, &*w, f) {
            add_adat(s, 0x7c, 28);
            add_adat(s, 0x90, 22);
        }
    }
    if design_gone(s, &*w, 0xb6) {
        leaves = true;
        s.win(0, true, w);
    }
    if leaves {
        s.hero_leaves(0xb6, 10, ID, w)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- 177, 178, 180, 184: heroes with a story

/// 177: with tech 7 and an attack above 960 (else 500 days later) a fleet is for hire (message 0x6eb): 8,500, then
/// yearly 8,500 (message 0x6ec) until it has fought twice (counted on finished battles), then 10,000 (message
/// 0x6f1, answer 4 pays from then on). After its second battle it asks for more (message 0x6ee, the fee counter cut
/// to 120 days). Turned down, it leaves (counter 1772: message 0x6f0, `rnd(1, 100) < 70` draws an empire). The
/// battle bonus `StoryAdat+0x90 += 10` goes to a battle without its ship (the source's slip).
pub(super) fn hero_177(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x6eb;
    const BATTLES: i32 = 0x6ea;
    const LATER: i32 = 0x6ec;
    const MORE: i32 = 0x6ee;
    const FEE2: i32 = 0x6f1;
    const PAID: i32 = 0x6f2;
    let en = w.human();
    let mut leaves = false;
    if s.starting_now() {
        s.sz_start(0x6f3, 1);
        s.v_set(BATTLES, 0);
        s.v_set(ID, 0);
        s.v_set(PAID, 0);
        s.add_undo(10, LATER, 0xb1);
        s.add_undo(10, MORE, 0xb1);
        s.add_undo(10, FEE2, 0xb1);
    }
    if s.counter(0x6f3) {
        if w.tech_status(7, en) == 5 && fleet_strength(&*w) / 3 > 0x140 {
            message(w, ID, 0x78);
        } else {
            s.sz_start(0x6f3, 500);
        }
    }
    if answered(s, ID, 2) {
        add_money(w, -8500.0);
        let f = hire(s, en, ID, w);
        s.v_set(ID, 1);
        to_last_planet(s, f, w);
        s.remove_fleet_id(ID, w);
        s.sz_start(ID, 0x168);
    }
    if answered(s, ID, 3) || promoted(s, ID) {
        s.win(0, true, w);
    }
    if s.battle_part(0) != 0 && s.v_get(ID) != 0 {
        let f = ship_fleet(&*w, en, ID);
        if f == 0 {
            s.win(0, true, w);
        } else if battle_part_slot(s, f) {
            s.v_inc(BATTLES);
            if s.v_get(BATTLES) == 2 {
                message(w, MORE, 100);
                let left = s.sz_get(ID);
                if 0 < left && left < 0x78 {
                    s.sz_start(ID, 0x78);
                }
            }
        }
    }
    if answered(s, MORE, 4) {
        s.v_set(PAID, 1);
    }
    if answered(s, MORE, 3) {
        leaves = true;
        s.sz_start(LATER, 10);
    }
    if promoted(s, MORE) {
        leaves = true;
        s.sz_start(LATER, 10);
        message(w, 0x6ef, 0);
    }
    if s.counter(LATER) {
        message(w, 0x6f0, 0);
        if super::rnd(w, 1, 100) < 0x46 {
            random_empire(w, 0, 0, 0)?;
        }
        s.win(0, true, w);
    }
    if s.counter(ID) {
        if ship_fleet(&*w, en, ID) == 0 {
            s.win(0, true, w);
        } else {
            if s.v_get(BATTLES) < 2 {
                message(w, LATER, 0x3c);
            } else if s.v_get(PAID) == 0 {
                message(w, FEE2, 0);
            } else {
                add_money(w, -10000.0);
            }
            s.sz_start(ID, 0x168);
        }
    }
    if answered(s, LATER, 2) {
        add_money(w, -8500.0);
    }
    fee_paid(s, w, FEE2, PAID, 10000);
    if answered(s, LATER, 3) || answered(s, FEE2, 3) {
        leaves = true;
        s.sz_stop(ID);
        s.sz_start(LATER, 0xf);
    }
    if promoted(s, LATER) || promoted(s, FEE2) {
        leaves = true;
        s.sz_stop(ID);
        s.sz_start(LATER, 0xf);
        message(w, 0x6ef, 0);
    }
    if s.flags[flag::SPACE_START] && s.v_get(ID) != 0 && ship_fleet(&*w, en, ID) == 0 && battle_with(s, &*w, 0) {
        add_adat(s, 0x90, 10);
    }
    if design_gone(s, &*w, 0xb1) {
        leaves = true;
        s.win(0, true, w);
    }
    if leaves {
        s.hero_leaves(0xb1, 10, ID, w)?;
    }
    Ok(())
}

/// The ships and units of `owner`'s fleets whose design's story key `/ 10` is `story` (178's search).
fn has_story_units(story: i32, w: &dyn StoryWorld) -> bool {
    let ships = (1..=w.ship_count()).any(|k| {
        let (f, d) = w.ship(k);
        d != 0x7fff && w.design_key(w.fleet_owner(f), d) / 10 == story
    });
    let tanks = (1..=w.tank_count()).any(|k| {
        let (place, d, _) = w.tank(k);
        d != 0x7fff && place > 0 && w.design_key(w.fleet_owner(place), d) / 10 == story
    });
    ships || tanks
}

/// 178: with techs 8 and 148 and an attack above 3,000 (else 530 days later) a pirate's fleet offers to join
/// (message 0x6f5, answer 6: 20,000): it comes to the planet `st_fNewEx` placed it by and, arriving, becomes the
/// human's (its ships' and units' story designs made the human's, `st_GetTerv(EN, key, 3)`); 900 days later it
/// remembers (message 0x6f8 while any of its ships lives, else the end), 1,000 days later it leaves: every fleet
/// of the human's gives its 178 ships and units to a new pirate fleet, which fills its hold with design 0x6f6 and
/// goes (message 0x6f7, +10,000). Fighting the human on the way or after, it goes at once (0x6f6, 0x6f9).
pub(super) fn hero_178(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x6f5;
    const JOINED: i32 = 0x6fc;
    const COMING: i32 = 0x6fd;
    let en = w.human();
    let mut leaves = false;
    if s.starting_now() || s.counter(COMING) {
        s.v_set(JOINED, 0);
        if w.tech_status(8, en) == 5 && w.tech_status(148, en) == 5 && fleet_strength(&*w) / 3 > 1000 {
            message(w, ID, 0x3c);
        } else {
            s.sz_start(COMING, 0x212);
        }
    }
    if answered(s, ID, 6) {
        add_money(w, -20000.0);
        let f = hire(s, 10, ID, w);
        to_last_planet(s, f, w);
        s.v_set(COMING, 1);
    }
    if answered(s, ID, 3) || answered(s, ID, 5) || answered(s, ID, 7) || promoted(s, ID) {
        s.win(0, true, w);
    }
    if s.daily() && s.v_get(COMING) != 0 {
        let f = s.f_search(ID, w);
        if f == 0 {
            s.win(0, true, w);
        } else if stands(s, f.wrapping_neg(), w) {
            let p = s.b_search(en.wrapping_neg(), 0, w);
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: p as f32, y: 0.0 });
        }
    }
    if s.battle_part(ID) != 0 && s.v_get(COMING) != 0 {
        let f = s.f_search(ID, w);
        if f != 0 {
            s.send_away(f.wrapping_neg(), w);
        }
        s.win(0, true, w);
    }
    if s.battle_part(0) != 0 && s.v_get(JOINED) != 0 {
        let g = ship_fleet(&*w, en, ID);
        if g == 0 {
            s.win(0, true, w);
        } else if battle_part_slot(s, g) {
            message(w, 0x6f6, 0);
            leaves = true;
            s.win(0, true, w);
        }
    }
    if s.ground_part(0, 0, w) && s.v_get(JOINED) != 0 {
        let g = ship_fleet(&*w, en, ID);
        if g != 0 && s.ground.fleet == g {
            message(w, 0x6f9, 0);
            leaves = true;
        }
        s.win(0, true, w);
    }
    if s.arrived_planet(ID, 0, w) && s.v_get(COMING) != 0 {
        s.v_set(COMING, 0);
        s.v_set(JOINED, 1);
        let f = s.f_search(ID, w);
        if f == 0 {
            return Err(Abort);
        }
        let o = w.fleet_owner(f);
        for k in 1..=w.ship_count() {
            let (g, d) = w.ship(k);
            if d != 0x7fff && g == f {
                let key = w.design_key(o, d);
                if key != 0 {
                    let design = w.story_design(en, key, 3);
                    w.event(StoryEvent::UpgradeShip { ship: k, design });
                }
            }
        }
        for k in 1..=w.tank_count() {
            let (g, d, _) = w.tank(k);
            if d != 0x7fff && g == f {
                let key = w.design_key(o, d);
                if key != 0 {
                    let design = w.story_design(en, key, 3);
                    w.event(StoryEvent::SetTankDesign { tank: k, design });
                }
            }
        }
        w.event(StoryEvent::SetFleetOwner { fleet: f, owner: en });
        s.remove_fleet_id(ID, w);
        s.sz_start(ID, 1000);
        s.sz_start(0x6f6, 900);
    }
    if s.counter(0x6f6) {
        if has_story_units(0xb2, &*w) {
            message(w, 0x6f8, 0);
        } else {
            s.win(0, true, w);
        }
    }
    if design_gone(s, &*w, 0xb2) {
        leaves = true;
        s.win(0, true, w);
    }
    if leaves || s.counter(ID) {
        let mut any = false;
        let mut a = 1;
        while a <= w.fleet_count() {
            if w.fleet_owner(a) == en {
                let mut nf = 0;
                let mut k = 1;
                while k <= w.ship_count() {
                    let (g, d) = w.ship(k);
                    if g == a && d != 0x7fff && w.design_key(en, d) / 10 == 0xb2 && w.fleet_owner(g) == en {
                        if nf == 0 {
                            nf = s.f_new_ex("", 10, 0, 0.0, -1, 0, w);
                            let (x, y) = w.fleet_xy(a);
                            w.event(StoryEvent::SetFleetPos { fleet: nf, x, y });
                        }
                        let design = w.story_design(10, w.design_key(en, d), 3);
                        w.event(StoryEvent::DeleteShip(k));
                        w.event(StoryEvent::NewShip { fleet: nf, design });
                    }
                    k += 1;
                }
                let mut k = 1;
                while k <= w.tank_count() {
                    let (g, d, count) = w.tank(k);
                    if g == a && d != 0x7fff && w.design_key(en, d) / 10 == 0xb2 && w.fleet_owner(g) == en {
                        if nf == 0 {
                            nf = s.f_new_ex("", 10, 0, 0.0, -1, 0, w);
                            let (x, y) = w.fleet_xy(a);
                            w.event(StoryEvent::SetFleetPos { fleet: nf, x, y });
                        }
                        let design = w.story_design(10, w.design_key(en, d), 3);
                        w.event(StoryEvent::NewTanks { fleet: nf, design, count });
                        w.event(StoryEvent::DeleteTanks { place: a, design: d, count });
                    }
                    k += 1;
                }
                if nf != 0 {
                    let (units, room) = w.fleet_units(nf);
                    for _ in 0..units - room {
                        let design = w.story_design(10, 0x6f6, 3);
                        w.event(StoryEvent::NewShip { fleet: nf, design });
                    }
                    if w.fleet_named(a, ID) {
                        w.event(StoryEvent::FleetFlags { fleet: a, set: 0, clear: 0x800 });
                        w.event(StoryEvent::NameFleet { fleet: a, text: None });
                    }
                    s.send_away(nf.wrapping_neg(), w);
                    any = true;
                }
            }
            a += 1;
        }
        if s.counter(ID) && any {
            message(w, 0x6f7, 0);
            add_money(w, 10000.0);
            s.win(0, true, w);
        }
    }
    Ok(())
}

/// 180: the human's strongest ship design without a story key (hull class `+0x16 == 1`) gets a famous captain
/// (message 0x709, the design's name): answered 2 or 4, a copy of it with story key 0x709 (`TervCopy` into
/// `st_GetTerv(EN, 0x709, 3)`, `+0x18 := 0`, `+0x19 := 1`, named `_s(5, 0x709)`) replaces the first such ship;
/// in a battle it starts `StoryAdat+0x7c += 30`; its design deleted (special 0x19) gives the design back its tech's
/// name, key 0, `+0x18 := 1`, `+0x19 := 0`.
pub(super) fn hero_180(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x709;
    const DESIGN: i32 = 0x70a;
    let en = w.human();
    if s.starting_now() {
        let (mut best, mut attack) = (0, 0);
        for k in 1..=w.ship_count() {
            let (f, d) = w.ship(k);
            if d != 0x7fff
                && w.fleet_owner(f) == en
                && w.design_kind(en, d) == 1
                && w.design_key(en, d) == 0
                && attack < w.design_attack(en, d)
            {
                attack = w.design_attack(en, d);
                best = d;
            }
        }
        if best == 0 {
            s.win(0, false, w);
        } else {
            message(w, ID, 0x3c);
            param(w, 1, MessageText::Design { empire: en, design: best });
            s.v_set(ID, 0);
            s.v_set(DESIGN, best);
        }
    }
    if answered(s, ID, 2) || answered(s, ID, 4) {
        let t = w.story_design(en, ID, 3);
        let from = s.v_get(DESIGN);
        w.event(StoryEvent::CopyDesign { empire: en, from, to: t });
        w.event(StoryEvent::SetDesignKey { empire: en, design: t, key: ID });
        w.event(StoryEvent::DesignFlags { empire: en, design: t, b18: 0, b19: 1 });
        w.event(StoryEvent::NameDesign { empire: en, design: t, text: Some(ID) });
        for k in 1..=w.ship_count() {
            let (f, d) = w.ship(k);
            if d == from && w.fleet_owner(f) == en {
                w.event(StoryEvent::UpgradeShip { ship: k, design: t });
                break;
            }
        }
        s.v_set(ID, 1);
    }
    if s.flags[flag::SPACE_START] && s.v_get(ID) != 0 {
        let f = ship_fleet(&*w, en, ID);
        if f == 0 {
            s.win(0, true, w);
        } else if battle_with(s, &*w, f) {
            add_adat(s, 0x7c, 30);
        }
    }
    if design_gone(s, &*w, 0xb4) {
        let t = w.story_design(en, ID, 3);
        w.event(StoryEvent::NameDesign { empire: en, design: t, text: None });
        w.event(StoryEvent::SetDesignKey { empire: en, design: t, key: 0 });
        w.event(StoryEvent::DesignFlags { empire: en, design: t, b18: 1, b19: 0 });
        s.win(0, true, w);
    }
    if answered(s, ID, 5) || promoted(s, ID) {
        s.win(0, true, w);
    }
    Ok(())
}

/// 184: with tech 11 and an attack above 9,000 (else 610 days later) a mercenary fleet is for hire (message 0x731):
/// two years for 100,000 or one for 50,000 (to the coordinates of the planet `st_fNewEx` placed it by), then yearly
/// 50,000 (message 0x732). Turned down, `rnd(0, 99) < 70` sends it to a random empire instead. Dismissed (0x732
/// answer 3: `rnd(1, 2)` decides between leaving and revenge) or promoted (revenge, message 0x733) its ships gather
/// in fleet 1842 (`st_HajoKigyujt(0xb8, 10)`); for revenge it heads for a planet of a random empire (id 1841) and,
/// arriving, its ships go to that empire. The battle bonus looks for 181's ship (`StoryAdat+0x7c += 20`, `+0x90
/// += 6`).
pub(super) fn hero_184(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x731;
    const FEE: i32 = 0x732;
    const PAID: i32 = 0x738;
    const TARGET: i32 = 0x739;
    const PRICE: i32 = 50000;
    let en = w.human();
    let (mut lose, mut revenge, mut arrived, mut to) = (false, false, false, 0);
    if s.starting_now() {
        s.v_set(TARGET, 0);
        s.add_undo(10, FEE, 0xb8);
        s.sz_start(TARGET, 1);
    }
    if s.counter(TARGET) {
        if w.tech_status(11, en) == 5 && fleet_strength(&*w) / 3 > 3000 {
            message(w, ID, 0x3c);
        } else {
            s.sz_start(TARGET, 0x262);
        }
    }
    for (part, price, days) in [(2, PRICE * 2, 0x2d0), (3, PRICE, 0x168)] {
        if answered(s, ID, part) {
            let f = hire(s, en, ID, w);
            let (x, y) = w.planet_xy(s.last_fn[last_fn::B]);
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 0, x, y });
            add_money(w, -price as f64);
            s.sz_start(ID, days);
        }
    }
    if answered(s, ID, 4) || promoted(s, ID) {
        if super::rnd(w, 0, 99) < 0x46 {
            let e = random_empire(w, 0, 0, 0)?;
            let p = s.b_search(e.wrapping_neg(), 0, w);
            let f = s.f_new_ex("", e, ID, 0.0, 2, p, w);
            w.event(StoryEvent::SetFleetId { fleet: f, id: 0 });
        }
        s.win(0, true, w);
    }
    fee(s, w, ID, PAID, (FEE, 0x3c), PRICE);
    fee_paid(s, w, FEE, PAID, PRICE);
    if answered(s, FEE, 3) {
        to = 10;
        s.sz_stop(ID);
        if super::rnd(w, 1, 2) == 1 {
            lose = true;
            s.win(0, true, w);
        } else {
            revenge = true;
        }
    }
    if promoted(s, FEE) {
        revenge = true;
        to = 10;
        message(w, 0x733, 0);
        s.sz_stop(ID);
    }
    if s.arrived_planet(FEE, 0, w) && s.v_get(TARGET) != 0 {
        to = s.v_get(TARGET);
        arrived = true;
        s.win(0, true, w);
    }
    if s.daily() && s.v_get(TARGET) != 0 {
        let f = s.f_search(FEE, w);
        if f == 0 {
            s.win(0, true, w);
        } else if stands(s, f.wrapping_neg(), w) {
            move_to_planet(s, FEE, ID, w);
        }
    }
    if s.flags[flag::SPACE_START] {
        let f = ship_fleet(&*w, en, 0x713);
        if f != 0 && battle_with(s, &*w, f) {
            add_adat(s, 0x7c, 20);
            add_adat(s, 0x90, 6);
        }
    }
    if design_gone(s, &*w, 0xb8) {
        to = 10;
        lose = true;
        s.win(0, true, w);
    }
    if to != 0 {
        let f = s.collect_ships(0xb8, to, w);
        if f == 0 {
            return Err(Abort);
        }
        w.event(StoryEvent::NameFleet { fleet: f, text: Some(ID) });
        w.event(StoryEvent::SetFleetId { fleet: f, id: FEE });
        if arrived {
            w.event(StoryEvent::SetFleetId { fleet: f, id: 0 });
        } else if revenge {
            let e = random_empire(w, 0, 0, 0)?;
            s.v_set(TARGET, e);
            let p = s.b_search(e.wrapping_neg(), 0, w);
            s.b_add_id(p, ID);
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: p as f32, y: 0.0 });
        } else if lose {
            s.send_away(f.wrapping_neg(), w);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 183, 185..=192: offers and events

/// 183: a known empire (not 5) offers its advisers (message 0x727, its race and name): answer 4 (4,000 a half year:
/// `StoryAdat+0xa0`, `+0xa4` `+= 1`, `+0x6c += 20`) or 7/9 (3,400 once: a random tech); 3, 6 and 10 decline
/// (relation +2). Nine half years at most; the fee comes as message 0x728 (first) or 0x72a, answers 6/4 pay at once
/// from then on; declined or lapsed (0x729) the bonuses go. `rnd(1, 100) > 60` starts counter 1832 (20 days).
pub(super) fn advisers_183(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x727;
    const FIRST: i32 = 0x728;
    const AGAIN: i32 = 0x72a;
    const PAID: i32 = 0x72d;
    const TIMES: i32 = 0x72e;
    const EMPIRE: i32 = 0x72f;
    let en = w.human();
    let mut ends = false;
    if s.starting_now() {
        let e = known_empire(w, 5, 0, 0)?;
        s.v_set(EMPIRE, e);
        message(w, ID, 0x3c);
        race(w, s.v_get(EMPIRE));
        empire_param(w, 1, s.v_get(EMPIRE));
        s.v_set(TIMES, 0);
        s.v_set(PAID, 0);
    }
    if answered(s, ID, 4) {
        add_money(w, -4000.0);
        s.v_set(ID, 1);
        s.v_set(FIRST, 1);
        add_adat(s, 0xa0, 1);
        add_adat(s, 0xa4, 1);
        add_adat(s, 0x6c, 20);
        s.sz_start(ID, 0xb4);
        if super::rnd(w, 1, 100) > 0x3c {
            s.sz_start(FIRST, 0x14);
        }
    }
    if answered(s, ID, 7) || answered(s, ID, 9) {
        add_money(w, -3400.0);
        s.v_set(ID, 1);
        let t = random_tech(w, en, 0);
        if t != 0 {
            s.set_tech_state(t, en, w);
        }
        s.sz_start(ID, 0xb4);
        if super::rnd(w, 1, 100) > 0x3c {
            s.sz_start(FIRST, 0x14);
        }
    }
    if answered(s, ID, 3) || answered(s, ID, 6) || answered(s, ID, 10) {
        let e = s.v_get(EMPIRE);
        relation(w, en, e, 2);
        s.win(0, true, w);
    }
    if promoted(s, ID) {
        s.win(0, true, w);
    }
    if s.counter(ID) {
        let n = s.v_get(TIMES) + 1;
        s.v_set(TIMES, n);
        if n < 9 {
            s.sz_start(ID, 0xb4);
        }
        if s.v_get(PAID) == 0 {
            if s.v_get(FIRST) == 0 {
                message(w, FIRST, 0x3c);
                race(w, s.v_get(EMPIRE));
                s.v_set(FIRST, 1);
            } else {
                message(w, AGAIN, 0x3c);
                race(w, s.v_get(EMPIRE));
            }
        } else {
            add_money(w, -4000.0);
        }
    }
    if answered(s, FIRST, 2) || answered(s, FIRST, 4) || answered(s, AGAIN, 2) || answered(s, FIRST, 6) || answered(s, AGAIN, 4) {
        add_money(w, -4000.0);
        if answered(s, FIRST, 6) || answered(s, AGAIN, 4) {
            s.v_set(PAID, 1);
        }
    }
    if answered(s, FIRST, 5) || answered(s, AGAIN, 3) {
        ends = true;
        s.sz_stop(ID);
        s.win(0, true, w);
    }
    if promoted(s, FIRST) || promoted(s, AGAIN) {
        message(w, 0x729, 0);
        race(w, s.v_get(EMPIRE));
        ends = true;
        s.sz_stop(ID);
        s.win(0, true, w);
    }
    if ends {
        add_adat(s, 0x6c, -20);
        add_adat(s, 0xa0, -1);
        add_adat(s, 0xa4, -1);
    }
    Ok(())
}

/// 185: an empire's maps for sale (message 0x73b, its name; 1,400): the human sees that empire's planets it knows and
/// the five free planets it knows nearest to them (squared distances as the exe rounds them; an insertion list of
/// ten), `StoryAdat+0x9c
/// += 1`. 100 days later, the empire alive: message 0x73c (its race and name): answer 2 or lapsing costs relations
/// (−9), answer 3 takes the bonus back.
pub(super) fn maps_185(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x73b;
    const EMPIRE: i32 = 0x743;
    let en = w.human();
    if s.starting_now() {
        let e = random_empire(w, 0, 0, 0)?;
        s.v_set(EMPIRE, e);
        message(w, ID, 0x3c);
        empire_param(w, 1, e);
    }
    if answered(s, ID, 2) {
        add_money(w, -1400.0);
        s.sz_start(ID, 100);
        let e = s.v_get(EMPIRE);
        let (mut best, mut dist) = ([0i32; 10], [10000i32; 10]);
        for p in 1..=w.planet_count() {
            let owner = w.planet_owner(p);
            if owner == e && w.planet_visibility(p) >= 0 {
                w.event(StoryEvent::SetPlanetView { planet: p, view: 1 });
            } else if owner == 0 && w.planet_visibility(p) >= 0 {
                let (px, py) = w.planet_xy(p);
                let mut d = 20000;
                for q in 1..=w.planet_count() {
                    if w.planet_owner(q) == e && w.planet_visibility(q) >= 0 {
                        // The exe: `sqr` (x87) of the y difference stored as f32, the x one's added at double
                        // precision, the sum stored as f32 and truncated (`cvttss2si`).
                        let (qx, qy) = w.planet_xy(q);
                        let (dx, dy) = ((qx - px) as f64, (qy - py) as f64);
                        let dd = super::cvtt(((dy * dy) as f32 as f64 + dx * dx) as f32 as f64);
                        if dd < d {
                            d = dd;
                        }
                    }
                }
                let mut i = 0;
                while i < 10 && dist[i] < d {
                    i += 1;
                }
                if i < 10 {
                    for j in (i..=8).rev() {
                        dist[j + 1] = dist[j];
                        best[j + 1] = best[j];
                    }
                    dist[i] = d;
                    best[i] = p;
                }
            }
        }
        for &p in &best[..5] {
            if p != 0 {
                w.event(StoryEvent::SetPlanetView { planet: p, view: 1 });
            }
        }
        add_adat(s, 0x9c, 1);
    }
    if answered(s, ID, 3) || promoted(s, ID) {
        s.win(0, true, w);
    }
    if s.counter(ID) && w.empire_alive(s.v_get(EMPIRE)) {
        message(w, 0x73c, 100);
        empire_param(w, 1, s.v_get(EMPIRE));
        race(w, s.v_get(EMPIRE));
    }
    if answered(s, 0x73c, 2) || promoted(s, 0x73c) {
        let e = s.v_get(EMPIRE);
        relation(w, en, e, -9);
        s.win(0, true, w);
    }
    if answered(s, 0x73c, 3) {
        add_adat(s, 0x9c, -1);
        s.win(0, true, w);
    }
    Ok(())
}

/// 186: a known empire's engineers (message 0x745, its name second): 2,000 and every half year 2,000 more (message
/// 0x746; `StoryAdat+0xa8 += 3` while paid). The count reads 183's `v(1838)` and goes to `v(1638)` (the source's
/// slips): below 6 the counter restarts.
pub(super) fn engineers_186(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x745;
    const FEE: i32 = 0x746;
    if s.starting_now() {
        let e = known_empire(w, 0, 0, 0)?;
        message(w, ID, 0x3c);
        empire_param(w, 2, e);
        s.v_set(0x74d, e);
        s.v_set(0x74c, 0);
    }
    if answered(s, ID, 2) {
        add_adat(s, 0xa8, 3);
        add_money(w, -2000.0);
        s.sz_start(FEE, 0xb4);
    }
    if answered(s, ID, 3) || promoted(s, ID) {
        s.win(0, true, w);
    }
    if s.counter(FEE) {
        let n = s.v_get(0x72e) + 1;
        s.v_set(0x666, n);
        if n < 6 {
            s.sz_start(FEE, 0xb4);
        }
        message(w, FEE, 0x3c);
    }
    if answered(s, FEE, 2) {
        add_money(w, -2000.0);
    }
    if answered(s, FEE, 3) {
        add_adat(s, 0xa8, -3);
        s.sz_stop(ID);
        s.win(0, true, w);
    }
    if promoted(s, FEE) {
        message(w, 0x747, 0);
        add_adat(s, 0xa8, -3);
        s.win(0, true, w);
    }
    Ok(())
}

/// 187: an empire's instructors (message 0x74f, its name): 3,500 every half year (message 0x750, answer 4 pays from
/// then on) for `StoryAdat+0x80 += 10`, `+0x78 += 15`, `+0x70 += 20`.
pub(super) fn instructors_187(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x74f;
    const FEE: i32 = 0x750;
    const PAID: i32 = 0x757;
    const BONUS: [(usize, i32); 3] = [(0x80, 10), (0x78, 15), (0x70, 20)];
    if s.starting_now() {
        let e = random_empire(w, 0, 0, 0)?;
        message(w, ID, 0x3c);
        empire_param(w, 1, e);
    }
    if answered(s, ID, 2) {
        add_money(w, -3500.0);
        for (off, x) in BONUS {
            add_adat(s, off, x);
        }
        s.sz_start(FEE, 0xb4);
    }
    if answered(s, ID, 3) || promoted(s, ID) {
        s.win(0, true, w);
    }
    if s.counter(FEE) {
        if s.v_get(PAID) == 0 {
            message(w, FEE, 0x3c);
        } else {
            add_money(w, -3500.0);
        }
        s.sz_start(FEE, 0xb4);
    }
    if answered(s, FEE, 2) || answered(s, FEE, 4) {
        add_money(w, -3500.0);
        if answered(s, FEE, 4) {
            s.v_set(PAID, 1);
        }
    }
    let lapsed = promoted(s, FEE);
    if answered(s, FEE, 3) || lapsed {
        if lapsed {
            message(w, 0x751, 0);
        }
        for (off, x) in BONUS {
            add_adat(s, off, -x);
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 188: terrorists (message 0x759): answer 3 or 6 pays 20,000 (the ELF's `st_TerrorCselValt(3)` is not in the exe);
/// 5 or 7 (or lapsing) wait: `rnd(1, 10) ≤ 4` (lapsing ≤ 2) ends it, else they come back in `rnd(4, 10)·10` days
/// (message 0x75a: 2 or 6 pays 45,000 and ends it in 30 days (0x75d); 3: message 0x75c in 20 days; 5 or lapsing:
/// 30 or 10 days). Message 0x75c's counter: `rnd(1, 10) > 5` → 0x75f and 25 days to the attack (counter 1886:
/// `rnd(1, 10) > 5` → 0x760, morale −6, else 0x761, −8); else 0x75e, the end.
pub(super) fn terrorists_188(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x759;
    if s.starting_now() {
        message(w, ID, 0x3c);
    }
    if answered(s, ID, 3) || answered(s, ID, 6) {
        add_money(w, -20000.0);
        s.win(0, true, w);
    }
    if answered(s, ID, 5) || answered(s, ID, 7) {
        if super::rnd(w, 1, 10) <= 4 {
            s.win(0, true, w);
        } else {
            let d = super::rnd(w, 4, 10);
            s.sz_start(ID, d * 10);
        }
    }
    if promoted(s, ID) {
        if super::rnd(w, 1, 10) <= 2 {
            s.win(0, true, w);
        } else {
            let d = super::rnd(w, 4, 10);
            s.sz_start(ID, d * 10);
        }
    }
    if s.counter(ID) {
        message(w, 0x75a, 0x3c);
    }
    if answered(s, 0x75a, 2) || answered(s, 0x75a, 6) {
        add_money(w, -45000.0);
        s.sz_start(0x75d, 0x1e);
    }
    if answered(s, 0x75a, 3) {
        s.sz_start(0x75b, 0x14);
    }
    if answered(s, 0x75a, 5) {
        s.sz_start(0x75c, 0x1e);
    }
    if promoted(s, 0x75a) {
        s.sz_start(0x75c, 10);
    }
    if s.counter(0x75b) {
        message(w, 0x75c, 0x3c);
    }
    if answered(s, 0x75c, 2) {
        add_money(w, -45000.0);
        s.sz_start(0x75d, 0x14);
    }
    if answered(s, 0x75c, 3) {
        s.sz_start(0x75c, 0x14);
    }
    if promoted(s, 0x75c) {
        s.sz_start(0x75c, 10);
    }
    if s.counter(0x75c) {
        if super::rnd(w, 1, 10) > 5 {
            message(w, 0x75f, 0);
            s.sz_start(0x75e, 0x19);
        } else {
            message(w, 0x75e, 0);
            s.win(0, true, w);
        }
    }
    if s.counter(0x75d) {
        message(w, 0x75d, 0);
        s.win(0, true, w);
    }
    if s.counter(0x75e) {
        if super::rnd(w, 1, 10) > 5 {
            message(w, 0x760, 0);
            w.event(StoryEvent::GlobalMorale(60));
        } else {
            message(w, 0x761, 0);
            w.event(StoryEvent::GlobalMorale(80));
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 189: a message about two empires (`rnd(1, 8)` each, the human's turned into the next): read, counters 1891 (200
/// days) and 1892 (40 days: one `rnd(1, 10)`); lapsing, message 0x76b and 10 days. It never ends by itself.
pub(super) fn rumour_189(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x763;
    let en = w.human();
    if s.starting_now() {
        message(w, ID, 0x3c);
        for var in [0x76a, 0x76b] {
            let mut e = super::rnd(w, 1, 8);
            if e == en {
                e = e % 8 + 1;
            }
            s.v_set(var, e);
        }
        empire_param(w, 1, s.v_get(0x76a));
        empire_param(w, 2, en);
    }
    if read(s, ID) {
        s.sz_start(ID, 200);
        s.sz_start(0x764, 0x28);
    }
    if promoted(s, ID) {
        message(w, 0x76b, 0);
        s.sz_start(ID, 10);
    }
    if s.counter(0x764) {
        super::rnd(w, 1, 10);
    }
    Ok(())
}

/// `make_point_in_dist(&x, &y, x0, y0, x1, y1, d)` (exe 0x5c9bf0): the point `d` from `(x0, y0)` towards `(x1,
/// y1)` (f32, the length `(f32) sqrt((f64)(dy² + dx²))`).
fn point_in_dist(x0: f32, y0: f32, x1: f32, y1: f32, d: f32) -> (f32, f32) {
    let len = (((y0 - y1) * (y0 - y1) + (x0 - x1) * (x0 - x1)) as f64).sqrt() as f32;
    (((x1 - x0) * d) / len + x0, ((y1 - y0) * d) / len + y0)
}

/// 190: a known empire's (not 5) spy ship comes from its planet nearest to the human's centre, outside the human's
/// radar: owner 10, id 1901 (`st_fNewPos`, radar-reported), heading 1,000 behind the human's planet (clamped to
/// ±300, `rnd(−20, 20)` each), sent away (`+0x18 |= 2`), message 0x76d, a video. Seen: 0x775. Its arrival (special
/// 0x10): 0x774 unless caught, the end. A battle with the human: a pirate escort 1902 joins (`st_AddFleetToSpWar`,
/// bonuses as 101's); afterwards the ship (if alive) goes on and the escort is deleted; caught (gone) → 70-day
/// question 0x76e (video): released (2) 10 days → `rnd(0, 1)` money, tech and relations; kept (3) → 0x772,
/// relations −8.
pub(super) fn spy_ship_190(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x76d;
    const ESCORT: i32 = 0x76e;
    const EMPIRE: i32 = 0x775;
    let en = w.human();
    if s.starting_now() {
        let e = known_empire(w, 5, 0, 0)?;
        s.v_set(EMPIRE, e);
        let home = s.b_search(en.wrapping_neg(), 0, w);
        let (cx, cy) = empire_centre(en, &*w);
        let q = nearest_planet(cx as i32, cy as i32, e, &*w);
        if q == 0 {
            return Err(Abort);
        }
        let (qx, qy) = w.planet_xy(q);
        if !w.in_radar_range(qx as i32, qy as i32, 3) {
            let f = fleet_at(s, 10, ID, qx as i32, qy as i32, 0.08, w);
            fleet_bit(s, ID, 0x40, true, w);
            const EPS: f32 = 0.0001;
            let (hx, hy) = w.planet_xy(home);
            let (mut x, mut y) = point_in_dist(hx, hy, qx, qy, -1000.0);
            if x < -300.0 {
                y = (y - qy) * ((-300.0 - qx) / ((x - qx) + EPS)) + qy;
                x = -300.0;
            }
            if 300.0 < x {
                y = (y - qy) * ((300.0 - qx) / ((x - qx) + EPS)) + qy;
                x = 300.0;
            }
            if y < -300.0 {
                x = (x - qx) * ((-300.0 - qy) / ((y - qy) + EPS)) + qx;
                y = -300.0;
            }
            if 300.0 < y {
                x = (x - qx) * ((300.0 - qy) / ((y - qy) + EPS)) + qx;
                y = 300.0;
            }
            x += super::rnd(w, -20, 20) as f32;
            y += super::rnd(w, -20, 20) as f32;
            s.v_set(ESCORT, x as i32);
            s.v_set(0x76f, y as i32);
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 0, x, y });
            w.event(StoryEvent::FleetState { fleet: f, set: 2, clear: 0 });
            message(w, ID, 0);
            anim(w, ID);
            race(w, e);
            empire_param(w, 1, e);
        } else {
            s.win(0, false, w);
        }
    }
    if s.flags[flag::RADAR_FLEET] && s.radar.1 == s.f_search(ID, w) {
        message(w, EMPIRE, 0);
    }
    if s.special_is(0x10, ID, 0, 0) {
        if s.v_get(ID) == 0 {
            message(w, 0x774, 0);
            race(w, s.v_get(EMPIRE));
        }
        s.win(0, true, w);
    }
    if battle_starts_with(s, ID, w) && battle_owner(s, &*w, en) {
        let f = s.f_search(ID, w);
        let g = s.f_new_ex("", 9, ESCORT, 0.06, 4, f, w);
        w.event(StoryEvent::JoinBattle(g));
        s.battle_pact[9] = 1;
        add_adat_set(s, 0x88);
        add_adat_set(s, 0x94);
        add_adat_set(s, 0x98);
    }
    if s.battle_part(ID) != 0 && s.battle_owner_part(en) != 0 {
        if s.f_search(ID, w) == 0 {
            if s.f_search(ESCORT, w) == 0 {
                s.sz_start(0x76f, 5);
            } else {
                s.sz_start(ID, 5);
                delete_fleet(s, ESCORT, w);
            }
        } else if s.f_search(ESCORT, w) == 0 {
            s.send_away(ID, w);
            s.win(0, true, w);
        } else {
            let f = s.f_search(ID, w);
            let (x, y) = (s.v_get(ESCORT) as f32, s.v_get(0x76f) as f32);
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 0, x, y });
            delete_fleet(s, ESCORT, w);
        }
        s.v_set(ID, 1);
    }
    if s.counter(0x76f) {
        message(w, 0x76f, 0);
        s.win(0, true, w);
    }
    if s.counter(ID) {
        message(w, ESCORT, 0x46);
        if w.campaign_type() != 1 {
            anim(w, ESCORT);
        }
        empire_param(w, 1, s.v_get(EMPIRE));
    }
    if answered(s, ESCORT, 2) {
        s.sz_start(ESCORT, 10);
    }
    if answered(s, ESCORT, 3) {
        s.sz_start(0x770, 0x14);
    }
    if promoted(s, 0x770) {
        message(w, 0x773, 0);
        s.sz_start(0x770, 5);
    }
    if s.counter(ESCORT) {
        let e = s.v_get(EMPIRE);
        if super::rnd(w, 0, 1) == 0 {
            message(w, 0x771, 0);
            race(w, e);
            empire_param(w, 1, e);
            add_money(w, 5000.0);
            let t = random_tech(w, en, 0);
            if t != 0 {
                s.set_tech_state(t, en, w);
            }
        } else {
            message(w, 0x770, 0);
            race(w, e);
            empire_param(w, 1, e);
            add_money(w, 30000.0);
        }
        relation(w, en, e, 15);
        s.win(0, true, w);
    }
    if s.counter(0x770) {
        let e = s.v_get(EMPIRE);
        message(w, 0x772, 0);
        race(w, e);
        empire_param(w, 2, e);
        relation(w, en, e, -8);
        s.win(0, true, w);
    }
    Ok(())
}

fn add_adat_set(s: &mut Story, off: usize) {
    s.set_adat_i32(off, 1);
}

/// `st_SpWarStart(id)`: a space battle starts and one of its two fleets has story id `id`.
fn battle_starts_with(s: &mut Story, id: i32, w: &mut dyn StoryWorld) -> bool {
    if !s.flags[flag::SPACE_START] {
        return false;
    }
    let (a, b) = w.battle_fleets();
    w.fleet_id(a) == id || w.fleet_id(b) == id
}

/// 191: up to 20 picks of a planet of the human's for one of surface 1, 2 or 5; found, it gets info 0x776; the end.
pub(super) fn find_191(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if !s.starting_now() {
        return Ok(());
    }
    let en = w.human();
    let mut tries = 20;
    let p = loop {
        let p = s.b_search(en.wrapping_neg(), 0, w);
        if p == 0 {
            return Err(Abort);
        }
        tries -= 1;
        let t = w.planet_surface(p);
        if tries == 0 || t == 1 || t == 2 || t == 5 {
            break p;
        }
    };
    if tries > 0 {
        w.event(StoryEvent::AddPlanetInfo { planet: p, info: 0x776 });
    }
    s.win(0, true, w);
    Ok(())
}

/// 192: with `v(1921)` (story 530's) the trade market is cheap for 360 days (messages 0x1fc2, 0x1fc3).
pub(super) fn market_192(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        if s.v_get(0x781) == 0 {
            s.win(0, false, w);
        } else {
            message(w, 0x1fc2, 0);
            s.sz_start(0x1fc3, 0x168);
            s.trade_cheap(true, w);
        }
    }
    if s.counter(0x1fc3) {
        s.trade_cheap(false, w);
        message(w, 0x1fc3, 0);
        s.win(0, true, w);
    }
    Ok(())
}
