//! The in-game multiplayer wait overlay (re/screens/input.md §9): `MOUSE_DrawOver` (exe 0x4c95b0, ELF
//! 0x5306a0) opens it once the lockstep has stood still for `STALL` (`MultiSendPackets` exe 0x5126b0 sets
//! `WaitingForVector` 0xdb9748 after 5000 ms without the turn's data) and runs its own frame loop until the data comes:
//! the screens below take no mouse input (`DoKeyboard` still runs in its `Fake_FRISSITES`). `FUN_004c8b80` draws the boxes (fader pass 3), `GFX_DrawOver` (exe 0x4c8f80, ELF
//! 0x52f920) the texts: the title, a cell per late player ("Slow connection!", the name, the race), after `ARM` the
//! host's "Close connection" under each (`MultiKickClient(client, 1)`) and "EXIT GAME" (`ExitToMainScreen`).
//!
//! Ours: a late player is one the host's relay lacks the next turn's input from (`Host::late`), or one the session
//! awaits (dropped in a battle, or a migration's survivor); a joiner that knows none waits for the host (the exe's
//! client without the host's late list, `FUN_00510fd0`). "Close connection" takes the drop path (the player may come
//! back through the door). The joining question (`DrawOverMode` 2) has no counterpart: the door admits a returning
//! human by name and refuses newcomers to a started game.

use std::time::{Duration, Instant};

use crate::font::{Align, Font, Virtual};
use crate::render::{Frame, TexId};

/// The stall before the overlay opens (`MultiSendPackets`: `SDL_GetTicks() - 0xdb9728 > 5000`).
pub const STALL: Duration = Duration::from_millis(5000);
/// The overlay's items come alive this long after it opens (`MOUSE_DrawOver`: `3999 < ticks - acttime`).
pub const ARM: Duration = Duration::from_millis(4000);
/// `DrawOverSelected` of the exit strip.
pub const EXIT: u32 = 9;

/// `DrawOverX/Y` (0x28, 0x32), `DrawOverXm` (0x230), `DrawOverYm` in the players mode (0x10e): literals of
/// `FUN_004c8b80`.
const X: f32 = 40.0;
const Y: f32 = 50.0;
const W: f32 = 560.0;
const H: f32 = 270.0;
/// A player cell's pitch (0x8c), four a row.
const PITCH: f32 = 140.0;

/// A late player's cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Late {
    pub empire: u8,
    pub name: Vec<u8>,
    /// The second line (the exe's per-slot text at 0xbae593; ours: the empire's race).
    pub text: Vec<u8>,
}

/// What a press on the overlay asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// The host's "Close connection" under `empire`'s cell.
    Close(u8),
    /// "EXIT GAME": `ExitToMainScreen`, the main menu.
    Exit,
}

/// The overlay's state (`DrawOverMode`, `DrawOverSelected`, `acttime`).
#[derive(Debug, Clone, Default)]
pub struct Wait {
    /// The local peer's turn at the last look and since when it has stood still (a pause doesn't count).
    turn: Option<(u64, Instant)>,
    /// Open since (`acttime` 0xaba978); `None`: closed.
    pub open: Option<Instant>,
    /// The item under the pointer: 0 none, `k + 1` the `k`-th cell's "Close connection", [`EXIT`].
    pub hover: u32,
    pub late: Vec<Late>,
    /// The session awaits a dropped or migrating player (`MultiPlayerReconnectingMode`: the title).
    pub reconnecting: bool,
    /// This machine hosts (`SuperVisor`): the cells' buttons.
    pub host: bool,
}

impl Wait {
    /// Each frame of a multiplayer game: `turn` = the local peer's, `paused` = a player's pause holds the session (no
    /// stall), the late players, and whether the session awaits a reconnect. Opens after [`STALL`] without a turn,
    /// closes once one runs.
    pub fn update(&mut self, now: Instant, turn: u64, paused: bool, reconnecting: bool, host: bool, late: Vec<Late>) {
        match self.turn {
            Some((t, _)) if t == turn && !paused => {}
            _ => self.turn = Some((turn, now)),
        }
        let stalled = self.turn.is_some_and(|(_, since)| now.duration_since(since) >= STALL);
        if !stalled {
            self.open = None;
            self.hover = 0;
        } else if self.open.is_none() {
            self.open = Some(now);
        }
        self.late = late;
        self.reconnecting = reconnecting;
        self.host = host;
    }

    /// `--mp-wait SPEC` (`--snapshot`, run_screendiff.py mpwait): the overlay open and armed as `probes/mpwait.js` stages
    /// the exe, `SPEC` = `flags;name|text;…` with flags among `host`, `reconnecting` (comma-separated).
    pub fn staged(spec: &str) -> Self {
        let mut parts = spec.split(';');
        let flags: Vec<&str> = parts.next().unwrap_or("").split(',').collect();
        let late = parts
            .filter(|p| !p.is_empty())
            .enumerate()
            .map(|(k, p)| {
                let (name, text) = p.split_once('|').unwrap_or((p, ""));
                Late { empire: k as u8 + 2, name: name.as_bytes().to_vec(), text: text.as_bytes().to_vec() }
            })
            .collect();
        let open = Instant::now().checked_sub(STALL).unwrap_or_else(Instant::now);
        Self { turn: None, open: Some(open), hover: 0, late, reconnecting: flags.contains(&"reconnecting"), host: flags.contains(&"host") }
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Anything of it is drawn: the overlay, or the title alone while a reconnect is awaited.
    pub fn shown(&self) -> bool {
        self.open.is_some() || self.reconnecting
    }

    fn armed(&self, now: Instant) -> bool {
        self.open.is_some_and(|t| now.duration_since(t) >= ARM)
    }

    /// The `k`-th cell's top left.
    fn cell(k: usize) -> [f32; 2] {
        [X + 10.0 + (k % 4) as f32 * PITCH, Y + 10.0 + (k / 4) as f32 * PITCH]
    }

    /// `MOUSE_DrawOver`'s hit test at `at` (virtual pixels): nothing before [`ARM`]; the host's cell buttons (the last
    /// hit wins), the exit strip.
    pub fn spot(&self, at: [f32; 2], now: Instant) -> u32 {
        if !self.armed(now) {
            return 0;
        }
        let [x, y] = at;
        let mut id = 0;
        if self.host {
            for k in 0..self.late.len() {
                let [cx, _] = Self::cell(k);
                let row = Y + (k / 4) as f32 * PITCH;
                if (cx..=cx + 120.0).contains(&x) && (row + 90.0..=row + 120.0).contains(&y) {
                    id = k as u32 + 1;
                }
            }
        }
        if (X..=X + W).contains(&x) && (Y + H + 10.0..=Y + H + 40.0).contains(&y) {
            id = EXIT;
        }
        id
    }

    /// The pointer moved: `DrawOverSelected`.
    pub fn hover(&mut self, at: [f32; 2], now: Instant) {
        self.hover = self.spot(at, now);
    }

    /// A press at `at`: the item's act; the overlay closes (it opens again at once while the stall lasts, its items
    /// [`ARM`] later).
    pub fn press(&mut self, at: [f32; 2], now: Instant) -> Option<Act> {
        let act = match self.spot(at, now) {
            0 => return None,
            EXIT => Act::Exit,
            k => Act::Close(self.late.get(k as usize - 1)?.empire),
        };
        self.open = None;
        self.hover = 0;
        Some(act)
    }

    /// `FUN_004c8b80`'s boxes (`FadeKerettelKerekit`, untextured) and `GFX_DrawOver`'s texts (`KiirStr`: len, size,
    /// flags & 3 = alignment), `texts` = `GetString(18, 5, n)`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, f: &mut Frame, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], texts: &dyn Fn(u16) -> Vec<u8>, now: Instant) {
        let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
        // The title comes first in `GFX_DrawOver`, before its `DrawOverMode` test: it shows whenever a reconnect is
        // awaited, the overlay open or not.
        let title = |f: &mut Frame| {
            if self.reconnecting {
                font.text(f, v, &texts(2), 40.0, 40.0, 0x28, 4, Align::Left, col(3), None);
            }
        };
        if self.open.is_none() {
            title(f);
            return;
        }
        let armed = self.armed(now);
        let panel = |f: &mut Frame, r: [f32; 4]| crate::kit::fader::fader(f, white, v, r, [0.0, 20.0, 30.0], 60.0, [0.0, 15.0, 25.0], 30.0);
        let cell = |f: &mut Frame, r: [f32; 4]| crate::kit::fader::fader(f, white, v, r, [0.0, 10.0, 0.0], 60.0, [0.0, 5.0, 0.0], 30.0);
        panel(f, [X, Y, W, H]);
        for k in 0..self.late.len() {
            let [cx, cy] = Self::cell(k);
            cell(f, [cx, cy, 120.0, 70.0]);
            if armed && self.host {
                cell(f, [cx, cy + 80.0, 120.0, 30.0]);
            }
        }
        if armed {
            panel(f, [X, Y + H + 10.0, W, 30.0]);
        }
        title(f);
        for (k, l) in self.late.iter().enumerate() {
            let [cx, cy] = Self::cell(k);
            for (dy, s) in [(10.0, texts(3)), (30.0, l.name.clone()), (50.0, l.text.clone())] {
                font.text(f, v, &s, cx, cy + dy, 0x14, 2, Align::Field, col(0x25), None);
            }
            if armed && self.host {
                let c = if self.hover == k as u32 + 1 { 0x25 } else { 1 };
                font.text(f, v, &texts(4), cx, cy + 90.0, 0x14, 2, Align::Field, col(c), None);
            }
        }
        if armed {
            let c = if self.hover == EXIT { 0x25 } else { 1 };
            font.text(f, v, &texts(5), 320.0, Y + H + 20.0, 0x46, 3, Align::Centre, col(c), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn late(n: u8) -> Vec<Late> {
        (0..n).map(|e| Late { empire: e + 2, name: b"x".to_vec(), text: Vec::new() }).collect()
    }

    /// The stall opens it after 5 s (a pause doesn't count), the items arm 4 s later, a turn closes it; the host's
    /// buttons and the exit strip at the exe's rectangles.
    #[test]
    fn the_overlay_opens_on_a_stall_and_arms_later() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut w = Wait::default();
        w.update(at(0), 7, false, false, true, late(5));
        w.update(at(4999), 7, false, false, true, late(5));
        assert!(!w.is_open());
        // A pause holds the turn without a stall.
        w.update(at(6000), 7, true, false, true, late(5));
        assert!(!w.is_open());
        w.update(at(11000), 7, false, false, true, late(5));
        assert!(w.is_open());
        assert_eq!(w.spot([100.0, 345.0], at(14999)), 0, "not armed yet");
        assert_eq!(w.spot([100.0, 345.0], at(15000)), EXIT);
        // Cell 4 (row 1, column 0): its button at (50, 280 .. 310).
        assert_eq!(w.spot([60.0, 285.0], at(15000)), 5);
        assert_eq!(w.spot([60.0, 145.0], at(15000)), 1);
        assert_eq!(w.spot([60.0, 125.0], at(15000)), 0, "the cell itself isn't a button");
        assert_eq!(w.press([60.0, 285.0], at(15000)), Some(Act::Close(6)));
        assert!(!w.is_open());
        w.update(at(15010), 7, false, false, true, late(5));
        assert!(w.is_open(), "the stall goes on: open again");
        assert_eq!(w.spot([60.0, 285.0], at(18000)), 0, "armed 4 s after it opened again");
        // A joiner has no buttons, only the exit.
        w.update(at(20000), 7, false, false, false, late(5));
        assert_eq!(w.spot([60.0, 285.0], at(20000)), 0);
        assert_eq!(w.press([300.0, 335.0], at(20000)), Some(Act::Exit));
        w.update(at(20010), 8, false, false, false, late(5));
        assert!(!w.is_open(), "a turn ran");
        // The title alone while a reconnect is awaited.
        assert!(!w.shown());
        w.update(at(20020), 8, false, true, false, Vec::new());
        assert!(w.shown() && !w.is_open());
    }
}
