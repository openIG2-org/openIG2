//! The tutorial on the game screens (re/rules/tutorial.md §2 "Drawing", §6): the interpreter (`ig2_sim::tutorial`)
//! on the 100 Hz UI clock beside the game, the recorded input it replays (`Mouse_Tutorial_Kezel` exe 0x510930), its
//! text window (`TutorialKeretFrissit`, `TutorialSzovegFrissit` exe 0x612740) and the pointer it parks.
//!
//! The interpreter keeps `TutorialActive`, `TutorialPaused` and `TutorialEvent` itself; the game's copies (exe
//! 0xf6915d, 0xf66f2a, 0xf66f40, which the sim's readers and `Game::tutorial_check_event` use) are written from it
//! every tick, and an event the game cleared resumes it (`TutorialCheckEvent`).

use ig2_formats::tutorial::{parse_mou, MouseFrame};
use ig2_formats::Install;
use ig2_sim::save::va;
use ig2_sim::tutorial::{Effect, Tutorial, TutorialHost, COLS, ROWS, SCREEN_OPTIONS};

use crate::font::{Align, Font, Virtual};
use crate::render::{Frame, TexId};

/// `tutorialpath` + `tutorialactpath` ("tutorial/" + "main/").
/// `TutorialDisableHajokEladasa`.
const SELL_SHIPS_OFF: u32 = 0xF66FFD;
const DIR: &str = "dataraw/tutorial/main/";
/// `TutorialStart("main", n)`: `tutorial/main/script/main.tut`.
const SCRIPT: &str = "dataraw/tutorial/main/script/main.tut";
/// Where `Mouse_Tutorial_Kezel` parks the pointer while no recording plays (`Mouse_x/y` = 0x140, 200).
pub const PARKED: [i32; 2] = [0x140, 200];

/// What the interpreter reads from the game and the screens this tick.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    /// `AktKep`.
    pub screen: i32,
    /// `Counter`, the 20 Hz game counter.
    pub counter: i32,
    /// `Atlepes || Belep`.
    pub transition: bool,
    pub explored: i32,
    pub owned: i32,
    pub tanks: i32,
    pub assault: i32,
}

impl Inputs {
    /// The counts `TutorialInitEvents` and `TutorialCheckEvents` read, from the game's state.
    pub fn counts(mut self, state: &ig2_sim::save::SaveState) -> Self {
        let m = &state.mem;
        let en = m.i32(va::EN);
        let planet = |p: usize| va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32;
        let planets = state.planet_count();
        self.explored = (1..=planets).filter(|&p| m.i16(planet(p) + 0xd8 + 2 * en.clamp(0, 18) as u32) == 1).count() as i32;
        self.owned = (1..=planets).filter(|&p| m.u8(planet(p) + 0x20) as i32 == en).count() as i32;
        // `CountMyTanks()` (exe 0x60b5d0), its fleet half: one per `Tankok` entry held by EN's fleets (entries, not
        // units; the battlefields' half is [`field_tanks`]); `TutorialCheckEvents`' count: the units of the fleets
        // named "Assault fleet" whose design has byte +0x36 = 0.
        let n = m.i32(va::VEHICLE_COUNT).max(0) as usize;
        let raw = m.read(va::VEHICLES, 8 * (n + 1));
        let (mut tanks, mut assault) = (0, 0);
        for c in raw.chunks_exact(8).skip(1) {
            let (holder, design, count) = (i16::from_le_bytes([c[0], c[1]]), i16::from_le_bytes([c[2], c[3]]), i32::from_le_bytes([c[4], c[5], c[6], c[7]]));
            let Some(f) = state.fleets.get(holder.max(0) as usize).filter(|f| holder > 0 && design != 0x7fff && f[0x15] as i32 == en) else { continue };
            tanks += 1;
            let grunt = state.designs.get(en.max(0) as usize).and_then(|d| d.get(design.max(0) as usize)).is_some_and(|r| r[0x36] == 0);
            if f[..0x15].split(|&b| b == 0).next() == Some(b"Assault fleet".as_slice()) && grunt {
                assault += count;
            }
        }
        (self.tanks, self.assault) = (tanks, assault);
        self
    }
}

/// `CountMyTanks()`' second half (exe 0x60b5d0): every planet's battlefield (`Warfare[p − 1]`), each alive unit
/// (`WF+0x188`) without flag 0x4028000 (unit `+0x188`) adds its group size (`+0x190`), whoever owns it. Built
/// tanks stand there, not in a fleet.
pub fn field_tanks(state: &ig2_sim::save::SaveState) -> i32 {
    (1..=state.planet_count())
        .filter_map(|p| state.ground_field(p))
        .map(|f| f.units.iter().zip(&f.alive).filter(|(u, alive)| **alive && u.flags & 0x402_8000 == 0).map(|(u, _)| u.hp.len() as i32).sum::<i32>())
        .sum()
}

/// The interpreter's view of the client.
struct Host<'a> {
    i: Inputs,
    /// The game, for the counts read only at a wait ([`field_tanks`]).
    state: Option<&'a ig2_sim::save::SaveState>,
    install: &'a Install,
    mou: &'a mut Vec<MouseFrame>,
    effects: &'a mut Vec<Effect>,
}

impl TutorialHost for Host<'_> {
    fn screen(&self) -> i32 {
        self.i.screen
    }
    fn counter(&self) -> i32 {
        self.i.counter
    }
    fn transition(&self) -> bool {
        self.i.transition
    }
    // `PLAYWAV`: the install ships no `mfx/tutorial/` speech (re/screens/audio.md), so `TutorialStartMp2_2` finds no
    // file: length 0, nothing plays (the exe's own case).
    fn mouse_samples(&mut self, name: &[u8]) -> i32 {
        let path = format!("{DIR}{}", String::from_utf8_lossy(name));
        *self.mou = self.install.read(&path).ok().and_then(|b| parse_mou(&b).ok()).unwrap_or_default();
        self.mou.len() as i32
    }
    fn explored_planets(&self) -> i32 {
        self.i.explored
    }
    fn owned_planets(&self) -> i32 {
        self.i.owned
    }
    fn my_tanks(&self) -> i32 {
        self.i.tanks + self.state.map_or(0, field_tanks)
    }
    fn assault_tanks(&self) -> i32 {
        self.i.assault
    }
    fn effect(&mut self, e: Effect) {
        self.effects.push(e);
    }
}

/// The recorded input of one UI tick (`Mouse_Tutorial_Kezel` step 3): the pointer in virtual pixels, the buttons,
/// and the keys `ReadNext` hands out.
#[derive(Debug, Clone, PartialEq)]
pub struct Replay {
    pub at: [i32; 2],
    pub left: bool,
    pub right: bool,
    pub modifiers: u8,
    pub keys: Vec<(u8, u32)>,
    /// No sample plays: `Mouse_Tipus = -2`, no pointer drawn [obs: the exe's captures while parked].
    pub parked: bool,
}

/// A running tutorial.
pub struct TutorialUi {
    pub t: Tutorial,
    /// The `.mou` recording `PLAYMOUSE` loaded last.
    mou: Vec<MouseFrame>,
    /// `AktIdo200`: the cursor blinks on the 200 ms half of the wall clock.
    pub ms: u32,
    /// `Counter`: the game's 20 Hz ticks since the start (`COUNT`).
    pub counter: i32,
    /// `AktKep` at the last tick: an entry raises the screen's `*_Belep` event.
    kep: i32,
    /// `DISABLE OVERRUN`'s saved battle debug flags (`DebugData_Battle`, `DebugDatagrBattle`, `GrSpOverrun`,
    /// `GrNoOverrun`: exe 0xB77230..0xB7723C), put back by `TutorialStop` (`KellOverrunVisszaAllitas`).
    pub overrun: Option<[i32; 4]>,
    /// The debug AI-off flag (exe 0xB77244) at `TutorialStart`, put back by `TutorialStop`.
    pub ai_off: Option<i32>,
    /// The first `TutorialFrissit` has run. The exe's runs at `Counter` 0 (traces `tutorial-ch1..5`), so the
    /// chapter's opening commands (`DISABLE AI`, the gates) come before the game's first tick.
    pub ticked: bool,
}

impl TutorialUi {
    /// `TutorialStart("main", chapter)` (exe 0x611f80).
    pub fn start(install: &Install, chapter: i32, i: Inputs, state: Option<&ig2_sim::save::SaveState>) -> Result<(Self, Vec<Effect>), String> {
        let script = install.read(SCRIPT).map_err(|e| format!("{SCRIPT}: {e}"))?;
        let mut ui = TutorialUi { t: Tutorial::default(), mou: Vec::new(), ms: 0, counter: 0, kep: 2, overrun: None, ai_off: None, ticked: false };
        let mut effects = Vec::new();
        let mut h = Host { i, state, install, mou: &mut ui.mou, effects: &mut effects };
        ui.t.start(script, chapter, &mut h);
        (ui.t.explored, ui.t.owned, ui.t.tanks) = (i.explored, i.owned, h.my_tanks());
        Ok((ui, effects))
    }

    /// `TutorialActive || TutorialPaused`.
    pub fn running(&self) -> bool {
        self.t.active || self.t.paused
    }

    /// While the script plays, the live pointer, buttons and modifier keys are dropped (`MainCallBackEventHandler`),
    /// except on the options screen.
    pub fn owns_input(&self, screen: i32) -> bool {
        self.t.active && screen != SCREEN_OPTIONS
    }

    /// `Mouse_Tutorial_Kezel` (before the screens' updates): the sample index steps once per tick of active play;
    /// the recorded input of this tick, or the parked pointer (`None` while paused or on the options screen).
    pub fn replay_step(&mut self, screen: i32) -> Option<Replay> {
        self.t.mouse_tick(screen);
        if !self.owns_input(screen) {
            return None;
        }
        Some(match self.t.mouse_sample().and_then(|i| self.mou.get(i)) {
            Some(s) => Replay { at: [s.x, s.y], left: s.left, right: s.right, modifiers: s.modifiers, keys: s.keys.iter().copied().filter(|k| k.1 != 0).collect(), parked: false },
            None => Replay { at: PARKED, left: false, right: false, modifiers: 0, keys: Vec::new(), parked: true },
        })
    }

    /// The screens' entry events (re/rules/tutorial.md §4): `Starmap_Belep` 0x1a, `Colony_Belep` 7,
    /// `Diplomacy_Belep` 0x28; `SpaceWar_Belep` ends `WAITFORBATTLE`.
    /// The screen the last tick saw (`entered`'s `AktKep`).
    pub fn screen(&self) -> i32 {
        self.kep
    }

    pub fn entered(&mut self, kep: i32, game: &mut ig2_sim::game::Game) {
        if kep == self.kep {
            return;
        }
        self.kep = kep;
        match kep {
            2 => game.tutorial_check_event(0x1a),
            3 => game.tutorial_check_event(7),
            6 => game.tutorial_check_event(0x28),
            11 => self.t.battle_started(),
            _ => {}
        }
    }

    /// An event the game raised (`TutorialCheckEvent`, mirrored in the game's `TutorialEvent`) resumes the script.
    pub fn pull(&mut self, mem: &ig2_sim::save::Memory) {
        if self.t.event != 0 && mem.i32(va::TUTORIAL_EVENT) == 0 {
            self.t.check_event(self.t.event);
        }
    }

    /// `TutorialFrissit` and `TutorialCounter++` (after the screens' updates); the effects in the script's order.
    pub fn tick(&mut self, install: &Install, i: Inputs, state: Option<&ig2_sim::save::SaveState>) -> Vec<Effect> {
        self.ticked = true;
        let mut effects = Vec::new();
        let mut h = Host { i, state, install, mou: &mut self.mou, effects: &mut effects };
        self.t.frissit(&mut h);
        self.t.count_tick();
        effects
    }

    /// The game's copies of the interpreter's flags.
    pub fn push(&self, mem: &mut ig2_sim::save::Memory) {
        mem.set_u8(va::TUTORIAL_ACTIVE, self.t.active as u8);
        mem.set_u8(va::TUTORIAL_PAUSED, self.t.paused as u8);
        mem.set_i32(va::TUTORIAL_EVENT, self.t.event);
        // `TutorialUjKemekLetiltva`, read by `FRISSIT_Spy` (the sim's `no_recruits`).
        mem.set_u8(va::TUTORIAL_NO_RECRUITS, self.t.gates.new_spies as u8);
        // `TutorialDisableHajokEladasa` (exe 0xf66ffd), read by the fleet info's sale menu.
        mem.set_u8(SELL_SHIPS_OFF, self.t.gates.sell_ships as u8);
        mem.set_i32(va::TUTORIAL_SPY_MISSION, self.t.gates.spy_mission);
    }

    /// A running tutorial as `tools/trace/probes/tutorial.js` logged it (`--tutorial-window`, a `tutwin` record): the
    /// window, the flags and the clock the overlay reads.
    pub fn from_record(line: &str) -> Result<Self, String> {
        let j: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let num = |k: &str| j.get(k).and_then(|v| v.as_f64()).ok_or(format!("tutwin: no {k}"));
        let mut t = Tutorial::default();
        let w = &mut t.window;
        (w.x, w.y) = (num("x")? as f32, num("y")? as f32);
        let grid = j.get("grid").and_then(|g| g.as_array()).ok_or("tutwin: no grid")?;
        for (i, c) in grid.chunks_exact(2).take(ROWS * COLS).enumerate() {
            w.grid[i / COLS][i % COLS] = (c[0].as_u64().unwrap_or(0x20) as u8, c[1].as_u64().unwrap_or(1) as u8);
        }
        w.column = num("column")? as i8;
        w.queue_len = num("queue")? as i32;
        t.blink = num("blink")? as i32;
        (t.active, t.paused) = (num("active")? != 0.0, num("paused")? != 0.0);
        (t.gates.text_shown, t.gates.text_window) = (num("shown")? != 0.0, num("disabled")? != 0.0);
        Ok(TutorialUi { t, mou: Vec::new(), ms: num("stamp")? as u32, counter: 0, kep: 2, overrun: None, ai_off: None, ticked: true })
    }

    /// `TutorialKeretFrissit` and `TutorialSzovegFrissit` (re/rules/tutorial.md §2 "Drawing"): the window's box,
    /// its 8 × 40 cells in font 2 (colour ≥ 0x40 blinks: `KiirStrSzinSzorzo = TutorialSzinVillogtato`) and the
    /// typing cursor. Nothing on the options screen, without the window or while it is hidden (`t`).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, f: &mut Frame, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], screen: i32) {
        let (t, w) = (&self.t, &self.t.window);
        if screen == SCREEN_OPTIONS || !self.running() || t.gates.text_window || !t.gates.text_shown {
            return;
        }
        let (cw, ch) = (font.advance(2), font.height(2));
        // `FadeKerettelKerekit(x − 5, y − 5, min(CharX·40 + 10, 0x27f − x), (CharY + 3)·8 + 10, (0, 0x36, 0x5a) at
        // 0x1e, (0, 0x5a, 0x82) at 0)`.
        let (x, y) = ((w.x - 5.0) as i32, (w.y - 5.0) as i32);
        let bw = (cw as i32 * COLS as i32 + 10).min(0x27f - x);
        let bh = (ch as i32 + 3) * ROWS as i32 + 10;
        crate::kit::fader::fader(f, white, v, [x as f32, y as f32, bw as f32, bh as f32], [0.0, 54.0, 90.0], 30.0, [0.0, 90.0, 130.0], 0.0);
        let colour = |c: u8| {
            let (k, s) = if c >= 0x40 { (c - 0x40, t.blink as f32 / 100.0) } else { (c, 1.0) };
            palette.get(k as usize).copied().unwrap_or([1.0; 3]).map(|v| v * s)
        };
        for (r, row) in w.grid.iter().enumerate() {
            for (c, &(b, k)) in row.iter().enumerate() {
                if b != b' ' {
                    let px = (w.x + (c as i32 * cw as i32) as f32) as i32 as f32;
                    let py = (w.y + (r as i32 * (ch as i32 + 3)) as f32) as i32 as f32;
                    font.text(f, v, &[b], px, py, 1, 2, Align::Left, colour(k), None);
                }
            }
        }
        // The cursor block on row 7: blinking while idle, steady while a word is typed; not while paused.
        let col = w.column.max(0) as usize;
        let typing = w.queue_len != 0 && col > 0 && w.grid[ROWS - 1][col - 1].0 != b' ';
        let idle = w.queue_len == 0 && self.ms % 400 < 200;
        if (idle || typing) && !t.paused {
            let cx = (w.x + (col as i32 * cw as i32) as f32) as i32 as f32;
            let cy = (w.y + ((ROWS as i32 - 1) * (ch as i32 + 3)) as f32) as i32 as f32;
            // `FillToScreen(x, y, CharX, CharY, 8, false)`: opaque in colour 8 on whole pixels, the corner rounded up
            // and the size truncated [obs: 7 × 8 pixels at (554, 363) for the cell (443, 290) at 800 × 600].
            let c = palette.get(8).copied().unwrap_or([1.0; 3]);
            let [px, py] = v.px(cx, cy).map(f32::ceil);
            let (pw, ph) = ((cw * v.k).trunc(), (ch * v.k).trunc());
            let i = crate::render::Instance::new(crate::render::kind::SPRITE, [px + pw / 2.0, py + ph / 2.0], [pw / 2.0, ph / 2.0], [c[0], c[1], c[2], 1.0]);
            f.push(white, crate::render::Blend::GammaAlpha, i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exe's window as `tools/trace/probes/tutorial.js` logged it (`run_screen.py quick-1.sav --kep 2 --script
    /// scripts/tutorial.json --tutorial 1`, Hungarian): chapter 1 started over the starmap, the window at t = 400,
    /// 2000 and 4000 UI ticks. The client's driver (the script read from the install, `Tutorial` on the UI tick, the
    /// starmap's screen id) gives the same window: position, grid, typing column and queue, blink. Skips without the
    /// install or the fixture.
    #[test]
    fn chapter_1_window_matches_the_exe() {
        let (Ok(root), Ok(dir)) = (std::env::var("IG2_INSTALL_DIR"), std::env::var("IG2_TRACE_DIR")) else { return };
        let Ok(lines) = std::fs::read_to_string(std::path::Path::new(&dir).join("tutorial-overlay/tutwin-grabs.jsonl")) else { return };
        let install = Install::open(root, ig2_formats::text::Lang::Hu).unwrap();
        let i = Inputs { screen: 2, ..Default::default() };
        let (mut ui, _) = TutorialUi::start(&install, 1, i, None).unwrap();
        let mut t = 0;
        for line in lines.lines() {
            let want = TutorialUi::from_record(line).unwrap();
            let at = serde_json::from_str::<serde_json::Value>(line).unwrap()["t"].as_i64().unwrap();
            while t < at {
                t += 1;
                ui.replay_step(2);
                // The capture's save load enters the starmap: its transition holds `WAITFORATLEPES` until the tick the
                // first `TYPE` ran [obs: queue 0 at t 101, 51 at t 102].
                ui.tick(&install, Inputs { transition: t < 102, ..i }, None);
            }
            let (w, e) = (&ui.t.window, &want.t.window);
            assert_eq!((w.x, w.y, w.column, w.queue_len), (e.x, e.y, e.column, e.queue_len), "t {t}");
            assert_eq!(w.grid, e.grid, "t {t}");
            assert_eq!((ui.t.blink, ui.t.active, ui.t.paused), (want.t.blink, want.t.active, want.t.paused), "t {t}");
        }
    }

    /// `CountMyTanks()` at each chapter's first traced tick in the exe (`traces/tutorial-ch1..5.jsonl`, `tanks`,
    /// 2026-10-22): 1, 2, 2, 26, 26 (chapters 4 and 5: 2 fleet entries and 24 on the battlefields).
    #[test]
    fn count_my_tanks_matches_the_exe_at_each_chapter_start() {
        let Some(Ok(install)) = ig2_formats::Install::from_env(ig2_formats::text::Lang::En) else { return };
        let (rules, _) = crate::rules::load(&install).unwrap();
        let mut got = Vec::new();
        for n in 1..=5 {
            let w = crate::world::World::new_game(crate::new_game::tutorial(&install, n, 4242, &rules).unwrap());
            let s = &w.game.state;
            got.push(Inputs::default().counts(s).tanks + field_tanks(s));
        }
        assert_eq!(got, [1, 2, 2, 26, 26]);
    }

    #[test]
    fn parked_pointer_without_a_recording() {
        let mut ui = TutorialUi { t: Tutorial::default(), mou: Vec::new(), ms: 0, counter: 0, kep: 2, overrun: None, ai_off: None, ticked: false };
        assert_eq!(ui.replay_step(2), None);
        ui.t.active = true;
        ui.t.mouse = -1;
        let r = ui.replay_step(2).unwrap();
        assert_eq!((r.at, r.parked), (PARKED, true));
        // The options screen keeps the live input.
        assert_eq!(ui.replay_step(SCREEN_OPTIONS), None);
    }
}
