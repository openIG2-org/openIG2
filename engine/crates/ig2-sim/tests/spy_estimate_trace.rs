//! Replays the spy screen's chance estimate recorded from the running GOG `ig2.exe` (tools/trace/run_spy.py
//! `--estimate N`, re/screens/spy.md §6.2): direct calls of `KemEselySzamolas1(e, k, m, t, rounds, stream)` (exe
//! 0x587b80) on fuzzed rosters. Each record holds the inputs (the spy, the target's roster, the race, difficulty and
//! human flag, the mission record, the stream's state) and the result and the stream's state after; the port
//! (`Spies::escape_chance`) must give both.
//! Traces hold game state, so they stay in `_local/` (never committed). The test skips unless the traces exist
//! (`IG2_TRACE_DIR`, else the repository's `_local/traces`).

use std::path::PathBuf;

use ig2_sim::rng::Streams;
use ig2_sim::spy::{MissionRule, Spies, Spy, SpyContext, SpyRules, RECORD, SLOTS};
use serde_json::Value;

fn traces(prefix: &str) -> Vec<PathBuf> {
    let dir = std::env::var_os("IG2_TRACE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    v.retain(|p| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with(prefix) && f.ends_with(".jsonl")));
    v.sort();
    v
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

#[test]
fn chance_estimate_matches_the_exe() {
    let files = traces("spyest-");
    if files.is_empty() {
        eprintln!("no spyest-*.jsonl in the traces; run tools/trace/run_spy.py --estimate N");
        return;
    }
    let (mut n, mut guarded) = (0, 0);
    for f in files {
        for line in std::fs::read_to_string(&f).unwrap().lines() {
            let r: Value = serde_json::from_str(line).unwrap();
            if r["k"] != "estimate" {
                continue;
            }
            let u = |k: &str| r[k].as_u64().unwrap() as usize;
            let (e, k, m, t, stream) = (u("e"), u("spy"), u("m"), u("t"), u("stream"));
            let rounds = r["rounds"].as_i64().unwrap() as i32;
            let mb = unhex(r["mission"].as_str().unwrap());
            let rule = MissionRule { needs_target: mb[0x1c] != 0, no_countdown: mb[0x1e] != 0, ..Default::default() };
            let mut missions = vec![MissionRule::default(); 27];
            missions[m] = rule;
            let rules = SpyRules { missions, ..Default::default() };
            let mut spies = Spies::default();
            let guards = unhex(r["guards"].as_str().unwrap());
            for j in 0..SLOTS {
                spies.roster[t][j] = Spy::from_bytes(&guards[j * RECORD..(j + 1) * RECORD]);
            }
            spies.count[t] = r["count"].as_i64().unwrap() as i32;
            spies.roster[e][k] = Spy::from_bytes(&unhex(r["me"].as_str().unwrap()));
            let mut ctx = SpyContext { difficulty: r["diff"].as_i64().unwrap() as i32, multiplayer: r["mp"].as_u64().unwrap() != 0, ..Default::default() };
            ctx.race[e] = r["race"].as_u64().unwrap() as u8;
            ctx.human[e] = r["human"].as_u64().unwrap() == 1;
            let mut rng = Streams::default();
            rng.set_state(stream, r["seed0"].as_u64().unwrap() as u32);
            let got = spies.escape_chance(&ctx, &rules, &mut rng, e, k, m, t, rounds, stream);
            let want = r["result"].as_i64().unwrap() as i32;
            assert_eq!((got, rng.state(stream)), (want, r["seed1"].as_u64().unwrap() as u32), "{}: {line}", f.display());
            n += 1;
            if rule.needs_target && rng.state(stream) != r["seed0"].as_u64().unwrap() as u32 && got < 100 {
                guarded += 1;
            }
        }
    }
    eprintln!("{n} estimates, {guarded} with a guard catching");
    assert!(n > 0);
}
