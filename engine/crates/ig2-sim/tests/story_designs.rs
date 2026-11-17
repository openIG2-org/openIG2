//! `StoryDesignData` as `NEWGAME_Story` (exe 0x5c3040) leaves it: fleet.txt's `D` lines, then designs.txt
//! (`ParseDesignData` exe 0x72ad70; `re/rules/story-data.md`), against the exe's live table.

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::game::StoryRules;

fn i32_at(r: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(r[o..o + 4].try_into().unwrap())
}

fn name(r: &[u8]) -> String {
    String::from_utf8_lossy(&r[4..0x1c]).trim_end_matches('\0').to_owned()
}

#[test]
fn story_designs_match_the_exe() {
    let Some(inst) = Install::from_env(Lang::En).map(|i| i.expect("valid install")) else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    let s = StoryRules::new(Vec::new(), Vec::new(), &[], Vec::new(), &inst);
    // Observed in ig2.exe (replay.js dump, replay-campaign-c1-x16): StoryDesignDataMut 0x1a9, StoryDesignElsoTerv 0x27e9.
    assert_eq!((s.designs.len(), s.designs_next), (425, 10217));
    let r = &s.designs[0];
    assert_eq!((i32_at(r, 0), name(r).as_str(), i32_at(r, 0x20), i32_at(r, 0x24), i32_at(r, 0x28)), (11, "Starlight", 4, 5, 2));
    assert_eq!((i32_at(r, 0x64), i32_at(r, 0x30), i32_at(r, 0x68), i32_at(r, 0x34)), (17, 3, 33, 1));
    let r = &s.designs[59];
    assert_eq!((i32_at(r, 0), name(r).as_str(), i32_at(r, 0x1c), i32_at(r, 0x20), i32_at(r, 0x24), i32_at(r, 0x28)),
        (2201, "Talos Mothership", 0, 5, 8, 7));
    assert_eq!((i32_at(r, 0x60 + 4 * 7), i32_at(r, 0x2c + 4 * 7)), (67, 4));
    assert_eq!((i32_at(&s.designs[208], 0), name(&s.designs[208]).as_str()), (10000, "Toxic"));
    assert_eq!((i32_at(&s.designs[424], 0), name(&s.designs[424]).as_str()), (10216, "b9-zs"));

    // The whole table, byte for byte, when the recording is here.
    let dir = std::env::var_os("IG2_TRACE_DIR").map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let Ok(text) = std::fs::read_to_string(dir.join("replay-campaign-c1-x16/replay.jsonl")) else {
        eprintln!("replay-campaign-c1-x16 missing; the byte compare skipped");
        return;
    };
    let first: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    let hex = first["vol"]["mem"].as_array().unwrap().iter().find(|e| e[0] == "stdesigns").unwrap()[2].as_str().unwrap().to_owned();
    let exe: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
    let ours = s.designs.concat();
    assert_eq!(exe.len(), ours.len());
    for (k, (a, b)) in exe.chunks(0x94).zip(ours.chunks(0x94)).enumerate() {
        assert_eq!(a, b, "record {k} (id {})", i32_at(a, 0));
    }
}
