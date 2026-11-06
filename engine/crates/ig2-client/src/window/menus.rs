//! Options and Help over the game: opening, their frames and keys, and what the Options screen's acts change.

use super::*;

impl App {
    /// `Options_Belep` (re/screens/options.md §3): the settings as the audio and the settings file hold them, the
    /// slots read; in a game the clock pauses (`SetIdoPause(1, 0)`) until it is left.
    /// `AktKepKell = 0xd`: from the messages screen `Messages_Kilep`'s closing runs first; from the battle its zoom-out
    /// (the tutorial's O too); else Options at once.
    pub(super) fn ask_options(&mut self) {
        if self.spacewar.is_some() {
            self.leave_battle(0xd);
        } else if self.overlay == Some(crate::ui::Screen::Messages) {
            self.request_screen(0xd);
        } else {
            self.open_options(false);
        }
    }

    pub(super) fn open_options(&mut self, load_only: bool) {
        if self.options.is_some() {
            return;
        }
        let Some(h) = &self.hud else { return };
        let s = crate::options::Settings { music: self.audio.music_volume, sound: self.audio.sound_volume, ..crate::options::start_settings(&self.settings) };
        let slots = crate::options::read_slots(crate::options::slot_dir().as_deref());
        let mp = self.multiplayer();
        let mut screen = crate::options::OptionsScreen::enter(&h.texts, self.opt_rows.clone(), &s, load_only, mp, slots);
        screen.lang_at_start = Some(self.lang_at_start);
        if self.lang_forced {
            if let Some(r) = screen.rows.iter_mut().find(|r| r.ours && r.key == crate::display::LANGUAGE) {
                r.locked = true;
            }
            crate::options::check_dependencies(&mut screen.rows, mp);
        }
        screen.tutorial = self.tutorial.as_ref().is_some_and(|t| t.t.active || t.t.paused);
        if screen.tutorial {
            screen.lock_battle_simulate();
        }
        screen.mp_save = mp && self.world.mp_host();
        // `OptionsHarcFolyik`: `SpaceWar_Kilep` sets it towards Options (exe 0x562c0f), `Colony_Kilep` when the planet's
        // ground war runs (exe 0x466631, `GetWarfareStatus` inlined); `Options_Belep` keeps it only from 3 or 0xb.
        let planet = self.world.colony_curr.as_ref().map(|c| c.planet).filter(|_| self.colony.is_some() && self.overlay.is_none());
        let war = planet.is_some_and(|p| self.world.game.warfare_status(p) == 1);
        screen.from_battle = !load_only && (self.spacewar.is_some() || self.spacewar_parked.is_some());
        screen.battle = !load_only && (screen.from_battle || war);
        screen.debug = self.world.game.state.mem.u8(ig2_sim::save::va::DEBUGX) != 0;
        // Ours (ADR 0067): the in-game menu (and the main menu's Load game) as the modern page.
        if self.display.modern {
            let name = self.settings.get("MultiplayerNev").filter(|n| !n.is_empty()).map_or_else(|| b"Player".to_vec(), |n| n.as_bytes().to_vec());
            screen.modern = Some(crate::options::modern::Modern::in_game(&screen, name, self.bindings.clone()));
        }
        let glow = self.gpu.as_mut().and_then(|g| {
            self.install.resolve_texture("textures/starmap/", "laservago.bmp").and_then(|p| crate::assets::Image::load(&self.install, &p).ok()).map(|img| g.renderer.add_texture(&img))
        });
        // `SetIdoPause` (exe 0x541300) changes nothing in MP: neither the pause here nor `Options_Kilep`'s restore.
        let was_paused = self.world.paused() || mp;
        if !load_only && !was_paused {
            self.send(StarmapOrder::Pause { paused: true });
        }
        // `Options_Belep`: from the messages screen `OptionsElottiKep = MessagesElottiScreen` (Messages_Kilep runs: back
        // goes to the screen under it, re/screens/options.md §3).
        if self.overlay == Some(crate::ui::Screen::Messages) {
            self.overlay = None;
        }
        self.options = Some(OptionsUp { s: screen, ui_ms: self.now_ms(), glow, was_paused });
    }

    /// The Options screen's UI ticks due and its live settings (`Gfx2D_2` sets the volumes every frame), then what
    /// its fade-out ends in.
    pub(super) fn options_tick(&mut self) {
        let now = self.now_ms();
        let Some(o) = &mut self.options else { return };
        while now >= o.ui_ms + 10 {
            o.ui_ms += 10;
            o.s.tick();
        }
        let s = o.s.settings();
        (self.audio.music_volume, self.audio.sound_volume) = (s.music, s.sound);
        // ADR 0058: the voice volume heard as it is set, too.
        if let Some(r) = o.s.rows.iter().find(|r| r.ours && r.key == crate::display::VOICE_VOLUME) {
            self.audio.voice_volume = (r.value > 0).then(|| (r.value - 1) * 10);
        }
        if std::mem::take(&mut o.s.dirty) {
            let snap = o.s.clone();
            self.store_settings_from(&snap);
        }
        let act = self.options.as_mut().and_then(|o| o.s.done());
        if let Some(a) = act {
            self.options_act(a);
        }
    }

    /// A key while Options is up: the name entry's keys (`DoKeyboard` with `IOMode`), else Esc or O back
    /// (`ESC_Pressed`).
    pub(super) fn options_key(&mut self, k: &Key) {
        use crate::options::EntryKey;
        let changing = self.screen_changing();
        let Some(o) = &mut self.options else { return };
        if o.s.modern.is_some() {
            use crate::options::modern::Nav;
            // ADR 0067: the save name's Home, End and Delete, as on the original's entry.
            let edit = match k.as_ref() {
                Key::Named(NamedKey::Home) => Some(EntryKey::Home),
                Key::Named(NamedKey::End) => Some(EntryKey::End),
                Key::Named(NamedKey::Delete) => Some(EntryKey::Delete),
                _ => None,
            };
            if let Some(e) = edit.filter(|_| o.s.entry.is_some()) {
                o.s.entry_key(e);
                return;
            }
            let typing = o.s.typing();
            let shift = self.modifiers.shift_key();
            let nav = match k.as_ref() {
                Key::Named(NamedKey::Escape) => Some(Nav::Back),
                Key::Named(NamedKey::ArrowUp) => Some(Nav::Up),
                Key::Named(NamedKey::ArrowDown) => Some(Nav::Down),
                Key::Named(NamedKey::ArrowLeft) => Some(Nav::Left),
                Key::Named(NamedKey::ArrowRight) => Some(Nav::Right),
                Key::Named(NamedKey::Tab) => Some(if shift { Nav::PrevTab } else { Nav::NextTab }),
                Key::Named(NamedKey::PageDown) => Some(Nav::NextTab),
                Key::Named(NamedKey::PageUp) => Some(Nav::PrevTab),
                Key::Named(NamedKey::Enter) => Some(Nav::Activate),
                Key::Named(NamedKey::Space) if !typing => Some(Nav::Activate),
                Key::Named(NamedKey::Space) => Some(Nav::Char(b' ')),
                Key::Named(NamedKey::Backspace) => Some(Nav::Erase),
                Key::Character(c) if typing && !self.modifiers.control_key() && !self.modifiers.alt_key() => self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c)).first().map(|&b| Nav::Char(b)),
                Key::Character("o" | "O") => Some(Nav::Back),
                _ => None,
            };
            let audio = &mut self.audio;
            let act = match (nav, &mut self.options) {
                (Some(n), Some(o)) => o.s.modern_key(n, &mut |id| audio.play(id)),
                _ => None,
            };
            // ADR 0067: the save name's Enter.
            if let Some(crate::options::Act::Save(slot, name)) = act {
                self.options_save(slot, name);
            }
            return;
        }
        let act = if o.s.entry.is_some() {
            let key = match k.as_ref() {
                // `DoKeyboard` case 0x1b (exe 0x4db1a0): nothing while a screen change is pending.
                Key::Named(NamedKey::Escape) if changing => None,
                Key::Named(NamedKey::Escape) => {
                    o.s.escape();
                    None
                }
                Key::Named(NamedKey::Enter) => Some(EntryKey::Enter),
                Key::Named(NamedKey::Backspace) => Some(EntryKey::Back),
                Key::Named(NamedKey::Delete) => Some(EntryKey::Delete),
                Key::Named(NamedKey::Home) => Some(EntryKey::Home),
                Key::Named(NamedKey::End) => Some(EntryKey::End),
                Key::Named(NamedKey::ArrowLeft) => Some(EntryKey::Left),
                Key::Named(NamedKey::ArrowRight) => Some(EntryKey::Right),
                Key::Named(NamedKey::Space) => Some(EntryKey::Char(b' ')),
                Key::Character(c) => {
                    // The text in the font's code page (`UiTexts::encode`), one byte a character.
                    let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                    b.first().copied().filter(|&b| b >= 0x20).map(EntryKey::Char)
                }
                _ => None,
            };
            key.and_then(|key| o.s.entry_key(key))
        } else {
            if matches!(k.as_ref(), Key::Named(NamedKey::Escape) | Key::Character("o" | "O")) {
                o.s.escape();
            }
            None
        };
        if let Some(crate::options::Act::Save(slot, name)) = act {
            self.options_save(slot, name);
        }
    }

    /// F1: `Help_Belep` over the screen that is up (`HelpElottiScreen`), opening animated; the game time stands until
    /// it is gone (`IdoPauseMertAtlepesAlattVan`). `DoKeyboard`'s F-key gate (exe 0x4db1a0): not on the main menu,
    /// the space battle or a colony at war without `DEBUGX`, Options only when it isn't load-only or in a battle
    /// (`OptionsCsakLoadLehet`, `OptionsHarcFolyik`), Messages only with no answers left and the text typed out, and
    /// not while the tutorial turns the screen off (`KepernyokLetiltva[1]`). From Options and Messages the screen
    /// goes (their `_Kilep`) and Help returns to the one before it (`OptionsElottiKep`, `MessagesElottiScreen`).
    pub(super) fn open_help(&mut self) {
        let debug = self.world.game.state.mem.u8(ig2_sim::save::va::DEBUGX) != 0;
        let war = self.colony.is_some() && self.overlay.is_none() && self.world.colony_curr.as_ref().is_some_and(|c| c.war_flag);
        let messages_busy = self.overlay == Some(crate::ui::Screen::Messages)
            && self.hud.as_ref().is_some_and(|h| h.messages.open.as_ref().is_some_and(|d| !d.answers.is_empty() || !d.typed_all));
        let options_locked = self.options.as_ref().is_some_and(|o| o.s.load_only || (o.s.battle && !debug));
        let off = self.tutorial.as_ref().is_some_and(|t| t.t.gates.screens[1]) && !debug;
        if self.help.is_some() || self.front.is_some() || ((self.spacewar.is_some() || war) && !debug) || options_locked || messages_busy || off {
            return;
        }
        // Over a parked battle (`DEBUGX`): `Options_Kilep` without re-entering it; `Help_Belep` takes
        // `OptionsElottiKep` (0xb) as `HelpElottiScreen`, so Help's Back goes to the starmap and on to the battle.
        if self.options.is_some() && self.spacewar_parked.is_some() {
            self.leave_options();
        } else if self.options.is_some() {
            self.options_act(crate::options::Act::Back);
        }
        if self.overlay == Some(crate::ui::Screen::Messages) {
            self.overlay = None;
        }
        let Some(font) = self.hud.as_ref().and_then(|h| h.font.as_ref()) else { return };
        let (index, read) = crate::help::install_files(&self.install);
        let s = crate::help::HelpScreen::enter(&index, read, &self.help_memory, true, crate::help::char_y(font));
        let glow = self.gpu.as_mut().and_then(|g| {
            self.install.resolve_texture("textures/starmap/", "laservago.bmp").and_then(|p| crate::assets::Image::load(&self.install, &p).ok()).map(|img| g.renderer.add_texture(&img))
        });
        self.help = Some(HelpUp { s, ui_ms: self.now_ms(), glow, pics: Default::default(), page: Default::default() });
    }

    /// Help goes at once (its screen changes under a key): the help state is kept, the game clock restarts.
    pub(super) fn leave_help(&mut self) {
        // Gone some other way than its closing: no Options after it.
        self.help_then_options = false;
        if let Some(h) = self.help.take() {
            self.help_memory = h.s.memory();
            self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
        }
    }

    /// The F-key asks for the screen Help is over (`HelpElottiScreen`): leaving Help is all it does.
    pub(super) fn help_under_is(&self, k: &Key<&str>) -> bool {
        use crate::ui::Screen;
        match k {
            Key::Named(NamedKey::F2) => self.overlay.is_none() && self.colony.is_none() && self.spy.is_none(),
            Key::Named(NamedKey::F3) => self.overlay.is_none() && self.colony.is_some(),
            Key::Named(NamedKey::F4) => self.overlay == Some(Screen::Research),
            Key::Named(NamedKey::F5) => self.overlay == Some(Screen::Design),
            Key::Named(NamedKey::F6) => self.overlay == Some(Screen::Diplomacy),
            Key::Named(NamedKey::F7) => self.spy.is_some(),
            Key::Named(NamedKey::F8) => self.overlay == Some(Screen::Trade),
            Key::Named(NamedKey::F9) => self.overlay == Some(Screen::Info),
            Key::Named(NamedKey::F10) => self.overlay == Some(Screen::Messages),
            _ => false,
        }
    }

    /// A game frame while Help is up: its UI ticks (no game ticks), black behind it (`Copy3hatterkep(0x35)`), the
    /// screen and the top bar; once it has closed, back to the screen before with the game clock restarted.
    pub(super) fn help_game_frame(&mut self) {
        let ov = self.frame_overlay();
        let now = self.now_ms();
        let due_ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50);
        self.ui_frames = now as u64 / 10;
        for _ in 0..due_ui {
            self.audio.tick();
        }
        // The top bar's speed sound (`top_bar`) over this screen.
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        self.audio.frame(human_race(&self.world.game));
        self.backdrop_switch();
        let backdrop = self.backdrop();
        let modern = self.help_modern_up();
        let Some(h) = &mut self.help else { return };
        while now >= h.ui_ms + 10 {
            h.ui_ms += 10;
            h.s.tick();
        }
        let (Some(g), Some(hud)) = (&mut self.gpu, &mut self.hud) else { return };
        h.pics.load(&mut g.renderer, &self.install, &h.s.text.pics);
        let (bw, bh) = g.band();
        let size = [bw as f32, bh as f32];
        let t = now as f32 / 1000.0;
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        frame.backdrop = backdrop;
        if let (Some(font), false) = (&hud.font, modern) {
            let d = crate::help::Draw { font, white: hud.white, glow: h.glow.unwrap_or(hud.white), v: crate::font::Virtual::new(size), palette: &hud.palette, texts: &hud.texts, pics: &h.pics, now_ms: self.start.elapsed().as_secs_f64() * 1000.0 };
            h.s.draw(&mut frame, &d, crate::help::char_y(font));
        }
        hud.paused = self.world.paused();
        hud.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Help, t);
        // Ours (ADR 0078): the modern page between the top bar and the pointer; it opens and closes with the
        // screen's frame.
        let mut top = None;
        if modern {
            let theme = page_theme(hud, size);
            let (page, s, install) = (&mut h.page, &mut h.s, &self.install);
            let fade = (s.frame / 100.0).clamp(0.0, 1.0);
            let mut act = None;
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| act = page.show(ctx, &hud.texts, s, install, fade));
            if act.is_some() {
                s.escape();
            }
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, hud, size);
            top = Some(f);
        } else {
            ov.draw(&mut frame, hud, size);
        }
        let done = h.s.done().is_some();
        if let Err(e) = g.present(frame, top, t) {
            self.surface_failed(e);
        }
        if done {
            if let Some(h) = self.help.take() {
                self.help_memory = h.s.memory();
            }
            self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
            if std::mem::take(&mut self.help_then_options) {
                self.open_options(false);
            }
        }
    }

    /// A game frame while Options is up: its ticks, the sound clock, black behind it (`Copy3hatterkep(0x35)`'s
    /// picture is black in the captures), the screen and the top bar.
    pub(super) fn options_game_frame(&mut self) {
        self.options_tick();
        if self.options.is_none() || self.front.is_some() {
            return;
        }
        let ov = self.frame_overlay();
        let now = self.now_ms();
        let due_ui = (now as u64 / 10).saturating_sub(self.ui_frames).min(50);
        self.ui_frames = now as u64 / 10;
        for _ in 0..due_ui {
            self.audio.tick();
        }
        // The top bar's speed sound (`top_bar`) over this screen.
        for id in self.sounds.drain(..) {
            self.audio.play(id);
        }
        self.audio.frame(human_race(&self.world.game));
        self.backdrop_switch();
        let backdrop = self.backdrop();
        let Some(g) = &mut self.gpu else { return };
        let (bw, bh) = g.band();
        let size = [bw as f32, bh as f32];
        let t = now as f32 / 1000.0;
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        frame.backdrop = backdrop;
        let (mut top, mut act) = (None, None);
        if let (Some(o), Some(h)) = (&mut self.options, &mut self.hud) {
            draw_options(o, h, &mut frame, size, self.start.elapsed().as_secs_f64() * 1000.0);
            if o.s.modern.is_some() {
                // ADR 0067: the egui page over the black, the pointer over it (the top bar is under the page).
                let theme = page_theme(h, size);
                let audio = &mut self.audio;
                g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| act = o.s.modern_page(ctx, &h.texts, &theme, &mut |id| audio.play(id)));
                let mut f = crate::render::Frame::new([0.0; 4]);
                ov.draw(&mut f, h, size);
                top = Some(f);
            } else {
                h.paused = true;
                h.draw(&mut frame, size, &self.world.ui, self.tables, crate::ui::Screen::Options, t);
                ov.draw(&mut frame, h, size);
            }
        }
        if let Err(e) = g.present(frame, top, t) {
            self.surface_failed(e);
            return;
        }
        if let Some(crate::options::Act::Save(slot, name)) = act {
            self.options_save(slot, name);
        }
    }

    /// What the screen asks once it has faded out (or, for a save, at once).
    pub(super) fn options_act(&mut self, a: crate::options::Act) {
        use crate::options::Act;
        let Some(o) = self.options.take() else { return };
        self.store_settings_from(&o.s);
        if !matches!(a, Act::Back | Act::Help) {
            self.trade_under_options = None;
        }
        let help = a == Act::Help;
        match a {
            Act::Back | Act::Help => {
                // `SetIdoPause(OptionsOldIdoPause)`.
                if !o.was_paused {
                    self.send(StarmapOrder::Pause { paused: false });
                }
                self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
                // Back to the trade screen it was opened from (`OldKep` 8), entered without its opening.
                if let Some(old) = self.trade_under_options.take() {
                    let mut t = crate::trade::Trade::back(&old, &self.world.game.state);
                    t.modern = self.display.modern.then(crate::gui::Transition::opening);
                    self.trade = Some(t);
                    self.overlay = Some(crate::ui::Screen::Trade);
                }
                // The screen before is entered again (`_Belep` with `OldKep` 0xd, re/screens/options.md §3.1): the spy
                // screen ignores `OldKep` and plays its opening again; research, diplomacy and info start over without
                // their opening (selection, menus, counters); the colony's `Colony_Belep` reads no `OldKep` (its
                // ambience again); the starmap's `Starmap_Belep` re-inits its faders (`fifo`, `starf`); design keeps
                // its state.
                use crate::ui::Screen;
                if let Some(mut sw) = self.spacewar_parked.take() {
                    sw.reenter(&mut self.world);
                    self.spacewar = Some(sw);
                } else if self.spy.is_some() {
                    self.spy = None;
                    self.toggle_spy();
                } else {
                    match self.overlay {
                        // `Research_Belep` again, unanimated: the state rebuilt, the field's `Res_Hangok[1]` once the
                        // models load (exe 0x523540, 0x51ee14); its place as the screen left it.
                        Some(Screen::Research) => {
                            self.drop_research();
                            self.enter_research(0xd);
                        }
                        Some(Screen::Diplomacy) => self.enter_diplomacy(false),
                        Some(Screen::Info) => {
                            if let Some(h) = &mut self.hud {
                                let tri = h.info.tri;
                                h.info = crate::ui::InfoScreen::enter(self.world.ui.human);
                                h.info.tri = tri;
                            }
                        }
                        None if self.colony.is_some() => self.reenter_colony(),
                        None => self.on_starmap = false,
                        _ => {}
                    }
                }
            }
            Act::MainMenu => {
                // `Options_Kilep` under the tutorial: `TutorialStop(1)` (the music's duck back) before `ExitToMainScreen`.
                if o.s.tutorial {
                    self.audio.set_duck(0);
                }
                self.exit_to_main_menu();
            }
            Act::Load(n) => {
                if let Some(path) = o.s.slots.get(n).and_then(Option::as_ref).map(|s| s.file.clone()) {
                    self.load_file(&path);
                }
            }
            Act::Save(..) => {}
            // Ours (ADR 0067): Exit game, as the window's close (in multiplayer the session is left first).
            Act::Exit => {
                self.exit_to_main_menu();
                if let Some(f) = &mut self.front {
                    f.quit = true;
                }
            }
        }
        if help {
            self.open_help();
        }
    }

    /// `SaveGlobalRegistrySettings`: the screen's settings into the remake's own settings file (beside the original's
    /// `options.dat`, which stays untouched), and the rows' values for the next time.
    pub(super) fn store_settings_from(&mut self, s: &crate::options::OptionsScreen) {
        crate::display::update_own_settings(|own| s.store(own));
        s.store(&mut self.settings);
        if let (Some(m), Some(f)) = (&s.modern, &mut self.front) {
            f.s.player = m.name.clone();
        }
        for r in &mut self.opt_rows {
            if let Some(n) = s.rows.iter().find(|n| n.key == r.key) {
                r.value = n.value;
            }
        }
        self.apply_battle_rows();
        self.apply_display();
    }

    /// `SaveOptions` (exe 0x51b640): the battle rows into the battle flags the sim reads (exe 0xB77230..0xB7723C), as
    /// a row is set and as a game starts or loads (the flags aren't in a save). Not in multiplayer: there they are
    /// lockstep state (ADR 0011). Under the tutorial's `DISABLE OVERRUN` they go into the values `TutorialStop` puts
    /// back, so its override holds while it runs.
    pub(super) fn apply_battle_rows(&mut self) {
        let Some(v) = crate::options::battle_flags(&self.opt_rows) else { return };
        if self.multiplayer() {
            return;
        }
        if let Some(saved) = self.tutorial.as_mut().and_then(|t| t.overrun.as_mut()) {
            *saved = v;
            return;
        }
        for (i, x) in v.into_iter().enumerate() {
            self.world.game.state.mem.set_i32(0xB77230 + 4 * i as u32, x);
        }
    }

    /// The display rows' settings (crate::display) applied where they changed: the window's mode and size, vsync,
    /// the letterbox, the live background and the names' language.
    pub(super) fn apply_display(&mut self) {
        let d = crate::display::Display::from_settings(&self.settings);
        let old = std::mem::replace(&mut self.display, d);
        if let Some(g) = &self.gpu {
            if (d.mode, d.size) != (old.mode, old.size) {
                g.window.set_fullscreen(d.fullscreen(g.window.current_monitor()));
                if let (crate::display::Mode::Window, Some((w, h))) = (d.mode, d.size) {
                    let _ = g.window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
                }
            }
        }
        if (d.vsync, d.letterbox) != (old.vsync, old.letterbox) {
            self.vsync = d.vsync;
            crate::kit::layout::set_letterbox(d.letterbox);
            self.reconfigure();
        }
        crate::kit::layout::set_box_models(d.box_models);
        crate::kit::layout::set_slider_under(d.slider_under);
        crate::diplomacy::set_articles(d.articles);
        crate::kit::layout::set_centred_exit(d.centred_exit);
        crate::render::set_sharp_ui(d.sharp_ui);
        crate::render::set_sharp_text(d.sharp_text);
        crate::upscale::set_factor(d.upscale);
        crate::render3d::set_lighting(d.lighting);
        crate::battle_fx::set_enhanced(d.battle_effects);
        crate::render::set_smooth_glows(d.smooth_glows);
        crate::gui::ds::cursor::set_modern(d.modern);
        crate::gui::set_ui_scale(crate::display::ui_scale(&self.settings));
        self.bindings = crate::bindings::Bindings::from_settings(&self.settings);
        self.audio.voice_volume = crate::display::voice_volume(&self.settings);
        if d.colour_blind != old.colour_blind {
            crate::starmap::set_colour_blind(d.colour_blind);
            self.reload_palettes();
        }
        if d.live_background != old.live_background {
            self.backdrop.live = d.live_background;
        }
        if let Some(f) = &mut self.front {
            f.s.settings = self.settings.int(crate::display::MENU_SETTINGS) != Some(0);
            f.s.flow = self.settings.int(crate::menu_flow::KEY) == Some(1);
            f.s.history = d.statistics;
            // ADR 0082: the main menu follows the interface at once.
            f.s.flow |= d.modern;
            if d.modern != old.modern {
                f.s.modern = d.modern;
                f.s.continue_line = d.modern.then(crate::mainscr_modern::continue_line).flatten();
            }
        }
        // ADR 0082: an open in-game menu turns into the other interface's, on the page with the Interface row.
        if let Some(o) = self.options.as_mut().filter(|o| d.modern != old.modern && !o.s.settings_only) {
            o.s.modern = d.modern.then(|| {
                let name = self.settings.get("MultiplayerNev").filter(|n| !n.is_empty()).map_or_else(|| b"Player".to_vec(), |n| n.as_bytes().to_vec());
                let mut m = crate::options::modern::Modern::in_game(&o.s, name, self.bindings.clone());
                m.menu = false;
                m.set_tab(crate::options::modern::Tab::Display);
                m
            });
            if !d.modern {
                o.s.set_page(crate::options::REMAKE_PAGE);
            }
        }
        if d.one_language != old.one_language {
            self.one_language = d.one_language;
            if let Some(h) = &mut self.hud {
                h.set_one_language(&self.install, d.one_language);
            }
        }
    }

    /// ADR 0036: the palettes read again after the colour-blind option changed.
    pub(super) fn reload_palettes(&mut self) {
        let Ok(p) = crate::starmap::read_palette(&self.install) else { return };
        if let Some(h) = &mut self.hud {
            h.palette = p.clone();
        }
        if let Some(g) = &mut self.gpu {
            g.assets.set_palette(p);
        }
    }
}
