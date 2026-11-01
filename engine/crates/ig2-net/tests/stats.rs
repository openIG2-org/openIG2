//! ADR 0034: the game's statistics fill a day at a time from a running game and travel in our save format.

mod common;

use ig2_net::snapshot::Snapshot;
use ig2_sim::game::Game;
use ig2_sim::save::va;

/// Forty game days of quick-1.sav: a day each day change, a sample each (the step is 1 this early), the human's
/// planets in every row; the save keeps the statistics bit for bit and the hash covers them.
#[test]
fn statistics_fill_daily_and_round_trip() {
    let Some((rules, state)) = common::setup("quick-1.sav") else { return };
    let mut g = Game::new(state, &rules);
    g.saved();
    g.set_speed(3, false);
    let en = g.state.mem.i32(va::EN) as usize;
    let mut days = 0;
    let mut nap = g.state.mem.i32(va::NAP);
    while days < 40 {
        g.advance_clock();
        g.run_tick();
        g.events.clear();
        let now = g.state.mem.i32(va::NAP);
        if now != nap {
            nap = now;
            days += 1;
        }
    }
    let st = &g.stats;
    assert_eq!(st.days, 40);
    assert_eq!(st.samples.len(), 40);
    assert!(st.samples.windows(2).all(|w| w[1].ido > w[0].ido));
    assert!(st.samples.iter().all(|s| s.rows[en - 1].planets > 0), "the human owns planets on every day");
    let last = st.samples.last().unwrap();
    assert_eq!(last.rows[en - 1], ig2_sim::stats::sample(&g.state).rows[en - 1], "nothing changed since the day's sample");

    let file = Snapshot::capture(&g, &[], 0, 0).to_file(0, false);
    let (snap, _) = Snapshot::from_file(&file).unwrap();
    let (back, _) = snap.restore(&rules).unwrap();
    assert_eq!(back.stats, g.stats);
    let h = ig2_net::snapshot::state_hash(&g, &[]);
    let mut other = back;
    other.stats.built[0] += 1;
    assert_ne!(ig2_net::snapshot::state_hash(&other, &[]), h, "the statistics are in the hash");
}
