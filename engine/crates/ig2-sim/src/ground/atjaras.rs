//! The planet's passability maps: `PlanetAtjarasMap::GenerateMap(p − 1, false)` (exe 0x479300, ELF
//! 0x4ae090), built lazily by `FUN_0045f600` at a planet's first path search (spec
//! `re/rules/ground-combat.md` §11.13).
//!
//! Heights sampled at every grid cell centre (`GetHeight`, a quarter of a grid cell per height cell), four
//! slope classes per inner cell, then per vehicle layer 1..=4 a gentle-cell map, a flood fill from a
//! spiral-searched start (layer 1 repeats it until 7000 cells are reached) and the colony map's obstacle
//! mask. Bit-exact against the exe (`tests/atjaras_trace.rs`).

use super::realtime::GroundMap;

/// A cell a vehicle can drive on (the path search's cost grid starts from it).
pub const FREE: u16 = 65000;
/// A cell it can't.
pub const BLOCKED: u16 = 65001;
/// A gentle cell before the flood fill, and a reached region too small for layer 1.
const GENTLE: u16 = 1;
const SMALL: u16 = 20;
const N: usize = 200;

/// `GenerateMap`'s result in game mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passability {
    /// The planet object's map pointers (`PlanetAtjarasMap` 0xA33388 + p·0x24, `[0..6]`), as
    /// [`super::GroundField::passability`] indexes them by unit kind: `[0]` (the editor's layer 0) stays
    /// empty, `[1..=4]` are the vehicle layers, `[5]` is layer 1 again (the exe copies the pointer).
    /// Each is 200 × 200, index `x·200 + y`, [`FREE`] or [`BLOCKED`].
    pub maps: Vec<Vec<u16>>,
    /// The flood fill's start cell left in the globals 0x99644C / 0x996450 (the last one layer 1 found).
    pub start: (i32, i32),
    /// Layer 1 found no region of 7000 cells: the exe then refills the region at (100, 100) forever (or,
    /// when that cell is blocked, floods the blocked cells and the bytes around the map) [original hang];
    /// the port stops after that fill.
    pub hung: bool,
    /// How many flood fills layer 1 needed (1 + the regions under 7000 cells it dropped).
    pub fills: u32,
}

/// Slope class: `(i8)(i32)(clamp((f32)((f64)d·0.0488), ±2.9f)·10)` (asm 0x479580). The exe compares in
/// f64 against ±2.9 and stores the f32 constants; no f32 lies strictly between ±2.9f and ±2.9, so an f32
/// clamp is the same.
fn slope_class(d: f32) -> i8 {
    let v = ((d as f64 * 0.0488) as f32).clamp(-2.9, 2.9);
    (v * 10.0) as i32 as i8
}

fn gentle(class: i8, limit: f32) -> bool {
    (class as i32).abs() as f32 <= limit
}

/// `FUN_0048b1a0(map, v)`: the first cell holding `v` on square rings around (99..100, 99..100), each ring
/// as the row `y = lo`, the row `y = hi`, the column `x = lo`, the column `x = hi`; none → (100, 100).
fn spiral(grid: &[u16], v: u16) -> Option<(i32, i32)> {
    let at = |x: i32, y: i32| grid[x as usize * N + y as usize] == v;
    for k in 0..=98 {
        let (lo, hi) = (99 - k, 100 + k);
        if let Some(x) = (lo..=hi).find(|&x| at(x, lo)) {
            return Some((x, lo));
        }
        if let Some(x) = (lo..=hi).find(|&x| at(x, hi)) {
            return Some((x, hi));
        }
        if let Some(y) = (lo..=hi).find(|&y| at(lo, y)) {
            return Some((lo, y));
        }
        if let Some(y) = (lo..=hi).find(|&y| at(hi, y)) {
            return Some((hi, y));
        }
    }
    None
}

/// `FUN_0048af10(map, slopes, 65000, sx, sy, limit)`: floods the 4-connected cells holding the start's
/// value `v` whose slope class from the cell it is reached from is gentle; every reached cell becomes
/// [`FREE`]; returns how many. The exe sweeps forward and backward until nothing changes; the slope test is
/// symmetric, so the reached set (and the count) is the connected component: a queue gives the same.
/// Neighbours are flat indices `n ± 1` (wrapping across rows) and `n ± 200`; outside the buffer the exe
/// reads heap bytes (never reached from a gentle start: the border is blocked), the port skips them.
fn flood(grid: &mut [u16], slopes: &[[i8; 4]], start: (i32, i32), limit: f32) -> usize {
    let n0 = (start.0 * N as i32 + start.1) as usize;
    let v = grid[n0];
    let mut mark = v.wrapping_add(1);
    if mark == FREE {
        mark = v.wrapping_add(2);
    }
    grid[n0] = mark;
    let mut queue = std::collections::VecDeque::from([n0]);
    let mut count = 0;
    while let Some(n) = queue.pop_front() {
        count += 1;
        let s = slopes[n];
        for (nb, class) in [(n.checked_sub(1), s[0]), (n.checked_sub(N), s[1]), (Some(n + N), s[2]), (Some(n + 1), s[3])] {
            if let Some(nb) = nb.filter(|&nb| nb < N * N) {
                if grid[nb] == v && gentle(class, limit) {
                    grid[nb] = mark;
                    queue.push_back(nb);
                }
            }
        }
        grid[n] = FREE;
    }
    count
}

fn replace(grid: &mut [u16], from: u16, to: u16) {
    grid.iter_mut().filter(|c| **c == from).for_each(|c| *c = to);
}

/// `GenerateMap(p − 1, false)` for a planet showing colony map `map` (heights; see [`GroundMap::heights`]
/// for the bytes past the grid) with that map's obstacle mask (`Terepek[m] + 0x330`, mask B: 50 rows × 13
/// bytes, 2 bits per 4 × 4-cell block) and the slope limits (`ig2_rules::Rules::ground_slope_limits`).
pub fn generate_map(map: &GroundMap, obstacles: &[u8], limits: &[f32]) -> Passability {
    // Heights at the cell centres, `GetHeight((f32)(((f64)x + 0.5)·0.25), (f32)(((f64)y + 0.5)·0.25))`
    // (the exe's `> 200 → 0` guard never fires).
    let centre = |i: usize| ((i as f64 + 0.5) * 0.25) as f32;
    let mut h = vec![0f32; N * N];
    for x in 0..N {
        for y in 0..N {
            h[x * N + y] = map.height(centre(x), centre(y));
        }
    }
    // Slope classes of the inner cells towards (x, y − 1), (x − 1, y), (x + 1, y), (x, y + 1) (the exe
    // stores all 8 neighbours; only these 4 are read). The border's stay unset in the exe (never read).
    let mut slopes = vec![[0i8; 4]; N * N];
    for x in 1..N - 1 {
        for y in 1..N - 1 {
            let n = x * N + y;
            slopes[n] = [n - 1, n - N, n + N, n + 1].map(|m| slope_class(h[m] - h[n]));
        }
    }
    let mut maps = vec![Vec::new(); 6];
    let mut start = (0, 0);
    let mut hung = false;
    let mut fills = 0;
    for layer in 1..=4 {
        let limit = limits[layer];
        let mut grid = vec![BLOCKED; N * N];
        for x in 1..N - 1 {
            for y in 1..N - 1 {
                let n = x * N + y;
                let ok = slopes[n].map(|c| gentle(c, limit));
                let count = ok.iter().filter(|&&g| g).count();
                // Two gentle directions count only when they are on different axes (asm 0x4796e8).
                let open = layer == 3 || count > 2 || (count == 2 && !(ok[0] && ok[3]) && !(ok[1] && ok[2]));
                if open {
                    grid[n] = GENTLE;
                }
            }
        }
        loop {
            replace(&mut grid, FREE, SMALL);
            let mut found = true;
            if layer == 1 {
                let s = spiral(&grid, GENTLE);
                found = s.is_some();
                start = s.unwrap_or((100, 100));
            }
            let reached = flood(&mut grid, &slopes, start, limit);
            fills += (layer == 1) as u32;
            if reached >= 7000 || layer != 1 {
                break;
            }
            if !found {
                hung = true;
                break;
            }
        }
        replace(&mut grid, GENTLE, BLOCKED);
        replace(&mut grid, SMALL, BLOCKED);
        // Obstacles: block (col, row) of mask B covers x ∈ 4·col.., y ∈ 4·row.. (cols 0..50 of 52).
        for row in 0..50 {
            for col in 0..50 {
                if (obstacles[row * 13 + col / 4] >> (col % 4 * 2)) & 3 != 0 {
                    for x in 4 * col..4 * col + 4 {
                        grid[x * N + 4 * row..x * N + 4 * row + 4].fill(BLOCKED);
                    }
                }
            }
        }
        maps[layer] = grid;
    }
    maps[5] = maps[1].clone();
    Passability { maps, start, hung, fills }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placement::Terrain;

    /// Made-up limits of the same shape as the exe's (layer 3 above every class).
    const LIMITS: [f32; 5] = [0.1, 4.0, 6.0, 50.0, 8.0];

    fn flat(h: i16) -> GroundMap {
        GroundMap {
            terrain: Terrain { mask: [0; 650], kind: 0, start: (0, 0) },
            heights: vec![h; 2551],
            base: 0.0,
            footprints: Vec::new(),
        }
    }

    #[test]
    fn slope_classes() {
        assert_eq!(slope_class(0.0), 0);
        assert_eq!(slope_class(10.0), 4);
        assert_eq!(slope_class(-10.0), -4);
        assert_eq!(slope_class(1000.0), 29);
        assert_eq!(slope_class(-1000.0), -29);
        // The product is f64 (`mulsd`): an f32 one gives 10, 16 and 21 here (no shipped limit sits there,
        // so the trace can't tell them apart).
        assert_eq!(slope_class(22.540983), 11);
        assert_eq!(slope_class(34.836063), 17);
        assert_eq!(slope_class(-45.081966), -22);
    }

    #[test]
    fn flat_map_is_open_inside() {
        let limits = LIMITS;
        let mut obstacles = [0u8; 650];
        obstacles[13 * 10 + 2] = 0b11 << 2; // block (col 9, row 10): x 36..40, y 40..44
        let p = generate_map(&flat(11200), &obstacles, &limits);
        assert!(!p.hung);
        assert_eq!(p.start, (99, 99));
        for layer in 1..=4 {
            let g = &p.maps[layer];
            assert_eq!(g[0], BLOCKED);
            assert_eq!(g[N + 1], FREE);
            assert_eq!(g[198 * N + 198], FREE);
            assert_eq!(g[199 * N + 5], BLOCKED);
            assert_eq!(g[37 * N + 41], BLOCKED);
            assert_eq!(g.iter().filter(|&&c| c == FREE).count(), 198 * 198 - 16);
        }
        assert_eq!(p.maps[5], p.maps[1]);
        assert!(p.maps[0].is_empty());
    }

    #[test]
    fn small_regions_are_dropped_from_layer_1() {
        // Cliffs box in the centre (height cells 22..=28, grid cells ~88..112): under 7000 cells, so layer 1
        // drops it and keeps the outside; layer 3 (everything gentle) floods across the walls.
        let mut m = flat(11200);
        for i in 22..=28 {
            for (x, y) in [(i, 22), (i, 28), (22, i), (28, i)] {
                m.heights[y * 50 + x] = 20000;
            }
        }
        let p = generate_map(&m, &[0; 650], &LIMITS);
        assert!(!p.hung);
        assert!(p.fills > 1);
        assert_eq!(p.maps[1][100 * N + 100], BLOCKED);
        assert_eq!(p.maps[1][10 * N + 10], FREE);
        assert_eq!(p.maps[2][100 * N + 100], BLOCKED);
        assert_eq!(p.maps[3][100 * N + 100], FREE);
    }
}
