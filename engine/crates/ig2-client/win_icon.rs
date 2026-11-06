//! A program file's icon on Windows (Explorer, shortcuts), shared by the client's and the check tool's `build.rs`: an
//! RGBA drawing written as an .ico and linked in as a resource through `windres` (MinGW; `WINDRES` names another).
//! Without one the build goes on with a warning. std alone (build scripts include it by path).

use std::path::PathBuf;
use std::process::Command;

/// An .ico of 32-bit images (`BITMAPINFOHEADER`, bottom-up BGRA and an empty AND mask) at `sizes`.
fn ico(rgba: &dyn Fn(u32) -> Vec<u8>, sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&n| {
            let px = rgba(n);
            let mut b = Vec::new();
            for v in [40u32, n, 2 * n] {
                b.extend(v.to_le_bytes());
            }
            b.extend(1u16.to_le_bytes());
            b.extend(32u16.to_le_bytes());
            b.extend([0u8; 24]);
            for y in (0..n).rev() {
                for x in 0..n {
                    let i = ((y * n + x) * 4) as usize;
                    b.extend([px[i + 2], px[i + 1], px[i], px[i + 3]]);
                }
            }
            b.extend(vec![0u8; (n.div_ceil(32) * 4 * n) as usize]);
            b
        })
        .collect();
    let mut out = Vec::new();
    for v in [0u16, 1, sizes.len() as u16] {
        out.extend(v.to_le_bytes());
    }
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&n, img) in sizes.iter().zip(&images) {
        let side = if n >= 256 { 0 } else { n as u8 };
        out.extend([side, side, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((img.len() as u32).to_le_bytes());
        out.extend(offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        out.extend(img);
    }
    out
}

/// Links `rgba`'s icon into the crate's binaries (Windows, gnu toolchain only).
pub fn link(name: &str, rgba: &dyn Fn(u32) -> Vec<u8>) {
    println!("cargo:rerun-if-env-changed=WINDRES");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico_path = out.join(format!("{name}.ico"));
    std::fs::write(&ico_path, ico(rgba, &[16, 24, 32, 48, 64, 256])).unwrap();
    let rc = out.join("icon.rc");
    std::fs::write(&rc, format!("1 ICON \"{}\"\n", ico_path.display().to_string().replace('\\', "/"))).unwrap();
    let obj = out.join("icon.o");
    let windres = std::env::var("WINDRES").unwrap_or_else(|_| "windres".into());
    match Command::new(&windres).arg("-O").arg("coff").arg("-i").arg(&rc).arg("-o").arg(&obj).status() {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", obj.display()),
        r => println!("cargo:warning=no program icon: {windres} failed ({r:?})"),
    }
}
