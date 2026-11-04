//! The starmap in the window: selection and orders, its presses and keys, the panels, the console, renames.

use super::*;

impl App {
    pub(super) fn multiplayer(&self) -> bool {
        self.world.game.state.mem.u8(ig2_sim::save::va::MULTIPLAYER) != 0
    }

    pub(super) fn speed_locked(&self) -> bool {
        self.world.game.state.mem.i32(ig2_sim::save::va::SPEED_LOCKED) != 0
    }

    /// The local selection (`StarmapMode`, `AktPlanet`, `AktFleet`).
    pub(super) fn selection(&self) -> Selection {
        let v = self.world.game.local_view();
        Selection { mode: v.starmap_mode, planet: v.planet.max(0) as usize, fleet: v.fleet.max(0) as usize }
    }

    pub(super) fn select(&mut self, s: Selection) {
        let v = self.world.game.local_view();
        self.world.game.set_view(LocalView { starmap_mode: s.mode, planet: s.planet as i32, fleet: s.fleet as i32, ..v });
        if let Some(view) = &mut self.view {
            view.selected = Some(if s.mode == 2 { Pick::Fleet(s.fleet) } else { Pick::Planet(s.planet) });
        }
    }

    /// Carry out what the panels asked for.
    pub(super) fn apply(&mut self, actions: Vec<Action>) {
        for a in actions {
            match a {
                Action::Send(o) => self.send(o),
                Action::Select(s) => self.select(s),
                Action::Sound(id) => self.sounds.push(id),
                Action::Tutorial(id) => self.world.game.tutorial_check_event(id),
                Action::ZoomIn { x, y, zoom } => {
                    if let (Some(v), None) = (&self.view, self.cam_saved) {
                        // `StarmapBeZoomol`: the zoom sound when not zoomed in yet (`StarmapZoomMode`).
                        self.sounds.push(crate::audio::SoundId::global(5));
                        self.cam_saved = Some([v.camera.x, v.camera.y, v.camera.zoom]);
                    }
                    self.tween_to([x, -y, zoom], 8);
                }
                Action::ZoomOut => {
                    if let Some(s) = self.cam_saved.take() {
                        // `StarmapKiZoomol`'s sound.
                        self.sounds.push(crate::audio::SoundId::global(5));
                        self.tween_to(s, 8);
                    }
                }
                Action::Design(o) => self.world.send(Command::from(o)),
                // The exe calls `Story_Special` itself, in single player only (no message).
                Action::HeroBack { fleet, design } => {
                    if !self.multiplayer() {
                        self.world.send(Command::HeroSentBack { fleet, design });
                    }
                }
                Action::StoryIcon { icon, target, click } => {
                    if icon == 0x12 || !self.multiplayer() {
                        if click {
                            self.sounds.push(crate::audio::CLICK);
                        }
                        self.world.send(Command::StoryIcon { icon, target });
                    }
                }
                Action::Centre { x, y } => {
                    let zoom = self.camera3()[2];
                    self.tween_to([x, -y, zoom], 8);
                }
            }
        }
    }

    /// Carry out what the menus asked for.
    pub(super) fn apply_do(&mut self, dos: Vec<Do>) {
        let sel = self.selection();
        for d in dos {
            let acts = match d {
                Do::Act(a) => vec![a],
                // The colony icon (`AktRopIkon == 4`), not while `KepernyokLetiltva[3]` (exe 0x5b5af4).
                Do::Colony => {
                    if self.colony.is_none() && !self.tutorial_screen_off(3) {
                        self.toggle_colony();
                    }
                    Vec::new()
                }
                Do::Production => self.panels.toggle_production(&self.world.game.state, &sel),
                Do::Tanks => self.panels.tanks_button(&self.world.game.state, &sel),
                Do::Join => self.panels.fleet_transfer(&self.world.game.state, &sel),
                Do::TanksInfo => {
                    self.panels.open_tanks_info(self.sale_names());
                    Vec::new()
                }
                Do::FleetInfo => self.fleet_info(),
            };
            self.apply(acts);
        }
    }

    /// `TargetXStart/YStart/Zoom` = `t` over `frames` frames (`Mozgasd`), `TargetRotate/Pitch` the current turn and
    /// tilt (as every caller but the console's views sets them).
    pub(super) fn tween_to(&mut self, t: [f32; 3], frames: i32) {
        if let Some(v) = &self.view {
            self.cam_tween = Some(([t[0], t[1], t[2], v.camera.rotate, v.camera.pitch], frames));
        }
    }

    /// The camera as `[XStart, YStart, Zoom]`.
    pub(super) fn camera3(&self) -> [f32; 3] {
        self.view.as_ref().map_or([0.0; 3], |v| [v.camera.x, v.camera.y, v.camera.zoom])
    }

    /// `ScrollHibaUzenet` (exe 0x556b20): `PlaySoundGlobal(6)`, the error text `HibaUzenetSzoveg` = `text`, its counter 0.
    pub(super) fn scroll_error(&mut self, text: Vec<u8>) {
        self.audio.play(crate::audio::SoundId::global(6));
        if let Some(h) = &mut self.hud {
            h.error = Some((text, 0));
        }
    }

    /// The sale menus' texts (`DesignTorlesValasztasNev`).
    pub(super) fn sale_names(&self) -> crate::starmap_info::SaleNames {
        let messages = &self.world.game.rules.messages;
        self.hud.as_ref().map(|h| crate::starmap_info::SaleNames::load(&h.texts, |n| messages.story_text(n).to_vec())).unwrap_or_default()
    }

    /// The info box's left click in fleet mode (`AktRopIkon` 0xf): the fleet's info panel toggles (§19.1).
    pub(super) fn fleet_info(&mut self) -> Vec<Action> {
        let sel = self.selection();
        self.cam_tween = None;
        let from = self.cam_saved.map_or(self.camera3()[2], |s| s[2]);
        let names = self.sale_names();
        self.panels.info_box_fleet(&self.world.game.state, &sel, names, from)
    }

    /// A left press on the starmap (`Starmap_Main`): the long info's hot-spots, the panels', the production view's
    /// planet list, then the map: the redirect pick, or a pick that becomes the selection and opens the radial menu.
    pub(super) fn starmap_press(&mut self) -> bool {
        let at = self.starmap_virtual(self.virtual_cursor());
        let sel = self.selection();
        // The tutorial's DISABLE STARMAP: `Starmap_MouseInit2` (exe 0x5b898f) switches every hot-spot off and only
        // 1..=7 back on (the console and the minimap; not under the long info): no picks, panels or info box.
        if self.starmap_locked() {
            return self.console_press(true) || at[1] >= 20.0;
        }
        if self.menus.info_on() {
            // Hot-spots 8 (left), 9 and 11 leave; the panel's buttons and menus; the rest of the screen is caught.
            // Under the modern HUD the info box and the production button are its card's (`modern_apply`).
            let classic = !self.modern_up();
            if (classic && at[0] < 166.0 && at[1] >= 424.0) || crate::starmap_menus::exit_spot(at) {
                self.leave_long_info(false);
            } else if classic && (422.0..466.0).contains(&at[0]) && (432.0..476.0).contains(&at[1]) {
                self.leave_long_info(true);
            } else if let Some(dos) = self.menus.press_info(&self.world.game.state, at) {
                self.apply_do(dos);
            }
            return true;
        }
        if self.panels.product && (276.0..363.0).contains(&at[0]) && (32.0..79.0).contains(&at[1]) {
            let dos = self.menus.open_menu(&self.world.game.state, MenuKind::Planets, at);
            self.apply_do(dos);
            return true;
        }
        // Hot-spots 1..=7 are the lowest ids: they win over the panels' and the map's (`Mouse_Darabol`).
        if self.console_press(true) {
            return true;
        }
        if self.panel_press(true) {
            self.panel_held = Some(true);
            return true;
        }
        // The rest of the top bar, the info box and the minimap act on the release (not under the modern HUD, ADR 0065).
        let classic = !self.modern_up();
        if (classic && (at[1] < 20.0 || (at[0] < 166.0 && at[1] >= 424.0))) || (self.panels.transfer.is_some() && at[1] >= 20.0) {
            return false;
        }
        // With an info panel up the map takes no picks (hot-spot 0x35 is off).
        if self.panels.info.is_some() {
            return true;
        }
        let Some(v) = &self.view else { return false };
        if self.menus.redirect {
            self.sounds.push(crate::audio::CLICK);
            let target = orders::select_nearest_planet(&self.world.curr, &v.camera, at, 1.0);
            let dos = self.menus.redirect_to(&self.world.game.state, &sel, target);
            self.apply_do(dos);
            return true;
        }
        let pick = if self.panels.production_open() {
            orders::select_nearest_planet(&self.world.curr, &v.camera, at, 1.0).map(|p| Selection { mode: 1, planet: p, ..sel })
        } else {
            orders::left_click(&self.world.curr, &v.camera, at, &sel)
        };
        if let Some(s) = pick {
            self.select(s);
            let s = self.selection();
            // `Starmap_Main` (ELF 0x6f8e3b): a fleet of the player's picked speaks its line's select.
            if s.mode == 2 && self.world.game.state.fleets.get(s.fleet).is_some_and(|f| f[0x15] == self.world.curr.human) {
                self.speech.push(crate::audio::speech::Speak { who: crate::audio::speech::Who::Starmap, line: s.fleet as i32 % 3 + 1, kind: crate::audio::speech::starmap::SELECT });
            }
            let blink = (self.now_ms() / 100) % 2 == 1;
            let radar = self.world.game.en_radar_range();
            let places = crate::starmap_menus::rop_places(&self.world.game.state, &s, &self.panels, self.menus.redirect, (self.menus.sat_sent, self.menus.col_sent), blink, radar);
            self.menus.open_rop(at, places);
        }
        true
    }

    /// Leave the long info (hot-spot 8, 9, or 11 with `production`): the tutorial's event 0x16 (exe 0x5b3035), the
    /// zoom sound (exe 0x5b303c), the camera flies back.
    pub(super) fn leave_long_info(&mut self, production: bool) {
        // Once: the original's fly out is a blocking loop that reads no input.
        if self.menus.info.is_some_and(|i| matches!(i.phase, crate::starmap_menus::InfoPhase::Leaving(_))) {
            return;
        }
        self.world.game.tutorial_check_event(0x16);
        self.sounds.push(crate::audio::SoundId::global(5));
        let cam = self.camera3();
        self.menus.leave_info(cam, production);
    }

    /// The left button went up after [`Self::starmap_press`]: an open select menu sends, the radial menu fires its
    /// icon.
    pub(super) fn starmap_release(&mut self) {
        // Also under DISABLE STARMAP (hot-spots off, exe 0x5b898f): an open menu holds `Mouse_Megfogva`, so
        // `IG2_gameUpdate` keeps the pressed hot-spot as `AktIkon` (exe 0x4e0440, no `Mouse_Darabol` while held) and
        // `Starmap_Main`'s release arm still runs.
        let sel = self.selection();
        if self.menus.menu.is_some() {
            let dos = self.menus.release_menu(&self.world.game.state, &sel);
            self.apply_do(dos);
        }
        if self.menus.rop.is_some() {
            let mut icon = self.menus.release_rop();
            // `Starmap_Main`'s production (2) and split (0xd) arms wait for the tutorial's gates.
            let locks = self.panels.locks;
            if (icon == 2 && locks.production) || (icon == 0xd && locks.split) {
                icon = 0;
            }
            let dos = self.menus.fire(icon, &self.world.game.state, &sel);
            self.apply_do(dos);
        }
    }

    /// The pointer in virtual pixels.
    pub(super) fn virtual_cursor(&self) -> [f32; 2] {
        self.view.as_ref().map_or([0.0; 2], |v| v.camera.virtual_of(self.cursor))
    }

    /// Ours (ADR 0079): under the modern HUD the long info's controls lie elsewhere; the virtual point `at` as its
    /// hot-spots see it.
    /// Ours (ADR 0077): so are the fleet and transfer panels'.
    pub(super) fn starmap_virtual(&self, at: [f32; 2]) -> [f32; 2] {
        match () {
            _ if self.modern_up() && self.modern.list_open() => self.modern.list_pointer(at),
            _ if self.modern_up() && self.menus.info_on() => self.modern.info_pointer(at),
            _ if self.fleet_modern_up() => self.fleet_modern.pointer(at),
            _ => at,
        }
    }

    /// `TutorialStarmapMindenLetiltvaKiveveAKurzorMozgatoBillentyuket` (exe 0xf66ff8, DISABLE STARMAP): read only by
    /// `Starmap_MouseInit2` (exe 0x5b898f) and `StarmapKeyboardHandler` (exe 0x5aed29).
    pub(super) fn starmap_locked(&self) -> bool {
        self.tutorial.as_ref().is_some_and(|t| t.t.gates.starmap)
    }

    /// A press on the starmap: the panels' hot-spots take it first (none under the tutorial's starmap lock).
    pub(super) fn panel_press(&mut self, left: bool) -> bool {
        if self.starmap_locked() {
            return false;
        }
        let at = self.starmap_virtual(self.virtual_cursor());
        let sel = self.selection();
        // Under the modern HUD the info box's spots (8, the tanks' 10, production's 11) are its card's (ADR 0065).
        if self.modern_up() && self.panels.spot(&self.world.game.state, &sel, at).is_some_and(|id| matches!(id, 8 | 10 | 11)) {
            return false;
        }
        match self.panels.press(&self.world.game.state, &sel, at, left) {
            Some(actions) => {
                self.apply(actions);
                true
            }
            None => false,
        }
    }

    /// The info box (hot-spot 8) clicked with the right button: the planet's colony screen, or the fleet splits
    /// (`Multi_Send(0x14)`, after closing a transfer panel; a fleet-to-fleet panel closing doesn't split).
    pub(super) fn info_box_click(&mut self) {
        let sel = self.selection();
        if sel.mode == 1 {
            // Hot-spot 8 in planet mode: the zoom sound before the branch (exe 0x5b6373).
            self.sounds.push(crate::audio::SoundId::global(5));
            // Not while `KepernyokLetiltva[3]` (exe 0x5b64da).
            if !self.tutorial_screen_off(3) {
                self.toggle_colony();
            }
            return;
        }
        let state = &self.world.game.state;
        let Some(r) = state.fleets.get(sel.fleet) else { return };
        // Not for `fSpec` fleets (flags 0x4400, `Starmap_Main`, re/screens/starmap.md §19.1).
        if u16::from_le_bytes([r[0x52], r[0x53]]) & 0x4400 != 0 {
            return;
        }
        // Fleet mode clicks (exe 0x5b6a07), again when it closes a transfer panel (0x5b6b99).
        self.sounds.push(crate::audio::CLICK);
        if self.panels.transfer.is_some() {
            self.sounds.push(crate::audio::CLICK);
        }
        let f = |o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let (mine, calm, pos) = (r[0x15] == self.world.curr.human, r[0x52] & 1 == 0, (f(0x30), f(0x34)));
        // The fleet's info panel goes without the zoom-out; the split then keeps the zoom (§19.1).
        let had_info = self.panels.info.take().is_some();
        let closed = self.panels.transfer;
        let acts = self.panels.close_transfer(state, &sel, true);
        self.apply(acts);
        // `TutorialSplitDisabled`'s second site (exe 0x5b6d90).
        if mine && calm && closed.is_none_or(|t| t.tank_mode) && !self.panels.locks.split {
            if closed.is_none() && !had_info {
                self.apply(vec![Action::ZoomIn { x: pos.0, y: pos.1, zoom: 590.0 }]);
            }
            self.send(StarmapOrder::Split { fleet: sel.fleet as i32 });
        }
    }

    /// The info box clicked with the left button: the fleet's info panel, or in planet mode the long info.
    pub(super) fn info_box_left(&mut self) {
        let sel = self.selection();
        if sel.mode == 2 {
            let acts = self.fleet_info();
            self.apply(acts);
        } else if sel.mode == 1 && sel.planet > 0 {
            self.sounds.push(crate::audio::SoundId::global(5));
            self.panels.info = None;
            self.panels.close_production();
            let acts = self.panels.close_transfer(&self.world.game.state, &sel, false);
            self.apply(acts);
            self.cam_tween = None;
            let cam = self.camera3();
            self.menus.enter_info(sel.planet, cam);
        }
    }

    /// A click on the starmap screen at physical `p` (left: buttons, minimap, selection; right: an order).
    /// The starmap's info box (hot-spot 8 at (0, 424, 166, 56)) under physical `p` (none under the modern HUD).
    pub(super) fn on_info_box(&self, p: [f32; 2]) -> bool {
        !self.modern_up() && self.view.as_ref().is_some_and(|v| {
            let at = v.camera.virtual_of(p);
            at[0] >= 0.0 && at[0] < 166.0 && at[1] >= 424.0
        })
    }

    pub(super) fn starmap_click(&mut self, p: [f32; 2], right: bool) {
        if self.starmap_locked() {
            return;
        }
        let Some(v) = &self.view else { return };
        let at = v.camera.virtual_of(p);
        let keys = Keys { shift: self.modifiers.shift_key(), ctrl: self.modifiers.control_key(), alt: self.modifiers.alt_key() };
        let sel = self.selection();
        if self.menus.info_on() {
            return;
        }
        // The info box (hot-spot 8 at (0, 424, 166, 56)): right = colony / split, left in planet mode = the long info.
        if at[0] >= 0.0 && at[0] < 166.0 && at[1] >= 424.0 && !self.modern_up() {
            if right {
                self.info_box_click();
            } else {
                self.info_box_left();
            }
            return;
        }
        // The transfer panel covers the map (hot-spot 0x58); under an info panel the map takes no orders.
        if (self.panels.transfer.is_some() || self.panels.info.is_some()) && at[1] >= 20.0 {
            return;
        }
        if right {
            // In planet mode on the player's planet the right button picks the rally planet (`0x400`), redirect mode or
            // not; in fleet mode the redirect mode gives way to the fleet's orders (`Starmap_Main` exe 0x5b53e2).
            let own = sel.mode == 1 && self.world.curr.planets.iter().any(|p| p.index == sel.planet && p.owner == self.world.curr.human);
            if own {
                // The rally pick clicks before it picks (exe 0x5b5406).
                self.sounds.push(crate::audio::CLICK);
                let target = orders::select_nearest_planet(&self.world.curr, &v.camera, at, 1.0);
                let dos = self.menus.redirect_to(&self.world.game.state, &sel, target);
                self.apply_do(dos);
            } else if let Some(o) = orders::right_click(&self.world.curr, &v.camera, at, &sel, keys) {
                // `Starmap_Main` (exe 0x5b4ca6 / 0x5b4d11): an enemy planet or fleet picked for the attack (no Ctrl, no
                // Alt) raises the tutorial's 0x11, then 0x1f.
                if matches!(o, StarmapOrder::Move { order: 2, kind: 1 | 2, .. }) && !keys.ctrl {
                    self.world.game.tutorial_check_event(0x11);
                    self.world.game.tutorial_check_event(0x1f);
                }
                // An own fleet's order (`StarmapVillogasMode` 1, 0xd, 0xe): a click with Shift (exe 0x5b51b3), else the
                // fleet's move line; an attack (2): its attack line (exe 0x5b5231).
                let own_fleet = sel.mode == 2 && self.world.game.state.fleets.get(sel.fleet).is_some_and(|f| f[0x15] == self.world.curr.human);
                if let (true, StarmapOrder::Move { order, .. }) = (own_fleet, &o) {
                    use crate::audio::speech::{starmap, Speak, Who};
                    let line = sel.fleet as i32 % 3 + 1;
                    match *order {
                        1 | 0xd | 0xe if keys.shift => self.sounds.push(crate::audio::CLICK),
                        1 | 0xd | 0xe => self.speech.push(Speak { who: Who::Starmap, line, kind: starmap::MOVE }),
                        2 => self.speech.push(Speak { who: Who::Starmap, line, kind: starmap::ATTACK }),
                        _ => {}
                    }
                }
                self.send(o);
            }
        }
        // The console and the minimap (hot-spots 1..=7) are `console_press`'s.
    }

    /// The console's and the minimap's hot-spot (1..=7) under virtual `at`, while they are on (not under the long
    /// info, the production view, the transfer or the fleet's panel): `Mouse_Darabol` reads `STAR_M.RAW`'s colour
    /// there, their rectangles ignored (re/screens/starmap.md §10.3).
    pub(super) fn console_spot(&mut self, at: [f32; 2]) -> Option<u32> {
        if self.panels.minimap_off() || self.menus.info_on() || self.modern_up() {
            return None;
        }
        let install = &self.install;
        let mask = self.star_mask.get_or_insert_with(|| install.read("dataraw/screen/star_m.raw").ok().and_then(|b| b.get(800..800 + 640 * 480).map(|x| x.to_vec())).unwrap_or_default());
        let (x, y) = (at[0].clamp(0.0, 639.0) as usize, at[1].clamp(0.0, 479.0) as usize);
        crate::orders::console_spot(*mask.get(y * 640 + x)?)
    }

    /// A press on the console or the minimap (`Starmap_Main` ELF 0x6f50a0 lines 163-285): the held buttons set the
    /// camera flags (rotate; zoom and pitch in with the left button, out with the right), the minimap retargets
    /// the view; the views (1, 4) act on the release. True when taken.
    pub(super) fn console_press(&mut self, left: bool) -> bool {
        let at = self.virtual_cursor();
        let Some(s) = self.console_spot(at) else { return false };
        self.console_act(s, left, at);
        self.console_held = Some((s, left));
        true
    }

    /// Console spot `s` pressed (`left`: with the left button; the minimap's at virtual `at`).
    pub(super) fn console_act(&mut self, s: u32, left: bool, at: [f32; 2]) {
        let k = &mut self.star_keys;
        match s {
            2 => (k.right, k.left) = (true, false),
            3 => (k.left, k.right) = (true, false),
            5 => (k.zoom_in, k.zoom_out) = (left, !left),
            6 => (k.up, k.down) = (left, !left),
            7 => {
                let t = orders::minimap_target(at);
                let zoom = self.camera3()[2];
                self.tween_to([t[0], t[1], zoom], 4);
            }
            _ => {}
        }
        if s != 1 && s != 4 {
            self.sounds.push(crate::audio::CLICK);
            self.star_buttons.press(s);
        }
    }

    /// The button went up over a held console spot: its flags clear; the top view (1) and the side view (4) start
    /// when released over their button (`StarmapSetTopView`, `StarmapSetSideView`, 8 frames).
    pub(super) fn console_release(&mut self) {
        let Some((s, _)) = self.console_held.take() else { return };
        self.console_let_go(s);
        let at = self.virtual_cursor();
        if !matches!(s, 1 | 4) || self.console_spot(at) != Some(s) {
            return;
        }
        self.console_view(s);
    }

    /// A held console spot let go: its camera flags clear.
    pub(super) fn console_let_go(&mut self, s: u32) {
        self.star_buttons.release();
        let k = &mut self.star_keys;
        match s {
            2 | 3 => (k.left, k.right) = (false, false),
            5 => (k.zoom_in, k.zoom_out) = (false, false),
            6 => (k.up, k.down) = (false, false),
            _ => {}
        }
    }

    /// The top view (1) or the side view (4) released over its button.
    pub(super) fn console_view(&mut self, s: u32) {
        self.sounds.push(crate::audio::CLICK);
        // The views' hot-spots act on the release (their `Mouse_Szoveg_Set` flag): the button is set, then let go.
        self.star_buttons.press(s);
        let Some(v) = &self.view else { return };
        let c = v.camera;
        if s == 1 {
            if let (t, true) = c.top_view(&self.world.curr) {
                self.cam_tween = Some((t, 8));
            }
        } else {
            let known = self.world.curr.planets.iter().any(|p| p.known);
            let sv = crate::starmap::Camera::side_view(&self.world.curr, c.size);
            let t = if known { [sv.x, sv.y, sv.zoom] } else { [c.x, c.y, c.zoom] };
            self.cam_tween = Some(([t[0], t[1], t[2], 0.0, -0.95], 8));
        }
    }

    /// ADR 0030: W/A/S/D held pan the starmap's camera (and with the option's third state the colony's) through the
    /// edge scroll; a key down taken here never reaches the screen (the colony's `a` and `s`). The key up is handled
    /// before the screens (`window_event`). True when taken.
    pub(super) fn wasd_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        let Some(i) = do_keyboard::wasd_key(event.physical_key) else { return false };
        if event.state != ElementState::Pressed {
            return false;
        }
        let shown = self.spacewar.is_none() && !self.under_screen() && self.options.is_none();
        let starmap = shown && self.view.is_some() && self.colony.is_none();
        let colony = shown && self.colony.is_some();
        if !do_keyboard::wasd_pans(self.display.wasd, starmap, colony, self.entry_up()) {
            return false;
        }
        match &mut self.colony {
            Some(c) => c.ui.cam_keys.pan[i] = true,
            None => self.star_keys.pan[i] = true,
        }
        true
    }

    /// ADR 0035: on the starmap Ctrl+1..9 binds the selected own fleet to the group, Alt+1..9 selects the group's fleet
    /// and, when it is already selected, centres the camera on it. Taken (never reaching `DoKeyboard`'s speed keys) only
    /// with the option on and the starmap free, as PgUp/PgDn's step (re/screens/starmap.md §10.6). True when taken.
    pub(super) fn fleet_group_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        use crate::fleet_groups::GroupKey;
        use crate::starmap_menus::InfoPhase;
        let m = self.modifiers;
        let Some(k) = crate::fleet_groups::group_key(event.physical_key, m.control_key(), m.alt_key()) else { return false };
        if !self.display.fleet_groups || self.view.is_none() || self.colony.is_some() || self.spacewar.is_some() || self.under_screen()
            || self.options.is_some() || self.entry_up() || self.starmap_locked()
        {
            return false;
        }
        if event.state != ElementState::Pressed {
            return true;
        }
        let blocked = self.menus.menu.is_some() || self.menus.info.is_some_and(|i| i.phase != InfoPhase::Up) || self.panels.fleet_info()
            || self.panels.transfer.is_some();
        if blocked {
            return true;
        }
        let human = self.world.curr.human;
        let sel = self.selection();
        match k {
            GroupKey::Store(g) => {
                if sel.mode == 2 && self.world.game.state.fleets.get(sel.fleet).is_some_and(|f| f[0x15] == human) {
                    self.fleet_groups.0[g] = sel.fleet;
                    self.sounds.push(crate::audio::CLICK);
                }
            }
            GroupKey::Recall(g) => {
                let f = self.fleet_groups.0[g];
                // shortcut: a freed slot reused by a new fleet of the human's recalls that fleet; a fleet id + birth
                // tick would tell them apart if players notice.
                let Some(r) = self.world.game.state.fleets.get(f).filter(|r| f > 0 && r[0x15] == human) else { return true };
                let at = [f32::from_le_bytes(r[0x30..0x34].try_into().unwrap()), f32::from_le_bytes(r[0x34..0x38].try_into().unwrap())];
                if sel.mode == 2 && sel.fleet == f {
                    self.apply(vec![Action::Centre { x: at[0], y: at[1] }]);
                } else {
                    self.sounds.push(crate::audio::CLICK);
                    self.select(Selection { mode: 2, fleet: f, ..sel });
                }
            }
        }
        true
    }

    /// `StarmapKeyboardHandler` (ELF 0x704580, exe 0x5aeab0; re/screens/starmap.md §10.6) on the starmap itself, down
    /// and up: the held camera keys (not under the long info, the zoom or the fleet's panel), the fleet panel's arrows,
    /// PgUp/PgDn and `c` (not while the tutorial locks the starmap). True when taken.
    pub(super) fn starmap_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        self.starmap_key_by(event.physical_key, event.logical_key.as_ref(), event.state == ElementState::Pressed)
    }

    pub(super) fn starmap_key_by(&mut self, physical: winit::keyboard::PhysicalKey, logical: Key<&str>, down: bool) -> bool {
        use crate::starmap::CamKey as K;
        use winit::keyboard::{KeyCode, PhysicalKey};
        // A key up reaches the handler with the rename's entry up too (`DoKeyboard` line 186).
        if self.view.is_none() || self.colony.is_some() || self.under_screen() || (self.menus.rename.is_some() && down) {
            return false;
        }
        // VK 0x25..=0x28 by the logical.clone() key (a NumLock-off numpad sends them too), numpad + / − by the physical one.
        let vk = match (physical, logical.clone()) {
            (PhysicalKey::Code(KeyCode::NumpadAdd), _) => Some(K::ZoomIn),
            (PhysicalKey::Code(KeyCode::NumpadSubtract), _) => Some(K::ZoomOut),
            (_, Key::Named(NamedKey::ArrowLeft)) => Some(K::Left),
            (_, Key::Named(NamedKey::ArrowUp)) => Some(K::Up),
            (_, Key::Named(NamedKey::ArrowRight)) => Some(K::Right),
            (_, Key::Named(NamedKey::ArrowDown)) => Some(K::Down),
            _ => None,
        };
        // The characters `=` and `-` (a numpad − gives its `-` after the key's own flag, which then ignores it).
        let ch = match (logical.clone(), down) {
            (Key::Character("="), true) => Some(K::StepIn),
            (Key::Character("-"), true) if vk.is_none() => Some(K::StepOut),
            _ => None,
        };
        let fleet_panel = self.panels.fleet_info();
        let gate = !self.menus.info_on() && self.cam_saved.is_none() && !fleet_panel;
        if let Some(k) = vk.or(ch) {
            if gate {
                self.star_keys.key(k, down);
            }
            if fleet_panel && down {
                let step = match k {
                    K::Up => -2,
                    K::Down => 2,
                    K::Left => -1,
                    K::Right => 1,
                    _ => 0,
                };
                let sel = self.selection();
                if let (true, Some(i)) = (step != 0, &mut self.panels.info) {
                    i.key(&self.world.game.state, &sel, step);
                }
            }
            return true;
        }
        if !down || self.starmap_locked() {
            return false;
        }
        match logical.clone() {
            Key::Named(k @ (NamedKey::PageUp | NamedKey::PageDown)) => {
                // `remapKeyCodes` swaps them: PgUp comes as vk 0x22, the step on (FX5).
                self.starmap_step(k == NamedKey::PageUp);
                true
            }
            Key::Character("c") => {
                self.starmap_colonise();
                true
            }
            _ => false,
        }
    }

    /// PgUp / PgDn on the starmap: the previous / next own planet or fleet, centred; not with a select menu, the
    /// long info flying, the fleet's panel or a transfer up (re/screens/starmap.md §10.6).
    pub(super) fn starmap_step(&mut self, next: bool) {
        use crate::starmap_menus::InfoPhase;
        let blocked = self.menus.menu.as_ref().is_some_and(|m| matches!(m.kind, MenuKind::Tax | MenuKind::AutoBuild | MenuKind::Priority | MenuKind::Sale(_)))
            || self.menus.info.is_some_and(|i| i.phase != InfoPhase::Up)
            || self.panels.fleet_info()
            || self.panels.transfer.is_some();
        let sel = self.selection();
        if blocked {
            return;
        }
        let Some((s, at)) = crate::starmap_panels::step_selection(&self.world.game.state, &sel, next, self.modifiers.shift_key()) else { return };
        // The long info follows the planet (its `StarSize` swap).
        if let (1, Some(i)) = (s.mode, &mut self.menus.info) {
            i.planet = s.planet;
        }
        self.select(s);
        self.apply(vec![Action::Centre { x: at[0], y: at[1] }]);
        if s.mode == 1 {
            if let Some(i) = self.panels.info.as_mut().filter(|i| i.kind == crate::starmap_info::Kind::Tanks) {
                (i.first, i.selected) = (0, 0);
            }
        }
        // `DeleteMenus`.
        self.menus.menu = None;
        if let Some(i) = &mut self.panels.info {
            i.sale = None;
        }
    }

    /// Both buttons down over the info box (hot-spot 8): the rename of the shown planet (any) or the human's fleet (not
    /// `fSpec`) starts (`Starmap_Main`, re/screens/starmap.md §10.7; a planet's zoom sound, a fleet's click).
    pub(super) fn start_rename(&mut self) {
        let sel = self.selection();
        let state = &self.world.game.state;
        let human = self.world.curr.human;
        let r = match sel.mode {
            1 if sel.planet >= 1 && sel.planet <= state.planet_count() => {
                self.sounds.push(crate::audio::SoundId::global(5));
                let r = state.mem.read(ig2_sim::save::va::PLANETS + (sel.planet * ig2_sim::save::PLANET_SIZE) as u32, 0x18);
                crate::starmap_menus::Rename::new(false, sel.planet, &r[..r.iter().position(|&c| c == 0).unwrap_or(r.len())])
            }
            2 => {
                let Some(f) = state.fleets.get(sel.fleet).filter(|f| f[0x15] == human && u16::from_le_bytes([f[0x52], f[0x53]]) & 0x4400 == 0) else { return };
                self.sounds.push(crate::audio::CLICK);
                crate::starmap_menus::Rename::new(true, sel.fleet, &f[..f.iter().take(0x14).position(|&c| c == 0).unwrap_or(0x14)])
            }
            _ => return,
        };
        // A fleet's rename sets its custom-name bit for good (`+0x52 |= 0x800`, `Starmap_Main` exe 0x5b62be / 0x5b6cf4;
        // Esc or a press elsewhere keeps it): `Show_Flottak2d` draws its name on the map from then on. Only that draw
        // and the Elem story read it, so the local edit (no message, as the name's) can't desync.
        if r.fleet {
            if self.world.net.is_some() {
                // Multiplayer: shown here at once, the bit and the name go through the lockstep when the rename ends
                // (deviation 395).
                let name = r.backup.clone();
                self.world.game.edit_local_name(true, r.index, &name, true);
            } else if let Some(f) = self.world.game.state.fleets.get_mut(r.index) {
                f[0x53] |= 0x08;
            }
        }
        self.menus.rename = Some(r);
    }

    /// The rename's text written into the record (`IOString` is the record's name).
    pub(super) fn write_rename(&mut self, text: &[u8]) {
        let Some(r) = &self.menus.rename else { return };
        // Multiplayer: the shared state isn't written outside the lockstep; the screen shows the typed name and the
        // rename's end sends it (`send_renames`, deviation 395).
        if self.world.net.is_some() {
            let (fleet, index) = (r.fleet, r.index);
            self.world.game.edit_local_name(fleet, index, text, fleet);
            return;
        }
        let state = &mut self.world.game.state;
        let mut name = text.to_vec();
        name.push(0);
        if r.fleet {
            if let Some(f) = state.fleets.get_mut(r.index) {
                f[..name.len()].copy_from_slice(&name);
            }
        } else {
            state.mem.write(ig2_sim::save::va::PLANETS + (r.index * ig2_sim::save::PLANET_SIZE) as u32, &name);
        }
    }

    /// A key while the rename is up (`DoKeyboard`'s editing branch): Enter ends it (the tutorial's events 4 / 3),
    /// Esc restores the old name.
    pub(super) fn rename_key(&mut self, k: &Key<&str>) {
        use crate::starmap_menus::RenameKey as R;
        let key = match k {
            Key::Named(NamedKey::Enter) => {
                // `DoKeyboard` (exe 0x4db9a7 / 0x4db9c0, `AktKep` 2): the tutorial's 4 for a fleet (`IOAzonosito` 0x16),
                // 3 for a planet (0x15), then `IOMode = 0`; on the colony screen (`AktKep` 3, 0x1f) only the end.
                if let Some(r) = self.menus.rename.take().filter(|_| self.colony.is_none()) {
                    self.world.game.tutorial_check_event(if r.fleet { 4 } else { 3 });
                }
                return;
            }
            // `DoKeyboard` case 0x1b (exe 0x4db1a0): nothing while a screen change is pending (`Belep`, `Atlepes`,
            // `AktKepKell`).
            Key::Named(NamedKey::Escape) if self.screen_changing() => return,
            Key::Named(NamedKey::Escape) => {
                if let Some(b) = self.menus.rename.as_ref().map(|r| r.backup.clone()) {
                    self.write_rename(&b);
                }
                self.menus.rename = None;
                return;
            }
            Key::Named(NamedKey::Backspace) => R::Back,
            Key::Named(NamedKey::Delete) => R::Delete,
            Key::Named(NamedKey::Home) => R::Home,
            Key::Named(NamedKey::End) => R::End,
            Key::Named(NamedKey::ArrowLeft) => R::Left,
            Key::Named(NamedKey::ArrowRight) => R::Right,
            Key::Named(NamedKey::Space) => R::Char(b' '),
            Key::Character(c) => {
                let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                match b.first().copied().filter(|&b| b >= 0x20 && b != 0x81) {
                    Some(b) => R::Char(b),
                    None => return,
                }
            }
            _ => return,
        };
        let Some(r) = &mut self.menus.rename else { return };
        r.key(key);
        let text = r.text.clone();
        self.write_rename(&text);
    }

    /// `DesignKeret1`'s phase is one set of globals (`dkm` 0xB34C0C, `dkmcel` 0x903548, `GfxNovelo` 0xB34BAC) that
    /// `Frissit2_Grafika` steps every UI frame and no screen resets; only the frame's places reset on a screen change.
    /// The shown screen's frame carries it; a newly shown one starts from it.
    pub(super) fn row_frame_phase(&mut self, due_ui: u64) {
        if self.front.is_some() {
            return;
        }
        use crate::ui::Screen;
        let (owner, frame) = if let Some(o) = &mut self.options {
            (1, Some(&mut o.s.design))
        } else if let (Some(Screen::Design), Some(d)) = (self.overlay, &mut self.design) {
            (2, Some(&mut d.list_frame))
        } else if let (Some(Screen::Trade), Some(t)) = (self.overlay, &mut self.trade) {
            (3, Some(&mut t.list_frame))
        } else if let (Some(Screen::Research), Some(r)) = (self.overlay, &mut self.research) {
            (4, Some(&mut r.design))
        } else if let (None, None, Some(i)) = (self.overlay, &self.colony, &mut self.panels.info) {
            (5, Some(&mut i.frame))
        } else {
            (0, None)
        };
        carry_row_frame(&mut self.dk, &mut self.dk_owner, owner, frame, due_ui);
    }

    /// `c` on the starmap (character 99): a free planet, a colony ship in the pool, none sent this frame or on the
    /// way, and no -0x2094 info: the colonise order 0x16 (kind 1).
    pub(super) fn starmap_colonise(&mut self) {
        let sel = self.selection();
        let state = &self.world.game.state;
        if sel.mode != 1 || self.menus.col_sent || !crate::starmap_menus::can_send_colony_ship(state, sel.planet) {
            return;
        }
        self.send(StarmapOrder::Colonise { planet: sel.planet as i32, kind: 1 });
        self.menus.col_sent = true;
    }
}
