//! Ours (ADR 0045): the enhanced computer empires' economy and research (`re/rules/ai-empire.md` §15.2), fleets and
//! war (§15.4). Each function runs after (or instead of) the original's step for the same empire, only when
//! `AiContext::enhanced` is on; it draws no random numbers, so the classic draws stay where they were.

use super::colonise::capturable;
use super::{category, AiEvent, AiState, World, BEST};
use crate::colony::Planet;
use crate::colony_side::{price, status, TECHS};
use crate::diplomacy::level;
use crate::fleet::{flag, order};
use crate::research::{self, ResearchEvent};

/// The autobuild modes (`autobldN.txt`): production, technological (labs), ship and vehicle production.
const MODE_PRODUCTION: u8 = 2;
const MODE_LABS: u8 = 3;
const MODE_SHIPS: u8 = 6;
/// Tax levels by morale band (the original sets 3, or 0 on unrest).
const TAX_LOW: u8 = 2;
const TAX_NORMAL: u8 = 3;
const TAX_HIGH: u8 = 4;
/// Morale (‰) below which the tax is [`TAX_LOW`], and from which it is [`TAX_HIGH`].
const MORALE_LOW: i32 = 500;
const MORALE_HIGH: i32 = 750;
/// Credits above which research runs urgent (double speed, double cost), and below which it stops being urgent.
const URGENT_ON: f64 = 150_000.0;
const URGENT_OFF: f64 = 50_000.0;
/// Colony ships wanted at once, at most.
const COLONY_SHIPS: i32 = 3;
/// Credits an enhanced empire keeps after paying for a lab.
const LAB_RESERVE: f64 = 20_000.0;
/// The autobuild category of the ship lab (`AutoBuildMitEpitMin/Max[5]`); 6 and 7 are the vehicle and building labs.
const LAB_CATEGORY: usize = 5;
/// The autobuild categories of the spaceship and the tank factory (`AutoBuildMitEpitMin/Max[2]`, `[3]`).
const SHIPYARD_CATEGORY: usize = 2;
const TANK_FACTORY_CATEGORY: usize = 3;
/// Credits an enhanced empire keeps after paying for a factory, at peace and at war.
const FACTORY_RESERVE_PEACE: f64 = 20_000.0;
const FACTORY_RESERVE_WAR: f64 = 30_000.0;
/// The ship production capacity (planet `+0x70`, summed) an enhanced empire builds factories up to, per planet.
const SHIP_CAPACITY_PER_PLANET: i32 = 3000;
/// A new war needs the main fleet this many times as strong as the target's.
const WAR_MARGIN: f64 = 1.5;
/// A planet is invaded only with this many times its defence in ground strength aboard.
pub(super) const INVADE_MARGIN: f64 = 1.5;
/// The main fleet waits for followers within this distance carrying at least a quarter of its strength.
const GATHER_RANGE: f32 = 150.0;
/// Planet garrisons (`+0x58`) the vehicle line fills up to at war (the original: 10).
pub(super) const GARRISON_WAR: i32 = 25;
/// Planet event: the AI must not colonise this planet.
const NO_AI_COLONY: i16 = -0x2094;

/// No tech is being researched and none is open with `e`'s labs: research waits for labs.
fn research_blocked(w: &World, e: usize) -> bool {
    let colony = &w.spy.colony;
    let labs = &colony.empires.labs[e];
    w.spy.research.current[e] < 1 && !(1..TECHS).any(|t| colony.tech.status(t, e) == status::RESEARCHABLE && research::labs_ok(&colony.tech, t, labs))
}

/// Free planets `e` sees and may colonise (`FRISSIT_MI_KOLONIZALAS`'s candidates, §9).
pub(super) fn free_planets(w: &World, e: usize) -> i32 {
    let mp = w.spy.ctx.multiplayer;
    w.spy.colony.planets.iter().skip(1).filter(|p| p.owner == 0 && p.visibility[e] > 0 && (mp || (capturable(p) && !p.events.contains(&NO_AI_COLONY)))).count() as i32
}

/// The colony ships an enhanced empire wants (`MiGyarthatoCuccok[e][3]` +0x14): one per free planet it sees, at
/// most [`COLONY_SHIPS`]; the original wants one while `VanUresBolygo`.
pub(super) fn colony_ships_wanted(w: &World, e: usize, empty: bool) -> i32 {
    if empty { free_planets(w, e).clamp(1, COLONY_SHIPS) } else { 0 }
}

/// The lab an enhanced empire builds while research waits for labs, and where: for the open tech closest to its
/// lab buildings (`LaborSzintOsszes`, working or not: a lab without power or workers needs those, not another lab;
/// the smallest total shortfall, then the cheaper, then the lower number), the lab type it lacks most (the first on
/// ties), its best researched building of that type, on the most populous planet with nothing under construction.
/// None while the credits after paying would fall below [`LAB_RESERVE`].
fn lab_to_build(w: &World, e: usize) -> Option<(usize, u8)> {
    let colony = &w.spy.colony;
    let labs = &colony.empires.lab_buildings[e];
    let short = |t: usize| colony.tech.labs_needed(t).map(|n| n as i32).iter().zip(&labs[1..]).map(|(n, l)| (n - l).max(0)).collect::<Vec<_>>();
    let t = (1..TECHS).filter(|&t| colony.tech.status(t, e) == status::RESEARCHABLE).min_by_key(|&t| (short(t).iter().sum::<i32>(), colony.tech.research_cost(t), t))?;
    let s = short(t);
    let i = (0..3).max_by_key(|&i| (s[i], std::cmp::Reverse(i)))?;
    if s[i] < 1 {
        return None;
    }
    place(w, e, LAB_CATEGORY + i, LAB_RESERVE, |p| p.population)
}

/// `e`'s best researched building of autobuild category `c` on its planet with nothing under construction and the
/// greatest `rank` (the first on ties), while the credits after paying stay at `reserve` or above.
fn place(w: &World, e: usize, c: usize, reserve: f64, rank: fn(&Planet) -> i32) -> Option<(usize, u8)> {
    let colony = &w.spy.colony;
    let rules = &w.spy.side_rules;
    let range = |v: &Vec<i32>| v.get(c).copied().unwrap_or(0).max(0) as usize;
    let (lo, hi) = (range(&rules.autobuild.min), range(&rules.autobuild.max));
    let kind = (lo..=hi).rev().find(|&k| rules.colony.buildings.get(k).is_some_and(|b| colony.tech.status(b.tech as usize, e) == status::OWNED))?;
    let cost = price(&colony.tech, w.race(e), rules.colony.buildings[kind].tech as usize);
    if colony.money[e].available(2) < cost as f64 + reserve {
        return None;
    }
    let p = (1..colony.planets.len())
        .filter(|&p| colony.planets[p].owner as usize == e && !colony.planets[p].buildings.iter().any(|b| b.kind != 0 && b.work_left > 0))
        .max_by_key(|&p| (rank(&colony.planets[p]), std::cmp::Reverse(p)))?;
    Some((p, kind as u8))
}

/// The factory an enhanced empire adds when research doesn't wait for labs: at war a tank factory while the
/// vehicle capacity (`+0x74`, summed) is below half the ship capacity, else a spaceship factory while the ship
/// capacity is below [`SHIP_CAPACITY_PER_PLANET`] per planet; on the planet with the most of that capacity (a
/// planet's line runs at its own capacity), with [`FACTORY_RESERVE_WAR`] at war and [`FACTORY_RESERVE_PEACE`] at peace.
fn factory_to_build(w: &World, e: usize) -> Option<(usize, u8)> {
    let war = w.danger[e] >= 2;
    let (mut ships, mut vehicles, mut n) = (0i32, 0i32, 0i32);
    for p in w.spy.colony.planets.iter().skip(1).filter(|p| p.owner as usize == e) {
        ships = ships.saturating_add(p.ship_capacity);
        vehicles = vehicles.saturating_add(p.vehicle_capacity);
        n += 1;
    }
    let reserve = if war { FACTORY_RESERVE_WAR } else { FACTORY_RESERVE_PEACE };
    if war && vehicles < ships / 2 {
        return place(w, e, TANK_FACTORY_CATEGORY, reserve, |p| p.vehicle_capacity);
    }
    if ships < SHIP_CAPACITY_PER_PLANET.saturating_mul(n) { place(w, e, SHIPYARD_CATEGORY, reserve, |p| p.ship_capacity) } else { None }
}

/// Whether an enhanced empire declares the war `MI_KitAkarTamadni` proposes on `t` without waiting for the
/// original's 1-in-720 day (§15.4): not under a ceasefire, pact or alliance, and its main fleet at least
/// [`WAR_MARGIN`] times as strong as `t`'s.
pub(super) fn war_ready(w: &World, e: usize, t: usize, main_fleet: &[i32]) -> bool {
    let strength = |f: i32| w.rec(f.max(0)).map_or(0, |r| r.strength) as f64;
    let mine = strength(main_fleet[e]);
    !matches!(w.spy.relations[e][t].level, level::CEASEFIRE | level::NAP | level::ALLIANCE) && mine > 0.0 && mine >= strength(main_fleet[t]) * WAR_MARGIN
}

/// The enemy fleet attacking one of `e`'s planets that the main fleet `m` outguns, the nearest to it (§15.4).
pub(super) fn defence_target(w: &World, e: usize, m: usize) -> Option<usize> {
    let me = w.rec(m as i32)?;
    let planets = &w.spy.colony.planets;
    let dist = |f: usize| {
        let r = &w.starmap.fleets[f];
        (r.x - me.x).powi(2) + (r.y - me.y).powi(2)
    };
    (1..w.starmap.fleets.len())
        .filter(|&f| {
            let r = &w.starmap.fleets[f];
            r.owner != crate::fleet::DELETED
                && r.story_id == 0
                && r.visible[e] > 0
                && w.spy.relations[e].get(r.owner as usize).is_some_and(|x| x.level == level::WAR)
                && r.order == order::ATTACK
                && r.kind == order::PLANET
                && planets.get(r.tx as usize).is_some_and(|pl| pl.owner as usize == e)
                && r.strength < me.strength
        })
        .min_by(|&a, &b| dist(a).total_cmp(&dist(b)).then(a.cmp(&b)))
}

/// Whether the main fleet `m` of `e` waits for its followers before attacking (§15.4): the fleets moving to it
/// within [`GATHER_RANGE`] carry at least a quarter of its strength.
pub(super) fn gather(w: &World, e: usize, m: usize) -> bool {
    let Some(me) = w.rec(m as i32) else { return false };
    let mut near = 0i32;
    for (f, r) in w.starmap.fleets.iter().enumerate().skip(1) {
        if f != m
            && r.owner as usize == e
            && r.story_id == 0
            && r.flags & (flag::COLONISING | flag::TERRAFORMING) == 0
            && r.order == order::MOVE
            && r.kind == order::FLEET
            && r.tx as usize == m
            && (r.x - me.x).powi(2) + (r.y - me.y).powi(2) < GATHER_RANGE * GATHER_RANGE
        {
            near = near.saturating_add(r.strength);
        }
    }
    near > 0 && near.saturating_mul(4) >= me.strength
}

/// `e`'s warships for the peace-time cap: ships in its fleets whose design is neither a colony ship nor a
/// terraformer (design `+0x34`, `+0x35`), and the warship batches queued on its yards (the warship category's
/// design is often a fighter, counted per fighter in the category's totals).
pub(super) fn warships(w: &World, e: usize, cats: &[super::Category; super::CATEGORIES]) -> i32 {
    let cw = &w.spy.combat;
    let ships = (1..cw.ships.len())
        .filter(|&i| {
            let s = &cw.ships[i];
            w.fleet_owner(s.fleet) as usize == e && w.design(e, s.design as i32).is_some_and(|r| r[0x34] == 0 && r[0x35] == 0)
        })
        .count() as i32;
    ships + cats[category::SHIP].queued[BEST] + cats[category::SHIP_ALT].queued[BEST] + cats[category::SHIP_ALT].queued[super::SPECIAL]
}

/// The warships an enhanced empire at peace keeps building up to: four, and two per planet.
pub(super) fn peace_warships(planets: usize) -> i32 {
    4 + 2 * planets as i32
}

impl AiState {
    /// After the original's planet settings (§3.3): while research waits for labs, a lab is placed ([`lab_to_build`]),
    /// otherwise a factory ([`factory_to_build`]); while research waits, every production planet but the main one
    /// builds labs; each planet's tax follows its morale (the lower of now and where it is heading): [`TAX_LOW`],
    /// [`TAX_NORMAL`] or [`TAX_HIGH`], 0 while it is restless.
    pub(super) fn enhanced_planet_settings(&mut self, w: &mut World, e: usize) {
        let labs = research_blocked(w, e);
        if let Some((planet, kind)) = if labs { lab_to_build(w, e) } else { factory_to_build(w, e) } {
            w.events.push(AiEvent::Build { empire: e as u8, planet, kind });
        }
        let main = self.main_planet[e];
        let colony = &mut *w.spy.colony;
        for p in 1..colony.planets.len() {
            if colony.planets[p].owner as usize != e {
                continue;
            }
            let needed = colony.needed_morale.get(p).copied().unwrap_or(1000);
            let pl = &mut colony.planets[p];
            if labs && p as i32 != main && matches!(pl.autobuild_mode, MODE_PRODUCTION | MODE_SHIPS) {
                pl.autobuild_mode = MODE_LABS;
            }
            let morale = pl.morale.min(needed);
            pl.tax_level = if pl.unrest > 0 {
                0
            } else if morale < MORALE_LOW {
                TAX_LOW
            } else if morale >= MORALE_HIGH {
                TAX_HIGH
            } else {
                TAX_NORMAL
            };
        }
    }

    /// Instead of `FRISSIT_MI_FEJLESZTES` (`production-research.md` §2.2): while there are planets to colonise
    /// and no colony-ship design, the cheapest open hull of the colony-ship slots (`MiGyartasMitMin/Max[3]`);
    /// else the open tech of the race's lowest non-zero priority, a tie to the cheaper one (the auto research pick
    /// of ADR 0037, every category). With plenty of credits the project runs urgent.
    pub(super) fn enhanced_research(&self, w: &mut World, e: usize) {
        let colony_ship = self.empty_planets[e] != 0 && self.categories[e][category::COLONY_SHIP].design[BEST] <= 0;
        let slot = |v: &Vec<u8>| v.get(category::COLONY_SHIP).copied().unwrap_or(0) as usize;
        let hulls: Vec<usize> = if colony_ship {
            (slot(&w.rules.build_min)..=slot(&w.rules.build_max)).filter_map(|s| w.rules.design.hulls.get(s)).map(|h| h.tech.max(0) as usize).collect()
        } else {
            Vec::new()
        };
        let colony = &mut *w.spy.colony;
        let research = &mut *w.spy.research;
        if research.current[e] == 0 {
            let race = w.spy.ctx.race.get(e).copied().unwrap_or(0);
            let r = if race < 9 { race as usize } else { 1 };
            let labs = &colony.empires.labs[e];
            let open = |t: usize| t > 0 && colony.tech.status(t, e) == status::RESEARCHABLE && research::labs_ok(&colony.tech, t, labs);
            let pick = hulls.iter().copied().filter(|&t| open(t)).min_by_key(|&t| (colony.tech.research_cost(t), t)).or_else(|| {
                (1..TECHS).filter(|&t| open(t)).min_by_key(|&t| {
                    let b = colony.tech.ai_priority(t, r);
                    (b == 0, b, colony.tech.research_cost(t), t)
                })
            });
            if let Some(t) = pick {
                colony.tech.set_status(t, e, status::RESEARCHING);
                research.current[e] = t as i32;
                w.events.push(AiEvent::Research(ResearchEvent::Started { empire: e as u8, tech: t }));
            }
        }
        let credits = colony.money[e].total;
        if research.current[e] < 1 || credits < URGENT_OFF {
            research.urgent[e] = 0;
        } else if credits > URGENT_ON {
            research.urgent[e] = 1;
        }
    }
}
