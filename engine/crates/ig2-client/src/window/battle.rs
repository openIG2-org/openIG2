//! The real-time space battle in the window: entering, its update and output, leaving and its keys.

use super::*;

impl App {
    /// The battle screen's part of the frame's update (re/screens/spacewar.md): a started battle opens it, an ended one
    /// zooms out, then [`crate::spacewar_live::Live::update`].
    pub(super) fn spacewar_update(&mut self, now: u32) {
        use ig2_sim::game::RealTimeEvent;
        for e in std::mem::take(&mut self.world.realtime) {
            match e {
                RealTimeEvent::SpaceStarted { slot } if self.spacewar.is_none() && self.spacewar_parked.is_none() => {
                    self.enter_space_battle(slot, now);
                }
                // `StartGrWar` (exe 0x4a4a70): 900 when the field is contested (`WF::GetStatus` −1 or −10), any planet;
                // it also ends the tutorial's `WAITFORBATTLE` (re/rules/tutorial.md §1).
                RealTimeEvent::GroundStarted { planet, holder } => {
                    if let Some(t) = &mut self.tutorial {
                        t.t.battle_started();
                    }
                    if holder.is_none() {
                        match &mut self.colony {
                            Some(c) => c.ui.war.banner = 900,
                            None => self.attack_banner = 900,
                        }
                        // SP, the human takes part (the sim paused it at speed 1), or MP (every machine, ELF 0x4a4f80):
                        // the war's colony screen; the old screen and planet are kept.
                        if self.multiplayer() || self.world.game.sp_battles.old_pause.is_some() {
                            self.grwar_old = Some((self.shown_kep(), self.akt_planet()));
                            self.show_colony_screen(planet);
                        }
                    }
                }
                // `GroundWarFinished(p, s, false)`: on the war's colony screen, back to the old screen (the battle
                // screen's 0xB becomes the starmap) and its pause.
                RealTimeEvent::GroundEnded { planet, .. } => {
                    let shown = self.shown_kep() == 3 && self.world.colony == Some(planet);
                    if let Some((kep, p)) = self.grwar_old.take().filter(|_| shown) {
                        let kep = if kep == 0xb { 2 } else { kep };
                        if kep == 3 {
                            self.show_colony_screen(p);
                        } else {
                            self.request_screen(kep);
                        }
                    }
                }
                RealTimeEvent::SpaceOver { slot, .. } => {
                    // Under the options screen (MP: the time runs on): the battle ends without its screen.
                    if self.spacewar_parked.as_ref().is_some_and(|sw| sw.ui.slot == slot) {
                        self.spacewar_parked = None;
                        self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game));
                    }
                    if let Some(sw) = self.spacewar.as_mut().filter(|sw| sw.ui.slot == slot) {
                        sw.over(&self.world, now);
                        // MP: `SetIdoPause` changes nothing (`leave_battle`); sent, it would pause every machine.
                        if !self.multiplayer() {
                            self.send(StarmapOrder::Pause { paused: true });
                        }
                    }
                }
                _ => {}
            }
        }
        self.mp_battle_check(now);
        self.resume_parked_battle();
        let Some(sw) = &mut self.spacewar else { return };
        sw.pointer = self.cursor_px;
        if let Some(g) = &self.gpu {
            let size = g.window.inner_size();
            sw.window = [size.width as i32, size.height as i32];
        }
        let out = sw.update(&mut self.world, now);
        self.spacewar_out(out);
    }

    /// `SpaceWar_Belep`: the battle screen for `slot`, always entered from the starmap.
    pub(super) fn enter_space_battle(&mut self, slot: usize, now: u32) {
        let Some(cam) = self.view.as_ref().map(|v| v.camera) else { return };
        self.colony = None;
        self.world.colony = None;
        self.overlay = None;
        self.spy = None;
        if let Some(g) = &mut self.gpu {
            g.assets.planet_meshes.load_battle(&mut g.renderer, &self.install);
        }
        // `SpaceWar_Belep` exe 0x5628ac: `PlayMusicSpWar(1, 0)` (crossfade) when no battle ran yet.
        self.audio.play_music(crate::audio::Music::SpWar, false, human_race(&self.world.game));
        self.spacewar = crate::spacewar_live::Live::enter(&mut self.world, slot, &cam, &self.install, now);
    }

    /// Multiplayer: every machine's strategic clock waits until each human has left the running battle's screen
    /// (0x44d). A machine without that screen up (a rejoin, a resumed save, a missed start, a battle decided under
    /// Options) opens it, or leaves at once when the battle is decided or the screen can't open.
    pub(super) fn mp_battle_check(&mut self, now: u32) {
        let waits = self.world.game.space_battle_waits_on(self.world.game.local_view().empire);
        if waits.is_none() {
            self.mp_left_sent = false;
        }
        let Some(slot) = waits.filter(|_| self.multiplayer() && !self.mp_left_sent) else { return };
        if self.spacewar.is_some() || self.spacewar_parked.is_some() || self.view.is_none() {
            return;
        }
        if self.world.game.realtime.slots.get(slot).is_some_and(|b| b.active && !b.ended) {
            self.enter_space_battle(slot, now);
        }
        if self.spacewar.is_none() {
            self.world.send(Command::SpaceBattleLeft);
            self.mp_left_sent = true;
        }
    }

    /// `AktKepKell = to` on the battle screen, once its opening zoom is over: `SpaceWar_Kilep`'s phase 1 (exe 0x562b50),
    /// the zoom-out to the starmap (five times faster towards Options), the tutorial's 0x20 (not towards Options, Help
    /// or the messages screen), `nemlehetidotallitani = 1; SetIdoPause(1, 0)` (ignored in MP).
    pub(super) fn leave_battle(&mut self, to: i32) {
        if !self.spacewar.as_ref().is_some_and(|sw| sw.clock_free()) {
            return;
        }
        if !matches!(to, 0xd | 1 | 10) {
            self.world.game.tutorial_check_event(0x20);
        }
        let now = self.now_ms();
        if let Some(sw) = &mut self.spacewar {
            if to == 0xd {
                sw.leave_for_options(&self.world, now);
            } else {
                sw.leave_for_screen(&self.world, now, to);
            }
        }
        if !self.multiplayer() {
            self.send(StarmapOrder::Pause { paused: true });
        }
    }

    /// `FUN_00594a40` (exe, from `Fake_FRISSITES` 0x4de788, only while `AktKep == 2`): a battle left running
    /// (`spacewarbanvanbarkacs` 0xf176ee) is asked for again at once (`AktKepKell = 0xb`), whatever screen was asked
    /// for before; `SpaceWar_Belep` resumes it with its opening zoom.
    pub(super) fn resume_parked_battle(&mut self) {
        if self.spacewar_parked.is_none() || self.kep_kell.is_some() || self.screen_changing() || self.akt_kep_now() != 2 {
            return;
        }
        if let Some(mut sw) = self.spacewar_parked.take() {
            sw.reenter(&mut self.world);
            self.spacewar = Some(sw);
        }
    }

    /// The battle screen's weapon loops stop with it (`lovedeksoundend`, `unloadgfx`'s `lovedekfreesound`).
    pub(super) fn stop_battle_loops(&mut self) {
        if let Some(sw) = &mut self.spacewar {
            for (_, ch) in sw.loops.drain() {
                self.audio.stop(ch);
            }
        }
    }

    /// What the battle screen asked for.
    pub(super) fn spacewar_out(&mut self, out: Vec<crate::spacewar_ui::Out>) {
        use crate::spacewar_ui::Out;
        for o in out {
            let Some(sw) = &self.spacewar else { return };
            // Towards Options a running battle stays: `SpaceWar_Kilep` (exe 0x562b50) returns before `SpaceWarVege` /
            // `Multi_Send(0x44d)`.
            if !(o == Out::Left && (sw.to_options || sw.to_screen.is_some()) && !sw.ended) {
                for c in sw.commands(&o) {
                    self.world.send(c);
                }
                self.mp_left_sent |= o == Out::Left;
            }
            match o {
                Out::Click => self.sounds.push(crate::audio::CLICK),
                Out::Sound(s) => {
                    self.audio.set_record_volume(s.id, s.volume);
                    self.sounds.push(s.id);
                }
                Out::Loop(l) => {
                    use crate::spacewar_fx::Loop;
                    let Some(sw) = &mut self.spacewar else { return };
                    match l {
                        Loop::Start { key, id } => {
                            if let Some(ch) = self.audio.start_loop(id) {
                                sw.loops.insert(key, ch);
                            }
                        }
                        Loop::Volume { key, volume } => {
                            if let Some(&ch) = sw.loops.get(&key) {
                                self.audio.set_channel_volume(ch, volume);
                            }
                        }
                        Loop::End { key } => {
                            if let Some(ch) = sw.loops.remove(&key) {
                                self.audio.stop(ch);
                            }
                        }
                    }
                }
                Out::Left if self.spacewar.as_ref().is_some_and(|sw| sw.to_screen.is_some() && !sw.ended) => {
                    // `SpaceWar_Kilep`'s phase 2 towards another screen (exe 0x562b50): the tutorial's 0x20 again (not
                    // towards Help or the messages screen), `starmaphovaleptovabb = target`, the battle parked (no
                    // `SpaceWarVege`, `SetIdoPause(1, 0)`) and the starmap entered, which asks for the battle again
                    // at once (`resume_parked_battle`): the target is dropped.
                    if self.spacewar.as_ref().and_then(|sw| sw.to_screen).is_some_and(|to| !matches!(to, 1 | 10)) {
                        self.world.game.tutorial_check_event(0x20);
                    }
                    self.world.game.sp_battles.on_screen = false;
                    self.stop_battle_loops();
                    self.spacewar_parked = self.spacewar.take();
                    if let Some(v) = &mut self.view {
                        crate::spacewar_live::restore_view(v);
                    }
                }
                Out::Left if self.spacewar.as_ref().is_some_and(|sw| sw.to_options && !sw.ended) => {
                    // `SpaceWar_Kilep`'s phase 2 towards Options (exe 0x562c0f): `GFX_Starmap(); SaveScreen(2);
                    // OptionsHarcFolyik = 1`; no tutorial event 0x20 (not towards 0xd), the battle stays.
                    self.world.game.sp_battles.on_screen = false;
                    self.stop_battle_loops();
                    self.spacewar_parked = self.spacewar.take();
                    if let Some(v) = &mut self.view {
                        crate::spacewar_live::restore_view(v);
                    }
                    self.open_options(false);
                }
                Out::Left => {
                    // `SpaceWar_Kilep`'s first statement (exe 0x562b68): the tutorial's 0x20 (not towards the options,
                    // Help or the trade screen).
                    self.world.game.tutorial_check_event(0x20);
                    // `SpaceWar_Kilep`'s second phase: a finished battle ends (`SpaceWarVege`); one still running stays.
                    self.world.game.sp_battles.on_screen = false;
                    self.stop_battle_loops();
                    // `Starmap_Belep` exe 0x5b1d74: `PlayMusicMain(0, 0)` unless a battle still runs.
                    if self.spacewar.as_ref().is_some_and(|sw| sw.ended) {
                        self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game));
                    }
                    // A battle over during the zoom-out towards Options (MP: the time runs on): phase 2 still takes
                    // the 0xd branch (`OptionsHarcFolyik = 1`) before its end, so Options opens.
                    let to_options = self.spacewar.as_ref().is_some_and(|sw| sw.to_options);
                    self.spacewar = None;
                    if let Some(v) = &mut self.view {
                        crate::spacewar_live::restore_view(v);
                    }
                    if to_options {
                        self.open_options(false);
                        if let Some(o) = &mut self.options {
                            o.s.from_battle = true;
                            o.s.battle = true;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// The battle screen's frame: its input and its picture.
    pub(super) fn spacewar_frame(&mut self, now: u32, alpha: f32, size: [f32; 2], t: f32, globe: Option<crate::render::TexId>) -> Option<crate::render::Frame> {
        let sw = self.spacewar.as_mut()?;
        let (g, v, h) = (self.gpu.as_mut()?, self.view.as_mut()?, self.hud.as_ref()?);
        let mouse = v.camera.virtual_of(self.cursor);
        let (frame, out) = sw.frame(&self.world, &mut g.renderer, &self.install, &g.assets, v, h, self.tables, mouse, self.modifiers.shift_key(), now, alpha, t, size, globe);
        self.spacewar_out(out);
        frame
    }

    /// `SpaceWarKeyboardHandler` (spec §7.1.5), with the shared keys it leaves to `DoKeyboard`: space pauses, Esc
    /// leaves the screen (`SpaceWar_Kilep`'s zoom-out) [u: `DoKeyboard` not read].
    pub(super) fn spacewar_key(&mut self, event: &winit::event::KeyEvent) {
        self.spacewar_key_by(event.logical_key.as_ref(), event.physical_key, event.state == ElementState::Pressed);
    }

    /// [`Self::spacewar_key`] by its parts: a live key, or one the tutorial's recording plays (`ReadNext`).
    pub(super) fn spacewar_key_by(&mut self, logical: Key<&str>, physical: winit::keyboard::PhysicalKey, down: bool) {
        use crate::spacewar_ui::Key as K;
        use winit::keyboard::{KeyCode, PhysicalKey};
        // MP: `DoKeyboard`'s chat entry (input.md §5; `AktKep` 0xb < 0xd) opens and takes the keys here too.
        if down && self.chat_key(&logical) {
            return;
        }
        // `DoKeyboard` (re/screens/input.md §1) first: the top row's 1..3 set the speed and unpause (sound 10) and
        // never reach the screen, Space and P pause, O and Esc (`ESC_Pressed`) open the options; the pause and speed
        // keys wait for the opening zoom (`nemlehetidotallitani`).
        let ch = match logical {
            Key::Character(c) => c.chars().next().map(|c| c.to_ascii_lowercase()),
            _ => None,
        };
        let digit = match physical {
            PhysicalKey::Code(c) if (KeyCode::Digit1..=KeyCode::Digit9).contains(&c) => Some(c as i32 - KeyCode::Digit0 as i32),
            _ => None,
        };
        if down {
            let free = self.multiplayer() || self.spacewar.as_ref().is_some_and(|sw| sw.clock_free());
            if matches!(logical, Key::Named(NamedKey::Space)) || ch == Some('p') {
                if free {
                    self.toggle_pause();
                }
                return;
            }
            if let Some(n @ 1..=3) = digit {
                if self.multiplayer() {
                    self.set_speed(n);
                } else if free && !self.speed_locked() {
                    self.sounds.push(crate::audio::SoundId::global(10));
                    self.send(StarmapOrder::Speed { level: n });
                    self.send(StarmapOrder::Pause { paused: false });
                }
                return;
            }
            // Every key repeats while held (`MainCallBackEventHandler` exe 0x4e2db0 pushes the OS repeats into the
            // key ring: no repeat test); the zoom-out holds the repeats off (`nemlehetidotallitani`).
            if matches!(logical, Key::Named(NamedKey::Escape)) || ch == Some('o') {
                self.leave_battle(0xd);
                return;
            }
        }
        // `SpaceWarKeyboardHandler`: the letters by the typed character (the char events), the rest by key code.
        let k = match (logical, ch, digit) {
            (Key::Named(NamedKey::ArrowLeft), ..) => Some(K::Left),
            (Key::Named(NamedKey::ArrowRight), ..) => Some(K::Right),
            (Key::Named(NamedKey::ArrowUp), ..) => Some(K::Up),
            (Key::Named(NamedKey::ArrowDown), ..) => Some(K::Down),
            _ if physical == PhysicalKey::Code(KeyCode::NumpadAdd) => Some(K::ZoomIn),
            _ if physical == PhysicalKey::Code(KeyCode::NumpadSubtract) => Some(K::ZoomOut),
            (_, Some('a'), _) => Some(K::All),
            (_, Some('c'), _) => Some(K::Camera),
            (_, Some('r'), _) => Some(K::Retreat),
            (_, _, Some(n)) => Some(K::Group(n - 1)),
            (Key::Named(NamedKey::Backspace), ..) => Some(K::Explode),
            _ => None,
        };
        if let Some(k) = k {
            let debug = self.world.game.state.mem.u8(ig2_sim::save::va::DEBUGX) != 0;
            let ctrl = self.modifiers.control_key();
            let mut out = Vec::new();
            if let Some(sw) = &mut self.spacewar {
                if let Some(b) = self.world.game.realtime.slots.get(sw.ui.slot) {
                    let tp = self.world.game.state.mem.u8(ig2_sim::save::va::TUTORIAL_PAUSED) != 0;
                    sw.ui.key(b, k, down, ctrl, debug, tp, &mut out);
                }
            }
            self.spacewar_out(out);
        }
    }
}
