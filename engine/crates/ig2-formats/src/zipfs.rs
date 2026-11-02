//! A zip mod's files, read in place (ADR 0068 §2): the central directory once, each file from its offset on demand.
//! Stored and deflate entries only; ZIP64, encryption and other methods are refused. A zip is untrusted input: an entry
//! name that is absolute, has a drive letter, a `..` part or a NUL is refused, the entries are capped at [`MAX_ENTRIES`]
//! and each file at [`MAX_FILE`], and inflation stops at the entry's stated size.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};

/// The most entries a mod's zip may hold.
pub const MAX_ENTRIES: usize = 65_536;
/// The largest file a mod's zip may hold, unpacked.
pub const MAX_FILE: u64 = 1 << 30;

const EOCD: u32 = 0x0605_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;

#[derive(Debug, Clone, Copy)]
struct Entry {
    offset: u64,
    packed: u64,
    size: u64,
    method: u16,
    crc: u32,
}

/// A zip's directory: its files by their path inside the mod (`/`-separated, as stored; one top folder that wraps
/// every file stripped).
#[derive(Debug, Clone, Default)]
pub struct Zip {
    entries: BTreeMap<String, Entry>,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// The entry's path, or why it is refused (ADR 0068 §2); `None` for a folder entry.
fn clean_name(raw: &[u8]) -> Result<Option<String>, String> {
    let name = String::from_utf8_lossy(raw).replace('\\', "/");
    let bad = name.starts_with('/') || name.contains(':') || name.contains('\0') || name.split('/').any(|p| p == "..");
    if bad {
        return Err(format!("unsafe file name {name:?}"));
    }
    if name.ends_with('/') {
        return Ok(None);
    }
    let parts: Vec<&str> = name.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    Ok((!parts.is_empty()).then(|| parts.join("/")))
}

impl Zip {
    /// Read the directory of the zip `r`.
    pub fn open<R: Read + Seek>(r: &mut R) -> Result<Self, String> {
        let io = |e: std::io::Error| format!("not a readable zip: {e}");
        let len = r.seek(SeekFrom::End(0)).map_err(io)?;
        // The end record (22 bytes) is followed by a comment of at most 65,535 bytes.
        let tail_len = len.min(22 + 65_535);
        r.seek(SeekFrom::Start(len - tail_len)).map_err(io)?;
        let mut tail = vec![0; tail_len as usize];
        r.read_exact(&mut tail).map_err(io)?;
        let at = (0..tail.len().saturating_sub(21)).rev().find(|&i| u32_at(&tail, i) == EOCD).ok_or("not a zip file (no end of central directory)")?;
        let e = &tail[at..];
        let (count, size, start) = (u16_at(e, 10) as usize, u32_at(e, 12) as u64, u32_at(e, 16) as u64);
        if count == 0xffff || start == 0xffff_ffff || size == 0xffff_ffff {
            return Err("ZIP64 archives aren't supported; zip it again with a usual zip tool".into());
        }
        if count > MAX_ENTRIES {
            return Err(format!("more than {MAX_ENTRIES} files"));
        }
        if start + size > len {
            return Err("broken zip: the directory lies past the end of the file".into());
        }
        r.seek(SeekFrom::Start(start)).map_err(io)?;
        let mut dir = vec![0; size as usize];
        r.read_exact(&mut dir).map_err(io)?;
        let mut entries = BTreeMap::new();
        let mut i = 0;
        for _ in 0..count {
            if i + 46 > dir.len() || u32_at(&dir, i) != CENTRAL {
                return Err("broken zip: a damaged directory entry".into());
            }
            let (flags, method) = (u16_at(&dir, i + 8), u16_at(&dir, i + 10));
            let (crc, packed, size) = (u32_at(&dir, i + 16), u32_at(&dir, i + 20) as u64, u32_at(&dir, i + 24) as u64);
            let (name_len, extra_len, comment_len) = (u16_at(&dir, i + 28) as usize, u16_at(&dir, i + 30) as usize, u16_at(&dir, i + 32) as usize);
            let offset = u32_at(&dir, i + 42) as u64;
            let end = i + 46 + name_len + extra_len + comment_len;
            if end > dir.len() {
                return Err("broken zip: a damaged directory entry".into());
            }
            let name = clean_name(&dir[i + 46..i + 46 + name_len])?;
            i = end;
            let Some(name) = name else { continue };
            if flags & 1 != 0 {
                return Err(format!("{name}: encrypted files aren't supported"));
            }
            if method != 0 && method != 8 {
                return Err(format!("{name}: compression method {method} isn't supported (only stored and deflate)"));
            }
            if size > MAX_FILE || packed > MAX_FILE || offset == 0xffff_ffff || offset >= len {
                return Err(format!("{name}: too large or a broken offset"));
            }
            entries.insert(name, Entry { offset, packed, size, method, crc });
        }
        // "Compress this folder" wraps everything in one top folder: the mod's files are inside it.
        let top = entries.keys().next().and_then(|k| k.split_once('/')).map(|(t, _)| format!("{t}/"));
        if let Some(top) = top.filter(|t| entries.keys().all(|k| k.starts_with(t.as_str()))) {
            entries = entries.into_iter().map(|(k, e)| (k[top.len()..].to_owned(), e)).collect();
        }
        Ok(Self { entries })
    }

    /// The files' paths, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// The file `name` (as [`Zip::names`] gives it) of the zip `r`, unpacked and checked against its CRC.
    pub fn read<R: Read + Seek>(&self, r: &mut R, name: &str) -> Result<Vec<u8>, String> {
        let e = self.entries.get(name).ok_or_else(|| format!("{name}: not in the zip"))?;
        let io = |err: std::io::Error| format!("{name}: {err}");
        r.seek(SeekFrom::Start(e.offset)).map_err(io)?;
        let mut head = [0; 30];
        r.read_exact(&mut head).map_err(io)?;
        if u32_at(&head, 0) != LOCAL {
            return Err(format!("{name}: broken zip (no local header)"));
        }
        r.seek(SeekFrom::Current(u16_at(&head, 26) as i64 + u16_at(&head, 28) as i64)).map_err(io)?;
        let mut packed = Vec::new();
        r.take(e.packed).read_to_end(&mut packed).map_err(io)?;
        if packed.len() as u64 != e.packed {
            return Err(format!("{name}: broken zip (the file is cut short)"));
        }
        let data = match e.method {
            0 => packed,
            _ => miniz_oxide::inflate::decompress_to_vec_with_limit(&packed, e.size as usize).map_err(|err| format!("{name}: broken zip (deflate: {:?})", err.status))?,
        };
        if data.len() as u64 != e.size || crc32(&data) != e.crc {
            return Err(format!("{name}: broken zip (size or checksum differs)"));
        }
        Ok(data)
    }
}

/// The zip's CRC-32 (IEEE, reflected).
fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| std::array::from_fn(|n| (0..8).fold(n as u32, |c, _| if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 })));
    !data.iter().fold(!0u32, |c, &b| t[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8))
}

/// A zip of `files` (name, data, deflated), for tests here and in the crates that use mods.
pub fn build(files: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let (mut out, mut dir) = (Vec::new(), Vec::new());
    for &(name, data, deflate) in files {
        let packed = if deflate { miniz_oxide::deflate::compress_to_vec(data, 6) } else { data.to_vec() };
        let method: u16 = if deflate { 8 } else { 0 };
        let offset = out.len() as u32;
        let common = |v: &mut Vec<u8>| {
            v.extend(20u16.to_le_bytes());
            v.extend([0u8; 2]);
            v.extend(method.to_le_bytes());
            v.extend([0u8; 4]);
            v.extend(crc32(data).to_le_bytes());
            v.extend((packed.len() as u32).to_le_bytes());
            v.extend((data.len() as u32).to_le_bytes());
            v.extend((name.len() as u16).to_le_bytes());
            v.extend([0u8; 2]);
        };
        out.extend(LOCAL.to_le_bytes());
        common(&mut out);
        out.extend(name.as_bytes());
        out.extend(&packed);
        dir.extend(CENTRAL.to_le_bytes());
        dir.extend(20u16.to_le_bytes());
        common(&mut dir);
        dir.extend([0u8; 10]);
        dir.extend(offset.to_le_bytes());
        dir.extend(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend(&dir);
    out.extend(EOCD.to_le_bytes());
    out.extend([0u8; 4]);
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((files.len() as u16).to_le_bytes());
    out.extend((dir.len() as u32).to_le_bytes());
    out.extend(start.to_le_bytes());
    out.extend([0u8; 2]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_stored_and_deflated_files_under_one_top_folder() {
        let big = b"deflate me ".repeat(500);
        let z = build(&[("MyMod/dataraw/a_bmp.pvr", b"plain", false), ("MyMod/mod.toml", &big, true), ("MyMod/", b"", false)]);
        let zip = Zip::open(&mut Cursor::new(&z)).unwrap();
        assert_eq!(zip.names().collect::<Vec<_>>(), ["dataraw/a_bmp.pvr", "mod.toml"]);
        assert_eq!(zip.read(&mut Cursor::new(&z), "dataraw/a_bmp.pvr").unwrap(), b"plain");
        assert_eq!(zip.read(&mut Cursor::new(&z), "mod.toml").unwrap(), big);
        assert!(z.len() < big.len());
    }

    #[test]
    fn refuses_unsafe_names_and_broken_zips() {
        for bad in ["../evil.dll", "a/../../evil", "/etc/x", "C:/x", "a\\..\\b"] {
            let e = Zip::open(&mut Cursor::new(build(&[(bad, b"x", false)]))).unwrap_err();
            assert!(e.contains("unsafe file name"), "{bad}: {e}");
        }
        assert!(Zip::open(&mut Cursor::new(b"not a zip at all".to_vec())).unwrap_err().contains("not a zip"));
        // Cut short, and a flipped byte in a file: the directory or the file read fails.
        let z = build(&[("a.txt", b"hello hello hello", false)]);
        assert!(Zip::open(&mut Cursor::new(&z[..z.len() - 30])).is_err());
        let mut flipped = z.clone();
        flipped[36] ^= 0xff;
        let zip = Zip::open(&mut Cursor::new(&flipped)).unwrap();
        assert!(zip.read(&mut Cursor::new(&flipped), "a.txt").unwrap_err().contains("checksum"));
    }

    #[test]
    fn crc_matches_the_standard() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }
}
