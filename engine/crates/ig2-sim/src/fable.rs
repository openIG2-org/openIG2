//! The campaign's random fables: `st_Fable_include` (exe 0x5e6ba0, ELF 0x770200; `st_fable.cpp`), run by
//! `Story_Check` after `st_BRnd_include`, before `st_Scen_include`. Spec: `re/rules/campaign.md` §8.15.
//!
//! Fifteen of the random stories story 400 draws (`RndStoryk`): 301..=305 the story merchants of empire 9
//! (fleet ids 10000..=11000: a pirate chase, a lost trader, an escort, a refit trader), 307 a wormhole map,
//! 308 a spy network, 309..=311 an agent's offer (they start 404, the refit trader), 312..=316 small planet
//! episodes. The world changes go out as [`StoryEvent`]s; the fleet helper `st_fDistFromBir`
//! lives here (`st_fEliminate` and `st_fGetNearest` are `brnd`'s `Story::send_away` and `nearest_fleet`).

use crate::story::{adat, flag, Abort, KemNews, MessageText, Script, Story, StoryEvent, StoryWorld, STREAM};

/// `st_Fable_include`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FableStories;

impl Script for FableStories {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        type Fable = fn(&mut Story, &mut dyn StoryWorld) -> Result<(), Abort>;
        let stories: [(i32, Fable); 15] = [
            (301, pirates_301),
            (302, lost_trader_302),
            (303, escort_303),
            (304, hunted_trader_304),
            (305, refit_trader_305),
            (307, wormholes_307),
            (308, spy_network_308),
            (309, offer_309),
            (310, agent_310),
            (311, ambush_311),
            (312, planet_312),
            (313, planet_313),
            (314, planet_314),
            (315, labs_315),
            (316, radar_316),
        ];
        for (id, story) in stories {
            // The catch handler of each story block: `WIN(0, 1)`, then the next story.
            if s.running(id) && story(s, w).is_err() {
                s.win(0, true, w);
            }
        }
    }
}

/// Fleet `+0x52` bits (`fleet-movement.md` §1).
mod bits {
    pub const BLINKING: u16 = 0x80;
    pub const VISIBLE: u16 = 0x100;
    pub const FRIENDLY: u16 = 0x200;
    pub const CUSTOM_NAME: u16 = 0x800;
}

/// The story merchants' ids (`10000..=11000`).
fn trader_id(id: i32) -> bool {
    (10000..=11000).contains(&id)
}

/// `cvttss2si`: f32 → i32, `INT_MIN` for NaN and out-of-range values.
fn trunc(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 { i32::MIN } else { x as i32 }
}

// ---------------------------------------------------------------- message predicates and helpers

/// `st_Message(b, r)`.
fn answered(s: &Story, b: i32, r: i32) -> bool {
    s.flags[flag::MESSAGE] && s.message.0 == b && s.message.1 == r
}

/// `st_Elevul(b)`.
fn promoted(s: &Story, b: i32) -> bool {
    s.flags[flag::PROMOTED] && s.message.0 == b
}

/// `st_Olvas(b)`: message `b` was read (flag 9).
fn read(s: &Story, b: i32) -> bool {
    s.flags[flag::READ] && s.message.0 == b
}

fn message(w: &mut dyn StoryWorld, id: i32, arg: i32) {
    w.event(StoryEvent::NewMessage { id, arg });
}

fn param(w: &mut dyn StoryWorld, slot: i32, text: MessageText) {
    w.event(StoryEvent::MessageParam { slot, text });
}

/// `NewMessage(id, 0)`; campaign 3's message ids are the others' + 1000.
fn campaign_message(w: &mut dyn StoryWorld, id: i32) {
    let id = if w.campaign_type() == 3 { id + 1000 } else { id };
    message(w, id, 0);
}

fn money(w: &mut dyn StoryWorld, amount: f64) {
    let empire = w.human();
    w.event(StoryEvent::AddMoney { empire, amount });
}

fn fleet_flags(w: &mut dyn StoryWorld, fleet: i32, set: u16, clear: u16) {
    w.event(StoryEvent::FleetFlags { fleet, set, clear });
}

/// `st_fShow` / `st_fHide` / `st_fSetVillogas` / `st_fSetBarat` on story fleet `id` (inlined): the bit, and
/// `StoryError` = not found.
fn fleet_bit(s: &mut Story, w: &mut dyn StoryWorld, id: i32, bit: u16, on: bool) {
    let f = s.f_search(id, w);
    if f >= 1 {
        fleet_flags(w, f, if on { bit } else { 0 }, if on { 0 } else { bit });
    }
    s.error = (f < 1) as i32;
}

impl Story {
    /// `st_SpWarLose(fid)` (exe 0x602c20): the participant with story id `fid` (`st_SpWarPart`) lost.
    pub fn battle_lost(&self, fid: i32) -> bool {
        let i = self.battle_part(fid);
        // A participant past the list (the fast path's second entry) has owner 0.
        i != 0 && self.battle.get(i as usize - 1).map_or(0, |p| p.owner) != self.battle_winner
    }

}

/// `st_fDistFromBir(f, e)` (exe 0x604650): the distance from fleet `f` to the nearest planet of empire `e`
/// (−1: the human) the human sees, over the planets 1..`HanyBolygo` (the last one is skipped); 1,000,000
/// without one.
pub fn f_dist_from_bir(f: i32, e: i32, w: &dyn StoryWorld) -> i32 {
    let en = w.human();
    let e = if e == -1 { en } else { e };
    let (fx, fy) = w.fleet_xy(f);
    let mut best = 1_000_000;
    for p in 1..w.planet_count() {
        if w.planet_owner(p) != e || w.planet_visibility(p) < 0 {
            continue;
        }
        let (px, py) = w.planet_xy(p);
        let dx = trunc(px).wrapping_sub(trunc(fx));
        let dy = trunc(py).wrapping_sub(trunc(fy));
        let sq = dy.wrapping_mul(dy).wrapping_add(dx.wrapping_mul(dx));
        let d = trunc((sq as f32 as f64).sqrt() as f32);
        if d < best {
            best = d;
        }
    }
    best
}

/// The nearest of empire 9's merchants that hasn't traded (`+0x18` bit 0 clear) over the fleets 1..=n
/// (`st_fDistFromBir`, the first of equals): `(fleet, distance)`.
fn nearest_trader(w: &dyn StoryWorld) -> (i32, i32) {
    let (mut best, mut dist) = (0, 0);
    for f in 1..=w.fleet_count() {
        if w.fleet_owner(f) != 9 || !trader_id(w.fleet_id(f)) || w.fleet_state(f) & 1 != 0 {
            continue;
        }
        let d = f_dist_from_bir(f, -1, w);
        if best == 0 || d < dist {
            best = f;
            dist = d;
        }
    }
    (best, dist)
}

/// 302/304's search: up to 100 random fleets of empire 9 until a merchant; the 100th draw isn't tested.
fn draw_trader(s: &mut Story, w: &mut dyn StoryWorld) -> Option<i32> {
    let mut tries = 100;
    loop {
        let f = s.f_search(-9, w);
        tries -= 1;
        if tries == 0 {
            return None;
        }
        if trader_id(w.fleet_id(f)) {
            return Some(f);
        }
    }
}

// ---------------------------------------------------------------- 301..=305: the merchants

/// 301: a battle of the human's with a merchant: a pirate of empire 9 chases the human's nearest fleet.
fn pirates_301(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    let mut chase = false;
    if s.battle_owner_part(en) != 0 && s.battle_owner_part(9) != 0 && s.v_get(3011) == 0 {
        if let Some(k) = s.battle.iter().position(|p| trader_id(p.fleet_id)) {
            let slot = s.battle[k].slot_id;
            let (mut best, mut dist) = (0, 0);
            for _ in 0..100 {
                let f = s.f_search(-9, w);
                if f == slot || !trader_id(w.fleet_id(f)) {
                    continue;
                }
                let d = f_dist_from_bir(f, -1, w);
                if d > 60 && (best == 0 || d < dist) {
                    best = f;
                    dist = d;
                }
            }
            if best == 0 {
                s.win(0, true, w);
            } else {
                s.v_set(3011, 1);
                campaign_message(w, 3011);
                param(w, 1, MessageText::Fleet(best));
                param(w, 2, MessageText::Participant(k as i32));
                // st_fSetBarat(−f, false).
                fleet_flags(w, best, 0, bits::FRIENDLY);
                s.error = 0;
                fleet_flags(w, best, bits::CUSTOM_NAME, 0);
                w.event(StoryEvent::FleetState { fleet: best, set: 0, clear: 1 });
                w.event(StoryEvent::SetFleetId { fleet: best, id: 3011 });
                s.auto_add_ship(best, 3011, w);
                s.sz_start(3011, 10);
                chase = true;
            }
        }
    }
    if s.counter(3011) || s.battle_part(3011) != 0 || chase {
        s.sz_start(3011, 10);
        let f = s.f_search(3011, w);
        if f != 0 {
            let (x, y) = w.fleet_xy(f);
            let g = crate::brnd::nearest_fleet(trunc(x), trunc(y), en, w);
            if g != 0 {
                w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 2, x: g as f32, y: 0.0 });
                return Ok(());
            }
            s.send_away(3011, w);
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 302: a merchant asks for help; it is lost in a battle; the human reads its last message and gets paid.
fn lost_trader_302(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        if s.is_running(304) {
            s.win(0, false, w);
        } else if let Some(f) = draw_trader(s, w) {
            campaign_message(w, 3021);
            param(w, 1, MessageText::Fleet(f));
            fleet_flags(w, f, bits::CUSTOM_NAME, 0);
            let id = w.fleet_id(f);
            s.v_set(3021, id);
        } else {
            s.win(0, true, w);
        }
    }
    if s.flags[flag::SPACE_END] {
        let id = s.v_get(3021);
        if s.battle_lost(id) {
            let id = s.v_get(3021);
            let f = s.f_search(id, w);
            if f != 0 {
                w.event(StoryEvent::DeleteFleet(f));
            }
            if s.battle_owner_part(w.human()) == 0 {
                s.win(0, true, w);
            } else {
                campaign_message(w, 3022);
            }
        }
    }
    if read(s, 4022) || read(s, 3022) {
        money(w, 24000.0);
        s.win(0, true, w);
    }
    Ok(())
}

/// 303: the nearest merchant asks for an escort; a fleet of the human's meets it (or the time runs out).
fn escort_303(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.sz_start(3031, 10);
    }
    if s.counter(3031) {
        let (f, d) = nearest_trader(&*w);
        if f == 0 || d > 79 {
            s.sz_start(3031, 10);
        } else {
            s.v_set(3031, w.fleet_id(f));
            s.v_set(3032, trunc(w.fleet_target(f)));
            w.event(StoryEvent::SetFleetId { fleet: f, id: 3031 });
            fleet_flags(w, f, bits::FRIENDLY | bits::CUSTOM_NAME | bits::VISIBLE, 0);
            w.event(StoryEvent::StopFleet(f));
            message(w, 3031, 0);
            param(w, 1, MessageText::Fleet(f));
            s.sz_start(3032, 240);
        }
    }
    let met = s.arrived_fleet(0, 3031) && w.fleet_owner(s.arrival.fleet) == w.human();
    if met || s.counter(3032) {
        let f = s.f_search(3031, w);
        // The fleet's own id back, read inline without touching StoryError.
        let id = s.vars.get(3031).unwrap_or(0);
        w.event(StoryEvent::SetFleetId { fleet: f, id });
        fleet_flags(w, f, 0, bits::FRIENDLY | bits::CUSTOM_NAME | bits::VISIBLE);
        let target = s.v_get(3032);
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: target as f32, y: 0.0 });
        if s.counter(3032) {
            message(w, 3032, 0);
        } else {
            money(w, 10000.0);
            message(w, 3033, 0);
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 304: a merchant hunted by pirates; the human gets paid when it loses a battle the human was in.
fn hunted_trader_304(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        if s.is_running(305) {
            s.win(0, false, w);
        } else if let Some(f) = draw_trader(s, w) {
            campaign_message(w, 3041);
            param(w, 1, MessageText::Fleet(f));
            fleet_flags(w, f, bits::CUSTOM_NAME, 0);
            w.event(StoryEvent::SetFleetId { fleet: f, id: 3041 });
        } else {
            s.win(0, true, w);
        }
    }
    if s.battle_lost(3041) {
        if s.battle_owner_part(w.human()) != 0 {
            campaign_message(w, 3042);
            money(w, 36000.0);
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 305: the nearest merchant offers a trade: paid for, it trades (`+0x18` bit 0) and the story ends.
fn refit_trader_305(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let (f, _) = nearest_trader(&*w);
        if f == 0 {
            s.win(0, true, w);
        } else {
            message(w, 3051, 100);
            param(w, 1, MessageText::Fleet(f));
            s.v_set(3051, w.fleet_id(f));
        }
    }
    if s.flags[flag::SPACE_END] && s.v_get(3052) == 0 {
        let id = s.v_get(3051);
        if s.battle_lost(id) {
            w.event(StoryEvent::WithdrawMessage { id: 3051, arg: 0 });
            s.win(0, true, w);
        }
    }
    if answered(s, 3051, 3) {
        s.v_set(3052, 1);
        money(w, -14000.0);
        let id = s.v_get(3051);
        let t = s.f_search(id, w);
        s.set_adat_i32(TRADER, t);
        s.add_undo(14, 0, 305);
        fleet_flags(w, t, bits::CUSTOM_NAME, 0);
        s.sz_start(3051, 1440);
    }
    if answered(s, 3051, 4) {
        s.sz_start(3053, 60);
    }
    if answered(s, 3051, 5) || promoted(s, 3051) {
        s.win(0, true, w);
    }
    if s.counter(3053) {
        message(w, 3051, 100);
        let id = s.v_get(3051);
        let f = s.f_search(id, w);
        if f == 0 {
            return Err(Abort);
        }
        param(w, 1, MessageText::Fleet(f));
    }
    if s.counter(3051) {
        let t = s.adat_i32(TRADER);
        if t != 0 && w.fleet_state(t) & 1 == 0 {
            s.sz_start(3051, 10);
        } else {
            if t != 0 {
                fleet_flags(w, t, 0, bits::CUSTOM_NAME);
                s.set_adat_i32(TRADER, 0);
                message(w, 3052, 0);
                param(w, 1, MessageText::Fleet(t));
            }
            s.win(0, true, w);
        }
    }
    let t = s.adat_i32(TRADER);
    if s.daily() && t != 0 && w.fleet_owner(t) != 9 {
        s.set_adat_i32(TRADER, 0);
        s.win(0, true, w);
    }
    Ok(())
}

/// `StoryAdat+0x28`: story 305's trader (undo type 14 clears it).
const TRADER: usize = 0x28;

// ---------------------------------------------------------------- 307, 308

/// 307: a map of a wormhole for sale.
fn wormholes_307(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let mut pick = 0;
        for k in 1..=w.wormhole_count() {
            // RndInitValasztas inside the loop: every offer is the first (rnd(5, 1, 1)), so the last one wins.
            w.rng().choice_begin();
            let [(known_a, a), (known_b, b)] = w.wormhole(k);
            if (!known_a || !known_b) && a > 0 && b > 0 && w.rng().choice_offer(STREAM, 1) {
                pick = k;
            }
        }
        if pick == 0 {
            s.win(0, true, w);
        } else {
            s.v_set(3071, pick);
            message(w, 3071, 100);
        }
    }
    if answered(s, 3071, 4) {
        money(w, -12000.0);
        let k = s.v_get(3071);
        let [(_, a), (_, b)] = w.wormhole(k);
        w.event(StoryEvent::ShowWormhole(k));
        fleet_flags(w, a, bits::VISIBLE, 0);
        fleet_flags(w, b, bits::VISIBLE, 0);
        s.win(0, true, w);
    }
    if answered(s, 3071, 3) || answered(s, 3071, 5) || promoted(s, 3071) {
        s.win(0, true, w);
    }
    Ok(())
}

/// `GetBarmilyenBirodalom(a, b, c)` (exe 0x5c24e0): a random live empire 1..=8 other than `a`, `b`, `c` and the
/// human (`rnd(5, 1, 8)`, then stepping on); the tenth miss aborts the story.
fn any_empire(s: &mut Story, w: &mut dyn StoryWorld, not: [i32; 3]) -> Result<i32, Abort> {
    let _ = s;
    let mut e = w.rng().range(STREAM, 1, 8);
    let en = w.human();
    let mut tries = 10;
    while not.contains(&e) || e == en || !w.empire_alive(e) {
        tries -= 1;
        if tries == 0 {
            return Err(Abort);
        }
        e = e % 8 + 1;
    }
    Ok(e)
}

/// 308: an empire's spy network for sale: four of its planets get the info line −3081 and a radar for 90 days.
fn spy_network_308(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const INFO: i32 = -3081;
    if s.starting_now() {
        let e = any_empire(s, w, [5, 1, 3])?;
        s.v_set(3081, e);
        message(w, 3081, 100);
        let e = s.v_get(3081);
        param(w, 1, MessageText::Empire(e));
    }
    if answered(s, 3081, 2) {
        money(w, -2000.0);
        let e = s.v_get(3081);
        let (mut left, mut tries) = (4, 100);
        loop {
            let p = s.b_search_try(e.wrapping_neg(), 0, w)?;
            if p != 0 && !w.planet_has_info(p, INFO) {
                s.b_add_id(p, 3085 - left);
                w.event(StoryEvent::AddPlanetInfo { planet: p, info: INFO });
                w.event(StoryEvent::SetPlanetRadar { planet: p, level: 4 });
                left -= 1;
            }
            if left == 0 {
                break;
            }
            tries -= 1;
            if tries == 0 {
                break;
            }
        }
        s.sz_start(3082, 90);
    }
    if answered(s, 3081, 3) || promoted(s, 3081) {
        s.win(0, true, w);
    }
    if s.counter(3082) {
        for i in 0..4 {
            let p = s.b_search(3081 + i, 1, w);
            if p != 0 {
                w.event(StoryEvent::RemovePlanetInfo { planet: p, info: INFO });
                w.event(StoryEvent::SetPlanetRadar { planet: p, level: 0 });
            }
        }
        w.event(StoryEvent::ScrollText { text: 3081, important: false });
        s.win(0, true, w);
    }
    if s.daily() {
        let e = s.v_get(3081);
        for i in 0..4 {
            let p = s.b_search(3081 + i, 1, w);
            if p != 0 && w.planet_owner(p) != e {
                w.event(StoryEvent::RemovePlanetInfo { planet: p, info: INFO });
                w.event(StoryEvent::SetPlanetRadar { planet: p, level: 0 });
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 309..=311: the agent

/// The guard of 309..=311: only one of them per game (`v(3099)`).
fn agent_guard(s: &mut Story) -> Result<(), Abort> {
    if s.v_get(3099) != 0 {
        return Err(Abort);
    }
    Ok(())
}

/// 309: an agent offers his services (message 3091, repeated every 90 days); bought, story 404 starts.
fn offer_309(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        agent_guard(s)?;
        s.v_set(3099, 1);
    }
    if s.starting_now() || s.counter(3091) {
        message(w, 3091, 120);
    }
    if answered(s, 3091, 2) {
        money(w, -20000.0);
        // A running 404 becomes StoryLastChecked, so the WIN ends 404.
        s.start_story(404, false);
        s.win(0, true, w);
    }
    if answered(s, 3091, 3) {
        s.sz_start(3091, 90);
    }
    if answered(s, 3091, 4) {
        s.win(0, true, w);
    }
    Ok(())
}

/// 310: the agent joins the human's spies; while he lives story 404 runs.
fn agent_310(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        agent_guard(s)?;
        if w.spy_count(en) > 11 {
            return Err(Abort);
        }
        s.v_set(3099, 1);
    }
    if s.starting_now() {
        message(w, 3092, 120);
        s.v_set(3101, 0);
    }
    if answered(s, 3092, 2) {
        let k = w.new_spy(en);
        if k < 1 {
            return Err(Abort);
        }
        // The record's rank (+0x40) is 1.
        let (lo, hi) = (w.spy_rank_xp(1), w.spy_rank_xp(2));
        let xp = w.rng().range(STREAM, lo, hi);
        w.event(StoryEvent::StorySpy { slot: k, xp });
        // KemJelentesAdd(2, k, 0, …) under Story_check_locked: its Story_KemEsemeny sets the spy flag for the
        // rest of this check, its Story_Check returns at once.
        s.kem = KemNews { news: 2, spy: k, ..KemNews::default() };
        if !w.multiplayer() {
            s.flags[flag::SPY] = true;
            s.spy_kind = 3;
        }
        w.event(StoryEvent::SpyReport { kind: 2, spy: k });
        s.start_story(404, false);
        s.sz_start(3101, 180);
        s.v_set(3101, 1);
    }
    if s.counter(3101) {
        let spare = w.spy_record(en, 0);
        if spare.state > 0 && spare.agent {
            w.event(StoryEvent::KillSpy { empire: en, slot: 0 });
        } else {
            for k in 1..=w.spy_count(en) {
                let spy = w.spy_record(en, k);
                if spy.state > 0 && spy.active != 0 && spy.agent {
                    w.event(StoryEvent::KillSpy { empire: en, slot: k });
                }
            }
        }
    }
    // st_KemMeghalt(0): a spy report of a death (kind 1, outcome 1) about the agent.
    let (kind, k, outcome) = (s.kem.news, s.kem.spy, s.kem.died);
    if s.flags[flag::SPY] && s.spy_kind == 3 && kind == 1 && outcome == 1 && w.spy_record(en, k).agent {
        message(w, 3093, 0);
        agent_gone(s, w);
    }
    if s.daily() && s.v_get(3101) != 0 {
        let alive = (0..=w.spy_count(en)).any(|k| {
            let spy = w.spy_record(en, k);
            spy.state > 0 && spy.agent
        });
        if !alive {
            agent_gone(s, w);
        }
    }
    if answered(s, 3091, 3) {
        s.win(0, true, w);
    }
    Ok(())
}

/// The agent is gone: 404's trader leaves, 310 and 404 end.
fn agent_gone(s: &mut Story, w: &mut dyn StoryWorld) {
    s.send_away(4041, w);
    s.win(0, true, w);
    s.win(404, true, w);
}

/// 311: the refit trader is ambushed by a pirate on its way; saved, it offers its services.
fn ambush_311(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const TRADER: i32 = 4041;
    const PIRATE: i32 = 3111;
    let en = w.human();
    if s.starting_now() {
        agent_guard(s)?;
        s.v_set(3099, 1);
    }
    if s.starting_now() {
        if w.planets_of(en) < 4 {
            s.win(0, false, w);
        } else {
            w.event(StoryEvent::Anim(3094));
            message(w, 3094, 0);
            let f = s.f_new_ex("", 9, TRADER, 0.03, 1, 0, w);
            let b = s.last_fn[crate::story::last_fn::B];
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: b as f32, y: 0.0 });
            fleet_bit(s, w, TRADER, bits::VISIBLE, true);
            fleet_bit(s, w, TRADER, bits::BLINKING, true);
            fleet_bit(s, w, TRADER, bits::FRIENDLY, true);
            // st_fNewPos: the pirate opposite the trader's start, mirrored through the planet.
            let (px, py) = w.planet_xy(b);
            let (lx, ly) = (s.last_fn[crate::story::last_fn::X], s.last_fn[crate::story::last_fn::Y]);
            let x = trunc(px - (px - lx as f32) * 2.0) as f32;
            let y = trunc(ly.wrapping_mul(2) as f32 - py) as f32;
            let p = w.new_fleet("", 10, PIRATE, x, y, 7.0);
            s.auto_add_ship(p, PIRATE, w);
            // st_fAttackFleet(3111, 4041).
            let (a, t) = (s.f_search(PIRATE, w), s.f_search(TRADER, w));
            if a > 0 && t > 0 {
                w.event(StoryEvent::MoveFleet { fleet: a, kind: 2, mode: 2, x: t as f32, y: 0.0 });
            }
            s.v_set(3112, 0);
        }
    }
    if s.flags[flag::SPACE_START] {
        let (a, b) = w.battle_fleets();
        let (ia, ib) = (w.fleet_id(a), w.fleet_id(b));
        if (ia == PIRATE || ib == PIRATE) && (ia == TRADER || ib == TRADER) {
            let f = s.f_search(TRADER, w);
            if f == 0 {
                return Err(Abort);
            }
            let (fx, fy) = w.fleet_xy(f);
            let (x, y) = (trunc(fx) as f32, trunc(fy) as f32);
            // The fleets up to the ship count (a slip kept).
            for k in 1..=w.ship_count() {
                if w.fleet_owner(k) != en {
                    continue;
                }
                let (kx, ky) = w.fleet_xy(k);
                let d = (y - ky) * (y - ky) + (x - kx) * (x - kx);
                if trunc((d as f64).sqrt() as f32) < 10 {
                    w.event(StoryEvent::JoinBattle(k));
                    s.battle_pact[9] = 1;
                    s.set_adat_i32(adat::BATTLE_88, 1);
                    s.set_adat_i32(adat::NO_RETREAT_94, 1);
                }
            }
        }
    }
    if s.battle_part(PIRATE) != 0 {
        if s.f_search(PIRATE, w) == 0 {
            if s.f_search(TRADER, w) == 0 {
                s.win(0, true, w);
            } else {
                fleet_bit(s, w, TRADER, bits::BLINKING, false);
                fleet_bit(s, w, TRADER, bits::VISIBLE, false);
                message(w, 3095, 0);
                s.v_set(3112, 1);
            }
        } else if s.f_search(TRADER, w) == 0 {
            s.send_away(PIRATE, w);
            s.win(0, true, w);
        }
    }
    if s.daily() && s.v_get(3112) != 0 && s.f_search(TRADER, w) == 0 {
        w.event(StoryEvent::RemoveMessage(3095));
        s.win(0, true, w);
    }
    if answered(s, 3095, 2) {
        s.start_story(404, false);
        s.win(0, true, w);
    }
    if answered(s, 3095, 3) {
        s.send_away(TRADER, w);
        money(w, 13500.0);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 312..=316: planet episodes

/// 312: an offer about one of the human's planets (repeated after 70 days); bought, the planet gets info 3120.
fn planet_312(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let p = s.b_search(w.human().wrapping_neg(), 0, w);
        if p != 0 {
            s.b_add_id(p, 3121);
        }
        message(w, 3120, 100);
        param(w, 1, MessageText::Planet(p));
    }
    let either = |s: &Story, r: i32| answered(s, 3120, r) || answered(s, 3121, r);
    if either(s, 2) {
        money(w, -2000.0);
        let p = s.b_search(3121, 0, w);
        w.event(StoryEvent::AddPlanetInfo { planet: p, info: 3120 });
        s.win(0, true, w);
    }
    if either(s, 3) {
        s.sz_start(3120, 70);
    }
    if either(s, 4) || promoted(s, 3120) || promoted(s, 3121) {
        s.win(0, true, w);
    }
    if s.counter(3120) {
        message(w, 3121, 50);
        let p = s.b_search(3121, 0, w);
        param(w, 1, MessageText::Planet(p));
    }
    Ok(())
}

/// A random planet of the human's for 313/314; none aborts the story.
fn human_planet(s: &mut Story, w: &mut dyn StoryWorld) -> Result<i32, Abort> {
    match s.b_search(w.human().wrapping_neg(), 0, w) {
        0 => Err(Abort),
        p => Ok(p),
    }
}

/// 313: news about one of the human's planets (info 3130).
fn planet_313(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let p = human_planet(s, w)?;
        s.b_add_id(p, 3130);
        w.event(StoryEvent::AddPlanetInfo { planet: p, info: 3130 });
        campaign_message(w, 3130);
        param(w, 1, MessageText::Planet(p));
        s.win(0, true, w);
    }
    Ok(())
}

/// 314: a warning about one of the human's planets; 30 days later, if the human still holds it, a message.
fn planet_314(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let p = human_planet(s, w)?;
        s.b_add_id(p, 3140);
        campaign_message(w, 3140);
        param(w, 1, MessageText::Planet(p));
        s.b_show(3140, -1, w);
        s.sz_start(3140, 30);
    }
    if s.counter(3140) {
        let p = s.b_search(3140, 0, w);
        if w.planet_owner(p) == w.human() {
            campaign_message(w, 3141);
            let p = s.b_search(3140, 0, w);
            param(w, 1, MessageText::Planet(p));
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 315: the human's research slows down by 30 for 150 days (`StoryAdat+0x6c`) once the labs work.
fn labs_315(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const RESEARCH: usize = 0x6c;
    let en = w.human();
    if s.starting_now() {
        s.sz_start(3151, 10);
    }
    if s.counter(3151) {
        let labs = (1..4).find(|&k| w.lab_level(en, k) != 0);
        if labs.is_some() && w.researching(en) > 0 {
            message(w, 3150, 0);
            s.set_adat_i32(RESEARCH, s.adat_i32(RESEARCH).wrapping_sub(30));
            s.sz_start(3150, 150);
        } else {
            s.sz_start(3151, 10);
        }
    }
    if s.counter(3150) {
        message(w, 3151, 0);
        s.set_adat_i32(RESEARCH, s.adat_i32(RESEARCH).wrapping_add(30));
        s.win(0, true, w);
    }
    Ok(())
}

/// 316: one of the human's planets with a radar gets a better one (level 2) for a year.
fn radar_316(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let mut tries = 10;
        let found = loop {
            let p = s.b_search(en.wrapping_neg(), 0, w);
            tries -= 1;
            // The 10th draw ends the search whatever it found.
            if tries == 0 || p == 0 {
                break None;
            }
            if w.planet_radar(p) != 0 {
                break Some(p);
            }
        };
        match found {
            None => s.win(0, true, w),
            Some(p) => {
                campaign_message(w, 3160);
                param(w, 1, MessageText::Planet(p));
                s.sz_start(3161, 360);
                s.b_add_id(p, 3161);
                s.b_show(3161, -1, w);
                w.event(StoryEvent::SetPlanetRadar { planet: p, level: 2 });
            }
        }
    }
    if s.counter(3161) {
        let p = s.b_search(3161, 0, w);
        let mine = w.planet_owner(p) == en;
        if mine {
            message(w, 3161, 0);
            param(w, 1, MessageText::Planet(p));
        }
        w.event(StoryEvent::SetPlanetRadar { planet: p, level: mine as i32 });
        s.win(0, true, w);
    }
    Ok(())
}
