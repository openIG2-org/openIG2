//! The remaster's `ecGs` / `neGs` / `pcGs` asset container.
//!
//! Spec: `re/formats/ecgs-container.md`. Evidence (ELF `ig2`): `AssetHelper::RWBuffer_DetectType`
//! @0x8c9070, `StringEncoder::file_encdec` @0x8f4110, `StringEncoder::inthash` @0x8f15e0,
//! `string_hash_t::compute` @0x41c920, `compression::decompress` @0x8dd4f0.

use std::borrow::Cow;
use std::io::{self, Read, Seek, SeekFrom};

use crate::{format_err, Result};

const MAGIC_EC: u32 = 0x7347_6365; // "ecGs": encrypted + LZ4
const MAGIC_NE: u32 = 0x7347_656E; // "neGs": encrypted, seekable
const MAGIC_PC: u32 = 0x7347_6370; // "pcGs": LZ4 only

/// `StringEncoder::cryptKey` (.data @0x119e72c).
const CRYPT_KEY: u32 = 0x5F14_8D4B;
/// Literal @0xd3fd8d, assigned to `AssetHelper::filekey_base64` in `main`.
const FILEKEY_B64: &str = "6fCoWoWJGGw";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Plain,
    Encrypted,
    Compressed,
    EncryptedCompressed,
}

pub fn kind(data: &[u8]) -> ContainerKind {
    match data.get(..4).map(|m| u32::from_le_bytes(m.try_into().unwrap())) {
        Some(MAGIC_EC) if data.len() >= 12 => ContainerKind::EncryptedCompressed,
        Some(MAGIC_PC) if data.len() >= 12 => ContainerKind::Compressed,
        Some(MAGIC_NE) => ContainerKind::Encrypted,
        _ => ContainerKind::Plain,
    }
}

/// Thomas Wang's 32-bit integer hash (`StringEncoder::inthash`).
pub fn inthash(mut a: u32) -> u32 {
    a = a ^ 0x3D ^ (a >> 16);
    a = a.wrapping_add(a << 3);
    a ^= a >> 4;
    a = a.wrapping_mul(0x27D4_EB2D);
    a ^ (a >> 15)
}

/// Per-file key: `string_hash_t::compute(to_lower(basename))`. Bytes are hashed as signed chars.
pub fn file_key(path: &str) -> u32 {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    base.bytes().fold(0u32, |h, b| {
        h.wrapping_add(b.to_ascii_lowercase() as i8 as i32 as u32).wrapping_mul(0x1003F)
    })
}

/// The global 64-bit key (`AssetHelper::filekey`), derived the same way `RWBuffer_Getkey` does it.
pub fn global_key() -> u64 {
    let mut raw = base64_decode(FILEKEY_B64);
    raw.resize(raw.len().div_ceil(4) * 4, 0);
    let mut seed = CRYPT_KEY;
    for word in raw.chunks_exact_mut(4) {
        seed = inthash(seed);
        let w = u32::from_le_bytes(word.try_into().unwrap()).wrapping_sub(seed);
        word.copy_from_slice(&w.to_le_bytes());
    }
    u64::from_le_bytes(raw[..8].try_into().unwrap())
}

fn base64_decode(s: &str) -> Vec<u8> {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|&c| c != b'=') {
        let v = ALPHA.iter().position(|&a| a == c).expect("static key is valid base64") as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// XOR keystream from `StringEncoder::file_encdec`. Two lanes are re-seeded every 16 bytes,
/// which gives random access to any offset.
#[derive(Clone)]
pub struct Keystream {
    lane_a: u32,
    lane_b: u32,
}

impl Keystream {
    pub fn new(path: &str) -> Self {
        Self::with_keys(global_key(), file_key(path))
    }

    pub fn with_keys(gkey: u64, fkey: u32) -> Self {
        Self {
            lane_a: ((gkey >> 32) as u32).wrapping_add(inthash(fkey)),
            lane_b: (gkey as u32).wrapping_add(inthash(fkey.wrapping_add(0x54_4836))),
        }
    }

    fn block(&self, index: u32) -> [u8; 16] {
        let h = inthash(index);
        let (mut ka, mut kb) = (self.lane_a ^ h, self.lane_b ^ h);
        let mut out = [0u8; 16];
        for (i, word) in out.chunks_exact_mut(4).enumerate() {
            // Word parity alternates lanes, starting with on even words.
            let k = if i & 1 == 1 {
                ka = inthash(ka);
                ka
            } else {
                kb = inthash(kb);
                kb
            };
            word.copy_from_slice(&k.to_le_bytes());
        }
        out
    }

    /// XOR `buf` in place, as if it starts at `offset` in the encrypted stream.
    pub fn apply(&self, buf: &mut [u8], offset: u64) {
        let mut pos = offset;
        let mut done = 0;
        while done < buf.len() {
            let block = self.block((pos / 16) as u32);
            let start = (pos % 16) as usize;
            let n = (16 - start).min(buf.len() - done);
            for (b, k) in buf[done..done + n].iter_mut().zip(&block[start..start + n]) {
                *b ^= k;
            }
            done += n;
            pos += n as u64;
        }
    }
}

/// Decode a whole file. `path` only needs the right basename: the key ignores directories.
pub fn decode<'a>(path: &str, data: &'a [u8]) -> Result<Cow<'a, [u8]>> {
    match kind(data) {
        ContainerKind::Plain => Ok(Cow::Borrowed(data)),
        ContainerKind::Encrypted => {
            let mut buf = data[4..].to_vec();
            Keystream::new(path).apply(&mut buf, 0);
            Ok(Cow::Owned(buf))
        }
        k @ (ContainerKind::Compressed | ContainerKind::EncryptedCompressed) => {
            let ks = (k == ContainerKind::EncryptedCompressed).then(|| Keystream::new(path));
            let mut header = [0u8; 8];
            header.copy_from_slice(&data[4..12]);
            if let Some(ks) = &ks {
                ks.apply(&mut header, 0);
            }
            let packed = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
            let raw = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
            let body = data
                .get(12..12 + packed)
                .ok_or_else(|| format_err("ecGs", format!("{path}: packed length {packed} exceeds file")))?;
            let mut body = body.to_vec();
            if let Some(ks) = &ks {
                ks.apply(&mut body, 8);
            }
            lz4_flex::block::decompress(&body, raw)
                .map(Cow::Owned)
                .map_err(|e| format_err("ecGs", format!("{path}: LZ4: {e}")))
        }
    }
}

/// Seekable reader over a `neGs` stream (Ogg/MP4), decrypting on the fly.
/// Plain files pass through unchanged.
pub struct DecryptingReader<R> {
    inner: R,
    keystream: Option<Keystream>,
    header: u64,
    pos: u64,
}

impl<R: Read + Seek> DecryptingReader<R> {
    pub fn new(path: &str, mut inner: R) -> io::Result<Self> {
        let mut magic = [0u8; 4];
        inner.seek(SeekFrom::Start(0))?;
        let n = inner.read(&mut magic)?;
        let encrypted = n == 4 && u32::from_le_bytes(magic) == MAGIC_NE;
        let header = if encrypted { 4 } else { 0 };
        inner.seek(SeekFrom::Start(header))?;
        Ok(Self { inner, keystream: encrypted.then(|| Keystream::new(path)), header, pos: 0 })
    }
}

impl<R: Read + Seek> Read for DecryptingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if let Some(ks) = &self.keystream {
            ks.apply(&mut buf[..n], self.pos);
        }
        self.pos += n as u64;
        Ok(n)
    }
}

impl<R: Read + Seek> Seek for DecryptingReader<R> {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(p) => SeekFrom::Start(p + self.header),
            SeekFrom::Current(d) => SeekFrom::Current(d),
            SeekFrom::End(d) => SeekFrom::End(d),
        };
        let abs = self.inner.seek(target)?;
        if abs < self.header {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before payload start"));
        }
        self.pos = abs - self.header;
        Ok(self.pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_python_reference_vectors() {
        // Values from the reference implementation tools/ecgs_decode.py. That implementation was
        // validated by decoding all 17,575 files of the GOG install.
        assert_eq!(inthash(0), 0xC0A9_496A);
        assert_eq!(inthash(1), 0x2792_2C9D);
        assert_eq!(inthash(0xDEAD_BEEF), 0x572E_7C2D);
        assert_eq!(global_key(), 0xD42C_7906_4F4C_EEB0);
        assert_eq!(file_key("attack.dsc"), 0x5DE1_E192);
    }

    #[test]
    fn keystream_is_an_involution_at_any_offset() {
        let ks = Keystream::with_keys(0x0123_4567_89AB_CDEF, 0xDEAD_BEEF);
        let plain: Vec<u8> = (0..100u8).collect();
        for off in [0u64, 3, 8, 15, 16, 17, 1000] {
            let mut buf = plain.clone();
            ks.apply(&mut buf, off);
            assert_ne!(buf, plain);
            ks.apply(&mut buf, off);
            assert_eq!(buf, plain);
        }
    }

    #[test]
    fn split_application_equals_whole() {
        let ks = Keystream::with_keys(global_key(), file_key("x.ogg"));
        let mut whole = vec![0u8; 77];
        ks.apply(&mut whole, 5);
        let mut parts = vec![0u8; 77];
        let (a, b) = parts.split_at_mut(30);
        ks.apply(a, 5);
        ks.apply(b, 35);
        assert_eq!(whole, parts);
    }

    #[test]
    fn file_key_ignores_case_and_directories() {
        assert_eq!(file_key("dataraw/scenario/ATTACK.dsc"), file_key("attack.dsc"));
        assert_eq!(file_key("a\\b\\attack.dsc"), file_key("attack.dsc"));
    }
}
