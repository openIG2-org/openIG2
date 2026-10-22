//! Input scripts (re/screens/sound.md §3): the same JSON drives ig2.exe (tools/trace/screen.js `--script`) and
//! `--snapshot --script`, so a screen's frame sequence can be compared grab by grab. Times are UI ticks (10 ms) from
//! the first frame the screen is up; an event at t goes in before tick t + 1, a grab at t shows the screen after
//! exactly t ticks.
//!
//! ```json
//! {"events": [{"t": 300, "mouse": [507, 50]}, {"t": 330, "press": [507, 50], "button": 1, "hold": 3},
//!             {"t": 400, "key": "pagedown", "hold": 3}],
//!  "grabs": [10, 30, 60]}
//! ```
//! A press is the pointer moved there, the button down, and up `hold` ticks later; a key is down and up `hold`
//! ticks later (names: down, up, right, left, enter, pageup, pagedown, escape, d, k, space).

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// The pointer at a virtual point.
    Move([f32; 2]),
    /// A mouse button (1 left, 3 right) down or up at a virtual point.
    Button { button: u8, down: bool, at: [f32; 2] },
    /// A key down or up.
    Key { name: String, down: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub t: u32,
    pub input: Input,
}

#[derive(Debug, Clone, Default)]
pub struct Script {
    /// In time order (stable: same-tick events keep the file's order).
    pub events: Vec<Event>,
    /// Sorted.
    pub grabs: Vec<u32>,
}

impl Script {
    pub fn load(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("--script {path}: {e}"))?;
        Self::parse(&text).map_err(|e| format!("--script {path}: {e}"))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let point = |e: &Value, k: &str| -> Option<[f32; 2]> {
            let a = e.get(k)?.as_array()?;
            Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32])
        };
        let mut events = Vec::new();
        for e in v.get("events").and_then(Value::as_array).into_iter().flatten() {
            let t = e.get("t").and_then(Value::as_u64).ok_or("an event without t")? as u32;
            let hold = e.get("hold").and_then(Value::as_u64).unwrap_or(3) as u32;
            if let Some(at) = point(e, "press") {
                let button = e.get("button").and_then(Value::as_u64).unwrap_or(1) as u8;
                events.push(Event { t, input: Input::Move(at) });
                events.push(Event { t, input: Input::Button { button, down: true, at } });
                events.push(Event { t: t + hold, input: Input::Button { button, down: false, at } });
            } else if let Some(at) = point(e, "mouse") {
                events.push(Event { t, input: Input::Move(at) });
            } else if let Some(name) = e.get("key").and_then(Value::as_str) {
                events.push(Event { t, input: Input::Key { name: name.to_string(), down: true } });
                events.push(Event { t: t + hold, input: Input::Key { name: name.to_string(), down: false } });
            } else {
                return Err(format!("event at t {t}: no press, mouse or key"));
            }
        }
        events.sort_by_key(|e| e.t);
        let mut grabs: Vec<u32> = v.get("grabs").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).map(|t| t as u32).collect();
        grabs.sort_unstable();
        Ok(Self { events, grabs })
    }

    /// The last tick the run needs.
    pub fn end(&self) -> u32 {
        self.grabs.last().copied().unwrap_or(0).max(self.events.last().map_or(0, |e| e.t))
    }

    /// The events that go in before tick `t + 1`.
    pub fn at(&self, t: u32) -> impl Iterator<Item = &Event> {
        self.events.iter().filter(move |e| e.t == t)
    }
}

/// `--snapshot OUT.png --script S`: grab t is written to `OUT-t<t>.png`.
pub fn grab_path(out: &std::path::Path, t: u32) -> std::path::PathBuf {
    let stem = out.file_stem().map_or_else(|| "snapshot".into(), |s| s.to_string_lossy().into_owned());
    out.with_file_name(format!("{stem}-t{t}.png"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presses_and_keys_expand() {
        let s = Script::parse(r#"{"events": [{"t": 9, "key": "d"}, {"t": 5, "press": [10, 20], "hold": 2}], "grabs": [30, 10]}"#).unwrap();
        assert_eq!(s.grabs, vec![10, 30]);
        let ts: Vec<u32> = s.events.iter().map(|e| e.t).collect();
        assert_eq!(ts, vec![5, 5, 7, 9, 12]);
        assert_eq!(s.events[1].input, Input::Button { button: 1, down: true, at: [10.0, 20.0] });
        assert_eq!(s.events[4].input, Input::Key { name: "d".into(), down: false });
        assert_eq!(s.end(), 30);
        assert_eq!(grab_path(std::path::Path::new("x/r-client.png"), 30), std::path::PathBuf::from("x/r-client-t30.png"));
    }
}
