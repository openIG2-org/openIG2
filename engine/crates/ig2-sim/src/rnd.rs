//! The campaign's random stories, part 2: `st_Rnd_include` (exe 0x5f0e20, ELF 0x74a2c0; `st_rnd.cpp`), the
//! script `Story_Check` runs right after `st_Begin_include`. Spec: `re/rules/campaign.md` §8.13.
//!
//! The script holds 62 stories (101..=199, 242, 302). Ported: part 1, the ones story 400 starts in every
//! campaign (118 the pirate bounty, 142 the wormhole warning, 199 the special-0x18 video, 242 the hero ships for
//! hire) and the small planet, tech and money events 144, 146, 149, 150, 163, 164, 169, 170, 171, 172, 173 (this
//! file); part 2a, the relations ([`relations`]: 116, 117, 145, 147, 148), a spy mission ([`spies`]: 127), planets
//! found ([`finds`]: 128, 129, 132, 133, 134, 136, 137, 139) and buildings ([`buildings`]: 152, 153, 155, 156,
//! 161, 165, 166); part 2b, escorts, pirates and battles ([`battles`]: 101, 102, 108, 124, 131, 151); part 2c,
//! spy recruits and a colony to spy out ([`spies`]: 138, 174) and the heroes ([`heroes`]: 176..=192). What the
//! stories do to the rest of the game goes out as [`StoryEvent`]s and [`StoryWorld`] calls.

mod battles;
mod buildings;
mod finds;
mod heroes;
pub(crate) mod relations;
pub(crate) mod spies;

use crate::begin::{delete_fleet, move_to_planet};
use crate::brnd::{fleet_strength, random_tech, ship_fleet};
use crate::story::{adat, flag, last_fn, Abort, MessageText, Script, Story, StoryEvent, StoryWorld, STREAM};

/// `st_Rnd_include`.
#[derive(Debug, Clone, Default)]
pub struct RndStories {
    /// Stories that don't run (a replay of a trace recorded before they were ported: the exe had them patched out).
    skip: Vec<i32>,
}

/// Part 2c's stories.
pub const PART_2C: [i32; 19] = [138, 174, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192];

impl RndStories {
    /// The script without stories `skip`.
    pub fn without(skip: &[i32]) -> Self {
        Self { skip: skip.to_vec() }
    }
}

type Block = fn(&mut Story, &mut dyn StoryWorld) -> Result<(), Abort>;

/// The stories ported, in the script's order.
pub const PORTED: [i32; 61] = [
    101, 102, 108, 116, 117, 118, 124, 127, 128, 129, 131, 132, 133, 134, 136, 137, 138, 139, 142, 144, 145, 146, 147,
    148, 149, 150, 151, 152, 153, 155, 156, 161, 163, 164, 165, 166, 169, 170, 171, 172, 173, 174, 176, 177, 178, 179,
    180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 199, 242,
];

impl Script for RndStories {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        // `Story_Check` runs the campaign's scripts in SP only.
        if w.multiplayer() {
            return;
        }
        let stories: [(i32, Block); 61] = [
            (101, battles::escort_101),
            (102, battles::escort_102),
            (108, battles::chase),
            (116, relations::peace),
            (117, relations::trade),
            (118, bounty),
            (124, battles::prey),
            (127, spies::mission),
            (128, finds::tech_planet),
            (129, finds::lost_tech),
            (131, battles::scientist),
            (132, finds::ruins),
            (133, finds::race),
            (134, finds::marked),
            (136, finds::deserter),
            (137, finds::far_tech),
            (138, spies::recruits_138),
            (139, finds::settlers),
            (142, wormholes),
            (144, low_morale),
            (145, relations::seized),
            (146, unrest),
            (147, relations::swap_147),
            (148, relations::swap_148),
            (149, nearest_planets),
            (150, epidemic),
            (151, battles::monster),
            (152, buildings::quake),
            (153, buildings::weaker_guns),
            (155, buildings::poorer),
            (156, buildings::disarmed),
            (161, buildings::stronger_shields),
            (163, tech_done),
            (164, richer),
            (165, buildings::gift),
            (166, buildings::festival),
            (169, announcement),
            (170, tech_offer),
            (171, tribute_171),
            (172, tribute_172),
            (173, tribute_173),
            (174, spies::colony_174),
            (176, heroes::hero_176),
            (177, heroes::hero_177),
            (178, heroes::hero_178),
            (179, heroes::hero_179),
            (180, heroes::hero_180),
            (181, heroes::hero_181),
            (182, heroes::hero_182),
            (183, heroes::advisers_183),
            (184, heroes::hero_184),
            (185, heroes::maps_185),
            (186, heroes::engineers_186),
            (187, heroes::instructors_187),
            (188, heroes::terrorists_188),
            (189, heroes::rumour_189),
            (190, heroes::spy_ship_190),
            (191, heroes::find_191),
            (192, heroes::market_192),
            (199, video),
            (242, heroes),
        ];
        for (id, story) in stories {
            // The catch handler of each story block: `WIN(0, 1)`, then the next story.
            if !self.skip.contains(&id) && s.running(id) && story(s, w).is_err() {
                s.win(0, true, w);
            }
        }
    }
}

// ---------------------------------------------------------------- helpers

fn rnd(w: &mut dyn StoryWorld, lo: i32, hi: i32) -> i32 {
    w.rng().range(STREAM, lo, hi)
}

/// `RndClass(5)` (exe `RndClass::Next`): the candidate offered with weight `x` replaces the pick when
/// `rnd(1, Σx) ≤ x`.
#[derive(Default)]
pub(crate) struct Pick {
    n: i32,
}

impl Pick {
    pub(crate) fn offer(&mut self, w: &mut dyn StoryWorld) -> bool {
        self.weighted(1, w)
    }

    pub(crate) fn weighted(&mut self, x: i32, w: &mut dyn StoryWorld) -> bool {
        self.n = self.n.wrapping_add(x);
        rnd(w, 1, self.n) <= x
    }
}

/// `st_Message(b, r)`: the human answered part `r` of message `b`.
fn answered(s: &Story, b: i32, r: i32) -> bool {
    s.flags[flag::MESSAGE] && s.message == (b, r)
}

/// `st_Elevul(b)`: message `b` was promoted (flag 7).
fn promoted(s: &Story, b: i32) -> bool {
    s.flags[flag::PROMOTED] && s.message.0 == b
}

/// `st_Olvas(b)`: message `b` was read (flag 9, `MessageSelectedBlokk`).
fn read(s: &Story, b: i32) -> bool {
    s.flags[flag::READ] && s.message.0 == b
}

fn message(w: &mut dyn StoryWorld, id: i32, arg: i32) {
    w.event(StoryEvent::NewMessage { id, arg });
}

fn param(w: &mut dyn StoryWorld, slot: i32, text: MessageText) {
    w.event(StoryEvent::MessageParam { slot, text });
}

fn param_int(w: &mut dyn StoryWorld, slot: i32, value: i32) {
    w.event(StoryEvent::MessageParamInt { slot, value });
}

/// `st_AddMoney(x, −1)`: the human gets `x`.
fn add_money(w: &mut dyn StoryWorld, x: f64) {
    let en = w.human();
    w.event(StoryEvent::AddMoney { empire: en, amount: x });
}

/// `st_ViszonyValt(a, b, x)`: `b`'s view of `a` (`Viszony[b][a]` value 2) changes by `100·x`.
fn relation(w: &mut dyn StoryWorld, a: i32, b: i32, x: i32) {
    w.event(StoryEvent::RelationChange { a, b, delta: x.wrapping_mul(100) });
}

/// `st_ShowAnim(_s(6, id))`: a video.
fn anim(w: &mut dyn StoryWorld, id: i32) {
    w.event(StoryEvent::Anim(id));
}

/// `id`, or `other` in campaign `campaign` (the campaigns' own message texts).
fn campaign_id(w: &dyn StoryWorld, campaign: i32, id: i32, other: i32) -> i32 {
    if w.campaign_type() == campaign {
        other
    } else {
        id
    }
}

/// `cvttsd2si`: out of range and NaN give `i32::MIN`.
pub(crate) fn cvtt(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `st_SpWarPartTulaj(e)` (exe 0x602d40): the 1-based participant of the finished space battle owned by `e`
/// among the first two, or the third with more than two; else 0. The second is read even after a one-sided
/// battle (owner 0 in the port).
fn battle_owner_part(s: &Story, e: i32) -> i32 {
    if !s.flags[flag::SPACE_END] {
        return 0;
    }
    let owner = |i: usize| s.battle.get(i).map_or(0, |p| p.owner);
    if owner(0) == e {
        1
    } else if owner(1) == e {
        2
    } else if s.battle.len() > 2 && owner(2) == e {
        3
    } else {
        0
    }
}

impl Story {
    /// `st_SetTalalmanyAllapot(t, 5, e)` (exe 0x602920) with `e` as passed: the tech is given to `e` (`e < 0`:
    /// the human) and the specials queue gets `(e, t)` with `e` resolved.
    pub fn set_tech_state(&mut self, t: i32, e: i32, w: &mut dyn StoryWorld) {
        w.event(StoryEvent::SetTechState { tech: t, state: 5, empire: e });
        let e = if e < 0 { w.human() } else { e };
        self.specials.add(e, t);
    }

    /// `st_SetTalalmanyAllapot(t, state, e)` (exe 0x602920) for any state: only state 5 queues the special.
    pub fn set_tech_state_to(&mut self, t: i32, state: i32, e: i32, w: &mut dyn StoryWorld) {
        if state == 5 {
            self.set_tech_state(t, e, w);
        } else {
            w.event(StoryEvent::SetTechState { tech: t, state, empire: e });
        }
    }
}

// ---------------------------------------------------------------- 118: the pirate bounty

/// 118: each space battle the human wins against a pirate fleet (owner 9) whose story id is 10000..=11000 (and
/// not story 302's `v(3021)`) counts; on other counts than the fifth one in three pays `rnd(3, 5)·2000`
/// (message 0x49d), the fifth sends message 0x4a7.
fn bounty(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const COUNT: i32 = 0x49d;
    const PAID: i32 = 0x49e;
    if s.starting_now() {
        s.v_set(COUNT, 0);
        s.v_set(PAID, 0);
    }
    let en = w.human();
    if battle_owner_part(s, en) == 0 || battle_owner_part(s, 9) == 0 || s.battle_winner != en {
        return Ok(());
    }
    let k = battle_owner_part(s, 9);
    let fleet_id = s.battle.get(k as usize - 1).map_or(0, |p| p.fleet_id);
    // `st_Story(302, 0)` (no mark); `v(3021)` read only when it runs.
    let story_302 = s.is_running(302) && s.v_get(0xbcd) == fleet_id;
    if !(10000..=11000).contains(&fleet_id) || story_302 {
        return Ok(());
    }
    let n = s.v_get(COUNT).wrapping_add(1);
    let paid = s.v_get(PAID);
    s.v_set(COUNT, n);
    if n == 5 {
        message(w, 0x4a7, 0);
    } else if rnd(w, 1, 3) == 1 {
        s.v_set(PAID, paid.wrapping_add(1));
        message(w, 0x49d, 0);
        let amount = rnd(w, 3, 5).wrapping_mul(2000);
        param_int(w, 1, amount);
        add_money(w, amount as f64);
    }
    Ok(())
}

// ---------------------------------------------------------------- 142: the wormholes

/// 142: once some wormhole end is shown but no wormhole has both ends shown, message 0x58d (campaign 3: 0x1145)
/// once (`v(1421)`); a wormhole with both ends shown ends the story (also at the start, as does having none).
/// Special 0x14 for the human: data 2 == 1 → message 0x591 and the end, else message 0x58f (0x1147).
fn wormholes(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const SENT: i32 = 0x58d;
    let n = w.wormhole_count();
    let ends = |w: &dyn StoryWorld, both: bool| {
        (1..=n).any(|k| {
            let [a, b] = w.wormhole(k);
            if both {
                a.0 && b.0
            } else {
                a.0 || b.0
            }
        })
    };
    if s.starting_now() {
        // A count of 0 ends it too (a negative one wouldn't).
        if ends(&*w, false) || n == 0 {
            s.win(0, true, w);
        }
        s.v_set(SENT, 0);
    } else if s.daily() && s.v_get(SENT) == 0 && ends(&*w, false) {
        if ends(&*w, true) {
            s.win(0, true, w);
        } else {
            s.v_set(SENT, 1);
            let id = campaign_id(&*w, 3, 0x58d, 0x1145);
            message(w, id, 0);
        }
    }
    let en = w.human();
    if s.special_is(0x14, en, 0, 0) {
        if s.special[2] == 1 {
            message(w, 0x591, 0);
            s.win(0, true, w);
        } else {
            let id = campaign_id(&*w, 3, 0x58f, 0x1147);
            message(w, id, 0);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 144, 146, 149, 150: planets

/// 144: every 30 days a random planet with morale (`+0x30`) below 20 % gets id 1441 (message 0x5a1, the
/// planet) and counter 1442 := 120 days, which stops while the planet isn't the human's; it running out with
/// the morale above 30 % → message 0x5a6. Specials 0x15 / 0x16 (once, `v(1441)`) → message 0x5a4 / 0x5a5.
fn low_morale(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const ID: i32 = 0x5a1;
    const LATER: i32 = 0x5a2;
    if s.starting_now() {
        s.sz_start(ID, 30);
        s.v_set(ID, 0);
    }
    if s.counter(ID) {
        let mut pick = Pick::default();
        let mut p = 0;
        for q in 1..=w.planet_count() {
            if w.planet_morale(q) / 10 < 200 && pick.offer(w) {
                p = q;
            }
        }
        if p == 0 {
            s.sz_start(ID, 30);
        } else {
            message(w, ID, 0);
            param(w, 1, MessageText::Planet(p));
            s.sz_start(LATER, 120);
            s.b_add_id(p, ID);
        }
    }
    let en = w.human();
    if s.daily() && s.sz_get(LATER) != 0 {
        let p = s.b_search(ID, 0, w);
        if w.planet_owner(p) != en {
            s.sz_stop(LATER);
        }
    }
    if s.counter(LATER) {
        let p = s.b_search(ID, 0, w);
        if w.planet_morale(p) / 10 > 300 {
            message(w, 0x5a6, 0);
            let p = s.b_search(ID, 0, w);
            param(w, 1, MessageText::Planet(p));
        }
    }
    if s.special_is(0x15, 0, 0, 0) && s.v_get(ID) == 0 {
        message(w, 0x5a4, 0);
        param(w, 1, MessageText::Planet(s.special[1]));
        s.v_set(ID, 1);
    }
    if s.special_is(0x16, 0, 0, 0) && s.v_get(ID) == 0 {
        let p = s.special[1];
        message(w, 0x5a5, 0);
        param(w, 1, MessageText::Planet(p));
        let owner = w.planet_owner(p);
        param(w, 2, MessageText::Empire(owner));
        s.v_set(ID, 1);
    }
    Ok(())
}

/// 146: after 10 days (then yearly, 11 more times) a random planet of the human's it sees (`+0xd8[EN] > 0`) with
/// morale below 350 gets id 1461 and, 2 days later, message 0x5b5 (counter 1462 := 30 days, never tested); a
/// last year without one ends it.
fn unrest(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const TRIES: i32 = 0x5bd;
    const ID: i32 = 0x5b5;
    if s.starting_now() {
        s.v_set(TRIES, 0);
        s.sz_start(TRIES, 10);
    }
    let en = w.human();
    if s.counter(TRIES) {
        let mut pick = Pick::default();
        let mut p = 0;
        for q in 1..=w.planet_count() {
            if w.planet_owner(q) == en && w.planet_visibility(q) > 0 && w.planet_morale(q) < 350 && pick.offer(w) {
                p = q;
            }
        }
        if p != 0 {
            s.b_add_id(p, ID);
            s.sz_start(ID, 2);
        } else {
            let n = s.v_get(TRIES);
            if n < 11 {
                s.v_set(TRIES, n + 1);
                s.sz_start(TRIES, 360);
            } else {
                s.win(0, true, w);
            }
        }
    }
    if s.counter(ID) {
        message(w, ID, 0);
        let p = s.b_search(ID, 0, w);
        param(w, 1, MessageText::Planet(p));
        s.sz_start(0x5b6, 30);
    }
    Ok(())
}

/// 149: with 8 planets or more, four times the human's planet (not of surface 7, without info 1491) nearest to a
/// random one of its planets gets info 1491; their names are the message's parameter; every 20 days one of them
/// (a random pick) loses the info, until one is left (message 0x5d4, the end).
fn nearest_planets(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const INFO: i32 = 0x5d3;
    let en = w.human();
    if s.starting_now() {
        if w.planets_of(en) < 8 {
            s.win(0, true, w);
        } else {
            let from = s.b_search(en.wrapping_neg(), 0, w);
            if from == 0 {
                return Err(Abort);
            }
            let (fx, fy) = w.planet_xy(from);
            let mut names = [0i32; 4];
            for name in &mut names {
                let (mut pick, mut best) = (0, 0);
                for p in 1..=w.planet_count() {
                    if w.planet_owner(p) != en || w.planet_surface(p) == 7 || w.planet_has_info(p, INFO) {
                        continue;
                    }
                    let (px, py) = w.planet_xy(p);
                    let (dx, dy) = (fx - px, fy - py);
                    let d = (dy * dy + dx * dx) as i32;
                    if pick == 0 || d < best {
                        pick = p;
                        best = d;
                    }
                }
                if pick != 0 {
                    w.event(StoryEvent::AddPlanetInfo { planet: pick, info: INFO });
                    *name = pick;
                }
            }
            if names == [0; 4] {
                return Err(Abort);
            }
            message(w, INFO, 0);
            param(w, 1, MessageText::Planets(names));
            s.sz_start(INFO, 150);
        }
    }
    if s.counter(INFO) {
        let mut pick = Pick::default();
        let (mut n, mut p) = (0, 0);
        for q in 1..=w.planet_count() {
            if w.planet_has_info(q, INFO) {
                if pick.offer(w) {
                    p = q;
                }
                n += 1;
            }
        }
        if p != 0 {
            w.event(StoryEvent::RemovePlanetInfo { planet: p, info: INFO });
        }
        if n < 2 {
            message(w, 0x5d4, 0);
            s.win(0, true, w);
        } else {
            s.sz_start(INFO, 20);
        }
    }
    Ok(())
}

/// 150: an epidemic on a random planet of the human's (id 1501, `+0xb8 += 50` as i16, `Virus_CheckAdd`, info
/// 100 not shown), message 0x5dd, the end.
fn epidemic(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if !s.starting_now() {
        return Ok(());
    }
    message(w, 0x5dd, 0);
    let en = w.human();
    let p = s.b_search(en.wrapping_neg(), 0, w);
    s.b_add_id(p, 0x5dd);
    let value = (w.planet_epidemic(p) as i16).wrapping_add(50) as i32;
    w.event(StoryEvent::SetEpidemic { planet: p, value });
    w.event(StoryEvent::VirusCheck(p));
    // `Virus_CheckAdd` (exe 0x542930): info 100 (shown) unless the planet has it, and the epidemic switch.
    s.set_adat_i32(adat::EPIDEMIC, 1);
    param(w, 1, MessageText::Planet(p));
    w.event(StoryEvent::AddPlanetInfoHidden { planet: p, info: 100 });
    s.win(0, true, w);
    Ok(())
}

// ---------------------------------------------------------------- 163, 164, 169: gifts

/// 163: of the twelve groups of 16 techs, each group's first tech the human is researching (status 4), one at
/// random (a draw per group), is done at once (message 0x65f, campaign 3: 0x1217); none → `WIN` without
/// history.
fn tech_done(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if !s.starting_now() {
        return Ok(());
    }
    let en = w.human();
    let (mut n, mut pick) = (0, 0);
    for g in 0..12 {
        let Some(t) = (1..=16).map(|j| 16 * g + j).find(|&t| w.tech_status(t, en) == 4) else { continue };
        let r = rnd(w, 0, n);
        n += 1;
        if r == 0 {
            pick = t;
        }
    }
    if pick == 0 {
        s.win(0, false, w);
        return Ok(());
    }
    s.set_tech_state(pick, en, w);
    let id = campaign_id(&*w, 3, 0x65f, 0x1217);
    message(w, id, 0);
    param(w, 1, MessageText::Tech(pick));
    s.win(0, true, w);
    Ok(())
}

/// 164: a random planet of the human's without 155's info 1551 gets id 1641 and `+0x24` (its richness) `+= 20`
/// (campaign 3: 16), message 0x669 (0x1221); the end.
fn richer(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if !s.starting_now() {
        return Ok(());
    }
    let en = w.human();
    let p = s.b_search(en.wrapping_neg(), 0, w);
    if !w.planet_has_info(p, 0x60f) {
        s.b_add_id(p, 0x669);
        let add = if w.campaign_type() != 3 { 20 } else { 16 };
        let value = w.planet_richness(p).wrapping_add(add);
        w.event(StoryEvent::SetPlanetRichness { planet: p, value });
        let id = campaign_id(&*w, 3, 0x669, 0x1221);
        message(w, id, 0);
        param(w, 1, MessageText::Planet(p));
    }
    s.win(0, true, w);
    Ok(())
}

/// 169: message 0x69b, the end.
fn announcement(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        message(w, 0x69b, 0);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 170: a tech for sale

/// 170: a random tech the human can be given (`st_GetRandomTalalmany(−1, 0)`, none → the end) is offered for
/// half its research cost (`+0x34 / 2`): message 0x6a5 (campaign 3: 0x125d); bought (answer 2) → paid, given,
/// the end; answer 3 → 140 days later the offer again (0x6a6 / 0x125e) unless the human has the tech by then;
/// researched meanwhile → the open offer (`v(1709)`: 1 or 2) is taken back. The reminder's answer 2 pays twice:
/// the price, then what the human has up to the price.
fn tech_offer(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const TECH: i32 = 0x6a5;
    const PRICE: i32 = 0x6a6;
    const OPEN: i32 = 0x6ad;
    // Campaign 3's message ids answer as 170's own: the block rewrites `StoryMessageBlokk` first.
    if s.message.0 == 0x125d {
        s.message.0 = 0x6a5;
    } else if s.message.0 == 0x125e {
        s.message.0 = 0x6a6;
    }
    let en = w.human();
    if s.starting_now() {
        // `−1` means the human.
        let t = random_tech(w, en, 0);
        if t == 0 {
            s.win(0, true, w);
        } else {
            s.v_set(TECH, t);
            let price = w.tech_research(t).0 / 2;
            s.v_set(PRICE, price);
            let id = campaign_id(&*w, 3, 0x6a5, 0x125d);
            message(w, id, 50);
            s.v_set(OPEN, 1);
            param(w, 1, MessageText::Tech(t));
            param_int(w, 2, price);
        }
    }
    if s.daily() {
        let t = s.v_get(TECH);
        if w.tech_status(t, en) == 5 {
            if s.v_get(OPEN) == 1 {
                let id = campaign_id(&*w, 3, 0x6a5, 0x125d);
                w.event(StoryEvent::RemoveMessage(id));
            } else if s.v_get(OPEN) == 2 {
                let id = campaign_id(&*w, 3, 0x6a6, 0x125e);
                w.event(StoryEvent::RemoveMessage(id));
            }
        }
    }
    if answered(s, 0x6a5, 2) || answered(s, 0x6a6, 2) || answered(s, 0x125d, 2) || answered(s, 0x125e, 2) {
        let price = s.v_get(PRICE);
        add_money(w, price.wrapping_neg() as f64);
        let t = s.v_get(TECH);
        s.set_tech_state(t, en, w);
        s.win(0, true, w);
    }
    if answered(s, 0x6a5, 3) || answered(s, 0x125d, 3) {
        s.sz_start(TECH, 140);
        s.v_set(OPEN, 0);
    }
    if promoted(s, 0x6a5)
        || answered(s, 0x6a6, 4)
        || promoted(s, 0x6a6)
        || promoted(s, 0x125d)
        || answered(s, 0x125e, 4)
        || promoted(s, 0x125e)
    {
        s.win(0, true, w);
    }
    if s.counter(TECH) {
        let t = s.v_get(TECH);
        if w.tech_status(t, en) < 5 {
            let id = campaign_id(&*w, 3, 0x6a6, 0x125e);
            message(w, id, 50);
            s.v_set(OPEN, 2);
            param(w, 1, MessageText::Tech(t));
            let price = s.v_get(PRICE);
            param_int(w, 2, price);
        } else {
            s.win(0, true, w);
        }
    }
    if answered(s, 0x6a6, 2) || answered(s, 0x125e, 2) {
        let money = cvtt(w.money(en));
        let price = s.v_get(PRICE);
        let pay = if price < money { price } else { money };
        add_money(w, pay.wrapping_neg() as f64);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 171, 172, 173: tributes

/// `M(m, 50)` with the sum in thousands as parameter 1.
fn ask(w: &mut dyn StoryWorld, m: i32, k: i32) {
    message(w, m, 50);
    param_int(w, 1, k);
}

/// The thank-you message 15 days after paying (campaign 2 has its own), the end.
fn thanks(s: &mut Story, w: &mut dyn StoryWorld, m: i32, id: i32, campaign_2: i32) {
    if s.counter(m) {
        let id = campaign_id(&*w, 2, id, campaign_2);
        message(w, id, 0);
        s.win(0, true, w);
    }
}

/// 171: a tribute of 7,000 (message 0x6af; answer 2 pays, 3 asks again in 30..=90 days, 4 refuses).
fn tribute_171(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let (m, r, k) = (0x6af, 0x6b0, 7);
    if s.starting_now() {
        ask(w, m, k);
    }
    if answered(s, m, 2) || answered(s, r, 2) {
        add_money(w, (k * -1000) as f64);
        s.sz_start(m, 15);
    }
    if answered(s, m, 3) {
        let days = rnd(w, 30, 90);
        s.sz_start(r, days);
    }
    if answered(s, m, 4) || promoted(s, m) {
        s.win(0, true, w);
    }
    if s.counter(r) {
        ask(w, r, k);
    }
    if answered(s, r, 3) || promoted(s, r) {
        s.win(0, true, w);
    }
    thanks(s, w, m, 0x6b1, 0x1269);
    Ok(())
}

/// 172: a tribute of 9,000 (0x6b9); the reminder's answer 5 pays 9 (the source's slip, kept).
fn tribute_172(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let (m, r, k) = (0x6b9, 0x6ba, 9);
    if s.starting_now() {
        ask(w, m, k);
    }
    if answered(s, m, 2) {
        add_money(w, (k * -1000) as f64);
        s.sz_start(m, 15);
    }
    if answered(s, m, 3) {
        let days = rnd(w, 30, 90);
        s.sz_start(r, days);
    }
    if promoted(s, m) || answered(s, m, 4) {
        s.win(0, true, w);
    }
    if s.counter(r) {
        ask(w, r, k);
    }
    if answered(s, r, 3) || answered(s, r, 6) || promoted(s, r) {
        s.win(0, true, w);
    }
    if answered(s, r, 5) {
        add_money(w, -(k as f64));
        s.sz_start(m, 15);
    }
    thanks(s, w, m, 0x6bb, 0x1273);
    Ok(())
}

/// 173: a tribute of 3,000 (0x6c3; answer 4 pays, 5 asks again, 6 refuses; the reminder's 2 pays).
fn tribute_173(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let (m, r, k) = (0x6c3, 0x6c4, 3);
    if s.starting_now() {
        ask(w, m, k);
    }
    if answered(s, m, 4) || answered(s, r, 2) {
        add_money(w, (k * -1000) as f64);
        s.sz_start(m, 15);
    }
    if answered(s, m, 5) {
        let days = rnd(w, 30, 90);
        s.sz_start(r, days);
    }
    if promoted(s, m) || promoted(s, r) || answered(s, r, 3) || answered(s, m, 6) {
        s.win(0, true, w);
    }
    if s.counter(r) {
        ask(w, r, k);
    }
    thanks(s, w, m, 0x6c5, 0x127d);
    Ok(())
}

// ---------------------------------------------------------------- 199, 242

/// 199: special 0x18 for the human: the video `_s(6, 1991)` (not in campaign 1); `v(1991) == 3` ends it.
fn video(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const V: i32 = 0x7c7;
    if s.starting_now() {
        s.v_set(V, 0);
    }
    let en = w.human();
    if s.special_is(0x18, en, 0, 0) {
        let v = s.v_get(V);
        if w.campaign_type() != 1 {
            w.event(StoryEvent::Anim(V));
        }
        if v == 3 {
            s.win(0, true, w);
        }
    }
    Ok(())
}

/// 242: the hero ships for hire (`RndHosok`, spec §8.1). After `rnd(1800, 2880)` days (then every `rnd(2520,
/// 3600)`, four offers in all) one of the nine heroes not offered yet whose techs fit and whose strength is below
/// a third of the human's fleet strength is offered (message 0x975); hired: paid, a fleet with the hero ship, a
/// yearly fee (its counter, the hero's id) while the ship lives; in a space battle each hired hero's fleet raises
/// the battle bonuses; the hero's design deleted (special 0x19) takes its ships.
fn heroes(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const OFFER: i32 = 0x975;
    const WHICH: i32 = 0x976;
    let en = w.human();
    // `RndHosok[k]`: id, tech, three bonuses, fee (thousands), strength.
    let hero: [[i32; 7]; 9] = std::array::from_fn(|k| std::array::from_fn(|i| w.campaign_tables().hero(k as i32, i as i32)));
    let at = |k: i32| hero[k.clamp(0, 8) as usize];
    if s.starting_now() {
        let days = rnd(w, 1800, 2880);
        s.sz_start(OFFER, days);
    }
    if s.counter(OFFER) {
        let strength = fleet_strength(&*w) / 3;
        let mut pick = Pick::default();
        let mut chosen = -1;
        for (k, h) in hero.iter().enumerate() {
            let tech = h[1];
            if s.v_get(h[0]) == 0
                && (tech < 3 || w.tech_status(tech + 3, en) == 5)
                && (tech > 4 || w.tech_status(tech + 7, en) != 5)
                && h[6] < strength
                && pick.offer(w)
            {
                chosen = k as i32;
            }
        }
        if chosen >= 0 {
            let h = at(chosen);
            s.v_inc(OFFER);
            s.v_set(WHICH, chosen);
            s.v_set(h[0], 1);
            message(w, OFFER, 120);
            let d = w.story_design(en, h[0], 3);
            param(w, 1, MessageText::Design { empire: en, design: d });
            param(w, 2, MessageText::Hero { hero: chosen, bonuses: false });
            param_int(w, 3, h[5]);
            param(w, 4, MessageText::Hero { hero: chosen, bonuses: true });
        }
        if s.v_get(OFFER) < 4 {
            let days = rnd(w, 2520, 3600);
            s.sz_start(OFFER, days);
        }
    }
    if answered(s, OFFER, 2) {
        let k = s.v_get(WHICH);
        let h = at(k);
        add_money(w, h[5].wrapping_mul(-1000) as f64);
        // The fleet is named after the design (`st_GetTerv` once for the name, once for the ship).
        w.story_design(en, h[0], 3);
        let f = s.f_new_ex("", en, 0, 0.0, 1, 0, w);
        let design = w.story_design(en, h[0], 3);
        w.event(StoryEvent::NewShip { fleet: f, design });
        let b = s.last_fn[last_fn::B];
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: b as f32, y: 0.0 });
        s.sz_start(h[0], 360);
    }
    if s.counter(0) {
        // A hired hero's yearly fee: its counter is its id.
        let id = s.counter_done;
        let mut hired = -1;
        for (k, h) in hero.iter().enumerate() {
            if h[0] == id && s.v_get(id) == 1 {
                hired = k as i32;
            }
        }
        if hired >= 0 {
            if ship_fleet(&*w, en, id) == 0 {
                s.v_set(id, 2);
            } else {
                add_money(w, at(hired)[5].wrapping_mul(-1000) as f64);
                s.sz_start(id, 360);
            }
        }
    }
    if s.flags[flag::SPACE_START] {
        for h in &hero {
            if s.v_get(h[0]) != 1 {
                continue;
            }
            let f = ship_fleet(&*w, en, h[0]);
            let (a, b) = w.battle_fleets();
            if f != 0 && (a == f || b == f) {
                for (off, field) in [(0x84, 2), (0x90, 3), (0x7c, 4)] {
                    let v = s.adat_i32(off).wrapping_add(h[field]);
                    s.set_adat_i32(off, v);
                }
            }
        }
    }
    if s.special_is(0x19, 0, 0, 0) {
        let (f, d) = (s.special[1], s.special[2]);
        let key = w.design_key(w.fleet_owner(f), d);
        if hero.iter().any(|h| h[0] == key) {
            let d = w.story_design(en, key, 3);
            for k in 1..=w.ship_count() {
                let (sf, sd) = w.ship(k);
                if sd == d && w.fleet_owner(sf) == en {
                    w.event(StoryEvent::DeleteShip(k));
                }
            }
            s.v_set(key, 2);
        }
    }
    Ok(())
}
