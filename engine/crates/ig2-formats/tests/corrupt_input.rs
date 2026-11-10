//! Damaged files give an `Err`, never a panic: a sample of the user's install files and saves (`IG2_SAVE_DIR`, else
//! `_local/saves`), cut short at many lengths and with bytes overwritten, through every binary parser. Without the
//! install the test skips.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use ig2_formats::text::Lang;
use ig2_formats::{ani, colony_map, dialogue, diplomacy, formation, misc_text, mp4, pvr, save, scn, story_data, text, tutorial, xv,
    Install};

/// A deterministic xorshift, so a failure names a reproducible case.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// The damaged copies of `b`: cut at 0, at each of the first 64 lengths and at 24 more, and 24 copies with 1–8 bytes
/// overwritten.
fn variants(b: &[u8], seed: u64) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut rng = Rng(seed | 1);
    let mut cuts: Vec<usize> = (0..b.len().min(64)).collect();
    cuts.extend((0..24).map(|_| rng.next() as usize % b.len().max(1)));
    for c in cuts {
        out.push((format!("cut at {c}"), b[..c].to_vec()));
    }
    for k in 0..24 {
        let mut v = b.to_vec();
        if v.is_empty() {
            break;
        }
        for _ in 0..1 + rng.next() % 8 {
            let at = rng.next() as usize % v.len();
            v[at] = rng.next() as u8;
        }
        out.push((format!("overwrite {k}"), v));
    }
    out
}

static FAILS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn no_panic(what: &str, data: &[u8], f: impl Fn(&[u8])) {
    for (case, v) in variants(data, data.len() as u64 ^ 0x9e37_79b9_7f4a_7c15) {
        std::panic::set_hook(Box::new(|_| {}));
        let r = catch_unwind(AssertUnwindSafe(|| f(&v)));
        let _ = std::panic::take_hook();
        if let Err(e) = r {
            let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
            FAILS.lock().unwrap().push(format!("{what}: {case} panics: {}", msg.unwrap_or_default()));
            return;
        }
    }
}

/// Every parser on a damaged file of its kind returns (`Ok` or `Err`) instead of panicking.
#[test]
fn damaged_files_dont_panic() {
    let Some(install) = Install::from_env(Lang::En) else { return };
    let install = install.unwrap();
    let sample = |prefix: &str, ext: &str, n: usize| -> Vec<(String, Vec<u8>)> {
        let files: Vec<&str> = install.list(prefix).into_iter().filter(|f| f.ends_with(ext)).collect();
        let step = (files.len() / n).max(1);
        files.iter().step_by(step).take(n).map(|f| (f.to_string(), install.read(f).unwrap())).collect()
    };
    for (f, b) in sample("", ".pvr", 12) {
        no_panic(&f, &b, |v| {
            if let Ok(p) = pvr::Pvr::parse(v) {
                let _ = p.decode_levels();
                let _ = p.constant("uv_mul");
            }
        });
    }
    for (f, b) in sample("", ".xv", 12) {
        no_panic(&f, &b, |v| {
            if let Ok(a) = xv::Archive::parse(v) {
                let _ = a.scene(None);
                for m in &a.members {
                    let _ = xv::Material::parse(m.data);
                    let _ = xv::Mesh::parse(m.data);
                    let _ = xv::NodeHeader::parse(m.data);
                }
            }
        });
    }
    for (f, b) in sample("", ".ani", 4) {
        no_panic(&f, &b, |v| {
            if let Ok(a) = ani::Ani::parse(v) {
                let _ = a.frames();
            }
        });
    }
    for (f, b) in sample("dataraw/colony/gen_maps/", "", 4) {
        no_panic(&f, &b, |v| drop(colony_map::ColonyMap::parse(v)));
    }
    for (f, b) in sample("dataraw/spacewar/", ".bin", 4) {
        no_panic(&f, &b, |v| drop(formation::parse(v)));
    }
    for (f, b) in sample("", ".mou", 4) {
        no_panic(&f, &b, |v| drop(tutorial::parse_mou(v)));
    }
    // The smallest video: the demuxer reads every sample table.
    let videos = sample("anim/mpeg4/", ".mp4", 400);
    if let Some((f, b)) = videos.iter().min_by_key(|(_, b)| b.len()) {
        no_panic(f, b, |v| drop(mp4::parse(v)));
    }
    // Text files (a mod can replace any of them): damaged bytes decode to any characters.
    type TextParser = fn(&str);
    let texts: [(&str, &str, TextParser); 22] = [
        ("dataraw/text/building.txt", "", |t| drop(story_data::parse_buildings(t))),
        ("dataraw/text/designs.txt", "", |t| drop(story_data::parse_designs(t))),
        ("dataraw/text/values.txt", "", |t| drop(story_data::parse_values(t))),
        ("dataraw/text/fleet.txt", "", |t| drop(story_data::parse_fleets(t))),
        ("dataraw/text/infostr.txt", "", |t| drop(misc_text::parse_infostr(t))),
        ("dataraw/text/res_info.txt", "", |t| drop(misc_text::parse_res_info(t))),
        ("dataraw/text/fajinfo.txt", "", |t| drop(misc_text::parse_fajinfo(t))),
        ("dataraw/text/gameinfo.txt", "", |t| drop(misc_text::parse_gameinfo(t))),
        ("dataraw/text/fabscr.txt", "", |t| drop(misc_text::parse_fabscr(t))),
        ("dataraw/text/autobld", "", |t| drop(misc_text::parse_autobuild(t))),
        ("", ".dsc", |t| drop(misc_text::parse_dsc(t))),
        ("", ".scn", |t| drop(scn::Scenario::parse(t))),
        ("dataraw/uzenetek/uzenetek.txt", "", |t| drop(dialogue::parse_message_index(t))),
        ("dataraw/uzenetek/msg", "", |t| drop(dialogue::parse_msg_script(t))),
        ("dataraw/uzenetek/txt", "", |t| drop(dialogue::parse_txt_script(t))),
        ("dataraw/text/uzenetanim_", "", |t| drop(dialogue::parse_clip_lists(t))),
        ("dataraw/diplomac/ajanlat.eng", "", |t| drop(diplomacy::PhraseFile::parse(t))),
        ("dataraw/diplomac/valaszok.eng", "", |t| drop(diplomacy::PhraseFile::parse(t))),
        ("dataraw/diplomac/special.eng", "", |t| drop(diplomacy::parse_specials(t))),
        ("", ".tut", |t| drop(tutorial::parse_script(t))),
        ("dataraw/colony/terepleiro.txt", "", |t| drop(colony_map::parse_surfaces(t))),
        ("dataraw/colony/terepleiro.txt", "", |t| drop(colony_map::parse_backgrounds(t))),
    ];
    for (prefix, ext, parse) in texts {
        let files = sample(prefix, ext, 3);
        assert!(!files.is_empty(), "no {prefix}*{ext}");
        for (f, b) in files {
            no_panic(&f, &b, |v| parse(&text::decode(v, Lang::En)));
        }
    }
    let dir = std::env::var_os("IG2_SAVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/saves"));
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        let Ok(b) = std::fs::read(&p) else { continue };
        let name = p.display().to_string();
        match p.extension().and_then(|x| x.to_str()) {
            Some("sav") => {
                no_panic(&name, &b, |v| {
                    let _ = save::Header::read(v);
                    let _ = save::SaveGame::parse(v);
                });
                // Damage inside the compressed stream reaches the schema walk only through a bad inflate; damage
                // the decompressed body too.
                if let Ok(s) = save::SaveGame::parse(&b) {
                    no_panic(&format!("{name} body"), &s.body, |v| drop(save::SaveGame::parse_stream(v.to_vec(), false)));
                }
            }
            Some("mpstate") => no_panic(&name, &b, |v| drop(save::SaveGame::parse_multiplayer(v))),
            _ => {}
        }
    }
    let fails = FAILS.lock().unwrap();
    assert!(fails.is_empty(), "{}", fails.join("
"));
}
