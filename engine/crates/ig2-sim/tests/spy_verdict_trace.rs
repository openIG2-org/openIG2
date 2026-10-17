//! Replays the spy screen's verdict on a caught enemy spy recorded from the running GOG `ig2.exe`
//! (tools/trace/run_screen.py `--kep 7 --spies verdict --verdict N --jsonl …`, re/screens/spy.md §6.2): the verdict
//! buttons 0x14..0x17 pressed through the game's own input, each on a fresh report (kind 3) with the prisoner's
//! loyalty, double-agent field and stream 0's state fuzzed. `Spy_Main` (exe 0x58d410) rolls `rndlog(0, 1, loyalty +
//! 3)` (spy.cpp line 0x313) and `rndlog(0, 1, 2)` (0x318); each record holds the choice, the prisoner's two fields,
//! the stream's state before and after, the report's follow-up kind and the fate `Multi_Send(0x4a)` carries. The
//! port (`Spies::judge`) must give all three.
//! Traces hold game state, so they stay in `_local/` (never committed). The test skips unless the traces exist
//! (`IG2_TRACE_DIR`, else the repository's `_local/traces`).

use std::path::PathBuf;

use ig2_sim::rng::Streams;
use ig2_sim::spy::{skill, Spies};
use serde_json::Value;

fn traces(prefix: &str) -> Vec<PathBuf> {
    let dir = std::env::var_os("IG2_TRACE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    v.retain(|p| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with(prefix) && f.ends_with(".jsonl")));
    v.sort();
    v
}

#[test]
fn verdict_matches_the_exe() {
    let files = traces("spyverdict-");
    if files.is_empty() {
        eprintln!("no spyverdict-*.jsonl in the traces; run tools/trace/run_screen.py --kep 7 --spies verdict --verdict N --jsonl …");
        return;
    }
    let mut n = 0;
    let mut kinds = [0usize; 16];
    for f in files {
        for line in std::fs::read_to_string(&f).unwrap().lines() {
            let r: Value = serde_json::from_str(line).unwrap();
            if r["k"] != "verdict" {
                continue;
            }
            let u = |k: &str| r[k].as_u64().unwrap() as usize;
            let (owner, k, choice) = (u("owner"), u("spy"), u("choice") as i32);
            let mut spies = Spies::default();
            spies.roster[owner][k].skills[skill::LOYALTY] = u("loyalty") as u8;
            spies.roster[owner][k].master = u("master") as u8;
            let mut rng = Streams::default();
            rng.set_state(0, u("seed0") as u32);
            let (fate, kind) = spies.judge(&mut rng, owner, k, choice);
            let want = (r["fate"].as_i64().unwrap() as i32, u("kind") as u8, u("seed1") as u32);
            assert_eq!((fate, kind, rng.state(0)), want, "{}: {line}", f.display());
            kinds[kind as usize & 15] += 1;
            n += 1;
        }
    }
    eprintln!("{n} verdicts, follow-up kinds 8..=0xe: {:?}", &kinds[8..15]);
    assert!(n > 0);
}
