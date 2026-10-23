//! The scrolling line's motion (`FRISSIT_Scroll2` exe 0x556210, `re/rules/info-messages.md` §2.3) against the
//! exe: `run_replay.py --scroll2` logs every call that changed the line (`_local/traces/scroll2-quick1/replay.jsonl`,
//! `{"k": "scroll2", "rows"}`: start, speed, limit as f32 bits, the text length and the queue length, before and
//! after). Each logged step must be [`Motion::advance`], or after a pop [`Motion::begin`] of the new text first.

use ig2_sim::messages::Motion;
use serde_json::Value;
use std::path::PathBuf;

fn local(var: &str, sub: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local").join(sub))
}

#[test]
fn scroll_steps_match_the_exe() {
    let path = local("IG2_TRACE_DIR", "traces").join("scroll2-quick1/replay.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: no {}", path.display());
        return;
    };
    let rows: Vec<Vec<i64>> = text
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["k"] == "scroll2")
        .flat_map(|e| e["rows"].as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect()).collect::<Vec<_>>())
        .collect();
    assert!(rows.len() > 1000, "only {} rows", rows.len());
    let motion = |r: &[i64]| Motion { start: f32::from_bits(r[0] as u32), speed: f32::from_bits(r[1] as u32), limit: f32::from_bits(r[2] as u32) };
    let (mut pops, mut holds) = (0, 0);
    for (i, r) in rows.iter().enumerate() {
        let (pre, post) = (&r[..5], &r[5..]);
        let (len, db) = (pre[3], pre[4]);
        let mut m = motion(pre);
        if len == 0 || len as f32 <= m.start {
            // A pop: the queue's first text goes on the line (or the line is cleared when nothing waits).
            if db < 1 {
                assert_eq!((post[3], post[4]), (0, db), "row {i}: {r:?}");
                continue;
            }
            assert_eq!(post[4], db - 1, "row {i}: {r:?}");
            m = Motion::begin(post[3] as usize);
            pops += 1;
        } else {
            assert_eq!((post[3], post[4]), (len, db), "row {i}: {r:?}");
        }
        if post[3] > 0 {
            if m.start == m.limit {
                holds += 1;
            }
            m.advance();
        }
        let got = [m.start.to_bits(), m.speed.to_bits(), m.limit.to_bits()];
        let want = [post[0] as u32, post[1] as u32, post[2] as u32];
        assert_eq!(got, want, "row {i}: {r:?} ({m:?})");
    }
    eprintln!("{} steps, {pops} pops, {holds} hold steps", rows.len());
    assert!(pops >= 6 && holds > 400);
}
