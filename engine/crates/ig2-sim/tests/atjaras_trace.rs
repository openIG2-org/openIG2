//! Replays `PlanetAtjarasMap::GenerateMap(p − 1, 0)` (exe 0x479300) recorded from the running GOG `ig2.exe`
//! (tools/trace/run_atjmap.py): every colony map as loaded, then (with `--fuzz`) records with perturbed
//! heights, obstacle mask and next-record head. The port gets the live record (heights `+0x138c` plus the
//! 102 bytes after them, base `+0x84`, mask B `+0x330`) and the rules' slope limits, and must reproduce the
//! four maps cell for cell, the pointer copy and the flood fill's start globals.
//! Rules come from the user's install (`IG2_INSTALL_DIR`); traces from `_local/traces/atj-*.jsonl`
//! (`IG2_TRACE_DIR` overrides; game state: never committed). Skips without them.

use std::io::BufRead;
use std::path::PathBuf;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_rules::Rules;
use ig2_sim::ground::{generate_map, GroundMap};
use ig2_sim::placement::Terrain;
use serde_json::Value;

fn events() -> Vec<Value> {
    let dir = std::env::var_os("IG2_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("atj-"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for p in files {
        for line in std::io::BufReader::new(std::fs::File::open(p).unwrap()).lines() {
            let v: Value = serde_json::from_str(&line.unwrap()).unwrap();
            if v["k"] == "gen" {
                out.push(v);
            }
        }
    }
    out
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// A colony-map record as the exe holds it (`Terepek[m]`, 0x2714 bytes) plus the bytes after its height
/// grid (the head of the next record) → the port's map and obstacle mask.
pub fn map_of_record(rec: &[u8], tail: &[u8]) -> (GroundMap, Vec<u8>) {
    let heights = rec[0x138c..0x2714].iter().chain(tail).copied().collect::<Vec<u8>>();
    let map = GroundMap {
        terrain: Terrain { mask: rec[0xa6..0xa6 + 650].try_into().unwrap(), kind: 0, start: (0, 0) },
        heights: heights.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect(),
        base: f32::from_le_bytes(rec[0x84..0x88].try_into().unwrap()),
        footprints: Vec::new(),
    };
    (map, rec[0x330..0x330 + 650].to_vec())
}

fn unrle(v: &Value) -> Vec<u16> {
    let a = v.as_array().unwrap();
    let mut out = Vec::with_capacity(40000);
    for p in a.chunks(2) {
        out.extend(std::iter::repeat_n(p[0].as_u64().unwrap() as u16, p[1].as_u64().unwrap() as usize));
    }
    out
}

#[test]
fn generate_map_replay() {
    let Some(inst) = Install::from_env(Lang::En) else { return };
    let rules = Rules::from_install(&inst.expect("valid install")).expect("catalogued exe");
    let evs = events();
    if evs.is_empty() {
        eprintln!("no atj-*.jsonl traces: skipped");
        return;
    }
    let (mut exact, mut fuzzed, mut cells) = (0, 0, 0usize);
    let (mut retried, mut max_fills, mut tail_dependent, mut tail_plain) = (0, 0, 0, 0);
    let mut bad = Vec::new();
    for ev in &evs {
        let tag = format!("map {} planet {} (fuzzed {})", ev["m"], ev["p"], ev["fuzzed"]);
        let (map, obstacles) = map_of_record(&unhex(ev["record"].as_str().unwrap()), &unhex(ev["tail"].as_str().unwrap()));
        let got = generate_map(&map, &obstacles, &rules.ground_slope_limits);
        let mut ok = !got.hung;
        for layer in 1..=4 {
            let want = unrle(&ev["maps"][layer - 1]);
            let diff = (0..40000).filter(|&i| want[i] != got.maps[layer][i]).count();
            if diff > 0 {
                ok = false;
                bad.push(format!("{tag}: layer {layer}: {diff} cells differ"));
            }
            cells += 40000;
        }
        let after = (ev["after"][0].as_i64().unwrap() as i32, ev["after"][1].as_i64().unwrap() as i32);
        if got.start != after {
            ok = false;
            bad.push(format!("{tag}: start {:?} != {after:?}", got.start));
        }
        assert_eq!(ev["alias"], true, "{tag}: pointer [6] = [2]");
        assert_eq!(got.maps[5], got.maps[1]);
        exact += ok as usize;
        retried += (got.fills > 1) as usize;
        max_fills = max_fills.max(got.fills);
        // The heights past the grid (the next record's head) change the result?
        let mut cut = map.clone();
        cut.heights.truncate(2500);
        let cutm = generate_map(&cut, &obstacles, &rules.ground_slope_limits);
        if cutm.maps != got.maps {
            tail_dependent += 1;
            tail_plain += !ev["fuzzed"].as_bool().unwrap() as usize;
        }
        fuzzed += ev["fuzzed"].as_bool().unwrap() as usize;
    }
    eprintln!(
        "GenerateMap: {exact}/{} calls exact ({fuzzed} fuzzed), {cells} cells; layer 1 refilled in {retried} (up to {max_fills} fills); {tail_dependent} ({tail_plain} unfuzzed) depend on the bytes past the height grid",
        evs.len()
    );
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad.iter().take(20).cloned().collect::<Vec<_>>().join("\n"));
}
