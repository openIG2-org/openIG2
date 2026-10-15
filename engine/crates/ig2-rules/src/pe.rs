//! Minimal PE32 reader: map image VAs of initialised data to file bytes.

use crate::{Error, Result};

pub struct PeImage<'a> {
    bytes: &'a [u8],
    image_base: u32,
    /// (virtual address, raw size, raw file offset)
    sections: Vec<(u32, u32, u32)>,
}

impl<'a> PeImage<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let u16_at = |o: usize| bytes.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
        let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        let pe = u32_at(0x3C).ok_or(Error::Pe("no e_lfanew"))? as usize;
        if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(Error::Pe("missing PE signature"));
        }
        let nsec = u16_at(pe + 6).ok_or(Error::Pe("truncated"))? as usize;
        let opt_size = u16_at(pe + 20).ok_or(Error::Pe("truncated"))? as usize;
        if u16_at(pe + 24) != Some(0x10B) {
            return Err(Error::Pe("not PE32"));
        }
        let image_base = u32_at(pe + 24 + 28).ok_or(Error::Pe("truncated"))?;
        let mut sections = Vec::with_capacity(nsec);
        for i in 0..nsec {
            let o = pe + 24 + opt_size + 40 * i;
            let va = u32_at(o + 12).ok_or(Error::Pe("truncated section table"))?;
            let raw_size = u32_at(o + 16).ok_or(Error::Pe("truncated section table"))?;
            let raw_off = u32_at(o + 20).ok_or(Error::Pe("truncated section table"))?;
            sections.push((va, raw_size, raw_off));
        }
        Ok(Self { bytes, image_base, sections })
    }

    /// `len` bytes at image VA `va` (must lie in one section's file-backed data).
    pub fn read(&self, va: u32, len: usize) -> Result<&'a [u8]> {
        let rva = va.checked_sub(self.image_base).ok_or(Error::Unmapped(va))?;
        for &(sva, size, off) in &self.sections {
            if rva >= sva && (rva - sva) as usize + len <= size as usize {
                let start = (off + rva - sva) as usize;
                return self.bytes.get(start..start + len).ok_or(Error::Unmapped(va));
            }
        }
        Err(Error::Unmapped(va))
    }
}
