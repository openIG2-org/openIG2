//! The starmap's mouse orders (re/screens/starmap.md §10.3): what `Starmap_Main` (ELF 0x6f50a0) picks under the
//! cursor (`SelectNearestUnit` ELF 0x6e46b0) and the move order a right click sends (`Multi_Send(0x15, EN, …)`).
//! Distances are squared virtual pixels between the cursor and a unit's projection (`Tavolsag` 0x6e45f0).

use ig2_sim::game::StarmapOrder;

use crate::starmap::Camera;
use crate::world::Snapshot;

/// What a pick found: `SelectNearestUnit`'s kind (1 planet, 2 fleet) and index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Planet(usize),
    Fleet(usize),
}

/// The local player's selection (`StarmapMode`, `AktPlanet`, `AktFleet`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub mode: i32,
    pub planet: usize,
    pub fleet: usize,
}

/// Modifier keys held during a click.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Keys {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// `max(225, (Zoom / 20)²)`: the pick radius² of `SelectNearestUnit` at radius 1.
fn reach(zoom: f32) -> f32 {
    let z = (zoom / 20.0) * (zoom / 20.0);
    if z <= 225.0 { 225.0 } else { z }
}

/// `Tavolsag(sx, sy, x, y)`: squared virtual distance from the cursor to the galaxy point's projection.
fn tavolsag(cam: &Camera, at: [f32; 2], p: [f32; 2]) -> f32 {
    match cam.virtual_point(p) {
        Some(v) => (at[0] - v[0]) * (at[0] - v[0]) + (at[1] - v[1]) * (at[1] - v[1]),
        None => f32::MAX,
    }
}

/// `SelectNearestUnit(sx, sy, r, exclude, &kind, skip_selected, enemies)`: the visible fleets and known planets
/// within `r` of the virtual point `at` (at most 10), ranked by `fleet + ships·10000 + own·10⁸` (planets 1); the
/// highest rank below `exclude`'s (a fleet's rank, 1 for a planet −p, so clicking again cycles through a stack),
/// else the highest whose distance is within 1.1 × the nearest.
#[allow(clippy::too_many_arguments)]
pub fn select_nearest_unit(s: &Snapshot, cam: &Camera, at: [f32; 2], r: f32, exclude: i32, sel: &Selection, skip_selected: bool, enemies: bool) -> Option<Unit> {
    let en = s.human;
    let limit2 = reach(cam.zoom) * r * r;
    let mut nearest = limit2;
    let mut cands: Vec<(Unit, i64)> = Vec::new();
    let rank = |slot: usize, ships: i32, owner: u8| slot as i64 + ships as i64 * 10000 + if owner == en { 100_000_000 } else { 0 };
    for f in &s.fleets {
        if (skip_selected && sel.fleet == f.slot) || (enemies && (f.owner == en || f.flags & 0x200 != 0)) || !f.visible {
            continue;
        }
        let d = tavolsag(cam, at, f.pos);
        nearest = nearest.min(d);
        if d < limit2 && cands.len() < 10 {
            cands.push((Unit::Fleet(f.slot), rank(f.slot, f.ships, f.owner)));
        }
    }
    for p in &s.planets {
        if (enemies && (p.owner == en || p.owner == 0 || p.friendly)) || !p.known {
            continue;
        }
        let d = tavolsag(cam, at, p.pos);
        nearest = nearest.min(d);
        if d < limit2 && cands.len() < 10 {
            cands.push((Unit::Planet(p.index), 1));
        }
    }
    // The original's exchange sort, highest rank first.
    for i in 0..cands.len() {
        for j in i + 1..cands.len() {
            if cands[i].1 < cands[j].1 {
                cands.swap(i, j);
            }
        }
    }
    let mut ceiling = 900_000_000i64;
    if exclude < 0 {
        ceiling = 1;
        let d = s.planets.iter().find(|p| p.index as i32 == -exclude).map_or(f32::MAX, |p| tavolsag(cam, at, p.pos));
        if limit2 < d {
            ceiling = 900_000_000;
        }
    } else if exclude > 0 {
        if let Some(f) = s.fleets.iter().find(|f| f.slot as i32 == exclude) {
            ceiling = rank(f.slot, f.ships, f.owner);
            if limit2 < tavolsag(cam, at, f.pos) {
                ceiling = 900_000_000;
            }
        }
    }
    let mut pick: Option<(Unit, i64)> = None;
    for &(u, k) in &cands {
        if k < ceiling && pick.is_none_or(|(_, b)| b < k) {
            pick = Some((u, k));
        }
    }
    if pick.is_none() {
        for &(u, k) in &cands {
            let pos = match u {
                Unit::Planet(p) => s.planets.iter().find(|x| x.index == p).map(|x| x.pos),
                Unit::Fleet(f) => s.fleets.iter().find(|x| x.slot == f).map(|x| x.pos),
            };
            let d = pos.map_or(f32::MAX, |p| tavolsag(cam, at, p));
            if pick.is_none_or(|(_, b)| b < k) && d < nearest * 1.1 {
                pick = Some((u, k));
            }
        }
    }
    pick.map(|(u, _)| u)
}

/// The order a right click at the virtual point `at` gives the selected fleet (`Starmap_Main`, the order branch
/// with `StarmapVillogasMode` 0): a planet or fleet under the cursor (an enemy one → attack, Ctrl picks enemies
/// only, Alt never), else the galaxy point there (`get_pos`, kept within 600 of the centre); Shift appends a
/// waypoint (14, patrol, when the click is on the fleet itself). `None` when there is nothing to order.
pub fn right_click(s: &Snapshot, cam: &Camera, at: [f32; 2], sel: &Selection, keys: Keys) -> Option<StarmapOrder> {
    if sel.mode != 2 {
        return None;
    }
    let me = s.fleets.iter().find(|f| f.slot == sel.fleet)?;
    // Only the player's fleets (and the story's steerable fleet 0x1f88).
    if me.owner != s.human && me.story_id != 0x1f88 {
        return None;
    }
    let exclude = match (me.order > 0, me.kind) {
        (true, 1) => -(me.target_index as i32),
        (true, 2) => me.target_index as i32,
        _ => 0,
    };
    let mut mode = if keys.ctrl { 2 } else { 0 };
    let mut unit = select_nearest_unit(s, cam, at, 0.5, exclude, sel, true, mode == 2);
    if keys.alt {
        mode = 0;
        unit = None;
    }
    let planet = |p: usize| s.planets.iter().find(|x| x.index == p);
    let fleet = |f: usize| s.fleets.iter().find(|x| x.slot == f);
    if mode == 0 {
        mode = 1;
        let hostile = match unit {
            Some(Unit::Planet(p)) => planet(p).is_some_and(|p| p.owner != 0 && p.owner != s.human && !p.friendly && p.owner != me.owner),
            Some(Unit::Fleet(f)) => fleet(f).is_some_and(|f| f.owner != s.human && f.flags & 0x200 == 0 && f.owner != me.owner),
            None => false,
        };
        if hostile {
            mode = 2;
        }
    }
    let send = me.flags & 1 == 0
        && ((mode == 1 && unit != Some(Unit::Fleet(sel.fleet)))
            || (mode == 2 && matches!(unit, Some(Unit::Planet(p)) if planet(p).is_some_and(|p| p.owner != s.human && p.owner != 0)))
            || matches!(unit, Some(Unit::Fleet(f)) if fleet(f).is_some_and(|f| f.owner != s.human)));
    if !send {
        return None;
    }
    let target = unit.filter(|_| !keys.shift);
    Some(match target {
        Some(u) => {
            let (kind, i) = match u {
                Unit::Planet(p) => (1, p),
                Unit::Fleet(f) => (2, f),
            };
            StarmapOrder::Move { fleet: sel.fleet as i32, order: mode, kind, x: i as f32, y: 0.0 }
        }
        None => {
            if keys.shift {
                mode = 13;
                if tavolsag(cam, at, me.pos) < reach(cam.zoom) * 0.25 {
                    mode = 14;
                }
            }
            let mut p = cam.virtual_to_world(at)?;
            let r = ((p[0] * p[0] + p[1] * p[1]) as f64).sqrt() as f32;
            if r > 600.0 {
                p = [p[0] / r * 600.0, p[1] / r * 600.0];
            }
            StarmapOrder::Move { fleet: sel.fleet as i32, order: mode, kind: 0, x: p[0], y: p[1] }
        }
    })
}

/// A left press on the map (`Starmap_Main`, which then opens the radial menu): the next unit under the cursor (`SelectNearestUnit` at
/// radius 1, cycling from the current selection) becomes the selection.
pub fn left_click(s: &Snapshot, cam: &Camera, at: [f32; 2], sel: &Selection) -> Option<Selection> {
    let exclude = if sel.mode == 2 { sel.fleet as i32 } else { -(sel.planet as i32) };
    Some(match select_nearest_unit(s, cam, at, 1.0, exclude, sel, false, false)? {
        Unit::Planet(p) => Selection { mode: 1, planet: p, fleet: sel.fleet },
        Unit::Fleet(f) => Selection { mode: 2, planet: sel.planet, fleet: f },
    })
}

/// `SelectNearestPlanet(sx, sy, r, &kind)` (ELF 0x6e5150): the player's own planet it sees whose star projects
/// nearest to `at` within `max(225, (Zoom/20)²)·r²`; no cycling.
pub fn select_nearest_planet(s: &Snapshot, cam: &Camera, at: [f32; 2], r: f32) -> Option<usize> {
    let mut best = (reach(cam.zoom) * r * r, None);
    for p in s.planets.iter().filter(|p| p.owner == s.human && p.known) {
        let d = tavolsag(cam, at, p.pos);
        if d < best.0 {
            best = (d, Some(p.index));
        }
    }
    best.1
}

/// A click in the minimap (hot-spot 7, `Starmap_Main`): the view's pivot moves over the galaxy point there
/// (`TargetXStart = −((x − 274)/92 − 0.5)·600`, `TargetYStart = −((y − 388)/65 − 0.5)·600`, within ±299.9).
pub fn minimap_target(at: [f32; 2]) -> [f32; 2] {
    let c = |v: f32| if v > 300.0 { 299.9 } else if v < -300.0 { -299.9 } else { v };
    [c(-((at[0] - 274.0) / 92.0 - 0.5) * 600.0), c(-((at[1] - 388.0) / 65.0 - 0.5) * 600.0)]
}

/// The minimap's hot-spot (`Mouse_Szoveg_Set(7, …, 0x105, 0x16f, 0x76, 0x53)`).
pub const MINIMAP_SPOT: [f32; 4] = [261.0, 367.0, 379.0, 450.0];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimap_clicks_map_back_to_the_galaxy() {
        // The minimap's centre (320, 420.5) is the galaxy's centre; its right edge is x = −300.
        let c = minimap_target([320.0, 420.5]);
        assert!(c[0].abs() < 1e-3 && c[1].abs() < 1e-3);
        assert_eq!(minimap_target([400.0, 420.5])[0], -299.9);
    }
}
