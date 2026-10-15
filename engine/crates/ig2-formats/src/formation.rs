//! `dataraw/spacewar/felallas<n>.bin`: space-combat starting formations ("felállás").
//!
//! Spec: `re/formats/formation.md`. Evidence (ELF `ig2`, `../sources_base/spacewar.cpp`):
//! `SpwC::FelallasLoad(char*, felallastipus*)` (game), `SpwC::loadfelallas`/`savefelallas`
//! (editor; `SPACEWAR/%s.bin`).

use crate::{format_err, Result};

const RECORD: usize = 84;

#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    /// `RACSPONTTYPE`: grid point (x, y, z/layer, w) [inf: battle-grid cell + size class].
    pub cell: [i32; 4],
    /// Stored to `urhajo+0x30` [inf: slot kind/ship class].
    pub kind: u32,
    /// 64 bytes stored to `urhajo+0x78`. In shipped files: the cell again, a second grid point
    /// [inf: facing/target], then uninitialised memory (0xCD fill).
    pub extra: [u8; 64],
}

impl Slot {
    /// The second grid point inside `extra` (i32 at 16, 20, 24) [inf].
    pub fn extra_point(&self) -> [i32; 3] {
        std::array::from_fn(|i| i32::from_le_bytes(self.extra[16 + 4 * i..20 + 4 * i].try_into().unwrap()))
    }
}

pub fn parse(b: &[u8]) -> Result<Vec<Slot>> {
    let n = u32::from_le_bytes(b.get(..4).ok_or_else(|| format_err("formation", "too short"))?.try_into().unwrap()) as usize;
    if b.len() != 4 + n * RECORD {
        return Err(format_err("formation", format!("size {} != 4 + {n} × 84", b.len())));
    }
    Ok(b[4..]
        .chunks_exact(RECORD)
        .map(|r| {
            let i = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
            Slot { cell: [i(0), i(4), i(8), i(12)], kind: i(16) as u32, extra: r[20..84].try_into().unwrap() }
        })
        .collect())
}
