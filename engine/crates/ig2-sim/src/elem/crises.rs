//! `st_Elem_include`, part 2d: 801 (the evacuation of the supernova planet), 804 (the pirates' bases), 806 (the police
//! strike), 807 (the pirates' ultimatum) and 809 (empire 13's blockade). Spec: `re/rules/campaign.md` §8.14, part 2d.

use super::{answered, param, promoted, read, supernova_planets};
use crate::brnd::{distance, nearest_fleet, ship_fleet};
use crate::multi::nearest_planet;
use crate::story::{flag, last_fn, Abort, InfoLine, MessageText, Story, StoryEvent, StoryWorld, DESIGN, STREAM};

fn message(w: &mut dyn StoryWorld, id: i32, arg: i32) {
    w.event(StoryEvent::NewMessage { id, arg });
}

/// `st_ViszonyValt(EN, e, x)`: `Viszony[e][EN].+4 += 100·x`.
fn relation(w: &mut dyn StoryWorld, e: i32, x: i32) {
    let a = w.human();
    w.event(StoryEvent::RelationChange { a, b: e, delta: 100 * x });
}

/// `st_ViszonyMindenkivel(x)` (exe 0x6030c0): every empire 1..=8 but the human and **5** (the exe's unrolled loop has
/// no 5) gets `rel(e, x)`.
fn relation_all(w: &mut dyn StoryWorld, x: i32) {
    let en = w.human();
    for e in [1, 2, 3, 4, 6, 7, 8] {
        if e != en {
            relation(w, e, x);
        }
    }
}

/// `st_BolygoDestroy(b(id))`: the planet is wiped out (`StoryError` = no planet).
fn destroy(s: &mut Story, id: i32, w: &mut dyn StoryWorld) {
    let p = s.b_search(id, 0, w);
    s.error = (p < 1) as i32;
    w.event(StoryEvent::DestroyPlanet(p));
}

fn add_adat(s: &mut Story, off: usize, x: i32) {
    let v = s.adat_i32(off).wrapping_add(x);
    s.set_adat_i32(off, v);
}

/// `st_fNewPos(name, owner, id, x, y, speed)` as the exe inlines it: `FlottaNew` at the truncated point, then
/// `AutoAddship(f, id)`.
fn fleet_at(s: &mut Story, owner: i32, id: i32, x: f32, y: f32, speed: f32, w: &mut dyn StoryWorld) -> i32 {
    let f = w.new_fleet("", owner, id, x as i32 as f32, y as i32 as f32, speed);
    s.auto_add_ship(f, id, w);
    f
}

/// The building slots 1..=`HanyEpuletLefoglalva[p]` as `(slot, kind, intact, flags +0x19)`.
fn slots(p: i32, w: &dyn StoryWorld) -> Vec<(i32, i32, bool, i32)> {
    (1..=w.building_count(p))
        .map(|k| {
            let (kind, _, built) = w.building(p, k);
            (k, kind, built as i16 == 0, w.building_bytes(p, k).1)
        })
        .collect()
}

fn set_flags(p: i32, k: i32, value: i32, w: &mut dyn StoryWorld) {
    w.event(StoryEvent::BuildingByte { planet: p, slot: k, offset: 0x19, value });
}

// ---------------------------------------------------------------- 801: the evacuation

/// Story design keys of 801's two designs (`+0x80`).
const KEY_A: i32 = 9998;
const KEY_B: i32 = 9999;
/// `StoryAdat` offsets holding their slots.
const DESIGN_A: usize = 0x3c;
const DESIGN_B: usize = 0x64;

/// 801's design record as the exe builds it on the stack: name `_s(5, id)`, `+0x18` 1 for A, 0 for B; the padding bytes
/// (`multi::FLAG_CARRIER_UNSET`) are the stack's (0 here).
pub fn refugee_design(name: &[u8], first: bool) -> [u8; DESIGN] {
    let mut r = [0u8; DESIGN];
    r[0x16] = 1;
    r[0x18] = first as u8;
    r[0x19] = 1;
    r[0x1a] = 1;
    r[0x20..0x24].copy_from_slice(&15_000i32.to_le_bytes());
    r[0x24..0x28].copy_from_slice(&300i32.to_le_bytes());
    r[0x3c..0x40].copy_from_slice(&0.1f32.to_le_bytes());
    r[0x40..0x42].copy_from_slice(&6i16.to_le_bytes());
    r[0x44..0x48].copy_from_slice(&12i32.to_le_bytes());
    r[0x48..0x4c].copy_from_slice(&12i32.to_le_bytes());
    // `strcpy` over the template: the name and its NUL.
    let n = name.len().min(DESIGN - 1);
    r[..n].copy_from_slice(&name[..n]);
    r[n] = 0;
    r
}

/// The two designs of the human's (`TervNew`, the record, `TervUjraSzamol`, then the bytes the script writes).
fn new_designs(s: &mut Story, w: &mut dyn StoryWorld) {
    let en = w.human();
    for (first, text, off, key) in [(true, 8010, DESIGN_A, KEY_A), (false, 8011, DESIGN_B, KEY_B)] {
        let name = w.story_text(text);
        let d = w.new_design(en, &refugee_design(&name, first));
        s.set_adat_i32(off, d);
        w.event(StoryEvent::DesignByte { empire: en, design: d, offset: 0x34, value: 0 });
        w.event(StoryEvent::SetDesignKey { empire: en, design: d, key });
        if first {
            w.event(StoryEvent::DesignByte { empire: en, design: d, offset: 0x28, value: 1 });
        }
    }
}

/// Planet 8010 is gone: the end of the evacuation (with `delivered`, every refugee brought away).
fn supernova(s: &mut Story, w: &mut dyn StoryWorld) {
    destroy(s, 8010, w);
    s.remove_game_info(8011, w);
    s.add_info_line(8012, InfoLine::Blank, -1, w);
    if w.campaign_type() != 1 {
        w.event(StoryEvent::Anim(8011));
    }
}

/// 801 (exe block 0x5d4bb7): the supernova planet 8010's people are carried away in the human's transports (design
/// A, loaded as design B at 8010) to new colonies or planets of the human's before the counter 8001 runs out.
pub(super) fn story_801(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        s.add_colonizers(3, w);
        s.v_set(8019, 2);
        supernova_planets(s, false, w)?;
        message(w, 5001, 180);
        param(w, 1, MessageText::Empire(en));
        let p = s.b_search(8010, 0, w);
        param(w, 2, MessageText::Planet(p));
    }
    if read(s, 5001) {
        s.b_search(8010, 0, w);
        s.b_search(8010, 0, w);
        let days = [0, 1200, 1000, 1000, 600][w.difficulty().clamp(0, 4) as usize];
        s.sz_start(8001, days);
        s.sz_start(8002, days + 200);
        s.b_show(8010, -1, w);
        s.b_set_friendly(8010, true, w);
        s.b_set_friendly(8011, true, w);
        new_designs(s, w);
        s.v_set(8009, 1);
    }
    if promoted(s, 5001) {
        s.sz_start(8001, 1);
        s.v_set(8009, 1);
    }
    // The transports reach 8010: up to `v(8019)` ships of design A are loaded (design B).
    if s.arrived_planet(0, 8010, w) && w.fleet_owner(s.arrival.fleet) == en && s.v_get(8009) != 0 {
        let mut taken = 0;
        let mut n = s.v_get(8019);
        let (a, b) = (s.v_get(8017), s.v_get(8018));
        let design = s.adat_i32(DESIGN_A);
        if n != 0 {
            for k in 1..=w.ship_count() {
                let (f, d) = w.ship(k);
                if d == design && f == s.arrival.fleet && n != 0 && k != a && k != b {
                    taken += 1;
                    s.v_set(8019 - n, k);
                    n -= 1;
                    let p = s.b_search(8010, 0, w);
                    w.event(StoryEvent::SetPlanetValue { planet: p, value: if n != 0 { 6800 } else { 300 } });
                    if w.fleet_flags(f) & 0x800 == 0 {
                        w.event(StoryEvent::FleetFlags { fleet: f, set: 0x800, clear: 0 });
                        w.event(StoryEvent::NameFleet { fleet: f, text: Some(8010) });
                    }
                    let design = s.adat_i32(DESIGN_B);
                    w.event(StoryEvent::UpgradeShip { ship: k, design });
                }
            }
        }
        if taken != 0 {
            s.v_set(8001, 1);
            let n = n.max(0);
            s.v_set(8019, n);
            if n == 0 {
                message(w, 8008, 0);
                let q = s.b_search(8011, 0, w);
                param(w, 3, MessageText::Planet(q));
                relation_all(w, 10);
                supernova(s, w);
                let q = s.b_search(8011, 0, w);
                if w.planet_owner(q) == 16 {
                    let q = s.b_search(8011, 0, w);
                    s.set_planet_owner(q, en, w);
                    s.b_set_friendly(8011, false, w);
                    let q = s.b_search(8011, 0, w);
                    w.event(StoryEvent::AddPlanetInfo { planet: q, info: 8001 });
                }
                for e in 1..9 {
                    if e != en {
                        relation(w, e, 3);
                    }
                }
                s.finished.add(801, 2);
                s.sz_stop(8001);
            } else {
                message(w, 8002, 0);
            }
        }
    }
    // A loaded transport reaches a planet of the human's (or a free one) it sees: the refugees land.
    let t = s.arrival.target;
    if s.flags[flag::ARRIVED]
        && s.arrival.target_type == 1
        && w.fleet_owner(s.arrival.fleet) == en
        && w.planet_visibility(t) >= 0
        && (w.planet_owner(t) == en || w.planet_owner(t) == 0)
    {
        let (a0, b0) = (s.v_get(8017), s.v_get(8018));
        let own = w.planet_owner(t) == en;
        let design = s.adat_i32(DESIGN_B);
        if a0 != 0 || b0 != 0 {
            let (mut a, mut b) = (a0, b0);
            for k in 1..=w.ship_count() {
                let (f, d) = w.ship(k);
                if d != design || f != s.arrival.fleet || !(k == a || k == b) {
                    continue;
                }
                let x = if k == a {
                    s.v_set(8017, 0);
                    8000
                } else {
                    s.v_set(8018, 0);
                    6800
                };
                // Fleet slot `k` (the ship's index, not its fleet) loses the transports' name.
                if (a0 == 0 || b0 == 0 || w.ship(a0).0 != w.ship(b0).0) && w.fleet_named(k, 8010) {
                    w.event(StoryEvent::NameFleet { fleet: k, text: None });
                    w.event(StoryEvent::FleetFlags { fleet: k, set: 0, clear: 0x800 });
                }
                (a, b) = (0, 0);
                let value = w.planet_value(t).wrapping_add(x);
                if own {
                    let back = s.adat_i32(DESIGN_A);
                    if s.v_get(8015) == t {
                        w.event(StoryEvent::SetPlanetValue { planet: t, value });
                        s.v_set(8001, 2);
                        w.event(StoryEvent::UpgradeShip { ship: k, design: back });
                    } else {
                        let was = s.v_get(8015);
                        s.v_set(8015, t);
                        w.event(StoryEvent::SetPlanetValue { planet: t, value });
                        s.v_set(8001, 2);
                        message(w, if was == 0 { 8004 } else { 8003 }, 0);
                        param(w, 1, MessageText::Planet(t));
                        w.event(StoryEvent::UpgradeShip { ship: k, design: back });
                        w.event(StoryEvent::AddPlanetInfo { planet: t, info: 8002 });
                    }
                } else {
                    message(w, 8005, 0);
                    s.v_set(8001, 2);
                    s.v_set(8015, t);
                    // `st_Colonise(T, EN, 11)`.
                    if w.planet_owner(t) != en {
                        w.event(StoryEvent::ColoniseAs { planet: t, owner: en, race: 11 });
                    }
                    for kind in [2, 2, 5, 6, 0x34] {
                        w.event(StoryEvent::PlaceBuildingUnbuilt { planet: t, kind });
                    }
                    w.event(StoryEvent::DeleteShip(k));
                    w.event(StoryEvent::SetPlanetValue { planet: t, value: x });
                    w.event(StoryEvent::AddPlanetInfo { planet: t, info: 8001 });
                }
            }
        }
        if s.v_get(8017) == 0 && s.v_get(8018) == 0 {
            s.remove_game_info(8012, w);
        }
    }
    if s.counter(8001) {
        s.finished.add(801, 2);
        if s.v_get(8001) == 1 {
            message(w, 8007, 0);
            let p = s.b_search(8010, 0, w);
            param(w, 1, MessageText::Planet(p));
            param(w, 2, MessageText::Empire(en));
            let q = s.b_search(8011, 0, w);
            param(w, 3, MessageText::Planet(q));
            let q = s.b_search(8011, 0, w);
            if w.planet_owner(q) == 16 {
                let q = s.b_search(8011, 0, w);
                s.set_planet_owner(q, en, w);
                s.b_set_friendly(8011, false, w);
            }
        } else {
            message(w, 8009, 0);
            s.b_set_friendly(8011, false, w);
        }
        supernova(s, w);
    }
    // The loaded ships still out: gone ones (free slot) are forgotten.
    let forget = |s: &mut Story, w: &mut dyn StoryWorld| {
        let (mut a, mut b) = (s.v_get(8017), s.v_get(8018));
        if a != 0 || b != 0 {
            if w.ship(a).1 == 0x7fff {
                a = 0;
                s.v_set(8017, 0);
            }
            if w.ship(b).1 == 0x7fff {
                b = 0;
                s.v_set(8018, 0);
            }
        }
        (a, b)
    };
    if s.counter(8002) {
        let (a, b) = forget(s, w);
        if a == 0 && b == 0 {
            s.win(0, true, w);
        } else {
            message(w, 8010, 0);
            s.sz_start(8003, 30);
        }
    }
    if s.counter(8003) {
        let (a, b) = forget(s, w);
        if a != 0 || b != 0 {
            if a != 0 {
                w.event(StoryEvent::DeleteShip(a));
            }
            if b != 0 {
                w.event(StoryEvent::DeleteShip(b));
            }
            message(w, 8011, 0);
        }
        s.win(0, true, w);
    }
    Ok(())
}

// ---------------------------------------------------------------- 804: the pirates' bases

impl Story {
    /// `st_fMarkNearPlanet(bid, id)` (exe 0x604a90): the idle fleets without a story id of planet `bid`'s owner standing
    /// exactly on it get id `id`.
    pub fn mark_near_planet(&mut self, bid: i32, id: i32, w: &mut dyn StoryWorld) {
        let p = self.b_search(bid, 0, w);
        let owner = w.planet_owner(p);
        let (px, py) = w.planet_xy(p);
        for f in 1..=w.fleet_count() {
            #[allow(clippy::float_cmp)]
            let here = w.fleet_xy(f) == (px, py);
            if w.fleet_owner(f) == owner && w.fleet_command(f) == 0 && w.fleet_id(f) == 0 && here {
                w.event(StoryEvent::SetFleetId { fleet: f, id });
            }
        }
    }
}

/// `OsszevonasCheck(id)` (exe 0x5c3100): every idle live fleet with story id `id` merges into the first later idle fleet
/// of its owner within `dy² + dx² < 10` (f32).
fn merge_check(id: i32, w: &mut dyn StoryWorld) {
    let mut f = 1;
    while f <= w.fleet_count() {
        let owner = w.fleet_owner(f);
        if owner != 0xff && w.fleet_command(f) == 0 && w.fleet_id(f) == id {
            let mut g = f + 1;
            while g <= w.fleet_count() {
                if w.fleet_owner(g) == owner && w.fleet_command(g) == 0 {
                    let ((fx, fy), (gx, gy)) = (w.fleet_xy(f), w.fleet_xy(g));
                    let (dx, dy) = (fx - gx, fy - gy);
                    if dy * dy + dx * dx < 10.0 {
                        w.event(StoryEvent::MergeFleets { into: g, from: f });
                        break;
                    }
                }
                g += 1;
            }
        }
        f += 1;
    }
}

/// 804 (exe block 0x5d6de5): three bases of empire 10 (8040..=8042, given to it by `StoryInit804`) send raiders against
/// the human; the bases taken, the end.
pub(super) fn story_804(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    const RAIDERS: i32 = 8049;
    let en = w.human();
    if s.starting_now() {
        for id in [8040, 8041] {
            let p = s.b_search(id, 0, w);
            s.f_new_ex("", 10, id, 0.06, p.wrapping_neg(), 5, w);
        }
        s.f_add_ship(8041, 3, 1, w);
        s.f_add_ship(8041, 3, 1, w);
        s.attack_planet(8040, en.wrapping_neg(), w);
        s.attack_planet(8041, en.wrapping_neg(), w);
        for id in 8040..=8042 {
            s.auto_add_tank(id, w);
            s.auto_add_building(id, w);
        }
        s.sz_start(8044, 100);
        s.sz_start(8045, 500);
        s.sz_start(8046, 350);
        s.sz_start(RAIDERS, 3);
        s.add_info_line(8041, InfoLine::Blank, -1, w);
        if w.difficulty() == 1 {
            let p = s.b_search(8040, 0, w);
            w.event(StoryEvent::SetPlanetView { planet: p, view: 1 });
        }
    }
    if s.counter(8044) || s.counter(8045) || s.counter(8046) {
        let c = s.counter_done;
        let p = s.b_search(c - 4, 0, w);
        if w.planet_owner(p) == 10 {
            let id = w.rng().range(STREAM, 8042, 8044);
            let f = s.f_new_ex("", 10, id, 0.05, 2, p, w);
            w.event(StoryEvent::SetFleetId { fleet: f, id: RAIDERS });
            let q = s.b_search(en.wrapping_neg(), 0, w);
            if q != 0 {
                w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 1, x: q as f32, y: 0.0 });
            }
            s.sz_start(c, 1000);
        }
    }
    if s.counter(RAIDERS) {
        s.sz_start(RAIDERS, 3);
        for k in 0..3 {
            let p = s.b_search(8040 + k, 0, w);
            if w.planet_owner(p) == 10 {
                s.mark_near_planet(8040 + k, RAIDERS, w);
            }
        }
        merge_check(RAIDERS, w);
        let mut f = 1;
        while f <= w.fleet_count() {
            if w.fleet_id(f) == RAIDERS && w.fleet_owner(f) == 10 && w.fleet_command(f) == 0 {
                if w.fleet_5c(f) == 0 {
                    let (x, y) = w.fleet_xy(f);
                    let g = nearest_fleet(x as i32, y as i32, en, &*w);
                    if g != 0 {
                        w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 2, x: g as f32, y: 0.0 });
                    }
                } else {
                    let q = s.b_search(en.wrapping_neg(), 0, w);
                    if q != 0 {
                        w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 1, x: q as f32, y: 0.0 });
                    }
                }
            }
            f += 1;
        }
    }
    if s.ground_part(0, 0, w) && s.v_get(RAIDERS) == 0 {
        let free = |s: &mut Story, id: i32, w: &mut dyn StoryWorld| {
            let p = s.b_search(id, 0, w);
            w.planet_owner(p) != 10
        };
        if free(s, 8040, w) && free(s, 8041, w) && free(s, 8042, w) {
            s.finished.add(804, 2);
            s.remove_game_info(8041, w);
            let mut p = s.b_search(8040, 0, w);
            for id in 8040..8043 {
                let q = s.b_search(id, 0, w);
                if q == s.ground.planet {
                    p = q;
                }
            }
            s.b_add_id(p, 8044);
            s.v_set(RAIDERS, 1);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 806: the police strike

/// The police strike is over on `p`: every slot's strike bit (`+0x19 & 0x10`) cleared.
fn end_strike(p: i32, w: &mut dyn StoryWorld) {
    for (k, _, _, fl) in slots(p, &*w) {
        set_flags(p, k, fl & !0x10, w);
    }
}

/// 806 (exe block 0x5d7387): the police of a big planet of the human's go on strike (id 8060) and the strike spreads
/// (`C(8060)`); two working police stations end it (a tech of 0x71..=0x78 for the human), else the planet goes to
/// empire 10 (`C(8061)`); a second strike may follow.
pub(super) fn story_806(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        s.v_set(8068, 0);
        s.v_set(8067, 0);
        s.sz_start(8069, 10);
    }
    if s.counter(8069) {
        // A planet of the human's it sees (not 854's riot planet) with more than 10 intact buildings, a police station
        // and (shields, or the last pick had none): the `n`-th candidate takes the pick on `rnd(0, 1, n) ≤ 1`
        // (stream 0).
        let riot = s.b_search(8540, 1, w);
        let (mut n, mut pick, mut last_shields) = (0, 0, 0);
        for p in 1..=w.planet_count() {
            if w.planet_owner(p) != en || w.planet_visibility(p) <= 0 || p == riot {
                continue;
            }
            let (mut intact, mut police, mut shields) = (0, 0, 0);
            for (_, kind, ok, _) in slots(p, &*w) {
                if kind != 0 && ok {
                    if kind == 7 {
                        police += 1;
                    } else if (0x20..0x24).contains(&kind) {
                        shields += 1;
                    }
                    intact += 1;
                }
            }
            if intact > 10 && police != 0 && (shields != 0 || last_shields == 0) {
                n += 1;
                if w.rng().range(0, 1, n) <= 1 {
                    last_shields = shields;
                    pick = p;
                }
            }
        }
        if pick == 0 {
            s.sz_start(8069, 90);
        } else {
            s.b_add_id(pick, 8060);
            for (k, kind, ok, fl) in slots(pick, &*w) {
                if (kind == 7 || (0x20..0x24).contains(&kind)) && ok {
                    set_flags(pick, k, fl | 0x10, w);
                }
            }
            if s.v_get(8067) == 0 {
                message(w, 6820, 0);
                w.event(StoryEvent::Anim(6820));
            } else {
                message(w, 8063, 0);
            }
            let p = s.b_search(8060, 0, w);
            param(w, 1, MessageText::Planet(p));
            s.b_show(8060, -1, w);
            let days = w.rng().range(STREAM, 20, 40);
            s.sz_start(8060, days);
            s.v_set(8068, 1);
            s.add_info_line(8061, InfoLine::Planet { planet: pick, count: None }, -1, w);
            s.b_set_extra(8060, true, w);
            let p = s.b_search(8060, 0, w);
            w.event(StoryEvent::PlanetExtraFlag { planet: p, on: false });
        }
    }
    if s.daily() && s.v_get(8068) != 0 {
        let p = s.b_search(8060, 0, w);
        if w.planet_owner(p) == en {
            let working = slots(p, &*w).iter().filter(|&&(_, kind, ok, fl)| kind == 7 && ok && fl & 1 != 0 && fl & 0x10 == 0).count();
            if working > 1 {
                end_strike(p, w);
                message(w, 8061, 0);
                let open = |t: &i32| w.tech_status(*t, en) != 5;
                let t = (0x75..0x79).find(open).or_else(|| (0x71..0x75).find(open)).unwrap_or(0x75);
                s.give_tech(t, w);
                param(w, 1, MessageText::Tech(t));
                // `st_BolygoMoralValt(p, 20)`, inlined.
                w.event(StoryEvent::PlanetMorale { planet: p, delta: -200 });
                s.win(0, true, w);
            }
        } else {
            message(w, 8068, 0);
            s.remove_game_info(8061, w);
            w.event(StoryEvent::RemovePlanetInfo { planet: p, info: -8060 });
            s.b_set_extra(8060, false, w);
            s.b_remove_id(8060);
            s.v_set(8068, 0);
            s.remove_game_info(8061, w);
            if s.v_get(8067) == 0 {
                s.v_set(8067, 1);
                s.sz_start(8069, 360);
            } else {
                s.win(0, true, w);
            }
        }
    }
    if s.counter(8060) {
        // Twice: a working building (not the police, not kind 1) joins the strike.
        let p = s.b_search(8060, 0, w);
        for _ in 0..2 {
            let (mut n, mut pick) = (0, None);
            for (k, kind, ok, fl) in slots(p, &*w) {
                if kind != 0 && ok && fl & 0x10 == 0 && kind != 7 && kind != 1 {
                    n += 1;
                    if w.rng().range(STREAM, 1, n) <= 1 {
                        pick = Some((k, fl));
                    }
                }
            }
            if let Some((k, fl)) = pick {
                set_flags(p, k, fl | 0x10, w);
            }
        }
        let all = slots(p, &*w);
        let struck = all.iter().filter(|&&(_, kind, ok, fl)| kind != 0 && ok && fl & 0x10 != 0).count() as i32;
        // Every slot counts (the exe divides by zero for a planet without slots).
        let total = (all.len() as i32).max(1);
        if struck * 100 / total < 70 {
            let d = Story::diff(w.difficulty(), 120, 80, 40, 20);
            let days = w.rng().range(STREAM, d, (d * 3) / 2);
            s.sz_start(8060, days);
        } else {
            s.sz_start(8061, 18);
        }
    }
    if s.counter(8061) {
        message(w, 8062, 0);
        let p = s.b_search(8060, 0, w);
        param(w, 1, MessageText::Planet(p));
        end_strike(p, w);
        s.set_planet_owner(p, 10, w);
        s.b_set_extra(p.wrapping_neg(), false, w);
        w.event(StoryEvent::RemovePlanetInfo { planet: p, info: -8060 });
        s.remove_game_info(8061, w);
        s.v_set(8068, 0);
        s.b_remove_id(8060);
        if s.v_get(8067) != 0 {
            s.win(0, true, w);
        } else {
            s.v_set(8067, 1);
            s.sz_start(8069, 360);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 807: the pirates' ultimatum

/// 807 (exe block 0x5d7c83): the pirates ask the human to leave a planet (id 8070) alone; a small fleet of the human's
/// near it when the counter 8071 runs out is attacked (fleet 8070); beaten, the pirates offer terms (message 8074).
pub(super) fn story_807(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    let mut escort = false;
    if s.starting_now() {
        message(w, 6840, 0);
    }
    if answered(s, 6840, 4) {
        s.sz_start(8078, 30);
    }
    if answered(s, 6840, 5) || answered(s, 6840, 6) {
        s.win(0, true, w);
    }
    if s.counter(8078) {
        message(w, 6841, 0);
    }
    if answered(s, 6841, 3) {
        s.sz_start(8070, 360);
    }
    if answered(s, 6841, 4) {
        s.win(0, true, w);
    }
    if s.counter(8070) {
        let mut p = s.b_search(8070, 1, w);
        if p == 0 {
            let q = s.b_search(en.wrapping_neg(), 0, w);
            let (x, y) = w.planet_xy(q);
            p = nearest_planet(x as i32, y as i32, 0, &*w);
            s.b_add_id(p, 8070);
        }
        s.b_show(8070, -1, w);
        message(w, 8070, 0);
        param(w, 1, MessageText::Planet(p));
        s.sz_start(8071, 360);
        let [y, m, d] = w.date();
        s.add_info_line(8071, InfoLine::PlanetDate { planet: p, date: [y + 1, m, d] }, -1, w);
    }
    if s.counter(8071) {
        s.remove_game_info(8071, w);
        let p = s.b_search(8070, 0, w);
        let at = w.planet_xy(p);
        let (mut found, mut pick) = (false, 0);
        for f in 1..=w.fleet_count() {
            if w.fleet_owner(f) != en || distance(w.fleet_xy(f), at) >= 20 {
                continue;
            }
            found = true;
            // Its ships by their design's hull class (`+0x48`) and its vehicles (designs with `+0x36`).
            let (mut a, mut b, mut c, mut d, mut e) = (0, 0, 0, 0, 0);
            for k in 1..=w.ship_count() {
                let (sf, sd) = w.ship(k);
                if sf == f && sd != 0x7fff {
                    match w.design_hull(en, sd) {
                        5 | 6 => a += 1,
                        7 | 8 => b += 1,
                        11 => d += 1,
                        _ => e += 1,
                    }
                }
            }
            for k in 1..=w.tank_count() {
                let (place, design, count) = w.tank(k);
                if design != 0x7fff && place == f && w.design_36(en, design) != 0 {
                    c += count;
                }
            }
            if (a < 7 && b < 3 && c < 21 && d == 0 && e == 0) || (a == 0 && b < 3 && c < 21 && d < 7 && e == 0) {
                pick = f;
            }
        }
        if !found {
            message(w, 8071, 0);
            s.win(0, true, w);
        } else if pick == 0 {
            message(w, 8072, 0);
            s.win(0, true, w);
        } else {
            let (x, y) = w.fleet_xy(pick);
            let f = fleet_at(s, 10, 8070, x, y, 10.0, w);
            w.event(StoryEvent::MoveFleet { fleet: f, kind: 2, mode: 2, x: pick as f32, y: 0.0 });
        }
    }
    if s.battle_part(8070) != 0 {
        if s.battle_won(8070) {
            message(w, 8079, 0);
            s.win(0, true, w);
        } else {
            message(w, 8074, 0);
        }
        // `st_fDelete(8070)`, inlined.
        let f = s.f_search(8070, w);
        if f < 1 {
            s.error = 1;
        } else {
            w.event(StoryEvent::DeleteFleet(f));
            s.error = 0;
        }
    }
    if answered(s, 8074, 3) {
        relation(w, 10, 10);
        relation(w, 9, 12);
        s.v_set(8070, 1);
        escort = true;
        s.win(0, true, w);
    }
    if answered(s, 8074, 4) {
        escort = true;
        relation(w, 10, 10);
        relation(w, 9, 16);
        s.win(0, true, w);
    }
    if answered(s, 8074, 5) {
        relation(w, 10, 10);
        relation(w, 9, 12);
        s.sz_start(8074, 60);
    }
    if s.counter(8074) {
        let f = s.f_new_ex("", en, 8074, 0.1, 1, 0, w);
        let b = s.last_fn[last_fn::B];
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: b as f32, y: 0.0 });
        s.remove_fleet_id(8074, w);
        message(w, 8075, 0);
        s.sz_start(8079, 30);
    }
    let pirate_ship = |w: &dyn StoryWorld, key: i32| ship_fleet(w, en, key);
    if s.counter(8079) {
        if pirate_ship(&*w, 7801) == 0 && pirate_ship(&*w, 7811) == 0 && pirate_ship(&*w, 7821) == 0 {
            s.win(0, true, w);
        } else {
            s.sz_start(8079, 30);
        }
    }
    if s.flags[flag::SPACE_START] {
        let mut h = pirate_ship(&*w, 7801);
        if h == 0 {
            h = pirate_ship(&*w, 7811);
        }
        if h == 0 {
            h = pirate_ship(&*w, 7821);
        }
        let (a, b) = w.battle_fleets();
        if h == 0 || a == h || b == h {
            add_adat(s, 0x84, 10);
            add_adat(s, 0x90, 15);
        }
    }
    if escort {
        let f = s.f_new_ex("", 10, 8072, 0.0, 1, 0, w);
        // `st_fShow(8072)`, inlined.
        let g = s.f_search(8072, w);
        if g >= 1 {
            w.event(StoryEvent::SetFleetVisible { fleet: g, on: true });
        }
        s.error = (g < 1) as i32;
        let b = s.last_fn[last_fn::B];
        w.event(StoryEvent::MoveFleet { fleet: f, kind: 1, mode: 1, x: b as f32, y: 0.0 });
    }
    Ok(())
}

// ---------------------------------------------------------------- 809: the blockade

/// `StoryInit809()` (exe 0x5c4b80): planets 8091..=8093 (planet 0 when missing) go to empire 13 and are shown.
fn init_809(s: &mut Story, w: &mut dyn StoryWorld) {
    for id in [8091, 8092, 8093] {
        let p = s.b_search(id, 0, w);
        s.set_planet_owner(p, 13, w);
        s.b_show(p.wrapping_neg(), -1, w);
    }
}

/// `make_point_in_dist(&x, &y, x0, y0, x1, y1, d)` (inlined): the point `d` from `(x0, y0)` towards `(x1, y1)` (f32,
/// the length `(f32) sqrt((f64)(dy² + dx²))`).
fn point_in_dist(x0: f32, y0: f32, x1: f32, y1: f32, d: f32) -> (f32, f32) {
    let len = (((y0 - y1) * (y0 - y1) + (x0 - x1) * (x0 - x1)) as f64).sqrt() as f32;
    (((x1 - x0) * d) / len + x0, ((y1 - y0) * d) / len + y0)
}

/// 809 (exe block 0x5d8ff7): empire 13 holds planets 8091..=8094; its blockade fleet 8091 sits 25 off the human's planet
/// nearest to 8091 (id 8099) and starves it; the fleet destroyed, messages; empire 13's planets taken, the end.
pub(super) fn story_809(s: &mut Story, w: &mut dyn StoryWorld) -> Result<(), Abort> {
    let en = w.human();
    if s.starting_now() {
        init_809(s, w);
        for id in 8091..=8093 {
            s.auto_add_building(id, w);
        }
        for id in 8091..=8093 {
            s.auto_add_tank(id, w);
        }
        s.add_info_line(8091, InfoLine::Blank, -1, w);
        let q = s.b_search(8091, 0, w);
        let (qx, qy) = w.planet_xy(q);
        let p = nearest_planet(qx as i32, qy as i32, en, &*w);
        if p == 0 {
            return Err(Abort);
        }
        s.b_add_id(p, 8099);
        let (px, py) = w.planet_xy(p);
        let q = s.b_search(8091, 0, w);
        let (qx, qy) = w.planet_xy(q);
        let (x, y) = point_in_dist(px, py, qx, qy, 25.0);
        fleet_at(s, 13, 8091, x, y, 5.0, w);
        s.f_add_ship(8091, 0x2d, 1, w);
        s.sz_start(8099, 5);
        for id in 8092..=8094 {
            let p = s.b_search(id - 1, 0, w);
            s.f_new_ex("", 13, id, 0.0, 2, p, w);
        }
    }
    if s.counter(8099) {
        let f = s.f_search(8091, w);
        let p = s.b_search(8099, 0, w);
        if f != 0 {
            if w.planet_owner(p) == en {
                let value = w.planet_value(p);
                if value > 1000 {
                    let loss = Story::diff(w.difficulty(), 10, 30, 30, 50);
                    w.event(StoryEvent::SetPlanetValue { planet: p, value: value - loss });
                }
                s.sz_start(8099, 5);
            } else {
                s.send_away(8091, w);
            }
        }
    }
    if s.flags[flag::SPACE_START] {
        let (a, b) = w.battle_fleets();
        if w.fleet_owner(a) == 13 || w.fleet_owner(b) == 13 {
            s.set_adat_i32(0x94, 1);
        }
    }
    if s.battle_part(8091) != 0 && s.f_search(8091, w) == 0 {
        s.remove_game_info(8091, w);
        if s.v_get(8091) == 0 {
            s.sz_start(8090, 6);
            s.v_set(8091, 1);
        }
    }
    if s.counter(8090) {
        message(w, 8090, 0);
        s.sz_start(8091, 30);
    }
    if s.counter(8091) {
        message(w, 8092, 0);
        s.add_info_line(8092, InfoLine::Blank, -1, w);
    }
    if s.ground_part(0, 0, w) {
        let mut k = 0;
        while k < 4 {
            let p = s.b_search(8091 + k, 0, w);
            if w.planet_owner(p) == 13 {
                break;
            }
            k += 1;
        }
        if k > 3 {
            s.win(0, true, w);
        }
    }
    Ok(())
}
