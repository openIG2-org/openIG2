//! PowerVR container v3 (`PVR\x03`). This is a public format (Imagination Technologies spec).
//!
//! Formats found in the GOG install (survey 2026-10-15, 2,081 files):
//! generic RGB565 (1,840), generic RGBA4444 (230), BC1/DXT1 (9), generic L8 (2).

use crate::{format_err, Result};

const VERSION: u32 = 0x0352_5650; // "PVR\x03" read as LE u32
const HEADER_LEN: usize = 52;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// Uncompressed. `channels` are ASCII channel names in order. `bits` are per channel.
    Generic { channels: [u8; 4], bits: [u8; 4] },
    /// Compressed format ID from the PVR spec (e.g. 7 = BC1).
    Compressed(u32),
}

#[derive(Debug, Clone)]
pub struct Pvr<'a> {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub surfaces: u32,
    pub faces: u32,
    pub mip_count: u32,
    pub format: PixelFormat,
    pub premultiplied: bool,
    pub metadata: &'a [u8],
    data: &'a [u8],
}

/// An RGBA8 image, rows top to bottom.
#[derive(Debug, Clone)]
pub struct Rgba8 {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl<'a> Pvr<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN {
            return Err(format_err("PVR", "file shorter than header"));
        }
        let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        if u32_at(0) != VERSION {
            return Err(format_err("PVR", "not a PVR v3 file"));
        }
        let pf = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let format = if pf >> 32 == 0 {
            PixelFormat::Compressed(pf as u32)
        } else {
            PixelFormat::Generic {
                channels: (pf as u32).to_le_bytes(),
                bits: ((pf >> 32) as u32).to_le_bytes(),
            }
        };
        let meta_len = u32_at(48) as usize;
        let data_start = HEADER_LEN + meta_len;
        if bytes.len() < data_start {
            return Err(format_err("PVR", "metadata runs past end of file"));
        }
        Ok(Self {
            premultiplied: u32_at(4) & 0x2 != 0,
            format,
            height: u32_at(24),
            width: u32_at(28),
            depth: u32_at(32).max(1),
            surfaces: u32_at(36).max(1),
            faces: u32_at(40).max(1),
            mip_count: u32_at(44).max(1),
            metadata: &bytes[HEADER_LEN..data_start],
            data: &bytes[data_start..],
        })
    }

    /// A named shader constant from the GOG remaster's metadata block (fourCC `SGLB`, key `CNST`: `u32` count, then
    /// per constant `u32` name length, the name, `f32 × 4`; `sg3d::texture_t::load_pvr` hands them to the material,
    /// e.g. the starmap planet maps' `uv_mul` = (1, 0.5, 0, 0): the maps hold the surface twice, one above the other).
    pub fn constant(&self, name: &str) -> Option<[f32; 4]> {
        let m = self.metadata;
        let u32_at = |o: usize| m.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        let mut o = 0;
        while o + 12 <= m.len() {
            let (fourcc, key, len) = (&m[o..o + 4], &m[o + 4..o + 8], u32_at(o + 8)? as usize);
            let data = m.get(o + 12..o + 12 + len)?;
            if fourcc == b"BLGS" && key == b"TSNC" {
                let d = |p: usize| data.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
                let mut p = 4;
                for _ in 0..d(0)? {
                    let n = d(p)? as usize;
                    let found = data.get(p + 4..p + 4 + n)? == name.as_bytes();
                    let v = data.get(p + 4 + n..p + 20 + n)?;
                    if found {
                        return Some(std::array::from_fn(|i| f32::from_le_bytes(v[4 * i..4 * i + 4].try_into().unwrap())));
                    }
                    p += 20 + n;
                }
            }
            o += 12 + len;
        }
        None
    }

    fn level_size(&self, w: u32, h: u32) -> usize {
        match self.format {
            PixelFormat::Generic { bits, .. } => {
                let bpp: u32 = bits.iter().map(|&b| b as u32).sum();
                (w as usize * h as usize * bpp as usize).div_ceil(8)
            }
            PixelFormat::Compressed(7) => w.div_ceil(4) as usize * h.div_ceil(4) as usize * 8,
            PixelFormat::Compressed(_) => 0,
        }
    }

    /// Decode the top mip level of the first surface, face and slice.
    pub fn decode_rgba8(&self) -> Result<Rgba8> {
        let (w, h) = (self.width, self.height);
        let size = self.level_size(w, h);
        let src = self
            .data
            .get(..size)
            .ok_or_else(|| format_err("PVR", "pixel data truncated"))?;
        let pixels = match self.format {
            PixelFormat::Generic { channels, bits } => decode_generic(src, w, h, channels, bits)?,
            PixelFormat::Compressed(7) => decode_bc1(src, w, h),
            PixelFormat::Compressed(id) => {
                return Err(format_err("PVR", format!("unsupported compressed format id {id}")));
            }
        };
        Ok(Rgba8 { width: w, height: h, pixels })
    }
}

fn expand(v: u32, bits: u8) -> u8 {
    match bits {
        0 => 0,
        8 => v as u8,
        b => ((v * 255 + ((1 << b) - 1) / 2) / ((1 << b) - 1)) as u8,
    }
}

fn decode_generic(src: &[u8], w: u32, h: u32, channels: [u8; 4], bits: [u8; 4]) -> Result<Vec<u8>> {
    let n = channels.iter().take_while(|&&c| c != 0).count();
    let total: u32 = bits[..n].iter().map(|&b| b as u32).sum();
    if !matches!(total, 8 | 16 | 24 | 32) {
        return Err(format_err("PVR", format!("unsupported generic pixel size {total} bits")));
    }
    let bytes_pp = (total / 8) as usize;
    // Packed formats (565, 4444) put the first channel in the most significant bits of an LE word.
    // Byte-aligned formats (8 bits per channel) store channels in byte order.
    let byte_order = bits[..n].iter().all(|&b| b == 8);
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for px in src.chunks_exact(bytes_pp).take(w as usize * h as usize) {
        let mut rgba = [0u8, 0, 0, 255];
        let word = px.iter().rev().fold(0u32, |acc, &b| (acc << 8) | b as u32);
        let mut shift = total;
        for (i, (&ch, &b)) in channels[..n].iter().zip(&bits[..n]).enumerate() {
            let v = if byte_order {
                px[i] as u32
            } else {
                shift -= b as u32;
                (word >> shift) & ((1 << b) - 1)
            };
            let v = expand(v, b);
            match ch {
                b'r' => rgba[0] = v,
                b'g' => rgba[1] = v,
                b'b' => rgba[2] = v,
                b'a' => rgba[3] = v,
                b'l' | b'i' => rgba[..3].fill(v),
                _ => {}
            }
        }
        out.extend_from_slice(&rgba);
    }
    Ok(out)
}

fn rgb565(c: u16) -> [u8; 4] {
    [expand((c >> 11) as u32, 5), expand(((c >> 5) & 63) as u32, 6), expand((c & 31) as u32, 5), 255]
}

fn decode_bc1(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![0u8; w * h * 4];
    for (bi, block) in src.chunks_exact(8).enumerate() {
        let (bx, by) = (bi % w.div_ceil(4) * 4, bi / w.div_ceil(4) * 4);
        let c0 = u16::from_le_bytes([block[0], block[1]]);
        let c1 = u16::from_le_bytes([block[2], block[3]]);
        let (p0, p1) = (rgb565(c0), rgb565(c1));
        let mix = |a: u8, b: u8, wa: u16, wb: u16| ((a as u16 * wa + b as u16 * wb) / (wa + wb)) as u8;
        let palette: [[u8; 4]; 4] = if c0 > c1 {
            [p0, p1, std::array::from_fn(|i| if i == 3 { 255 } else { mix(p0[i], p1[i], 2, 1) }),
                std::array::from_fn(|i| if i == 3 { 255 } else { mix(p0[i], p1[i], 1, 2) })]
        } else {
            [p0, p1, std::array::from_fn(|i| if i == 3 { 255 } else { mix(p0[i], p1[i], 1, 1) }), [0, 0, 0, 0]]
        };
        let idx = u32::from_le_bytes(block[4..8].try_into().unwrap());
        for py in 0..4 {
            for px in 0..4 {
                let (x, y) = (bx + px, by + py);
                if x < w && y < h {
                    let c = palette[((idx >> (2 * (py * 4 + px))) & 3) as usize];
                    out[(y * w + x) * 4..][..4].copy_from_slice(&c);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_come_from_the_cnst_block() {
        let mut meta = Vec::new();
        meta.extend_from_slice(b"BLGSVNXT");
        meta.extend_from_slice(&4u32.to_le_bytes());
        meta.extend_from_slice(&11u32.to_le_bytes());
        meta.extend_from_slice(b"BLGSTSNC");
        meta.extend_from_slice(&30u32.to_le_bytes());
        meta.extend_from_slice(&1u32.to_le_bytes());
        meta.extend_from_slice(&6u32.to_le_bytes());
        meta.extend_from_slice(b"uv_mul");
        for x in [1.0f32, 0.5, 0.0, 0.0] {
            meta.extend_from_slice(&x.to_le_bytes());
        }
        let mut f = header(7, 4, 4);
        f[48..52].copy_from_slice(&(meta.len() as u32).to_le_bytes());
        f.extend_from_slice(&meta);
        f.extend_from_slice(&[0; 8]);
        let p = Pvr::parse(&f).unwrap();
        assert_eq!(p.constant("uv_mul"), Some([1.0, 0.5, 0.0, 0.0]));
        assert_eq!(p.constant("uv_add"), None);
    }

    fn header(pf: u64, w: u32, h: u32) -> Vec<u8> {
        let mut v = Vec::new();
        for x in [VERSION, 0] {
            v.extend_from_slice(&x.to_le_bytes());
        }
        v.extend_from_slice(&pf.to_le_bytes());
        for x in [0u32, 0, h, w, 1, 1, 1, 1, 0] {
            v.extend_from_slice(&x.to_le_bytes());
        }
        v
    }

    #[test]
    fn rgb565_pure_colours() {
        let pf = u64::from_le_bytes(*b"rgb\0\x05\x06\x05\0");
        let mut f = header(pf, 3, 1);
        for c in [0xF800u16, 0x07E0, 0x001F] {
            f.extend_from_slice(&c.to_le_bytes());
        }
        let img = Pvr::parse(&f).unwrap().decode_rgba8().unwrap();
        assert_eq!(img.pixels, [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn rgba4444_alpha_in_low_nibble() {
        let pf = u64::from_le_bytes(*b"rgba\x04\x04\x04\x04");
        let mut f = header(pf, 1, 1);
        f.extend_from_slice(&0xF00Au16.to_le_bytes());
        let img = Pvr::parse(&f).unwrap().decode_rgba8().unwrap();
        assert_eq!(img.pixels, [255, 0, 0, 170]);
    }
}
