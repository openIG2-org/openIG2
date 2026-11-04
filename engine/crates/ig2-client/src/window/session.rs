//! What a frame hands between the world and the window: the multiplayer wait, the story's screen requests, the match record, transfers and renames for the lockstep.

use super::*;

impl App {
    /// The multiplayer wait overlay's state this frame (`MOUSE_DrawOver`'s open and hover).
    pub(super) fn update_mp_wait(&mut self) {
        let Some(crate::world::WaitView { turn, paused, reconnecting, host, late }) = self.world.wait_view() else {
            self.mp_wait = Default::default();
            return;
        };
        let races = |e: u8| {
            let race = self.world.game.state.mem.u8(ig2_sim::save::va::PLAYER_FAJ + e as u32);
            self.hud.as_ref().map_or_else(Vec::new, |h| h.texts.race(race))
        };
        let late = late.into_iter().map(|(e, name)| crate::mp_wait::Late { empire: e, name: name.into_bytes(), text: races(e) }).collect();
        let now = Instant::now();
        self.mp_wait.update(now, turn, paused, reconnecting, host, late);
        let at = self.virtual_cursor();
        self.mp_wait.hover(at, now);
        // `MOUSE_DrawOver` tests `Mouse_Gomb` (0xdb4650: left or right held) every pass, a level: a button held as the
        // items arm acts too.
        if self.mp_wait.is_open() && self.mouse_held & 3 != 0 {
            self.mp_wait_press();
        }
    }

    /// A press on the wait overlay (`MOUSE_DrawOver`): the host's "Close connection", "EXIT GAME".
    pub(super) fn mp_wait_press(&mut self) {
        let at = self.virtual_cursor();
        match self.mp_wait.press(at, Instant::now()) {
            Some(crate::mp_wait::Act::Close(e)) => self.world.drop_player(e),
            Some(crate::mp_wait::Act::Exit) => self.exit_to_main_menu(),
            None => {}
        }
    }

    /// The story's UI effects ([`crate::world::StoryUi`]): the storyline into the registry (written to
    /// `options.dat` at once; the exe writes it with the next `SaveGlobalRegistrySettings`), the colony screen asked
    /// for, the earthquake, the riot's pause, an automatic message's messages screen.
    pub(super) fn take_story_ui(&mut self) {
        use crate::world::StoryUi;
        for e in std::mem::take(&mut self.world.story_ui) {
            let p = match e {
                StoryUi::Storyline { campaign, value } => {
                    let key = format!("Fable_{campaign}");
                    self.settings.set(&key, &value.to_string());
                    crate::display::update_own_settings(|own| own.set(&key, &value.to_string()));
                    continue;
                }
                StoryUi::Earthquake(p) => {
                    self.quake = Some((p, None));
                    p
                }
                StoryUi::Hold(p) => {
                    self.send(StarmapOrder::Pause { paused: true });
                    p
                }
                StoryUi::Messages => {
                    // The exe sets `AktKepKell` over any pending change and from any screen; the port keeps a change
                    // under way and the battle screens (their leave paths aren't a plain request).
                    let s = self.akt_kep_now();
                    if s != 10 && s < 0xe && s != 11 && !(s == 3 && self.colony_war_up()) && self.kep_kell.is_none() {
                        self.request_screen(10);
                    }
                    continue;
                }
            };
            self.show_colony_screen(p);
        }
    }

    /// Ours (ADR 0034, re/rules/game-end.md §11): the game ended for this machine: its record is added to the match
    /// history and, unless it watches on as a spectator, shown once the end videos are over.
    pub(super) fn record_match(&mut self) {
        let Some(o) = self.world.game.over.filter(|_| self.display.statistics) else { return };
        let view = self.world.game.local_view();
        let (y, m, d) = self.world.curr.date();
        let when = web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |t| t.as_secs());
        let rec = crate::stats_screen::Match::record(&self.world.game, o, view.empire, self.world.net.is_some(), [y, m, d], when);
        if let Some(dir) = crate::stats_screen::history_dir() {
            if let Err(e) = crate::stats_screen::append(&dir, &rec) {
                eprintln!("ig2-client: match history: {e}");
            }
        }
        if !view.spectating {
            self.stats_end = Some(rec);
        }
    }

    /// The transfer panel's fleets to the sim (classic: the state's globals); multiplayer sends each change as a
    /// command, the panel counting only on the starmap (`AktKep == 2`, re/screens/starmap.md §10.2).
    pub(super) fn sync_transfer(&mut self) {
        let fleets = self.panels.transfer.map(|t| (t.f1, t.f2));
        // Only a change: a write of the same value still adds the byte to the state a replay's hash checks (ADR 0038).
        let m = &self.world.game.state.mem;
        let now = (m.u8(ig2_sim::save::va::STARMAP_TRANSFER_MODE) != 0).then(|| (m.i32(ig2_sim::save::va::STARMAP_TRANSFER_FLEETS.0), m.i32(ig2_sim::save::va::STARMAP_TRANSFER_FLEETS.1)));
        if now != fleets {
            self.world.game.set_transfer(fleets);
        }
        let fleets = fleets.filter(|_| self.shown_kep() == 2);
        if self.world.net.is_some() && fleets != self.sent_transfer {
            self.sent_transfer = fleets;
            self.world.send(Command::TransferPanel { fleets });
        }
    }

    /// Multiplayer: an ended rename's name (and a fleet's custom-name bit) as a command (deviation 395); the screen
    /// shows it until the lockstep has carried it.
    pub(super) fn send_renames(&mut self) {
        if self.world.net.is_none() || self.menus.rename.is_some() {
            return;
        }
        let mut out = Vec::new();
        for n in self.world.game.local_names.iter_mut().filter(|n| !n.sent) {
            n.sent = true;
            let name = n.shown.iter().take_while(|&&c| c != 0).copied().collect();
            out.push(Command::Rename { fleet: n.fleet, index: n.index as u32, name, custom: n.custom });
        }
        for c in out {
            self.world.send(c);
        }
    }

    /// Multiplayer: the session runs on under the screens that hold a single-player game
    /// ([`crate::world::World::run_on`]).
    pub(super) fn mp_run_on(&mut self) {
        let now = self.now_ms();
        self.world.run_on(&mut self.clock, now);
    }
}
