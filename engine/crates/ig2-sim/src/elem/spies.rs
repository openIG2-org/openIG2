//! `st_Elem_include`, part 2a: the spy episodes 802 (Zachkin's volcano), 824 (a planet to win from an empire),
//! 850 (the three planets' tribute and the mad tanks), 852 (a spy for an empire's protection) and 861 (between
//! empires 1 and 5). Spec: `re/rules/campaign.md` §8.14, part 2a.

use super::{add_money, answered, param, promoted};
use crate::begin::{delete_fleet, move_to_planet, planet_distance, show_fleet};
use crate::rnd::spies::{mission_enabled, spy_caught, spy_chance, spy_success};
use crate::rnd::Pick;
use crate::story::{flag, Abort, InfoLine, MessageText, Story, StoryEvent, StoryWorld, STREAM};

fn rnd(w: &mut dyn StoryWorld, lo: i32, hi: i32) -> i32 {
    w.rng().range(STREAM, lo, hi)
}

fn message(w: &mut dyn StoryWorld, id: i32, arg: i32) {
    w.event(StoryEvent::NewMessage { id, arg });
}

/// Message `id`, or `other` in campaign 2 (its own texts).
fn message2(w: &mut dyn StoryWorld, id: i32, other: i32, arg: i32) {
    let id = if w.campaign_type() == 2 { other } else { id };
    message(w, id, arg);
}

/// `SetMessageFajOverride(Kemek[EN][KemkedesAktKem].+0x12)`: the story mission's spy's race.
fn spy_race(s: &Story, w: &mut dyn StoryWorld) {
    let en = w.human();
    let race = w.spy_record(en, s.spy_slot).race;
    w.event(StoryEvent::MessageRace(race));
}

/// The story mission's spy's name (`Kemek[EN][KemkedesAktKem]`) as parameter `slot`.
fn spy_param(s: &Story, slot: i32, w: &mut dyn StoryWorld) {
    let empire = w.human();
    param(w, slot, MessageText::Spy { empire, slot: s.spy_slot });
}

/// `st_HaborubanAllE(a, b)`: level `Viszony[a][b].+0x25 < 3`.
fn at_war(w: &dyn StoryWorld, a: i32, b: i32) -> bool {
    w.relation_level(a, b) < 3
}

/// `st_SetSemlegesViszony(EN, e)` when at war with `e` (the exe inlines it).
fn make_peace(w: &mut dyn StoryWorld, e: i32) {
    let en = w.human();
    if at_war(&*w, en, e) {
        w.event(StoryEvent::MakeNeutral { a: en, b: e });
    }
}

fn declare_war(w: &mut dyn StoryWorld, e: i32) {
    let en = w.human();
    w.event(StoryEvent::DeclareWar { a: en, b: e });
}

/// `st_KemEselyLin(kind, a, m, lo, hi, true)` (exe 0x5eb5a0): the last story mission's spy (`Kemek[KemkedesTulaj]
/// [KemkedesAktKem]`, skills `+0x31..+0x33`) succeeds: kinds 1 and 2 `100·s₁`, 3 `70·s₃ + 30·s₁`, 4 `40·s₃ + 60·s₂`,
/// else 0; `(uint)((f64)(x·m) / 100.0 + a)` clamped to `hi`, then `lo`; `rnd(0, 99)` below it.
fn spy_chance_lin(s: &Story, kind: i32, a: i32, m: i32, lo: i32, hi: i32, w: &mut dyn StoryWorld) -> i32 {
    let spy = w.spy_record(s.spy_owner, s.spy_slot);
    let (s1, s2, s3) = (spy.skill_31, spy.skill.0, spy.skill.1);
    let x = match kind {
        1 | 2 => s1 * 100,
        3 => s3 * 70 + s1 * 30,
        4 => s3 * 40 + s2 * 60,
        _ => 0,
    };
    let c = (x.wrapping_mul(m) as f64 / 100.0 + a as f64) as i64 as u32 as i32;
    let c = c.min(hi).max(lo);
    (rnd(w, 0, 99) < c) as i32
}

/// `st_GetKuldetesID(id)` (exe 0x5ea710): the spy mission of story mission `id` (`14 + k`), else 0.
fn mission_of(id: i32, w: &dyn StoryWorld) -> i32 {
    (0..12).find(|&k| w.campaign_tables().story_mission(k) == id).map_or(0, |k| k + 14)
}

/// `Multi_Send(0x49, EN, spy, st_GetKuldetesID(id), 0, 0, 0, 10)`: the human's spy goes on story mission `id`.
fn send_spy(spy: i32, id: i32, w: &mut dyn StoryWorld) {
    let mission = mission_of(id, &*w);
    w.event(StoryEvent::SpyOrder { spy, mission });
}

/// A `Pick(rank + 1)` (stream 5) over the human's spies 1..=`OsszHanyKem` at home (state 1); 0 without one.
fn spy_at_home(w: &mut dyn StoryWorld) -> i32 {
    let en = w.human();
    let (mut pick, mut r) = (0, Pick::default());
    for k in 1..=w.spy_count(en) {
        let spy = w.spy_record(en, k);
        if spy.state == 1 && r.weighted(w.spy_rank(en, k) + 1, w) {
            pick = k;
        }
    }
    pick
}

/// `st_SpWarStart(id)`: a space battle starts with a fleet of story id `id` among `spFlottak`'s two.
fn battle_starts_with(s: &Story, id: i32, w: &dyn StoryWorld) -> bool {
    let (a, b) = w.battle_fleets();
    s.flags[flag::SPACE_START] && (w.fleet_id(a) == id || w.fleet_id(b) == id)
}

/// `st_SpWarStartTulaj(e)` (exe 0x602e70): one of the starting battle's two fleets is `e`'s.
fn battle_starts_owner(s: &Story, e: i32, w: &dyn StoryWorld) -> bool {
    let (a, b) = w.battle_fleets();
    s.flags[flag::SPACE_START] && (w.fleet_owner(a) == e || w.fleet_owner(b) == e)
}

impl Story {
    /// `st_GrWarStart(fid, bid)`: a ground battle starts with fleet id `fid` (0: any) on planet `bid` (0: any).
    pub fn ground_start(&mut self, fid: i32, bid: i32, w: &mut dyn StoryWorld) -> bool {
        if !self.flags[flag::GROUND_START] || (fid != 0 && self.ground.fleet_id != fid) {
            return false;
        }
        bid == 0 || self.ground.planet == self.b_search(bid, 0, w)
    }

    /// `st_SetBirodalomVedelem(e, −1)` (exe 0x6025a0): `e` in 1..=8 is protected while the story runs
    /// (`StoryAdat+0xd0 + 4e += 1`, undone by the undo entry `(13, e, StoryLastChecked)`).
    pub fn protect_empire(&mut self, e: i32) {
        if (1..9).contains(&e) {
            let off = 0xd0 + 4 * e as usize;
            self.set_adat_i32(off, self.adat_i32(off).wrapping_add(1));
            self.add_undo(13, e, self.last_checked);
        }
    }

    /// `st_GetNearestBirZachkinhoz()` (exe 0x5ea8e0): the owner of the planet nearest to a random planet of the
    /// human's (truncated coordinates, f32 squares) among the empires 1..=8 other than the human, 1, 3 and 5; once
    /// an empire the human knows was taken, only known ones replace it. The planet is `nearest_bir_planet`; none
    /// aborts.
    pub fn nearest_empire_808(&mut self, w: &mut dyn StoryWorld) -> Result<i32, Abort> {
        let en = w.human();
        let q = self.b_search(en.wrapping_neg(), 0, w);
        if q == 0 {
            return Err(Abort);
        }
        let (qx, qy) = w.planet_xy(q);
        let (cx, cy) = (qx as i32 as f32, qy as i32 as f32);
        let (mut best, mut dist, mut known) = (0, 0i32, false);
        for p in 1..=w.planet_count() {
            let o = w.planet_owner(p);
            if o == 0 || o == en || o >= 9 || o == 1 || o == 5 || o == 3 {
                continue;
            }
            let (px, py) = w.planet_xy(p);
            let d = (cy - py) * (cy - py) + (cx - px) * (cx - px);
            if best != 0 && d >= dist as f32 {
                continue;
            }
            let k = w.knows(en, o);
            if known && !k {
                continue;
            }
            best = p;
            dist = d as i32;
            known |= k;
        }
        self.nearest_planet = best;
        if best == 0 {
            return Err(Abort);
        }
        Ok(w.planet_owner(best))
    }

    /// `st_bGet3Nearest(e)` (exe 0x603470): the human's three planets it sees (1..`HanyBolygo` − 1) nearest to empire
    /// `e`'s (`st_bDistFromBir(q, e)`); a static the calls refill (never cleared): `None` with fewer than 3 planets
    /// of the human's or a slot still 0.
    pub fn three_nearest(&mut self, e: i32, w: &dyn StoryWorld) -> Option<[i32; 3]> {
        let en = w.human();
        if w.planets_of(en) < 3 {
            return None;
        }
        let mut l = [0i32; 3];
        for p in 1..w.planet_count() {
            if w.planet_owner(p) != en || w.planet_visibility(p) < 0 {
                continue;
            }
            let d = empire_distance(p, e, w);
            let mut k = 0;
            if l[1] == 0 || (l[0] != 0 && l[0] < l[1]) {
                k = 1;
            }
            if l[2] == 0 || (l[k] != 0 && l[k] < l[2]) {
                k = 2;
            }
            if l[k] == 0 || d < l[k] {
                l[k] = d;
                self.nearest3[k] = p;
            }
        }
        self.nearest3.iter().all(|&p| p != 0).then_some(self.nearest3)
    }
}

/// `st_bDistFromBir(q, e)` (exe 0x603240): from empire `e`'s planets the human sees (1..`HanyBolygo` − 1) to planet
/// `q`; 1,000,000 without one.
pub(crate) fn empire_distance(q: i32, e: i32, w: &dyn StoryWorld) -> i32 {
    let en = w.human();
    let e = if e == -1 { en } else { e };
    let target = w.planet_xy(q);
    (1..w.planet_count())
        .filter(|&p| w.planet_owner(p) == e && w.planet_visibility(p) >= 0)
        .map(|p| planet_distance(w.planet_xy(p), target))
        .fold(1_000_000, i32::min)
}

// ---------------------------------------------------------------- 802

/// 802: Zachkin's volcano. A built-up planet of the human's (8020) will erupt in 1,500 days; a spy mission (8021)
/// finds the nearest empire's (`st_GetNearestBirZachkinhoz`) planet behind it (8021); conquering it in 30 days, or
/// beating its fleet (8020) sent to a far free planet (8022), saves the planet; tech 183 researched calms it for 15
/// days (the end), else the eruption (`Story_VulkanKitores`). Campaign 2 has its own texts.
pub(super) fn story_802(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    let mut end = false;
    let mut cheaper = false;
    if s.starting_now() {
        s.v_set(8020, 0);
        let mut p = s.b_search(8020, 1, w);
        if p == 0 || w.planet_owner(p) != en {
            p = 0;
            let mut r = Pick::default();
            for q in 1..=w.planet_count() {
                let n = w.buildings_occupied(q);
                if w.planet_owner(q) == en && n > 3 && r.weighted(n, w) {
                    p = q;
                }
            }
            if p == 0 {
                return Err(Abort);
            }
            s.b_add_id(p, 8020);
        }
        s.b_show(8020, -1, w);
        message2(w, 8020, 9020, 0x78);
        param(w, 1, MessageText::Planet(p));
        s.sz_start(8020, 1500);
        s.sz_start(8025, 140);
        s.b_set_extra(p.wrapping_neg(), true, w);
    }
    if answered(s, 8020, 2) || answered(s, 9020, 2) {
        s.spy_mission(8021, true, w);
        s.add_game_info(8021, 0, -1, w);
    }
    if promoted(s, 8020) || promoted(s, 9020) {
        message2(w, 8021, 9021, 0);
        let p = s.b_search(8020, 0, w);
        param(w, 1, MessageText::Planet(p));
        s.spy_mission(8021, true, w);
        s.add_game_info(8021, 0, -1, w);
    }
    if spy_caught(s, 8021) {
        s.spy_answer = 0;
    }
    if spy_success(s, 8021) {
        s.spy_answer = if mission_enabled(8021, &*w) { spy_chance_lin(s, 2, 50, 10, 10, 100, w) } else { 0 };
        if s.spy_answer != 0 {
            s.spy_mission(8021, false, w);
            s.remove_game_info(8021, w);
            s.add_game_info(8022, 0, -1, w);
            let e = s.nearest_empire_808(w)?;
            s.v_set(8029, e);
            let mut q = s.b_search(8021, 0, w);
            if q == 0 || w.planet_owner(q) != e {
                q = s.b_search(e.wrapping_neg(), 0, w);
                s.b_add_id(q, 8021);
            }
            s.v_set(9029, 1);
            message2(w, 8022, 9022, 0);
            spy_race(s, w);
            param(w, 1, MessageText::Planet(q));
            s.b_show(8021, -1, w);
            s.b_set_extra(8021, true, w);
            s.sz_start(8022, 30);
        }
    }
    // Saved by conquest, or by beating the fleet: the eruption's counter, or 8029's when the volcano went quiet.
    let saved = |s: &mut Story| {
        if s.v_get(8023) == 0 {
            s.sz_start(8021, 21);
        } else {
            s.sz_start(8029, 10);
        }
    };
    if s.ground_part(0, 8021, w) && s.sz_get(8022) != 0 {
        let q = s.b_search(8021, 0, w);
        if w.planet_owner(q) == en {
            message2(w, 8024, 9024, 0);
            let p = s.b_search(8020, 0, w);
            param(w, 1, MessageText::Planet(p));
            let q = s.b_search(8021, 0, w);
            w.event(StoryEvent::RemovePlanetInfo { planet: q, info: -8021 });
            s.remove_game_info(8022, w);
            s.sz_stop(8022);
            s.v_set(9029, 1);
            s.v_set(8022, 1);
            saved(s);
        }
    }
    if s.counter(8022) {
        s.b_set_extra(8021, false, w);
        let p = s.b_search(8021, 0, w);
        let mut far = 0;
        if s.b_search(0, 1, w) == 0 {
            s.v_set(8028, 1);
        } else {
            for _ in 0..100 {
                let q = s.b_search_try(0, 0, w)?;
                if far == 0 || planet_distance(w.planet_xy(p), w.planet_xy(far)) < planet_distance(w.planet_xy(p), w.planet_xy(q)) {
                    far = q;
                }
            }
            if planet_distance(w.planet_xy(p), w.planet_xy(far)) < 200 {
                s.v_set(8028, 1);
            } else {
                s.b_add_id(far, 8022);
            }
        }
        s.v_set(8025, 1);
        let owner = s.v_get(8029);
        s.f_new_ex(&format!("story:{}", 8020), owner, 8020, 0.05, p.wrapping_neg(), 1, w);
        s.f_add_ship(8020, 3, 3, w);
        let q = s.b_search(8021, 0, w);
        w.event(StoryEvent::RemovePlanetInfo { planet: q, info: -8021 });
        show_fleet(s, 8020, true, w);
        if s.v_get(8028) == 0 {
            move_to_planet(s, 8020, 8022, w);
        } else {
            s.send_away(8020, w);
        }
    }
    if s.arrived_planet(8020, 8022, w) || s.special_is(0x10, 8020, 0, 0) {
        s.remove_game_info(8022, w);
        if s.arrived_planet(0, 0, w) {
            delete_fleet(s, 8020, w);
        }
        s.v_set(8022, 1);
        if s.v_get(8023) == 0 {
            let p = s.b_search(8020, 0, w);
            if w.planet_owner(p) == en {
                message2(w, 8023, 9023, 0);
                let p = s.b_search(8020, 0, w);
                param(w, 1, MessageText::Planet(p));
            }
        } else {
            end = true;
        }
    }
    if battle_starts_with(s, 8020, &*w) && battle_starts_owner(s, en, &*w) {
        s.set_adat_i32(0x88, 1);
        s.set_adat_i32(0x94, 1);
        s.set_adat_i32(0x98, 1);
    }
    if s.battle_lost(8020) && s.battle_owner_part(en) != 0 {
        let f = s.f_search(8020, w);
        if f != 0 {
            w.event(StoryEvent::DeleteFleet(f));
        }
        s.v_set(8022, 1);
        s.remove_game_info(8022, w);
        saved(s);
        message2(w, 8024, 9024, 0);
        let p = s.b_search(8020, 0, w);
        param(w, 1, MessageText::Planet(p));
    }
    if s.battle_part(8020) != 0 && s.f_search(8020, w) != 0 {
        if s.v_get(8028) == 0 {
            move_to_planet(s, 8020, 8022, w);
        } else {
            s.send_away(8020, w);
        }
    }
    if s.counter(8021) {
        message2(w, 8028, 9028, 0);
        cheaper = true;
        s.add_game_info(8023, 0, -1, w);
        s.v_set(8020, 1);
        s.set_adat_i32(0x6c, s.adat_i32(0x6c).wrapping_add(25));
        s.set_tech_state_to(183, 3, en, w);
        param(w, 1, MessageText::Tech(183));
    }
    if ((s.special_is(0xd, en, 0, 0) && s.special[2] == 183) || s.daily()) && w.tech_status(183, en) == 5 && s.v_get(8026) == 0 {
        s.remove_game_info(8023, w);
        s.sz_start(8023, 15);
        s.sz_stop(8020);
        message2(w, 8224, 9224, 0);
        s.v_set(8026, 1);
    }
    if s.counter(8023) {
        s.b_set_extra(8020, false, w);
        message2(w, 8029, 9029, 0);
        let p = s.b_search(8020, 0, w);
        param(w, 1, MessageText::Planet(p));
        let p = s.b_search(8020, 0, w);
        w.event(StoryEvent::PlanetMorale { planet: p, delta: -200 });
        end = true;
    }
    if s.counter(8020) {
        s.remove_game_info(8022, w);
        let p = s.b_search(8020, 0, w);
        if w.planet_owner(p) == en {
            w.event(StoryEvent::GlobalMorale(150));
        }
        let p = s.b_search(8020, 0, w);
        w.event(StoryEvent::Volcano(p));
        s.b_set_extra(8020, false, w);
        s.v_set(8023, 1);
        if s.v_get(8022) == 0 && (s.f_search(8025, w) == 0 || s.f_search(8020, w) != 0) {
            let p = s.b_search(8020, 0, w);
            if w.planet_owner(p) == en {
                message2(w, 8228, 9228, 0);
            }
            w.event(StoryEvent::Anim(8228));
            s.finished.add(802, 2);
            s.v_set(8024, 1);
            s.sz_start(8026, 360);
        } else {
            let p = s.b_search(8020, 0, w);
            if w.planet_owner(p) == en {
                message2(w, 8227, 9227, 0);
            }
            w.event(StoryEvent::Anim(8227));
            end = true;
        }
    }
    if s.counter(8025) {
        let p = s.b_search(8020, 0, w);
        for k in 1..=w.building_count(p) {
            let (kind, damage, _) = w.building(p, k);
            if kind == 0 || rnd(w, 0, 1) == 0 {
                continue;
            }
            let hp = w.building_hp(kind);
            let mut damage = damage.wrapping_add(rnd(w, 1, hp - 1));
            if hp - 1 < damage {
                damage = hp - 1;
            }
            w.event(StoryEvent::DamageBuilding { planet: p, slot: k, damage });
        }
        w.event(StoryEvent::Earthquake(p));
    }
    if s.counter(8026) && s.v_get(9029) == 0 {
        s.win(0, true, w);
    }
    if s.counter(8029) {
        message2(w, 8226, 9226, 0);
        s.v_set(8020, 1);
        s.set_adat_i32(0x6c, s.adat_i32(0x6c).wrapping_add(25));
        end = true;
    }
    if end {
        if s.v_get(8024) == 0 {
            s.finished.add(802, 2);
        }
        s.win(0, true, w);
    }
    if cheaper {
        let d = w.story_design(en, 8029, 3);
        w.event(StoryEvent::DesignPriceDiv { empire: en, design: d, div: 2 });
    }
    Ok(())
}

// ---------------------------------------------------------------- 824

/// The planet of 824 (8241) and its empire (`v(5208)`).
fn planet_824(s: &mut Story, w: &mut dyn StoryWorld) -> i32 {
    s.b_search(8241, 0, w)
}

fn empire_params(s: &mut Story, w: &mut dyn StoryWorld) {
    let e = s.v_get(5208);
    param(w, 1, MessageText::Empire(e));
    let p = planet_824(s, w);
    param(w, 2, MessageText::Planet(p));
}

/// `st_SetBolygoTulaj(b(8241), v(5208), −1)`, after the human's peace with it (answer 4 or 5 of 8251).
fn hand_back(s: &mut Story, w: &mut dyn StoryWorld) {
    let e = s.v_get(5208);
    let p = planet_824(s, w);
    s.set_planet_owner(p, e, w);
}

/// 824: an empire (`v(5208)`) offers a weak planet of its own (8241, a building of kind 0x2b and 11 units) for
/// peace; refused (answer 3), three spy missions (8241..=8243) prepare its conquest within 720 days; conquered, the
/// planet goes back to the empire for a lasting peace, or, refused, the human keeps it and is at war (with another
/// empire too, without story 808, `v(8082)`); while the peace holds, gifts of ships arrive every year.
pub(super) fn story_824(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        s.v_set(8245, 0);
        let e = s.v_get(5208);
        if e == 0 {
            return Err(Abort);
        }
        if !w.empire_alive(e) || s.b_search(e.wrapping_neg(), 1, w) == 0 {
            return Err(Abort);
        }
        s.add_undo(15, 0, 824);
        make_peace(w, e);
        s.protect_empire(e);
        let skip = s.b_search(8080, 0, w);
        let (mut pick, mut n, mut total) = (0, 0i32, 0i32);
        for q in 1..=w.planet_count() {
            if w.planet_owner(q) == e {
                total = total.wrapping_add(w.buildings_occupied(q));
                n += 1;
                if q != skip {
                    pick = q;
                }
            }
        }
        let avg = total / n.max(1);
        let mut best = 1_000_000;
        for q in 1..=w.planet_count() {
            if w.planet_owner(q) == e && q != skip {
                let d = empire_distance(q, -1, &*w);
                if w.buildings_occupied(q) < avg && d < best {
                    pick = q;
                    best = d;
                }
            }
        }
        if pick == 0 {
            return Err(Abort);
        }
        s.b_add_id(pick, 8241);
        s.b_show(8241, -1, w);
        w.event(StoryEvent::PlaceBuilding { planet: pick, kind: 0x2b });
        w.event(StoryEvent::NewTanks { fleet: pick.wrapping_neg(), design: 1, count: 11 });
        message(w, if s.v_get(8082) == 0 { 8242 } else { 8241 }, 100);
        let e = s.v_get(5208);
        w.event(StoryEvent::MessageRace(e));
        param(w, 1, MessageText::Empire(e));
        param(w, 2, MessageText::Planet(pick));
    }
    if promoted(s, 8241) || answered(s, 8241, 4) {
        if promoted(s, 8241) {
            message(w, 8243, 0);
            let e = s.v_get(5208);
            param(w, 1, MessageText::Empire(e));
            w.event(StoryEvent::MessageRace(e));
        }
        s.win(0, true, w);
    }
    if promoted(s, 8242) || answered(s, 8242, 4) {
        if promoted(s, 8242) {
            message(w, 8244, 0);
            let e = s.v_get(5208);
            w.event(StoryEvent::MessageRace(e));
            param(w, 1, MessageText::Empire(e));
        }
        let e = s.v_get(5208);
        declare_war(w, e);
        s.win(0, true, w);
    }
    if answered(s, 8241, 3) || answered(s, 8242, 3) {
        s.spy_mission(8241, true, w);
        s.sz_start(8241, 720);
        let p = planet_824(s, w);
        s.add_info_line(8241, InfoLine::Planet { planet: p, count: None }, -1, w);
    }
    if s.counter(8241) && s.v_get(8244) == 0 {
        message(w, 8253, 0);
        let e = s.v_get(5208);
        w.event(StoryEvent::MessageRace(e));
        empire_params(s, w);
        let e = s.v_get(5208);
        declare_war(w, e);
        s.win(0, true, w);
    }
    if spy_caught(s, 8241) {
        s.spy_answer = (spy_chance(s, 1, 30, 50, 100, w) == 0) as i32;
    }
    if spy_success(s, 8241) {
        s.spy_answer = if mission_enabled(8241, &*w) { spy_chance_lin(s, 2, 50, 10, 10, 100, w) } else { 0 };
        if s.spy_answer != 0 {
            s.spy_mission(8241, false, w);
            message(w, 8245, 0);
            spy_race(s, w);
            empire_params(s, w);
            spy_param(s, 3, w);
            w.event(StoryEvent::MessageParamInt { slot: 4, value: 8 });
            s.spy_mission(8242, true, w);
            s.spy_mission(8243, true, w);
            s.v_set(8241, 1);
            s.remove_game_info(8241, w);
            let p = planet_824(s, w);
            s.add_info_line(8242, InfoLine::Planet { planet: p, count: None }, -1, w);
        }
    }
    if spy_caught(s, 8242) {
        s.spy_answer = (spy_chance(s, 1, 30, 50, 100, w) == 0) as i32;
    }
    for (id, text) in [(8242, 8246), (8243, 8247)] {
        if !spy_success(s, id) {
            continue;
        }
        s.spy_answer = if mission_enabled(id, &*w) { spy_chance_lin(s, 2, 50, 10, 10, 100, w) } else { 0 };
        if s.spy_answer == 0 {
            continue;
        }
        s.spy_mission(id, false, w);
        if id == 8242 {
            w.event(StoryEvent::Anim(8245));
        }
        message(w, text, 0);
        spy_race(s, w);
        empire_params(s, w);
        spy_param(s, 3, w);
        s.v_set(id, 1);
        // Both missions done: the last info line.
        let other = if id == 8242 { 8243 } else { 8242 };
        if s.v_get(other) != 0 {
            s.remove_game_info(8242, w);
            let p = planet_824(s, w);
            s.add_info_line(8243, InfoLine::Planet { planet: p, count: None }, -1, w);
        }
    }
    if s.daily() && s.v_get(8244) == 0 && s.v_get(8242) == 0 {
        let p = planet_824(s, w);
        let (px, py) = w.planet_xy(p);
        let (cx, cy) = (px as i32 as f32, py as i32 as f32);
        let mut near = 1000;
        for f in 1..=w.fleet_count() {
            if w.fleet_owner(f) == en {
                let (fx, fy) = w.fleet_xy(f);
                let d = ((cy - fy) * (cy - fy) + (cx - fx) * (cx - fx)) as i32;
                near = near.min(d);
            }
        }
        if near < 100 {
            s.set_planet_owner(p, 0, w);
            message(w, if s.v_get(8241) == 0 { 8248 } else { 8249 }, 0);
            empire_params(s, w);
            s.win(0, true, w);
        }
    }
    if s.ground_start(0, 8241, w) && s.ground.fleet_owner == en && s.v_get(8244) == 0 {
        let p = planet_824(s, w);
        let value = if s.v_get(8243) != 0 { 3000 } else { 200 };
        w.event(StoryEvent::SelfDestruct { planet: p, value });
        s.set_adat_i32(0x68, p);
        s.set_adat_i32(0x88, 1);
    }
    if s.ground_part(0, 8241, w) && s.ground.fleet_owner == en && s.v_get(8244) == 0 {
        s.v_set(8244, 1);
        s.set_adat_i32(0x68, 0);
        let p = planet_824(s, w);
        w.event(StoryEvent::RemovePlanetInfo { planet: p, info: -8241 });
        s.remove_game_info(8241, w);
        s.remove_game_info(8242, w);
        s.remove_game_info(8243, w);
        let p = planet_824(s, w);
        w.event(StoryEvent::SelfDestruct { planet: p, value: -1 });
        let (o, e) = (w.planet_owner(p), s.v_get(5208));
        if o == 0 || o == e {
            if o == e {
                s.set_planet_owner(p, 0, w);
            }
            message(w, 8250, 0);
            param(w, 1, MessageText::Empire(e));
            w.event(StoryEvent::MessageRace(e));
        } else {
            if s.v_get(8082) == 0 {
                let x = crate::brnd::random_empire(w, e, 0, 0)?;
                s.v_set(8246, x);
                message(w, 8252, 0);
                w.event(StoryEvent::MessageRace(e));
                param(w, 3, MessageText::Empire(x));
            } else {
                message(w, 8251, 0);
                w.event(StoryEvent::MessageRace(e));
            }
            empire_params(s, w);
        }
    }
    if answered(s, 8251, 2) {
        s.win(0, true, w);
    }
    for part in [4, 5] {
        if !answered(s, 8251, part) {
            continue;
        }
        let e = s.v_get(5208);
        w.event(StoryEvent::RelationChange { a: en, b: e, delta: 2500 });
        make_peace(w, e);
        hand_back(s, w);
        if part == 4 {
            s.set_adat_i32(0x24, e);
            s.add_info_line(8244, InfoLine::Empire(e), -1, w);
            s.sz_start(8243, 1800);
        } else {
            s.sz_start(8245, 1100);
        }
        s.sz_start(8244, 30);
    }
    if answered(s, 8252, 2) {
        let e = s.v_get(5208);
        declare_war(w, e);
        let x = s.v_get(8246);
        declare_war(w, x);
        s.win(0, true, w);
    }
    if answered(s, 8252, 4) {
        hand_back(s, w);
        let e = s.v_get(5208);
        s.set_adat_i32(0x24, e);
        // The German build (`Lang == 4`) shows the race's name with a suffix instead (UI; the same reads).
        s.add_info_line(8244, InfoLine::Empire(e), -1, w);
        s.sz_start(8243, 1080);
    }
    if answered(s, 8252, 5) {
        hand_back(s, w);
        s.win(0, true, w);
    }
    let x = s.adat_i32(0x24);
    if s.daily() && x != 0 && (!w.empire_alive(x) || at_war(&*w, en, x)) {
        s.win(0, true, w);
    }
    if s.counter(8243) {
        s.set_adat_i32(0x24, 0);
        s.remove_game_info(8244, w);
        let e = s.v_get(5208);
        if w.empire_alive(e) {
            message(w, 8254, 0);
            w.event(StoryEvent::MessageRace(e));
            param(w, 1, MessageText::Empire(e));
        }
        s.win(0, true, w);
    }
    if s.counter(8244) {
        let n = s.v_get(8245);
        s.v_set(8245, n.wrapping_add(1));
        let e = s.v_get(5208);
        if n < 4 && w.empire_alive(e) && !at_war(&*w, en, e) {
            s.sz_start(8244, 360);
            let mut p = s.b_search(en.wrapping_neg(), 0, w);
            if p == 0 {
                return Err(Abort);
            }
            for q in 1..=w.planet_count() {
                if w.planet_owner(q) == en && w.planet_value(p) < w.planet_value(q) {
                    p = q;
                }
            }
            let f = s.f_new_ex(&format!("story:{}", 8241), en, 0, 0.0, 2, p, w);
            for (key, n) in [(8241, 2), (8242, 2), (8243, 1)] {
                let d = w.story_design(en, key, 3);
                s.f_add_ship(f.wrapping_neg(), d, n, w);
            }
            w.event(StoryEvent::ScrollEmpirePlanet { text: 8242, empire: e, planet: p });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 850

/// 850: an empire not at war with the human (`v(8509)`) offers protection for three planets near it (8500..=8502)
/// against tribute (`StoryAdat+0x2c`, a tenth kept, 1.2 tenths paid back every 30 days; seven years); a spy may find
/// out the empire's plot (8501): mad tanks in the human's ground battles; caught in time, 2,000 is lost.
pub(super) fn story_850(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let e = super::super::rnd::relations::friendly_empire(5, 0, w);
        s.v_set(8509, e);
        if e == 0 {
            return Err(Abort);
        }
        s.v_set(8507, 0);
        if !w.knows(en, e) {
            w.event(StoryEvent::DiscoverEmpire(e));
        }
        s.protect_empire(e);
        let Some(three) = s.three_nearest(e, &*w) else { return Err(Abort) };
        if s.v_get(8001) == 2 {
            message(w, 5310, 100);
        } else {
            message(w, 5311, 100);
            let e = s.v_get(8509);
            w.event(StoryEvent::MessageRace(e));
        }
        let e = s.v_get(8509);
        param(w, 4, MessageText::Empire(e));
        for (k, &p) in three.iter().enumerate() {
            s.b_add_id(p, 8500 + k as i32);
            param(w, k as i32 + 1, MessageText::Planet(p));
        }
    }
    if answered(s, 5310, 2) || answered(s, 5311, 2) {
        s.v_set(5310, 1);
        s.v_get(8509);
        for k in 0..3 {
            s.b_add_info(8500 + k, 200, -1, w);
        }
        s.set_adat_i32(0x2c, 0);
        s.sz_start(8110, 30);
    }
    if answered(s, 5310, 3) || promoted(s, 5310) {
        s.v_set(5310, 0);
        s.win(0, true, w);
    }
    if answered(s, 5311, 3) || promoted(s, 5311) {
        s.v_set(5310, 0);
        s.sz_start(5311, 300);
    }
    if s.counter(8110) {
        let tribute = s.adat_i32(0x2c);
        let paid = (tribute as f64 * 1.2 / 10.0) as i32;
        let kept = tribute / 10;
        add_money(w, paid, -1);
        s.set_adat_i32(0x2c, 0);
        let n = s.v_get(8110).wrapping_add(1);
        s.v_set(8110, n);
        w.event(StoryEvent::ScrollNumbers { text: 8501, a: kept, b: paid });
        if n == 84 {
            s.win(0, true, w);
        } else if n == 12 {
            s.v_set(8508, 1);
            message(w, 8112, 100);
            let e = s.v_get(5809);
            w.event(StoryEvent::MessageRace(e));
            w.event(StoryEvent::MessageParamInt { slot: 3, value: kept });
            w.event(StoryEvent::MessageParamInt { slot: 4, value: paid });
        } else {
            s.sz_start(8110, 30);
            if s.v_get(8508) == 0 && rnd(w, 1, 6) == 1 {
                s.v_set(8508, 1);
                s.sz_start(5313, 13);
            }
        }
    }
    if answered(s, 8112, 2) {
        s.v_set(8110, 0);
        s.sz_start(8110, 30);
    }
    if answered(s, 8112, 3) || promoted(s, 8112) {
        s.win(0, true, w);
    }
    if s.counter(5313) {
        let spy = spy_at_home(w);
        message(w, 5320, 100);
        let race = if spy == 0 { 1 } else { w.spy_record(en, spy).race };
        w.event(StoryEvent::MessageRace(race));
        s.v_set(8502, spy);
        let e = s.v_get(8509);
        param(w, 1, MessageText::Empire(e));
        param(w, 2, if spy == 0 { MessageText::Text(8502) } else { MessageText::Spy { empire: en, slot: spy } });
        s.spy_mission(8501, true, w);
    }
    if answered(s, 5320, 2) {
        let spy = s.v_get(8502);
        if spy != 0 {
            send_spy(spy, 8501, w);
        }
    }
    if spy_caught(s, 8501) {
        s.spy_answer = 0;
    }
    if spy_success(s, 8501) {
        s.spy_answer = if mission_enabled(8501, &*w) { spy_chance(s, 4, 30, 30, 100, w) } else { 0 };
        if s.spy_answer != 0 {
            if s.v_get(8056) == 0 {
                message(w, 5321, 0);
                spy_race(s, w);
                let e = s.v_get(8509);
                param(w, 1, MessageText::Empire(e));
                spy_param(s, 2, w);
                s.sz_stop(5311);
                s.sz_stop(8110);
                s.sz_start(5314, 140);
            } else {
                message(w, 5322, 0x78);
                spy_race(s, w);
                let e = s.v_get(8509);
                param(w, 1, MessageText::Empire(e));
                spy_param(s, 2, w);
            }
            s.spy_mission(8501, false, w);
        }
    }
    if s.counter(5311) {
        s.v_set(8055, 1);
    }
    if s.ground_start(0, 0, w) && s.v_get(8055) == 1 && w.planet_owner(s.ground.planet) == en {
        let p = s.ground.planet;
        let mut mad = false;
        for (k, (alive, owner)) in w.battlefield_units(p).into_iter().enumerate() {
            if alive && owner == en && rnd(w, 0, 1) != 0 {
                w.event(StoryEvent::MadUnit { planet: p, unit: k as i32 });
                s.v_set(8056, 1);
                mad = true;
            }
        }
        if mad && s.v_get(8507) == 0 {
            s.sz_start(8507, 2);
            s.v_set(8507, 1);
        }
    }
    if s.counter(8507) {
        message(w, 5328, 0);
        w.event(StoryEvent::Anim(5328));
        s.sz_start(5314, 400);
    }
    if s.counter(5314) {
        message(w, 5329, 0);
        s.v_set(8055, 0);
        add_money(w, -2000, -1);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 852

/// 852: an empire (`v(8521)`) is protected (`StoryAdat+0x24`) for 800 days; one of the human's spies at home is
/// offered for a mission against it (8521); its success brings an offer (6703) that ends in war 25 days later.
pub(super) fn story_852(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    let named = |s: &mut Story, w: &mut dyn StoryWorld| {
        let e = s.v_get(8521);
        w.event(StoryEvent::MessageRace(e));
        param(w, 1, MessageText::Empire(e));
    };
    if s.starting_now() {
        s.sz_start(8100, 800);
        let e = s.v_get(8521);
        s.set_adat_i32(0x24, e);
        let e = s.v_get(8521);
        s.protect_empire(e);
        s.sz_start(8520, 400);
        s.v_set(8323, 0);
    }
    if s.daily() {
        let e = s.v_get(8521);
        if at_war(&*w, en, e) {
            message(w, 6011, 0);
            named(s, w);
            s.set_adat_i32(0x24, 0);
            s.win(0, true, w);
        }
    }
    if s.counter(8100) {
        message(w, 6429, 0);
        named(s, w);
        s.set_adat_i32(0x24, 0);
        s.win(0, true, w);
    }
    if s.counter(8520) {
        let spy = spy_at_home(w);
        if spy == 0 {
            s.v_inc(8520);
            if s.v_get(8520) < 10 {
                s.sz_start(8520, 20);
            }
        } else {
            s.v_set(8522, spy);
            message(w, 6701, 100);
            let race = w.spy_record(en, spy).race;
            w.event(StoryEvent::MessageRace(race));
            s.v_set(8523, race);
            let e = s.v_get(8521);
            param(w, 1, MessageText::Empire(e));
            param(w, 2, MessageText::Spy { empire: en, slot: spy });
        }
    }
    if answered(s, 6701, 2) {
        let spy = s.v_get(8522);
        if spy != 0 {
            s.spy_mission(8521, true, w);
            send_spy(spy, 8521, w);
        }
    }
    if spy_caught(s, 8521) {
        s.spy_answer = 0;
    }
    if spy_success(s, 8521) {
        s.spy_answer = mission_enabled(8521, &*w) as i32;
        if s.spy_answer != 0 {
            s.spy_mission(8521, false, w);
            message(w, 6703, 0x78);
            let e = s.v_get(8521);
            param(w, 1, MessageText::Empire(e));
            spy_race(s, w);
        }
    }
    if answered(s, 6703, 2) {
        s.sz_start(6705, 25);
        s.set_adat_i32(0x24, 0);
    }
    if s.counter(6705) {
        message(w, 6709, 0);
        let e = s.v_get(8521);
        declare_war(w, e);
        let e = s.v_get(8521);
        param(w, 1, MessageText::Empire(e));
        w.event(StoryEvent::MessageRace(e));
        s.win(0, true, w);
    }
    if s.counter(6706) {
        s.set_adat_i32(0x24, 0);
        message(w, 6708, 0);
        let e = s.v_get(8521);
        w.event(StoryEvent::MessageRace(e));
        declare_war(w, e);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 861

/// 861: empires 1 and 5 alive; a spy mission against empire 1 (8611) brings evidence the human may sell (200,000)
/// or keep; 100 days later the choice between them (8613): allied with one, at war with the other.
pub(super) fn story_861(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        if !w.empire_alive(1) || !w.empire_alive(5) {
            return Err(Abort);
        }
        s.protect_empire(1);
        message(w, 8610, 0);
        s.spy_mission(8611, true, w);
        s.add_game_info(8611, 0, -1, w);
    }
    if spy_caught(s, 8611) {
        s.spy_answer = (spy_chance(s, 1, 30, 50, 100, w) == 0) as i32;
    }
    if spy_success(s, 8611) {
        s.spy_answer = if mission_enabled(8611, &*w) { spy_chance(s, 2, 50, 20, 100, w) } else { 0 };
        if s.spy_answer != 0 {
            s.spy_mission(8611, false, w);
            s.remove_game_info(8611, w);
            if s.v_get(8610) == 5 {
                s.sz_start(8612, 10);
            } else {
                message(w, 8611, 200);
            }
        }
    }
    if answered(s, 8611, 2) || answered(s, 8611, 4) {
        add_money(w, 200_000, -1);
        s.sz_start(8611, 100);
    }
    if answered(s, 8611, 3) || answered(s, 8611, 5) {
        s.sz_start(8612, 100);
    }
    if promoted(s, 8611) {
        s.sz_start(8612, 10);
    }
    if s.counter(8612) {
        message(w, 8612, 0);
        s.sz_start(8611, 100);
    }
    if s.counter(8611) {
        if !w.empire_alive(1) || !w.empire_alive(5) {
            return Err(Abort);
        }
        message(w, 8613, 0x78);
        param(w, 1, MessageText::Empire(5));
    }
    let choose = |s: &mut Story, w: &mut dyn StoryWorld, ally: i32, enemy: i32, v: i32| {
        s.v_set(8611, v);
        if w.relation_level(en, ally) < 4 {
            w.event(StoryEvent::MakeNeutral { a: en, b: ally });
        }
        w.event(StoryEvent::DeclareWar { a: en, b: enemy });
        s.win(0, true, w);
    };
    if answered(s, 8613, 2) {
        choose(s, w, 1, 5, 1);
    }
    if answered(s, 8613, 3) || promoted(s, 8613) {
        choose(s, w, 5, 1, 0);
    }
    Ok(())
}
