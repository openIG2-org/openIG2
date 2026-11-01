//! A tiny made-up story over a synthetic world (no game content).

use super::*;
use crate::rng::Streams;
use crate::story::{flag, FleetName};

#[derive(Default)]
struct W {
    ido: i32,
    rng: Streams,
    /// Planet owners, slot 0 unused.
    owners: Vec<i32>,
    /// `(owner, story id, x, y)`, slot 0 unused.
    fleets: Vec<(i32, i32, f32, f32)>,
    alive: [bool; 19],
    events: Vec<StoryEvent>,
}

impl StoryWorld for W {
    fn ido(&self) -> i32 {
        self.ido
    }
    fn speed_mult(&self) -> i32 {
        1
    }
    fn human(&self) -> i32 {
        1
    }
    fn difficulty(&self) -> i32 {
        1
    }
    fn multiplayer(&self) -> bool {
        false
    }
    fn rng(&mut self) -> &mut Streams {
        &mut self.rng
    }
    fn planet_count(&self) -> i32 {
        self.owners.len() as i32 - 1
    }
    fn planet_owner(&self, p: i32) -> i32 {
        self.owners[p as usize]
    }
    fn planet_value(&self, _p: i32) -> i32 {
        0
    }
    fn planet_visibility(&self, _p: i32) -> i32 {
        1
    }
    fn fleet_count(&self) -> i32 {
        self.fleets.len() as i32 - 1
    }
    fn fleet_owner(&self, f: i32) -> i32 {
        self.fleets[f as usize].0
    }
    fn fleet_id(&self, f: i32) -> i32 {
        self.fleets[f as usize].1
    }
    fn empire_alive(&self, e: i32) -> bool {
        self.alive[e as usize]
    }
    fn planets_of(&self, _e: i32) -> i32 {
        1
    }
    fn money(&self, _e: i32) -> f64 {
        0.0
    }
    fn tech_status(&self, _t: i32, _e: i32) -> i32 {
        0
    }
    fn relation_level(&self, _a: i32, _b: i32) -> i32 {
        0
    }
    fn event(&mut self, ev: StoryEvent) {
        self.events.push(ev);
    }
    fn race(&self, e: i32) -> i32 {
        e + 100
    }
    fn planet_xy(&self, p: i32) -> (f32, f32) {
        (p as f32 * 100.0, 0.0)
    }
    fn fleet_xy(&self, f: i32) -> (f32, f32) {
        let r = self.fleets[f as usize];
        (r.2, r.3)
    }
    fn new_fleet(&mut self, _name: &FleetName, owner: i32, id: i32, x: f32, y: f32, _speed: f32) -> i32 {
        self.fleets.push((owner, id, x, y));
        self.fleet_count()
    }
}

fn world() -> W {
    let mut alive = [false; 19];
    alive[1] = true;
    alive[3] = true;
    W { owners: vec![0, 1, 3, 0], fleets: vec![(0xff, 0, 0.0, 0.0)], alive, ..W::default() }
}

const STORY: &str = r#"
[[story]]
name = "hello"
when = [{ day = 10 }, { planet_owner = { planet = 1, empire = "human" } }]
do = [{ message = 9001 }, { set_flag = "greeted" }, { add_counter = { name = "favours", value = 2 } }]

[[story]]
name = "yes"
when = [{ flag = "greeted" }, { answered = { message = 9001, answer = 2 } }]
do = [{ give_planet = { planet = 3, empire = "human" } },
      { spawn_fleet = { id = 20001, owner = 3, planet = 2, design = 4, ships = 2 } },
      { ticker = 9002 }]

[[story]]
name = "arrived"
when = [{ fleet_at = { fleet = 20001, planet = 2 } }, { counter = { name = "favours", at_least = 2 } }]
do = [{ clear_flag = "greeted" }, { set_counter = { name = "favours", value = -1 } }]

[[story]]
name = "end"
when = [{ empire_dead = 3 }]
do = [{ win = true }]
"#;

fn story() -> Story {
    Story { mod_defs: parse(STORY, "zorg").unwrap().into(), ..Story::default() }
}

fn check(s: &mut Story, w: &mut W) -> Vec<StoryEvent> {
    s.check(true, &crate::game::story_chain(), w);
    std::mem::take(&mut w.events)
}

#[test]
fn a_story_runs_its_course() {
    let (mut s, mut w) = (story(), world());
    w.ido = 9 * DAY;
    assert_eq!(check(&mut s, &mut w), [], "day 9: nothing yet");
    w.ido = 10 * DAY;
    assert_eq!(check(&mut s, &mut w), [StoryEvent::NewMessage { id: 9001, arg: 0 }]);
    assert!(s.mod_state.flags.contains("greeted") && s.mod_state.counters["favours"] == 2);
    assert_eq!(check(&mut s, &mut w), [], "it fired once");
    // Answer 1 isn't the one the story waits for; answer 2 is.
    s.message = (9001, 1);
    s.flags[flag::MESSAGE] = true;
    assert_eq!(check(&mut s, &mut w), []);
    s.message = (9001, 2);
    s.flags[flag::MESSAGE] = true;
    let ev = check(&mut s, &mut w);
    assert_eq!(&ev[..1], [StoryEvent::SetPlanetOwner { planet: 3, owner: 1, race: 101 }]);
    assert_eq!(ev.iter().filter(|e| matches!(e, StoryEvent::NewShip { fleet: 1, design: 4 })).count(), 2);
    assert_eq!(ev.last(), Some(&StoryEvent::ScrollText { text: 9002, important: false }));
    assert_eq!(w.fleets[1], (3, 20001, 200.0, 0.0), "on planet 2");
    // The new fleet sits on planet 2 and the counter is 2: "arrived" fires in the next check.
    check(&mut s, &mut w);
    assert!(!s.mod_state.flags.contains("greeted") && s.mod_state.counters["favours"] == -1);
    w.alive[3] = false;
    assert_eq!(check(&mut s, &mut w), [StoryEvent::Won]);
    assert_eq!(s.mod_state.fired.iter().map(String::as_str).collect::<Vec<_>>(), ["zorg/arrived", "zorg/end", "zorg/hello", "zorg/yes"]);
}

#[test]
fn the_state_goes_through_the_save_state() {
    let (mut s, mut w) = (story(), world());
    w.ido = 10 * DAY;
    check(&mut s, &mut w);
    let mut save = crate::save::SaveState::from_parts(crate::save::Memory::default(), Default::default());
    save.store_story(&s);
    assert_eq!(save.mod_stories, s.mod_state);
    assert_eq!(save.story().mod_state, s.mod_state);
}

#[test]
fn without_stories_nothing_changes() {
    let (mut s, mut w) = (Story::default(), world());
    w.ido = 10 * DAY;
    let before = s.clone();
    ModScripts.run(&mut s, &mut w);
    assert_eq!(s, before);
    assert_eq!(w.events, []);
}

#[test]
fn errors_name_the_story_and_the_entry() {
    let err = |t: &str| parse(t, "m").unwrap_err();
    assert_eq!(err("[[story]]\nname = \"a\"\nwhen = [{ dya = 3 }]\ndo = [{ win = true }]"), "story a: when 1: dya: unknown trigger (known: day, planet_owner, empire_dead, flag, not_flag, counter, fleet_at, answered)");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ explode = 1 }]").split(" (").next().unwrap(), "story a: do 1: explode: unknown action");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ give_planet = { planet = 3, empire = 19 } }]"), "story a: do 1: give_planet.empire: 19 is out of range 0..=18");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ give_planet = { planet = 3, emprie = 1 } }]"), "story a: do 1: give_planet.empire: missing");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ give_planet = { planet = 3, empire = 1, x = 1 } }]"), "story a: do 1: give_planet: unknown field `x` (known: planet, empire)");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ spawn_fleet = { id = 868, owner = 1, planet = 1, design = 0 } }]"), "story a: do 1: spawn_fleet.id: 868 is out of range 10000..=32767");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ win = true }]\n[[story]]\nname = \"a\"\ndo = [{ win = true }]"), "story a: a second story of that name");
    assert_eq!(err("[[story]]\nname = \"a\"\nwhen = []"), "story a: no `do` actions");
    assert_eq!(err("[[story]]\nname = \"a b\"\ndo = [{ win = true }]"), "story 1: \"a b\" is not a name (letters, digits, '_' and '-')");
    assert_eq!(err("[[story]]\nname = \"a\"\nthen = 1\ndo = [{ win = true }]"), "story a: unknown field `then` (known: name, when, do)");
    assert_eq!(err("[stories]\n"), "unknown table `stories` (a file holds [[story]] tables)");
    assert!(err("[[story]\n").contains("TOML parse error") || !err("[[story]\n").is_empty());
}
