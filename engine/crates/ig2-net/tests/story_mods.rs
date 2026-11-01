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
do = [{ add_counter = { name = "n", value = 1 } }]
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

    let file = Snapshot::capture(&g, &[], 0, 0).to_file(0, false);
    let (snap, _) = Snapshot::from_file(&file).unwrap();
    let (back, _) = snap.restore(&rules).unwrap();
    assert_eq!(back.state.mod_stories, g.state.mod_stories);
    let h = ig2_net::snapshot::state_hash(&g, &[]);
    assert_eq!(ig2_net::snapshot::state_hash(&back, &[]), h);
    let mut other = back;
    other.state.mod_stories.flags.insert("x".into());
    assert_ne!(ig2_net::snapshot::state_hash(&other, &[]), h, "the story mods' state is in the hash");
}
