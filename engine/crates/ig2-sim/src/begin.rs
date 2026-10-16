//! The campaign's opening episodes: `st_Begin_include` (exe 0x5cec20, ELF 0x7407b0; `st_begin.cpp`), the first
//! script `Story_Check` runs. Spec: `re/rules/campaign.md` §8.12.
//!
//! Stories 1..=24 are the episodes story 401 draws from `StoryEleje` (§8.4): a pirate base nearby (1), the
//! plague ship (4), the hunter (5, with 51), rebels (6), troops wanted (7), a design for sale (8), a trade treaty
//! (9), a colony (10, 11), a research grant (12), a saboteur (19), a tech to steal (20), a spy's target (21), a far
//! planet (22), a fugitive (23) and an escort (24). What they do to the rest of the game goes out as
//! [`StoryEvent`]s and [`StoryWorld`] calls.

use crate::brnd::{can_give, distance, random_empire};
use crate::story::{flag, last_fn, Abort, InfoLine, KemNews, MessageText, Script, Story, StoryEvent, StoryWorld, STREAM};

/// `st_Begin_include`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BeginStories;

type Block = fn(&mut Story, &mut dyn StoryWorld, &dyn Script) -> Result<(), Abort>;

impl Script for BeginStories {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        self.run_in(s, w, self);
    }

    fn run_in(&self, s: &mut Story, w: &mut dyn StoryWorld, all: &dyn Script) {
        // `Story_Check` runs the campaign's scripts in SP only.
        if w.multiplayer() {
            return;
        }
        let stories: [(i32, Block); 17] = [
            (1, base),
            (4, plague),
            (5, hunter),
            (51, cheap_design),
            (6, rebels),
            (7, troops),
            (8, for_sale),
            (9, treaty),
            (10, colony),
            (11, colony_in_time),
            (12, grant),
            (19, saboteur),
            (20, steal),
            (21, spy_target),
            (22, far_planet),
            (23, fugitive),
            (24, escort),
        ];
        for (id, story) in stories {
            // The catch handler of each story block: `WIN(0, 1)`, then the next story.
            if s.running(id) && story(s, w, all).is_err() {
                s.win(0, true, w);
            }
        }
    }
}

// ---------------------------------------------------------------- helpers

fn rnd(w: &mut dyn StoryWorld, lo: i32, hi: i32) -> i32 {
    w.rng().range(STREAM, lo, hi)
}

/// A value of a table indexed by `Nehezseg` (the scripts' local arrays of five).
fn by_difficulty(w: &dyn StoryWorld, table: [i32; 5]) -> i32 {
    table.get(w.difficulty() as usize).copied().unwrap_or(0)
}

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

fn message(w: &mut dyn StoryWorld, id: i32, arg: i32) {
    w.event(StoryEvent::NewMessage { id, arg });
}

fn param(w: &mut dyn StoryWorld, slot: i32, text: MessageText) {
    w.event(StoryEvent::MessageParam { slot, text });
}

fn param_int(w: &mut dyn StoryWorld, slot: i32, value: i32) {
    w.event(StoryEvent::MessageParamInt { slot, value });
}

fn money(w: &mut dyn StoryWorld, amount: i32) {
    let en = w.human();
    w.event(StoryEvent::AddMoney { empire: en, amount: amount as f64 });
}

fn move_fleet(w: &mut dyn StoryWorld, fleet: i32, kind: i32, mode: i32, x: f32, y: f32) {
    w.event(StoryEvent::MoveFleet { fleet, kind, mode, x, y });
}

/// The fleet slot of `x` (`< 1`: the slot `−x`), as the fleet helpers take it.
pub(crate) fn slot_of(s: &mut Story, x: i32, w: &mut dyn StoryWorld) -> i32 {
    if x < 1 {
        x.wrapping_neg()
    } else {
        s.f_search(x, w)
    }
}

/// `st_fAttackFleet(id, x)` (exe 0x604520): fleet `id` follows fleet `x` (`x < 0`: a random fleet of `−x`).
fn attack_fleet(s: &mut Story, id: i32, x: i32, w: &mut dyn StoryWorld) -> bool {
    let f = s.f_search(id, w);
    let g = s.f_search(x, w);
    let ok = f > 0 && g > 0;
    if ok {
        move_fleet(w, f, 2, 2, g as f32, 0.0);
    }
    s.error = !ok as i32;
    ok
}

/// `st_fAttackPlanet(id, x)` (exe 0x604590): fleet `id` attacks planet `x` (`x < 0`: a random visible planet of
/// `−x`).
fn attack_planet(s: &mut Story, id: i32, x: i32, w: &mut dyn StoryWorld) -> bool {
    let f = s.f_search(id, w);
    let g = s.b_search(x, 0, w);
    let ok = f > 0 && g > 0;
    if ok {
        move_fleet(w, f, 2, 1, g as f32, 0.0);
    }
    s.error = !ok as i32;
    ok
}

/// Fleet `id` goes for one of the human's fleets, else one of its planets.
fn attack_human(s: &mut Story, id: i32, w: &mut dyn StoryWorld) {
    let en = w.human();
    if !attack_fleet(s, id, en.wrapping_neg(), w) {
        attack_planet(s, id, en.wrapping_neg(), w);
    }
}

/// `st_fMoveToPlanet(x, b)` (exe 0x604b40).
pub(crate) fn move_to_planet(s: &mut Story, x: i32, b: i32, w: &mut dyn StoryWorld) {
    let f = slot_of(s, x, w);
    let g = s.b_search(b, 0, w);
    let ok = f > 0 && g > 0;
    if ok {
        move_fleet(w, f, 1, 1, g as f32, 0.0);
    }
    s.error = !ok as i32;
}

/// `st_FlottaAll(x)` (exe 0x5ea3b0): the fleet has no order (`+0x40 == 0`).
fn stands(s: &mut Story, x: i32, w: &mut dyn StoryWorld) -> bool {
    let f = slot_of(s, x, w);
    s.error = (f == 0) as i32;
    f != 0 && w.fleet_command(f) == 0
}

/// `st_fDelete(x)`.
pub(crate) fn delete_fleet(s: &mut Story, x: i32, w: &mut dyn StoryWorld) {
    let f = if x < 0 { x.wrapping_neg() } else { s.f_search(x, w) };
    if f >= 1 {
        w.event(StoryEvent::DeleteFleet(f));
    }
    s.error = (f < 1) as i32;
}

/// `st_fShow(id)` / `st_fHide(x)`: fleet `+0x52` bit 0x100 (always visible).
fn show_fleet(s: &mut Story, x: i32, on: bool, w: &mut dyn StoryWorld) {
    let f = if on { s.f_search(x, w) } else if x < 0 { x.wrapping_neg() } else { s.f_search(x, w) };
    if f >= 1 {
        w.event(StoryEvent::SetFleetVisible { fleet: f, on });
    }
    s.error = (f < 1) as i32;
}

/// `(int)(f32) sqrt((f64)(f32)(dy² + dx²))` of f32 coordinates (`st_bDistFromBir`'s and `st_bGetFromDist`'s).
pub(crate) fn planet_distance(a: (f32, f32), b: (f32, f32)) -> i32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    ((dy * dy + dx * dx) as f64).sqrt() as f32 as i32
}

/// `st_bDistFromBir(q, −1)` (exe 0x603240): the distance from the human's nearest visible planet (1..n − 1) to
/// planet `q`, 1,000,000 without one.
pub(crate) fn human_distance(q: i32, w: &dyn StoryWorld) -> i32 {
    let en = w.human();
    let target = w.planet_xy(q);
    (1..w.planet_count())
        .filter(|&p| w.planet_owner(p) == en && w.planet_visibility(p) >= 0)
        .map(|p| planet_distance(w.planet_xy(p), target))
        .fold(1_000_000, i32::min)
}

/// `st_fDistFromBir(f, −1)` (exe 0x604650): the same to fleet `f` with `st_tav`.
fn fleet_distance(f: i32, w: &dyn StoryWorld) -> i32 {
    let en = w.human();
    let target = w.fleet_xy(f);
    (1..w.planet_count())
        .filter(|&p| w.planet_owner(p) == en && w.planet_visibility(p) >= 0)
        .map(|p| distance(w.planet_xy(p), target))
        .fold(1_000_000, i32::min)
}

impl Story {
    /// `st_bGetFromDist(lo, hi)` (exe 0x6036e0): a free planet the human sees, preferably between `lo` and `hi`
    /// (`0`: `2·lo`) from its planets (a reservoir pick, stream 5), else the nearest beyond `hi` of 100 tries,
    /// else any; aborts when there is none.
    pub fn planet_at_distance(&mut self, lo: i32, hi: i32, w: &mut dyn StoryWorld) -> Result<i32, Abort> {
        let hi = if hi == 0 { lo.wrapping_mul(2) } else { hi };
        if self.b_search_try(0, 1, w)? == 0 {
            return self.b_search_try(0, 0, w);
        }
        let (mut pick, mut best, mut k) = (0, 1_000_500, 1);
        for _ in 0..100 {
            let q = self.b_search_try(0, 0, w)?;
            let d = human_distance(q, &*w);
            if hi < d && d < best {
                pick = q;
                best = d;
            } else if lo < d && d < hi {
                let r = rnd(w, 1, k);
                k += 1;
                if r == 1 {
                    pick = q;
                    best = d;
                }
            }
        }
        if pick != 0 {
            Ok(pick)
        } else {
            self.b_search_try(0, 0, w)
        }
    }

    /// `st_GetNearestBir(x, y)` (exe 0x5ea800): the owner (1..=8, not the human) of the planet nearest to `(x, y)`
    /// (f32, against the last pick's truncated value), 0 without one; the planet goes to `nearest_bir_planet`.
    pub fn nearest_empire(&mut self, x: i32, y: i32, w: &dyn StoryWorld) -> i32 {
        let en = w.human();
        let (mut pick, mut best) = (0, 0i32);
        for p in 1..=w.planet_count() {
            let o = w.planet_owner(p);
            if o == 0 || o == en || o >= 9 {
                continue;
            }
            let (px, py) = w.planet_xy(p);
            let (dx, dy) = (x as f32 - px, y as f32 - py);
            let d = dy * dy + dx * dx;
            if pick == 0 || d < best as f32 {
                pick = p;
                best = d as i32;
            }
        }
        self.nearest_planet = pick;
        if pick == 0 {
            0
        } else {
            w.planet_owner(pick)
        }
    }

    /// `st_CounterIntelligence(x)` (exe 0x5d4900): each of the human's spies at home on counter-espionage gets a
    /// chance (its skills + `x`, 10..=100 %, stream 5) to catch the enemy; the first that does is
    /// `CounterIntelligence_kem`.
    pub fn counter_intelligence(&mut self, x: i32, w: &mut dyn StoryWorld) -> bool {
        let en = w.human();
        self.counter_spy = 0;
        let mut k = 1;
        while k <= w.spy_count(en) {
            let spy = w.spy_record(en, k);
            if spy.state == 3 && spy.mission == 13 {
                let sum = (spy.skill.1 * 40 + spy.skill.0 * 60) * 10;
                let chance = ((sum as f64) / 100.0 + x as f64) as i32;
                let chance = if chance > 100 { 100 } else { chance.max(10) };
                if rnd(w, 0, 99) < chance {
                    self.counter_spy = k;
                    return true;
                }
            }
            k += 1;
        }
        false
    }

    /// `infect(p)`: the planet's epidemic `+0xb8 += 20`, then `Virus_CheckAdd(p)` (exe 0x542930): info 100 and the
    /// epidemic switch.
    fn infect(&mut self, p: i32, w: &mut dyn StoryWorld) {
        let value = (w.planet_epidemic(p) + 20) as i16 as i32;
        w.event(StoryEvent::SetEpidemic { planet: p, value });
        w.event(StoryEvent::VirusCheck(p));
        self.set_adat_i32(crate::story::adat::EPIDEMIC, 1);
    }

    /// `st_bSetRadarMsg(id, true)`: the planet with story id `id` gets `+0xbb := 1`.
    pub(crate) fn radar_notice(&mut self, id: i32, w: &mut dyn StoryWorld) {
        match self.planets.iter().find(|e| e.0 == id) {
            Some(&(_, p)) if p != 0 => {
                self.error = 0;
                w.event(StoryEvent::RadarNotice(p as i32));
            }
            _ => self.error = 1,
        }
    }

    /// Fleet `id` sent away and hidden; `v(var) := 1` (stories 5's and 6's "it gives up").
    fn give_up(&mut self, id: i32, var: i32, w: &mut dyn StoryWorld) {
        self.send_away(id, w);
        show_fleet(self, id, false, w);
        self.v_set(var, 1);
    }
}

/// `st_eSearch(e, mode)` (exe 0x604290): a building of empire `e`'s visible planets (`(planet, slot)`, `(0, 0)`
/// without one) for a saboteur: an undamaged one fitting the mode (any, or the kinds 0x34..=0x37) at random
/// (stream 5), else a damaged fitting one, else the first one found.
pub fn sabotage_target(e: i32, defences: bool, w: &mut dyn StoryWorld) -> (i32, i32) {
    let (mut p_out, mut slot, mut first_damage, mut n) = (0, 0, 0, 0);
    for p in 1..=w.planet_count() {
        if w.planet_owner(p) != e || w.planet_visibility(p) < 0 {
            continue;
        }
        for k in 1..=w.building_count(p) {
            let (kind, damage, b8) = w.building(p, k);
            if kind == 0 || b8 != 0 {
                continue;
            }
            if p_out == 0 {
                (p_out, slot, first_damage) = (p, k, damage);
            }
            if defences && !(0x33 < kind && kind < 0x38) {
                continue;
            }
            if damage == 0 && first_damage != 0 {
                // The first undamaged one restarts the pick.
                (p_out, slot, first_damage, n) = (p, k, 0, 1);
            } else if damage == 0 || first_damage != 0 || n == 0 {
                if n == 0 {
                    (n, p_out, slot) = (1, p, k);
                } else {
                    if rnd(w, 0, n) == 0 {
                        (p_out, slot) = (p, k);
                    }
                    n += 1;
                }
            }
        }
    }
    (p_out, slot)
}

/// `st_eSearch3(p, kind)` (exe 0x6041c0): a random building slot (stream 5) of kind `kind` (0: any) of planet `p`
/// (0: any planet), 0 without one.
pub fn random_building(p: i32, kind: i32, w: &mut dyn StoryWorld) -> i32 {
    let (mut pick, mut n) = (0, 0);
    for q in 1..=w.planet_count() {
        if p != 0 && p != q {
            continue;
        }
        for k in 1..=w.building_count(q) {
            let (kd, _, b8) = w.building(q, k);
            if kd != 0 && b8 == 0 && (kind == 0 || kind == kd) {
                if rnd(w, 0, n) == 0 {
                    pick = k;
                }
                n += 1;
            }
        }
    }
    pick
}

/// A story fleet `(owner, id, speed)` with the given ships and tanks at planet `at` (`st_fNewEx(…, 2, at)`).
fn story_fleet(s: &mut Story, (owner, id, speed): (i32, i32, f32), at: i32, ships: &[(i32, i32)], tanks: i32, w: &mut dyn StoryWorld) -> i32 {
    let f = s.f_new_ex("", owner, id, speed, 2, at, w);
    for &(design, n) in ships {
        s.f_add_ship(id, design, n, w);
    }
    if tanks > 0 {
        s.f_add_tank(id, 1, tanks, w);
    }
    f
}

/// The owner of a `Tankok` record's place (`≤ 0`: planet `−place`).
fn place_owner(place: i32, w: &dyn StoryWorld) -> i32 {
    if place < 1 {
        w.planet_owner(place.wrapping_neg())
    } else {
        w.fleet_owner(place)
    }
}

/// The owner of a finished battle's participant `i` (1-based; owner 0 past the list).
fn participant_owner(s: &Story, i: i32) -> i32 {
    s.battle.get((i - 1) as usize).map_or(0, |p| p.owner)
}

/// `st_SpWarLose(fid)`.
fn battle_lost(s: &Story, fid: i32) -> bool {
    let i = s.battle_part(fid);
    i != 0 && participant_owner(s, i) != s.battle_winner
}

/// `st_SpWarPartTulaj(e)` (exe 0x602d40): the human (or `e`) owns one of the first entries.
fn battle_owner_part(s: &Story, e: i32) -> bool {
    if !s.flags[flag::SPACE_END] {
        return false;
    }
    let owner = |i: usize| s.battle.get(i).map_or(0, |p| p.owner);
    owner(0) == e || owner(1) == e || (s.battle.len() > 2 && owner(2) == e)
}

/// `st_GrWarPart(fid, bid)`: a ground battle on planet `bid` (by fleet `fid`, 0 any) ended.
fn ground_war(s: &mut Story, fid: i32, bid: i32, w: &mut dyn StoryWorld) -> bool {
    if !s.flags[flag::GROUND_END] || (fid != 0 && s.ground.fleet_id != fid) {
        return false;
    }
    s.b_search(bid, 0, w) == s.ground.planet
}

// ---------------------------------------------------------------- 1: a pirate base nearby

/// The base's counter 11: [0, 500, 350, 350, 250] days.
const BASE_DAYS: [i32; 5] = [0, 500, 350, 350, 250];

fn base(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    let ships = [(3, 2), (2, 3)];
    if s.starting_now() {
        let mut p = s.b_search(11, 0, w);
        if p == 0 {
            p = s.planet_at_distance(35, 80, w)?;
            s.b_add_id(p, 11);
        }
        s.b_unhide(11, w);
        s.set_planet_owner(p, 10, w);
        s.radar_notice(11, w);
        s.auto_add_building(11, w);
        s.auto_add_tank(11, w);
        w.event(StoryEvent::SetPlanetView { planet: p, view: 0 });
        story_fleet(s, (10, 11, 0.06), p, &ships, 2, w);
        s.sz_start(18, 30);
        attack_human(s, 11, w);
        s.l_set(11, false);
        s.sz_start(19, 10);
    }
    if s.counter(18) {
        let p = s.b_search(11, 0, w);
        story_fleet(s, (10, 12, 0.06), p, &ships, 2, w);
        attack_human(s, 12, w);
    }
    if s.counter(12) {
        let p = s.b_search(11, 0, w);
        if w.planet_owner(p) == 10 {
            s.l_set(11, true);
            message(w, 11, 0);
            let p = s.b_search(11, 0, w);
            // Slot 11: the original's slip.
            param(w, 11, MessageText::Planet(p));
        }
    }
    if read(s, 11) {
        s.b_show(11, -1, w);
        let p = s.b_search(11, 0, w);
        s.add_info_line(11, InfoLine::Planet { planet: p, count: None }, -1, w);
    }
    if s.flags[flag::SPACE_START] {
        let (a, b) = w.battle_fleets();
        let id = |f: i32| if (0..=w.fleet_count()).contains(&f) { w.fleet_id(f) } else { 0 };
        let (ia, ib) = (id(a), id(b));
        if ia == 11 || ib == 11 || ia == 12 || ib == 12 {
            s.set_adat_i32(0x94, 1);
        }
    }
    if ground_war(s, 0, 11, w) && s.ground.attacker_won {
        message(w, 12, 0);
        money(w, 12_000);
        s.remove_game_info(11, w);
        if s.f_search(11, w) == 0 && s.f_search(12, w) == 0 && s.f_search(14, w) == 0 {
            s.win(0, true, w);
        }
    }
    if s.counter(19) {
        for id in [11, 12, 14] {
            let f = s.f_search(id, w);
            if f != 0 && stands(s, f.wrapping_neg(), w) && !attack_fleet(s, id, en.wrapping_neg(), w) {
                attack_planet(s, id, en.wrapping_neg(), w);
            }
        }
        s.sz_start(19, 10);
    }
    if s.counter(11) {
        // Fleet 11's slot as the planet: the original's slip.
        let at = s.f_search(11, w);
        story_fleet(s, (10, 14, 0.06), at, &[(3, 3), (2, 5)], 4, w);
        attack_human(s, 14, w);
        message(w, 13, 0);
    }
    if s.flags[flag::RADAR_PLANET] && s.radar.0 == s.b_search(11, 0, w) && s.l_is_false(11) {
        s.l_set(11, true);
        s.sz_start(12, 1);
        s.sz_start(11, by_difficulty(&*w, BASE_DAYS));
    }
    if s.battle_part(11) != 0 || s.battle_part(12) != 0 || s.battle_part(14) != 0 {
        if s.l_is_false(11) {
            s.l_set(11, true);
            s.sz_start(12, 5);
            s.sz_start(11, by_difficulty(&*w, BASE_DAYS));
        }
        if s.f_search(11, w) == 0 && s.f_search(12, w) == 0 && s.f_search(14, w) == 0 {
            let p = s.b_search(11, 0, w);
            if w.planet_owner(p) != 10 {
                s.win(0, true, w);
            }
        }
    }
    for id in [14, 12, 11] {
        if s.battle_won(id) && !attack_fleet(s, id, en.wrapping_neg(), w) {
            attack_planet(s, id, en.wrapping_neg(), w);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 4: the plague ship

fn plague(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let (mut pick, mut most) = (0, 0);
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) == en && most < w.building_count(p) {
                (pick, most) = (p, w.building_count(p));
            }
        }
        if pick == 0 {
            return Err(Abort);
        }
        s.b_add_id(pick, 41);
        message(w, 41, 100);
        s.l_set(41, false);
    }
    if answered(s, 41, 7) {
        money(w, -7000);
        s.win(0, true, w);
    }
    if answered(s, 41, 4) || answered(s, 41, 6) || promoted(s, 41) {
        let p = s.b_search(41, 0, w);
        if w.planet_owner(p) == en {
            let p = s.b_search(41, 0, w);
            let f = s.f_new_ex("", 10, 41, 0.04, 1, p << 16, w);
            s.add_info_line(41, InfoLine::Blank, -1, w);
            // It stops `k` short of the planet (f32 throughout).
            let (px, py) = w.planet_xy(p);
            let (fx, fy) = w.fleet_xy(f);
            let (dy, dx) = (py - fy, px - fx);
            let mut d = ((dy * dy + dx * dx) as f64).sqrt() as f32;
            if d == 0.0 {
                d = 1.0;
            }
            let k = Story::diff(w.difficulty(), 1, 5, 15, 25) as f32;
            let x = (px - k * dx / d) as i32;
            let y = (py - k * dy / d) as i32;
            move_fleet(w, f, 1, 0, x as f32, y as f32);
        } else {
            s.win(0, true, w);
        }
    }
    if s.arrived_point(41) {
        s.sz_start(49, by_difficulty(&*w, [0, 60, 30, 20, 10]));
    }
    if s.counter(49) {
        let p = s.b_search(41, 0, w);
        if s.f_search(41, w) == 0 || w.planet_owner(p) != en {
            s.win(0, true, w);
        } else {
            s.remove_game_info(41, w);
            s.add_info_line(42, InfoLine::Blank, -1, w);
            s.send_away(41, w);
            s.infect(p, w);
            s.l_set(41, true);
            message(w, 44, 0);
        }
    }
    if s.daily() && s.l_is_true(41) {
        let f = s.f_search(41, w);
        if f == 0 {
            s.win(0, true, w);
        } else if stands(s, f.wrapping_neg(), w) {
            s.send_away(f.wrapping_neg(), w);
        }
    }
    if battle_lost(s, 41) && s.f_search(41, w) == 0 {
        if s.l_is_true(41) {
            let p = s.b_search(41, 0, w);
            let e = w.planet_epidemic(p);
            if e > 0 && w.planet_owner(p) == en {
                message(w, 46, 0);
                w.event(StoryEvent::SetEpidemic { planet: p, value: e.min(5) });
            }
            s.win(0, true, w);
        } else {
            message(w, 42, 0);
            s.win(0, true, w);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 5, 51: the hunter

fn hunter(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let (en, n) = (w.human(), w.difficulty());
    if s.starting_now() || s.counter(59) {
        if s.f_search(en.wrapping_neg(), w) == 0 {
            return Err(Abort);
        }
        let p = s.b_search(en.wrapping_neg(), 0, w);
        s.b_add_id(p, 51);
        if p == 0 {
            return Err(Abort);
        }
        let speed = Story::diff(n, 5, 6, 7, 8) as f32;
        s.f_new_ex("", 13, 51, speed, 1, 120, w);
        s.sz_start(51, 1);
        attack_planet(s, 51, 51, w);
        message(w, 51, 0);
        param(w, 1, MessageText::Planet(p));
        s.v_set(51, 0);
        s.v_set(52, 0);
        s.v_set(53, 0);
    }
    if read(s, 51) {
        s.add_info_line(51, InfoLine::Blank, -1, w);
        show_fleet(s, 51, true, w);
    }
    if s.daily() {
        let f = s.f_search(51, w);
        if f == 0 {
            return Err(Abort);
        }
        let mode = match n {
            1 => 1,
            2 => 2,
            _ => {
                let p = s.b_search(51, 0, w);
                if w.planet_owner(p) == 13 || w.fleet_5c(f) == 0 {
                    if n == 4 && s.f_search(en.wrapping_neg(), w) == 0 {
                        4
                    } else {
                        2
                    }
                } else {
                    3
                }
            }
        };
        match mode {
            4 => move_to_planet(s, 51, 51, w),
            3 => {
                attack_planet(s, 51, 51, w);
            }
            _ => {
                // The weakest level of the human's fleets against fleet 51 (2: by strength), the nearest of it.
                let power = w.fleet_power(f);
                let me = w.fleet_xy(f);
                let (mut pick, mut level, mut best) = (0, 3, 0);
                for g in 1..=w.fleet_count() {
                    if w.fleet_owner(g) != en {
                        continue;
                    }
                    let l = if mode == 1 {
                        3
                    } else {
                        let theirs = w.fleet_power(g);
                        if power / 2 < theirs {
                            if power < theirs { 3 } else { 2 }
                        } else {
                            1
                        }
                    };
                    let d = distance(me, w.fleet_xy(g));
                    if l < level {
                        (level, pick, best) = (l, g, d);
                    } else if l == level && (pick == 0 || d < best) {
                        (pick, best) = (g, d);
                    }
                }
                if pick == 0 {
                    s.give_up(51, 53, w);
                } else {
                    move_fleet(w, f, 2, 2, pick as f32, 0.0);
                }
            }
        }
        if s.v_get(53) != 0 {
            let f = s.f_search(51, w);
            if f != 0 && stands(s, f.wrapping_neg(), w) {
                s.send_away(f.wrapping_neg(), w);
            }
        }
    }
    if s.flags[flag::SPECIAL] && s.special[0] == 0x10 && s.special[1] == 51 {
        s.win(0, true, w);
    }
    if s.battle_part(51) != 0 && battle_owner_part(s, en) {
        if battle_lost(s, 51) {
            w.event(StoryEvent::GlobalMorale(-200));
            message(w, 52, 0);
            s.start_story(51, false);
            s.win(0, true, w);
        } else if s.v_get(52) == 0 && n > 2 && s.b_search(en.wrapping_neg(), 1, w) != 0 {
            s.v_set(52, 1);
            message(w, 53, 0);
        }
    }
    if ground_war(s, 0, 51, w) {
        if !s.ground.attacker_won {
            if s.f_search(51, w) == 0 {
                message(w, 52, 0);
                s.start_story(51, false);
                s.win(0, true, w);
            } else {
                s.give_up(51, 53, w);
            }
        } else if s.v_get(51) == 0 {
            s.v_set(51, 1);
            if s.b_search(en.wrapping_neg(), 1, w) != 0 {
                message(w, 54, 0);
            }
        }
    }
    Ok(())
}

/// 51: the hunter's design (story design 511) at half price.
fn cheap_design(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    if s.starting_now() {
        let en = w.human();
        let d = w.story_design(en, 511, 3);
        w.event(StoryEvent::DesignPriceDiv { empire: en, design: d, div: 2 });
        w.event(StoryEvent::ScrollText(510));
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 6: rebels

fn rebels(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let p = s.b_search(61, 0, w);
        s.set_planet_owner(p, 10, w);
        s.auto_add_building(61, w);
        s.auto_add_tank(61, w);
        story_fleet(s, (10, 61, 0.1), p, &[(3, 2), (2, 4)], 0, w);
        s.sz_start(61, 250);
        s.sz_start(62, 150);
        message(w, 61, 0);
        param(w, 1, MessageText::Planet(p));
        s.v_set(61, 0);
    }
    if read(s, 61) {
        s.b_show(61, -1, w);
        let p = s.b_search(61, 0, w);
        s.add_info_line(61, InfoLine::Planet { planet: p, count: None }, -1, w);
        s.v_set(61, 2);
    }
    if s.daily() {
        let f = s.f_search(63, w);
        if f != 0 && stands(s, f.wrapping_neg(), w) && !attack_planet(s, 63, en.wrapping_neg(), w) {
            attack_fleet(s, 63, en.wrapping_neg(), w);
        }
        let f = s.f_search(69, w);
        if f != 0 && stands(s, f.wrapping_neg(), w) {
            let p = s.b_search(61, 0, w);
            if w.planet_owner(p) == 10 {
                attack_planet(s, 69, 61, w);
            } else {
                delete_fleet(s, f.wrapping_neg(), w);
            }
        }
        let v = s.v_get(61);
        let p = s.b_search(61, 0, w);
        if v == 1 {
            if w.planet_owner(p) == 10 {
                s.add_info_line(61, InfoLine::Planet { planet: p, count: None }, -1, w);
                s.v_set(61, 2);
            } else {
                s.remove_game_info(61, w);
                s.v_set(61, 1);
            }
        }
    }
    if s.flags[flag::GROUND_END] && s.ground.fleet_id == 69 && s.b_search(61, 0, w) == s.ground.planet {
        let f = s.f_search(69, w);
        if f != 0 {
            delete_fleet(s, f.wrapping_neg(), w);
        }
        s.win(0, true, w);
    } else if ground_war(s, 0, 61, w) && s.ground.attacker_won {
        if s.v_get(69) == 0 {
            message(w, 62, 0);
            money(w, 15_000);
            w.event(StoryEvent::RelationChange { a: en, b: 9, delta: 1000 });
        } else {
            message(w, 64, 0);
            money(w, 10_000);
            w.event(StoryEvent::RelationChange { a: en, b: 9, delta: 800 });
        }
        s.send_away(69, w);
        s.win(0, true, w);
    }
    if s.counter(62) {
        let p = s.b_search(61, 0, w);
        if w.planet_owner(p) == 10 {
            let p = s.b_search(61, 0, w);
            story_fleet(s, (10, 63, 0.09), p, &[(3, 1)], 0, w);
            attack_planet(s, 63, en.wrapping_neg(), w);
        }
    }
    if s.counter(61) {
        let p = s.b_search(61, 0, w);
        if w.planet_owner(p) != 10 {
            message(w, 62, 0);
            money(w, 15_000);
            w.event(StoryEvent::RelationChange { a: en, b: 9, delta: 1000 });
            s.win(0, true, w);
        } else {
            s.finished.add(6, 2);
            message(w, 63, 0);
            let (px, py) = w.planet_xy(p);
            let e = s.nearest_empire(px as i32, py as i32, &*w);
            s.v_set(68, e);
            if e == 0 {
                s.win(0, true, w);
            } else {
                let at = s.nearest_planet;
                story_fleet(s, (e, 69, 0.1), at, &[(3, 3), (2, 6)], 6, w);
                attack_planet(s, 69, 61, w);
                s.v_set(69, 1);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 7: troops wanted

/// The days to deliver the troops: [0, 1000, 780, 630, 500].
const TROOP_DAYS: [i32; 5] = [0, 1000, 780, 630, 500];
/// The price of a delivered unit.
const TROOP_PRICE: i32 = 15_000;

/// A `Tankok` record of the wanted units (size 19) on planet `p`.
fn wanted_unit(w: &dyn StoryWorld, k: i32, p: i32) -> Option<(i32, i32, i32)> {
    let (place, design, count) = w.tank(k);
    // (The exe tests the design's class and size first; the place decides alone, the reads have no effect.)
    if design == 0x7fff || place != p.wrapping_neg() {
        return None;
    }
    let owner = place_owner(place, w);
    (w.design_class(owner, design) == 2 && w.design_size(owner, design) == 19).then_some((place, design, count))
}

fn troops(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        s.sz_start(71, 20);
        message(w, 71, 0);
        s.l_set(71, false);
        s.l_set(72, false);
    }
    if s.counter(71) {
        let (mut pick, mut best) = (0, 0);
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) != en || w.planet_visibility(p) < 0 {
                continue;
            }
            let v = w.planet_production(p).1.wrapping_add(w.building_count(p));
            if pick == 0 || best < v {
                (pick, best) = (p, v);
            }
        }
        if pick == 0 {
            return Err(Abort);
        }
        s.b_add_id(pick, 73);
        s.v_set(71, 4);
        message(w, 72, 150);
        let n = s.v_get(71);
        param_int(w, 1, n);
        let n = s.v_get(71);
        param_int(w, 2, TROOP_PRICE.wrapping_mul(n));
        param(w, 3, MessageText::Planet(pick));
        let days = by_difficulty(&*w, TROOP_DAYS);
        param_int(w, 4, days);
    }
    if answered(s, 72, 2) {
        s.sz_start(72, by_difficulty(&*w, TROOP_DAYS));
        s.l_set(71, true);
    }
    if s.daily() && s.l_is_true(71) {
        s.remove_game_info(71, w);
        let days = s.sz_get(72);
        let p = s.b_search(73, 0, w);
        s.add_info_line(71, InfoLine::Planet { planet: p, count: Some(days) }, -1, w);
    }
    if answered(s, 72, 3) {
        s.win(0, true, w);
    }
    if promoted(s, 72) {
        s.win(0, true, w);
    }
    if s.daily() && s.l_is_true(71) {
        let p = s.b_search(73, 0, w);
        let mut sum = 0i32;
        for k in 1..=w.tank_count() {
            if let Some((_, _, count)) = wanted_unit(&*w, k, p) {
                sum = sum.wrapping_add(count);
            }
        }
        let _ = s.v_get(71);
        if s.v_get(71) <= sum {
            let mut left = s.v_get(71);
            let mut k = 1;
            while k <= w.tank_count() && left > 0 {
                if let Some((place, design, count)) = wanted_unit(&*w, k, p) {
                    let take = count.min(left);
                    w.event(StoryEvent::DeleteTanks { place, design, count: take });
                    left -= take;
                }
                k += 1;
            }
            s.sz_stop(72);
            let n = s.v_get(71);
            money(w, TROOP_PRICE.wrapping_mul(n));
            if s.l_is_true(72) {
                message(w, 75, 0);
                s.win(0, true, w);
            } else if s.planet_at_distance(80, 0, w)? == 0 {
                s.win(0, true, w);
            } else {
                message(w, 73, 0);
            }
            s.l_set(71, false);
            s.remove_game_info(71, w);
        }
    }
    if answered(s, 73, 2) {
        s.add_info_line(73, InfoLine::Blank, -1, w);
        s.sz_start(73, 360);
        s.set_adat_i32(0xac, 1);
    }
    if answered(s, 73, 3) {
        s.win(0, true, w);
    }
    if s.counter(72) {
        message(w, 74, 0);
        s.win(0, true, w);
    }
    if s.counter(73) {
        s.set_adat_i32(0xac, 0);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 8: a design for sale

fn for_sale(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        message(w, 81, 0);
        let d = w.story_design(en, 81, 3);
        let k = w.trade_offer_count();
        w.event(StoryEvent::DesignForSale { empire: en, design: d });
        s.v_set(82, k);
        s.v_set(89, 0);
    }
    if read(s, 81) {
        s.add_info_line(81, InfoLine::Blank, -1, w);
    }
    if s.daily() {
        let offered = (0..w.trade_offer_count()).any(|k| {
            let (class, key) = w.trade_offer(k);
            class > 0 && key == 81
        });
        if !offered {
            message(w, 82, 0);
            s.win(0, true, w);
        }
    }
    if s.special_is(0x13, en, 0, 0) && s.v_get(89) == 0 && w.design_key(en, s.special[2]) == 81 {
        message(w, 83, 0);
        s.v_set(89, 1);
    }
    Ok(())
}

// ---------------------------------------------------------------- 9: a trade treaty

fn treaty(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let e = random_empire(w, 0, 0, 0)?;
        if !w.knows(en, e) || w.trade_agreement(en, e) {
            return Err(Abort);
        }
        s.v_set(91, 1000);
        s.v_set(92, e);
        message(w, 91, 120);
        let e = s.v_get(92);
        param(w, 1, MessageText::Empire(e));
        let price = s.v_get(91);
        param_int(w, 2, price);
    }
    if s.counter(91) {
        let e = s.v_get(92);
        if !w.empire_alive(e) {
            return Err(Abort);
        }
        message(w, 92, 120);
        let e = s.v_get(92);
        param(w, 1, MessageText::Empire(e));
        let price = s.v_get(91);
        param_int(w, 2, price);
    }
    if answered(s, 91, 2) || answered(s, 92, 2) {
        let price = s.v_get(91);
        money(w, price.wrapping_neg());
        // The price taken for the empire: the original's slip.
        let x = s.v_get(91);
        w.event(StoryEvent::TradeAgreement { a: en, b: x });
        let p = s.b_search(x.wrapping_neg(), 0, w);
        w.event(StoryEvent::PlaceBuilding { planet: p, kind: 0x30 });
        w.event(StoryEvent::RelationChange { a: en, b: x, delta: 2000 });
        s.win(0, true, w);
    }
    if answered(s, 91, 3) {
        s.sz_start(91, 300);
    }
    if answered(s, 91, 4) || answered(s, 92, 3) || answered(s, 92, 4) {
        s.win(0, true, w);
    }
    if promoted(s, 91) {
        s.win(0, true, w);
    }
    if promoted(s, 92) {
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 10, 11: a colony

/// The population the colony needs (thousands): [0, 5, 7, 8, 10].
const COLONY_SIZE: [i32; 5] = [0, 5, 7, 8, 10];

fn colony(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        if w.planets_of(en) < 3 {
            s.sz_start(109, by_difficulty(&*w, [0, 2000, 1600, 1200, 1000]));
            s.v_set(101, 0);
            s.v_set(102, 0);
            message(w, 111, 0);
            s.v_set(103, 0);
        } else {
            s.win(0, true, w);
        }
    }
    if read(s, 111) {
        s.v_set(103, 1);
    }
    if s.counter(109) {
        message(w, 113, 0);
        s.win(0, true, w);
    }
    if s.special_is(10, en, 0, 0) && s.v_get(101) == 0 {
        s.sz_stop(109);
        s.b_add_id(s.special[2], 101);
        s.sz_start(101, 1080);
        message(w, 101, 0);
        let p = s.b_search(101, 0, w);
        param(w, 1, MessageText::Planet(p));
        let size = by_difficulty(&*w, COLONY_SIZE);
        param_int(w, 2, size);
        s.b_show(101, -1, w);
        s.v_set(101, 1);
        s.remove_game_info(103, w);
    }
    let was_read = read(s, 101);
    if was_read {
        s.v_set(102, 1);
    }
    if was_read || (s.daily() && s.v_get(102) != 0) {
        s.remove_game_info(101, w);
        s.remove_game_info(102, w);
        s.remove_game_info(105, w);
        let p = s.b_search(101, 0, w);
        if random_building(p, 1, w) == 0 {
            s.add_info_line(105, InfoLine::Planet { planet: p, count: None }, -1, w);
        } else {
            let d = s.sz_get(101);
            if d >= 31 {
                s.add_info_line(101, InfoLine::Countdown { count: (d + 29) / 30, planet: p }, -1, w);
            } else if d >= 2 {
                s.add_info_line(102, InfoLine::Countdown { count: d, planet: p }, -1, w);
            }
        }
    }
    if (s.daily() && s.sz_get(101) != 0) || s.counter(101) {
        let p = s.b_search(101, 0, w);
        if w.planet_value(p) < by_difficulty(&*w, COLONY_SIZE).wrapping_mul(1000) {
            if s.counter(101) {
                message(w, 104, 0);
                s.win(0, true, w);
            }
        } else {
            message(w, 102, 0);
            let d = s.sz_get(101);
            let reward = (d / 90 + 4).wrapping_mul(5000);
            money(w, reward);
            param_int(w, 1, reward / 1000);
            s.win(0, true, w);
        }
    }
    if s.daily() && s.running(10) {
        let colonised = s.v_get(101) != 0;
        if colonised && {
            let p = s.b_search(101, 0, w);
            w.planet_owner(p) != en
        } {
            if s.v_get(102) == 0 {
                w.event(StoryEvent::RemoveMessage(101));
            }
            s.win(0, true, w);
        } else if s.v_get(103) != 0 {
            if s.v_get(101) != 0 {
                s.remove_game_info(103, w);
                s.remove_game_info(104, w);
            } else {
                let ship = (1..=w.ship_count()).any(|k| {
                    let (f, d) = w.ship(k);
                    d != 0x7fff && w.fleet_owner(f) == en && w.design_colony(en, d)
                });
                if ship {
                    s.remove_game_info(103, w);
                    s.add_info_line(104, InfoLine::Blank, -1, w);
                } else {
                    s.remove_game_info(104, w);
                    s.add_info_line(103, InfoLine::Blank, -1, w);
                }
            }
        }
    }
    Ok(())
}

fn colony_in_time(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        message(w, 111, 90);
    }
    if read(s, 111) {
        s.sz_start(111, 920);
        s.v_set(111, 1);
    }
    if promoted(s, 111) {
        s.win(0, true, w);
    }
    if s.special_is(10, en, 0, 0) && s.v_get(111) != 0 {
        message(w, 112, 0);
        w.event(StoryEvent::GlobalMorale(-300));
        s.win(0, true, w);
    }
    if s.counter(111) {
        message(w, 113, 0);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 12: a research grant

fn grant(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    let mut pick = false;
    if s.starting_now() {
        s.v_set(121, 0);
        if w.researching(en) == 0 {
            pick = true;
        } else {
            s.v_set(129, 1);
            s.sz_start(129, 180);
        }
    }
    if s.counter(129) && s.v_get(129) != 0 {
        s.v_set(129, 0);
        pick = true;
    }
    if s.special_is(0xd, en, 0, 0) && s.v_get(129) != 0 {
        s.v_set(129, 0);
        pick = true;
    }
    if pick {
        let (mut t, mut best) = (0, 100);
        for k in 1..=192 {
            if can_give(&*w, k, en) {
                let p = match w.tech_priority(k) {
                    0 => 20,
                    p => p,
                };
                if t == 0 || p < best {
                    (t, best) = (k, p);
                }
            }
        }
        if t < 1 {
            s.win(0, true, w);
        } else {
            let (cost, time) = w.tech_research(t);
            s.v_set(121, t);
            s.v_set(122, time);
            s.v_set(122, cost);
            message(w, 121, 0);
            param(w, 1, MessageText::Tech(t));
            w.event(StoryEvent::TechCheaper(t));
        }
    }
    if s.special_is(0xd, en, 0, 0) && s.special[2] == s.v_get(121) {
        message(w, 122, 0);
        s.win(0, true, w);
    }
    if s.daily() && s.v_get(121) != 0 {
        let t = s.v_get(121);
        if w.tech_status(t, en) == 5 {
            s.win(0, true, w);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 19: a saboteur

fn saboteur(s: &mut Story, w: &mut dyn StoryWorld, all: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    let (mut mode, mut caught) = (0, false);
    if s.starting_now() {
        if !s.counter_intelligence(20, w) {
            mode = 1;
            s.sz_start(196, 10);
        } else {
            caught = true;
            s.win(0, true, w);
        }
    }
    if s.counter(196) {
        message(w, 191, 0);
        w.event(StoryEvent::Anim(191));
        let p = s.b_search(191, 0, w);
        param(w, 1, MessageText::Planet(p));
        s.sz_start(191, 300);
        s.sz_start(195, 80);
        s.sz_start(197, 140);
        s.sz_start(198, 200);
    }
    if s.counter(195) || s.counter(198) || s.counter(197) {
        if !s.counter_intelligence(40, w) {
            mode = 2;
        } else {
            caught = true;
            s.win(0, true, w);
        }
    }
    if s.counter(191) {
        if s.counter_intelligence(40, w) {
            message(w, 193, 0);
            let race = if s.counter_spy == 0 { 1 } else { w.spy_record(en, s.counter_spy).race };
            w.event(StoryEvent::MessageRace(race));
        }
        s.win(0, true, w);
    }
    if mode != 0 {
        let (p, slot) = sabotage_target(en, mode == 1, w);
        if p == 0 {
            s.win(0, true, w);
        } else {
            s.b_add_id(p, 191);
            let (kind, damage, _) = w.building(p, slot);
            let hp = w.building_hp(kind);
            let r = rnd(w, 40, 90);
            let mut damage = damage.wrapping_add(hp.wrapping_mul(r) / 100);
            if hp <= damage {
                damage = hp - 1;
            }
            w.event(StoryEvent::DamageBuilding { planet: p, slot, damage });
            let tech = w.building_tech(kind);
            w.event(StoryEvent::ScrollSabotage { tech, planet: p });
        }
    }
    if caught {
        let e = random_empire(w, 5, 1, 0)?;
        let k = (1..=w.spy_count(e)).filter(|&k| w.spy_record(e, k).state > 0).last().unwrap_or(0);
        if k != 0 {
            let spy = s.counter_spy;
            w.event(StoryEvent::SpyCaught { spy, empire: e, enemy: k });
            // KemJelentesAdd's Story_KemEsemeny: a nested top-level check, with every script.
            let news = KemNews { news: 3, spy, target: e, tech: k, ..KemNews::default() };
            s.spy_news(news, all, w);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 20: a tech to steal

/// `KemEsemeny` with flag 15 (`KemkedesTipus` 3): spy news of a successful mission `mission`.
fn spy_success(s: &Story, mission: i32) -> bool {
    s.flags[flag::SPY] && s.spy_kind == 3 && s.kem.news == 1 && s.kem.success && s.kem.mission == mission
}

fn steal(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let mut e = (1..9).find(|&e| e != en && w.knows(en, e) && w.empire_alive(e)).unwrap_or(9);
        if e >= 9 {
            let (mut pick, mut best) = (0, 1_000_000);
            for p in 1..=w.planet_count() {
                let o = w.planet_owner(p);
                if o != 0 && o != en && o != 5 {
                    let d = human_distance(p, &*w);
                    if d < best {
                        (pick, best) = (p, d);
                    }
                }
            }
            if pick == 0 {
                return Err(Abort);
            }
            s.b_add_id(pick, 201);
            s.b_show(201, -1, w);
            e = w.planet_owner(pick);
            if !w.knows(en, e) {
                w.event(StoryEvent::DiscoverEmpire(e));
            }
            message(w, 201, 0);
            param(w, 1, MessageText::Empire(e));
            param(w, 2, MessageText::Planet(pick));
        }
        s.sz_start(201, 20);
        s.v_set(201, e);
    }
    if s.counter(201) {
        message(w, 202, 0);
        let e = s.v_get(201);
        param(w, 1, MessageText::Empire(e));
        s.v_set(202, 1);
    }
    let mut checked = false;
    if s.flags[flag::SPY] {
        if spy_success(s, 4) && s.kem.target == s.v_get(201) && 0 < s.v_get(202) && s.v_get(202) < 3 {
            let (mut pick, mut n) = (0, 0);
            for t in s.kem.techs {
                let status = w.tech_status(t, en);
                if t != 0 && status != 0 && status < 5 {
                    n += 1;
                    if rnd(w, 1, n) < 2 {
                        pick = t;
                    }
                }
            }
            if pick == 0 {
                if s.v_get(202) == 1 {
                    message(w, 206, 0);
                    s.v_set(202, 2);
                } else {
                    message(w, 207, 0);
                    s.win(0, true, w);
                }
            } else {
                message(w, 203, 0);
                param(w, 1, MessageText::Tech(pick));
                s.v_set(203, pick);
                s.v_set(202, 3);
            }
        }
        if spy_success(s, 8) && s.kem.target == s.v_get(201) && s.v_get(202) == 3 && s.kem.tech == s.v_get(203) {
            message(w, 204, 0);
            let race = w.spy_record(en, s.kem.spy).race;
            w.event(StoryEvent::MessageRace(race));
            s.win(0, true, w);
            checked = true;
        }
    }
    if !checked && s.v_get(202) == 3 {
        let t = s.v_get(203);
        if w.tech_status(t, en) == 5 {
            message(w, 205, 0);
            s.win(0, true, w);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 21: a spy's target

fn spy_target(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let (mut e, mut sum) = (0, 0);
        for k in 1..9 {
            if k == en || k == 5 || !w.empire_alive(k) {
                continue;
            }
            let weight = if w.knows(en, k) { 10 } else { 1 };
            sum += weight;
            if rnd(w, 1, sum) <= weight {
                e = k;
            }
        }
        if e == 0 {
            return Err(Abort);
        }
        let (mut slot, mut sum) = (0, 0);
        for k in 1..w.spy_count(en) {
            let spy = w.spy_record(en, k);
            if spy.state == 0 {
                continue;
            }
            if spy.target != 0 {
                (slot, e) = (k, spy.target);
                break;
            }
            let weight = if spy.state == 1 { 10 } else { 1 };
            sum += weight;
            if rnd(w, 1, sum) <= weight {
                slot = k;
            }
        }
        if slot == 0 {
            return Err(Abort);
        }
        w.event(StoryEvent::SpyTarget { slot, empire: e });
        s.v_set(211, slot);
        message(w, 211, 0);
        s.sz_start(211, 360);
    }
    if s.counter(211) {
        message(w, 213, 0);
        w.event(StoryEvent::GlobalMorale(40));
        s.win(0, true, w);
    }
    let gone = s.daily() && {
        let k = s.v_get(211);
        w.spy_record(en, k).state == 0
    };
    let died = s.flags[flag::SPY] && s.spy_kind == 3 && s.kem.news == 1 && s.kem.died == 1 && {
        let k = s.v_get(211);
        k == 0 || s.kem.spy == k
    };
    if !died {
        if gone {
            message(w, 215, 0);
            s.win(0, true, w);
        }
    } else if s.kem.spy == s.v_get(211) {
        message(w, 214, 0);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 22: a far planet

fn far_planet(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    let second = w.campaign_type() == 2;
    if s.starting_now() {
        let n = s.v_get(500);
        if s.f_search(en.wrapping_neg(), w) == 0 || n == 0 {
            return Err(Abort);
        }
        let (mut pick, mut best, mut which) = (0, 1_000_000, 0);
        for k in 0..n {
            let p = s.b_search(500 + k, 0, w);
            if p == 0 {
                return Err(Abort);
            }
            let d = human_distance(p, &*w);
            if w.planet_visibility(p) == 0 && d < best {
                (pick, best, which) = (p, d, k);
            }
        }
        if pick == 0 {
            return Err(Abort);
        }
        s.b_add_id(pick, 221);
        message(w, if second { 9221 } else { 221 }, 0);
        param(w, 1, MessageText::Star(which + 1));
    }
    let arrived = s.flags[flag::ARRIVED]
        && s.arrival.target_type == 1
        && s.b_search(221, 0, w) == s.arrival.target
        && w.fleet_owner(s.arrival.fleet) == en;
    let colonised = s.special_is(10, en, 0, 0) && s.special[2] == s.b_search(221, 0, w);
    if colonised || arrived {
        let p = s.b_search(221, 0, w);
        let o = w.planet_owner(p);
        if o == 0 || o == en {
            message(w, if second { 9223 } else { 223 }, 0);
            money(w, 20_000);
            param(w, 1, MessageText::Planet(p));
        }
        s.win(0, true, w);
    } else if s.special_is(10, 0, 0, 0) && s.special[2] == s.b_search(221, 0, w) && s.special[1] != en {
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 23: a fugitive

/// The fugitive flees from `q`: 30 units away at about the opposite angle (±50°, stream 5).
fn flee(f: i32, q: (f32, f32), w: &mut dyn StoryWorld) {
    let (fx, fy) = w.fleet_xy(f);
    let (mut dx, dy) = (fx - q.0, fy - q.1);
    let mut d = ((dy * dy + dx * dx) as f64).sqrt() as f32;
    if (d as f64) < 0.1 {
        dx = 0.05;
        d = 0.1;
    }
    if dx > d {
        dx = d;
    }
    if -d > dx {
        dx = -d;
    }
    let mut a = ((dx / d) as f64).acos() as f32;
    if dy < 0.0 {
        a = (std::f64::consts::TAU - a as f64) as f32;
    }
    let r = rnd(w, -50, 50);
    let a = ((r as f64 * std::f64::consts::PI) / 180.0 + a as f64) as f32;
    let sin = (a as f64).sin() as f32;
    let cos = (a as f64).cos() as f32;
    move_fleet(w, f, 1, 0, (fx + dx) + cos * 30.0, (fy + dy) + sin * 30.0);
}

fn fugitive(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let speed = by_difficulty(&*w, [0, 6, 7, 7, 8]) as f32;
        let f = s.f_new_ex("", 9, 231, speed, 1, 0, w);
        s.f_add_ship(231, 3, 2, w);
        message(w, 231, 0);
        s.sz_start(231, (w.difficulty() * 5 + 20) * 8);
        let b = s.last_fn[last_fn::B];
        move_fleet(w, f, 1, 1, b as f32, 0.0);
        s.v_set(232, 1);
    }
    if s.daily() && s.v_get(232) != 0 {
        let f = s.f_search(231, w);
        if f == 0 {
            return Err(Abort);
        }
        let (fx, fy) = w.fleet_xy(f);
        let near = |(x, y): (f32, f32)| {
            let (dx, dy) = (x - fx, y - fy);
            (dy * dy + dx * dx) as i32
        };
        let (mut planet, mut planet_d) = (0, 1_000_000);
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) == en && w.planet_visibility(p) >= 0 {
                let d = near(w.planet_xy(p));
                if d < planet_d {
                    (planet, planet_d) = (p, d);
                }
            }
        }
        if planet == 0 {
            return Err(Abort);
        }
        let (mut fleet, mut fleet_d) = (0, 1_000_000);
        for g in 1..=w.fleet_count() {
            if w.fleet_owner(g) == en {
                let d = near(w.fleet_xy(g));
                if d < fleet_d {
                    (fleet, fleet_d) = (g, d);
                }
            }
        }
        if fleet_d <= 144 {
            let q = w.fleet_xy(fleet);
            flee(f, q, w);
        } else if planet_d < 100 {
            let q = w.planet_xy(planet);
            flee(f, q, w);
        } else {
            move_fleet(w, f, 1, 1, planet as f32, 0.0);
        }
    }
    if s.counter(231) {
        let f = s.f_search(231, w);
        if f == 0 {
            return Err(Abort);
        }
        let at = s.b_search(en.wrapping_neg(), 0, w);
        let g = s.f_new_ex("", 9, 232, 0.0, 2, at, w);
        s.f_add_ship(232, 3, 3, w);
        s.f_add_ship(232, 2, 5, w);
        move_fleet(w, g, 1, 2, f as f32, 0.0);
        s.v_set(231, 1);
    }
    if s.arrived_fleet(232, 231) {
        let g = s.f_search(232, w);
        if g != 0 {
            w.event(StoryEvent::SetFleetId { fleet: g, id: 231 });
        }
        let f = s.f_search(231, w);
        if f == 0 {
            return Err(Abort);
        }
        w.event(StoryEvent::SetFleetSpeed { fleet: f, speed: 0.0 });
        w.event(StoryEvent::RenameFleet(f));
        message(w, 232, 0);
        s.send_away(231, w);
        s.v_set(232, 0);
    }
    if s.battle_part(232) != 0 && s.f_search(232, w) == 0 {
        message(w, 236, 0);
        s.send_away(231, w);
        s.v_set(232, 0);
    }
    if s.battle_part(231) != 0 && s.f_search(231, w) == 0 {
        if s.v_get(231) == 0 {
            message(w, 234, 0);
            money(w, 25_000);
        } else {
            message(w, 233, 0);
            money(w, 14_000);
        }
        s.win(0, true, w);
    }
    if s.flags[flag::SPECIAL] && s.special[0] == 0x10 && s.special[1] == 231 {
        message(w, 235, 0);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 24: an escort

/// The escorted fleet's `+0x52` bits: always visible, friendly, a custom name.
const ESCORTED: u16 = 0x100 | 0x200 | 0x800;

fn escort(s: &mut Story, w: &mut dyn StoryWorld, _: &dyn Script) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        s.sz_start(241, 10);
    }
    if s.counter(241) {
        let (mut pick, mut best) = (0, 0);
        for g in 1..=w.fleet_count() {
            let id = w.fleet_id(g);
            if w.fleet_owner(g) == 9 && (10_000..=11_000).contains(&id) && w.fleet_state(g) & 1 == 0 {
                let d = fleet_distance(g, &*w);
                if pick == 0 || d < best {
                    (pick, best) = (g, d);
                }
            }
        }
        if pick == 0 || best > 79 {
            s.sz_start(241, 10);
        } else {
            s.v_set(241, w.fleet_id(pick));
            s.v_set(242, w.fleet_target(pick) as i32);
            w.event(StoryEvent::SetFleetId { fleet: pick, id: 241 });
            w.event(StoryEvent::FleetFlags { fleet: pick, set: ESCORTED, clear: 0 });
            w.event(StoryEvent::StopFleet(pick));
            message(w, 241, 0);
            param(w, 1, MessageText::Fleet(pick));
            s.sz_start(242, 400);
        }
    }
    let met = s.arrived_fleet(0, 241) && w.fleet_owner(s.arrival.fleet) == en;
    if met || s.counter(242) {
        let f = s.f_search(241, w);
        if f == 0 {
            return Err(Abort);
        }
        let id = s.v_get(241) as i16 as i32;
        w.event(StoryEvent::SetFleetId { fleet: f, id });
        w.event(StoryEvent::FleetFlags { fleet: f, set: 0, clear: ESCORTED });
        let target = s.v_get(242);
        move_fleet(w, f, 1, 1, target as f32, 0.0);
        w.event(StoryEvent::StopFleet(s.arrival.fleet));
        if s.counter(242) {
            message(w, 243, 0);
            param(w, 1, MessageText::Fleet(f));
            s.win(0, true, w);
        } else {
            s.v_set(249, 1);
            s.sz_stop(242);
            message(w, 242, 0);
            money(w, 4000);
            param(w, 1, MessageText::Fleet(f));
            s.win(0, true, w);
        }
    }
    Ok(())
}
