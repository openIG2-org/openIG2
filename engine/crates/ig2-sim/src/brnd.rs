//! The campaign's random stories, part 1: `st_BRnd_include` (exe 0x5c9f20, ELF 0x768d50; `st_brnd.cpp`), the
//! first script `Story_Check` runs. Spec: `re/rules/campaign.md` §8.11.
//!
//! Stories 220..=239 are the random stories story 400 draws (`RndStoryk`): mercenaries for hire (220..=229), a
//! fleet in quarantine (230, with 2301), a planet to blow up (231, with 2311), planets revealed (232), strikes
//! (233), a tax (234, 235), a refugee planet (236), every fleet shown (237), a switch for 0x38 (238) and a
//! wormhole (239). What they do to the rest of the game goes out as [`StoryEvent`]s and [`StoryWorld`] calls
//! (designs made and deleted, fleets, money, morale, techs, planet values and views).

use crate::multi::empire_centre;
use crate::story::{flag, Abort, InfoLine, MessageText, Script, Story, StoryEvent, StoryWorld, DESIGN, STREAM};

/// `st_BRnd_include`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BRndStories;

type Block = fn(&mut Story, &mut dyn StoryWorld) -> Result<(), Abort>;

impl Script for BRndStories {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        // `Story_Check` runs the campaign's scripts in SP only.
        if w.multiplayer() {
            return;
        }
        delete_story_design(s, w);
        let stories: [(i32, Block); 22] = [
            (220, |s, w| mercenary(s, w, &MERCENARIES[0])),
            (221, |s, w| mercenary(s, w, &MERCENARIES[1])),
            (222, |s, w| mercenary(s, w, &MERCENARIES[2])),
            (223, |s, w| mercenary(s, w, &MERCENARIES[3])),
            (224, |s, w| mercenary(s, w, &MERCENARIES[4])),
            (225, |_, _| Ok(())),
            (226, |s, w| mercenary(s, w, &MERCENARIES[5])),
            (227, |s, w| mercenary(s, w, &MERCENARIES[6])),
            (228, |s, w| mercenary(s, w, &MERCENARIES[7])),
            (229, |s, w| mercenary(s, w, &MERCENARIES[8])),
            (230, quarantine),
            (2301, quarantine_end),
            (231, blow_up),
            (2311, blow_up_end),
            (232, reveal),
            (233, strikes),
            (234, |s, w| tax(s, w, &TAXES[0])),
            (235, |s, w| tax(s, w, &TAXES[1])),
            (236, refugees),
            (237, all_fleets),
            (238, switch_38),
            (239, wormhole),
        ];
        for (id, story) in stories {
            // The catch handler of each story block: `WIN(0, 1)`, then the next story.
            if s.running(id) && story(s, w).is_err() {
                s.win(0, true, w);
            }
        }
    }
}

// ---------------------------------------------------------------- helpers

fn rnd(w: &mut dyn StoryWorld, lo: i32, hi: i32) -> i32 {
    w.rng().range(STREAM, lo, hi)
}

/// `st_Message(b, r)`: the human answered part `r` of message `b`.
fn answered(s: &Story, b: i32, r: i32) -> bool {
    s.flags[flag::MESSAGE] && s.message == (b, r)
}

/// `st_Elevul(b)`: message `b` was promoted (flag 7).
fn promoted(s: &Story, b: i32) -> bool {
    s.flags[flag::PROMOTED] && s.message.0 == b
}

fn message(w: &mut dyn StoryWorld, id: i32, arg: i32) {
    w.event(StoryEvent::NewMessage { id, arg });
}

fn param(w: &mut dyn StoryWorld, slot: i32, text: MessageText) {
    w.event(StoryEvent::MessageParam { slot, text });
}

fn add_adat(s: &mut Story, off: usize, x: i32) {
    let v = s.adat_i32(off).wrapping_add(x);
    s.set_adat_i32(off, v);
}

/// `st_tav` on truncated coordinates: `(int)(f32) sqrt((f64)(f32)(dx² + dy²))`.
pub fn distance(a: (f32, f32), b: (f32, f32)) -> i32 {
    let dx = (a.0 as i32).wrapping_sub(b.0 as i32);
    let dy = (a.1 as i32).wrapping_sub(b.1 as i32);
    let sum = dy.wrapping_mul(dy).wrapping_add(dx.wrapping_mul(dx));
    (sum as f32 as f64).sqrt() as f32 as i32
}

/// `st_GetOsszFlottaEro()` (exe 0x5eaaf0): the attack (`+0x2c`) of the human's ships.
pub fn fleet_strength(w: &dyn StoryWorld) -> i32 {
    let en = w.human();
    (1..=w.ship_count())
        .map(|k| w.ship(k))
        .filter(|&(f, d)| d != 0x7fff && w.fleet_owner(f) == en)
        .fold(0i32, |sum, (_, d)| sum.wrapping_add(w.design_attack(en, d)))
}

/// `st_hSearch(e, key)` (exe 0x6052a0): the fleet of empire `e`'s first ship of design `key` (a story design key
/// above 40, matched against the designs from slot 40 on), else 0.
pub fn ship_fleet(w: &dyn StoryWorld, e: i32, key: i32) -> i32 {
    let d = if key > 40 {
        match (40..=w.design_count(e)).find(|&d| w.design_key(e, d) == key) {
            Some(d) => d,
            None => return 0,
        }
    } else {
        key
    };
    (1..=w.ship_count()).map(|k| w.ship(k)).find(|&(f, dd)| dd == d && w.fleet_owner(f) == e).map_or(0, |(f, _)| f)
}

/// `st_SpWarStartID(f)` (exe 0x602e30): a space battle starts with fleet slot `f` (0: any).
fn battle_with(s: &Story, w: &dyn StoryWorld, f: i32) -> bool {
    let (a, b) = w.battle_fleets();
    s.flags[flag::SPACE_START] && (f == 0 || a == f || b == f)
}

/// `st_fGetNearest(x, y, owner)` (exe 0x604970): the fleet of `owner` (0: any live one) nearest to `(x, y)`
/// (f32, against the last pick's truncated value; the first of equals), else 0.
pub fn nearest_fleet(x: i32, y: i32, owner: i32, w: &dyn StoryWorld) -> i32 {
    let (mut best, mut best_d) = (0, 0);
    for f in 1..=w.fleet_count() {
        let o = w.fleet_owner(f);
        if !((owner != 0 && owner == o) || (owner == 0 && o != 0xff)) {
            continue;
        }
        let (fx, fy) = w.fleet_xy(f);
        let (dx, dy) = (x as f32 - fx, y as f32 - fy);
        let d = dy * dy + dx * dx;
        if best == 0 || d < best_d as f32 {
            best = f;
            best_d = d as i32;
        }
    }
    best
}

/// `GetRndBirodalom(a, b, c)` (exe 0x5c27c0): a random live empire 1..=8 other than `a`, `b`, `c` and the
/// human, one the human knows if there is one in ten steps; aborts when there is none.
pub fn random_empire(w: &mut dyn StoryWorld, a: i32, b: i32, c: i32) -> Result<i32, Abort> {
    let en = w.human();
    let mut e = rnd(w, 1, 8);
    let unfit = |w: &dyn StoryWorld, e: i32, known: bool| {
        e == a || e == b || e == c || e == en || !w.empire_alive(e) || (known && !w.knows(en, e))
    };
    for known in [true, false] {
        let mut tries = 10;
        while unfit(&*w, e, known) {
            tries -= 1;
            if tries == 0 {
                break;
            }
            e = e % 8 + 1;
        }
        if tries != 0 {
            return Ok(e);
        }
    }
    Err(Abort)
}

/// The record of the designs 230 and 231 make (spec §8.11): `size` at `+0x44`/`+0x48`, `key` at `+0x80`.
pub fn story_design_record(en: i32, size: i32, key: i32) -> [u8; DESIGN] {
    let mut r = [0u8; DESIGN];
    r[0x16] = 1;
    r[0x18] = 1;
    r[0x19] = 1;
    r[0x1a] = 1;
    r[0x20..0x24].copy_from_slice(&150_000i32.to_le_bytes());
    r[0x24..0x28].copy_from_slice(&300i32.to_le_bytes());
    r[0x3c..0x40].copy_from_slice(&0.1f32.to_le_bytes());
    r[0x40..0x42].copy_from_slice(&(en as i16).to_le_bytes());
    r[0x44..0x48].copy_from_slice(&size.to_le_bytes());
    r[0x48..0x4c].copy_from_slice(&size.to_le_bytes());
    r[0x80..0x84].copy_from_slice(&key.to_le_bytes());
    r
}

/// `st_TalalmanyAdhato(t, e)`: tech `t` can be given (status 1..=3, every prerequisite done).
pub fn can_give(w: &dyn StoryWorld, t: i32, e: i32) -> bool {
    let status = w.tech_status(t, e);
    (1..4).contains(&status) && w.tech_prereqs(t).iter().all(|&p| p == 0 || w.tech_status(p, e) == 5)
}

/// `st_GetRandomTalalmany(e, group)` (exe 0x5eab60): of the groups of 16 techs (one group with `group != 0`),
/// each group's first tech `e` can be given, one at random (stream 5, a draw per group); 0 when none.
pub fn random_tech(w: &mut dyn StoryWorld, e: i32, group: i32) -> i32 {
    let (mut n, mut pick) = (0, 0);
    for g in 0..11 {
        let Some(t) = (1..=16).map(|j| 16 * g + j).find(|&t| can_give(&*w, t, e)) else { continue };
        if group == 0 || g == group {
            let r = rnd(w, 0, n);
            n += 1;
            if r == 0 {
                pick = t;
            }
        }
    }
    pick
}

impl Story {
    /// `st_RemoveFlottaAzonosito(id)` (exe 0x733a70): the first live fleet with story id `id` loses it.
    pub fn remove_fleet_id(&mut self, id: i32, w: &mut dyn StoryWorld) {
        let f = self.f_search(id, w);
        if f != 0 {
            w.event(StoryEvent::SetFleetId { fleet: f, id: 0 });
        }
    }

    /// `st_fEliminate(id)` (exe 0x604750): fleet `id` (`< 0`: the slot `−id`) is sent 300 off, away from the
    /// human's planets (stream 5), unless it already was and still has ships. `StoryError` = no fleet.
    pub fn send_away(&mut self, id: i32, w: &mut dyn StoryWorld) {
        let f = if id < 0 { id.wrapping_neg() } else { self.f_search(id, w) };
        if f < 1 {
            self.error = 1;
            return;
        }
        if !(w.fleet_state(f) & 2 != 0 && w.fleet_command(f) != 0) {
            w.event(StoryEvent::SendAway(f));
            let mut x = rnd(w, 0, 1) * 600 - 300;
            let mut y = rnd(w, -300, 300);
            if rnd(w, 0, 1) != 0 {
                (x, y) = (y, x);
            }
            let en = w.human();
            let (mut cx, mut cy, mut n) = (0i32, 0i32, 0);
            for p in 1..=w.planet_count() {
                if w.planet_owner(p) == en && w.planet_visibility(p) >= 0 {
                    let (px, py) = w.planet_xy(p);
                    n += 1;
                    cx = (cx as f32 + px) as i32;
                    cy = (cy as f32 + py) as i32;
                }
            }
            if n == 0 {
                (cx, cy) = (0, 0);
            } else {
                (cx, cy) = (cx / n, cy / n);
            }
            let (fx, fy) = w.fleet_xy(f);
            if (cx as f32 - fx < 50.0) != (x > -1) {
                x = x.wrapping_neg();
            }
            if (cy as f32 - fy < 50.0) != (y > -1) {
                y = y.wrapping_neg();
            }
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 0, x: x as f32, y: y as f32 });
        }
        self.error = 0;
    }

    /// `st_bGet3BestProd()` (exe 0x603350): the human's three visible planets with the most production
    /// (`+0x70 + +0x74`); `None` with fewer than three planets or a slot never filled. The chosen planets are a
    /// static: a slot not refilled keeps an earlier call's planet.
    pub fn best_three(&mut self, w: &dyn StoryWorld) -> Option<[i32; 3]> {
        let en = w.human();
        if w.planets_of(en) < 3 {
            return None;
        }
        let mut val = [0i32; 3];
        for p in 1..w.planet_count() {
            if w.planet_owner(p) != en || w.planet_visibility(p) < 0 {
                continue;
            }
            let (a, b) = w.planet_production(p);
            let v = a.wrapping_add(1).wrapping_add(b);
            let mut k = 0;
            if val[1] == 0 || (val[0] != 0 && val[1] < val[0]) {
                k = 1;
            }
            if val[2] == 0 || (val[k] != 0 && val[2] < val[k]) {
                k = 2;
            }
            if val[k] == 0 || val[k] < v {
                val[k] = v;
                self.best_prod[k] = p;
            }
        }
        (!self.best_prod.contains(&0)).then_some(self.best_prod)
    }

    /// `st_GetUnknownPlanet()` (exe 0x5eaea0): up to 101 random free planets (thrown away; aborts without
    /// one), then `st_bGetNearestUnknown()` (0x603a60): of the planets 11001..=11015 the human doesn't know the
    /// nearest to its planets' centre, shown to everybody; aborts when there is none.
    pub fn unknown_planet(&mut self, w: &mut dyn StoryWorld) -> Result<i32, Abort> {
        for _ in 0..101 {
            let p = self.b_search_try(0, 0, w)?;
            if w.planet_visibility(p) == 0 {
                break;
            }
        }
        let (cx, cy) = empire_centre(w.human(), &*w);
        let (mut pick, mut best) = (0, 0);
        for k in 1..=15 {
            let p = self.b_search(11000 + k, 1, w);
            if p == 0 || w.planet_visibility(p) >= 0 {
                continue;
            }
            let (px, py) = w.planet_xy(p);
            let (dx, dy) = (cx - px, cy - py);
            let d = dy * dy + dx * dx;
            if pick == 0 || d < best as f32 {
                pick = p;
                best = d as i32;
            }
        }
        if pick == 0 {
            return Err(Abort);
        }
        self.b_unhide(pick.wrapping_neg(), w);
        Ok(pick)
    }

    /// `st_SetTalalmanyAllapot(t, 5, −1)` (exe 0x602920): the human gets tech `t`; the specials queue gets `(EN,
    /// t)`.
    pub fn give_tech(&mut self, t: i32, w: &mut dyn StoryWorld) {
        w.event(StoryEvent::GiveTech(t));
        self.specials.add(w.human(), t);
    }

    /// `AutoAddBuilding(id)` (exe 0x5c1c80): planet `id`'s story buildings; its `+0x28` becomes 3000 + their
    /// workers + `rnd(−200, 1300)`.
    pub fn auto_add_building(&mut self, id: i32, w: &mut dyn StoryWorld) {
        let p = self.b_search(id, 0, w);
        if p == 0 {
            return;
        }
        let mut sum = 3000i32;
        for k in 0..self.building_data.len() {
            let r = self.building_data[k];
            if r.planet == id && (r.story == 0 || self.is_running(r.story)) {
                w.event(StoryEvent::PlaceStoryBuilding { planet: p, kind: r.kind, level: r.level });
                sum = sum.wrapping_add(w.building_workers(r.kind));
            }
        }
        let value = sum.wrapping_add(rnd(w, -200, 1300));
        w.event(StoryEvent::SetPlanetValue { planet: p, value });
    }

    /// `AutoAddtank(id)` (exe 0x5c1ef0): the vehicles `flotta.txt` lists for fleet `−id` land on planet `id`.
    pub fn auto_add_tank(&mut self, id: i32, w: &mut dyn StoryWorld) {
        let p = self.b_search(id, 0, w);
        if p == 0 {
            return;
        }
        for k in 0..self.fleet_data.len() {
            let r = self.fleet_data[k].clone();
            if r.fleet != id.wrapping_neg() || (r.story != 0 && !self.is_running(r.story)) {
                continue;
            }
            let design = if r.design < 1 { r.design.wrapping_neg() } else { w.story_design(w.planet_owner(p), r.design, 1) };
            let count = r.counts.get((w.difficulty() - 1) as usize).map_or(0, |c| c[1]) as i32;
            w.event(StoryEvent::NewTanks { fleet: p.wrapping_neg(), design, count });
        }
    }
}

/// Special 0x19 (every check): a design of the human's made from a story design of these stories (key
/// 2200..=2299) is deleted.
fn delete_story_design(s: &mut Story, w: &mut dyn StoryWorld) {
    if !s.special_is(0x19, 0, 0, 0) {
        return;
    }
    let (f, d) = (s.special[1], s.special[2]);
    let key = w.design_key(w.fleet_owner(f), d) / 10;
    if 219 < key && key < 230 {
        let en = w.human();
        w.event(StoryEvent::DeleteDesign { empire: en, design: d, fleet: f, mark: true });
    }
}

// ---------------------------------------------------------------- 220..=229: mercenaries

/// When the offer comes.
enum Offer {
    /// Tech `tech` done and the human's attack `/ div > above` (else 500 days later).
    Tech { tech: i32, div: i32, above: i32 },
    /// The human's attack `/ div ≥ at_least` (else 50 days later).
    Strength { div: i32, at_least: i32 },
    /// At once.
    Always,
}

struct Mercenary {
    /// The offer's message (`M`; `M + 1` the reminder, `M + 9` the start counter).
    m: i32,
    /// The price; `None`: one message, free.
    price: Option<i32>,
    start: (i32, i32),
    offer: Offer,
    /// The reminder's delay in days.
    recall: i32,
    /// `StoryAdat` raised when the hired fleet starts a space battle.
    bonus: &'static [(usize, i32)],
}

const MERCENARIES: [Mercenary; 9] = [
    Mercenary { m: 2200, price: Some(80_000), start: (500, 1000), offer: Offer::Tech { tech: 7, div: 3, above: 2020 }, recall: 360, bonus: &[(0x84, 20)] },
    Mercenary { m: 2210, price: Some(200_000), start: (500, 1000), offer: Offer::Tech { tech: 8, div: 3, above: 2520 }, recall: 360, bonus: &[(0x7c, 10), (0x90, 22)] },
    Mercenary { m: 2220, price: None, start: (500, 1000), offer: Offer::Tech { tech: 4, div: 3, above: 1020 }, recall: 0, bonus: &[(0x90, 8)] },
    Mercenary { m: 2230, price: Some(27_000), start: (2000, 3000), offer: Offer::Strength { div: 3, at_least: 721 }, recall: 360, bonus: &[(0x84, 5)] },
    Mercenary { m: 2240, price: Some(230_000), start: (500, 1000), offer: Offer::Tech { tech: 8, div: 3, above: 1520 }, recall: 360, bonus: &[(0x84, 12), (0x90, 4)] },
    Mercenary { m: 2260, price: None, start: (2500, 3500), offer: Offer::Strength { div: 2, at_least: 521 }, recall: 0, bonus: &[(0x84, 10), (0x7c, 10)] },
    Mercenary { m: 2270, price: Some(10_000), start: (1500, 2000), offer: Offer::Always, recall: 60, bonus: &[(0x90, 8)] },
    Mercenary { m: 2280, price: Some(24_000), start: (500, 1000), offer: Offer::Tech { tech: 6, div: 3, above: 250 }, recall: 360, bonus: &[(0x7c, 12)] },
    Mercenary { m: 2290, price: Some(250_000), start: (500, 1000), offer: Offer::Tech { tech: 9, div: 2, above: 3520 }, recall: 360, bonus: &[(0x84, 10), (0x7c, 16)] },
];

/// 227: the enemy fleet that comes when the offer is turned down, and the counter of its chase.
const CHASE: i32 = 2272;

fn mercenary(s: &mut Story, w: &mut dyn StoryWorld, c: &Mercenary) -> Result<(), Abort> {
    let (m, x, en) = (c.m, c.m + 9, w.human());
    if s.starting_now() {
        let days = rnd(w, c.start.0, c.start.1);
        s.sz_start(x, days);
        s.v_set(x, 0);
    }
    if s.counter(x) {
        let offer = match c.offer {
            Offer::Tech { tech, div, above } => w.tech_status(tech, en) == 5 && fleet_strength(&*w) / div > above,
            Offer::Strength { div, at_least } => fleet_strength(&*w) / div >= at_least,
            Offer::Always => true,
        };
        if offer {
            message(w, m, 160);
        } else {
            s.sz_start(x, if matches!(c.offer, Offer::Strength { .. }) { 50 } else { 500 });
        }
    }
    let hire = |s: &mut Story, w: &mut dyn StoryWorld| {
        if let Some(price) = c.price {
            w.event(StoryEvent::AddMoney { empire: en, amount: -price as f64 });
        }
        let f = s.f_new_ex("", en, m, 0.0, 1, 0, w);
        let b = s.last_fn[crate::story::last_fn::B];
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: b as f32, y: 0.0 });
        s.remove_fleet_id(m, w);
        s.v_set(x, 1);
    };
    if c.price.is_none() {
        if answered(s, m, 2) {
            hire(s, w);
        }
        if answered(s, m, 3) || promoted(s, m) {
            s.win(0, true, w);
        }
    } else {
        if answered(s, m, 3) || answered(s, m + 1, 3) {
            s.sz_start(m + 1, c.recall);
        }
        if s.counter(m + 1) {
            message(w, m + 1, 160);
        }
        if answered(s, m, 2) || answered(s, m + 1, 2) {
            hire(s, w);
        }
        let refused = answered(s, m, 4) || promoted(s, m) || answered(s, m + 1, 4) || promoted(s, m + 1);
        if m == 2270 {
            // 227: the refused mercenaries come as an enemy and chase the human's nearest fleet.
            if refused {
                s.f_new_ex("", 10, m, 0.0, 1, 0, w);
            }
            if s.counter(CHASE) || refused {
                let f = s.f_search(m, w);
                if f == 0 {
                    s.win(0, true, w);
                } else {
                    let (fx, fy) = w.fleet_xy(f);
                    let n = nearest_fleet(fx as i32, fy as i32, en, &*w);
                    if n == 0 {
                        s.send_away(m, w);
                        s.win(0, true, w);
                    } else {
                        w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 2, x: n as f32, y: 0.0 });
                    }
                }
                s.sz_start(CHASE, 4);
            }
        } else if refused {
            s.win(0, true, w);
        }
    }
    if s.flags[flag::SPACE_START] && s.v_get(x) != 0 {
        let f = ship_fleet(&*w, en, m + 1);
        if f == 0 {
            s.win(0, true, w);
        } else if battle_with(s, &*w, f) {
            for &(off, add) in c.bonus {
                add_adat(s, off, add);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 230, 2301: a fleet in quarantine

/// 230: a known empire asks the human to bring a sick fleet's ships to its planets; each arrival changes the
/// ship and halves the planet; 1080 days later the story design is withdrawn.
fn quarantine(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let e = random_empire(w, 0, 0, 0)?;
        s.v_set(2302, e);
        message(w, 2300, 70);
        param(w, 1, MessageText::Empire(e));
        s.v_set(2309, 0);
    }
    if answered(s, 2300, 2) {
        let e = s.v_get(2302);
        s.add_info_line(2301, InfoLine::Empire(e), -1, w);
        let d = w.new_design_raw(en, &story_design_record(en, 12, 2300));
        s.v_set(2301, d);
        w.event(StoryEvent::RecalcDesign { empire: en, design: d });
        w.event(StoryEvent::DesignCostTimes5 { empire: en, design: d });
        let mut r = story_design_record(en, 12, 2301);
        r[0x18] = 0;
        let d = w.new_design_raw(en, &r);
        w.event(StoryEvent::RecalcDesign { empire: en, design: d });
        w.event(StoryEvent::DesignCostTimes5 { empire: en, design: d });
        s.sz_start(2300, 3);
        s.sz_start(2302, 1080);
        w.event(StoryEvent::ScrollText(2302));
    }
    if answered(s, 2300, 3) || promoted(s, 2300) {
        s.win(0, true, w);
    }
    if s.counter(2300) {
        let d = s.v_get(2301);
        let e = s.v_get(2302);
        s.sz_start(2300, 3);
        for k in 1..=w.ship_count() {
            let (f, dd) = w.ship(k);
            if dd != d || w.fleet_owner(f) != en || w.fleet_command(f) == 0 || w.fleet_order(f) != 1 {
                continue;
            }
            let p = w.fleet_target(f) as i32;
            if w.planet_infected(p) || w.planet_owner(p) != e || distance(w.planet_xy(p), w.fleet_xy(f)) >= 20 {
                continue;
            }
            message(w, 2302, 0);
            let design = w.story_design(en, 2301, 3);
            w.event(StoryEvent::UpgradeShip { ship: k, design });
            let value = w.planet_value(p) / 2;
            w.event(StoryEvent::SetPlanetValue { planet: p, value });
            let n = s.v_get(2309).wrapping_add(1);
            if n == 3 {
                s.sz_start(2302, 1);
            }
            s.v_set(2309, n);
            break;
        }
    }
    if s.counter(2302) {
        let d = s.v_get(2301);
        let left = (1..=w.ship_count()).map(|k| w.ship(k)).any(|(f, dd)| dd == d && w.fleet_owner(f) == en);
        message(w, if !left && s.v_get(2309) == 0 { 2304 } else { 2301 }, 0);
        let design = w.story_design(en, 2301, 3);
        let d = s.v_get(2301);
        for k in 1..=w.ship_count() {
            let (f, dd) = w.ship(k);
            if dd == d && w.fleet_owner(f) == en {
                w.event(StoryEvent::UpgradeShip { ship: k, design });
            }
        }
        w.event(StoryEvent::DeleteDesign { empire: en, design: d, fleet: 0, mark: true });
        s.start_story(2301, false);
        s.win(0, true, w);
    }
    Ok(())
}

/// 2301: the story design goes once no ship of it is left.
fn quarantine_end(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const C: i32 = 23011;
    if s.starting_now() {
        s.sz_start(C, 10);
    }
    if s.counter(C) {
        s.sz_start(C, 10);
        let en = w.human();
        if ship_fleet(&*w, en, 2301) == 0 {
            let design = w.story_design(en, 2301, 3);
            w.event(StoryEvent::DeleteDesign { empire: en, design, fleet: 0, mark: true });
            s.win(0, true, w);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 231, 2311: a planet to blow up

/// 231: a known empire's planet; a ship of the story design brought to it blows it up two days later.
fn blow_up(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let e = random_empire(w, 0, 0, 0)?;
        if !w.knows(en, e) {
            s.win(0, false, w);
        } else {
            let p = s.b_search(e.wrapping_neg(), 0, w);
            if p == 0 {
                return Err(Abort);
            }
            s.b_add_id(p, 2310);
            s.b_show(2310, -1, w);
            message(w, 2310, 0);
            param(w, 1, MessageText::Planet(p));
        }
    }
    if answered(s, 2310, 3) {
        let d = w.new_design_raw(en, &story_design_record(en, 13, 2310));
        s.v_set(2310, d);
        w.event(StoryEvent::RecalcDesign { empire: en, design: d });
        s.sz_start(2310, 3);
        let p = s.b_search(2310, 0, w);
        s.add_info_line(2311, InfoLine::Planet { planet: p, count: None }, -1, w);
    }
    if answered(s, 2310, 4) || promoted(s, 2310) {
        s.win(0, true, w);
    }
    let mut done = false;
    if s.daily() {
        let p = s.b_search(2310, 0, w);
        if w.planet_owner(p) == en {
            done = true;
            s.win(0, true, w);
        } else {
            let d = s.v_get(2310);
            s.sz_start(2310, 3);
            for k in 1..=w.ship_count() {
                let (f, dd) = w.ship(k);
                if dd == d && w.fleet_owner(f) == en && distance(w.fleet_xy(f), w.planet_xy(p)) < 20 {
                    message(w, 2312, 0);
                    s.b_add_id(p, 23111);
                    s.start_story(2311, false);
                    done = true;
                    w.event(StoryEvent::DeleteFleet(f));
                    s.win(0, true, w);
                    break;
                }
            }
        }
    }
    if done {
        let design = w.story_design(en, 2310, 3);
        w.event(StoryEvent::DeleteDesign { empire: en, design, fleet: 0, mark: true });
    }
    Ok(())
}

/// 2311: the planet is destroyed.
fn blow_up_end(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const C: i32 = 23111;
    if s.starting_now() {
        s.sz_start(C, 2);
    }
    if s.counter(C) {
        w.event(StoryEvent::Anim(6));
        let p = s.b_search(C, 0, w);
        s.error = (p < 1) as i32;
        w.event(StoryEvent::DestroyPlanet(p));
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 232, 233

/// 232: free planets (any with more than 50 of the human's) the human sees but hasn't explored become known,
/// each with chance 1/3.
fn reveal(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        let en = w.human();
        let mut n = 0;
        for p in 1..w.planet_count() {
            if (w.planets_of(en) > 50 || w.planet_owner(p) == 0) && w.planet_visibility(p) == 0 && rnd(w, 0, 2) == 0 {
                w.event(StoryEvent::ShowPlanet(p));
                n += 1;
            }
        }
        if n != 0 {
            message(w, 2320, 0);
        }
        s.win(0, true, w);
    }
    Ok(())
}

/// 233: strikes on the human's three most productive planets (info 2331, morale −20 for 1080 days).
fn strikes(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        match s.best_three(&*w) {
            None => s.win(0, false, w),
            Some(best) => {
                message(w, 2330, 100);
                for (k, p) in (0..).zip(best) {
                    if p < 1 {
                        return Err(Abort);
                    }
                    s.b_add_id(p, 2330 + k);
                    param(w, k + 1, MessageText::Planet(p));
                }
            }
        }
    }
    if answered(s, 2330, 2) {
        for k in 0..3 {
            let p = s.b_search(2330 + k, 0, w);
            w.event(StoryEvent::AddPlanetInfo { planet: p, info: 2331 });
            w.event(StoryEvent::PlanetMorale { planet: p, delta: 200 });
        }
        s.sz_start(2331, 1080);
    }
    if answered(s, 2330, 3) || promoted(s, 2330) {
        s.win(0, true, w);
    }
    if s.counter(2331) {
        message(w, 2331, 0);
        for k in 0..3 {
            let p = s.b_search(2330 + k, 0, w);
            w.event(StoryEvent::RemovePlanetInfo { planet: p, info: 2331 });
            w.event(StoryEvent::PlanetMorale { planet: p, delta: -200 });
        }
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 234, 235: a tax

struct Tax {
    m: i32,
    /// The story that excludes this one once it ran.
    other: i32,
    /// The three shares the human keeps (f32) and the message's parameters (f64).
    factors: [f32; 3],
    params: [f64; 3],
}

const TAXES: [Tax; 2] = [
    Tax { m: 2340, other: 235, factors: [0.9, 0.8, 0.7], params: [0.9, 0.8, 0.7] },
    Tax { m: 2350, other: 234, factors: [0.85, 0.75, 0.65], params: [0.85, 0.75, 0.65] },
];

/// 235's fleet: the payment's steps (thousands) and the ground units per step.
const STEPS: [i32; 7] = [7, 15, 30, 50, 80, 140, -1];
const TANKS: [i32; 7] = [901, 901, 902, 902, 903, 903, 903];

/// 234: the human pays 10, 20 or 30% of its planets' `+0x28`: morale +40 (and later +70 and a tech) for more.
/// 235: the same in 15/25/35%; the payment comes back as a fleet.
fn tax(s: &mut Story, w: &mut dyn StoryWorld, c: &Tax) -> Result<(), Abort> {
    let (m, en) = (c.m, w.human());
    if s.starting_now() {
        if w.planets_of(en) < 5 {
            s.win(0, false, w);
        } else if s.history.contains(c.other) {
            s.win(0, true, w);
        } else {
            message(w, m, 0);
            let total = (1..=w.planet_count()).filter(|&p| w.planet_owner(p) == en).fold(0i32, |t, p| t.wrapping_add(w.planet_value(p)));
            for (k, q) in (1..).zip(c.params) {
                w.event(StoryEvent::MessageParamInt { slot: k, value: (total as f64 * q) as i32 });
            }
        }
    }
    if s.flags[flag::MESSAGE] && s.message.0 == m && (2..=4).contains(&s.message.1) {
        let part = s.message.1;
        let q = c.factors[(part - 2) as usize];
        let mut paid = 0i32;
        for p in 1..w.planet_count() {
            if w.planet_owner(p) != en {
                continue;
            }
            let v = w.planet_value(p) as f32;
            if m == 2350 {
                paid = ((1.0 - q) * v + paid as f32) as i32;
            }
            w.event(StoryEvent::SetPlanetValue { planet: p, value: (v * q) as i32 });
        }
        if m == 2350 {
            s.v_set(2351, paid);
        }
        match part {
            3 => s.sz_start(m, 50),
            4 => s.sz_start(m + 1, 50),
            _ => s.win(0, true, w),
        }
    }
    if m == 2340 {
        if s.counter(2340) {
            message(w, 2341, 0);
            w.event(StoryEvent::GlobalMorale(-400));
            s.win(0, true, w);
        }
        if s.counter(2341) {
            message(w, 2342, 0);
            w.event(StoryEvent::GlobalMorale(-700));
            let t = random_tech(w, en, 0);
            if t == 0 {
                param(w, 1, MessageText::Text(2341));
            } else {
                s.give_tech(t, w);
                param(w, 1, MessageText::Tech(t));
            }
            s.win(0, true, w);
        }
    } else if s.counter(2350) || s.counter(2351) {
        let paid = s.v_get(2351);
        let n = (if s.counter(2350) { 3i32 } else { 6 }).wrapping_add(paid / 140_000);
        let (mut lo, mut k) = (0, 0);
        while STEPS[k] > 0 && STEPS[k] * 1000 < paid {
            lo = STEPS[k];
            k += 1;
        }
        let hi = if STEPS[k] == -1 { 280 } else { STEPS[k] };
        let big = (paid.wrapping_sub(lo * 1000) / (hi - lo)).wrapping_mul(n) / 1000;
        let big = if big > n { n } else { big.max(0) };
        message(w, 2351, 0);
        let f = s.f_new_ex("", en, 0, 0.0, 1, 0, w);
        let b = s.last_fn[crate::story::last_fn::B];
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: b as f32, y: 0.0 });
        let d = w.story_design(en, 2350 + k as i32, 3);
        s.f_add_ship(f.wrapping_neg(), d, n - big, w);
        let d = w.story_design(en, 2351 + k as i32, 3);
        s.f_add_ship(f.wrapping_neg(), d, big, w);
        let d = w.story_design(en, TANKS[k], 3);
        s.f_add_tank(f.wrapping_neg(), d, n, w);
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 236: a refugee planet

fn refugees(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        if w.planets_of(en) < 10 {
            s.win(0, false, w);
        } else {
            message(w, 2360, 100);
            w.event(StoryEvent::AddMoney { empire: en, amount: 12000.0 });
            let p = s.b_search(en.wrapping_neg(), 0, w);
            s.b_add_id(p, 2360);
            param(w, 1, MessageText::Planet(p));
        }
    }
    if answered(s, 2360, 2) || promoted(s, 2360) {
        s.win(0, true, w);
    }
    if answered(s, 2360, 3) {
        s.sz_start(2360, 50);
    }
    if s.flags[flag::DISABLED] && s.message.0 == 2360 {
        let p = s.b_search(2360, 0, w);
        if w.planet_owner(p) != en {
            w.event(StoryEvent::DisablePart(4));
        }
    }
    if answered(s, 2360, 4) {
        let p = s.b_search(2360, 0, w);
        if w.planet_owner(p) == en {
            let p = s.b_search(2360, 0, w);
            w.event(StoryEvent::PlanetMorale { planet: p, delta: -300 });
        }
        s.win(0, true, w);
    }
    if s.counter(2360) {
        message(w, 2361, 100);
    }
    if answered(s, 2361, 2) {
        let p = s.unknown_planet(w)?;
        s.b_add_id(p, 2360);
        w.event(StoryEvent::SetPlanetOwner { planet: p, owner: en, race: 10 });
        w.event(StoryEvent::SetPlanetValue { planet: p, value: 0x2800 });
        s.auto_add_building(2360, w);
        s.auto_add_tank(2360, w);
        s.f_new_ex("", en, 2360, 0.0, p.wrapping_neg(), 20, w);
        s.f_add_ship(2360, 3, 3, w);
        s.f_add_ship(2360, 2, 5, w);
        s.remove_fleet_id(2360, w);
        s.f_new_ex("", en, 2361, 0.0, p.wrapping_neg(), 20, w);
        s.remove_fleet_id(2361, w);
        s.win(0, true, w);
    }
    if answered(s, 2361, 3) {
        s.sz_start(2361, 50);
    }
    if answered(s, 2361, 4) {
        s.sz_start(2362, 6);
    }
    if promoted(s, 2361) {
        s.win(0, true, w);
    }
    if s.counter(2361) {
        message(w, 2362, 0);
        w.event(StoryEvent::GlobalMorale(-100));
        s.win(0, true, w);
    }
    if s.counter(2362) {
        message(w, 2363, 0);
        for _ in 0..5 {
            w.rng().choice_begin();
            let mut pick = 0;
            for p in 1..w.planet_count() {
                if w.planet_visibility(p) == 0 && w.rng().choice_offer(STREAM, 1) {
                    pick = p;
                }
            }
            if pick == 0 {
                break;
            }
            w.event(StoryEvent::ShowPlanet(pick));
        }
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 237..=239

/// 237: every fleet is visible for 200 days.
fn all_fleets(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        message(w, 2370, 0);
        s.sz_start(2370, 200);
        s.all_fleets_visible = 1;
        s.add_info_line(2371, InfoLine::Blank, -1, w);
    }
    if s.counter(2370) {
        s.all_fleets_visible = 0;
        message(w, 2371, 0);
        s.win(0, true, w);
    }
    Ok(())
}

/// 238: `StoryAdat+0x38` for 720 days if the human agrees.
fn switch_38(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    if s.starting_now() {
        message(w, 2380, 60);
    }
    if answered(s, 2380, 2) {
        s.set_adat_i32(0x38, 1);
        s.sz_start(2380, 720);
    }
    if answered(s, 2380, 3) || promoted(s, 2380) {
        s.win(0, true, w);
    }
    if s.counter(2380) {
        message(w, 2381, 0);
        s.set_adat_i32(0x38, 0);
        s.win(0, true, w);
    }
    Ok(())
}

/// 239: with a wormhole shown, a ship of design 0x1c that opens another (special 0x11) near a planet of the
/// human's.
fn wormhole(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        if !w.wormhole_shown() {
            s.win(0, false, w);
        } else {
            let p = s.b_search(en.wrapping_neg(), 0, w);
            if p == 0 {
                return Err(Abort);
            }
            s.v_set(2391, 0);
            message(w, 2390, 80);
            s.b_add_id(p, 2390);
            param(w, 1, MessageText::Planet(p));
        }
    }
    if answered(s, 2390, 2) {
        s.v_set(2391, 1);
        let p = s.b_search(2390, 0, w);
        s.f_new_ex("", en, 2390, 0.0, 2, p, w);
        s.f_add_ship(2390, 0x1c, 1, w);
        s.remove_fleet_id(2390, w);
    }
    if answered(s, 2390, 3) || promoted(s, 2390) {
        s.win(0, true, w);
    }
    if s.daily() && s.v_get(2391) != 0 && ship_fleet(&*w, en, 0x1c) == 0 {
        message(w, 2391, 0);
        s.win(0, true, w);
    }
    if s.special_is(0x11, 0, 0, 0) {
        message(w, 2392, 0);
        let p = s.b_search(2390, 0, w);
        let r = rnd(w, 20, 28);
        let q = rnd(w, 1, 360);
        let a = q as f64 * std::f64::consts::PI / 180.0;
        let dx = (a.cos() * r as f64) as i32;
        let dy = (a.sin() * r as f64) as i32;
        let (fx, fy) = w.fleet_xy(s.special[1]);
        let (px, py) = w.planet_xy(p);
        w.event(StoryEvent::Wormhole { x1: fx as i32, y1: fy as i32, x2: (dx as f32 + px) as i32, y2: (dy as f32 + py) as i32 });
        w.event(StoryEvent::RevealWormhole);
        s.win(0, true, w);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Streams;

    /// A small world: the human is empire 2.
    #[derive(Default)]
    struct W {
        rng: Streams,
        owners: Vec<i32>,
        vis: Vec<i32>,
        prod: Vec<(i32, i32)>,
        events: Vec<StoryEvent>,
    }

    impl StoryWorld for W {
        fn ido(&self) -> i32 {
            0
        }
        fn speed_mult(&self) -> i32 {
            1
        }
        fn human(&self) -> i32 {
            2
        }
        fn difficulty(&self) -> i32 {
            1
        }
        fn multiplayer(&self) -> bool {
            false
        }
        fn rng(&mut self) -> &mut Streams {
            &mut self.rng
        }
        fn planet_count(&self) -> i32 {
            self.owners.len() as i32 - 1
        }
        fn planet_owner(&self, p: i32) -> i32 {
            self.owners[p as usize]
        }
        fn planet_value(&self, _p: i32) -> i32 {
            0
        }
        fn planet_visibility(&self, p: i32) -> i32 {
            self.vis[p as usize]
        }
        fn fleet_count(&self) -> i32 {
            0
        }
        fn fleet_owner(&self, _f: i32) -> i32 {
            0xff
        }
        fn fleet_id(&self, _f: i32) -> i32 {
            0
        }
        fn empire_alive(&self, _e: i32) -> bool {
            true
        }
        fn planets_of(&self, e: i32) -> i32 {
            self.owners.iter().filter(|&&o| o == e).count() as i32
        }
        fn money(&self, _e: i32) -> f64 {
            0.0
        }
        fn tech_status(&self, _t: i32, _e: i32) -> i32 {
            0
        }
        fn relation_level(&self, _a: i32, _b: i32) -> i32 {
            0
        }
        fn event(&mut self, ev: StoryEvent) {
            self.events.push(ev);
        }
        fn planet_production(&self, p: i32) -> (i32, i32) {
            self.prod[p as usize]
        }
    }

    #[test]
    fn best_three_keeps_a_stale_slot() {
        // Planets 1..=4 of the human's (the loop stops before the last planet), productions 10, 50, 30.
        let mut w = W { owners: vec![0, 2, 2, 2, 2, 0], vis: vec![0; 6], prod: vec![(0, 0), (9, 0), (49, 0), (29, 0), (99, 0), (0, 0)], ..W::default() };
        let mut s = Story::default();
        assert_eq!(s.best_three(&w), Some([3, 2, 4]));
        // Now only planets 1 and 2 qualify: slot 0 keeps planet 3 from before.
        w.owners = vec![0, 2, 2, 7, 7, 2];
        assert_eq!(s.best_three(&w), Some([3, 2, 1]));
    }

    #[test]
    fn st_tav_truncates_the_coordinates() {
        assert_eq!(distance((10.9, 0.0), (0.2, 0.0)), 10);
        assert_eq!(distance((3.0, 4.9), (0.0, 0.0)), 5);
    }
}
