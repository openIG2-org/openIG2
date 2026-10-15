//! Replays values recorded from the running GOG `ig2.exe` (tools/trace/run_trace.py, 2026-10-15).
//! The fixture holds numbers only (RNG states/results, game-time samples), no game assets.

use ig2_sim::rng::{self, Streams};
use ig2_sim::time::{GameTime, Speed};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/time_rng_trace.json")).unwrap()
}

#[test]
fn rng_matches_every_traced_call() {
    let f = fixture();
    let calls = f["rnd"].as_array().unwrap();
    assert!(calls.len() >= 300);
    let mut streams = Streams::default();
    for c in calls {
        let s = c["s"].as_u64().unwrap() as usize;
        let (lo, hi) = (c["lo"].as_i64().unwrap() as i32, c["hi"].as_i64().unwrap() as i32);
        streams.set_state(s, c["before"].as_u64().unwrap() as u32);
        let r = streams.range(s, lo, hi);
        assert_eq!(streams.state(s), c["after"].as_u64().unwrap() as u32, "state, call {c}");
        assert_eq!(r as i64, c["r"].as_i64().unwrap(), "value, call {c}");
    }
    // The first 150 fixture calls are a contiguous slice of the trace: on each stream, a call's
    // `before` must equal the previous call's `after` (nothing else touched the state in between).
    let mut last = std::collections::HashMap::new();
    let (mut chained, mut pairs) = (0, 0);
    for c in calls.iter().take(150) {
        let s = c["s"].as_u64().unwrap();
        if let Some(prev) = last.insert(s, c["after"].as_u64().unwrap()) {
            pairs += 1;
            chained += (prev == c["before"].as_u64().unwrap()) as usize;
        }
    }
    assert_eq!(chained, pairs, "stream state changed outside rndlog");
    // And replaying a whole stream from its first state reproduces the traced results.
    let first = calls.iter().find(|c| c["s"] == 11).unwrap();
    let mut s = first["before"].as_u64().unwrap() as u32;
    for c in calls.iter().take(150).filter(|c| c["s"] == 11) {
        s = rng::step(s);
        assert_eq!(s as u64, c["after"].as_u64().unwrap());
    }
}

#[test]
fn calendar_and_speed_match_traced_ticks() {
    let f = fixture();
    let ticks = f["ticks"].as_array().unwrap();
    for t in ticks {
        let g = GameTime(t["ido"].as_i64().unwrap() as i32);
        assert_eq!(
            (g.year() as i64, g.month() as i64, g.day() as i64),
            (t["ev"].as_i64().unwrap(), t["honap"].as_i64().unwrap(), t["nap"].as_i64().unwrap())
        );
        let mult = t["mult"].as_i64().unwrap() as i32;
        assert!([Speed::Normal, Speed::Fast, Speed::Fastest].iter().any(|s| s.ido_per_tick(false) == mult));
    }
    // Every 4th tick was kept: within one speed segment Ido advances by a multiple of the multiplier
    // (4× when no tick was skipped) and the samples are 200 ms apart at 20 Hz.
    let mut exact = 0;
    for w in ticks.windows(2) {
        if w[0]["mult"] == w[1]["mult"] {
            let m = w[0]["mult"].as_i64().unwrap();
            let d = w[1]["ido"].as_i64().unwrap() - w[0]["ido"].as_i64().unwrap();
            assert_eq!(d % m, 0);
            exact += (d == 4 * m) as usize;
        }
    }
    assert!(exact * 10 >= ticks.len() * 9, "only {exact} of {} samples are exactly 4 ticks apart", ticks.len());
}
