//! `dataraw/colony/gen_maps/<n>.map`: precomputed colony (planet surface) terrain.
//!
//! Spec: `re/formats/colony-map.md`. Evidence: `InitHatterekEsTerepek()` (ELF 0x4bd5b0) reads the
//! file with `DatFile::dread` straight into `Terepek[n]` (struct stride 0x2720).
//! `CheckTerepTargyak` (ELF 0x4bd150) gives the mask addressing.
//!
//! Also `colony/terepleiro.txt` ("terrain description"), parsed by the same function: its `[felszin]`
//! sections describe the surfaces in map order ([`parse_surfaces`]).

use crate::{format_err, Result};

/// Mask/grid side length in cells. One cell covers 4×4 units of the 200×200 `Real2Grid` grid.
pub const SIDE: usize = 50;
/// Bytes per mask row: 13 bytes × 4 cells (2 bits each) = 52 cells. Columns 50–51 (the high nibble of
/// each row's last byte) are padding: `Decode2LargeMap` and `PlanetAtjarasMap::GenerateMap` read only
/// cells 0..50 of a row, so whatever the exporter left there is never used.
pub const MASK_STRIDE: usize = 13;
const MASK_LEN: usize = SIDE * MASK_STRIDE; // 650
const FIXED_LEN: usize = 2 * MASK_LEN + SIDE * SIDE + SIDE * SIDE * 2 + 4; // 8,804, then N×4 + 4

#[derive(Debug, Clone)]
pub struct ColonyMap {
    /// `Terepek+0xae`: the build mask (`Decode2LargeMap` → building placement). A cell is blocked when
    /// its 2 bits are non-zero. Terrain-object footprints are OR-ed in as 0b11 at load.
    pub mask_a: [u8; MASK_LEN],
    /// `Terepek+0x338`: the terrain-object (obstacle) mask. `CheckTerepTargyak` marks object
    /// footprints into it, and `PlanetAtjarasMap::GenerateMap` blocks those cells for the vehicle
    /// passability layers 1..=4 (layer 0 doesn't use it).
    pub mask_b: [u8; MASK_LEN],
    /// `Terepek+0x9d0`. 50×50 byte grid, row-major [inf: tile/texture IDs]. Loaded, but no code in the
    /// ELF reads it (the remaster textures the terrain from `textures/colony/terrainmap/<n>/`).
    pub tiles: Vec<u8>,
    /// `Terepek+0x1394`. 50×50 heights, row-major, **i16** (`fix2float` sign-extends; map 56 has
    /// negative ones). See [`ColonyMap::height_world`].
    pub heights: Vec<i16>,
    /// `Terepek+0x5cc`, with its count at `+0x9cc`. N little-endian f32. Loaded, never read by the ELF
    /// (dead 1999 editor data).
    pub extra: Vec<f32>,
    /// `Terepek+0x8c` as stored in the file. The loader turns it into `200 − base`
    /// ([`ColonyMap::base_height`]), which overwrites `terepleiro.txt`'s "special height".
    pub base: f32,
}

fn mask_get(mask: &[u8; MASK_LEN], x: usize, y: usize) -> u8 {
    (mask[y * MASK_STRIDE + x / 4] >> ((x % 4) * 2)) & 3
}

impl ColonyMap {
    pub fn parse(b: &[u8]) -> Result<Self> {
        let err = |m: &str| format_err("colony map", m.to_owned());
        if b.len() < FIXED_LEN + 4 {
            return Err(err("file too short"));
        }
        let mut o = 0;
        let mut take = |n: usize| {
            let s = &b[o..o + n];
            o += n;
            s
        };
        let mask_a: [u8; MASK_LEN] = take(MASK_LEN).try_into().unwrap();
        let mask_b: [u8; MASK_LEN] = take(MASK_LEN).try_into().unwrap();
        let tiles = take(SIDE * SIDE).to_vec();
        let heights = take(SIDE * SIDE * 2).chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        let n = u32::from_le_bytes(take(4).try_into().unwrap()) as usize;
        if b.len() != FIXED_LEN + 4 * n + 4 {
            return Err(err(&format!("size {} doesn't match list count {n}", b.len())));
        }
        let extra = take(4 * n).chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
        let base = f32::from_le_bytes(take(4).try_into().unwrap());
        Ok(Self { mask_a, mask_b, tiles, heights, extra, base })
    }

    pub fn mask_a(&self, x: usize, y: usize) -> u8 {
        mask_get(&self.mask_a, x, y)
    }

    pub fn mask_b(&self, x: usize, y: usize) -> u8 {
        mask_get(&self.mask_b, x, y)
    }

    pub fn tile(&self, x: usize, y: usize) -> u8 {
        self.tiles[y * SIDE + x]
    }

    /// The loaded base height `Terepek+0x8c`: `-(base − 200)` (`InitHatterekEsTerepek` after the
    /// `dread`; `GetHeightFloat` adds it to every sample).
    pub fn base_height(&self) -> f32 {
        -(self.base - 200.0)
    }

    /// Height of grid vertex `(x, y)` in world units, as `GetHeightFloat` (ELF 0x464090) uses it:
    /// `fix2float(h) + base_height`, `fix2float(h) = (f32)((f64)(i16)h / 16.0) − 700.0` (ELF 0x4bb7a0).
    /// One height cell spans 4 of the 200 `Real2Grid` cells; the terrain is `TEREPXSIZE` world units
    /// wide (the terrain mesh's bounding box, set in `Init3dColony`).
    pub fn height_world(&self, x: usize, y: usize) -> f32 {
        ((self.height(x, y) as f64 / 16.0) as f32 - 700.0) + self.base_height()
    }

    pub fn height(&self, x: usize, y: usize) -> i16 {
        self.heights[y * SIDE + x]
    }
}

/// One `[felszin]` section of `colony/terepleiro.txt`: section `n` (0-based) belongs to `gen_maps/<n>.map`
/// (`Terepek[n]`). Values are positional, one per line, `;` starts a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface {
    /// Surface type 0..=8 (forest, rocky, desert, volcanic, grass, Mars, icy, cracked, moon).
    pub kind: i32,
    /// Possible weathers (0 none, 1 rain, 2 snow, 3 sandstorm).
    pub weathers: Vec<i32>,
    /// Terrain object (`Terepek+0x11`, loaded by `Init3dColony`).
    pub model: String,
    /// Heightmap object, radar heightmap (`colony/bf.b`), radar pattern (`colony/talaj.b`):
    /// `Terepek+0x2f/+0x4d/+0x6b`. Stored, but the shipped code never opens these files
    /// (`Colony_Belep` copies the pattern name into a global nothing reads).
    pub height_model: String,
    pub radar_height: String,
    pub radar_pattern: String,
    /// "spec. magassag" (special height): read as a float + 110 into `Terepek+0x8c`, then
    /// overwritten by the map file's base height ([`ColonyMap::base_height`]). Dead.
    pub special_height: i32,
    /// Terrain-object description (`obj*.res`, `Terepek+0x90`, never opened) and passability map
    /// (`a.m`, read into a scratch buffer and dropped).
    pub objects: String,
    pub passability: String,
    /// "Az MI itt kezd el epiteni": where automatic building placement starts on an empty map
    /// (`Terepek+0x5c4/+0x5c8`; `re/rules/colony-placement.md` §3), in 200 × 200 grid cells.
    pub ai_build_start: (i32, i32),
    /// `colony/landzone.map`: read into a scratch buffer and dropped; landing zones are generated at
    /// runtime (`LandingZone::Generate`).
    pub landing_zone: String,
}

/// `[felszin]` sections of `colony/terepleiro.txt`, in file order (`[hatter]` backgrounds are skipped).
pub fn parse_surfaces(text: &str) -> Result<Vec<Surface>> {
    let err = |m: String| format_err("terepleiro.txt", m);
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.lines() {
        // Strip a `;` comment outside quotes.
        let mut quoted = false;
        let end = line.char_indices().find(|&(_, c)| {
            if c == '"' {
                quoted = !quoted;
            }
            c == ';' && !quoted
        });
        let v = line[..end.map_or(line.len(), |e| e.0)].trim();
        if v.is_empty() {
            continue;
        }
        if v.starts_with('[') && v.ends_with(']') {
            sections.push((v[1..v.len() - 1].to_owned(), Vec::new()));
        } else if let Some(s) = sections.last_mut() {
            s.1.push(v.to_owned());
        }
    }
    let ints = |v: &str| -> Result<Vec<i32>> {
        v.split(',').map(|x| x.trim().parse().map_err(|_| err(format!("bad number {v:?}")))).collect()
    };
    let unquote = |v: &str| v.trim_matches('"').to_owned();
    sections
        .iter()
        .filter(|s| s.0 == "felszin")
        .enumerate()
        .map(|(n, (_, v))| {
            if v.len() < 11 {
                return Err(err(format!("surface {n}: {} values, expected 11", v.len())));
            }
            let start = ints(&v[9])?;
            if start.len() != 2 {
                return Err(err(format!("surface {n}: AI build start {:?}", v[9])));
            }
            Ok(Surface {
                kind: ints(&v[0])?[0],
                weathers: ints(&v[1])?,
                model: unquote(&v[2]),
                height_model: unquote(&v[3]),
                radar_height: unquote(&v[4]),
                radar_pattern: unquote(&v[5]),
                special_height: ints(&v[6])?[0],
                objects: unquote(&v[7]),
                passability: unquote(&v[8]),
                ai_build_start: (start[0], start[1]),
                landing_zone: unquote(&v[10]),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_sections_are_positional() {
        let t = "; header
[hatter]
\"h.xvb\" ; bg
1,2,3

[felszin]
0 ; type
0,1
\"a;b.xvb\"
\"14.xvb\"
\"colony/bf.b\"
\"e.bmp\"
-831
\"o.res\"
\"a.m\"		; pass
100, 90 ; start
\"colony/landzone.map\"
";
        let s = parse_surfaces(t).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].kind, s[0].weathers.clone(), s[0].special_height), (0, vec![0, 1], -831));
        assert_eq!(s[0].model, "a;b.xvb", "a ; inside quotes isn't a comment");
        assert_eq!((s[0].passability.as_str(), s[0].ai_build_start), ("a.m", (100, 90)));
        assert_eq!(s[0].landing_zone, "colony/landzone.map");
    }

    #[test]
    fn layout_and_mask_addressing() {
        let mut f = vec![0u8; FIXED_LEN];
        f[MASK_STRIDE * 2 + 1] = 0b0000_1100; // mask_a row 2, cell x=5 → byte 1, bits 2..3
        f[2 * MASK_LEN + 3 * SIDE + 7] = 42; // tile (7,3)
        let n_off = FIXED_LEN - 4;
        f[n_off..n_off + 4].copy_from_slice(&2u32.to_le_bytes());
        f.extend_from_slice(&1.5f32.to_le_bytes());
        f.extend_from_slice(&2.5f32.to_le_bytes());
        f.extend_from_slice(&(-7.0f32).to_le_bytes());
        let m = ColonyMap::parse(&f).unwrap();
        assert_eq!(m.mask_a(5, 2), 3);
        assert_eq!(m.mask_a(4, 2), 0);
        assert_eq!(m.tile(7, 3), 42);
        assert_eq!(m.extra, [1.5, 2.5]);
        assert_eq!(m.base, -7.0);
    }
}
