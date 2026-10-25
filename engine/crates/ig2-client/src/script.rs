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
//! A press is the pointer moved there, the button down, and up `hold` ticks later; a drag
//! (`{"t": 5, "drag": [[x0, y0], [x1, y1]], "button": 1, "hold": 20}`) the button down at the first point, the pointer
//! moved a step a tick on the straight line, and up at the second point `hold` ticks later; a key is down and up `hold`
//! ticks later (names: down, up, right, left, enter, pageup, pagedown, escape, d, k, space, c, e, i, m, s, 1, 2, 3, tab, delete);
//! `{"t": 5, "keydown": "enter"}` / `{"t": 9, "keyup": "enter"}` send one key event alone (a held key's OS repeats).

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
        let point_of = |v: &Value| -> Option<[f32; 2]> {
            let a = v.as_array()?;
            Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32])
        };
        let point = |e: &Value, k: &str| e.get(k).and_then(point_of);
        let mut events = Vec::new();
        for e in v.get("events").and_then(Value::as_array).into_iter().flatten() {
            let t = e.get("t").and_then(Value::as_u64).ok_or("an event without t")? as u32;
            let hold = e.get("hold").and_then(Value::as_u64).unwrap_or(3) as u32;
            if let Some(at) = point(e, "press") {
                let button = e.get("button").and_then(Value::as_u64).unwrap_or(1) as u8;
                events.push(Event { t, input: Input::Move(at) });
                events.push(Event { t, input: Input::Button { button, down: true, at } });
                events.push(Event { t: t + hold, input: Input::Button { button, down: false, at } });
            } else if let Some((from, to)) = e.get("drag").and_then(Value::as_array).and_then(|d| Some((point_of(d.first()?)?, point_of(d.get(1)?)?))) {
                let button = e.get("button").and_then(Value::as_u64).unwrap_or(1) as u8;
                let hold = hold.max(1);
                events.push(Event { t, input: Input::Move(from) });
                events.push(Event { t, input: Input::Button { button, down: true, at: from } });
                for k in 1..hold {
                    let f = k as f32 / hold as f32;
                    let at = [(from[0] + (to[0] - from[0]) * f).round(), (from[1] + (to[1] - from[1]) * f).round()];
                    events.push(Event { t: t + k, input: Input::Move(at) });
                }
                events.push(Event { t: t + hold, input: Input::Move(to) });
                events.push(Event { t: t + hold, input: Input::Button { button, down: false, at: to } });
            } else if let (Some((button, down)), Some(at)) = (e.get("down").map(|b| (b, true)).or_else(|| e.get("up").map(|b| (b, false))), point(e, "at")) {
                // A lone button down or up (screen.js's raw mouse events).
                events.push(Event { t, input: Input::Button { button: button.as_u64().unwrap_or(1) as u8, down, at } });
            } else if let Some(at) = point(e, "mouse") {
                events.push(Event { t, input: Input::Move(at) });
            } else if let Some(name) = e.get("text").and_then(Value::as_str) {
                // A typed character alone (the exe's SDL_TEXTINPUT): the client's key down with that character.
                events.push(Event { t, input: Input::Key { name: name.to_string(), down: true } });
            } else if let Some((name, down)) = e.get("keydown").map(|v| (v, true)).or_else(|| e.get("keyup").map(|v| (v, false))) {
                // A lone key down (an OS repeat when the key is already down) or up, as screen.js passes them on.
                let name = name.as_str().ok_or("keydown/keyup without a name")?;
                events.push(Event { t, input: Input::Key { name: name.to_string(), down } });
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

/// `OUT-sounds.jsonl`: the sounds a script's run started, one record per sound in the tracer's shape
/// (re/screens/sound.md §2: `{k: 'sound', table, idx, t}`; `t` null = before the script's first frame).
pub fn write_sounds(out: &std::path::Path, heard: &[(Option<u32>, crate::audio::SoundId)]) -> Result<std::path::PathBuf, String> {
    let stem = out.file_stem().map_or_else(|| "snapshot".into(), |s| s.to_string_lossy().into_owned());
    let path = out.with_file_name(format!("{stem}-sounds.jsonl"));
    let mut text = String::new();
    for (t, id) in heard {
        let rec = serde_json::json!({"k": "sound", "table": id.table.name(), "idx": id.idx, "t": t});
        text.push_str(&rec.to_string());
        text.push('\n');
    }
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
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
        let d = Script::parse(r#"{"events": [{"t": 2, "drag": [[0, 0], [40, 20]], "hold": 4}]}"#).unwrap();
        let moves: Vec<_> = d.events.iter().filter_map(|e| if let Input::Move(at) = e.input { Some((e.t, at)) } else { None }).collect();
        assert_eq!(moves, vec![(2, [0.0, 0.0]), (3, [10.0, 5.0]), (4, [20.0, 10.0]), (5, [30.0, 15.0]), (6, [40.0, 20.0])]);
        assert_eq!(d.events.last().unwrap().input, Input::Button { button: 1, down: false, at: [40.0, 20.0] });
        let r = Script::parse(r#"{"events": [{"t": 4, "up": 3, "at": [1, 2]}, {"t": 1, "down": 3, "at": [1, 2]}]}"#).unwrap();
        assert_eq!(r.events[0].input, Input::Button { button: 3, down: true, at: [1.0, 2.0] });
        assert_eq!(r.events[1].input, Input::Button { button: 3, down: false, at: [1.0, 2.0] });
        assert_eq!(grab_path(std::path::Path::new("x/r-client.png"), 30), std::path::PathBuf::from("x/r-client-t30.png"));
    }
}
