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
    /// `(tech, empire)` → status.
    techs: BTreeMap<(i32, i32), i32>,
    money: [f64; 19],
    /// `(a, b)` → level.
    relations: BTreeMap<(i32, i32), i32>,
    /// Per planet: `(kind, unfinished)`.
    buildings: BTreeMap<i32, Vec<(i32, i32)>>,
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
    fn money(&self, e: i32) -> f64 {
        self.money[e as usize]
    }
    fn tech_status(&self, t: i32, e: i32) -> i32 {
        self.techs.get(&(t, e)).copied().unwrap_or(1)
    }
    fn relation_level(&self, a: i32, b: i32) -> i32 {
        self.relations.get(&(a, b)).copied().unwrap_or(3)
    }
    fn building_count(&self, p: i32) -> i32 {
        self.buildings.get(&p).map_or(0, |b| b.len() as i32)
    }
    fn building(&self, p: i32, k: i32) -> (i32, i32, i32) {
        let (kind, unfinished) = self.buildings[&p][k as usize - 1];
        (kind, 0, unfinished)
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
    assert_eq!(err("[[story]]\nname = \"a\"\nwhen = [{ dya = 3 }]\ndo = [{ win = true }]").split(" (").next().unwrap(), "story a: when 1: dya: unknown trigger");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ explode = 1 }]").split(" (").next().unwrap(), "story a: do 1: explode: unknown action");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ give_planet = { planet = 3, empire = 19 } }]"), "story a: do 1: give_planet.empire: 19 is out of range 0..=18");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ give_planet = { planet = 3, emprie = 1 } }]"), "story a: do 1: give_planet.empire: missing");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ give_planet = { planet = 3, empire = 1, x = 1 } }]"), "story a: do 1: give_planet: unknown field `x` (known: planet, empire)");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ spawn_fleet = { id = 868, owner = 1, planet = 1, design = 0 } }]"), "story a: do 1: spawn_fleet.id: 868 is out of range 10000..=32767");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ win = true }]\n[[story]]\nname = \"a\"\ndo = [{ win = true }]"), "story a: a second story of that name");
    assert_eq!(err("[[story]]\nname = \"a\"\nwhen = []"), "story a: no `do` actions");
    assert_eq!(err("[[story]]\nname = \"a b\"\ndo = [{ win = true }]"), "story 1: \"a b\" is not a name (letters, digits, '_' and '-')");
    assert_eq!(err("[[story]]\nname = \"a\"\nthen = 1\ndo = [{ win = true }]"), "story a: unknown field `then` (known: name, when, do, repeat, every, times)");
    assert_eq!(err("[stories]\n"), "unknown table `stories` (a file holds [[story]] tables)");
    assert!(err("[[story]\n").contains("TOML parse error") || !err("[[story]\n").is_empty());
}

// ------------------------------------------------------------------------------------------------ part 2

/// One mod `m`'s stories over the synthetic world.
fn stories(text: &str) -> Story {
    Story { mod_defs: parse(text, "m").unwrap().into(), ..Story::default() }
}

/// Runs the interpreter alone (the event flags a test sets stay as they are); the events it caused.
fn run(s: &mut Story, w: &mut W) -> Vec<StoryEvent> {
    ModScripts.run(s, w);
    std::mem::take(&mut w.events)
}

fn fired(s: &Story, name: &str) -> bool {
    s.mod_state.fired.contains(&format!("m/{name}"))
}

#[test]
fn all_any_and_not_combine_triggers() {
    let mut s = stories(
        r#"
        [[story]]
        name = "x"
        when = [{ any = [{ flag = "a" }, { all = [{ flag = "b" }, { flag = "c" }] }] }, { not = { flag = "d" } }]
        do = [{ set_flag = "done" }]
        "#,
    );
    let mut w = world();
    let mut with = |flags: &[&str], s: &mut Story| {
        s.mod_state.flags = flags.iter().map(|f| f.to_string()).collect();
        run(s, &mut w);
        std::mem::take(&mut s.mod_state.fired).contains("m/x")
    };
    assert!(!with(&[], &mut s));
    assert!(!with(&["b"], &mut s), "all needs both");
    assert!(with(&["b", "c"], &mut s));
    assert!(with(&["a"], &mut s));
    assert!(!with(&["a", "d"], &mut s), "not");
}

#[test]
fn battle_triggers_see_the_winner_and_the_sides() {
    let text = r#"
        [[story]]
        name = "won"
        repeat = true
        when = [{ battle = { result = "won" } }]
        do = [{ add_counter = { name = "won", value = 1 } }]

        [[story]]
        name = "lost-ground"
        repeat = true
        when = [{ battle = { kind = "ground", result = "lost", planet = 2 } }]
        do = [{ add_counter = { name = "lost", value = 1 } }]
    "#;
    let count = |s: &Story, n: &str| s.mod_state.counters.get(n).copied().unwrap_or(0);
    let part = |owner| crate::story::Participant { owner, slot_id: 1, fleet_id: 0 };
    let mut w = world();
    // A space battle the human (1) won against empire 3.
    let mut s = stories(text);
    s.flags[flag::SPACE_END] = true;
    s.battle = vec![part(1), part(3)];
    s.battle_winner = 1;
    run(&mut s, &mut w);
    assert_eq!((count(&s, "won"), count(&s, "lost")), (1, 0));
    // Empire 3 won it: not the human's win.
    let mut s = stories(text);
    s.flags[flag::SPACE_END] = true;
    s.battle = vec![part(1), part(3)];
    s.battle_winner = 3;
    run(&mut s, &mut w);
    assert_eq!(count(&s, "won"), 0);
    // Empire 3 landed on the human's planet 2 and won: the human lost a ground battle there.
    let mut s = stories(text);
    s.flags[flag::GROUND_END] = true;
    s.ground = crate::story::GroundBattle { fleet: 1, fleet_id: 0, fleet_owner: 3, planet: 2, old_owner: 3, attacker_won: true, defender_won: false };
    s.set_adat_i32(0xf4, 1);
    run(&mut s, &mut w);
    assert_eq!((count(&s, "won"), count(&s, "lost")), (0, 1));
    // The human landed and won.
    let mut s = stories(text);
    s.flags[flag::GROUND_END] = true;
    s.ground = crate::story::GroundBattle { fleet: 1, fleet_id: 0, fleet_owner: 1, planet: 2, old_owner: 1, attacker_won: true, defender_won: false };
    s.set_adat_i32(0xf4, 3);
    run(&mut s, &mut w);
    assert_eq!((count(&s, "won"), count(&s, "lost")), (1, 0));
}

#[test]
fn tech_money_relation_building_and_spy_triggers() {
    let text = r#"
        [[story]]
        name = "tech"
        when = [{ tech = { tech = 40 } }]
        do = [{ set_flag = "t" }]

        [[story]]
        name = "rich"
        when = [{ money = { empire = 3, at_least = 5000 } }]
        do = [{ set_flag = "r" }]

        [[story]]
        name = "war"
        when = [{ relation = { a = "human", b = 3, at_most = 1 } }]
        do = [{ set_flag = "w" }]

        [[story]]
        name = "built"
        when = [{ building = { kind = 7 } }]
        do = [{ set_flag = "b" }]

        [[story]]
        name = "spied"
        when = [{ spy_report = { target = 3, success = true } }]
        do = [{ set_flag = "s" }]
    "#;
    let (mut s, mut w) = (stories(text), world());
    w.buildings.insert(1, vec![(7, 1)]);
    w.buildings.insert(2, vec![(7, 0)]);
    s.kem = crate::story::KemNews { target: 3, success: false, ..Default::default() };
    s.flags[flag::SPY] = true;
    s.spy_kind = 3;
    run(&mut s, &mut w);
    assert!(s.mod_state.fired.is_empty(), "nothing holds yet (planet 2's finished building is empire 3's)");
    w.techs.insert((40, 1), 5);
    w.money[3] = 5000.0;
    w.relations.insert((1, 3), 1);
    w.buildings.insert(1, vec![(3, 0), (7, 0)]);
    s.kem.success = true;
    run(&mut s, &mut w);
    for n in ["tech", "rich", "war", "built", "spied"] {
        assert!(fired(&s, n), "{n}");
    }
}

#[test]
fn timers_run_out_and_stop() {
    let text = r#"
        [[story]]
        name = "start"
        do = [{ start_timer = { name = "siege", days = 5 } }, { start_timer = { name = "other", days = 1 } }]

        [[story]]
        name = "stop"
        when = [{ timer = "other" }]
        do = [{ stop_timer = "siege" }, { stop_timer = "other" }, { start_timer = { name = "late", days = 2 } }]

        [[story]]
        name = "late"
        when = [{ timer = "late" }]
        do = [{ win = true }]
    "#;
    let (mut s, mut w) = (stories(text), world());
    w.ido = 3 * DAY;
    run(&mut s, &mut w);
    assert_eq!(s.mod_state.timers, BTreeMap::from([("siege".into(), 8 * DAY), ("other".into(), 4 * DAY)]));
    w.ido = 4 * DAY;
    run(&mut s, &mut w);
    assert_eq!(s.mod_state.timers, BTreeMap::from([("late".into(), 6 * DAY)]));
    w.ido = 6 * DAY - 1;
    assert_eq!(run(&mut s, &mut w), []);
    w.ido = 6 * DAY;
    assert_eq!(run(&mut s, &mut w), [StoryEvent::Won]);
}

#[test]
fn repeating_stories_wait_their_days_and_stop_after_times() {
    let mut s = stories(
        r#"
        [[story]]
        name = "tax"
        repeat = true
        every = 2
        times = 3
        do = [{ money = { empire = "human", amount = 100 } }]
        "#,
    );
    let mut w = world();
    let mut paid = Vec::new();
    for day in 0..10 {
        w.ido = day * DAY;
        if !run(&mut s, &mut w).is_empty() {
            paid.push(day);
        }
    }
    assert_eq!(paid, [0, 2, 4]);
    assert_eq!(s.mod_state.repeats["m/tax"], (4 * DAY, 3));
    assert!(s.mod_state.fired.is_empty());
}

#[test]
fn the_new_actions_send_their_events() {
    let mut s = stories(
        r#"
        [[story]]
        name = "x"
        do = [{ give_tech = { tech = 12 } }, { remove_tech = { tech = 13, empire = 3 } },
              { change_relation = { a = 3, b = "human", by = -300 } },
              { diplomacy = { a = "human", b = 3, state = "war" } }, { diplomacy = { a = 1, b = 3, state = "neutral" } },
              { diplomacy = { a = 1, b = 3, state = "non_aggression" } }, { diplomacy = { a = 1, b = 3, state = "alliance" } },
              { diplomacy = { a = 1, b = 3, state = "trade" } },
              { add_building = { planet = 1, kind = 9 } }, { add_building = { planet = 9, kind = 9 } },
              { spawn_fleet = { id = 20002, owner = 1, planet = 1, design = 0 } },
              { ground_units = { planet = 2, design = 5, count = 4 } }, { ground_units = { fleet = 20002, design = 6 } },
              { ground_units = { fleet = 20003, design = 6 } }]
        "#,
    );
    let mut w = world();
    let ev: Vec<StoryEvent> = run(&mut s, &mut w).into_iter().filter(|e| !matches!(e, StoryEvent::NewShip { .. })).collect();
    assert_eq!(
        ev,
        [
            StoryEvent::SetTechState { tech: 12, state: 5, empire: 1 },
            StoryEvent::SetTechState { tech: 13, state: 3, empire: 3 },
            StoryEvent::RelationChange { a: 3, b: 1, delta: -300 },
            StoryEvent::DeclareWar { a: 1, b: 3 },
            StoryEvent::MakeNeutral { a: 1, b: 3 },
            StoryEvent::MakeNonAggression { a: 1, b: 3 },
            StoryEvent::MakeAllies { a: 1, b: 3 },
            StoryEvent::TradeAgreement { a: 1, b: 3 },
            StoryEvent::PlaceStoryBuilding { planet: 1, kind: 9, level: 0 },
            StoryEvent::NewTanks { fleet: -2, design: 5, count: 4 },
            StoryEvent::NewTanks { fleet: 1, design: 6, count: 1 },
        ],
        "planet 9 and fleet 20003 don't exist: nothing"
    );
    assert_eq!(s.specials.entries, [(1, 12)], "a given tech is queued for the stories, as the built-in stories do");
}

#[test]
fn chains_fire_another_story_at_once() {
    let mut s = stories(
        r#"
        [[story]]
        name = "a"
        do = [{ set_flag = "a" }, { chain = "b" }, { chain = "c" }]

        [[story]]
        name = "b"
        when = [{ day = 1000 }]
        do = [{ add_counter = { name = "n", value = 1 } }, { chain = "c" }]

        [[story]]
        name = "c"
        repeat = true
        times = 1
        when = [{ day = 1000 }]
        do = [{ add_counter = { name = "n", value = 10 } }]
        "#,
    );
    let mut w = world();
    run(&mut s, &mut w);
    assert!(fired(&s, "a") && fired(&s, "b"));
    assert_eq!(s.mod_state.counters["n"], 11, "b ran once, c once (its times = 1 stops the second chain)");
    w.ido = 1000 * DAY;
    run(&mut s, &mut w);
    assert_eq!(s.mod_state.counters["n"], 11, "fired stories stay fired");
}

#[test]
fn part_two_errors() {
    let err = |t: &str| parse(t, "m").unwrap_err();
    let first = |t: &str| err(t).split(" (").next().unwrap().to_owned();
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ chain = \"b\" }]"), "story a: chain: no story \"b\" in this file");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ chain = \"b\" }]\n[[story]]\nname = \"b\"\ndo = [{ chain = \"a\" }]"), "story a: its chains lead back to it");
    assert_eq!(err("[[story]]\nname = \"a\"\nevery = 2\ndo = [{ win = true }]"), "story a: every and times need repeat = true");
    assert_eq!(err("[[story]]\nname = \"a\"\nwhen = [{ money = { empire = 1 } }]\ndo = [{ win = true }]"), "story a: when 1: money: give at_least, at_most or both");
    assert_eq!(err("[[story]]\nname = \"a\"\nwhen = [{ battle = { result = \"draw\" } }]\ndo = [{ win = true }]"), "story a: when 1: battle.result: expected one of \"won\", \"lost\"");
    assert_eq!(first("[[story]]\nname = \"a\"\nwhen = [{ not = { dya = 1 } }]\ndo = [{ win = true }]"), "story a: when 1: not: dya: unknown trigger");
    assert_eq!(first("[[story]]\nname = \"a\"\nwhen = [{ any = [{ day = 1 }, { dya = 1 }] }]\ndo = [{ win = true }]"), "story a: when 1: any 2: dya: unknown trigger");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ ground_units = { design = 1 } }]"), "story a: do 1: ground_units: give planet or fleet (one of them)");
    assert_eq!(err("[[story]]\nname = \"a\"\ndo = [{ give_tech = { tech = 193 } }]"), "story a: do 1: give_tech.tech: 193 is out of range 0..=192");
    assert_eq!(
        err("[[story]]\nname = \"a\"\ndo = [{ diplomacy = { a = 1, b = 2, state = \"peace\" } }]"),
        "story a: do 1: diplomacy.state: expected one of \"war\", \"neutral\", \"non_aggression\", \"alliance\", \"trade\""
    );
}

#[test]
fn the_user_guide_example_parses() {
    let guide = include_str!("../../../../../docs/USER_GUIDE.md").replace("\r\n", "\n");
    let block = guide.split("`mods/zorg-ultimatum/stories.toml`:\n\n```toml\n").nth(1).expect("the example").split("```").next().unwrap();
    assert_eq!(parse(block, "zorg-ultimatum").unwrap().len(), 6);
}
