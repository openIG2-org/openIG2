//! List the GOG remaster's PVR metadata (re/screens/render.md §3): per directory the `SGLB` keys, the `TXNV`
//! sampling words and mip counts, and every texture carrying `CNST` shader constants.
//!
//! `cargo run --release -p ig2-formats --example pvrmeta -- <install> [prefix]`

use std::collections::BTreeMap;

use ig2_formats::pvr::Pvr;
use ig2_formats::text::Lang;
use ig2_formats::Install;

/// The metadata blocks: (fourCC, key, data).
fn blocks(m: &[u8]) -> Vec<([u8; 4], [u8; 4], &[u8])> {
    let mut v = Vec::new();
    let mut o = 0;
    while o + 12 <= m.len() {
        let len = u32::from_le_bytes(m[o + 8..o + 12].try_into().unwrap()) as usize;
        let Some(data) = m.get(o + 12..o + 12 + len) else { break };
        v.push((m[o..o + 4].try_into().unwrap(), m[o + 4..o + 8].try_into().unwrap(), data));
        o += 12 + len;
    }
    v
}

/// The `CNST` block's constants.
fn constants(d: &[u8]) -> Vec<(String, [f32; 4])> {
    let u = |p: usize| d.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let mut v = Vec::new();
    let mut p = 4;
    for _ in 0..u(0).unwrap_or(0) {
        let Some(n) = u(p).map(|n| n as usize) else { break };
        let (Some(name), Some(x)) = (d.get(p + 4..p + 4 + n), d.get(p + 4 + n..p + 20 + n)) else { break };
        v.push((String::from_utf8_lossy(name).into_owned(), std::array::from_fn(|i| f32::from_le_bytes(x[4 * i..4 * i + 4].try_into().unwrap()))));
        p += 20 + n;
    }
    v
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let install = Install::open(&a[1], Lang::En).expect("install");
    let prefix = a.get(2).map_or("dataraw/", String::as_str);
    // directory → (keys/words/mips summary → count)
    let mut dirs: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for path in install.list(prefix).into_iter().filter(|p| p.ends_with(".pvr")) {
        let Ok(bytes) = install.read(path) else { continue };
        let Ok(p) = Pvr::parse(&bytes) else { continue };
        let mut keys = Vec::new();
        for (fourcc, key, d) in blocks(p.metadata) {
            let tag = format!("{}/{}", String::from_utf8_lossy(&fourcc), String::from_utf8_lossy(&key));
            match &key {
                b"VNXT" if d.len() >= 4 => keys.push(format!("TXNV={}", u32::from_le_bytes(d[..4].try_into().unwrap()))),
                b"TSNC" => {
                    let c = constants(d);
                    println!("CNST {path}: {c:?}");
                    keys.push(format!("CNST[{}]", c.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(",")));
                }
                _ => keys.push(tag),
            }
        }
        let dir = path.rsplit_once('/').map_or("", |x| x.0).to_owned();
        let fmt = match p.format {
            ig2_formats::pvr::PixelFormat::Generic { channels, bits } => format!("{}{:?}", String::from_utf8_lossy(&channels).trim_end_matches('\0'), bits.iter().filter(|&&b| b > 0).collect::<Vec<_>>()),
            ig2_formats::pvr::PixelFormat::Compressed(id) => format!("c{id}"),
        };
        *dirs.entry(dir).or_default().entry(format!("mips {} {fmt} {}", p.mip_count, keys.join(" "))).or_default() += 1;
    }
    for (dir, kinds) in dirs {
        for (k, n) in kinds {
            println!("{dir}: {n} × {k}");
        }
    }
}
