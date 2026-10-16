//! `st_Rnd_include`, part 2a: stories about the human's buildings (spec `re/rules/campaign.md` §8.13): an
//! earthquake (152), guns and shields changed (153, 156, 161), a poorer planet (155), a gift (165), a festival (166).
//! The building records (`Epuletek[p][k]`, 0x20 bytes: kind `+0`, damage `+4`, `+8`, strength `+0x18`, flags
//! `+0x19`) are written directly.

use super::{add_money, anim, campaign_id, message, param};
use crate::begin::random_building;
use crate::story::{Abort, MessageText, Story, StoryEvent, StoryWorld};

/// A random planet of the human's it sees (`st_bSearch(−EN, 0)`).
fn own_planet(s: &mut Story, w: &mut dyn StoryWorld) -> i32 {
    let en = w.human();
    s.b_search(en.wrapping_neg(), 0, w)
}

/// `st_BolygoMoralValt(p, x)`: the planet's morale changes by `−10·x`.
fn planet_morale(w: &mut dyn StoryWorld, p: i32, x: i32) {
    w.event(StoryEvent::PlanetMorale { planet: p, delta: x.wrapping_mul(-10) });
}

/// 152: an earthquake on a random planet of the human's (id 1521, message 0x5f1): each building (kind ≠ 0) is hit
/// with chance `rnd(0, 100) < 40` for `hp·rnd(20, 75)/100` more damage, at most `hp − 1`; the colony screen shows it
/// (`st_Foldrenges`); the end.
pub(super) fn quake(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x5f1;
    if !s.starting_now() {
        return Ok(());
    }
    message(w, ID, 0);
    let p = own_planet(s, w);
    s.b_add_id(p, ID);
    param(w, 1, MessageText::Planet(p));
    for k in 1..=w.building_count(p) {
        let (kind, damage, _) = w.building(p, k);
        if kind == 0 || super::rnd(w, 0, 100) >= 40 {
            continue;
        }
        let hp = w.building_hp(kind);
        let r = super::rnd(w, 20, 75);
        let mut damage = damage.wrapping_add(hp.wrapping_mul(r) / 100);
        if damage >= hp {
            damage = hp - 1;
        }
        w.event(StoryEvent::DamageBuilding { planet: p, slot: k, damage });
    }
    w.event(StoryEvent::Earthquake(p));
    s.win(0, true, w);
    Ok(())
}

/// The human's planet with a building of `kind` once it has tech `tech` (153, 161): `None` without one.
fn planet_with(s: &mut Story, tech: i32, kind: i32, w: &mut dyn StoryWorld) -> Option<i32> {
    let en = w.human();
    if w.tech_status(tech, en) != 5 {
        return None;
    }
    let p = own_planet(s, w);
    (random_building(p, kind, w) != 0).then_some(p)
}

/// Every building of `kind` on planet `p`: its `+0x18` byte becomes `f(byte)`.
fn rescale(p: i32, kind: i32, f: impl Fn(i32) -> i32, w: &mut dyn StoryWorld) {
    for k in 1..=w.building_count(p) {
        if w.building(p, k).0 == kind {
            let value = f(w.building_bytes(p, k).0) as u8 as i32;
            w.event(StoryEvent::BuildingByte { planet: p, slot: k, offset: 0x18, value });
        }
    }
}

/// 153: from 10 days on, every 60 days, once the human has tech 133 and a random planet of its has a building of
/// kind 0x35: its kind-0x35 buildings' strength (`+0x18`) drops to `rnd(10, 80)` % (message 0x5fb, campaign 3
/// 0x11b3; info 0x5fb; id 1531); the end.
pub(super) fn weaker_guns(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x5fb;
    if s.starting_now() {
        s.sz_start(ID, 10);
    }
    if s.counter(ID) {
        match planet_with(s, 133, 0x35, w) {
            None => s.sz_start(ID, 60),
            Some(p) => {
                let id = campaign_id(&*w, 3, ID, 0x11b3);
                message(w, id, 0);
                let r = super::rnd(w, 10, 80);
                s.v_set(ID, r);
                param(w, 1, MessageText::Planet(p));
                rescale(p, 0x35, |b| b.wrapping_mul(r) / 100, w);
                w.event(StoryEvent::AddPlanetInfo { planet: p, info: ID });
                s.b_add_id(p, ID);
                s.win(0, true, w);
            }
        }
    }
    Ok(())
}

/// 155: with 6..=39 planets, a random planet of the human's (up to 30 picks for one of richness 100 or less) without
/// 164's info 1641 gets id 1551 and half its richness (`+0x24 / 2`): message 0x60f with a mine (kinds 0x13..=0x16)
/// on it, else 0x610 (campaign 3: 0x11c7 / 0x11c8); the end (fewer or more planets: without history).
pub(super) fn poorer(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x60f;
    if !s.starting_now() {
        return Ok(());
    }
    let n = w.planets_of(w.human());
    if !(6..=39).contains(&n) {
        s.win(0, false, w);
        return Ok(());
    }
    let mut tries = 30;
    let p = loop {
        let p = own_planet(s, w);
        tries -= 1;
        if tries == 0 || w.planet_richness(p) <= 100 {
            break p;
        }
    };
    if !w.planet_has_info(p, 0x669) {
        let value = w.planet_richness(p) / 2;
        w.event(StoryEvent::SetPlanetRichness { planet: p, value });
        let mine = (1..=w.building_count(p)).any(|k| (0x13..=0x16).contains(&w.building(p, k).0));
        let id = match (mine, w.campaign_type() == 3) {
            (true, false) => ID,
            (true, true) => 0x11c7,
            (false, false) => 0x610,
            (false, true) => 0x11c8,
        };
        message(w, id, 0);
        s.b_add_id(p, ID);
        param(w, 1, MessageText::Planet(p));
    }
    s.win(0, true, w);
    Ok(())
}

/// 156: a random planet of the human's (id 1561, info 0x619) loses its kind-0x35 buildings' strength (`+0x18 :=
/// 0`; message 0x619, campaign 3 0x11d1); the end.
pub(super) fn disarmed(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x619;
    if !s.starting_now() {
        return Ok(());
    }
    let id = campaign_id(&*w, 3, ID, 0x11d1);
    message(w, id, 0);
    let p = own_planet(s, w);
    s.b_add_id(p, ID);
    rescale(p, 0x35, |_| 0, w);
    param(w, 1, MessageText::Planet(p));
    w.event(StoryEvent::AddPlanetInfo { planet: p, info: ID });
    s.win(0, true, w);
    Ok(())
}

/// 161: as 153 with tech 134 and kind 0x36: the strength grows by a third (`(int)(byte·1.33)` in f64, as a byte;
/// message 0x64b, campaign 3 0x1203; id and info 0x64b); the end.
pub(super) fn stronger_shields(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x64b;
    if s.starting_now() {
        s.sz_start(ID, 10);
    }
    if s.counter(ID) {
        match planet_with(s, 134, 0x36, w) {
            None => s.sz_start(ID, 60),
            Some(p) => {
                s.b_add_id(p, ID);
                let id = campaign_id(&*w, 3, ID, 0x1203);
                message(w, id, 0);
                param(w, 1, MessageText::Planet(p));
                rescale(p, 0x36, |b| (b as f64 * 1.33) as i32, w);
                w.event(StoryEvent::AddPlanetInfo { planet: p, info: ID });
                s.win(0, true, w);
            }
        }
    }
    Ok(())
}

/// 165: a gift of 100,000 (message 0x673, a video): a random planet of the human's (id 1651) is shown, its morale
/// drops by 200 and its mines (kinds 0x13..=0x16) stop (`+0x19` bit 0 cleared); the end 60 days later.
pub(super) fn gift(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x673;
    if s.starting_now() {
        s.sz_start(ID, 60);
        let p = own_planet(s, w);
        planet_morale(w, p, 20);
        s.b_add_id(p, ID);
        for k in 1..=w.building_count(p) {
            if (0x13..=0x16).contains(&w.building(p, k).0) {
                let value = w.building_bytes(p, k).1 & 0xfe;
                w.event(StoryEvent::BuildingByte { planet: p, slot: k, offset: 0x19, value });
            }
        }
        s.b_show(ID, -1, w);
        add_money(w, 100_000.0);
        message(w, ID, 0);
        anim(w, ID);
        param(w, 1, MessageText::Planet(p));
    }
    if s.counter(ID) {
        s.win(0, true, w);
    }
    Ok(())
}

/// 166: a festival on a random planet of the human's with an undamaged (`+8 == 0`) building of kind 13 (up to 20
/// picks; none → the end without history): a video, message 0x67d (the human's name, the planet; id 1661), the
/// global morale −50 and the planet's −100; the end.
pub(super) fn festival(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x67d;
    if !s.starting_now() {
        return Ok(());
    }
    let mut tries = 20;
    let (p, found) = loop {
        let p = own_planet(s, w);
        let found = (1..=w.building_count(p)).any(|k| {
            let (kind, _, b8) = w.building(p, k);
            kind == 13 && b8 == 0
        });
        if found {
            break (p, true);
        }
        tries -= 1;
        if tries == 0 {
            break (p, false);
        }
    };
    if !found {
        s.win(0, false, w);
        return Ok(());
    }
    anim(w, ID);
    message(w, ID, 0);
    s.b_add_id(p, ID);
    param(w, 2, MessageText::Planet(p));
    let en = w.human();
    param(w, 1, MessageText::Empire(en));
    w.event(StoryEvent::GlobalMorale(-50));
    planet_morale(w, p, 10);
    s.win(0, true, w);
    Ok(())
}
