//! `IG2_PROFILE=1`: the window's frame time per phase, logged to stderr once a second (docs/perf.md).
//!
//! A line per second: the frames, their mean and worst time, each phase's mean ms per frame (wall clock on the
//! main thread; `present` includes the wait for the swap chain), and the counters per frame (texture and mesh
//! uploads, quads, draw batches). `IG2_PROFILE_TOUR=KEYS` also walks the screens: every `IG2_PROFILE_STEP` seconds
//! (default 4) the next key of the comma list (`F1`..`F10`, `Esc`, `Enter`, `Tab`, `Space`, a letter, `vsync`; any key skips a video) goes in as if pressed, and
//! the log names the screen of each second. Without the variable every call is one load of a flag.

use std::cell::RefCell;
use std::sync::OnceLock;
use std::time::Instant;

pub fn on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("IG2_PROFILE").is_ok_and(|v| v != "0" && !v.is_empty()))
}

#[derive(Default)]
struct State {
    /// Per phase: total ms over the second and the worst frame's ms.
    phases: Vec<(&'static str, f64, f64)>,
    counters: Vec<(&'static str, u64)>,
    frames: u32,
    frame_max: f64,
    frame_at: Option<Instant>,
    since: Option<Instant>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// A phase's timer: its time is added when it drops.
pub struct Scope(Option<(&'static str, Instant)>);

pub fn scope(name: &'static str) -> Scope {
    Scope(on().then(|| (name, Instant::now())))
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some((name, at)) = self.0 {
            let ms = at.elapsed().as_secs_f64() * 1000.0;
            STATE.with_borrow_mut(|s| match s.phases.iter_mut().find(|p| p.0 == name) {
                Some(p) => {
                    p.1 += ms;
                    p.2 = p.2.max(ms);
                }
                None => s.phases.push((name, ms, ms)),
            });
        }
    }
}

pub fn count(name: &'static str, n: u64) {
    if !on() {
        return;
    }
    STATE.with_borrow_mut(|s| match s.counters.iter_mut().find(|c| c.0 == name) {
        Some(c) => c.1 += n,
        None => s.counters.push((name, n)),
    });
}

/// The end of a drawn frame on `screen`: once a second the log line.
pub fn frame(screen: &str) {
    if !on() {
        return;
    }
    let now = Instant::now();
    STATE.with_borrow_mut(|s| {
        if let Some(at) = s.frame_at {
            s.frame_max = s.frame_max.max((now - at).as_secs_f64() * 1000.0);
        }
        s.frame_at = Some(now);
        s.frames += 1;
        let since = *s.since.get_or_insert(now);
        let secs = (now - since).as_secs_f64();
        if secs < 1.0 {
            return;
        }
        let n = s.frames as f64;
        let mut line = format!("[prof] {screen:<10} {:>6.1} fps  mean {:>6.2} ms  worst {:>6.1} ms |", n / secs, secs * 1000.0 / n, s.frame_max);
        for (name, total, max) in &s.phases {
            line += &format!(" {name} {:.2} (max {max:.1})", total / n);
        }
        line += " |";
        for (name, c) in &s.counters {
            line += &format!(" {name} {:.3}", *c as f64 / n);
        }
        eprintln!("{line}");
        *s = State { frame_at: s.frame_at, since: Some(now), ..State::default() };
    });
}

/// `IG2_PROFILE_TOUR`'s keys and their step.
pub struct Tour {
    keys: Vec<String>,
    step: f32,
    start: Instant,
    next: usize,
}

impl Tour {
    pub fn from_env() -> Option<Self> {
        let keys = std::env::var("IG2_PROFILE_TOUR").ok()?;
        let step = std::env::var("IG2_PROFILE_STEP").ok().and_then(|s| s.parse().ok()).unwrap_or(4.0);
        Some(Self { keys: keys.split(',').map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect(), step, start: Instant::now(), next: 0 })
    }

    /// The key due now, if any; `Some(None)` once the tour is over (quit).
    pub fn due(&mut self) -> Option<Option<String>> {
        if self.start.elapsed().as_secs_f32() < self.step * (self.next + 1) as f32 {
            return None;
        }
        self.next += 1;
        Some(self.keys.get(self.next - 1).cloned())
    }
}
