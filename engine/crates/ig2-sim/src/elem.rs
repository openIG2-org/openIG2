//! The storyline episodes: `st_Elem_include` (exe 0x5d4b70, ELF 0x77ac00; `st_elem.cpp`), the script
//! `Story_Check` runs after `st_Line_include`. Spec: `re/rules/campaign.md` §8.14.
//!
//! Stories 801..=868 are the episodes the storylines (§8.9) start. Part 1 is ported: 805, 811, 813, 816, 817,
//! 818, 820, 830, 831, 835, 837, 841, 842 and 864..=868; part 2a the spy episodes 802, 824, 850, 852 and 861
//! ([`spies`]); part 2b the ground-war episodes 808 and 854 ([`ground`]); part 2c the diplomacy episodes 810, 857,
//! 858 and 860 ([`diplomacy`]); part 2d 801, 804, 806, 807 and 809 ([`crises`]); part 2e 814, 815, 821, 832 and 834
//! ([`threats`]); part 2f 836, 839, 840 and 843 ([`waves`]); the other blocks of the script (844, 856, 862, 863)
//! are not yet (STATUS U24). What the episodes do to the rest of the game goes out as
//! [`StoryEvent`]s and [`StoryWorld`] calls.

mod crises;
mod diplomacy;
mod finale;
mod ground;
mod spies;
mod threats;
mod waves;

use crate::story::{flag, Abort, InfoLine, MessageText, Script, Story, StoryEvent, StoryWorld, STREAM};

/// `st_Elem_include` (parts 1 and 2a–2g).
#[derive(Debug, Clone, Default)]
pub struct ElemStories {
    /// Stories that don't run (a replay of a trace recorded before they were ported: the exe had them patched out).
    skip: Vec<i32>,
}

/// Part 2a's stories.
pub const PART_2A: [i32; 5] = [802, 824, 850, 852, 861];

/// Part 2b's stories.
pub const PART_2B: [i32; 2] = [808, 854];

/// Part 2c's stories.
pub const PART_2C: [i32; 4] = [810, 857, 858, 860];

/// Part 2d's stories.
pub const PART_2D: [i32; 5] = [801, 804, 806, 807, 809];

/// Part 2e's stories.
pub const PART_2E: [i32; 5] = [814, 815, 821, 832, 834];

/// Part 2f's stories.
pub const PART_2F: [i32; 4] = [836, 839, 840, 843];

/// Part 2g's stories.
pub const PART_2G: [i32; 4] = [844, 856, 862, 863];

impl ElemStories {
    /// The script without stories `skip`.
    pub fn without(skip: &[i32]) -> Self {
        Self { skip: skip.to_vec() }
    }
}

type Block = fn(&mut Story, &mut dyn StoryWorld) -> Result<(), Abort>;

impl Script for ElemStories {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        // `Story_Check` runs the campaign's scripts in SP only.
        if w.multiplayer() {
            return;
        }
        let stories: [(i32, Block); 47] = [
            (801, crises::story_801),
            (802, spies::story_802),
            (804, crises::story_804),
            (805, |s, w| {
                if s.starting_now() {
                    s.win(0, true, w);
                }
                Ok(())
            }),
            (806, crises::story_806),
            (807, crises::story_807),
            (808, ground::story_808),
            (809, crises::story_809),
            (810, diplomacy::story_810),
            (811, story_811),
            (813, story_813),
            (814, threats::story_814),
            (815, threats::story_815),
            (816, story_816),
            (817, story_817),
            (818, story_818),
            (820, story_820),
            (821, threats::story_821),
            (824, spies::story_824),
            (830, story_830),
            (831, story_831),
            (832, threats::story_832),
            (834, threats::story_834),
            (835, story_835),
            (836, waves::story_836),
            (837, story_837),
            (839, waves::story_839),
            (840, waves::story_840),
            (841, story_841),
            (842, story_842),
            (843, waves::story_843),
            (844, finale::story_844),
            (850, spies::story_850),
            (852, spies::story_852),
            (854, ground::story_854),
            (856, finale::story_856),
            (857, diplomacy::story_857),
            (858, diplomacy::story_858),
            (860, diplomacy::story_860),
            (861, spies::story_861),
            (862, finale::story_862),
            (863, finale::story_863),
            (864, story_864),
            (865, story_865),
            (866, story_866),
            (867, story_867),
            (868, |s, w| {
                if s.starting_now() {
                    s.v_set(8681, 0);
                }
                let _ = w;
                Ok(())
            }),
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

/// `st_Message(b, r)`: the human answered part `r` of message `b`.
fn answered(s: &Story, b: i32, r: i32) -> bool {
    s.flags[flag::MESSAGE] && s.message == (b, r)
}

/// `st_Elevul(b)`: message `b` was promoted (flag 7).
fn promoted(s: &Story, b: i32) -> bool {
    s.flags[flag::PROMOTED] && s.message.0 == b
}

/// `st_Olvas(b)`: message `b` was read (flag 9).
fn read(s: &Story, b: i32) -> bool {
    s.flags[flag::READ] && s.message.0 == b
}

fn message(w: &mut dyn StoryWorld, id: i32) {
    w.event(StoryEvent::NewMessage { id, arg: 0 });
}

fn param(w: &mut dyn StoryWorld, slot: i32, text: MessageText) {
    w.event(StoryEvent::MessageParam { slot, text });
}

/// `st_AddMoney(x, e)` (exe 0x5c9e40): `FUN_0053fb10(e < 1 ? EN : e, −1, (f64) x)`.
fn add_money(w: &mut dyn StoryWorld, x: i32, e: i32) {
    let empire = if e > 0 { e } else { w.human() };
    w.event(StoryEvent::AddMoney { empire, amount: x as f64 });
}

impl Story {
    /// `st_GrWarPart(fid, bid)`: a ground battle ended with fleet id `fid` (0: any) on planet `bid` (0: any).
    pub fn ground_part(&mut self, fid: i32, bid: i32, w: &mut dyn StoryWorld) -> bool {
        if !self.flags[flag::GROUND_END] || (fid != 0 && self.ground.fleet_id != fid) {
            return false;
        }
        bid == 0 || self.ground.planet == self.b_search(bid, 0, w)
    }

    /// `st_RangeB(bid)`: planet `bid` (0: any) came into the human's radar range (flag 11).
    pub fn radar_planet_is(&mut self, bid: i32, w: &mut dyn StoryWorld) -> bool {
        if !self.flags[flag::RADAR_PLANET] {
            return false;
        }
        bid == 0 || self.radar.0 == self.b_search(bid, 0, w)
    }

    /// `st_SpWarPartTulaj(owner)` (exe 0x602d40): the 1-based participant of the finished battle owned by
    /// `owner` among the first two (three with more than two participants), else 0.
    pub fn battle_owner_part(&self, owner: i32) -> i32 {
        if !self.flags[flag::SPACE_END] {
            return 0;
        }
        let n = if self.battle.len() > 2 { 3 } else { 2 };
        (0..n).find(|&i| self.battle.get(i).map_or(0, |p| p.owner) == owner).map_or(0, |i| i as i32 + 1)
    }

    /// `st_fAttackPlanet(fid, bid)` (exe 0x604590): fleet `fid` attacks planet `bid` (`FlottaMozgat(f, 2, 1, p, 0)`;
    /// `bid ≤ 0` is a random planet of empire `−bid`, stream 5). `StoryError` = either is missing.
    pub fn attack_planet(&mut self, fid: i32, bid: i32, w: &mut dyn StoryWorld) {
        let f = self.f_search(fid, w);
        let p = self.b_search(bid, 0, w);
        if f < 1 || p < 1 {
            self.error = 1;
        } else {
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 1, x: p as f32, y: 0.0 });
            self.error = 0;
        }
    }

    /// `st_fAttackFleet(fid, fid2)` (exe 0x604520): fleet `fid` chases fleet `fid2` (`FlottaMozgat(f, 2, 2, g, 0)`).
    pub fn attack_fleet(&mut self, fid: i32, fid2: i32, w: &mut dyn StoryWorld) {
        let f = self.f_search(fid, w);
        let g = self.f_search(fid2, w);
        if f < 1 || g < 1 {
            self.error = 1;
        } else {
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 2, x: g as f32, y: 0.0 });
            self.error = 0;
        }
    }

    /// `st_FlottaAll(id)`: fleet `id` (`< 1`: the slot `−id`) has `+0x40 == 0`. `StoryError` = no fleet.
    pub fn fleet_idle(&mut self, id: i32, w: &mut dyn StoryWorld) -> bool {
        let f = if id < 1 { id.wrapping_neg() } else { self.f_search(id, w) };
        self.error = (f == 0) as i32;
        f != 0 && w.fleet_command(f) == 0
    }

    /// `st_bSetExtra(x, on)` (exe 0x603ed0): planet `−x` (`x < 1`) or `b(x)` gets `+0x186`/`+0x187 := on` and the
    /// undo entry `(8, p, StoryLastChecked)`.
    pub fn b_set_extra(&mut self, x: i32, on: bool, w: &mut dyn StoryWorld) {
        let p = if x < 1 { x.wrapping_neg() } else { self.b_search(x, 0, w) };
        if p != 0 {
            w.event(StoryEvent::PlanetExtra { planet: p, on });
            self.add_undo(8, p, self.last_checked);
        }
    }

    /// `st_bIsExtra(x)`: planet `+0x187`.
    pub fn b_is_extra(&mut self, x: i32, w: &mut dyn StoryWorld) -> bool {
        let p = if x < 1 { x.wrapping_neg() } else { self.b_search(x, 0, w) };
        p != 0 && w.planet_extra(p)
    }

    /// `st_bAddInfo(x, info, owner)` (exe 0x603170): `BolygoInfoAdd(p, info, true)` and the undo entry `(6, p << 16 |
    /// info & 0xffff, owner)` (owner −1: `StoryLastChecked`; 0: none). `StoryError` = no planet.
    pub fn b_add_info(&mut self, x: i32, info: i32, owner: i32, w: &mut dyn StoryWorld) {
        let p = if x < 1 { x.wrapping_neg() } else { self.b_search(x, 0, w) };
        let owner = if owner == -1 { self.last_checked } else { owner };
        if p < 1 {
            self.error = 1;
            return;
        }
        self.error = 0;
        w.event(StoryEvent::AddPlanetInfo { planet: p, info });
        if owner != 0 {
            self.add_undo(6, (p << 16) | (info & 0xffff), owner);
        }
    }

    /// `st_bSetRadarMsg(id, on)` (inlined in the exe): planet `+0xbb`. `StoryError` = no planet.
    pub fn b_set_radar_msg(&mut self, id: i32, on: bool, w: &mut dyn StoryWorld) {
        let p = self.b_search(id, 0, w);
        if p < 1 {
            self.error = 1;
        } else {
            self.error = 0;
            w.event(StoryEvent::PlanetRadarMsg { planet: p, on });
        }
    }

    /// `st_SetBolygoTulaj(p, owner, race)` with an explicit race (`−1`: `PlayerFaj[owner]`).
    pub fn set_planet_owner_race(&mut self, p: i32, owner: i32, race: i32, w: &mut dyn StoryWorld) {
        let race = if race == -1 { w.race(owner) } else { race };
        w.event(StoryEvent::SetPlanetOwner { planet: p, owner, race });
    }

    /// `st_SetTozsdeOlcso(on)` (exe 0x602b20): `StoryAdat+0x60` counts the cheap-trade requests; the trade
    /// goods get cheaper on the first and dearer again when the last one ends.
    pub fn trade_cheap(&mut self, on: bool, w: &mut dyn StoryWorld) {
        const OFF: usize = 0x60;
        let c = self.adat_i32(OFF);
        if !on && c < 1 {
            return;
        }
        self.set_adat_i32(OFF, if on { c.wrapping_add(1) } else { c - 1 });
        if (on && c == 0) || (!on && c == 1) {
            w.event(StoryEvent::TradePrices { cheap: on });
        }
    }

    /// `st_SetBolygoCoord(p, w)` (exe 0x6025d0): planet `p` moves to a free point (no planet within 20, outside
    /// every radar range) `r` from a random planet of the human's (`r = rnd(5, (int)(R + w·0.7), (int)(R + w·1.5))`
    /// in f64, `R = RadarHatotav2[owner][level]`, `w` 0 → 40; angle `rnd(5, 1, 360)`), at most 999 tries. False
    /// when the human has no planet or no try fits.
    pub fn set_planet_coord(&mut self, p: i32, width: i32, w: &mut dyn StoryWorld) -> bool {
        let width = if width == 0 { 40 } else { width };
        let en = w.human();
        let mut tries = 1000;
        loop {
            tries -= 1;
            let q = self.b_search(en.wrapping_neg(), 0, w);
            if q == 0 {
                return false;
            }
            let range = w.radar_range(w.planet_owner(q), w.planet_radar(q)) as f64;
            let lo = (width as f64 * 0.7 + range) as i32;
            let hi = (width as f64 * 1.5 + range) as i32;
            let r = w.rng().range(STREAM, lo, hi);
            let a = w.rng().range(STREAM, 1, 360);
            // The exe's constant, not π.
            #[allow(clippy::approx_constant)]
            let a = (a as f64 * 3.141592) / 180.0;
            let (qx, qy) = w.planet_xy(q);
            let x = (a.cos() * r as f64 + qx as f64) as f32;
            let y = (a.sin() * r as f64 + qy as f64) as f32;
            let mut free = true;
            for k in 1..=w.planet_count() {
                let (kx, ky) = w.planet_xy(k);
                let (dx, dy) = (x - kx, y - ky);
                if dy * dy + dx * dx < 400.0 {
                    free = false;
                }
            }
            if tries == 0 {
                return false;
            }
            if free && !w.in_radar_range(x as i32, y as i32, 3) {
                w.event(StoryEvent::SetPlanetXY { planet: p, x, y });
                return true;
            }
        }
    }

    /// `st_bGetLegerosebb()` (exe 0x6038a0): the human's planet (with a view `> 0`) with the most defence: per
    /// built slot (`+8 == 0`) 1, the shields 0x20..=0x23 `3·kind − 90` more; the running best is compared after
    /// every slot (a planet's empty slots keep its sum). 0 when none.
    pub fn strongest_planet(&self, w: &dyn StoryWorld) -> i32 {
        let en = w.human();
        let (mut best, mut pick) = (0, 0);
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) != en || w.planet_visibility(p) <= 0 {
                continue;
            }
            let mut sum = 0i32;
            for k in 1..=w.building_count(p) {
                let (kind, _, built) = w.building(p, k);
                if kind != 0 && built == 0 {
                    if (0x20..0x24).contains(&kind) {
                        sum = sum.wrapping_add(kind * 3 - 0x5b);
                    }
                    sum = sum.wrapping_add(1);
                }
                if best < sum {
                    best = sum;
                    pick = p;
                }
            }
        }
        pick
    }
}

// ---------------------------------------------------------------- the small episodes

/// 811: a timer, then the end.
fn story_811(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.sz_start(8110, 30);
    }
    if s.counter(8110) {
        s.win(0, true, w);
    }
    Ok(())
}

/// 813: cheap trade for a year.
fn story_813(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.sz_start(8131, 360);
        s.trade_cheap(true, w);
    }
    if s.counter(8131) {
        s.trade_cheap(false, w);
        message(w, 8131);
        s.win(0, true, w);
    }
    Ok(())
}

/// 816: planet 8160 (empire 10's, with story 403's escorts) is shown; when it falls the escorts' end is
/// awaited; while empire 10 still holds it a raider (8185) comes and flies off.
fn story_816(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const BASE: i32 = 8160;
    if s.starting_now() {
        s.b_show(BASE, -1, w);
        let p = s.b_search(BASE, 0, w);
        s.b_add_id(p, 4031);
        s.auto_add_tank(BASE, w);
        s.auto_add_building(BASE, w);
        s.start_story(403, false);
        let p = s.b_search(BASE, 0, w);
        s.add_info_line(8161, InfoLine::Planet { planet: p, count: None }, -1, w);
        s.v_set(8161, 1);
    }
    if s.flags[flag::GROUND_END] || s.battle_part(0) != 0 {
        let p = s.b_search(BASE, 0, w);
        if w.planet_owner(p) != 10 {
            if s.v_get(8161) != 0 {
                s.remove_game_info(8161, w);
                let p = s.b_search(BASE, 0, w);
                w.event(StoryEvent::RemovePlanetInfo { planet: p, info: -BASE });
                s.v_set(8161, 0);
            }
            let mut any = false;
            for k in 0..10 {
                if s.f_search(4030 + k, w) != 0 {
                    any = true;
                }
            }
            if !any {
                s.win(0, true, w);
            }
        }
    }
    if s.counter(8183) {
        let p = s.b_search(BASE, 0, w);
        if w.planet_owner(p) == 10 {
            message(w, 5614);
            s.sz_start(8189, 80);
        }
    }
    if s.counter(8189) {
        let p = s.b_search(BASE, 0, w);
        s.f_new_ex("", 10, 8185, 0.06, 2, p, w);
        s.send_away(8185, w);
        s.v_set(8181, 1);
    }
    if s.arrived_point(8185) {
        s.win(0, true, w);
    }
    if s.battle_owner_part(w.human()) != 0 && s.battle_part(8185) != 0 {
        message(w, 5615);
        s.sz_start(5619, 10);
    }
    if s.counter(5619) {
        s.v_set(8181, 2);
        s.win(0, true, w);
    }
    Ok(())
}

/// 817: empire 11's fleet (8170) attacks the human; at difficulty 4 empire 11's planets are razed daily.
fn story_817(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const F: i32 = 8170;
    let en = w.human();
    if s.starting_now() {
        s.f_new_ex("", 11, F, 0.07, 3, 0, w);
        let target = if s.v_get(7250) != 0 {
            en.wrapping_neg()
        } else {
            let p = s.b_search(8011, 0, w);
            if w.planet_owner(p) == en { 8011 } else { en.wrapping_neg() }
        };
        s.attack_planet(F, target, w);
        s.sz_start(8179, 20);
        s.add_info_line(8171, InfoLine::Blank, -1, w);
    }
    if s.counter(8179) {
        if s.f_search(F, w) == 0 {
            s.win(0, true, w);
        } else {
            if s.fleet_idle(F, w) {
                s.attack_planet(F, en.wrapping_neg(), w);
            }
            s.sz_start(8179, 10);
        }
    }
    if s.daily() && w.difficulty() == 4 {
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) == 11 {
                w.event(StoryEvent::RazePlanet(p));
            }
        }
    }
    Ok(())
}

/// 818: story 816 with `v(8189) := 1`; ends with it.
fn story_818(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.v_set(8189, 1);
        s.start_story(816, false);
    }
    if s.story_finished(816) {
        s.win(0, true, w);
    }
    Ok(())
}

/// 820: planet 8200 goes to empire 10; once the human has two planets it is offered (821), moved out of the
/// radar ranges with a guard fleet; taking it pays 234,000.
fn story_820(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const P: i32 = 8200;
    if s.starting_now() {
        let p = s.b_search(P, 1, w);
        s.set_planet_owner_race(p, 10, -1, w);
        s.auto_add_tank(P, w);
        s.auto_add_building(P, w);
        s.sz_start(8201, 1);
    }
    if s.counter(8201) {
        if w.planets_of(w.human()) < 2 {
            s.sz_start(8201, 10);
        } else {
            message(w, 8206);
            s.start_story(821, false);
            s.b_add_info(P, -P, -1, w);
            s.v_set(8209, 0);
        }
    }
    if read(s, 8206) {
        s.b_unhide(P, w);
        s.b_set_radar_msg(P, true, w);
        let p = s.b_search(P, 0, w);
        s.set_planet_coord(p, 50, w);
        s.f_new_ex("", 10, 8201, 0.06, 2, p, w);
        s.sz_start(8209, 420);
    }
    if (s.radar_planet_is(P, w) || s.counter(8209)) && s.v_get(8209) == 0 {
        message(w, 8203);
        let p = s.b_search(P, 0, w);
        param(w, 1, MessageText::Planet(p));
        s.b_show(P, -1, w);
        s.v_set(8209, 1);
    }
    if s.ground_part(0, P, w) {
        let p = s.b_search(P, 0, w);
        if w.planet_owner(p) != 10 {
            message(w, 8205);
            w.event(StoryEvent::MessageParamInt { slot: 1, value: 234_000 });
            add_money(w, 234_000, -1);
            s.win(0, true, w);
        }
    }
    Ok(())
}

/// `StoryInit_GenerateSupernovaBolygok_de_nem_kell_elore_meghivni(k)` (exe 0x5c5230): planets 8010/8011 (`k`:
/// 8301/8302) are shown and go to empire 16 with race 11, the first worth 14,800 and marked extra, both with their
/// story buildings. Aborts when either is missing.
pub(super) fn supernova_planets(s: &mut Story, second: bool, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let (a, b) = if second { (8301, 8302) } else { (8010, 8011) };
    let p = s.b_search(a, 1, w);
    if p == 0 {
        return Err(Abort);
    }
    let q = s.b_search(b, 1, w);
    if q == 0 {
        return Err(Abort);
    }
    s.b_unhide(p.wrapping_neg(), w);
    s.b_unhide(q.wrapping_neg(), w);
    s.set_planet_owner_race(p, 16, 11, w);
    s.set_planet_owner_race(q, 16, 11, w);
    w.event(StoryEvent::SetPlanetValue { planet: p, value: 14_800 });
    s.auto_add_building(a, w);
    s.auto_add_building(b, w);
    s.b_set_extra(p.wrapping_neg(), true, w);
    Ok(())
}

/// 830: the supernova: planet 8301 explodes in 900 days unless the human takes it; 8302 holds the warning.
fn story_830(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        supernova_planets(s, true, w)?;
        message(w, 8309);
        param(w, 1, MessageText::Empire(en));
        let p = s.b_search(8301, 0, w);
        param(w, 2, MessageText::Planet(p));
        s.sz_start(8301, 900);
        s.sz_start(8302, 15);
        s.b_show(8301, -1, w);
        s.b_set_radar_msg(8302, true, w);
        s.auto_add_tank(8301, w);
        s.auto_add_tank(8302, w);
    }
    if s.radar_planet_is(8302, w) {
        message(w, 8308);
        s.v_set(8300, 1);
    }
    if s.counter(8302) {
        message(w, 8300);
    }
    if read(s, 8300) {
        s.add_info_line(8301, InfoLine::Blank, -1, w);
    }
    if s.ground_part(0, 8301, w) && s.ground.attacker_won {
        s.v_set(8301, 1);
        s.remove_game_info(8301, w);
        if s.v_get(8300) == 0 {
            message(w, 8301);
            s.b_show(8302, -1, w);
            s.b_set_radar_msg(8302, false, w);
            s.add_info_line(8302, InfoLine::Blank, -1, w);
        } else {
            let p = s.b_search(8302, 0, w);
            if w.planet_owner(p) == 16 {
                message(w, 8307);
                s.add_info_line(8302, InfoLine::Blank, -1, w);
            } else {
                message(w, 8306);
            }
        }
    }
    if s.ground_part(0, 8302, w) && s.ground.attacker_won {
        // The UI line only (`RemoveGameInfo`, not `st_RemoveGameInfo`: the undo list keeps its entry).
        w.event(StoryEvent::RemoveGameInfo(8302));
    }
    if s.counter(8301) {
        message(w, if s.v_get(8301) == 1 { 8302 } else { 8303 });
        let p = s.b_search(8301, 0, w);
        s.error = (p < 1) as i32;
        w.event(StoryEvent::DestroyPlanet(p));
        w.event(StoryEvent::Anim(8301));
        s.win(0, true, w);
    }
    Ok(())
}

/// 831's research rush, by difficulty (`Nehezseg` 0..=4).
const RUSH: [i32; 5] = [0, 12, 10, 7, 5];
/// The techs 831 speeds up.
const RUSH_TECHS: std::ops::RangeInclusive<i32> = 161..=171;

/// 831: the techs 161..=171 research twice as fast for a while (and back).
fn story_831(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    let d = RUSH.get(w.difficulty() as usize).copied().unwrap_or(0);
    if s.starting_now() {
        if w.tech_status(162, en) == 5 {
            s.win(0, true, w);
        } else {
            message(w, 8310);
        }
    }
    if read(s, 8310) {
        s.add_info_line(8311, InfoLine::Blank, -1, w);
        s.sz_start(8312, 4);
        s.sz_start(8311, d.wrapping_mul(30));
    }
    if s.counter(8312) {
        message(w, 8311);
        w.event(StoryEvent::MessageParamInt { slot: 1, value: d.wrapping_add(4) });
        w.event(StoryEvent::MessageParamInt { slot: 2, value: d });
        for t in RUSH_TECHS {
            w.event(StoryEvent::TechTimes { tech: t, empire: en, halve: true, price: false });
        }
        s.v_set(8319, 1);
    }
    let mut done = false;
    if s.special_is(13, en, 0, 0) && RUSH_TECHS.contains(&s.special[2]) {
        if s.sz_is_now_going(8311) {
            message(w, 8312);
            param(w, 1, MessageText::Tech(s.special[2]));
            done = true;
        } else {
            message(w, 8313);
        }
        s.win(0, true, w);
    }
    if s.counter(8311) {
        let busy = RUSH_TECHS.clone().any(|t| w.tech_status(t, en) == 4 || w.tech_time_left(t, en) > 0);
        message(w, if busy { 8314 } else { 8315 });
        done = true;
    }
    if done && s.v_get(8319) != 0 {
        s.v_set(8319, 0);
        for t in RUSH_TECHS {
            w.event(StoryEvent::TechTimes { tech: t, empire: en, halve: false, price: false });
        }
    }
    Ok(())
}

/// 835: empire `v(8359)`'s last visible planet is marked; when it has none left the human's strongest planet
/// gets id 8351 (20 days later).
fn story_835(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        // The German build (`Lang == 4`) shows the race's name with a suffix instead (UI; the same reads).
        let e = s.v_get(8359);
        s.add_info_line(8351, InfoLine::Empire(e), -1, w);
        s.v_set(8351, 0);
    }
    if s.flags[flag::GROUND_END] || s.daily() {
        let (mut n, mut last) = (0, 0);
        let e = s.v_get(8359);
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) == e && w.planet_visibility(p) >= 0 {
                n += 1;
                last = p;
                if s.b_is_extra(p.wrapping_neg(), w) {
                    s.b_set_extra(p.wrapping_neg(), false, w);
                }
            }
        }
        if n == 1 {
            s.b_set_extra(last.wrapping_neg(), true, w);
        } else if n == 0 && s.v_get(8351) == 0 {
            message(w, 8352);
            s.remove_game_info(8351, w);
            s.sz_start(8351, 20);
            s.v_set(8351, 1);
        }
    }
    if s.counter(8351) {
        message(w, 8353);
        let p = s.strongest_planet(&*w);
        s.b_add_id(p, 8351);
        let p = s.b_search(8351, 0, w);
        param(w, 1, MessageText::Planet(p));
        s.win(0, true, w);
    }
    if s.counter(8350) {
        message(w, 8351);
        s.win(0, true, w);
    }
    Ok(())
}

/// 837: once the human owns more than two thirds of the owned planets it sees, a fleet of its own joins.
fn story_837(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const F: i32 = 8370;
    let en = w.human();
    if s.starting_now() {
        s.sz_start(F, 5);
    }
    if s.counter(F) {
        let (mut seen, mut own) = (0i32, 0i32);
        for p in 1..=w.planet_count() {
            let o = w.planet_owner(p);
            if o != 0 && w.planet_visibility(p) >= 0 {
                seen += 1;
                if o == en {
                    own += 1;
                }
            }
        }
        if own.wrapping_mul(3) > seen.wrapping_mul(2) {
            w.event(StoryEvent::ScrollText { text: 8371, important: true });
            let f = s.f_new_ex("", en, F, 0.0, 1, 0, w);
            let b = s.last_fn[crate::story::last_fn::B];
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: b as f32, y: 0.0 });
            w.event(StoryEvent::Anim(F));
            message(w, F);
            param(w, 1, MessageText::Planet(b));
            s.win(0, true, w);
        } else {
            s.sz_start(F, 5);
        }
    }
    Ok(())
}

/// 841: tech 6 researches twice as fast and costs half until the human has it.
fn story_841(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const T: i32 = 6;
    let en = w.human();
    if s.starting_now() {
        message(w, 8410);
        s.sz_start(8411, 280);
        if w.tech_status(T, en) == 5 {
            s.win(0, true, w);
        } else {
            s.v_set(8418, T);
            w.event(StoryEvent::TechTimes { tech: T, empire: en, halve: true, price: true });
            s.v_set(8419, 1);
            s.add_info_line(8411, InfoLine::Tech(T), -1, w);
        }
    }
    if s.daily() && w.tech_status(T, en) == 5 {
        message(w, 8412);
        s.win(0, true, w);
    }
    Ok(())
}

/// 842's deadline in days by difficulty (`Nehezseg` 0..=4).
const COLONY_DAYS: [i32; 5] = [0, 1080, 900, 540, 540];

/// 842: with a ship of a flagged design (`+0x34`) the human has a deadline to found two colonies.
fn story_842(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const C: i32 = 8429;
    let en = w.human();
    if s.starting_now() {
        s.v_set(C, 1);
    }
    if s.daily() && s.v_get(C) != 0 {
        let any = (1..=w.ship_count()).map(|k| w.ship(k)).any(|(f, d)| d != 0x7fff && w.fleet_owner(f) == en && w.design_colony(en, d));
        if any {
            message(w, 8420);
            s.sz_start(C, COLONY_DAYS.get(w.difficulty() as usize).copied().unwrap_or(0));
            s.v_set(8420, 0);
            s.v_set(C, 0);
            s.add_info_line(8421, InfoLine::Blank, -1, w);
        }
    }
    if s.special_is(10, en, 0, 0) && s.v_get(C) == 0 {
        s.v_inc(8420);
        if s.v_get(8420) == 1 {
            s.remove_game_info(8421, w);
        } else {
            s.remove_game_info(8422, w);
        }
        if s.v_get(8420) == 2 {
            message(w, 8421);
            w.event(StoryEvent::GlobalMorale(-200));
            s.win(0, true, w);
        } else {
            s.add_info_line(8422, InfoLine::Blank, -1, w);
        }
    }
    if s.counter(C) {
        message(w, C);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 864..=868: the alliance with empire 5

/// 864: an alliance with empire 5 against empire 1: money asked for, a fleet sent; it ends with war, either
/// empire's death or story 866.
fn story_864(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        if !w.empire_alive(1) || !w.empire_alive(5) {
            return Err(Abort);
        }
        s.v_set(8651, 0);
        message(w, 8640);
        s.sz_start(8640, 200);
        w.event(StoryEvent::MakeAllies { a: en, b: 5 });
    }
    if s.daily() && w.relation_level(en, 5) < 3 && s.v_get(8460) == 0 {
        s.v_set(8460, 1);
        message(w, 8649);
        s.start_story(866, false);
        s.win(0, true, w);
    }
    if s.counter(8640) {
        if w.money(en) <= 100_000.0 {
            s.sz_start(8460, 52);
        } else {
            message(w, 8641);
            param(w, 1, MessageText::Empire(5));
        }
    }
    for (part, share) in [(2, 0.75), (3, 0.4)] {
        if answered(s, 8641, part) {
            let x = (w.money(en) * share) as i32;
            add_money(w, x.wrapping_neg(), -1);
            add_money(w, x, 5);
        }
    }
    if answered(s, 8641, 4) || promoted(s, 8641) {
        s.sz_start(8640, 100);
    }
    if answered(s, 8644, 2) {
        s.sz_start(8642, 60);
    }
    if answered(s, 8644, 3) || promoted(s, 8644) {
        s.sz_start(8643, 200);
    }
    if s.arrived_point(8642) {
        s.attack_fleet(8642, 8640, w);
    }
    if s.daily() {
        if !w.empire_alive(1) {
            message(w, 8645);
            w.event(StoryEvent::DeclareWar { a: en, b: 5 });
            s.win(0, true, w);
        } else if !w.empire_alive(5) {
            message(w, 8646);
            s.v_set(8661, 1);
            s.start_story(866, false);
            s.win(0, true, w);
        }
    }
    Ok(())
}

/// Empire 1 is dead: unless story 405 runs, the end message and story 867 (`StoryAdat+0xc8 := 1`).
fn empire_1_beaten(s: &mut Story, w: &mut dyn StoryWorld) {
    if !s.is_running(405) {
        message(w, 8622);
        s.set_adat_i32(0xc8, 1);
        s.start_story(867, false);
    }
    s.win(0, true, w);
}

/// 865: daily while `v(8651)`: empire 5 dead → the end; empire 1 dead → the victory.
fn story_865(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.daily() && s.v_get(8651) != 0 {
        if !w.empire_alive(5) {
            message(w, 8623);
            s.win(0, true, w);
        } else if !w.empire_alive(1) {
            empire_1_beaten(s, w);
        }
    }
    Ok(())
}

/// 866: daily: empire 5's death is reported once; empire 1's death ends it (with the victory unless empire 5 died).
fn story_866(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.daily() {
        if s.v_get(8661) == 0 {
            if !w.empire_alive(5) {
                message(w, 8646);
                s.v_set(8661, 1);
            } else if !w.empire_alive(1) {
                empire_1_beaten(s, w);
            }
        } else if !w.empire_alive(1) {
            message(w, 8647);
            s.win(0, true, w);
        }
    }
    Ok(())
}

/// 867: `StoryAdat+0xc8` for 120 days.
fn story_867(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        s.sz_start(8671, 120);
    }
    if s.counter(8671) {
        s.set_adat_i32(0xc8, 0);
        s.win(0, true, w);
    }
    Ok(())
}
