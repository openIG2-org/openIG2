//! ADR 0061: a mod's story runs in a real game's story check and its state travels in our save format and the hash.

mod common;

use ig2_net::snapshot::Snapshot;
use ig2_sim::game::Game;

/// A made-up story: it fires in the first check, sets a flag and a counter.
const STORY: &str = r#"
[[story]]
name = "first"
do = [{ set_flag = "started" }, { add_counter = { name = "n", value = 3 } }]

[[story]]
name = "second"
when = [{ flag = "started" }, { counter = { name = "n", at_least = 3 } }]
do = [{ add_counter = { name = "n", value = 1 } }, { start_timer = { name = "later", days = 100 } }]

[[story]]
name = "daily"
repeat = true
times = 2
do = [{ add_counter = { name = "days", value = 1 } }]
"#;

#[test]
fn mod_story_state_round_trips_and_is_hashed() {
    let Some((mut rules, state)) = common::setup("quick-1.sav") else { return };
    rules.story.mods = ig2_sim::story_mods::parse(STORY, "test").unwrap().into();
    let mut g = Game::new(state, &rules);
    g.saved();
    g.set_speed(3, false);
    for _ in 0..200 {
        g.advance_clock();
        g.run_tick();
        g.events.clear();
    }
    let st = &g.state.mod_stories;
    assert!(st.fired.contains("test/first") && st.fired.contains("test/second"), "{st:?}");
    assert_eq!(st.counters["n"], 4, "each story fired once");
    assert_eq!((st.counters["days"], st.repeats["test/daily"].1), (2, 2), "the repeating one twice");
    assert!(st.timers.contains_key("later"));

    let file = Snapshot::capture(&g, &[], 0, 0).to_file(0, false);
    let (snap, _) = Snapshot::from_file(&file).unwrap();
    let (back, _) = snap.restore(&rules).unwrap();
    assert_eq!(back.state.mod_stories, g.state.mod_stories);
    let h = ig2_net::snapshot::state_hash(&g, &[]);
    assert_eq!(ig2_net::snapshot::state_hash(&back, &[]), h);
    let mut other = back;
    other.state.mod_stories.flags.insert("x".into());
    assert_ne!(ig2_net::snapshot::state_hash(&other, &[]), h, "the story mods' state is in the hash");
    other.state.mod_stories = g.state.mod_stories.clone();
    other.state.mod_stories.timers.insert("later".into(), 0);
    assert_ne!(ig2_net::snapshot::state_hash(&other, &[]), h, "timers too");
}

/// A version 14 state (part 1: no repeats, no timers) loads with both empty.
#[test]
fn version_14_story_state_loads() {
    let Some((mut rules, state)) = common::setup("quick-1.sav") else { return };
    rules.story.mods = ig2_sim::story_mods::parse(STORY, "test").unwrap().into();
    let mut g = Game::new(state, &rules);
    g.state.mod_stories.flags.insert("old".into());
    g.state.mod_stories.counters.insert("n".into(), 7);
    let mut snap = Snapshot::capture(&g, &[], 0, 0);
    // The state ends with the two empty maps' u32 lengths: without them it is what version 14 wrote.
    assert!(snap.state.ends_with(&[0; 8]));
    snap.state.truncate(snap.state.len() - 8);
    let (back, _) = snap.restore(&rules).unwrap();
    assert_eq!(back.state.mod_stories, g.state.mod_stories);
}
