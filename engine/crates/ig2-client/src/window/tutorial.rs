//! The tutorial's hold on the window: its screen, input locks, recorded input and effects.

use super::*;

impl App {
    /// `TutorialActive` (exe 0xf6915d): the script plays (its recording drives the input), not waiting for the player.
    pub(super) fn tutorial_playing(&self) -> bool {
        self.tutorial.as_ref().is_some_and(|t| t.t.active)
    }

    /// `KepernyokLetiltva[n]` (exe 0xf66f2c + n): the tutorial has turned screen `n` off.
    pub(super) fn tutorial_screen_off(&self, n: usize) -> bool {
        self.tutorial.as_ref().is_some_and(|t| t.t.gates.screens.get(n).copied().unwrap_or(false))
    }

    /// `ExitToMainScreen` (exe 0x5117f0), then `AktKepKell = 0xF`: `MainScr_Belep` (the rings fly in again, the menu
    /// music). The world stays as it is until the next game replaces it.
    /// `AktKep` as the tutorial reads it: the help screen 1, the spy screen 7 (up over the screen it came from), else
    /// the overlay's or the base screen's.
    pub(super) fn tutorial_kep(&self) -> i32 {
        if self.spy.is_some() && self.help.is_none() && self.spacewar.is_none() {
            return 7;
        }
        akt_kep(if self.help.is_some() { Some(crate::ui::Screen::Help) } else { self.overlay }, self.colony.is_some(), self.spacewar.is_some())
    }

    pub(super) fn tutorial_owns_input(&self) -> bool {
        self.options.is_none() && self.tutorial.as_ref().is_some_and(|t| t.owns_input(self.tutorial_kep()))
    }

    pub(super) fn tutorial_inputs(&self) -> crate::tutorial_ui::Inputs {
        let counter = self.tutorial.as_ref().map_or(0, |t| t.counter);
        let screen = self.tutorial_kep();
        // `Atlepes || Belep` (`WAITFORATLEPES`): the whole change runs inside one `IG2_gameUpdate` (exe 0x4e0440), so the
        // ticks that see them are the old screen's closing loop (`*_Kilep`: research, design, trade, spy, diplomacy,
        // messages; 84 ticks in the chapter 3/5 traces) and the tick the screen changes.
        let transition = self.tutorial.as_ref().is_some_and(|t| t.screen() != screen) || self.screen_changing();
        crate::tutorial_ui::Inputs { screen, counter, transition, ..Default::default() }.counts(&self.world.game.state)
    }

    /// `TutorialSetCampaignGameOptions` has made the game; `TutorialStart("main", n)` (exe 0x611f80).
    pub(super) fn start_tutorial(&mut self, chapter: i32) {
        let inputs = self.tutorial_inputs();
        match crate::tutorial_ui::TutorialUi::start(&self.install, chapter, inputs, Some(&self.world.game.state)) {
            Ok((mut t, effects)) => {
                t.ai_off = Some(self.world.game.state.mem.i32(ig2_sim::save::va::AI_OFF));
                t.push(&mut self.world.game.state.mem);
                self.tutorial = Some(t);
                self.world.drop_replay();
                (self.tutorial_at, self.tutorial_buttons) = (crate::tutorial_ui::PARKED, (false, false));
                self.tutorial_effects(effects);
                // `GlobalMusicVolumeCsokkento` (after `TutorialStart`'s own `TutorialStop(true)`): the music ducked while
                // the tutorial runs. `ExitToMainScreen` clears Active/Paused without `TutorialStop`, so the duck stays.
                self.audio.set_duck(crate::audio::tutorial_duck(self.audio.music_volume));
            }
            Err(e) => eprintln!("ig2-client: tutorial {chapter}: {e}"),
        }
    }

    /// One UI tick of the tutorial: the recorded input (`Mouse_Tutorial_Kezel`, `ReadNext`) goes to the screens, an
    /// event the game raised resumes the script, then `TutorialFrissit` and its effects.
    pub(super) fn tutorial_tick(&mut self) {
        let kep = self.tutorial_kep();
        let replay = if self.options.is_some() { None } else { self.tutorial.as_mut().and_then(|t| t.replay_step(kep)) };
        match replay {
            Some(r) => self.tutorial_replay(r),
            // The live input takes over (`Mouse_Lekerdez` reads the live buttons again): a recorded button held is let go.
            None => {
                let (left, right) = std::mem::take(&mut self.tutorial_buttons);
                if left {
                    self.mouse(MouseButton::Left, false);
                }
                if right {
                    self.mouse(MouseButton::Right, false);
                }
            }
        }
        let inputs = self.tutorial_inputs();
        let now = self.now_ms();
        let routed = std::mem::take(&mut self.world.ground_war_routed);
        let Some(t) = &mut self.tutorial else { return };
        // `StartGrWar` (exe 0x4a4a70) clears `TutorialWaitForBattle` first thing; single player's real-time ground
        // war has no `StartGrWar` of its own here, so the routing's real-time branch stands for it.
        if routed {
            t.t.battle_started();
        }
        t.entered(kep, &mut self.world.game);
        t.pull(&self.world.game.state.mem);
        t.ms = now;
        let effects = t.tick(&self.install, inputs, Some(&self.world.game.state));
        t.push(&mut self.world.game.state.mem);
        let g = &t.t.gates;
        self.panels.locks = crate::starmap_panels::TutorialLocks { production: g.production, exit: g.exit_button, split: g.split };
        if let Some(c) = &mut self.colony {
            c.ui.locked = g.buildings.to_vec();
            (c.ui.no_demolish, c.ui.all_off, c.ui.starmap_off) = (g.demolish, g.colony, g.screens[2]);
            c.ui.war.locked = g.colony;
        }
        self.tutorial_effects(effects);
    }

    /// The recording's pointer, buttons and keys as live input.
    pub(super) fn tutorial_replay(&mut self, r: crate::tutorial_ui::Replay) {
        self.tutorial_kind = if r.parked { -2 } else { 0 };
        if r.at != self.tutorial_at {
            self.tutorial_at = r.at;
            let (bw, bh) = self.gpu.as_ref().map_or((640, 480), |g| g.band());
            let p = crate::font::Virtual::new([bw as f32, bh as f32]).px(r.at[0] as f32, r.at[1] as f32);
            self.cursor_moved(p);
        }
        // `KeyAlt`, `KeyShift`, `KeyCtrl` from the recording (1, 2, 4).
        let mut m = winit::keyboard::ModifiersState::empty();
        m.set(winit::keyboard::ModifiersState::ALT, r.modifiers & 1 != 0);
        m.set(winit::keyboard::ModifiersState::SHIFT, r.modifiers & 2 != 0);
        m.set(winit::keyboard::ModifiersState::CONTROL, r.modifiers & 4 != 0);
        self.modifiers = m;
        let (left, right) = self.tutorial_buttons;
        self.tutorial_buttons = (r.left, r.right);
        if r.left != left {
            self.mouse(MouseButton::Left, r.left);
        }
        if r.right != right {
            self.mouse(MouseButton::Right, r.right);
        }
        for (kind, code) in r.keys {
            // On the battle screen the recorded keys go to its handler as live ones do (`SpaceWarKeyboardHandler`
            // behind `DoKeyboard`): Space by its code, the top row's digits and the numpad's + / − by their keys.
            if self.spacewar.is_some() {
                use winit::keyboard::{KeyCode, NativeKeyCode, PhysicalKey};
                let physical = match (kind, code) {
                    (0, d @ 0x31..=0x39) => PhysicalKey::Code([KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9][(d - 0x31) as usize]),
                    (1 | 2, 0x6b) => PhysicalKey::Code(KeyCode::NumpadAdd),
                    (1 | 2, 0x6d) => PhysicalKey::Code(KeyCode::NumpadSubtract),
                    _ => PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
                };
                let logical = match (kind, code) {
                    (0, 0x20) => Some(Key::Named(NamedKey::Space)),
                    (1 | 2, 0x6b) => Some(Key::Character("+".into())),
                    (1 | 2, 0x6d) => Some(Key::Character("-".into())),
                    (2, c) => recorded_key(1, c),
                    _ => recorded_key(kind, code),
                };
                if let Some(k) = logical {
                    self.spacewar_key_by(k.as_ref(), physical, kind != 2);
                }
                continue;
            }
            // The held keys (arrows, numpad + / −) go down and up through the screens' held-key handlers, as live
            // ones do; the other key-ups have no reader.
            if matches!(kind, 1 | 2) {
                use winit::keyboard::{KeyCode, PhysicalKey};
                let held = match code {
                    0x25 => Some((KeyCode::ArrowLeft, Key::Named(NamedKey::ArrowLeft))),
                    0x26 => Some((KeyCode::ArrowUp, Key::Named(NamedKey::ArrowUp))),
                    0x27 => Some((KeyCode::ArrowRight, Key::Named(NamedKey::ArrowRight))),
                    0x28 => Some((KeyCode::ArrowDown, Key::Named(NamedKey::ArrowDown))),
                    0x6b => Some((KeyCode::NumpadAdd, Key::Character("+".into()))),
                    0x6d => Some((KeyCode::NumpadSubtract, Key::Character("-".into()))),
                    _ => None,
                };
                if let Some((pk, logical)) = held {
                    let (pk, down) = (PhysicalKey::Code(pk), kind == 1);
                    if self.colony_camera_key_by(pk, logical.as_ref(), down) || self.trade_key_by(pk, logical.as_ref(), down) || self.starmap_key_by(pk, logical.as_ref(), down) {
                        continue;
                    }
                }
            }
            if let Some(k) = recorded_key(kind, code) {
                self.key(k.as_ref());
            }
            // `oldEnterlenyomva` (`research_key_seen`): a key up, or a down the research screen has no key for.
            if let Some(r) = self.research.as_mut().filter(|_| self.overlay == Some(crate::ui::Screen::Research)) {
                if kind == 2 || !matches!(recorded_key(kind, code), Some(Key::Named(n)) if research_key(n).is_some()) {
                    r.key_seen(kind != 2);
                }
            }
        }
    }

    /// What the script does outside the interpreter (re/rules/tutorial.md §3): the game's in-place writes, the
    /// starmap's selection, the clock, leaving.
    pub(super) fn tutorial_effects(&mut self, effects: Vec<ig2_sim::tutorial::Effect>) {
        use ig2_sim::tutorial::Effect as E;
        // `IdoPause` as the tick's commands leave it: the pause orders reach the game at the next pump, so a
        // `GAMESPEED 1` then `PAUSE` in one tick (chapter 4's battle) must see the first one's unpause.
        let mut paused = self.world.paused();
        for e in effects {
            if self.world.game.tutorial_effect(&e) {
                // The new fleet of EN is selected and centred (`AktFleet`, `StarmapMode = 2`, `KellSelect`).
                let en = self.world.game.state.mem.i32(ig2_sim::save::va::EN);
                match e {
                    E::KillerFleet(x) if x == en => self.tutorial_centre(&E::FindFleet(b"Terminator".to_vec())),
                    E::ColonizationFleet => self.tutorial_centre(&E::FindFleet(b"Colonization".to_vec())),
                    _ => {}
                }
                continue;
            }
            match e {
                E::ReturnToMainMenu => self.exit_to_main_menu(),
                E::Quit => {
                    self.exit_to_main_menu();
                    if let Some(f) = &mut self.front {
                        f.quit = true;
                    }
                }
                E::End => self.world.local_lines.push(b"Tutorial END.".to_vec()),
                // `DISABLE OVERRUN` (re/rules/tutorial.md §3 12): the four battle debug flags saved (a second one saves the
                // changed values, as in the exe), then 0, 0, 1, 1; `TutorialStop` puts them back.
                E::DisableOverrun => {
                    let m = &mut self.world.game.state.mem;
                    let saved = std::array::from_fn(|i| m.i32(0xB77230 + 4 * i as u32));
                    for (i, v) in [0, 0, 1, 1].into_iter().enumerate() {
                        m.set_i32(0xB77230 + 4 * i as u32, v);
                    }
                    if let Some(t) = &mut self.tutorial {
                        t.overrun = Some(saved);
                    }
                }
                E::Stopped { restore_overrun, restore_music } => {
                    if restore_music {
                        self.audio.set_duck(0);
                    }
                    if let Some(v) = self.tutorial.as_mut().and_then(|t| t.overrun.take()).filter(|_| restore_overrun) {
                        for (i, x) in v.into_iter().enumerate() {
                            self.world.game.state.mem.set_i32(0xB77230 + 4 * i as u32, x);
                        }
                    }
                }
                // `DOUBLEGOMB ON`: `ResearchDoubleGomb()` from any screen (the click and message 0x2b; the button's look
                // only shows on the research screen, which reads `Double_Lenyomva` as it opens).
                E::DoubleButton => match &mut self.research {
                    Some(r) => {
                        for a in r.double() {
                            let crate::research::Act::Send(c) = a;
                            self.world.send(c);
                        }
                    }
                    None => {
                        self.audio.play(crate::audio::CLICK);
                        self.world.send(Command::Research { order: ig2_net::ResearchCmd::Urgent });
                    }
                },
                // `KURVAANYAD`: the colony's planet info (chapter 2 runs it on the colony screen).
                E::PlanetInfo => self.colony_input(|ui, c, _, _| ui.planet_info(c)),
                E::CenterFleet | E::FindFleet(_) | E::FindPlanet(_) | E::CenterPlanet | E::CenterUncolonisedPlanet => self.tutorial_centre(&e),
                E::GameSpeed(0) | E::Pause(true) => {
                    if !paused {
                        self.send(StarmapOrder::Pause { paused: true });
                        paused = true;
                    }
                }
                // `SetIdoSpeed(n, false)` (skipped while `nemlehetidotallitani`, no sound), then `SetIdoPause(false)`.
                E::GameSpeed(n) => {
                    if !self.speed_locked() && self.world.game.speed_level() != n {
                        self.send(StarmapOrder::Speed { level: n });
                    }
                    if paused {
                        self.send(StarmapOrder::Pause { paused: false });
                        paused = false;
                    }
                }
                E::Pause(false) => {
                    if paused {
                        self.send(StarmapOrder::Pause { paused: false });
                        paused = false;
                    }
                }
                // `GOTO SCREEN n` (`AktKepKell = n`): as the F-key of that screen (F1 = 1 .. F11 = 11) when another is up.
                E::GotoScreen(n) if (1..=11).contains(&n) => {
                    let now = self.tutorial_kep();
                    const F: [NamedKey; 11] = [
                        NamedKey::F1, NamedKey::F2, NamedKey::F3, NamedKey::F4, NamedKey::F5, NamedKey::F6, NamedKey::F7, NamedKey::F8,
                        NamedKey::F9, NamedKey::F10, NamedKey::F11,
                    ];
                    if now != n {
                        self.key(Key::Named(F[n as usize - 1]));
                    }
                }
                e => eprintln!("ig2-client: tutorial: {e:?} isn't carried out yet"),
            }
        }
    }

    /// `CENTERFLEET`, `FIND FLEET`, `FIND PLANET`, `CENTERPLANET`, `CENTERUNCOLONISEDPLANET`: select the first match
    /// and centre it (`StarmapKozepreAllit`).
    pub(super) fn tutorial_centre(&mut self, e: &ig2_sim::tutorial::Effect) {
        use ig2_sim::tutorial::Effect as E;
        let state = &self.world.game.state;
        let en = state.mem.i32(ig2_sim::save::va::EN);
        let name = |b: &[u8]| b.split(|&c| c == 0).next().unwrap_or_default().to_vec();
        let f32_at = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let fleet = |want: Option<&[u8]>| {
            state.fleets.iter().enumerate().skip(1).find(|(_, f)| f[0x15] as i32 == en && want.is_none_or(|w| name(&f[..0x15]) == w)).map(|(i, f)| (i, f32_at(f, 0x30), f32_at(f, 0x34)))
        };
        let planet = |test: &dyn Fn(&[u8]) -> bool| {
            (1..=state.planet_count()).find_map(|p| {
                let r = state.mem.read(ig2_sim::save::va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, ig2_sim::save::PLANET_SIZE);
                test(&r).then(|| (p, f32_at(&r, 0x18), f32_at(&r, 0x1c)))
            })
        };
        let k = 0xd8 + 2 * en.clamp(0, 18) as usize;
        let explored = |r: &[u8]| i16::from_le_bytes([r[k], r[k + 1]]) == 1;
        let cur = self.selection();
        let hit = match e {
            E::CenterFleet => fleet(None).map(|(i, x, y)| (Selection { mode: 2, fleet: i, ..cur }, x, y)),
            E::FindFleet(n) => fleet(Some(n)).map(|(i, x, y)| (Selection { mode: 2, fleet: i, ..cur }, x, y)),
            E::FindPlanet(n) => planet(&|r| r[0x20] as i32 == en && name(&r[..0x18]) == *n).map(|(p, x, y)| (Selection { mode: 1, planet: p, ..cur }, x, y)),
            E::CenterPlanet => planet(&|r| r[0x20] as i32 == en).map(|(p, x, y)| (Selection { mode: 1, planet: p, ..cur }, x, y)),
            E::CenterUncolonisedPlanet => planet(&|r| r[0x20] == 0 && explored(r)).map(|(p, x, y)| (Selection { mode: 1, planet: p, ..cur }, x, y)),
            _ => None,
        };
        if let Some((s, x, y)) = hit {
            self.apply(vec![Action::Select(s), Action::Centre { x, y }]);
        }
    }
}
