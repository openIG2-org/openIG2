//! Videos, the messages screen's talking head and the sound of a frame.

use super::*;

impl App {
    pub(super) fn audio_frame(&mut self, due_ui: u64) {
        use crate::audio::SoundId;
        use crate::ui::Screen;
        use ig2_rules::sounds::Table;
        // `Colony_Kilep` (exe 0x466390): `DeInitSounds` of `Colony_Villamok`, `Colony_Hangok` and `Gr_Hangok` stops
        // whatever of them still plays (the wind, the thunder, the war).
        if self.sound_colony && self.colony.is_none() {
            for t in [Table::Villam, Table::Colony, Table::Gr] {
                self.audio.deinit(t);
            }
        }
        self.sound_colony = self.colony.is_some();
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        // `UpdateFrame`: the start voice's ducking (the main menu and the start screens are past 0xd).
        self.audio.start_voice_frame(self.front.is_some());
        // `FRISSIT_Scroll2` (exe 0x556210): a message taken onto the line speaks, unless it is one of the four
        // silent ids; the brutal voice for empire 5 (`EN`), else the human one.
        let en = self.world.game.state.mem.i32(ig2_sim::save::va::EN);
        for (id, text) in std::mem::take(&mut self.world.popped) {
            if !matches!(id, 1 | 0x14 | 0x15 | 0x1a) {
                let voice = if en == 5 { "brutal" } else { "human" };
                let ch = self.audio.play_wav(&format!("sounds/scroll/{voice}/{id:04}.wav"));
                if let Some(ch) = ch.filter(|_| self.display.subtitles) {
                    self.subtitle = Some((ch, text));
                }
            }
        }
        if self.subtitle.as_ref().is_some_and(|(ch, _)| !self.display.subtitles || !self.audio.playing(*ch)) {
            self.subtitle = None;
        }
        if let Some(h) = &mut self.hud {
            h.subtitle = self.subtitle.as_ref().map(|(_, t)| t.clone());
        }
        // `WF::PlaySpeechGrWar` (exe 0x48c670) and the retreat 0x12f: only while the colony screen shows the war's
        // planet; the line and kind for this player's empire (ground-combat.md §11.12).
        let shown = self.world.colony.filter(|_| self.on_colony());
        for (p, v) in std::mem::take(&mut self.world.ground_voices) {
            if shown == Some(p) {
                let (line, kind) = v.line(en);
                self.speech.push(crate::audio::speech::Speak { who: crate::audio::speech::Who::Colony, line, kind });
            }
        }
        // `Gr_Hangok` and the flashes of the shown war (the sim gates them by the screen; another planet's are dropped).
        for (p, e) in std::mem::take(&mut self.world.ground_fx) {
            let Some(c) = self.colony.as_mut().filter(|_| shown == Some(p)) else { continue };
            match e {
                ig2_sim::ground::GroundEvent::Sound(i) => c.ui.sounds.push(crate::audio::SoundId::new(ig2_rules::sounds::Table::Gr, i as u16)),
                e => {
                    let counter = c.ui.war.counter();
                    c.ui.fx.event(&e, counter);
                }
            }
        }
        if let Some(c) = &mut self.colony {
            c.scene.take_fx(&mut c.ui.fx);
            c.ui.fx.step_layers(c.ui.war.counter());
        }
        if let Some(c) = &mut self.colony {
            for id in c.ui.sounds.drain(..) {
                self.audio.play(id);
            }
            for (line, kind) in c.ui.voices.drain(..) {
                self.speech.push(crate::audio::speech::Speak { who: crate::audio::speech::Who::Colony, line, kind });
            }
        }
        if let Some(sw) = &mut self.spacewar {
            for (line, kind) in sw.ui.voices.drain(..) {
                self.speech.push(crate::audio::speech::Speak { who: crate::audio::speech::Who::SpaceWar, line, kind });
            }
        }
        for s in std::mem::take(&mut self.speech) {
            self.audio.speak(s, en);
        }
        let on_starmap = self.on_starmap();
        let on_battle = self.spacewar.is_some();
        for _ in 0..due_ui {
            self.audio.speakers.tick(on_starmap);
            if on_battle {
                self.audio.speakers.tick_spacewar();
            }
        }
        if let Some(h) = &mut self.hud {
            for id in h.info.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        if let Some(d) = self.hud.as_mut().and_then(|h| h.messages.open.as_mut()) {
            for t in d.typing.drain(..) {
                self.audio.typing(t);
            }
        }
        if let Some(d) = &mut self.diplomacy {
            for id in d.screen.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        if let Some(s) = &mut self.spy {
            for id in s.screen.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        if let Some(r) = &mut self.research {
            for id in r.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        for id in self.panels.sounds.drain(..) {
            self.audio.play(id);
        }
        if let Some(d) = &mut self.design {
            for id in d.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        if let Some(t) = &mut self.trade {
            for id in t.sounds.drain(..) {
                self.audio.play(id);
            }
        }
        // `Messages_Belep`/`_Kilep` exe 0x507f90/0x5083d0 (the diplomacy screen queues its own: `Screen::enter`/`leave`).
        // `Messages_Kilep` (exe 0x5083d0) as it begins (before its closing): the typer and the speech stop, the ducking
        // ends, `scrn23ki.wav`; a screen gone some other way makes them as it goes.
        let kilep = self.hud.as_mut().is_some_and(|h| std::mem::take(&mut h.messages.kilep));
        let gone = self.overlay != self.sound_overlay && self.sound_overlay == Some(Screen::Messages);
        if kilep || (gone && !self.hud.as_ref().is_some_and(|h| h.messages.left)) {
            self.audio.stop_sound(SoundId::global(7));
            self.audio.stop_message_speech();
            self.audio.set_duck(0);
            self.audio.message.indult = false;
            self.audio.play(SoundId::new(Table::Mes, 1));
        }
        if self.overlay != self.sound_overlay {
            if self.overlay == Some(Screen::Messages) {
                self.audio.play(SoundId::new(Table::Mes, 0));
            }
            self.sound_overlay = self.overlay;
        }
        // `Spy_Belep` exe 0x58cd50, `Spy_Kilep` 0x58d250 (when the closing starts).
        let spy = self.spy.as_ref().map(|s| s.screen.closing);
        match (self.sound_spy, spy) {
            (None, Some(_)) => self.audio.play(SoundId::new(Table::Spy, 0)),
            (Some(false), Some(true)) => self.audio.play(SoundId::new(Table::Spy, 2)),
            _ => {}
        }
        self.sound_spy = spy;
        for _ in 0..due_ui {
            self.audio.tick();
        }
        // The top bar's speed sound (`top_bar`) over this screen.
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        self.audio.frame(human_race(&self.world.game));
    }

    /// `st_ShowAnim` for each name in turn (`OpenBink(name, 1, 0, 0, force)`, `PlayBink(1, 0, 0)`, `CloseBink`);
    /// `then` once the last ends.
    pub(super) fn queue_videos(&mut self, names: impl IntoIterator<Item = String>, force: bool, then: AfterVideo) {
        self.video_queue.extend(names.into_iter().map(|n| if force { format!("!{n}") } else { n }));
        self.after_video = then;
    }

    /// The sim's video requests (`st_ShowAnim`, `MEGHALAS`, `MEGNYERES`) become the queue.
    /// A game end (`MEGHALAS`, `MEGNYERES`) plays its videos (none for a race of 9+ or `MEGNYERES(false)`), then
    /// leaves for the main menu (re/rules/game-end.md §2).
    pub(super) fn take_video_requests(&mut self) {
        let reqs = std::mem::take(&mut self.world.videos);
        // A headless script run has no screen to play them on.
        if reqs.is_empty() || self.headless.is_some() {
            return;
        }
        let anims = self.anim_names.get_or_insert_with(|| crate::video::anim_names(&self.install));
        let race = human_race(&self.world.game) as usize;
        let campaign = self.world.game.state.mem.i32(ig2_sim::save::va::CAMPAIGN_TYPE) > 0;
        let names: Vec<String> = reqs.iter().flat_map(|r| r.names(anims, &self.tables.videos, race, campaign)).collect();
        if reqs.iter().any(crate::video::Request::ends_game) {
            self.record_match();
        }
        // A spectator's game goes on after its defeat (ADR 0026).
        let ends = reqs.iter().any(crate::video::Request::ends_game) && !self.world.game.local_view().spectating;
        match (ends, names.is_empty()) {
            (true, true) => self.exit_to_main_menu(),
            (true, false) => self.queue_videos(names, false, AfterVideo::MainMenu),
            (false, _) => self.queue_videos(names, false, AfterVideo::Resume),
        }
    }

    /// A frame while fullscreen videos play: the next one opens when one ends; the game and the menu wait. When the
    /// last ends their clocks go on from now (`FRISSITES_IGAZIT`: no catching up).
    pub(super) fn video_frame(&mut self) {
        let now = self.now_ms();
        if self.video.as_ref().is_some_and(|v| v.done()) {
            self.video = None;
            self.audio.stop_video();
        }
        while self.video.is_none() {
            let Some(name) = self.video_queue.pop_front() else { break };
            let (name, force) = match name.strip_prefix('!') {
                Some(n) => (n.to_string(), true),
                None => (name, false),
            };
            if let Some(v) = crate::video::playback::Playback::open(&self.install, &name, crate::video::playback::Place::Full, false, force) {
                // Ours (ADR 0051): the title's music only with the intro.
                let gate = (self.display.title_music && name.eq_ignore_ascii_case("title.bik")).then_some(crate::video::TITLE_INTRO_MS);
                self.audio.play_video(&v.player, true, gate);
                self.video = Some(v);
            }
        }
        let Some(v) = &mut self.video else {
            self.clock = TickClock::new(SIM_TICK_MS, now);
            if let Some(f) = &mut self.front {
                f.ui_ms = now;
            }
            match std::mem::take(&mut self.after_video) {
                AfterVideo::MenuMusic => self.audio.play_music(crate::audio::Music::Menu, true, human_race(&self.world.game)),
                AfterVideo::StarmapEntry(c) => {
                    self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game));
                    self.audio.play_start_voice(c);
                }
                AfterVideo::MainMenu => self.exit_to_main_menu(),
                AfterVideo::Resume => {}
            }
            return;
        };
        let Some(g) = &mut self.gpu else { return };
        v.frame(&mut g.renderer, &self.install, now as u64, g.config.height);
        let win = [g.config.width as f32, g.config.height as f32];
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        v.draw(&mut frame, win, &crate::font::Virtual::new(win));
        let Ok(out) = g.surface.get_current_texture() else {
            self.reconfigure();
            return;
        };
        g.renderer.draw(&frame, &out.texture, (g.config.width, g.config.height), now as f32 / 1000.0);
        g.window.pre_present_notify();
        out.present();
    }

    /// The messages screen's talking-head clip (re/screens/video.md §5.3) and its speech (re/screens/audio.md §5): a
    /// `TE` part read sets the clips and arms `StartAnim`, which fires when the typer's front reaches the other side's
    /// colour (`MessagesGfx2D_2`) and starts the speech and the alien voice; the speech's end (`FRISSIT_Messages2`,
    /// every 11th UI tick) or the typer's end moves to the wait clip, and a plain message or a dialogue's last text to
    /// the end clip (`messages-end.wav`, the music back). Leaving the screen closes it (`Messages_Kilep`). Drawn by
    /// the HUD at (201, 204) with the interlace over it.
    pub(super) fn head_clip_frame(&mut self, now: u32, due_ui: u64) {
        use crate::audio::SoundId;
        use crate::messages_screen::ClipStep;
        use ig2_rules::sounds::Table;
        let Some(h) = &mut self.hud else { return };
        // `MessagesAnimFrissit` (exe 0x506f60) and `StartAnim`'s test: the clip only at `mesframe` 30, not closing.
        let open = self.overlay == Some(crate::ui::Screen::Messages) && h.messages.text_up();
        let Some(d) = h.messages.open.as_mut().filter(|_| open) else {
            self.head.close();
            self.head_armed = false;
            h.clip = None;
            return;
        };
        let state = &self.world.game.state;
        let en = state.mem.i32(ig2_sim::save::va::EN);
        let lists = self.clip_lists.get_or_insert_with(|| crate::video::message::load_lists(&self.install, en));
        let k = state.mem.i32(ig2_sim::save::va::INBOX_RANDOM + 4 * d.index as u32);
        let race = human_race(&self.world.game);
        let table = &self.tables.videos.message_clips;
        let steps = std::mem::take(&mut d.clip);
        let audio = &mut self.audio;
        // `StopMessagesHang`, `StopHalkAlienHang`, `MesHangIndult = 0` (`Messages_Main`'s presses, a new message).
        if steps.contains(&ClipStep::Hush) {
            audio.stop_message_speech();
            audio.stop_alien();
            audio.message.indult = false;
        }
        // `StartAnim_EndOfMessage`'s sounds when its clip starts.
        let end_sounds = |audio: &mut crate::audio::Audio| {
            audio.stop_alien();
            audio.play(SoundId::new(Table::Mes, 2));
            audio.set_duck(0);
        };
        for s in &steps {
            if let ClipStep::PartRead(part) = *s {
                self.head.part_read(lists, d.id.max(0) as u32, part);
                self.head_armed = true;
            }
        }
        if self.head_armed && d.front_colour() == 2 {
            // `StartAnim` exe 0x5095b0: the typer muted, the clip, the speech, the alien voice.
            audio.mute(SoundId::global(7), true);
            self.head.start(&self.install, table, k);
            audio.play_message_speech(en, &self.head.sound);
            audio.stop_alien();
            let idx = crate::video::message::alien_index(&self.head.anim, en, k, self.head.face);
            if let Some(rec) = crate::video::message::alien_record(idx) {
                audio.play_alien(rec);
            }
            audio.message.indult = audio.sound;
            self.head_armed = false;
        }
        // `FRISSIT_Messages2` exe 0x505180: the speech over → the wait clip (a conversation) or the end clip.
        for _ in 0..due_ui {
            self.mes_gyorsito += 1;
            if self.mes_gyorsito > 10 {
                self.mes_gyorsito = 0;
                if audio.sound && audio.message.indult && self.head.akt == 0 && self.head.sound.contains("ogg") && !audio.message_speech_playing() {
                    let ended = if d.conversation {
                        audio.message.indult = audio.sound;
                        self.head.wait(&self.install, table, k, race)
                    } else {
                        self.head.end(&self.install, race)
                    };
                    if ended {
                        end_sounds(audio);
                    }
                }
            }
        }
        for s in &steps {
            if let ClipStep::TypedOut { plain, ends } = *s {
                let mut ended = false;
                if self.head.akt == 0 {
                    audio.message.indult = audio.sound;
                    ended = self.head.wait(&self.install, table, k, race);
                }
                if plain || ends {
                    ended |= self.head.end(&self.install, race);
                }
                self.head.fresh = true;
                if ended {
                    end_sounds(audio);
                }
            }
        }
        let Some((renderer, size, _)) = gfx(&mut self.gpu, &mut self.headless) else { return };
        if h.interlace.is_none() {
            h.interlace = renderer.file_texture(&self.install, "dataraw/textures/messages/finalinter_bmp.pvr").ok();
        }
        h.clip = None;
        let fresh = std::mem::take(&mut self.head.fresh);
        if let Some(p) = self.head.playback.as_mut().filter(|_| !fresh) {
            p.frame(renderer, &self.install, now as u64, size.1);
            let win = [size.0 as f32, size.1 as f32];
            h.clip = p.quad(win, &crate::font::Virtual::new(win));
        }
    }
}
