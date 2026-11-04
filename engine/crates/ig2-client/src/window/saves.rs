//! Games started, saved, loaded and left: saves, autosave, replays and the way back to the main menu.

use super::*;

impl App {
    /// Ours (ADR 0050, re/rules/game-end.md §6): a single-player scenario this machine won with the high-score entry
    /// on: its registry name and score, `Ido − scenario_kezdesi_ido` (the exe has no formula: `scenario_score` is never
    /// written; its dialog shows `score / 50` days, so the score is the days to win in `Ido` units).
    pub(super) fn won_scenario(&self) -> Option<(String, i32)> {
        if !self.display.high_scores || self.world.net.is_some() || self.world.game.over != Some(ig2_sim::game::GameOver::Won) {
            return None;
        }
        let m = &self.world.game.state.mem;
        let name: Vec<u8> = (0..0x1e).map(|i| m.u8(SCENARIO_REGISTRY + i)).take_while(|&b| b != 0).collect();
        let score = m.i32(ig2_sim::save::va::IDO) - m.i32(SCENARIO_START);
        (!name.is_empty() && score > 0).then(|| (String::from_utf8_lossy(&name).into_owned(), score))
    }

    pub(super) fn exit_to_main_menu(&mut self) {
        // `ExitToMainScreen`: `TutorialStop` (the overrun flags back); in multiplayer the session's teardown
        // (`MultiCloseClient`/`Server`).
        if let Some(t) = self.tutorial.take() {
            let m = &mut self.world.game.state.mem;
            for (i, x) in t.overrun.into_iter().flatten().enumerate() {
                m.set_i32(0xB77230 + 4 * i as u32, x);
            }
            if let Some(a) = t.ai_off {
                m.set_i32(ig2_sim::save::va::AI_OFF, a);
            }
        }
        if let Some((net, _)) = self.world.net.take() {
            net.leave();
        }
        let scene = self.front.take().and_then(|f| f.scene);
        let mut front = Front::new(scene, self.now_ms(), &self.settings);
        // Ours (ADR 0050): back from a won scenario, its list under the high-score entry (`MainScr_Belep` step 7).
        if let Some((registry, score)) = self.won_scenario() {
            let a = front.scenarios.get_or_insert_with(|| crate::mainscr_scenario::Assets::load(&self.install, &crate::ui::UiTexts::load(&self.install)));
            front.s.open_high_score(a.entries.clone(), &registry, score);
        }
        // Ours (ADR 0034): a finished game's statistics, after its end videos.
        front.stats = self.stats_end.take().map(crate::stats_screen::StatsScreen::end);
        self.front = Some(front);
        self.close_screens();
        self.audio.play_music(crate::audio::Music::Menu, false, human_race(&self.world.game));
    }

    /// `Options_EnterPressed`: `LoadSave(slot, name, 2)` as the remake's own save (`save<n>.ig2s`), the slots read
    /// again and back to the screen before; a failed save stays (the exe's "Failed to save game!" line).
    pub(super) fn options_save(&mut self, slot: usize, name: Vec<u8>) {
        let Some(dir) = crate::options::slot_dir() else { return };
        let ok = self.write_save(&dir.join(format!("save{slot}.ig2s")), &name);
        let slots = crate::options::read_slots(Some(&dir));
        let Some(o) = &mut self.options else { return };
        match ok {
            Ok(()) => o.s.saved(slots),
            Err(e) => {
                self.world.local_lines.push(e.into_bytes());
                o.s.refresh(slots);
            }
        }
    }

    /// `LoadSave(…, 2)`: the game as a slot file (the state, then the title `name` and today's date) at `path`; a
    /// multiplayer game saves the session's state, on the host only.
    pub(super) fn write_save(&mut self, path: &std::path::Path, name: &[u8]) -> Result<(), String> {
        let (y, m, d) = self.world.curr.date();
        let state = match self.world.mp_save() {
            // `LoadSave(…, 2)` writes the live starmap camera (`StarmapXStart` … `StarmapPlanetKering`, exe 0xB9A968..);
            // a multiplayer save leaves the synced state alone.
            None => {
                if let Some(v) = &self.view {
                    self.world.store_view(&v.camera);
                }
                // The research screen's place (one peer's UI) as the exe's globals hold it when saving.
                if let Some(p) = self.research.as_ref().map(|r| r.place()).or(self.research_place) {
                    p.write(&mut self.world.game.state);
                }
                self.world.save_file()
            }
            Some(r) => r?,
        };
        let file = crate::options::slot_file(state, name, [y, m, d]);
        let groups = self.fleet_groups;
        path.parent().map_or(Ok(()), ig2_formats::userfs::create_dir_all).and_then(|_| ig2_formats::userfs::write(path, file)).and_then(|_| groups.store(path)).map_err(|e| {
            eprintln!("ig2-client: {}: {e}", path.display());
            String::from_utf8_lossy(&crate::tr::fill(crate::tr::game(), "Cannot save: {}", &[e.to_string().as_bytes()])).into_owned()
        })
    }

    /// The Options screen's save gates (`Options_Main`'s save button) without the screen: a game runs, not under the
    /// tutorial, not in a battle (the space battle or a ground war) unless `DEBUGX`.
    pub(super) fn can_save(&mut self) -> bool {
        let debug = self.world.game.state.mem.u8(ig2_sim::save::va::DEBUGX) != 0;
        let planet = self.world.colony_curr.as_ref().map(|c| c.planet).filter(|_| self.colony.is_some() && self.overlay.is_none());
        let war = planet.is_some_and(|p| self.world.game.warfare_status(p) == 1);
        let battle = self.spacewar.is_some() || self.spacewar_parked.is_some() || war;
        let tutorial = self.tutorial.as_ref().is_some_and(|t| t.t.active || t.t.paused);
        self.front.is_none() && self.options.is_none() && self.help.is_none() && !tutorial && (!battle || debug)
    }

    /// ADR 0032: F12 saves into the quick save beside the slots.
    pub(super) fn quick_save(&mut self) {
        let Some(dir) = crate::options::slot_dir() else { return };
        if !self.can_save() {
            return;
        }
        let (y, m, d) = self.world.curr.date();
        let line = match self.write_save(&dir.join(crate::options::QUICKSAVE), &crate::tr::fill(crate::tr::game(), "Quicksave {}", &[format!("{y} {m} {d}").as_bytes()])) {
            Ok(()) => crate::tr::get(crate::tr::game(), "Quick saved."),
            Err(e) => e.into_bytes(),
        };
        self.world.local_lines.push(line);
    }

    /// The main menu's "History" (ours, ADR 0034): the match history over the menu.
    pub(super) fn open_history(&mut self) {
        let (list, error) = match crate::stats_screen::history_dir().map(|d| crate::stats_screen::load(&d)) {
            Some(Ok(l)) => (l, None),
            Some(Err(e)) => (Vec::new(), Some(e)),
            None => (Vec::new(), None),
        };
        if let Some(f) = &mut self.front {
            f.stats = Some(crate::stats_screen::StatsScreen::history(list, error));
            f.opening = true;
        }
        self.audio.play(crate::audio::CLICK);
    }

    /// The main menu's "Replays" (ours, ADR 0038 section 6): the replays list over the menu.
    pub(super) fn open_replays(&mut self) {
        if let Some(f) = &mut self.front {
            f.replays = Some(crate::replay_viewer::ReplayList::open(None));
            f.opening = true;
        }
        self.audio.play(crate::audio::CLICK);
    }

    /// ADR 0032: Shift+F12 loads the quick save (single player only), as the Options screen's load does.
    pub(super) fn quick_load(&mut self) {
        let Some(path) = crate::options::slot_dir().map(|d| d.join(crate::options::QUICKSAVE)) else { return };
        let tutorial = self.tutorial.as_ref().is_some_and(|t| t.t.active || t.t.paused);
        if self.multiplayer() || self.front.is_some() || self.options.is_some() || self.help.is_some() || tutorial || !path.exists() {
            return;
        }
        if self.load_file(&path) {
            self.world.local_lines.push(crate::tr::get(crate::tr::game(), "Quick loaded."));
        }
    }

    /// `LoadSave(slot, "", 1)`, then the starmap; true when it loaded.
    pub(super) fn load_file(&mut self, path: &std::path::Path) -> bool {
        match World::load_slot(path, self.rules, &self.tech_defaults, &self.volatile) {
            Ok(w) => {
                self.start_world(w);
                self.fleet_groups = crate::fleet_groups::Groups::load(path);
                // `Options_Main`'s load: the last planet whose ground war runs (`GetWarfareStatus == 1`) opens on its
                // colony screen (`AktKepKell 3`, back to the starmap).
                let n = self.world.game.state.planet_count();
                if let Some(p) = (1..=n).filter(|&p| self.world.game.warfare_status(p) == 1).last() {
                    let sel = self.selection();
                    self.select(Selection { mode: 1, planet: p, ..sel });
                    self.open_colony(p);
                }
                true
            }
            Err(e) => {
                eprintln!("ig2-client: load: {e}");
                false
            }
        }
    }

    /// `IdoNapValtozott` (exe 0x620720) after a tick: on a new day the autosave (ADR 0032), into slot 24 at the
    /// original's point, or every N days with the last one moved to slot 23. Single player not under the tutorial,
    /// with the "Auto Save" row on; in multiplayer only with an N-day cadence and on the host.
    pub(super) fn autosave_tick(&mut self) {
        if self.world.viewer.is_some() {
            return;
        }
        let today = self.world.curr.date();
        if today == self.autosave.0 {
            return;
        }
        self.autosave = (today, self.autosave.1 + 1);
        let row = |key: &str| self.opt_rows.iter().find(|r| r.key == key).map(|r| r.value);
        let every = crate::options::AUTOSAVE_DAYS[row(crate::display::AUTOSAVE).unwrap_or(0).clamp(0, 3) as usize];
        let on = row("OptionsKellAutoSave") != Some(0);
        let tutorial = self.tutorial.as_ref().is_some_and(|t| t.t.active || t.t.paused);
        let mp = self.world.net.is_some();
        let (y, m, d) = today;
        if !on || tutorial || (mp && (every == 0 || !self.world.mp_host())) || !crate::options::autosave_due(every, self.autosave.1, m, d) {
            return;
        }
        let Some(dir) = crate::options::slot_dir() else { return };
        self.autosave.1 = 0;
        if every > 0 {
            if let Err(e) = crate::options::rotate_autosaves(&dir) {
                eprintln!("ig2-client: autosave: {e}");
            }
        }
        if let Err(e) = self.write_save(&dir.join("save24.ig2s"), &crate::tr::fill(crate::tr::game(), "Autosave {}", &[format!("{y} {m} {d}").as_bytes()])) {
            self.world.local_lines.push(e.into_bytes());
        }
    }

    /// The screens of the game before close (a load or the main menu starts afresh).
    pub(super) fn close_screens(&mut self) {
        self.colony = None;
        self.overlay = None;
        self.spy = None;
        self.diplomacy = None;
        // Dropped, not `drop_design`: the old game's queued design orders must not reach the next world.
        self.design = None;
        self.trade = None;
        self.trade_under_options = None;
        self.drop_research();
        self.stop_battle_loops();
        self.spacewar = None;
        self.spacewar_parked = None;
        self.cam_tween = None;
        self.cam_saved = None;
        self.panels = Panels::default();
        self.menus = Menus::default();
    }

    /// A loaded slot (`Options_Main`'s load: `LoadSave(slot, "", 1)`, `SetIdoSpeed(1, 0)`, unpaused, the starmap).
    pub(super) fn start_world(&mut self, w: World<'static>) {
        self.close_screens();
        self.tutorial = None;
        self.research_place = None;
        self.akt_planet = None;
        self.fleet_groups = Default::default();
        self.volt_harc = false;
        self.world.end_replay();
        self.world = w;
        self.autosave = (self.world.curr.date(), 0);
        // A replay's battle flags are its start's.
        if self.world.viewer.is_none() {
            self.apply_battle_rows();
        }
        // `_s(0x3b, n)` is the install's table, not the save's: a loaded game names the AI's designs too.
        if self.world.game.design_texts.is_empty() {
            self.world.game.design_texts = crate::new_game::design_texts(&self.install);
        }
        self.world.set_speed(ig2_sim::time::Speed::Normal);
        if self.world.paused() {
            self.send(StarmapOrder::Pause { paused: false });
        }
        if let Some(g) = &self.gpu {
            let [_, _, bw, bh] = g.layout().band();
            self.view = Some(View::new(&self.world.curr, &self.world.view, (bw as u32, bh as u32), g.window.scale_factor() as f32));
        }
        if let Some(h) = &self.hud {
            self.world.game.empire_names = h.empire_names.clone();
        }
        self.front = None;
        // A campaign's intro comes first; the music follows it (`AfterVideo::StarmapEntry`).
        if self.world.videos.is_empty() {
            self.audio.play_music(crate::audio::Music::Main, false, human_race(&self.world.game));
        }
        self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
        if self.world.viewer.is_none() {
            self.start_replay();
        }
    }

    /// Ours (ADR 0038 §6): a replay from the list opens read-only; one that doesn't read stays in the list with why.
    pub(super) fn watch_replay(&mut self, path: &std::path::Path) {
        let w = ig2_formats::userfs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| ig2_net::replay::Replay::read(&b).map_err(|e| e.to_string()))
            .and_then(|r| {
                if r.meta.build != crate::version() {
                    eprintln!("ig2-client: replay recorded by {}, played by {}", r.meta.build, crate::version());
                }
                World::watch(r, self.rules)
            });
        match w {
            Ok(w) => self.start_world(w),
            Err(e) => {
                if let Some(f) = &mut self.front {
                    f.replays = Some(crate::replay_viewer::ReplayList::open(Some(format!("{}: {e}", path.display()))));
                }
            }
        }
    }

    /// The replay viewer's bar is up: a replay on the game screens, nothing over them.
    pub(super) fn replay_bar_up(&self) -> bool {
        self.world.viewer.is_some() && self.front.is_none() && self.options.is_none() && self.help.is_none() && self.video.is_none()
    }

    /// The bar's or a key's act; Exit, or the replay's end, goes back to the replays list.
    pub(super) fn replay_act(&mut self, a: crate::replay_viewer::Act) {
        use crate::replay_viewer::{Act, End};
        let Some(v) = &mut self.world.viewer else { return };
        v.act(a);
        let end = v.end;
        if a != Act::Exit && end.is_none() {
            return;
        }
        self.exit_to_main_menu();
        // A replay that drifted from its recording says where (ADR 0038 §3).
        let error = match end {
            Some(End::Diverged(d)) => Some(format!("The replay stopped: {d}.")),
            _ => None,
        };
        if let Some(f) = &mut self.front {
            f.replays = Some(crate::replay_viewer::ReplayList::open(error));
        }
    }

    /// ADR 0038: the game's replay into `replays/` beside the slots, unless the "Record replays" row is off.
    pub(super) fn start_replay(&mut self) {
        let on = self.settings.int(crate::display::REPLAYS) != Some(0);
        let Some(dir) = crate::options::slot_dir().map(|d| d.join("replays")).filter(|_| on) else { return };
        let mp = self.world.net.is_some();
        let r = ig2_formats::userfs::create_dir_all(&dir).and_then(|()| {
            crate::options::prune_replays(&dir, crate::options::KEPT_REPLAYS - 1)?;
            self.world.record_replay(dir.join(crate::options::replay_name(mp)))
        });
        if let Err(e) = r {
            eprintln!("ig2-client: replay: {e}");
        }
    }
}
