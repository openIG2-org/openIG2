//! Replays the story engine recorded from the running GOG `ig2.exe` (tools/trace/run_story.py,
//! `re/rules/campaign.md`): every record of `story-*.jsonl`, plain and fuzzed, on the skirmish scenarios.
//! - `tick`: one `FRISSIT_Story` (exe 0x5c1ff0) the game ran (counters, the daily check, the scripts);
//! - `check`: one outermost `Story_Check` (0x5c5610) a game hook ran (its flags and payload are in `pre`);
//! - `staged`: a call the tracer made: `StartStory` + `Story_Check(1)`, `Story_Special`, a fleet arrival,
//!   `Story_SpaceBattleFinished`, raw event flags + `Story_Check(1)`, `st_AddGameInfo`, `st_RemoveGameInfo`, `WIN`.
//!
//! The port starts from the record's `pre` story state and world inputs; afterwards the whole story state, the
//! story stream (5) and the calls of `MEGNYERES` (the game won) and `MEGHALAS` (an empire dies) must equal
//! `post`. Traces hold game state, so they stay in `_local/` (never committed; `IG2_TRACE_DIR` points
//! elsewhere). The test skips without traces.

use std::path::PathBuf;

use ig2_sim::rng::Streams;
use ig2_sim::story::{self, Participant, Queue, ScenarioStories, Story, StoryEvent, StoryWorld, FLAGS};
use serde_json::Value;

/// `_local/traces` of the checkout, or `IG2_TRACE_DIR` (a git worktree without the gitignored traces).
fn traces(prefix: &str) -> Vec<PathBuf> {
    let dir = std::env::var_os("IG2_TRACE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/traces"));
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with(prefix) && f.ends_with(".jsonl")));
    v.sort();
    v
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}
fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn ints(v: &Value) -> Vec<i32> {
    v.as_array().unwrap().iter().map(int).collect()
}
fn pairs(v: &Value) -> Vec<(i32, i32)> {
    v.as_array().unwrap().iter().map(|p| (int(&p[0]), int(&p[1]))).collect()
}
fn queue(v: &Value) -> Queue {
    Queue { entries: pairs(v) }
}

/// The world inputs of one record.
struct W {
    ido: i32,
    speed: i32,
    en: i32,
    difficulty: i32,
    mp: bool,
    alive: Vec<u8>,
    planets_of: Vec<i32>,
    money: f64,
    relation: Vec<i32>,
    tech: (i32, Vec<u8>),
    rng: Streams,
    planets: Vec<(i32, i32, i32)>,
    fleets: Vec<(i32, i32)>,
    events: Vec<StoryEvent>,
}

impl W {
    fn new(w: &Value) -> Self {
        let mut rng = Streams::default();
        rng.set_state(story::STREAM, w["seed5"].as_u64().unwrap() as u32);
        let tech = &w["tech"];
        Self {
            ido: int(&w["ido"]),
            speed: int(&w["speed"]),
            en: int(&w["en"]),
            difficulty: int(&w["neh"]),
            mp: int(&w["mp"]) != 0,
            alive: unhex(w["el"].as_str().unwrap()),
            planets_of: ints(&w["hanyb"]),
            money: w["money"].as_f64().unwrap(),
            relation: ints(&w["visz"]),
            tech: (int(&tech[0]), unhex(tech[1].as_str().unwrap())),
            rng,
            planets: w["planets"].as_array().unwrap().iter().map(|p| (int(&p[0]), int(&p[1]), int(&p[2]))).collect(),
            fleets: pairs(&w["fleets"]),
            events: Vec::new(),
        }
    }
}

impl StoryWorld for W {
    fn ido(&self) -> i32 {
        self.ido
    }
    fn speed_mult(&self) -> i32 {
        self.speed
    }
    fn human(&self) -> i32 {
        self.en
    }
    fn difficulty(&self) -> i32 {
        self.difficulty
    }
    fn multiplayer(&self) -> bool {
        self.mp
    }
    fn rng(&mut self) -> &mut Streams {
        &mut self.rng
    }
    fn planet_count(&self) -> i32 {
        self.planets.len() as i32 - 1
    }
    fn planet_owner(&self, p: i32) -> i32 {
        self.planets[p as usize].0
    }
    fn planet_value(&self, p: i32) -> i32 {
        self.planets[p as usize].1
    }
    fn planet_visibility(&self, p: i32) -> i32 {
        self.planets[p as usize].2
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
        self.alive[e as usize] != 0
    }
    fn planets_of(&self, e: i32) -> i32 {
        self.planets_of[e as usize]
    }
    fn money(&self, e: i32) -> f64 {
        assert_eq!(e, self.en);
        self.money
    }
    fn tech_status(&self, tech: i32, e: i32) -> i32 {
        assert_eq!(tech, self.tech.0, "the trace holds one tech row (variable 3732's)");
        self.tech.1[e as usize] as i32
    }
    fn relation_level(&self, a: i32, b: i32) -> i32 {
        assert_eq!(a, self.en);
        self.relation[b as usize]
    }
    fn event(&mut self, ev: StoryEvent) {
        self.events.push(ev);
    }
}

fn load(s: &Value) -> Story {
    let mut st = Story::default();
    for (k, q) in s["buckets"].as_object().unwrap() {
        st.buckets[k.parse::<usize>().unwrap()] = queue(q);
    }
    st.history = queue(&s["hist"]);
    st.starting = queue(&s["start"]);
    st.finished = queue(&s["vegzes"]);
    st.specials = queue(&s["talal"]);
    st.read = queue(&s["olvas"]);
    st.planets = pairs(&s["b"]).into_iter().map(|(id, p)| (id, p as u8)).collect();
    st.counters.set_live(&pairs(&s["sz"]));
    st.logic.set_live(&pairs(&s["l"]));
    st.vars.set_live(&pairs(&s["v"]));
    st.error = int(&s["err"]);
    let flags = unhex(s["flags"].as_str().unwrap());
    for (f, &b) in st.flags.iter_mut().zip(&flags[..FLAGS]) {
        *f = b != 0;
    }
    st.last_checked = int(&s["last"]);
    st.counter_done = int(&s["szv"]);
    st.old_ido = int(&s["oldido"]);
    st.delayed = int(&s["delayed"]) != 0;
    st.skip_first = int(&s["skip"]);
    st.restarting = int(&s["restart"]);
    st.adat.copy_from_slice(&unhex(s["adat"].as_str().unwrap()));
    st.undo = s["ozon"].as_array().unwrap().iter().map(|e| (int(&e[0]), int(&e[1]), int(&e[2]))).collect();
    let sig = ints(&s["signal"]);
    st.signal = (sig[0], sig[1]);
    st.last_addship = int(&s["addship"]);
    st.special.copy_from_slice(&ints(&s["special"]));
    let r = ints(&s["radar"]);
    st.radar = (r[0], r[1]);
    let d = ints(&s["dipl"]);
    st.diplomacy = (d[0], d[1], d[2]);
    let a = ints(&s["arr"]);
    st.arrival = story::Arrival { fleet_id: a[0], fleet: a[1], target: a[2], target_type: a[3], target_fleet_id: a[4] };
    let g = ints(&s["gr"]);
    st.ground = story::GroundBattle {
        fleet: g[0],
        fleet_id: g[1],
        fleet_owner: g[2],
        planet: g[3],
        old_owner: g[4],
        attacker_won: g[5] != 0,
        defender_won: g[6] != 0,
    };
    st.you_were_in_battle = int(&s["youwas"]) != 0;
    st.spy_kind = int(&s["kem"]);
    st
}

/// Compare the port's state with a recorded snapshot; returns the differing fields.
fn compare(st: &Story, s: &Value) -> Vec<String> {
    let want = load(s);
    let mut bad = Vec::new();
    macro_rules! field {
        ($($f:ident).+) => {
            if st.$($f).+ != want.$($f).+ {
                bad.push(format!("{}: port {:?} exe {:?}", stringify!($($f).+), st.$($f).+, want.$($f).+));
            }
        };
    }
    for k in 0..story::BUCKETS {
        if st.buckets[k] != want.buckets[k] {
            bad.push(format!("bucket {k}: port {:?} exe {:?}", st.buckets[k].entries, want.buckets[k].entries));
        }
    }
    field!(history);
    field!(starting);
    field!(finished);
    field!(specials);
    field!(read);
    field!(planets);
    if st.counters.live() != want.counters.live() {
        bad.push(format!("counters: port {:?} exe {:?}", st.counters.live(), want.counters.live()));
    }
    if st.logic.live() != want.logic.live() {
        bad.push(format!("logic: port {:?} exe {:?}", st.logic.live(), want.logic.live()));
    }
    if st.vars.live() != want.vars.live() {
        bad.push(format!("vars: port {:?} exe {:?}", st.vars.live(), want.vars.live()));
    }
    field!(error);
    field!(flags);
    field!(last_checked);
    field!(counter_done);
    field!(old_ido);
    field!(delayed);
    field!(skip_first);
    field!(restarting);
    if st.adat != want.adat {
        bad.push("adat".into());
    }
    field!(undo);
    field!(signal);
    field!(last_addship);
    field!(special);
    field!(radar);
    field!(diplomacy);
    field!(arrival);
    field!(ground);
    field!(you_were_in_battle);
    if st.battle.len() as i32 != ints(&s["spw"])[1] {
        bad.push(format!("battle size: port {} exe {}", st.battle.len(), ints(&s["spw"])[1]));
    }
    field!(spy_kind);
    bad
}

#[derive(Default)]
struct Stats {
    records: usize,
    ticks: usize,
    checks: usize,
    staged: std::collections::BTreeMap<String, usize>,
    hooks: std::collections::BTreeMap<String, usize>,
    counters_fired: usize,
    daily: usize,
    started: std::collections::BTreeMap<i32, usize>,
    wins: std::collections::BTreeMap<i32, usize>,
    won: usize,
    died: usize,
    infos: usize,
    draws: usize,
}

#[test]
fn story_engine_matches_exe() {
    let files = traces("story-");
    if files.is_empty() {
        eprintln!("skipped: no story-*.jsonl traces");
        return;
    }
    let mut stats = Stats::default();
    let mut failures = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let mut battle: Vec<Participant> = Vec::new();
        for line in text.lines() {
            let r: Value = serde_json::from_str(line).unwrap();
            let kind = r["k"].as_str().unwrap();
            if !matches!(kind, "tick" | "check" | "staged") {
                continue;
            }
            let (pre, post) = (&r["pre"], &r["post"]);
            let mut st = load(&pre["s"]);
            st.battle.clone_from(&battle);
            let mut w = W::new(&pre["w"]);
            let seed0 = w.rng.state(story::STREAM);
            let sc = ScenarioStories;
            stats.records += 1;
            match kind {
                "tick" => {
                    stats.ticks += 1;
                    st.tick(&sc, &mut w);
                }
                "check" => {
                    stats.checks += 1;
                    st.check(int(&r["arg"]) != 0, &sc, &mut w);
                }
                _ => {
                    let call = r["call"].as_str().unwrap();
                    *stats.staged.entry(call.into()).or_default() += 1;
                    let a = &r["args"];
                    match call {
                        "start" => {
                            st.start_story(int(&a[0]), false);
                            st.check(true, &sc, &mut w);
                        }
                        "special" => {
                            let d = ints(a);
                            st.story_special(d.try_into().unwrap(), &sc, &mut w);
                        }
                        "arrive" => {
                            let (f, order, target) = (int(&a[0]), int(&a[1]), int(&a[2]));
                            st.fleet_arrived_save(f, order, target, &w);
                            st.fleet_arrived_check(&sc, &mut w);
                        }
                        "spwfinished" => {
                            battle = a
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|p| Participant { owner: int(&p[0]), slot_id: int(&p[1]), fleet_id: int(&p[1]) })
                                .collect();
                            st.space_battle_finished(&battle, true, &sc, &mut w);
                        }
                        "flag" => {
                            let f = int(&a[0]) as usize;
                            st.flags[f] = true;
                            if f == story::flag::COUNTER {
                                st.counter_done = int(&a[1]);
                            }
                            st.check(true, &sc, &mut w);
                        }
                        "gameinfo" => st.add_game_info(int(&a[0]), 0, int(&a[1]), &mut w),
                        "rmgameinfo" => st.remove_game_info(int(&a[0]), &mut w),
                        "win" => st.win(int(&a[0]), int(&a[1]) != 0, &mut w),
                        other => panic!("unknown staged call {other}"),
                    }
                }
            }
            // Coverage.
            let pre_s = load(&pre["s"]);
            for k in 0..story::BUCKETS {
                for &(id, _) in &st.buckets[k].entries {
                    if !pre_s.buckets[k].contains(id) {
                        *stats.started.entry(id).or_default() += 1;
                    }
                }
            }
            for k in 0..story::BUCKETS {
                for &(id, _) in &pre_s.buckets[k].entries {
                    if !st.buckets[k].contains(id) {
                        *stats.wins.entry(id).or_default() += 1;
                    }
                }
            }
            if kind == "check" {
                *stats.hooks.entry(r["from"].as_str().unwrap().to_string()).or_default() += 1;
            }
            stats.counters_fired += st.counters.live().iter().zip(pre_s.counters.live()).filter(|(a, b)| a.1 == -1 && b.1 >= 0).count();
            if st.old_ido != pre_s.old_ido {
                stats.daily += 1;
            }
            let mut s5 = seed0;
            for _ in 0..10_000 {
                if s5 == w.rng.state(story::STREAM) {
                    break;
                }
                s5 = ig2_sim::rng::step(s5);
                stats.draws += 1;
            }
            // MEGNYERES / MEGHALAS calls (the tracer's counters: [won, last arg, died, last arg]).
            let (e0, e1) = (ints(&pre["ends"]), ints(&post["ends"]));
            let won = w.events.iter().filter(|e| **e == StoryEvent::Won).count() as i32;
            let died: Vec<i32> = w.events.iter().filter_map(|e| if let StoryEvent::EmpireDies(x) = e { Some(*x) } else { None }).collect();
            stats.won += won as usize;
            stats.died += died.len();
            stats.infos += w.events.iter().filter(|e| matches!(e, StoryEvent::GameInfo { .. })).count();
            let mut bad = compare(&st, &post["s"]);
            if w.rng.state(story::STREAM) != post["w"]["seed5"].as_u64().unwrap() as u32 {
                bad.push(format!("stream 5: port {:#x} exe {:#x}", w.rng.state(story::STREAM), post["w"]["seed5"].as_u64().unwrap()));
            }
            if won != e1[0] - e0[0] {
                bad.push(format!("MEGNYERES calls: port {won} exe {}", e1[0] - e0[0]));
            }
            if died.len() as i32 != e1[2] - e0[2] || died.last().is_some_and(|&x| x != e1[3]) {
                bad.push(format!("MEGHALAS calls: port {died:?} exe {} (last {})", e1[2] - e0[2], e1[3]));
            }
            if !bad.is_empty() {
                failures.push(format!("{} seq {} {kind} {}: {}", path.display(), r["seq"], r["call"], bad.join("; ")));
            }
        }
    }
    eprintln!(
        "story replay: {} records ({} ticks, {} hook checks {:?}, staged {:?}), {} counters fired, {} daily checks, \
         stories started {:?}, ended {:?}, {} MEGNYERES, {} MEGHALAS, {} info lines, {} stream-5 draws",
        stats.records, stats.ticks, stats.checks, stats.hooks, stats.staged, stats.counters_fired, stats.daily, stats.started, stats.wins,
        stats.won, stats.died, stats.infos, stats.draws
    );
    assert!(failures.is_empty(), "{} mismatches, first: {}", failures.len(), failures[..failures.len().min(8)].join("\n"));
}
