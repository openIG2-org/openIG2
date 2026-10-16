//! Building placement on the colony map: `LocateSuitableLocation` (exe 0x4869d0) with its grid helpers
//! `Decode2LargeMap` (0x46d8a0), `DecodedAllocate` (0x46da50), `DecodedCheckAllocation` (0x46db30),
//! `CheckKeretAllocation` (0x460800) and the spiral search `LocatePos` (0x486610).
//! Spec: `re/rules/colony-placement.md`.

use crate::colony::Planet;

/// Grid side (`DecodedAlloc` is 200 × 200, index `y × 200 + x`).
pub const SIDE: i32 = 200;
/// Terrain mask: 50 rows × 13 bytes, 2 bits per cell (`colony-map.md`).
const MASK_ROWS: usize = 50;
const MASK_STRIDE: usize = 13;

/// Per-map inputs: terrain mask A of `gen_maps/<m>.map`, the surface type of its `terepleiro.txt`
/// section (`Terepek[m]+0`, used to pick maps on terraforming) and its AI build start.
#[derive(Debug, Clone)]
pub struct Terrain {
    pub mask: [u8; MASK_ROWS * MASK_STRIDE],
    pub kind: u8,
    pub start: (i32, i32),
}

/// What the search needs besides the planet: terrains by map index and footprints by building kind.
#[derive(Debug, Clone, Default)]
pub struct PlacementRules {
    pub terrains: Vec<Terrain>,
    /// `(width, height)` by kind (`ig2_rules::Rules::building_footprints`).
    pub footprints: Vec<(i32, i32)>,
}

impl PlacementRules {
    fn footprint(&self, kind: u8) -> (i32, i32) {
        self.footprints.get(kind as usize).copied().unwrap_or((0, 0))
    }

    /// `LocateSuitableLocation(p, kind, &x, &y)`: a free spot for `kind` on `planet`'s colony map, or `None`.
    /// `vehicles` are the cells of the planet's ground-war units on the map (spec §1); `hint` replaces the
    /// start point (`EpuletNewSearchLocation` passes none).
    pub fn locate(&self, planet: &Planet, kind: u8, vehicles: &[(i32, i32)], hint: Option<(i32, i32)>) -> Option<(i32, i32)> {
        let terrain = self.terrains.get(planet.colony_map as usize)?;
        let mut grid = Grid::from_terrain(&terrain.mask);
        let (mut sx, mut sy, mut n) = (0i32, 0i32, 0i32);
        for b in planet.buildings.iter().filter(|b| b.kind != 0) {
            let (w, h) = self.footprint(b.kind);
            grid.allocate(b.x as i32, b.y as i32, w, h, true);
            n += 1;
            sx = (sx as f32 + b.x) as i32;
            sy = (sy as f32 + b.y) as i32;
        }
        for &(x, y) in vehicles {
            grid.allocate(x, y, 1, 1, false);
        }
        let mut start = terrain.start;
        if n != 0 {
            start = (sx / n, sy / n);
        }
        if let Some(h) = hint.filter(|h| h.0 >= 0) {
            start = h;
        }
        let (w, h) = self.footprint(kind);
        grid.search(start, w, h)
    }

    /// [`Self::locate`] as a [`crate::colony_side::Placement`] result: the position `EpuletNew` stores.
    pub fn place(&self, planet: &Planet, kind: u8, vehicles: &[(i32, i32)]) -> Option<(f32, f32)> {
        self.locate(planet, kind, vehicles, None).map(|(x, y)| (x as f32, y as f32))
    }
}

/// `LandingZone::Generate` (exe 0x478dd0, ELF 0x46b610; `ground-combat.md` §11.1): around every building
/// twelve nested boxes, `w + 22 × h + 22` shrinking by 2 per level, mark each cell with the highest level
/// covering it (1 = outermost). Row `y − 1` is written (index `(y − 1)·200 + x`, checked against the
/// buffer only). Cells above 0 are closed to the real-time landing.
pub fn landing_zone(buildings: &[(u8, f32, f32)], footprints: &[(i32, i32)]) -> Vec<i8> {
    let mut z = vec![0i8; (SIDE * SIDE) as usize];
    for &(kind, x, y) in buildings.iter().filter(|b| b.0 != 0) {
        let (w, h) = footprints.get(kind as usize).copied().unwrap_or((0, 0));
        let (bx, by) = (x as i32, y as i32);
        let (mut w, mut h) = (w + 0x16, h + 0x16);
        for level in 1..0xd {
            for ix in bx - w / 2..bx + w / 2 + w % 2 {
                for iy in by - h / 2 - h % 2..by + h / 2 {
                    let i = (iy - 1) * SIDE + ix;
                    if (0..SIDE * SIDE).contains(&i) && z[i as usize] < level {
                        z[i as usize] = level;
                    }
                }
            }
            w -= 2;
            h -= 2;
        }
    }
    z
}

/// `LocateTankSuitablePosition(p, &x, &y, sx, sy)` (exe 0x486b90, ELF 0x488d80; `ground-combat.md` §11.1):
/// a free cell for one vehicle. The grid is the building search's (terrain, buildings with frames), plus
/// a 3 × 3 block per placed unit (`units`: cells of alive units without flags 2 / 0x40000000 and without
/// flags2 bit 2); a given start (`sx ≠ −1`) also closes the landing zone and replaces the centroid start.
pub fn locate_tank(
    terrain: &Terrain,
    footprints: &[(i32, i32)],
    buildings: &[(u8, f32, f32)],
    landing: Option<&[i8]>,
    units: &[(i32, i32)],
    start: Option<(i32, i32)>,
) -> Option<(i32, i32)> {
    let mut grid = Grid::from_terrain(&terrain.mask);
    if let (Some(z), Some(_)) = (landing, start) {
        for (c, &l) in grid.0.iter_mut().zip(z) {
            if l > 0 {
                *c = 1;
            }
        }
    }
    let (mut sx, mut sy, mut n) = (0i32, 0i32, 0i32);
    for &(kind, x, y) in buildings.iter().filter(|b| b.0 != 0) {
        let (w, h) = footprints.get(kind as usize).copied().unwrap_or((0, 0));
        grid.allocate(x as i32, y as i32, w, h, true);
        n += 1;
        sx = (sx as f32 + x) as i32;
        sy = (sy as f32 + y) as i32;
    }
    for &(x, y) in units {
        grid.allocate(x, y, 3, 3, false);
    }
    let mut at = terrain.start;
    if n != 0 {
        at = (sx / n, sy / n);
    }
    if let Some(s) = start {
        at = s;
    }
    grid.search(at, 1, 1)
}

/// The occupancy grid `DecodedAlloc`: 1 = blocked, multiples of 0x20 = frames of placed buildings.
#[derive(Clone)]
pub struct Grid(Vec<u8>);

impl Grid {
    /// `Decode2LargeMap(m, grid, false, p)`: each terrain cell becomes a 4 × 4 block of 0/1.
    pub fn from_terrain(mask: &[u8; MASK_ROWS * MASK_STRIDE]) -> Self {
        let mut g = vec![0u8; (SIDE * SIDE) as usize];
        for r in 0..MASK_ROWS {
            for c in 0..50 {
                let v = (mask[r * MASK_STRIDE + c / 4] >> ((c % 4) * 2)) & 3 != 0;
                for y in 4 * r..4 * r + 4 {
                    g[y * SIDE as usize + 4 * c..y * SIDE as usize + 4 * c + 4].fill(v as u8);
                }
            }
        }
        Self(g)
    }

    pub fn get(&self, x: i32, y: i32) -> u8 {
        self.0[(y * SIDE + x) as usize]
    }

    /// A cell by linear index; the original doesn't clip rows, so only the buffer bounds matter.
    fn at(&mut self, x: i32, y: i32) -> Option<&mut u8> {
        let i = y * SIDE + x;
        if (0..SIDE * SIDE).contains(&i) { self.0.get_mut(i as usize) } else { None }
    }

    /// `DecodedAllocate(x, y, w, h, ring)` (spec §2).
    pub fn allocate(&mut self, x: i32, y: i32, w: i32, h: i32, ring: bool) {
        if !(0..SIDE).contains(&x) || !(0..SIDE).contains(&y) {
            return;
        }
        let (x0, x1) = (x - w / 2 - 1, x + w / 2 + w % 2);
        let (y0, y1) = (y - h / 2 - h % 2 - 1, y + h / 2);
        for ix in x0..=x1 {
            for iy in y0..=y1 {
                let border = ix == x0 || iy == y0 || ix == x1 || iy == y1;
                if let Some(c) = self.at(ix, iy) {
                    if !border {
                        *c = 1;
                    } else if ring {
                        *c = c.wrapping_add(0x20);
                    }
                }
            }
        }
    }

    fn blocked_at(&self, x: i32, y: i32, mask: u8) -> bool {
        let i = y * SIDE + x;
        (0..SIDE * SIDE).contains(&i) && self.0[i as usize] & mask != 0
    }

    /// `CheckKeretAllocation(grid, x, y, w + 2, h + 2)`: terrain or an interior on the frame ring.
    fn frame_blocked(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        let (fw, fh) = (w + 2, h + 2);
        if (fw == 3 && fh == 3) || !(0..SIDE).contains(&x) || !(0..SIDE).contains(&y) {
            return false;
        }
        let (x0, xe) = (x - fw / 2, x + fw / 2 + fw % 2);
        let (y0, ye) = (y - fh / 2 - fh % 2, y + fh / 2);
        let cols = (x0..xe).step_by((fw - 1) as usize);
        if cols.clone().any(|ix| (y0..ye).any(|iy| self.blocked_at(ix, iy, 0x1f))) {
            return true;
        }
        (x0..xe).any(|ix| (y0..ye).step_by((fh - 1) as usize).any(|iy| self.blocked_at(ix, iy, 0x1f)))
    }

    /// `DecodedCheckAllocation(x, y, w, h)` (spec §4): true = taken.
    pub fn taken(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        if !(15..=184).contains(&x) || !(15..=184).contains(&y) || self.frame_blocked(x, y, w, h) {
            return true;
        }
        let (x0, x1) = (x - w / 2, x + w / 2 + w % 2 - 1);
        let (y0, y1) = (y - h / 2 - h % 2, y + h / 2 - 1);
        x0 >= 0 && x1 < SIDE && y0 >= 0 && y1 < SIDE && (x0..=x1).any(|ix| (y0..=y1).any(|iy| self.get(ix, iy) != 0))
    }

    /// `LocatePos(DecodedCheckAllocation, cx, cy, &x, &y, w, h)` (spec §5).
    pub fn search(&self, start: (i32, i32), w: i32, h: i32) -> Option<(i32, i32)> {
        locate_pos(start, w, h, |x, y| self.taken(x, y, w, h))
    }
}

/// `LocatePos(taken, cx, cy, &x, &y, w, h)` (exe 0x486610, spec §5): the square spiral from `(cx, cy)`
/// (which must lie in 10..=189) to the first cell `taken` refuses; `w`, `h` only bound the reach.
pub fn locate_pos((cx, cy): (i32, i32), w: i32, h: i32, taken: impl Fn(i32, i32) -> bool) -> Option<(i32, i32)> {
    if !(10..=189).contains(&cx) || !(10..=189).contains(&cy) {
        return None;
    }
    let a = cx - (w / 2 + 1);
    let b = cy - (h / 2 + 1);
    let c = SIDE - (w / 2 + 1) - cx;
    let d = SIDE - (h / 2 + 1) - cy;
    let reach = 0.max(a).max(b).max(c).max(d);
    for r in 0..reach {
        for k in 0..=r {
            let candidates = [
                (r <= a && k <= b, (cx - r, cy - k)),
                (r <= a && k <= d, (cx - r, cy + k)),
                (r <= c && k <= b, (cx + r, cy - k)),
                (r <= c && k <= d, (cx + r, cy + k)),
                (r <= b && k <= a, (cx - k, cy - r)),
                (r <= b && k <= c, (cx + k, cy - r)),
                (r <= d && k <= a, (cx - k, cy + r)),
                (r <= d && k <= c, (cx + k, cy + r)),
            ];
            if let Some(&(_, p)) = candidates.iter().find(|(ok, p)| *ok && !taken(p.0, p.1)) {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colony::Building;

    fn rules() -> PlacementRules {
        // Synthetic footprints (the real ones come from the user's exe): kind 1 = 4 × 4, kind 2 = 3 × 2, kind 3 = 1 × 1.
        PlacementRules { terrains: vec![Terrain { mask: [0; 650], kind: 0, start: (100, 100) }], footprints: vec![(0, 0), (4, 4), (3, 2), (1, 1)] }
    }

    fn planet(buildings: &[(u8, f32, f32)]) -> Planet {
        Planet {
            buildings: buildings.iter().map(|&(kind, x, y)| Building { kind, x, y, ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn landing_zone_rings_and_the_row_above_quirk() {
        let fp = rules().footprints;
        let z = landing_zone(&[(3, 100.0, 100.0)], &fp);
        let at = |x: i32, y: i32| z[((y - 1) * SIDE + x) as usize];
        // A 1 × 1 building: the outer box is 23 × 23 (level 1), the innermost 1 × 1 (level 12) sits on
        // the row above (an odd height puts the extra row low).
        assert_eq!((at(100, 99), at(100, 100)), (12, 11));
        assert_eq!(at(100 - 11, 100), 1);
        assert_eq!(at(100 + 11, 100), 1);
        assert_eq!(at(100 + 12, 100), 0);
        assert_eq!(at(100, 100 + 11), 0, "the outer box runs y − 12 ..= y + 10");
        assert_eq!(at(100, 100 - 12), 1);
    }

    #[test]
    fn locate_tank_closes_the_landing_zone_only_for_a_given_start() {
        let r = rules();
        let b = [(3u8, 100.0f32, 100.0f32)];
        let z = landing_zone(&b, &r.footprints);
        let t = &r.terrains[0];
        // Without a start: the centroid of the buildings, beside the building's frame.
        let free = locate_tank(t, &r.footprints, &b, Some(&z), &[], None).unwrap();
        assert!((free.0 - 100).abs() <= 3 && (free.1 - 100).abs() <= 3);
        // With a start inside the zone the search spirals out of it.
        let out = locate_tank(t, &r.footprints, &b, Some(&z), &[], Some((100, 100))).unwrap();
        assert_eq!(z[((out.1 - 1) * SIDE + out.0) as usize], 0);
        // Units block 3 × 3.
        let blocked = locate_tank(t, &r.footprints, &[], None, &[(100, 100)], None).unwrap();
        assert!((blocked.0 - 100).abs() >= 2 || (blocked.1 - 100).abs() >= 2);
    }

    #[test]
    fn empty_map_takes_the_start_and_footprints_are_odd_sided_like_the_original() {
        let r = rules();
        assert_eq!(r.locate(&planet(&[]), 1, &[], None), Some((100, 100)));
        let mut g = Grid(vec![0; 40000]);
        g.allocate(50, 50, 3, 2, true);
        // x: interior 49..=51 (odd width: the extra column is on the high side), frame 48 and 52;
        // y: interior 49..=50 (even height), frame 48 and 51.
        assert_eq!((g.get(49, 49), g.get(51, 50), g.get(52, 50), g.get(48, 48), g.get(50, 51)), (1, 1, 0x20, 0x20, 0x20));
        g.allocate(50, 50, 3, 3, true);
        assert_eq!(g.get(52, 50), 0x40, "frames add up");
    }

    #[test]
    fn next_building_goes_beside_the_first_sharing_one_frame_cell() {
        let r = rules();
        let p = planet(&[(1, 100.0, 100.0)]);
        // Centroid (100, 100) is taken; ring 1 of the spiral is all overlapping, so the first free spot of
        // the left column (cx − r, cy) needs r = 5: interior 93..=96 touches the old frame column 97 only
        // through its own frame (also 97), which holds 0x20 only.
        assert_eq!(r.locate(&p, 1, &[], None), Some((95, 100)));
        let g = {
            let mut g = Grid(vec![0; 40000]);
            g.allocate(100, 100, 4, 4, true);
            g
        };
        assert!(g.taken(96, 100, 4, 4), "interior on the old frame");
        assert!(!g.taken(95, 100, 4, 4));
    }

    #[test]
    fn terrain_vehicles_and_bounds_block() {
        let mut r = rules();
        // A 1 × 1 at (x, y) occupies cell (x, y − 1) (odd heights lean to low y). Terrain cell (25, 24)
        // → grid 100..104 × 96..100 covers (100, 99).
        r.terrains[0].mask[24 * 13 + 6] = 0b1100;
        let p = planet(&[]);
        assert_ne!(r.locate(&p, 3, &[], None), Some((100, 100)), "terrain under the start");
        assert_eq!(r.locate(&p, 3, &[], None), Some((99, 100)), "1×1: no frame test, the left neighbour is free");
        r.terrains[0].mask = [0; 650];
        assert_eq!(r.locate(&p, 3, &[(100, 100)], None), Some((99, 100)), "a vehicle at (100, 100) blocks (100, 99) too");
        assert_eq!(r.locate(&p, 1, &[], Some((5, 100))), None, "start outside 10..=189");
        let g = Grid(vec![0; 40000]);
        assert!(g.taken(14, 100, 1, 1) && g.taken(100, 185, 1, 1) && !g.taken(15, 184, 1, 1), "edge band");
    }

    #[test]
    fn centroid_truncates_after_every_add() {
        let r = rules();
        // (int)((f32)0 + 100.9) = 100, (int)(100 + 100.9) = 200 → 100, not 100.9.
        let p = planet(&[(3, 100.9, 60.0), (3, 100.9, 60.0)]);
        let start = r.locate(&p, 3, &[], None).unwrap();
        assert_eq!(start.1, 60, "centroid row");
        assert!((start.0 - 100).abs() <= 2);
    }
}
