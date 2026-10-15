//! `.ani` "SpidyAni" v2: the 1999 palettised, delta-coded animation format.
//! In the GOG install these are the talking heads in `anim/fejek/` (14 files, 128×128, 100 frames).
//!
//! Spec: `re/formats/ani.md`. Evidence (ELF `ig2`, source `../sources_base/anim.cpp`):
//! `InitAnim(...)::$_0::operator()` @0x45ece0 (header), `LoadFrame` @0x45e460 (chunk sizes),
//! `OneFrame` @0x45da70 (pixel opcodes), `Decode(int)` (LZSS), `AnimMakePaletteTable` (palette).

use crate::{format_err, Result};

fn err(msg: impl Into<String>) -> crate::Error {
    format_err("ani", msg)
}

const HEADER_LEN: usize = 19;
const PAL_CHUNK: usize = 4 + 768;

#[derive(Debug, Clone)]
pub struct Ani<'a> {
    /// Type flags: 0x02 palette flag [unused in data], 0x04 = 256 colours (else 128), 0x20 = LZSS-packed.
    pub flags: u16,
    pub frame_count: u16,
    pub width: u16,
    pub height: u16,
    /// Header word at 0x11 [unk; 1 in all shipped files].
    pub blocks: u16,
    body: &'a [u8],
}

/// One decoded frame as RGBA8. Pixels never written by any frame so far are transparent.
#[derive(Debug, Clone)]
pub struct Frame {
    pub rgba: Vec<u8>,
}

impl<'a> Ani<'a> {
    pub fn parse(b: &'a [u8]) -> Result<Self> {
        if b.len() < HEADER_LEN || &b[..8] != b"SpidyAni" {
            return Err(err("not a SpidyAni file"));
        }
        if b[8] != b'2' {
            return Err(err(format!("unsupported version {:?}", b[8] as char)));
        }
        let w = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        Ok(Self { flags: w(9), frame_count: w(11), width: w(13), height: w(15), blocks: w(17), body: &b[HEADER_LEN..] })
    }

    pub fn packed(&self) -> bool {
        self.flags & 0x20 != 0
    }

    /// Decode every frame in order. Frames are deltas over the previous frame.
    pub fn frames(&self) -> Result<Vec<Frame>> {
        let n = self.width as usize * self.height as usize;
        let mut palette = [[0u8; 4]; 256];
        let mut indices: Vec<Option<u8>> = vec![None; n];
        let mut out = Vec::with_capacity(self.frame_count as usize);
        let mut pos = 0usize;
        let b = self.body;
        let mut scratch;
        while pos + 4 <= b.len() {
            match &b[pos..pos + 4] {
                b"Pal " => {
                    let p = b.get(pos + 4..pos + PAL_CHUNK).ok_or_else(|| err("truncated palette"))?;
                    for (i, c) in p.chunks_exact(3).enumerate() {
                        palette[i] = [c[0], c[1], c[2], 255];
                    }
                    pos += PAL_CHUNK;
                }
                b"Data" => {
                    let h = b.get(pos..pos + 10).ok_or_else(|| err("truncated data header"))?;
                    let raw = u16::from_le_bytes([h[4], h[5]]) as usize;
                    let packed_len = u16::from_le_bytes([h[6], h[7]]);
                    let (stream, advance): (&[u8], usize) = if self.packed() && packed_len != 0xFFFF {
                        let src = b.get(pos + 12..pos + 12 + packed_len as usize).ok_or_else(|| err("truncated packed frame"))?;
                        scratch = lzss_decode(src);
                        (&scratch, 12 + packed_len as usize)
                    } else {
                        let src = b.get(pos + 10..pos + 10 + raw).ok_or_else(|| err("truncated frame"))?;
                        (src, if self.packed() { 12 + raw } else { 10 + raw })
                    };
                    let used = apply_ops(stream, &mut indices)?;
                    if used > raw.max(stream.len()) {
                        return Err(err(format!("frame {} opcodes overran", out.len())));
                    }
                    let mut rgba = Vec::with_capacity(n * 4);
                    for px in &indices {
                        rgba.extend_from_slice(&px.map_or([0; 4], |i| palette[i as usize]));
                    }
                    out.push(Frame { rgba });
                    pos += advance;
                }
                tag => return Err(err(format!("unknown chunk {tag:?} at {}", pos + HEADER_LEN))),
            }
        }
        Ok(out)
    }
}

/// Apply one frame's opcode stream to the persistent index buffer. Returns bytes consumed.
fn apply_ops(s: &[u8], px: &mut [Option<u8>]) -> Result<usize> {
    let (mut i, mut o) = (0usize, 0usize);
    let byte = |i: usize| s.get(i).copied().ok_or_else(|| err("opcode stream truncated"));
    let word = |i: usize| Ok::<usize, crate::Error>(u16::from_le_bytes([byte(i)?, byte(i + 1)?]) as usize);
    while o < px.len() {
        let op = byte(i)?;
        match op {
            0x00..=0x7F => {
                px[o] = Some(op);
                o += 1;
                i += 1;
            }
            0x80 => {
                o += word(i + 1)?;
                i += 3;
            }
            0x81..=0xBF => {
                o += (op & 0x3F) as usize;
                i += 1;
            }
            0xC0 => {
                let (count, color) = (word(i + 1)?, byte(i + 3)?);
                let end = (o + count).min(px.len());
                px[o..end].fill(Some(color));
                o += count;
                i += 4;
            }
            0xC1..=0xFF => {
                let color = byte(i + 1)?;
                let end = (o + (op & 0x3F) as usize).min(px.len());
                px[o..end].fill(Some(color));
                o += (op & 0x3F) as usize;
                i += 2;
            }
        }
    }
    // `OneFrame` stops once the remaining-pixel counter reaches zero or would go negative, so a
    // skip or run may overshoot the end (e.g. "skip the rest": politikus.ani).
    Ok(i)
}

/// Okumura-style LZSS as in `Decode(int)`: 4 KiB ring buffer zero-filled, write position 0xFEE,
/// flag bits LSB-first (1 = literal), match = 12-bit position + 4-bit length (+3).
pub fn lzss_decode(src: &[u8]) -> Vec<u8> {
    let mut ring = [0u8; 4096];
    let mut r = 0xFEEusize;
    let mut out = Vec::with_capacity(src.len() * 2);
    let mut i = 0;
    while i < src.len() {
        let flags = src[i];
        i += 1;
        for bit in 0..8 {
            if i >= src.len() {
                return out;
            }
            if flags >> bit & 1 == 1 {
                let c = src[i];
                i += 1;
                out.push(c);
                ring[r] = c;
                r = (r + 1) & 0xFFF;
            } else {
                if i + 1 >= src.len() {
                    return out;
                }
                let (lo, hi) = (src[i] as usize, src[i + 1] as usize);
                i += 2;
                let mut p = lo | ((hi & 0xF0) << 4);
                for _ in 0..(hi & 0x0F) + 3 {
                    let c = ring[p];
                    p = (p + 1) & 0xFFF;
                    out.push(c);
                    ring[r] = c;
                    r = (r + 1) & 0xFFF;
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
    fn opcodes() {
        let mut px = vec![None; 10];
        // literal 5, skip 2, run 3×9, long run (u16=4) of 7
        let used = apply_ops(&[5, 0x82, 0xC3, 9, 0xC0, 4, 0, 7], &mut px).unwrap();
        assert_eq!(used, 8);
        assert_eq!(px, [Some(5), None, None, Some(9), Some(9), Some(9), Some(7), Some(7), Some(7), Some(7)]);
    }

    #[test]
    fn lzss_literal_and_match() {
        // flags 0b011: two literals 'a','b', then match at ring pos 0xFEE length 3 → "aba"
        let src = [0b0000_0011, b'a', b'b', 0xEE, 0xF0];
        assert_eq!(lzss_decode(&src), b"ababa");
    }
}
