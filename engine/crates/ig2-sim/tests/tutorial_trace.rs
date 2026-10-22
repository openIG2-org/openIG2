//! The tutorial interpreter against ig2.exe (`re/rules/tutorial.md` §11): `tools/trace/run_tutorial.py`
//! records every `TutorialFrissit` tick of a tutorial run (its inputs and the interpreter's state after it), the
//! commands it ran with their calls to other systems, and the events fired between ticks. The port is fed the
//! same inputs and must end every tick in the same state: script pointer, waits, event, gate flags, the text
//! window's position, typing counter, queue and grid, the `.mou` index.
//!
//! Traces `tutorial*.jsonl` in `_local/traces` (or `IG2_TRACE_DIR`); the script comes from the install
//! (`IG2_INSTALL_DIR`; the language whose `main.tut` has the traced length). Skips without either.

use std::collections::VecDeque;
use std::path::PathBuf;

use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::tutorial::{Effect, Tutorial, TutorialHost, COLS, ROWS};
use serde_json::Value;

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
    v.as_i64().unwrap_or_else(|| panic!("int: {v}")) as i32
}
fn f32v(v: &Value) -> f32 {
    v.as_f64().unwrap() as f32
}

/// The scripts of every language, to pick the one the exe ran by its length.
fn scripts() -> Option<Vec<Vec<u8>>> {
    let base = Install::from_env(Lang::En)?.expect("valid install").root().to_owned();
    Some(Lang::ALL.iter().filter_map(|&l| Install::open(&base, l).ok()?.read("dataraw/tutorial/main/script/main.tut").ok()).collect())
}

/// One tick's inputs from the trace.
#[derive(Default)]
struct Host {
    kep: i32,
    cnt: i32,
    tr: bool,
    selfp: bool,
    own: i32,
    expl: i32,
    tanks: i32,
    /// The hooked calls' results, in order (`speech?`, `mp2`, `mou`).
    speech: VecDeque<bool>,
    mp2: VecDeque<i32>,
    mou: VecDeque<i32>,
    money: Vec<f64>,
    effects: Vec<Effect>,
}

impl TutorialHost for Host {
    fn screen(&self) -> i32 {
        self.kep
    }
    fn counter(&self) -> i32 {
        self.cnt
    }
    fn transition(&self) -> bool {
        self.tr
    }
    fn self_playing(&self) -> bool {
        self.selfp
    }
    fn speech_playing(&mut self) -> bool {
        self.speech.pop_front().expect("ExistAnyTutorialMp2 call in the trace")
    }
    fn start_speech(&mut self, _name: &[u8]) -> i32 {
        self.mp2.pop_front().expect("TutorialStartMp2_2 call in the trace")
    }
    fn mouse_samples(&mut self, _name: &[u8]) -> i32 {
        self.mou.pop_front().expect("TutorialPlayMouse call in the trace")
    }
    fn explored_planets(&self) -> i32 {
        self.expl
    }
    fn owned_planets(&self) -> i32 {
        self.own
    }
    fn my_tanks(&self) -> i32 {
        self.tanks
    }
    fn effect(&mut self, e: Effect) {
        self.effects.push(e);
    }
}

/// The port's state in the trace's terms (`tutorial.js` `state()`), for the fields the port models.
fn compare(t: &Tutorial, post: &Value, n: i64) {
    let w = &t.window;
    let g = &t.gates;
    let checks: Vec<(&str, bool)> = vec![
        ("act", t.active as i32 == int(&post["act"])),
        ("pau", t.paused as i32 == int(&post["pau"])),
        ("ptr", t.ptr == int(&post["ptr"])),
        ("tt", t.target_time == int(&post["tt"])),
        ("tc", t.target_counter == int(&post["tc"])),
        ("sc", t.start_counter == int(&post["sc"])),
        ("rs", t.restarting as i32 == int(&post["rs"])),
        ("ch", t.chapter == int(&post["ch"])),
        ("bl", t.blink == int(&post["bl"])),
        ("bd", t.blink_up as i32 == int(&post["bd"])),
        ("x", w.x.to_bits() == f32v(&post["x"]).to_bits()),
        ("y", w.y.to_bits() == f32v(&post["y"]).to_bits()),
        ("txy", (w.target.0, w.target.1) == (int(&post["txy"][0]), int(&post["txy"][1]))),
        ("add", w.step.0.to_bits() == f32v(&post["add"][0]).to_bits() && w.step.1.to_bits() == f32v(&post["add"][1]).to_bits()),
        ("ql", w.queue_len == int(&post["ql"])),
        ("sp", w.column as i32 == int(&post["sp"])),
        ("col", w.colour == int(&post["col"])),
        ("vil", w.blink as i32 == int(&post["vil"])),
        ("ty", w.typing.to_bits() == f32v(&post["ty"]).to_bits()),
        ("tm", w.typing_max.to_bits() == f32v(&post["tm"]).to_bits()),
        ("ts", w.typing_step.to_bits() == f32v(&post["ts"]).to_bits()),
        ("shown", g.text_shown as i32 == int(&post["shown"])),
        ("notext", g.text_window as i32 == int(&post["notext"])),
        ("md", w.move_div == int(&post["md"])),
        ("cd", t.countdown == int(&post["cd"])),
        ("ev", t.event == int(&post["ev"])),
        ("w", {
            let x = &post["w"];
            t.wait_mouse as i32 == int(&x[0]) && t.wait_text as i32 == int(&x[1]) && t.wait_transition as i32 == int(&x[2])
                && t.wait_screen.is_some() as i32 == int(&x[3]) && t.wait_screen.is_none_or(|s| s == int(&x[4]))
                && t.wait_battle as i32 == int(&x[5])
        }),
        ("spb", t.speech_blocked as i32 == int(&post["spb"])),
        ("mo", t.mouse == int(&post["mo"])),
        ("mm", t.mouse_max == int(&post["mm"])),
        ("uc", t.ui_counter == int(&post["uc"])),
        ("oc", t.old_counter == int(&post["oc"])),
        ("mp2", t.speech_len == int(&post["mp2"])),
        ("tl", t.text_len == int(&post["tl"])),
        ("scr", g.screens.iter().map(|&b| b as u8).collect::<Vec<_>>() == unhex(post["scr"].as_str().unwrap())),
        ("bld", g.buildings.iter().map(|&b| b as u8).collect::<Vec<_>>() == unhex(post["bld"].as_str().unwrap())),
        ("g", {
            let x: Vec<i32> = post["g"].as_array().unwrap().iter().map(int).collect();
            x == [g.split, g.production, g.starmap, g.colony, g.sell_ships, g.exit_button, g.demolish, g.new_spies, t.overrun].map(|b| b as i32)
        }),
        ("mp", g.main_planet == post["mp"].as_str().unwrap().as_bytes()),
    ];
    let bad: Vec<&str> = checks.iter().filter(|c| !c.1).map(|c| c.0).collect();
    assert!(bad.is_empty(), "tick {n}: {bad:?} differ\nexe: {post}\nport: active {} paused {} ptr {} ev {} waits m{} t{} a{} s{:?} b{} sp{} x {} y {} add {:?} ql {} col {} ty {} ts {} md {} cd {} mo {}/{} uc {} oc {} bl {}",
        t.active, t.paused, t.ptr, t.event, t.wait_mouse, t.wait_text, t.wait_transition, t.wait_screen, t.wait_battle,
        t.speech_blocked, w.x, w.y, w.step, w.queue_len, w.column, w.typing, w.typing_step, w.move_div, t.countdown,
        t.mouse, t.mouse_max, t.ui_counter, t.old_counter, t.blink);
}

#[test]
fn tutorial_matches_the_exe() {
    let files = traces("tutorial");
    if files.is_empty() {
        eprintln!("no tutorial traces; skipping");
        return;
    }
    let Some(scripts) = scripts() else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return;
    };
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let recs: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        let mut t = Tutorial::default();
        let (mut ticks, mut cmds, mut events, mut forced, mut chapters) = (0, 0, 0, 0, 0);
        let mut chapter = 0;
        let mut fresh = false;
        let mut pending: Vec<i32> = Vec::new();
        // The counts `TutorialInitEvents` / `TutorialCheckEvents` read; older traces have them only on ticks that
        // start paused, so a tick without them takes the next tick's (they don't change within a tick).
        let mut counts = vec![None; recs.len()];
        let mut next = None;
        for (i, r) in recs.iter().enumerate().rev() {
            if let Some(own) = r.get("own") {
                next = Some((int(own), int(&r["expl"]), int(&r["tanks"])));
            }
            counts[i] = next;
        }
        for (i, r) in recs.iter().enumerate() {
            match r["k"].as_str().unwrap() {
                "start" => {
                    chapter = int(&r["chapter"]);
                    fresh = true;
                    chapters += 1;
                }
                "ev" => {
                    pending.push(int(&r["id"]));
                    events += 1;
                    forced += r["forced"].as_bool().unwrap_or(false) as i32;
                }
                "battle" => t.battle_started(),
                "tick" => {
                    let n = r["n"].as_i64().unwrap();
                    let post = &r["post"];
                    let pre = &r["pre"];
                    let mut h = Host {
                        kep: int(&r["kep"]), cnt: int(&r["cnt"]), tr: int(&r["tr"]) != 0, selfp: int(&r["self"]) != 0,
                        own: counts[i].map_or(0, |c| c.0), expl: counts[i].map_or(0, |c| c.1), tanks: counts[i].map_or(0, |c| c.2),
                        ..Host::default()
                    };
                    for c in r["cmds"].as_array().unwrap() {
                        for fx in c["fx"].as_array().unwrap() {
                            match fx[0].as_str().unwrap() {
                                "speech?" => h.speech.push_back(int(&fx[1]) != 0),
                                "mp2" => h.mp2.push_back(int(&fx[1])),
                                "mou" => h.mou.push_back(int(&fx[2])),
                                "money" => h.money.push(fx[3].as_f64().unwrap()),
                                _ => {}
                            }
                        }
                    }
                    cmds += r["cmds"].as_array().unwrap().len();
                    if fresh {
                        // The chapter's `TutorialStart` (from `MainScr_Kilep`) ran before this tick: start the port
                        // the same way, at the traced start counter, with the language's script of the traced
                        // length. `TutorialCounter` (set by `InitNewGame`) and `aktct2` (never reset) are taken
                        // from the trace.
                        let len = int(&post["len"]) as usize;
                        let script = scripts.iter().find(|s| s.len() == len).unwrap_or_else(|| panic!("no main.tut of {len} bytes")).clone();
                        let mut sh = Host { cnt: int(&post["sc"]), ..Host::default() };
                        t.start(script, chapter, &mut sh);
                        t.ui_counter = int(&pre["uc"]);
                        t.window.move_div = (int(&post["md"]) + 5) % 6;
                        fresh = false;
                    }
                    // `Mouse_Tutorial_Kezel`, the events fired by the screens' updates, `TutorialFrissit`.
                    t.mouse_tick(h.kep);
                    assert_eq!((t.mouse, t.old_counter), (int(&pre["mo"]), int(&pre["oc"])), "tick {n}: replay step");
                    // The screen gates' other writers (spec §5) ran since the last tick.
                    if let Some(scr) = pre.get("scr") {
                        for (g, b) in t.gates.screens.iter_mut().zip(unhex(scr.as_str().unwrap())) {
                            *g = b != 0;
                        }
                    }
                    for id in pending.drain(..) {
                        t.check_event(id);
                    }
                    t.frissit(&mut h);
                    compare(&t, post, n);
                    if let Some(q) = r.get("q") {
                        let q = unhex(q.as_str().unwrap());
                        assert_eq!(t.window.queue[..q.len()], q[..], "tick {n}: queue");
                    }
                    if let Some(g) = r.get("tomb") {
                        let g = unhex(g.as_str().unwrap());
                        let mine: Vec<u8> = (0..ROWS).flat_map(|row| (0..COLS).flat_map(move |c| [row, c])).collect::<Vec<_>>()
                            .chunks(2).flat_map(|rc| { let (a, b) = t.window.grid[rc[0]][rc[1]]; [a, b] }).collect();
                        assert_eq!(mine, g, "tick {n}: grid");
                    }
                    let money: Vec<f64> = h.effects.iter().filter_map(|e| if let Effect::GiveMoney(x) = e { Some(*x as f64) } else { None }).collect();
                    assert_eq!(money, h.money, "tick {n}: GIVE MONEY");
                    assert!(h.speech.is_empty() && h.mp2.is_empty() && h.mou.is_empty(), "tick {n}: unused calls");
                    t.count_tick();
                    ticks += 1;
                }
                _ => {}
            }
        }
        eprintln!("{}: {chapters} chapters, {ticks} ticks, {cmds} commands, {events} events ({forced} forced) exact",
            f.file_name().unwrap().to_string_lossy());
        assert!(ticks > 0);
    }
}
