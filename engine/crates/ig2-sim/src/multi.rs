//! The multiplayer game modes on the story engine: `st_Multi_include` (exe 0x5ee4d0), which `Story_Check`
//! runs after the other scripts in SP and MP. 1001 and 1002 are king of the hill, 1003 three planets, 1005
//! capture the flag; 1004 and 1006 are empty. The game set-up starts them. Spec: `re/rules/campaign.md` §8.10.
//!
//! What the modes do to the rest of the game goes out as [`StoryEvent`]s (texts, info lines, ships, a planet
//! given back, fleets shown) and [`StoryWorld`] calls (the flag carrier's design).

use crate::story::{flag, Abort, InfoLine, MessageText, Notice, Script, Story, StoryEvent, StoryWorld, DESIGN};

/// The modes in the exe's order. A mode a search aborts is `WIN(0, 1)`ed and the next one runs.
pub struct MultiStories;

impl Script for MultiStories {
    fn run(&self, s: &mut Story, w: &mut dyn StoryWorld) {
        type Mode = fn(&mut Story, &mut dyn StoryWorld) -> Result<(), Abort>;
        let modes: [(i32, Mode); 6] = [
            (1001, king_of_the_hill),
            (1002, king_of_the_hill_per_owner),
            (1003, three_planets),
            (1004, empty),
            (1005, capture_the_flag),
            (1006, empty),
        ];
        for (id, mode) in modes {
            if s.running(id) && mode(s, w).is_err() {
                s.win(0, true, w);
            }
        }
    }
}

/// The story ids (decimal = the exe's hex).
pub mod id {
    pub const HILL: i32 = 10011;
    pub const HILL_DAYS: i32 = 10010;
    pub const HILL2: i32 = 10021;
    pub const HILL2_DAYS: i32 = 10020;
    /// `v(HILL2_LEFT + e)`: owner `e`'s days left in 1002.
    pub const HILL2_LEFT: i32 = 10041;
    /// 1003's planets 10031..=10033 (and the counter re-checking them), the radar counter 10033, the
    /// message counter 10034.
    pub const THREE: i32 = 10031;
    pub const RADAR: i32 = 10033;
    pub const OFFER: i32 = 10034;
    /// Ours (canonical multiplayer, `re/rules/multiplayer.md` §5.3): a variable with a bit per empire that bought the
    /// radar. No script uses this id.
    pub const RADAR_BUYERS: i32 = 10039;
    /// The radar offer (message block 0x2347).
    pub const OFFER_MESSAGE: i32 = 0x2347;
    /// 1005: the bases 10051/10052 (and the carriers' designs `v(10051)`/`v(10052)`), who holds the other's
    /// flag `v(10053)`/`v(10054)`, scores `v(10055)`/`v(10056)`, touchdowns `v(10057)`/`v(10058)`, the
    /// touchdown points `v(10059)` and their step `v(10060)`.
    pub const BASE: i32 = 10051;
    pub const HOLDS: i32 = 10053;
    pub const SCORE: i32 = 10055;
    pub const TOUCHDOWNS: i32 = 10057;
    pub const POINTS: i32 = 10059;
    pub const STEP: i32 = 10060;
}

/// The bytes of the flag carrier's record the exe leaves as its stack held them.
pub const FLAG_CARRIER_UNSET: [usize; 9] = [0x15, 0x1b, 0x29, 0x2a, 0x2b, 0x3a, 0x3b, 0x42, 0x43];

/// The "Flag Carrier" design record (spec §8.10); the race field (`+0x40`) gets the player number `en`.
pub fn flag_carrier(en: i32) -> [u8; DESIGN] {
    let mut r = [0u8; DESIGN];
    r[..12].copy_from_slice(b"Flag Carrier");
    r[0x16] = 1;
    r[0x19] = 1;
    r[0x1a] = 1;
    r[0x20..0x24].copy_from_slice(&250_000i32.to_le_bytes());
    r[0x24..0x28].copy_from_slice(&3_500i32.to_le_bytes());
    r[0x3c..0x40].copy_from_slice(&1.0f32.to_le_bytes());
    r[0x40..0x42].copy_from_slice(&(en as i16).to_le_bytes());
    r[0x44..0x48].copy_from_slice(&13i32.to_le_bytes());
    r[0x48..0x4c].copy_from_slice(&13i32.to_le_bytes());
    r
}

/// `(i32)(f32) sqrt((f64)(f32)(dx² + dy²))`: the modes' distances.
fn distance(a: (f32, f32), b: (f32, f32)) -> i32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    ((dy * dy + dx * dx) as f64).sqrt() as f32 as i32
}

/// `st_GetBirodalomSulypont(&x, &y, e)` (exe 0x5ea570): the mean position of empire `e`'s planets the human
/// sees, planet slot 0 included (f32, summed in order); (0, 0) when there are none.
pub fn empire_centre(e: i32, w: &dyn StoryWorld) -> (f32, f32) {
    let (mut x, mut y, mut n) = (0f32, 0f32, 0);
    for p in 0..=w.planet_count() {
        if w.planet_owner(p) == e && w.planet_visibility(p) >= 0 {
            let (px, py) = w.planet_xy(p);
            x += px;
            y += py;
            n += 1;
        }
    }
    if n == 0 { (0.0, 0.0) } else { (x / n as f32, y / n as f32) }
}

/// `st_bGetNearest(x, y, owner)` (exe 0x603980): the planet the human sees with owner `owner` (−1: any live
/// one) nearest to `(x, y)` (f32 `dy² + dx²` against the last pick's truncated value; the first of equals);
/// 0 when there is none.
pub fn nearest_planet(x: i32, y: i32, owner: i32, w: &dyn StoryWorld) -> i32 {
    let (mut best, mut best_d) = (0, 0);
    for q in 1..=w.planet_count() {
        let o = w.planet_owner(q);
        if w.planet_visibility(q) < 0 || !(o == owner || (owner == -1 && o != 0xff)) {
            continue;
        }
        let (qx, qy) = w.planet_xy(q);
        let (dx, dy) = (x as f32 - qx, y as f32 - qy);
        let d = dy * dy + dx * dx;
        if best == 0 || d < best_d as f32 {
            best = q;
            best_d = d as i32;
        }
    }
    best
}

impl Story {
    /// `st_bShow(x, owner)` (exe 0x603f20): planet `−x` (`x ≤ 0`) or the planet with story id `x` is shown
    /// to everybody (`st_bUnhide`), its owner met by the human, the human's view of it at least 1, its info
    /// string `−x` (if any) added as an info line, and an undo entry `(2, p << 16 | x, owner)` registered
    /// (`owner == −1`: `x / 10`). `StoryError` = no planet.
    pub fn b_show(&mut self, x: i32, owner: i32, w: &mut dyn StoryWorld) {
        let p = if x < 1 { x.wrapping_neg() } else { self.b_search(x, 0, w) };
        let owner = if owner == -1 { x / 10 } else { owner };
        if p < 1 {
            self.error = 1;
            return;
        }
        self.error = 0;
        w.event(StoryEvent::UnhidePlanet(p));
        let (en, o) = (w.human(), w.planet_owner(p));
        if o != 0 && o != en && !w.knows(en, o) {
            w.event(StoryEvent::DiscoverEmpire(o));
        }
        if w.planet_visibility(p) < 1 {
            w.event(StoryEvent::ShowPlanet(p));
        }
        if x > 0 && w.info_string(x.wrapping_neg()) {
            w.event(StoryEvent::AddPlanetInfo { planet: p, info: x.wrapping_neg() });
        }
        if owner != 0 {
            self.add_undo(2, (p << 16) | (x & 0xffff), owner);
        }
    }
}

/// The hill planet of 1001/1002: the one with story id `id`, else a random free planet the human sees gets it.
fn hill_planet(s: &mut Story, id: i32, w: &mut dyn StoryWorld) -> Result<i32, Abort> {
    let p = s.b_search(id, 1, w);
    if p != 0 {
        return Ok(p);
    }
    let p = s.b_search_try(0, 0, w)?;
    s.b_add_id(p, id);
    Ok(p)
}

/// A ground battle on planet `id` (any with 0), a colonisation (special 10) or counter `id`: 1001..=1003
/// look at their planets' owners.
fn owner_check(s: &mut Story, id: i32, w: &mut dyn StoryWorld) -> bool {
    (s.flags[flag::GROUND_END] && (id == 0 || s.ground.planet == s.b_search(id, 0, w))) || s.special_is(10, 0, 0, 0) || s.counter(id)
}

/// The hill's info line: 10012 for the human's, 10013 for another owner's (10011 for none, on removal).
fn hill_info(base: i32, owner: i32, en: i32) -> i32 {
    if owner == en { base + 1 } else { base + 2 }
}

/// 1001: whoever holds the hill planet for `v(10010)` days (150 by default) wins.
fn king_of_the_hill(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let p = hill_planet(s, id::HILL, w)?;
        s.add_info_line(id::HILL, InfoLine::Planet { planet: p, count: None }, -1, w);
        s.b_show(id::HILL, -1, w);
        s.v_set(id::HILL, 0);
    }
    if owner_check(s, id::HILL, w) {
        let p = s.b_search(id::HILL, 0, w);
        let o = s.v_get(id::HILL);
        let owner = w.planet_owner(p);
        if owner != o {
            s.remove_game_info(if o == 0 { id::HILL } else { hill_info(id::HILL, o, en) }, w);
            s.v_set(id::HILL, owner);
            let days = match s.v_get(id::HILL_DAYS) {
                0 => 150,
                d => d,
            };
            s.v_set(id::HILL + 1, days);
            s.sz_start(id::HILL, 1);
            let n = s.v_get(id::HILL + 1);
            s.add_info_line(hill_info(id::HILL, owner, en), InfoLine::Planet { planet: p, count: Some(n) }, -1, w);
        }
    }
    if s.counter(id::HILL) {
        let n = s.v_get(id::HILL + 1);
        let o = s.v_get(id::HILL);
        let p = s.b_search(id::HILL, 0, w);
        let n = n.wrapping_sub(1);
        s.v_set(id::HILL + 1, n);
        if n < 1 {
            w.event(StoryEvent::Notice(Notice::Winner { empire: o, amazing: true }));
            s.win(0, true, w);
        } else {
            let info = hill_info(id::HILL, o, en);
            s.remove_game_info(info, w);
            let n = s.v_get(id::HILL + 1);
            s.add_info_line(info, InfoLine::Planet { planet: p, count: Some(n) }, -1, w);
            s.sz_start(id::HILL, 1);
        }
    }
    Ok(())
}

/// 1002: as 1001, but every owner has its own `v(10020)` days (600 by default) of holding the hill.
fn king_of_the_hill_per_owner(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        let p = hill_planet(s, id::HILL2, w)?;
        s.add_info_line(id::HILL2, InfoLine::Planet { planet: p, count: None }, -1, w);
        s.b_show(id::HILL2, -1, w);
        s.v_set(id::HILL2, 0);
        let days = match s.v_get(id::HILL2_DAYS) {
            0 => 600,
            d => d,
        };
        for e in 1..9 {
            s.v_set(id::HILL2_LEFT + e, days);
        }
    }
    if owner_check(s, id::HILL2, w) {
        let p = s.b_search(id::HILL2, 0, w);
        let o = s.v_get(id::HILL2);
        let owner = w.planet_owner(p);
        if owner != o {
            s.remove_game_info(if o == 0 { id::HILL2 } else { hill_info(id::HILL2, o, en) }, w);
            s.v_set(id::HILL2, owner);
            s.sz_start(id::HILL2, 1);
            let n = s.v_get(id::HILL2_LEFT + owner);
            s.add_info_line(hill_info(id::HILL2, owner, en), InfoLine::Planet { planet: p, count: Some(n) }, -1, w);
        }
    }
    if s.counter(id::HILL2) {
        let o = s.v_get(id::HILL2);
        let n = s.v_get(id::HILL2_LEFT + o);
        let p = s.b_search(id::HILL2, 0, w);
        let n = n.wrapping_sub(1);
        s.v_set(id::HILL2_LEFT + o, n);
        if n < 1 {
            w.event(StoryEvent::Notice(Notice::Winner { empire: o, amazing: true }));
            s.win(0, true, w);
        } else {
            let info = hill_info(id::HILL2, o, en);
            s.remove_game_info(info, w);
            let n = s.v_get(id::HILL2_LEFT + o);
            s.add_info_line(info, InfoLine::Planet { planet: p, count: Some(n) }, -1, w);
            s.sz_start(id::HILL2, 1);
        }
    }
    Ok(())
}

/// 1003: whoever owns three far-apart planets at once wins; the human can buy a radar on them.
fn three_planets(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        if s.b_search(id::THREE, 1, w) == 0 {
            // 51 tries of three random free planets (stream 2); the widest triangle wins.
            let (mut best, mut widest) = ([1, 1, 1], 0);
            for _ in 0..51 {
                let a = s.b_search_try(0, 2, w)?;
                let b = s.b_search_try(0, 2, w)?;
                let c = s.b_search_try(0, 2, w)?;
                if a != b && b != c && a != c {
                    let (pa, pb, pc) = (w.planet_xy(a), w.planet_xy(b), w.planet_xy(c));
                    let sum = distance(pa, pb).wrapping_add(distance(pb, pc)).wrapping_add(distance(pc, pa));
                    if sum > widest {
                        best = [a, b, c];
                        widest = sum;
                    }
                }
            }
            for (k, p) in (0..).zip(best) {
                s.b_add_id(p, id::THREE + k);
            }
        }
        for k in 0..3 {
            s.b_show(id::THREE + k, -1, w);
        }
        s.add_info_line(id::THREE, InfoLine::Blank, -1, w);
        s.sz_start(id::THREE, 10);
        s.sz_start(id::OFFER, 50);
    }
    // The radar offer: message 0x2347, answered (part 2 buys, part 4 asks again later) or promoted.
    if s.counter(id::OFFER) {
        w.event(StoryEvent::NewMessage { id: id::OFFER_MESSAGE, arg: 300 });
        w.event(StoryEvent::MessageParam { slot: 1, text: MessageText::Player(en) });
    }
    if s.flags[flag::MESSAGE] && s.message == (id::OFFER_MESSAGE, 2) {
        w.event(StoryEvent::SendStorySpecial(id::THREE));
    }
    if s.flags[flag::MESSAGE] && s.message == (id::OFFER_MESSAGE, 4) {
        s.sz_start(id::OFFER, 50);
    }
    if s.flags[flag::PROMOTED] && s.message.0 == id::OFFER_MESSAGE {
        s.sz_start(id::OFFER, 100);
    }
    // The purchase comes back from the lockstep queue as special 15 (sender, 10031).
    if s.special_is(15, 0, id::THREE, 0) {
        let buyer = s.special[1];
        let payer = if buyer > 0 { buyer } else { en };
        w.event(StoryEvent::AddMoney { empire: payer, amount: -20000.0 });
        if w.canonical() {
            // Every buyer's radar, not only the local human's (each machine's in the original).
            let bought = s.v_get(id::RADAR_BUYERS);
            s.v_set(id::RADAR_BUYERS, bought | 1i32.wrapping_shl(payer as u32));
            s.sz_start(id::RADAR, 1);
        } else if buyer == en {
            s.sz_start(id::RADAR, 1);
        }
    }
    if s.counter(id::RADAR) {
        s.sz_start(id::RADAR, 1);
        // The radar's owners: the human (classic), every buyer (canonical multiplayer).
        let mask = if w.canonical() { s.v_get(id::RADAR_BUYERS) } else { 1i32.wrapping_shl(en as u32) };
        let bought = |o: i32| (0..32).contains(&o) && mask >> o & 1 != 0;
        let enemy_of = |w: &dyn StoryWorld, f: i32, of: &dyn Fn(i32) -> bool| {
            let o = w.fleet_owner(f);
            o != 0xff && !of(o) && o <= 8
        };
        for f in 1..=w.fleet_count() {
            if enemy_of(&*w, f, &bought) {
                w.event(StoryEvent::SetFleetVisible { fleet: f, on: false });
            }
        }
        // The nearest enemy's owner carries over from one planet to the next.
        let mut nearest = 0;
        for k in 0..3 {
            let p = s.b_search(id::THREE + k, 0, w);
            let mut best = 1000;
            let po = w.planet_owner(p);
            if bought(po) {
                for f in 1..=w.fleet_count() {
                    if !enemy_of(&*w, f, &|o| o == po) {
                        continue;
                    }
                    let d = distance(w.planet_xy(p), w.fleet_xy(f));
                    if d < best {
                        best = d;
                        nearest = w.fleet_owner(f);
                    }
                    if d < 80 {
                        w.event(StoryEvent::SetFleetVisible { fleet: f, on: true });
                    }
                }
            }
            let info = id::THREE + 1 + k;
            s.remove_game_info(info, w);
            let level = match best {
                ..50 => 3,
                50..80 => 2,
                80..110 => 1,
                _ => 0,
            };
            if level > 0 {
                s.add_info_line(info, InfoLine::Threat { empire: nearest, level, planet: p }, -1, w);
            }
        }
    }
    if s.flags[flag::GROUND_END] || s.special_is(10, 0, 0, 0) || s.counter(id::THREE) {
        s.sz_start(id::THREE, 10);
        let mut owners = [0; 3];
        for (k, o) in (0..).zip(owners.iter_mut()) {
            let p = s.b_search(id::THREE + k, 0, w);
            *o = w.planet_owner(p);
        }
        if owners[0] != 0 && owners[0] == owners[1] && owners[1] == owners[2] {
            w.event(StoryEvent::Notice(Notice::Winner { empire: owners[0], amazing: false }));
            s.win(0, true, w);
        }
    }
    Ok(())
}

/// 1004, 1006: nothing (the exe only marks them checked).
fn empty(_: &mut Story, _: &mut dyn StoryWorld) -> Result<(), Abort> {
    Ok(())
}

/// 1005: empires 1 and 2 each have a base; a fleet that reaches the other's base takes its flag (a "Flag
/// Carrier" ship), one that brings a carrier home scores; lost carriers return the flag.
fn capture_the_flag(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    let mut changed = false;
    if s.starting_now() {
        if s.b_search(id::BASE, 1, w) == 0 {
            let (x, y) = empire_centre(1, w);
            let a = nearest_planet(x as i32, y as i32, 0, w);
            s.b_add_id(a, id::BASE);
            w.event(StoryEvent::RenamePlanet { planet: a, name: "Base 1" });
            let (x, y) = empire_centre(2, w);
            let mut b = nearest_planet(x as i32, y as i32, 0, w);
            if b == a {
                b = s.b_search_try(0, 0, w)?;
            }
            s.b_add_id(b, id::BASE + 1);
            w.event(StoryEvent::RenamePlanet { planet: b, name: "Base 2" });
        }
        if s.v_get(id::BASE) == 0 {
            // The original writes its local `EN` (each machine its own); canonical multiplayer the owner's number.
            let (r1, r2) = if w.canonical() { (flag_carrier(1), flag_carrier(2)) } else { (flag_carrier(en), flag_carrier(en)) };
            let d = w.new_design(1, &r1);
            s.v_set(id::BASE, d);
            let d = w.new_design(2, &r2);
            s.v_set(id::BASE + 1, d);
        }
        s.b_show(id::BASE, -1, w);
        s.b_show(id::BASE + 1, -1, w);
        s.v_set(id::SCORE, 0);
        s.v_set(id::SCORE + 1, 0);
        if s.v_get(id::POINTS) == 0 {
            s.v_set(id::POINTS, 250);
        }
        if s.v_get(id::STEP) == 0 {
            s.v_set(id::STEP, 50);
        }
        s.add_info_line(id::BASE, InfoLine::Blank, -1, w);
        changed = true;
    }
    // A base can't be colonised: it goes back to owner 0.
    if s.special_is(10, 0, 0, 0) {
        let p = s.special[2];
        if p == s.b_search(id::BASE, 0, w) || p == s.b_search(id::BASE + 1, 0, w) {
            if s.special[1] == en {
                w.event(StoryEvent::Notice(Notice::CannotColonise(p)));
            }
            w.event(StoryEvent::SetPlanetOwner { planet: p, owner: 0, race: w.race(0) });
        }
    }
    if s.flags[flag::ARRIVED] && s.arrival.target_type == 1 {
        let target = s.arrival.target;
        let which = if s.b_search(id::BASE, 0, w) == target {
            1
        } else if s.b_search(id::BASE + 1, 0, w) == target {
            2
        } else {
            0
        };
        let fleet = s.arrival.fleet;
        let o = w.fleet_owner(fleet);
        // The other's base reached: its flag is taken (`x` = the taker).
        let x = if s.v_get(id::HOLDS + 1) == 0 && which == 1 && o == 2 {
            2
        } else if s.v_get(id::HOLDS) == 0 && which == 2 && o == 1 {
            1
        } else {
            0
        };
        if x != 0 {
            if en == x || en == which {
                w.event(StoryEvent::Notice(Notice::FlagTaken { by: x, from: which }));
            }
            s.v_set(id::BASE + 1 + x, 1);
            let design = s.v_get(id::BASE - 1 + x);
            w.event(StoryEvent::NewShip { fleet, design });
            changed = true;
        } else if which != 0 {
            // Home with a carrier: a touchdown.
            let carriers = [0, s.v_get(id::BASE), s.v_get(id::BASE + 1)];
            let mut ship = 0;
            if let Some(&want) = carriers.get(o as usize) {
                for k in 1..=w.ship_count() {
                    if w.ship(k) == (fleet, want) {
                        ship = k;
                    }
                }
            }
            if ship != 0 {
                let y = if s.v_get(id::HOLDS + 1) == 0 && which == 1 && o == 1 {
                    1
                } else if s.v_get(id::HOLDS) == 0 && which == 2 && o == 2 {
                    2
                } else {
                    0
                };
                if y != 0 {
                    if en == y || en == 3 - y {
                        w.event(StoryEvent::Notice(Notice::Touchdown { by: y }));
                    }
                    s.v_set(id::BASE + 1 + y, 0);
                    w.event(StoryEvent::DeleteShip(ship));
                    let touchdowns = s.v_get(id::SCORE + 1 + y);
                    let step = s.v_get(id::STEP);
                    let score = s.v_get(id::SCORE - 1 + y);
                    let points = s.v_get(id::POINTS);
                    s.v_set(id::SCORE - 1 + y, touchdowns.wrapping_mul(step).wrapping_add(score).wrapping_add(points));
                    let t = s.v_get(id::SCORE + 1 + y);
                    s.v_set(id::SCORE + 1 + y, t.wrapping_add(1));
                    changed = true;
                }
            }
        }
    }
    // A space battle with empire 1 or 2: points for the attack the other side lost, flags of lost carriers
    // go home.
    let owner = |s: &Story, i: usize| s.battle.get(i).map_or(0, |p| p.owner);
    let third = if s.battle.len() > 2 { owner(s, 2) } else { 0 };
    let sides = [owner(s, 0), owner(s, 1), third];
    if s.flags[flag::SPACE_END] && (sides.contains(&1) || sides.contains(&2)) {
        let part = |e: i32| if sides[0] == e { 1 } else if sides[1] == e { 2 } else { 0 };
        let (a, b) = (part(1), part(2));
        let holds = [s.v_get(id::HOLDS), s.v_get(id::HOLDS + 1)];
        let designs = [s.v_get(id::BASE), s.v_get(id::BASE + 1)];
        let mut carrier = [false, false];
        for k in 1..=w.ship_count() {
            let (f, d) = w.ship(k);
            let o = w.fleet_owner(f);
            carrier[0] |= d == designs[0] && o == 1;
            carrier[1] |= d == designs[1] && o == 2;
        }
        if a != 0 {
            let v = s.v_get(id::SCORE);
            s.v_set(id::SCORE, v.wrapping_add(w.ctf_value(3 - a)));
            changed = true;
        }
        if b != 0 {
            let v = s.v_get(id::SCORE + 1);
            s.v_set(id::SCORE + 1, v.wrapping_add(w.ctf_value(3 - b)));
            changed = true;
        }
        for e in 0..2 {
            if holds[e] != 0 && !carrier[e] {
                s.v_set(id::HOLDS + e as i32, 0);
                if en == 1 || en == 2 {
                    w.event(StoryEvent::Notice(Notice::FlagLost { by: e as i32 + 1 }));
                }
            }
        }
    }
    if changed {
        s.remove_game_info(id::BASE, w);
        // The exe's read order (the last read sets `StoryError`).
        let f2 = s.v_get(id::HOLDS + 1);
        let t2 = s.v_get(id::TOUCHDOWNS + 1);
        let s2 = s.v_get(id::SCORE + 1);
        let f1 = s.v_get(id::HOLDS);
        let t1 = s.v_get(id::TOUCHDOWNS);
        let s1 = s.v_get(id::SCORE);
        let line = InfoLine::Score { score: [s1, s2], touchdowns: [t1, t2], flag: [f1 != 0, f2 != 0] };
        s.add_info_line(id::BASE, line, -1, w);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Streams;

    /// Human 6 (`EN`); planets 1..=3 owned by 6, 3 and nobody; fleets of 3 and 2 near planet 1, of 6 near planet 2,
    /// of 2 far away.
    struct W {
        canonical: bool,
        rng: Streams,
        designs: Vec<(i32, i16)>,
        events: Vec<StoryEvent>,
    }

    const OWNERS: [i32; 4] = [0, 6, 3, 0];
    const PLANETS: [(f32, f32); 4] = [(0.0, 0.0), (100.0, 100.0), (500.0, 500.0), (900.0, 100.0)];
    const FLEETS: [(i32, (f32, f32)); 5] = [(0, (0.0, 0.0)), (3, (120.0, 100.0)), (6, (520.0, 500.0)), (2, (100.0, 130.0)), (2, (900.0, 900.0))];

    impl StoryWorld for W {
        fn ido(&self) -> i32 {
            0
        }
        fn speed_mult(&self) -> i32 {
            1
        }
        fn human(&self) -> i32 {
            6
        }
        fn canonical(&self) -> bool {
            self.canonical
        }
        fn difficulty(&self) -> i32 {
            1
        }
        fn multiplayer(&self) -> bool {
            true
        }
        fn rng(&mut self) -> &mut Streams {
            &mut self.rng
        }
        fn planet_count(&self) -> i32 {
            3
        }
        fn planet_owner(&self, p: i32) -> i32 {
            OWNERS[p as usize]
        }
        fn planet_value(&self, _p: i32) -> i32 {
            0
        }
        fn planet_visibility(&self, _p: i32) -> i32 {
            1
        }
        fn planet_xy(&self, p: i32) -> (f32, f32) {
            PLANETS[p as usize]
        }
        fn fleet_count(&self) -> i32 {
            FLEETS.len() as i32 - 1
        }
        fn fleet_owner(&self, f: i32) -> i32 {
            FLEETS[f as usize].0
        }
        fn fleet_xy(&self, f: i32) -> (f32, f32) {
            FLEETS[f as usize].1
        }
        fn fleet_id(&self, _f: i32) -> i32 {
            0
        }
        fn empire_alive(&self, _e: i32) -> bool {
            true
        }
        fn planets_of(&self, _e: i32) -> i32 {
            1
        }
        fn money(&self, _e: i32) -> f64 {
            0.0
        }
        fn tech_status(&self, _t: i32, _e: i32) -> i32 {
            0
        }
        fn relation_level(&self, _a: i32, _b: i32) -> i32 {
            3
        }
        fn new_design(&mut self, e: i32, record: &[u8; DESIGN]) -> i32 {
            self.designs.push((e, i16::from_le_bytes([record[0x40], record[0x41]])));
            self.designs.len() as i32
        }
        fn event(&mut self, ev: StoryEvent) {
            self.events.push(ev);
        }
    }

    fn world(canonical: bool) -> W {
        W { canonical, rng: Streams::default(), designs: Vec::new(), events: Vec::new() }
    }

    fn three() -> Story {
        let mut s = Story::default();
        for k in 0..3 {
            s.b_add_id(k + 1, id::THREE + k);
        }
        s
    }

    /// The purchase coming back from the lockstep queue (special 15 from `buyer`).
    fn buy(s: &mut Story, w: &mut W, buyer: i32) {
        s.flags[flag::SPECIAL] = true;
        s.special = [15, buyer, id::THREE, 0, 0, 0, 0, 0];
        three_planets(s, w).unwrap();
        s.flags[flag::SPECIAL] = false;
    }

    /// The radar counter firing: the fleets hidden and shown.
    fn radar(s: &mut Story, w: &mut W) -> (Vec<i32>, Vec<i32>) {
        w.events.clear();
        s.flags[flag::COUNTER] = true;
        s.counter_done = id::RADAR;
        three_planets(s, w).unwrap();
        s.flags[flag::COUNTER] = false;
        let pick = |on: bool| -> Vec<i32> {
            w.events.iter().filter_map(|e| match e {
                StoryEvent::SetFleetVisible { fleet, on: o } if *o == on => Some(*fleet),
                _ => None,
            }).collect()
        };
        (pick(false), pick(true))
    }

    #[test]
    fn classic_radar_is_the_local_humans_only() {
        let (mut s, mut w) = (three(), world(false));
        buy(&mut s, &mut w, 3);
        assert!(s.counters.get(id::RADAR).is_none(), "another player's purchase starts nothing here");
        buy(&mut s, &mut w, 6);
        assert!(s.sz_get(id::RADAR) > 0);
        assert!(s.vars.get(id::RADAR_BUYERS).is_none(), "classic mode leaves the story tables alone");
        // Every fleet but the human's is hidden; the two enemies near its planet show.
        assert_eq!(radar(&mut s, &mut w), (vec![1, 3, 4], vec![1, 3]));
    }

    #[test]
    fn canonical_radar_runs_for_every_buyer() {
        let (mut s, mut w) = (three(), world(true));
        buy(&mut s, &mut w, 3);
        assert_eq!(s.v_get(id::RADAR_BUYERS), 1 << 3);
        assert!(s.sz_get(id::RADAR) > 0);
        // Buyer 3 alone: everything not of 3 hidden, 6's fleet near 3's planet shown.
        assert_eq!(radar(&mut s, &mut w), (vec![2, 3, 4], vec![2]));
        buy(&mut s, &mut w, 6);
        assert_eq!(s.v_get(id::RADAR_BUYERS), (1 << 3) | (1 << 6));
        // Both: the buyers' fleets stay, each planet shows its owner's enemies.
        assert_eq!(radar(&mut s, &mut w), (vec![3, 4], vec![1, 3, 2]));
    }

    #[test]
    fn flag_carriers_carry_their_owner_in_canonical_mode() {
        for (canonical, want) in [(false, [(1, 6), (2, 6)]), (true, [(1, 1), (2, 2)])] {
            let (mut s, mut w) = (Story::default(), world(canonical));
            s.b_add_id(1, id::BASE);
            s.b_add_id(3, id::BASE + 1);
            s.last_checked = 1005;
            s.buckets[1005 & 0xff].add(1005, 1);
            capture_the_flag(&mut s, &mut w).unwrap();
            assert_eq!(w.designs, want);
        }
    }
}
