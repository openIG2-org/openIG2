//! The colony screen in the window: opening and leaving it, its input, camera keys and renames.

use super::*;

impl App {
    /// Open the colony screen of `AktPlanet`, or close it.
    pub(super) fn toggle_colony(&mut self) {
        if self.colony.take().is_some() {
            self.world.show_colony(None);
            return;
        }
        if !self.colony_tutorial_gate() {
            return;
        }
        if let Some(p) = self.colony_target() {
            self.open_colony(p);
        }
    }

    /// The planet the colony screen opens on: `AktPlanet` ([`Self::akt_planet`]; `Colony_Belep` exe 0x465b40 reads
    /// 0xb9933c from 0x465bb9 on, so after a fleet pick the planet picked or shown before it; re/deviations.md row
    /// 342).
    pub(super) fn colony_target(&mut self) -> Option<usize> {
        let p = self.akt_planet();
        self.world.curr.planets.iter().any(|q| q.index == p).then_some(p)
    }

    /// `AktPlanet` (exe 0xb9933c): the colony shown, else the planet last picked on the starmap (`Starmap_Main` exe
    /// 0x5b2b70 sets it on a planet pick, a fleet pick leaves it) or shown, else the saved one.
    pub(super) fn akt_planet(&mut self) -> usize {
        let selected = match self.view.as_ref().and_then(|v| v.selected) {
            Some(crate::starmap::Pick::Planet(p)) => Some(p),
            _ => None,
        };
        if let Some(p) = self.world.colony.or(selected) {
            self.akt_planet = Some(p);
        }
        self.akt_planet.unwrap_or_else(|| self.world.game.state.mem.i32(ig2_sim::save::va::AKT_PLANET).max(0) as usize)
    }

    /// The starmap's ways to the colony screen (F3 `DoKeyboard` 0x4db3eb, the screen menu `MOUSE_SelectScreens`
    /// 0x55555f, the colony icon 0x5b5b51 and the planet picture 0x5b6533 of `Starmap_Main`): while the tutorial awaits
    /// 0x1c, the main planet (`Tutorial_MainBolygoNeve`) stays shut (false) and any other raises the event. The test
    /// reads `AktPlanet` ([`Self::akt_planet`]), whatever the screen.
    pub(super) fn colony_tutorial_gate(&mut self) -> bool {
        let p = self.akt_planet();
        if self.world.game.state.mem.i32(ig2_sim::save::va::TUTORIAL_EVENT) == 0x1c {
            let main = self.tutorial.as_ref().map(|t| t.t.gates.main_planet.clone()).unwrap_or_default();
            let rec = self.world.game.state.mem.read(ig2_sim::save::va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, 0x18);
            if rec.split(|&b| b == 0).next() == Some(main.split(|&b| b == 0).next().unwrap_or_default()) {
                return false;
            }
            self.world.game.tutorial_check_event(0x1c);
        }
        true
    }

    /// Planet `p`'s colony screen (`Colony_Belep`).
    pub(super) fn open_colony(&mut self, p: usize) {
        let Some((renderer, band, scale)) = gfx(&mut self.gpu, &mut self.headless) else { return };
        self.world.show_colony(Some(p));
        let Some(snap) = self.world.colony_curr.clone() else { return };
        match crate::colony::ColonyScene::load(renderer, &self.install, self.tables, self.maps, &snap) {
            Ok(scene) => {
                let view = crate::colony::View::new(&snap, band, scale, &scene);
                let own = snap.owner == self.world.curr.human && !snap.at_war;
                let mut ui = crate::colony_ui::ColonyUi::enter(&snap, own, &self.world.game.state);
                ui.cam_keys = self.cam_keep.reentered();
                if let Some(field) = self.world.game.state.ground_field(p) {
                    ui.war.seed_groups(field.units.iter().enumerate().map(|(slot, u)| (slot, u.tail.f13c)));
                }
                ui.war.banner = std::mem::take(&mut self.attack_banner);
                self.colony = Some(Colony { scene, view, ui, panel: Default::default() });
                self.send_watching(p);
            }
            Err(e) => {
                eprintln!("ig2-client: colony {p}: {e}");
                self.world.show_colony(None);
            }
        }
    }

    /// `Colony_Belep` (exe 0x465b40) over the colony that is up (back from the options screen; it reads no `OldKep`):
    /// the screen's state starts over as on any entry (the selection rule, the war groups from the units' bytes, the
    /// planet's ambience again); the camera is the one `Colony_Kilep` wrote to the planet, the view's, so it stays.
    pub(super) fn reenter_colony(&mut self) {
        let (Some(c), Some(snap)) = (&mut self.colony, self.world.colony_curr.as_ref()) else { return };
        let own = snap.owner == self.world.curr.human && !snap.at_war;
        let mut ui = crate::colony_ui::ColonyUi::enter(snap, own, &self.world.game.state);
        ui.cam_keys = c.ui.cam_keys.reentered();
        if let Some(field) = self.world.game.state.ground_field(snap.planet) {
            ui.war.seed_groups(field.units.iter().enumerate().map(|(slot, u)| (slot, u.tail.f13c)));
        }
        ui.war.banner = std::mem::take(&mut c.ui.war.banner);
        c.ui = ui;
        let p = snap.planet;
        self.send_watching(p);
    }

    /// `Colony_Belep`'s `Multi_Send(0x26)`: MP tells the others which planet this machine's colony screen shows
    /// (`KiMelyikBolygonVan`, re/screens/colony.md §9.1); nothing clears it on leaving.
    pub(super) fn send_watching(&mut self, p: usize) {
        if self.multiplayer() {
            self.world.send(ig2_sim::game::ColonyOrder::Watch { planet: p as i32 }.into());
        }
    }

    /// A key of the war (`ColonyKeyboardHandler` exe 0x4628c0) by its character, at war or not: S stops
    /// (`StopSelected`), `a` (not under the tutorial's colony lock 0xf66ff9) selects every own unit. The groups go by
    /// virtual key (`colony_camera_key_by`).
    pub(super) fn colony_war_key(&self, ch: &str) -> Option<crate::colony_war::WarKey> {
        use crate::colony_war::WarKey;
        let c = self.colony.as_ref().filter(|_| self.world.colony_curr.is_some() && !self.under_screen())?;
        match ch {
            "s" | "S" => Some(WarKey::Stop),
            "a" if !c.ui.all_off => Some(WarKey::All),
            _ => None,
        }
    }

    /// The colony screen's input with its context; the outcome carried out.
    pub(super) fn colony_input(&mut self, f: impl FnOnce(&mut crate::colony_ui::ColonyUi, &crate::colony_ui::Ctx, [f32; 2], u32) -> crate::colony_ui::Outcome) {
        let now = self.now_ms();
        self.colony_input_inner(f, now);
        // `WF::SaveSelection` writes the groups into the units' bytes `+0x13c + n`, which the save keeps
        // (re/screens/colony.md §11.2); in multiplayer each peer keeps its own (ADR 0007: no per-peer state in the sim).
        let mp = self.multiplayer();
        let Some((p, groups)) = self.colony.as_mut().zip(self.world.colony).and_then(|(c, p)| Some((p, c.ui.war.take_groups()?.clone()))) else { return };
        if mp {
            return;
        }
        if let Some(mut field) = self.world.game.state.ground_field(p) {
            for (slot, u) in field.units.iter_mut().enumerate() {
                for (n, g) in groups.iter().enumerate() {
                    u.tail.f13c[n] = g.contains(&slot) as u8;
                }
            }
            self.world.game.state.store_ground_field(p, field);
        }
    }

    pub(super) fn colony_input_inner(&mut self, f: impl FnOnce(&mut crate::colony_ui::ColonyUi, &crate::colony_ui::Ctx, [f32; 2], u32) -> crate::colony_ui::Outcome, now: u32) {
        let (Some(c), Some(h), Some(snap)) = (&mut self.colony, &self.hud, &self.world.colony_curr) else { return };
        let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
        if let Some(gm) = self.maps.get(snap.map) {
            c.ui.war.project(&c.view, gm, snap, &c.scene.unit_centres);
        }
        c.ui.war.mods = crate::colony_war::Mods { shift: self.modifiers.shift_key(), ctrl: self.modifiers.control_key(), alt: self.modifiers.alt_key() };
        let ctx = crate::colony_ui::Ctx { state: &self.world.game.state, snap, rules: self.tables, sim: self.world.game.rules, texts: &h.texts, tech_name: &|t| h.tech_name(t) };
        let out = f(&mut c.ui, &ctx, at, now);
        if out.click {
            self.audio.play(crate::audio::SoundId::global(0));
        }
        for cmd in out.commands {
            self.world.send(cmd);
        }
        if let Some(e) = out.error {
            self.scroll_error(e);
        }
        if out.starmap {
            self.toggle_colony();
        } else if let Some(p) = out.colony {
            self.colony = None;
            self.open_colony(p);
        }
    }

    /// `AktKepKell := 3`, `AktPlanetKell := p`: the colony screen of planet `p`.
    pub(super) fn show_colony_screen(&mut self, p: usize) {
        match self.colony {
            Some(_) if self.world.colony == Some(p) => {}
            Some(_) => {
                self.colony = None;
                self.open_colony(p);
            }
            None => {
                self.planet_kell = Some(p);
                self.request_screen(3);
            }
        }
    }

    /// `ShowColony`: the cell and the building under the pointer (`AktBGridX/Y`, the model pick), on every pointer move
    /// and again every drawn frame (the camera or the buildings may have moved under a still pointer); the hold's
    /// `Colony_Main` pass (`drag`) on a move and every frame a button is held. `Mouse_Jobb` is read live.
    pub(super) fn colony_pointer(&mut self, moved: bool) {
        let p = self.cursor;
        let (held, right) = (moved || self.mouse_held != 0, self.mouse_held & 2 != 0);
        let gm = self.colony.as_ref().zip(self.world.colony_curr.as_ref()).and_then(|(c, s)| self.maps.get(s.map).map(|gm| (c, gm)));
        let gm_cell = gm.and_then(|(c, gm)| crate::colony_ui::cell_under(&c.view, gm, p));
        let start = gm.and_then(|(c, gm)| crate::colony_ui::cursor_cell_under(&c.view, gm, p));
        let visible = self.world.colony_curr.as_ref().is_some_and(|s| s.visible);
        let under = gm.and_then(|(c, gm)| c.view.building_at(gm, &c.scene, visible, p));
        // The pointer's cell and building first: the hold's building rule reads them.
        self.colony_input(|ui, ctx, at, _| {
            ui.war.under = under;
            ui.war.right_held = right;
            ui.hover(ctx, gm_cell, start);
            if held {
                ui.drag(ctx, at);
            }
            Default::default()
        });
    }

    /// `ColonyKeyboardHandler`'s held camera keys (spec §11.9), down and up: the arrows turn and tilt, numpad + / −
    /// zoom. True when taken.
    pub(super) fn colony_camera_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        self.colony_camera_key_by(event.physical_key, event.logical_key.as_ref(), event.state == ElementState::Pressed)
    }

    pub(super) fn colony_camera_key_by(&mut self, physical: winit::keyboard::PhysicalKey, logical: Key<&str>, down: bool) -> bool {
        use winit::keyboard::{KeyCode, PhysicalKey};
        // `DoKeyboard` (exe 0x4db1a0 line 186) hands every key up to the screen's handler, a text entry up or not;
        // a key down goes to the entry.
        let entry = self.menus.rename.is_some() || self.hud.as_ref().is_some_and(|h| h.chat.entry.is_some());
        if self.overlay.is_some() || self.spy.is_some() || self.colony.is_none() || (entry && down) {
            return false;
        }
        // `ColonyKeyboardHandler` (exe 0x4628c0 line 26): the top row's 4..9 (vk 0x34..=0x39, not the numpad) call
        // a group back, with Ctrl keep it, in a war or not.
        let group = match physical {
            PhysicalKey::Code(KeyCode::Digit4) => Some(0),
            PhysicalKey::Code(KeyCode::Digit5) => Some(1),
            PhysicalKey::Code(KeyCode::Digit6) => Some(2),
            PhysicalKey::Code(KeyCode::Digit7) => Some(3),
            PhysicalKey::Code(KeyCode::Digit8) => Some(4),
            PhysicalKey::Code(KeyCode::Digit9) => Some(5),
            _ => None,
        };
        if let Some(n) = group {
            if down {
                self.colony_input(move |ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::War(crate::colony_war::WarKey::Group(n))));
            }
            return true;
        }
        let Some(c) = &mut self.colony else { return false };
        let k = &mut c.ui.cam_keys;
        // VK 0x25..=0x28 by the logical key (a NumLock-off numpad sends them too), numpad + / − by
        // the physical one.
        let Some(i) = do_keyboard::colony_camera_key(physical, logical) else { return false };
        let flag = match i {
            0 => &mut k.left,
            1 => &mut k.right,
            2 => &mut k.up,
            3 => &mut k.down,
            4 => &mut k.zoom_in,
            _ => &mut k.zoom_out,
        };
        *flag = down;
        true
    }

    /// The building names' option row (`KellEpuletnevKiiras` exe 0x996468, id 202): `ShowBuildings` (exe 0x4c9220)
    /// writes them only while it is on. The handler's End toggle (exe 0x4628c0 line 284) never runs in the GOG exe:
    /// `remapKeyCodes` (exe 0x4e4670) has no case for SDLK_END, so its key down is never queued.
    pub(super) fn building_names(&self) -> bool {
        self.opt_rows.iter().find(|r| r.key == NAMES_KEY).is_none_or(|r| r.value != 0)
    }

    /// The pointer on the colony's short info (hot-spot 4, (0, 424) 166 × 56) with no unit or building selected
    /// (`ColonyInfoMod` 0).
    pub(super) fn colony_rename_spot(&self) -> bool {
        let Some(c) = self.colony.as_ref().filter(|_| !self.colony_modern_up()) else { return false };
        let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
        (0.0..166.0).contains(&at[0]) && (424.0..480.0).contains(&at[1]) && c.ui.selected == 0 && c.ui.war.selected.is_empty()
    }

    /// The colony's name entry (`Colony_Main`, `IOAzonosito` 0x1f): `IOString` = the shown planet's name, 15
    /// characters, at (50, 427) in colour 0x22, the first key replacing it.
    pub(super) fn start_colony_rename(&mut self) {
        let Some(p) = self.world.colony_curr.as_ref().map(|s| s.planet) else { return };
        let r = self.world.game.state.mem.read(ig2_sim::save::va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, 0x18);
        self.menus.rename = Some(crate::starmap_menus::Rename::new(false, p, &r[..r.iter().position(|&c| c == 0).unwrap_or(r.len())]));
    }
}
