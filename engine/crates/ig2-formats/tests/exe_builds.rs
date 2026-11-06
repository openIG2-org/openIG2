//! The Steam `ig2.exe` rebuilt in the GOG layout against the GOG `ig2.exe` (`re/formats/exe-builds.md`). Needs
//! `IG2_INSTALL_DIR` (GOG) and `IG2_STEAM_DIR` (the Steam folder); skips without them.

use ig2_formats::exe_builds::{converted_from, to_gog_layout};

#[test]
fn steam_exe_matches_gog_in_the_read_tables() {
    let (Some(gog), Some(steam)) = (std::env::var_os("IG2_INSTALL_DIR"), std::env::var_os("IG2_STEAM_DIR")) else {
        eprintln!("IG2_INSTALL_DIR or IG2_STEAM_DIR not set; skipping");
        return;
    };
    let gog = std::fs::read(std::path::Path::new(&gog).join("ig2.exe")).unwrap();
    let steam = std::fs::read(std::path::Path::new(&steam).join("ig2.exe")).unwrap();
    let conv = to_gog_layout(&steam).expect("a known build");
    assert_eq!(converted_from(&conv), Some("Steam 2018-04-18"));
    assert_eq!(conv.len(), gog.len());
    // (file offset, length) in the GOG layout: the tables (`.data` raw), the strings (`.rdata`) and the decoded code
    let file = |va: u32, len: usize| -> (usize, usize) {
        let (sva, off) = if va >= 0x8de000 { (0x8de000, 0x4dc800) } else if va >= 0x835000 { (0x835000, 0x434200) } else { (0x401000, 0x400) };
        ((va - sva + off) as usize, len)
    };
    let checks = [
        ("tables", file(0x8de000, 0x510bc)),
        ("strings", file(0x835eb7, 0x12641)),
        ("InitColonySpeech", file(0x47f400, 0x37ee)),
        ("InitSpaceWarSpeech", file(0x55e8b0, 0x2cfb)),
        ("StarmapInitSpeech", file(0x5ad820, 0x1055)),
    ];
    for (name, (at, len)) in checks {
        // a converted dword may differ only where it is zero-filled or a pointer into unmapped code
        let bad: Vec<usize> = (at..at + len).filter(|&i| conv[i] != gog[i]).collect();
        let shown: Vec<String> = bad.iter().take(8).map(|&i| format!("{i:#x}: {:02x} != {:02x}", conv[i], gog[i])).collect();
        assert!(bad.len() * 50 < len, "{name}: {} differing bytes, first {shown:?}", bad.len());
        eprintln!("{name}: {} differing bytes of {len}, first {shown:?}", bad.len());
    }
}
