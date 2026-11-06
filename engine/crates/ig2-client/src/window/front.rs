//! The main menu and the screens before a game (new game, multiplayer rooms, mods): their frame, acts and keys.

use super::*;

impl App {
    /// A frame of the main menu: its UI ticks due (10 ms each), the frame (`Mainscreen_Update3D`), the hover.
    pub(super) fn front_frame(&mut self) {
        self.options_tick();
        let ov = self.frame_overlay();
        let now = self.now_ms();
        let (Some(fr), true) = (&mut self.front, self.gpu.is_some()) else { return };
        let mut due = 0;
        let mut outs = Vec::new();
        let mut acts = Vec::new();
        while now >= fr.ui_ms + 10 {
            fr.ui_ms += 10;
            fr.s.tick();
            acts.extend(fr.s.due());
            if let Some(mp) = &mut fr.mp {
                outs.extend(mp.tick(&mut fr.net));
            }
            due += 1;
        }
        for o in outs {
            self.front_mp_out(o);
        }
        for a in acts {
            self.front_act(a);
        }
        let (Some(fr), Some(g)) = (&mut self.front, &mut self.gpu) else { return };
        for _ in 0..due {
            self.audio.tick();
        }
        self.audio.frame(human_race(&self.world.game));
        fr.s.frame();
        // The main menu (`AktKep` 15) steps the row frame's globals too; the scenario list's frame carries them.
        let list = fr.s.scenario.as_mut().map(|sc| &mut sc.frame);
        carry_row_frame(&mut self.dk, &mut self.dk_owner, if list.is_some() { 6 } else { 0 }, list, due);
        fr.s.hover(fr.at);
        if let Some(st) = &mut fr.stats {
            st.hover(fr.at);
        }
        if let Some(rl) = &mut fr.replays {
            rl.hover(fr.at);
        }
        if let Some(st) = fr.s.take_started() {
            self.front_start(st);
            return;
        }
        let (bw, bh) = g.band();
        let size = [bw as f32, bh as f32];
        let scene = fr.scene.get_or_insert_with(|| crate::mainscr_scene::MainScene::load(&mut g.renderer, &self.install));
        let mut frame = crate::render::Frame::new([0.0, 0.0, 0.0, 1.0]);
        fr.s.race_boxes = scene.draw(&mut frame, &mut g.renderer, &self.install, &fr.s, &crate::font::Virtual::new(size));
        // Ours (ADR 0059): the panel stays out of reach under History and Replays.
        fr.s.covered = self.options.is_some() || fr.mods.is_some() || (fr.s.flow && (fr.stats.is_some() || fr.replays.is_some()));
        if let Some(h) = &self.hud {
            let now = self.start.elapsed().as_secs_f64() * 1000.0;
            draw_front_2d(fr, h, &mut g.renderer, &self.install, &mut frame, size, now, self.display.statistics);
        }
        if let (Some(o), Some(h)) = (&mut self.options, &self.hud) {
            draw_options(o, h, &mut frame, size, self.start.elapsed().as_secs_f64() * 1000.0);
        }
        // Ours (ADR 0062): the modern Options screen and the multiplayer pages (ADR 0064) are egui pages, over the 2D
        // and under the pointer.
        let mut top = None;
        let mut mp_outs = Vec::new();
        let mut mods_acts = Vec::new();
        let mut main_pick = None;
        let mut ng_act = None;
        let mut name_pick = None;
        let mut replay_act = None;
        let theme = |h: &crate::ui::Hud| page_theme(h, size);
        if let (Some(o), Some(h)) = (self.options.as_mut().filter(|o| o.s.modern.is_some()), &self.hud) {
            let audio = &mut self.audio;
            let theme = theme(h);
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| {
                // Load only: no save to hand back.
                let _ = o.s.modern_page(ctx, &h.texts, &theme, &mut |id| audio.play(id));
            });
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        } else if let (Some(mods), Some(h)) = (fr.mods.as_mut(), &self.hud) {
            // Ours (ADR 0068): the mod manager.
            let theme = theme(h);
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| mods_acts = mods.show(ctx, &theme));
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        } else if let (Some(st), true, Some(h)) = (fr.stats.as_mut(), fr.s.modern, &self.hud) {
            // Ours (ADR 0078): the modern end-of-game page over the scene.
            let theme = theme(h);
            let heads = fr.stats_heads.get_or_insert_with(|| crate::stats_screen::modern::load_heads(&g.gui.ctx, &self.install));
            let mut close = false;
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| close = st.modern(ctx, &h.texts, &h.palette, heads, 1.0));
            if close {
                fr.stats = None;
                self.audio.play(crate::audio::CLICK);
            }
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        } else if let (Some(rl), true, Some(h)) = (fr.replays.as_mut(), fr.s.modern, &self.hud) {
            // Ours (ADR 0078): the modern replays page over the scene.
            let theme = theme(h);
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| replay_act = rl.modern(ctx, &h.texts));
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        } else if let (Some(n), true, Some(h)) = (fr.s.name.as_ref(), fr.s.name_page_up() && fr.mp.is_none(), &self.hud) {
            // Ours (ADR 0078): the modern name prompt over the scene.
            let theme = theme(h);
            let gt = fr.s.gt.min(1.0);
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| name_pick = crate::mainscr_modern::name_page(ctx, &h.texts, n, gt));
            ov.draw(&mut frame, h, size);
        } else if let (true, Some(h)) = (fr.s.new_game_up() && fr.mp.is_none(), &self.hud) {
            // Ours (ADR 0078): the modern new game page over the scene.
            let theme = theme(h);
            let heads = fr.ng_heads.get_or_insert_with(|| crate::newgame_modern::load_heads(&g.gui.ctx, &self.install));
            let (st, blurbs, gt) = (&mut fr.ng, &fr.s.blurbs, fr.s.gt.min(1.0));
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| ng_act = crate::newgame_modern::page(ctx, &h.texts, st, blurbs, heads, gt));
            ov.draw(&mut frame, h, size);
        } else if let (true, Some(p), Some(h)) = (fr.s.modern && fr.mp.is_none(), fr.s.panel(), &self.hud) {
            // Ours (ADR 0078): the modern main menu over the scene.
            let theme = theme(h);
            let (history, can, line, gt) = (fr.s.history, fr.s.can_continue, fr.s.continue_line.clone(), fr.s.gt.min(1.0));
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| main_pick = crate::mainscr_modern::page(ctx, &h.texts, p, history, can, line.as_ref(), gt));
            ov.draw(&mut frame, h, size);
        } else if let (Some(mp), Some(h)) = (fr.mp.as_mut(), &self.hud) {
            let theme = theme(h);
            mp.load_heads(&g.gui.ctx, &self.install);
            let mut acts = Vec::new();
            g.gui.run((bw, bh), &crate::gui::style(&theme), |ctx| acts = mp.page(ctx, &h.texts, &h.palette, &theme, fr.s.flow));
            mp_outs = mp.apply(acts, &mut fr.net);
            let mut f = crate::render::Frame::new([0.0; 4]);
            ov.draw(&mut f, h, size);
            top = Some(f);
        } else if let Some(h) = &self.hud {
            ov.draw(&mut frame, h, size);
        }
        if let Err(e) = g.present(frame, top, now as f32 / 1000.0) {
            self.surface_failed(e);
            return;
        }
        for o in mp_outs {
            self.front_mp_out(o);
        }
        for a in mods_acts {
            self.mods_act(a);
        }
        match replay_act {
            Some(crate::replay_viewer::ListAct::Close) => {
                if let Some(f) = &mut self.front {
                    f.replays = None;
                }
                self.audio.play(crate::audio::CLICK);
            }
            Some(crate::replay_viewer::ListAct::Open(path)) => {
                self.audio.play(crate::audio::CLICK);
                self.watch_replay(&path);
            }
            None => {}
        }
        if let Some(p) = name_pick {
            self.audio.play(crate::audio::CLICK);
            let Some(f) = &mut self.front else { return };
            match p {
                crate::mainscr_modern::NamePick::Accept => {
                    if let Some(a) = f.s.name_key(crate::options::EntryKey::Enter) {
                        self.front_act(a);
                    }
                }
                crate::mainscr_modern::NamePick::Back => f.s.escape(),
            }
        }
        if let Some(a) = ng_act {
            use crate::newgame_modern::Act;
            self.audio.play(crate::audio::CLICK);
            let Some(f) = &mut self.front else { return };
            match a {
                Act::Back => f.s.close_race(),
                Act::Picked => {}
                Act::Start { race, level } => f.s.start_campaign(race, level),
                Act::Scenarios => {
                    f.s.close_race();
                    if let Some(a) = f.s.act_item(crate::menu_flow::Item::Scenarios) {
                        self.front_act(a);
                    }
                }
            }
        }
        match main_pick {
            Some(crate::mainscr_modern::Pick::Item(i)) => {
                if let Some(a) = self.front.as_mut().and_then(|f| f.s.act_item(i)) {
                    self.front_act(a);
                }
            }
            Some(crate::mainscr_modern::Pick::Back) => {
                if let Some(f) = &mut self.front {
                    f.s.escape();
                }
            }
            None => {}
        }
    }

    /// The mod manager's request (ADR 0068 §5).
    pub(super) fn mods_act(&mut self, a: crate::mods_page::Act) {
        use crate::mods_page::Act;
        let Some(f) = &mut self.front else { return };
        match a {
            Act::Back => f.mods = None,
            Act::Restart => match crate::mods_page::restart() {
                Ok(()) => f.quit = true,
                Err(e) => eprintln!("ig2-client: restart: {e}"),
            },
            Act::Click => {}
        }
        self.audio.play(crate::audio::CLICK);
    }

    /// A ring's request (`crate::mainscr::Act`): load game starts the game from the save (the Options screen's load
    /// slots aren't ported yet), quit leaves; the rest wait for their screens.
    pub(super) fn front_act(&mut self, a: crate::mainscr::Act) {
        use crate::mainscr::Act;
        match a {
            Act::Quit => {
                if let Some(f) = &mut self.front {
                    f.quit = true;
                }
            }
            // `OptionsCsakLoadLehet = 1; AktKep = 0xd`: Options in load-only mode over the main menu's scene.
            Act::LoadGame => self.open_options(true),
            // Ours (ADR 0031): the same screen's settings without the slot list, Load and Save, drawn and driven as the
            // modern main-menu screen (ADR 0053) with the multiplayer name.
            Act::Settings => {
                self.open_options(true);
                let name = self.front.as_ref().map(|f| f.s.player.clone()).unwrap_or_default();
                if let Some(o) = &mut self.options {
                    o.s.settings_only = true;
                    o.s.slots = vec![None; crate::options::SLOTS + 1];
                    o.s.modern = Some(crate::options::modern::Modern::new(&o.s, name, self.bindings.clone()));
                }
            }
            // `Mainscreen_Update3D` exe 0x50147d: `OpenBink("misc/credits.bik", 1, 0, 0, force)`, blocking.
            Act::Credits => self.queue_videos(["misc/credits.bik".to_string()], true, AfterVideo::Resume),
            Act::Click => self.audio.play(crate::audio::CLICK),
            Act::OpenScenarios | Act::Campaign | Act::QuickGame => {
                let Some(f) = &mut self.front else { return };
                let owned;
                let texts = match self.hud.as_ref() {
                    Some(h) => &h.texts,
                    None => {
                        owned = crate::ui::UiTexts::load(&self.install);
                        &owned
                    }
                };
                crate::mainscr_scene::prepare(&mut f.s, a, &mut f.scenarios, &self.install, texts);
            }
            // `MainScr_IPEntered`: `SaveGlobalRegistrySettings` (the name into the settings file).
            Act::NameEntered => {
                if let Some(f) = &self.front {
                    let name = String::from_utf8_lossy(&f.s.player).into_owned();
                    self.settings.set("MultiplayerNev", &name);
                    crate::display::update_own_settings(|own| own.set("MultiplayerNev", &name));
                }
            }
            // `MainScr_Main` 0x23 / 0x22: the server path and the remaster's matchmaking; ours: our MP screens
            // (ADR 0014 §4; "inet" is the same screen without matchmaking).
            Act::MultiServer | Act::MultiMatchmaking => {
                let rules = self.install.read_raw("ig2.exe").map(|exe| ig2_net::session::rules_fingerprint(&exe)).unwrap_or_default();
                let mods = crate::mods_fingerprint(&self.install);
                let rules = ig2_net::session::modded_fingerprint(rules, mods);
                let Some(f) = &mut self.front else { return };
                let name = String::from_utf8_lossy(&f.s.player).into_owned();
                let story = ig2_formats::config::Config::read(self.install.root()).int("mpstory");
                let races = crate::rules::added_names(&self.install).0.iter().map(|a| a.race as u8).collect();
                f.mp = Some(crate::mp_lobby::MpScreen::open(a == Act::MultiMatchmaking, &name, rules).with_mods(mods).with_races(races).with_story(story));
            }
            Act::Tutorial(_) => {}
            // Ours (ADR 0059): the newest save that reads, as the Load screen's Load.
            Act::Continue => {
                let saves = crate::options::slot_dir().map(|d| crate::menu_flow::saves_newest_first(&d)).unwrap_or_default();
                let _ = saves.iter().any(|p| self.load_file(p));
            }
            Act::History => self.open_history(),
            Act::Replays => self.open_replays(),
            Act::Mods => {
                if let Some(f) = &mut self.front {
                    f.mods = Some(crate::mods_page::ModsPage::open(&self.install));
                }
                self.audio.play(crate::audio::CLICK);
            }
            // Ours (ADR 0050): `AddHighscore` (`LoadHighscores`, the entry slotted in, `SaveHighscores`) into our
            // settings file (`options.dat` stays untouched), and the list's tables reloaded.
            Act::HighScore => {
                let Some(h) = self.front.as_mut().and_then(|f| f.s.scored.take()) else { return };
                let defaults = crate::mainscr_scenario::default_scores(&self.install);
                let mut reg = ig2_formats::options::Options::load();
                let mut t = ig2_formats::highscore::Table::load(&reg, &h.registry, &defaults);
                t.add(&ig2_formats::text::decode(&h.entry.text, self.install.lang()), h.score);
                t.save(&mut reg, &h.registry);
                let pairs: Vec<(String, String)> = reg.pairs.iter().filter(|(k, _)| k.starts_with(&format!("{}_", h.registry))).cloned().collect();
                let pairs: Vec<(&str, &str)> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                if let Err(e) = ig2_formats::options::Options::store_own(&pairs) {
                    eprintln!("ig2-client: high scores: {e}");
                }
                let Some(f) = &mut self.front else { return };
                let texts = crate::ui::UiTexts::load(&self.install);
                let scores = crate::mainscr_scenario::high_scores(&reg, &defaults, &h.registry, &texts);
                let lists = f.scenarios.iter_mut().map(|a| &mut a.entries).chain(f.s.scenario.iter_mut().map(|sc| &mut sc.entries));
                for e in lists.flatten().filter(|e| e.registry.eq_ignore_ascii_case(&h.registry)) {
                    e.scores = scores.clone();
                }
            }
        }
    }

    /// What our MP screens ask (ADR 0014): the click, leaving them, or the room's start: the game built here
    /// (`mp_lobby::launch`), then the menu's exit animation into the starmap.
    pub(super) fn front_mp_out(&mut self, o: crate::mp_lobby::Out) {
        use crate::mp_lobby::Out;
        match o {
            Out::Click => self.audio.play(crate::audio::CLICK),
            Out::Copy(s) => {
                if let Err(e) = arboard::Clipboard::new().and_then(|mut c| c.set_text(s)) {
                    eprintln!("ig2-client: clipboard: {e}");
                }
                self.audio.play(crate::audio::CLICK);
            }
            Out::Close => {
                if let Some(f) = &mut self.front {
                    f.mp = None;
                    f.s.close_mp();
                }
            }
            Out::Start(b) => {
                let rules = self.rules;
                let ticks = crate::new_game::wall_ticks();
                let built = crate::new_game::StartTexts::read(&self.install).and_then(|t| t.with(&self.install, |data| crate::mp_lobby::launch(b, data, rules, ticks, Box::new(crate::mp_lobby::ClientNet::default())))).and_then(|r| r);
                let Some(f) = &mut self.front else { return };
                match built {
                    Ok(s) => {
                        f.session = Some(s);
                        f.mp = None;
                        f.s.start_mp();
                    }
                    Err(e) => {
                        eprintln!("ig2-client: multiplayer start: {e}");
                        if let Some(mp) = &mut f.mp {
                            mp.line = e.into_bytes();
                        }
                    }
                }
            }
        }
    }

    /// `MainScr_Kilep` step 3 (spec §2.2): the game the menu was left for, built from the install; the starmap
    /// follows (a campaign after its intro).
    pub(super) fn front_start(&mut self, st: crate::mainscr::Start) {
        use crate::mainscr::Start;
        if let Start::Multiplayer = st {
            match self.front.as_mut().and_then(|f| f.session.take()) {
                Some(s) => self.start_world(World::new_mp(s, self.rules)),
                None => {
                    let scene = self.front.take().and_then(|f| f.scene);
                    self.front = Some(Front::new(scene, self.now_ms(), &self.settings));
                }
            }
            return;
        }
        let seed = crate::new_game::wall_ticks();
        let rules = self.rules;
        let game = match &st {
            Start::Campaign { race, level } => {
                // `Nehezseg` by the ring: easy 1, normal (index 2) 2, hard (index 1) 4.
                let difficulty = [1, 4, 2][(*level as usize).min(2)];
                // `InitStory` (exe 0x5c2940): the config's `storyline` and `storymap` (0 = not set).
                let config = ig2_formats::config::Config::read(self.install.root());
                let (storyline, storymap) = (config.int("storyline"), config.int("storymap"));
                let cfg = ig2_sim::game::new_game::NewCampaign { campaign: *race as i32 + 1, difficulty, seed, ticks: seed, storyline, storymap, last_storyline: self.settings.int(&format!("Fable_{}", *race as i32 + 1)) };
                crate::new_game::campaign(&self.install, &cfg, rules)
            }
            Start::Scenario { file, .. } => crate::new_game::scenario(&self.install, file, seed, rules),
            Start::Tutorial(n) => crate::new_game::tutorial(&self.install, *n, seed, rules),
            Start::Multiplayer => unreachable!("handled above"),
            Start::Skirmish { settings: k, kind } => {
                // Ours (ADR 0039): the quick game's next settings.
                self.settings.set(crate::mainscr_scenario::QUICK_GAME, &k.to_setting());
                if let Err(e) = ig2_formats::options::Options::store_own(&[(crate::mainscr_scenario::QUICK_GAME, &k.to_setting())]) {
                    eprintln!("ig2-client: settings: {e}");
                }
                match self.front.as_ref().and_then(|f| f.scenarios.as_ref()) {
                    Some(a) => crate::new_game::skirmish(&self.install, &k.config(*kind, &a.tables, seed), rules),
                    None => Err("no scenario list".into()),
                }
            }
        };
        match game {
            Ok(g) => {
                let mut w = World::new_game(g);
                // Ours (ADR 0045): the computer empires' ruleset of the new game, from the Options row.
                w.game.enhanced_ai = self.settings.int(crate::display::ENHANCED_AI) == Some(1);
                // Ours (ADR 0052): the campaign story's ruleset, likewise.
                w.game.story_fixes = self.settings.int(crate::display::STORY_FIXES) == Some(1);
                // `MainScr` 0x4fef1b: `StoreScenarioDescription` after `Betolt_Scenario`.
                w.scenario = matches!(st, Start::Scenario { .. });
                w.ui.scenario = w.scenario;
                // `MainScr_Kilep` 0x4fef45..63: a scenario's start time and registry name (saved; only ADR 0050's
                // high-score entry reads them).
                if let Start::Scenario { registry, .. } = &st {
                    let m = &mut w.game.state.mem;
                    m.set_i32(SCENARIO_START, m.i32(ig2_sim::save::va::IDO));
                    let name = registry.as_bytes();
                    for i in 0..0x1e {
                        m.set_u8(SCENARIO_REGISTRY + i as u32, if i < 0x1d { name.get(i).copied().unwrap_or(0) } else { 0 });
                    }
                }
                let campaign = matches!(st, Start::Campaign { .. });
                if campaign {
                    w.campaign_intro();
                }
                let voice = w.start_voice.take();
                self.start_world(w);
                if let Start::Tutorial(n) = st {
                    self.start_tutorial(n as i32);
                }
                if campaign {
                    // The intro first; then `Starmap_Belep`'s music and the start voice.
                    self.take_video_requests();
                    self.after_video = AfterVideo::StarmapEntry(voice.unwrap_or(0));
                } else if let Some(c) = voice {
                    self.audio.play_start_voice(c);
                }
            }
            Err(e) => {
                eprintln!("ig2-client: {st:?}: {e}");
                let scene = self.front.take().and_then(|f| f.scene);
                self.front = Some(Front::new(scene, self.now_ms(), &self.settings));
            }
        }
    }

    /// A key on the main menu: Esc (`MainScr_ESCPressedWhenMultiplayerselect`), or the name entry's keys; our MP
    /// screens take every key while they are up.
    pub(super) fn front_key(&mut self, k: &Key) {
        use crate::options::EntryKey;
        let Some(f) = &mut self.front else { return };
        if let Some(st) = &mut f.stats {
            match k.as_ref() {
                Key::Named(NamedKey::Escape | NamedKey::Enter) => {
                    if st.back().is_some() {
                        f.stats = None;
                    }
                }
                Key::Named(NamedKey::ArrowLeft) => st.step_metric(false),
                Key::Named(NamedKey::ArrowRight) => st.step_metric(true),
                _ => {}
            }
            return;
        }
        if f.replays.is_some() {
            if matches!(k.as_ref(), Key::Named(NamedKey::Escape | NamedKey::Enter)) {
                f.replays = None;
            }
            return;
        }
        if let Some(m) = &mut f.mods {
            if let Some(a) = m.key(k, self.modifiers.shift_key()) {
                self.mods_act(a);
            }
            return;
        }
        if let Some(mp) = &mut f.mp {
            // Ctrl+V pastes (a join code), Ctrl+C copies the host's join code.
            if self.modifiers.control_key() {
                match k.as_ref() {
                    Key::Character(c) if c.eq_ignore_ascii_case("v") => match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                        Ok(text) => mp.paste(&text),
                        Err(e) => eprintln!("ig2-client: clipboard: {e}"),
                    },
                    Key::Character(c) if c.eq_ignore_ascii_case("c") => {
                        if let Some(o) = mp.copy() {
                            self.front_mp_out(o);
                        }
                    }
                    _ => {}
                }
                return;
            }
            // Ours (ADR 0064): Tab moves between the connection page's fields, the arrows through its list.
            match k.as_ref() {
                Key::Named(NamedKey::Tab) => return mp.next_field(),
                Key::Named(NamedKey::ArrowUp | NamedKey::ArrowDown) => return mp.step_list(matches!(k.as_ref(), Key::Named(NamedKey::ArrowDown))),
                _ => {}
            }
            let escape = matches!(k.as_ref(), Key::Named(NamedKey::Escape));
            // An open drop-down takes Esc (it closes) and the keys meant for it.
            if let Some(g) = self.gpu.as_ref().filter(|g| egui::Popup::is_any_open(&g.gui.ctx)) {
                if escape {
                    egui::Popup::close_all(&g.gui.ctx);
                }
                return;
            }
            let key = match k.as_ref() {
                Key::Named(NamedKey::Enter) => Some(EntryKey::Enter),
                Key::Named(NamedKey::Backspace) => Some(EntryKey::Back),
                Key::Named(NamedKey::Delete) => Some(EntryKey::Delete),
                Key::Named(NamedKey::Home) => Some(EntryKey::Home),
                Key::Named(NamedKey::End) => Some(EntryKey::End),
                Key::Named(NamedKey::ArrowLeft) => Some(EntryKey::Left),
                Key::Named(NamedKey::ArrowRight) => Some(EntryKey::Right),
                Key::Named(NamedKey::Space) => Some(EntryKey::Char(b' ')),
                Key::Character(c) => {
                    let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                    b.first().copied().filter(|&b| b >= 0x20).map(EntryKey::Char)
                }
                _ => None,
            };
            if let Some(o) = mp.key(key, escape, &mut f.net) {
                self.front_mp_out(o);
            }
            return;
        }
        if matches!(k.as_ref(), Key::Named(NamedKey::Escape)) {
            f.s.escape();
            return;
        }
        if f.s.name.is_none() && f.s.high_score.is_none() {
            if let (Key::Character("o" | "O"), Some(a)) = (k.as_ref(), f.s.settings_key()) {
                self.front_act(a);
            }
            return;
        }
        let key = match k.as_ref() {
            Key::Named(NamedKey::Enter) => Some(EntryKey::Enter),
            Key::Named(NamedKey::Backspace) => Some(EntryKey::Back),
            Key::Named(NamedKey::Delete) => Some(EntryKey::Delete),
            Key::Named(NamedKey::Home) => Some(EntryKey::Home),
            Key::Named(NamedKey::End) => Some(EntryKey::End),
            Key::Named(NamedKey::ArrowLeft) => Some(EntryKey::Left),
            Key::Named(NamedKey::ArrowRight) => Some(EntryKey::Right),
            Key::Named(NamedKey::Space) => Some(EntryKey::Char(b' ')),
            // The high-score name takes the code page's letters (`HighScoreNev` goes through `KiirStr`).
            Key::Character(c) if f.s.high_score.is_some() => {
                let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                b.first().copied().map(EntryKey::Char)
            }
            Key::Character(c) => c.bytes().next().filter(|b| b.is_ascii()).map(EntryKey::Char),
            _ => None,
        };
        let act = match f.s.high_score.is_some() {
            true => key.and_then(|key| f.s.high_score_key(key)),
            false => key.and_then(|key| f.s.name_key(key)),
        };
        if let Some(a) = act {
            self.front_act(a);
        }
    }
}
