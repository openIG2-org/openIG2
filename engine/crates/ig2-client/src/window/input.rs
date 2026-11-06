//! The pointer and keyboard as the window gets them: moves, wheel, buttons, modifiers, and the keys that go to no one screen.

use super::*;

impl App {
    /// `MultiPlayerMode`.
    /// `DoKeyboard`'s chat entry in multiplayer (`IOMode` 1, exe 0x4db1a0 cases 0xd and 0x60): Enter or the
    /// backquote opens it on a game screen (`AktKep` < 0xd); while open the keys type into it and Enter sends the line
    /// (`ExecuteString`, `crate::console`: the host's `/kick`, else the 0x3fc chat), Esc drops it; the F-keys still switch
    /// screens, which closes it (`IG2_gameUpdate`). True when the key was taken. Enter that opens it still reaches
    /// the screen (the exe's case 0xd falls into the screen's handler); the backquote doesn't.
    pub(super) fn chat_key(&mut self, key: &Key<&str>) -> bool {
        use crate::chat::EntryKey as E;
        let changing = self.screen_changing();
        let Some(h) = &mut self.hud else { return false };
        let Some(e) = &mut h.chat.entry else {
            let opens = matches!(key, Key::Named(NamedKey::Enter) | Key::Character("`"));
            if opens && self.world.net.is_some() && self.options.is_none() && self.front.is_none() {
                h.chat.entry = Some(Default::default());
                return matches!(key, Key::Character("`"));
            }
            return false;
        };
        let k = match key {
            Key::Character(s) => s.bytes().next().map(E::Char),
            Key::Named(NamedKey::Space) => Some(E::Char(b' ')),
            Key::Named(NamedKey::Backspace) => Some(E::Backspace),
            Key::Named(NamedKey::Delete) => Some(E::Delete),
            Key::Named(NamedKey::Home) => Some(E::Home),
            Key::Named(NamedKey::End) => Some(E::End),
            Key::Named(NamedKey::ArrowLeft) => Some(E::Left),
            Key::Named(NamedKey::ArrowRight) => Some(E::Right),
            Key::Named(NamedKey::Enter) => Some(E::Enter),
            // `DoKeyboard` case 0x1b (exe 0x4db1a0): nothing while a screen change is pending.
            Key::Named(NamedKey::Escape) if changing => None,
            Key::Named(NamedKey::Escape) => Some(E::Escape),
            Key::Named(NamedKey::F1 | NamedKey::F2 | NamedKey::F3 | NamedKey::F4 | NamedKey::F5 | NamedKey::F6 | NamedKey::F7 | NamedKey::F8 | NamedKey::F9 | NamedKey::F10 | NamedKey::F11) => {
                h.chat.entry = None;
                return false;
            }
            _ => None,
        };
        if let Some(line) = k.and_then(|k| e.key(k)) {
            h.chat.entry = None;
            if !line.is_empty() {
                let races = h.texts.races();
                self.world.say(&line, &races);
            }
        }
        true
    }

    /// The screen up, for the `IG2_PROFILE` log.
    pub(super) fn profile_screen(&self) -> &'static str {
        use crate::ui::Screen as S;
        match self.overlay {
            _ if self.video.is_some() => "video",
            _ if self.front.is_some() => "mainmenu",
            _ if self.options.is_some() => "options",
            _ if self.help.is_some() => "help",
            _ if self.spacewar.is_some() => "spacewar",
            _ if self.spy.is_some() => "spy",
            Some(S::Info) => "info",
            Some(S::Messages) => "messages",
            Some(S::Research) => "research",
            Some(S::Design) => "design",
            Some(S::Trade) => "trade",
            Some(S::Diplomacy) => "diplomacy",
            _ if self.colony.is_some() && self.world.colony_curr.as_ref().is_some_and(|s| s.at_war) => "groundwar",
            _ if self.colony.is_some() => "colony",
            _ => "starmap",
        }
    }

    /// One of `IG2_PROFILE_TOUR`'s keys, as if pressed.
    pub(super) fn tour_key(&mut self, k: &str) {
        eprintln!("[prof] tour key {k}");
        if let Some(v) = &mut self.video {
            v.press();
            v.release();
            return;
        }
        let named = match k {
            "vsync" => {
                self.vsync = !self.vsync;
                self.reconfigure();
                return;
            }
            "Esc" => NamedKey::Escape,
            "Enter" => NamedKey::Enter,
            "Tab" => NamedKey::Tab,
            "Space" => NamedKey::Space,
            "F1" => NamedKey::F1,
            "F2" => NamedKey::F2,
            "F3" => NamedKey::F3,
            "F4" => NamedKey::F4,
            "F5" => NamedKey::F5,
            "F6" => NamedKey::F6,
            "F7" => NamedKey::F7,
            "F8" => NamedKey::F8,
            "F9" => NamedKey::F9,
            "F10" => NamedKey::F10,
            c => {
                self.key(Key::Character(c));
                return;
            }
        };
        self.key(Key::Named(named));
    }

    /// The pointer moved to `p` (the band's pixels).
    pub(super) fn cursor_moved(&mut self, p: [f32; 2]) {
        if let Some(at) = self.spy_virtual(p) {
            let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
            // Ours (ADR 0076): under the modern page the hidden screen sees a resting pointer (no edge scrolling).
            let at = if self.spy_modern_up() { [320.0, 400.0] } else { at };
            if let Some(spy) = &mut self.spy {
                spy.hover(&d, at);
            }
        }
        let info_modern = self.info_modern_up();
        let messages_modern = self.messages_modern_up();
        if let Some(v) = &self.view {
            let at = self.starmap_virtual(v.camera.virtual_of(p));
            self.panels.hover(at);
            self.menus.hover(at);
            if let Some(h) = &mut self.hud {
                match self.overlay {
                    Some(crate::ui::Screen::Info) if !info_modern => {
                        let id = h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui);
                        h.info.hover(id);
                        let b = crate::info_screen::Buttons { down: self.info_down, left: true, ..Default::default() };
                        for c in h.info.orders(id, at, b, &self.world.ui.info, self.world.ui.human) {
                            self.world.send(c);
                        }
                    }
                    Some(crate::ui::Screen::Messages) if !messages_modern => {
                        h.messages.hover(at);
                        h.messages.log.hover_at(at, &self.world.log);
                    }
                    _ => {}
                }
            }
            if let Some(d) = &mut self.design {
                d.moved(at);
            }
            if let Some(tr) = &mut self.trade {
                tr.moved(at);
            }
        }
        if self.colony.is_some() {
            self.cursor = p;
            self.colony_pointer(true);
        }
        if let Some(c) = self.colony.as_mut().filter(|_| self.overlay.is_none() && self.spy.is_none()) {
            // `ShowColony`: both buttons held turn and tilt the camera (colony.md §2.1).
            if self.mouse_held == 3 && !c.ui.picking() {
                let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(p);
                c.ui.cam_keys.mouse_turn(&mut c.view.camera, at);
            }
        }
        self.cursor = p;
        self.top_menus_frame();
    }

    /// A 2D screen (`overlay`) or the spy screen is up over the starmap or the colony.
    pub(super) fn under_screen(&self) -> bool {
        self.overlay.is_some() || self.spy.is_some()
    }

    /// The wheel turned by `steps` notches.
    pub(super) fn wheel(&mut self, steps: f32) {
        if let Some(h) = self.hud.as_mut().filter(|h| self.overlay == Some(crate::ui::Screen::Messages) && h.messages.log.open) {
            h.messages.log.scroll_by((-steps * 3.0).round() as i32, &self.world.log);
            return;
        }
        // None of the 2D screens (`Design_Main`, `Trade_Main`, `Research_Main`, …, `Spy_Main`) has a wheel.
        if self.under_screen() {
            return;
        }
        if let Some(c) = &mut self.colony {
            c.ui.cam_keys.wheel(&c.view.camera, steps);
        } else if let (Some(v), false) = (&mut self.view, !self.display.starmap_mouse || self.menus.info_on() || self.cam_saved.is_some() || self.panels.fleet_info()) {
            v.camera.zoom_steps(steps);
        }
    }

    /// A mouse button went down or up.
    /// `KeyShift`/`KeyCtrl`/`KeyAlt` (0xbae295/0xbae27f/0xbae294) are set on every KEYDOWN of their key, an OS
    /// repeat too, and cleared on its KEYUP, either side's (`MainCallBackEventHandler` exe 0x4e2db0 reads no mod state
    /// and filters no repeats; `remapKeyCodes` 0x4e4670 maps both keys of a pair to one VK; not while the tutorial
    /// holds the input, unless `AktKep` 0xd): after a middle click dropped them, the next repeat of a held modifier
    /// brings it back. AltGr (SDL's RALT) is Alt; the focus gain's synthetic presses aren't key-downs.
    pub(super) fn modifier_key(&mut self, k: &winit::event::KeyEvent) {
        if self.tutorial_owns_input() {
            return;
        }
        let m = match k.logical_key.as_ref() {
            Key::Named(NamedKey::Shift) => ModifiersState::SHIFT,
            Key::Named(NamedKey::Control) => ModifiersState::CONTROL,
            Key::Named(NamedKey::Alt | NamedKey::AltGraph) => ModifiersState::ALT,
            _ => return,
        };
        self.modifiers.set(m, k.state == ElementState::Pressed);
    }

    /// Row 379: `MainCallBackEventHandler` sends a press or release of any button but 1 and 3 to the block at exe
    /// 0x4e345a, which with `IsActive` (0x912a38) set zeroes `KeyShift`/`KeyCtrl`/`KeyAlt` and the held bytes
    /// 0xdb467d..f: the next `Mouse_Lekerdez` (exe 0x5105d0) sees every held button released (`Mouse_Felenged` and the
    /// per-button edges), and the real release later is no edge. So: the modifiers drop, each held button gets its
    /// release now, and its real release is swallowed. True: the event is swallowed.
    pub(super) fn middle_clears(&mut self, el: &ActiveEventLoop, device_id: winit::event::DeviceId, state: ElementState, button: MouseButton) -> bool {
        let bit = match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            _ => 0,
        };
        if bit != 0 {
            if state == ElementState::Pressed {
                self.buttons_down |= bit;
                self.released_early &= !bit;
                return false;
            }
            self.buttons_down &= !bit;
            let early = self.released_early & bit != 0;
            self.released_early &= !bit;
            return early;
        }
        if !self.gpu.as_ref().is_some_and(|g| g.window.has_focus()) {
            return false;
        }
        self.modifiers = ModifiersState::empty();
        for (b, button) in [(1, MouseButton::Left), (2, MouseButton::Right)] {
            if self.buttons_down & b != 0 {
                self.window_event(el, WindowId::dummy(), WindowEvent::MouseInput { device_id, state: ElementState::Released, button });
                self.released_early |= b;
            }
        }
        false
    }

    pub(super) fn mouse(&mut self, button: MouseButton, pressed: bool) {
        let state = if pressed { ElementState::Pressed } else { ElementState::Released };
        let bit = match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            _ => 0,
        };
        self.mouse_held = if pressed { self.mouse_held | bit } else { self.mouse_held & !bit };
        if let Some(c) = &mut self.colony {
            c.ui.war.right_held = self.mouse_held & 2 != 0;
        }
        // `Mouse_FelengedesMajdMegvar`: `IG2_gameUpdate` (exe 0x4e0440) runs no screen until every button is up, so
        // nothing (the top bar included) takes the presses and releases meanwhile.
        if self.wait_release {
            if self.mouse_held == 0 {
                self.wait_release = false;
                if let Some(c) = &mut self.colony {
                    c.ui.cam_keys.mouse_release();
                }
            }
            return;
        }
        // Ours (ADR 0065): a press over the modern HUD's panels is its own, and so is that button's release; both
        // buttons there over the info card start the rename, as on the original's info box.
        if bit != 0 && (self.gui_held & bit != 0 || (pressed && self.over_gui())) {
            if pressed {
                self.gui_held |= bit;
                if self.mouse_held == 3 && self.modern_up() && self.over_info_card() && !self.starmap_locked() && !self.menus.info_on() {
                    self.wait_release = true;
                    self.gui_held = 0;
                    self.start_rename();
                }
                // The colony's card is hot-spot 4: both buttons with nothing selected start the planet's name entry.
                if self.mouse_held == 3 && self.colony_modern_up() && self.over_colony_card() && self.colony.as_ref().is_some_and(|c| c.ui.selected == 0 && c.ui.war.selected.is_empty()) {
                    self.wait_release = true;
                    self.gui_held = 0;
                    self.colony_acts.clear();
                    self.start_colony_rename();
                }
            } else {
                self.gui_held &= !bit;
            }
            return;
        }
        let modern = self.modern_up() || self.colony_modern_up() || self.info_modern_up() || self.messages_modern_up() || self.pages_modern_up() || self.research_modern_up() || self.design_modern_up();
        if !modern && self.top_menus_button(button, pressed) {
            return;
        }
        // The top bar's spots win over every screen's (`top_bar`).
        let at = self.virtual_cursor();
        if !modern && self.top_bar_mouse(button, pressed, at) {
            return;
        }
        // Ours (ADR 0072): the modern info screen's panels own the whole screen; no press reaches the hidden hot-spots.
        if self.info_modern_up() || self.messages_modern_up() || self.pages_modern_up() {
            return;
        }
        if self.spy.is_some() && (bit == 0 || self.mouse_held & !bit != 0) {
            // `Spy_Main` (exe 0x58d410) tests only `Mouse_Lenyom`/`Felenged`, either button's edges: the first button
            // down presses, the last one up releases (`Mouse_Lekerdez` exe 0x510694..0x510717 takes both edges from
            // `Mouse_Gomb`, any button: a second button's edge while one is held is none); nothing reaches the
            // starmap under it.
            self.right_down = None;
            return;
        }
        if self.spy.is_some() {
            let Some(at) = self.spy_virtual(self.cursor) else { return };
            let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
            let Some(spy) = &mut self.spy else { return };
            let orders = if state == ElementState::Pressed { spy.press(&d, at) } else { spy.release(&d) };
            let errors = spy.take_errors();
            for c in orders {
                self.world.send(c);
            }
            if let Some(e) = errors.into_iter().last() {
                self.scroll_error(e);
            }
            return;
        }
        let on_starmap = self.colony.is_none() && self.overlay.is_none();
        // `ShowColony`'s both-button turn: the first press keeps the pointer and the camera, any release lets go.
        if let Some(c) = self.colony.as_mut().filter(|_| self.overlay.is_none() && self.spy.is_none() && bit != 0) {
            if !pressed {
                c.ui.cam_keys.mouse_release();
            } else if !c.ui.picking() {
                let at = crate::font::Virtual::new(c.view.camera.size).virtual_of(self.cursor);
                c.ui.cam_keys.mouse_press(&c.view.camera, at);
            }
        }
        // `Colony_Main` hot-spot 4 (exe 0x4667fb): both buttons held over the short info with `ColonyInfoMod` 0 start
        // the planet's name entry (`IOAzonosito` 0x1f; any planet, no sound) and wait for the release.
        if self.colony.is_some() && !self.under_screen() && pressed && self.mouse_held == 3 && self.colony_rename_spot() {
            self.wait_release = true;
            self.start_colony_rename();
            return;
        }
        let moved = |d: Option<[f32; 2]>, p: [f32; 2]| d.is_none_or(|d| (d[0] - p[0]).abs() + (d[1] - p[1]).abs() > 4.0);
        if on_starmap && pressed && self.spacewar.is_none() {
            let at = self.virtual_cursor();
            let on_box = at[0] >= 0.0 && at[0] < 166.0 && at[1] >= 424.0 && !self.menus.info_on() && !modern;
            // `Starmap_Main`: a press ends the rename, the old name back unless it is on the info box.
            if let Some(r) = self.menus.rename.take() {
                if !on_box {
                    self.menus.rename = Some(r.clone());
                    self.write_rename(&r.backup);
                    self.menus.rename = None;
                }
            }
            if on_box && self.mouse_held == 3 {
                // `Mouse_FelengedesMajdMegvar`: the releases don't reach the info box (the long info, the colony).
                self.left_down = None;
                self.right_down = None;
                self.wait_release = true;
                self.start_rename();
                return;
            }
        }
        match (button, state) {
            // Ours (ADR 0075): under the modern panels the left button's hidden hot-spots are off; the right one still
            // turns the hull.
            (MouseButton::Left, _) if self.design_modern_up() => {}
            (MouseButton::Left | MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Design) => {
                let at = self.virtual_cursor();
                if button == MouseButton::Left {
                    self.design_do(|d, c, mask| d.press(c, mask, at));
                } else if let Some(d) = &mut self.design {
                    d.press_right(at);
                }
            }
            (MouseButton::Left | MouseButton::Right, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Design) => {
                if button == MouseButton::Left {
                    self.design_do(|d, c, _| d.release(c));
                } else if let Some(d) = &mut self.design {
                    d.release_right();
                }
            }
            (MouseButton::Left | MouseButton::Right, _) if self.overlay == Some(crate::ui::Screen::Trade) => {
                let at = self.virtual_cursor();
                if let Some(tr) = &mut self.trade {
                    if pressed {
                        tr.press(button == MouseButton::Left, at);
                    } else {
                        tr.release(button == MouseButton::Left, at);
                    }
                }
            }
            // `Research_Main` (exe 0x523c10) acts on either button's press (re/screens/input.md §7).
            (MouseButton::Left | MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Research) && !self.research_modern_up() => self.research_press(button == MouseButton::Left),
            // The buttons for the drags and the faders (`Mouse_Lekerdez`).
            (MouseButton::Left | MouseButton::Right, _) if self.overlay == Some(crate::ui::Screen::Research) => {
                if let Some(r) = &mut self.research {
                    let bit = if button == MouseButton::Left { 1 } else { 2 };
                    r.buttons = if pressed { r.buttons | bit } else { r.buttons & !bit };
                    if button == MouseButton::Left {
                        r.held = pressed;
                    }
                }
            }
            (MouseButton::Left, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Diplomacy) => {
                self.dip_input.pressed = true;
                self.dip_input.held = true;
            }
            (MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Diplomacy) => self.dip_input.right_pressed = true,
            (MouseButton::Left, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Diplomacy) => {
                self.dip_input.released = true;
                self.dip_input.held = false;
            }
            (MouseButton::Left | MouseButton::Right, ElementState::Released) if self.overlay == Some(crate::ui::Screen::Info) => self.info_button(false, button == MouseButton::Right),
            (MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Info) => {
                self.info_right_press();
                self.info_button(true, true);
            }
            // `Messages_Main`: `Mouse_Lenyom` is either button's (FX3). The right one also sets `SkipAll` (0xd90dc4), which only
            // `MessagesGfx2D_2`'s `PlayBack` pause reads, and `PlayBack` is never set: nothing more to do (audio.md §5).
            (MouseButton::Right, ElementState::Pressed) if self.overlay == Some(crate::ui::Screen::Messages) => self.screen_press(crate::ui::Screen::Messages),
            (MouseButton::Left, ElementState::Pressed) if self.overlay.is_some() => {
                let s = self.overlay.unwrap();
                self.screen_press(s);
            }
            // The other 2D screens' right button doesn't reach the starmap or colony under them (no order).
            (MouseButton::Right, _) if self.overlay.is_some() => self.right_down = None,
            (MouseButton::Left, ElementState::Pressed) if self.colony.is_some() => self.colony_input(|ui, ctx, at, now| ui.press(ctx, at, true, now)),
            (MouseButton::Left, ElementState::Released) if self.colony.is_some() => {
                self.colony_input(|ui, ctx, at, now| ui.release(ctx, at, true, now));
            }
            // A second button's press is no new press on the starmap (`Mouse_Lenyom` is the first button down only,
            // `Mouse_Lekerdez` exe 0x5105d0): over a held console spot it is the spot's; after a press on the
            // map, which latches hot-spot 0x35 (`Mouse_Megfogva`, `Starmap_Main` exe 0x5b54a2), no console spot, pick or
            // order starts.
            (MouseButton::Left | MouseButton::Right, ElementState::Pressed) if on_starmap && (self.console_held.is_some() || self.mouse_held & !bit & 3 != 0) => {}
            (MouseButton::Left, ElementState::Pressed) => {
                self.menus.held = true;
                if !(on_starmap && self.starmap_press()) {
                    self.left_down = Some(self.cursor);
                }
            }
            // A held console spot stays latched (`Mouse_Megfogva`) until every button is up (`Mouse_Felenged`,
            // `Mouse_Lekerdez` exe 0x5105d0); a second button's press there is taken by it.
            (MouseButton::Left | MouseButton::Right, ElementState::Released) if self.console_held.is_some() => {
                if self.mouse_held & 3 == 0 {
                    self.menus.held = false;
                    self.console_release();
                }
            }
            (MouseButton::Left, ElementState::Released) if self.menus.menu.is_some() || self.menus.rop.is_some() => {
                self.menus.held = false;
                self.starmap_release();
            }
            (MouseButton::Left, ElementState::Released) if self.panel_held.is_some() => {
                self.menus.held = false;
                self.panel_held = None;
                let sel = self.selection();
                let acts = self.panels.release(&self.world.game.state, &sel);
                self.apply(acts);
            }
            (MouseButton::Left, ElementState::Released) => {
                self.menus.held = false;
                let down = self.left_down.take();
                if on_starmap && !moved(down, self.cursor) {
                    self.starmap_click(self.cursor, false);
                }
            }
            (MouseButton::Right, ElementState::Pressed) if on_starmap && self.console_held.is_none() && self.console_press(false) => {}
            (MouseButton::Right, ElementState::Pressed) if on_starmap && self.panel_press(false) => self.panel_held = Some(false),
            (MouseButton::Right, ElementState::Pressed) if self.colony.is_some() => self.colony_input(|ui, ctx, at, now| ui.press(ctx, at, false, now)),
            // `Starmap_Main` gives the map's right-button orders (0x15, the rally 0x400) on the press, not under the
            // radial menu (ELF 0x6f50a0 lines 731-732; a press with the left button held is no press, above); the right
            // button never drags the camera (ADR 0049 keeps it so: a pan there moved the map under rapid orders). The
            // info box's right action (colony / split) is the release's.
            (MouseButton::Right, ElementState::Pressed) => {
                self.right_down = Some(self.cursor);
                let gated = self.menus.rop.is_some();
                if on_starmap && !gated && !self.on_info_box(self.cursor) {
                    self.starmap_click(self.cursor, true);
                }
            }
            (MouseButton::Right, ElementState::Released) if self.panel_held.is_some() => {
                self.panel_held = None;
                let sel = self.selection();
                let acts = self.panels.release(&self.world.game.state, &sel);
                self.apply(acts);
            }
            // `Colony_Main` takes any right release (`Mouse_Felenged`), moved or not.
            (MouseButton::Right, ElementState::Released) if self.colony.is_some() => self.colony_input(|ui, ctx, at, now| ui.release(ctx, at, false, now)),
            // The exe tests only the pointer at the release (`AktIkon2 == 8`, lines 1309, 1455), wherever the press was.
            (MouseButton::Right, ElementState::Released) => {
                self.right_down = None;
                if on_starmap && self.on_info_box(self.cursor) {
                    self.starmap_click(self.cursor, true);
                }
            }
            _ => {}
        }
    }

    /// ADR 0040: on the messages screen `L` opens the event log; while it is up every key but the F-keys is its
    /// (the search, Esc, the arrows and PgUp/PgDn scroll). True when taken.
    /// ADR 0058: `k` as the key of the action the player bound it to, at the screen up; false = the default key of an
    /// action bound elsewhere, which does nothing. Not in the menus, a video, a text entry or the open event log.
    pub(super) fn rebind(&self, k: &mut winit::event::KeyEvent) -> bool {
        use crate::bindings::Remap;
        let winit::keyboard::PhysicalKey::Code(code) = k.physical_key else { return true };
        let log_open = self.hud.as_ref().is_some_and(|h| h.messages.log.open);
        let modern = self.options.as_ref().is_some_and(|o| o.s.modern.is_some());
        if self.bindings.is_default() || self.front.is_some() || self.video.is_some() || modern || self.entry_up() || log_open {
            return true;
        }
        match self.bindings.remap(crate::bindings::Ctx::of_screen(self.akt_kep_now()), code) {
            Remap::Keep => true,
            Remap::Drop => false,
            Remap::To(to) => {
                let upper = matches!(k.logical_key.as_ref(), Key::Character(c) if c.chars().any(char::is_uppercase));
                let (logical, text) = crate::bindings::logical(to, upper);
                k.physical_key = winit::keyboard::PhysicalKey::Code(to);
                k.logical_key = logical;
                k.text = if k.state == ElementState::Pressed { text } else { None };
                true
            }
        }
    }

    pub(super) fn event_log_key(&mut self, key: &Key<&str>) -> bool {
        use crate::chat::EntryKey as E;
        let on = self.display.event_log && self.overlay == Some(crate::ui::Screen::Messages);
        let Some(h) = self.hud.as_mut().filter(|h| on && h.messages.list_up()) else { return false };
        let log = &mut h.messages.log;
        if !log.open {
            if matches!(key, Key::Character("l" | "L")) {
                log.toggle();
                return true;
            }
            return false;
        }
        let k = match key {
            Key::Character(s) => s.bytes().next().map(E::Char),
            Key::Named(NamedKey::Space) => Some(E::Char(b' ')),
            Key::Named(NamedKey::Backspace) => Some(E::Backspace),
            Key::Named(NamedKey::Delete) => Some(E::Delete),
            Key::Named(NamedKey::Home) => Some(E::Home),
            Key::Named(NamedKey::End) => Some(E::End),
            Key::Named(NamedKey::ArrowLeft) => Some(E::Left),
            Key::Named(NamedKey::ArrowRight) => Some(E::Right),
            Key::Named(NamedKey::Escape) => Some(E::Escape),
            Key::Named(n @ (NamedKey::ArrowUp | NamedKey::ArrowDown | NamedKey::PageUp | NamedKey::PageDown)) => {
                let lines = match n {
                    NamedKey::ArrowUp => -1,
                    NamedKey::ArrowDown => 1,
                    NamedKey::PageUp => -30,
                    _ => 30,
                };
                log.scroll_by(lines, &self.world.log);
                None
            }
            _ => None,
        };
        if let Some(k) = k {
            log.key(k);
        }
        true
    }

    /// ADR 0040: a log line's planet or fleet selected and centred on the starmap (the messages screen closes first).
    pub(super) fn log_jump(&mut self, t: crate::event_log::Target) {
        use crate::event_log::Target;
        let sel = self.selection();
        let curr = &self.world.curr;
        let hit = match t {
            Target::Planet(p) => curr.planets.iter().find(|v| v.index == p && v.on_map()).map(|v| (Selection { mode: 1, planet: p, ..sel }, v.pos)),
            Target::Fleet(f) => curr.fleets.iter().find(|v| v.slot == f && v.visible).map(|v| (Selection { mode: 2, fleet: f, ..sel }, v.pos)),
        };
        let Some((s, at)) = hit else { return };
        self.sounds.push(crate::audio::CLICK);
        self.request_screen(2);
        self.apply(vec![Action::Select(s), Action::Centre { x: at[0], y: at[1] }]);
    }

    /// A key went down; true = quit.
    pub(super) fn key(&mut self, key: Key<&str>) -> bool {
        if self.chat_key(&key) {
            return false;
        }
        // `DoKeyboard`'s F-keys (exe 0x4db1a0): before the text entries, which don't take them.
        if let Some(n) = do_keyboard::f_key_screen(&key) {
            self.f_key(n);
            return false;
        }
        // ADR 0032: F12 (no screen of the original's takes it) quick-saves, Shift+F12 quick-loads.
        if key == Key::Named(NamedKey::F12) {
            if self.modifiers.shift_key() {
                self.quick_load();
            } else {
                self.quick_save();
            }
            return false;
        }
        if self.event_log_key(&key) {
            return false;
        }
        // The replay viewer's keys (ours, ADR 0038 §6).
        if let (true, Key::Character(c)) = (self.replay_bar_up(), &key) {
            if let Some(a) = crate::replay_viewer::key_act(c) {
                self.replay_act(a);
                return false;
            }
        }
        match key {
            // The starmap's rename takes the keys (`IOMode`).
            _ if self.menus.rename.is_some() && !self.under_screen() => self.rename_key(&key),
            // `DoKeyboard` case 0x1b (exe 0x4db1a0): the name input ignores Esc while a screen change is pending.
            Key::Named(NamedKey::Escape)
                if self.overlay == Some(crate::ui::Screen::Design) && self.design.as_ref().is_some_and(|d| d.entry.is_some()) && self.screen_changing() => {}
            // `DesignKeyboardEscPressed`: the name input ends, an edit is cancelled; else the options screen.
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Design) => {
                if self.design_do(|d, c, _| d.escape(c)) != Some(true) && self.o_key_allowed() {
                    self.open_options(false);
                }
            }
            // The name input (`IOMode`, `IOAzonosito` 0x33; exe `DoKeyboard` 0x4db1a0): Enter ends it
            // (`DesignEntertNyomott`, no `DesignKeyboardHandler` after it); the other keys, Space, the digits and the
            // arrows included, go to the input or nowhere.
            _ if self.overlay == Some(crate::ui::Screen::Design) && self.design.as_ref().is_some_and(|d| d.entry.is_some()) => {
                self.design_entry_key(&key)
            }
            Key::Named(NamedKey::Enter) if self.overlay == Some(crate::ui::Screen::Design) => {
                self.design_do(|d, c, _| d.edit_key(c, false));
            }
            Key::Character("r") if self.overlay == Some(crate::ui::Screen::Design) => {
                self.design_do(|d, c, _| d.edit_key(c, true));
            }
            // The trade screen has no keyboard handler but its arrows and ± (`trade_key`): Esc asks for the options
            // screen at once.
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Trade) => {
                if self.o_key_allowed() {
                    self.trade_under_options = self.trade.take();
                    self.overlay = None;
                    self.open_options(false);
                }
            }
            Key::Character(c) if self.overlay == Some(crate::ui::Screen::Trade) && !matches!(c, "1" | "2" | "3" | "p" | "P" | "m" | "M" | "o" | "O") => {}
            Key::Named(NamedKey::Tab | NamedKey::PageUp | NamedKey::PageDown | NamedKey::Delete) if self.overlay == Some(crate::ui::Screen::Trade) => {}
            // The design screen's own keys; the other screens' do nothing here (no `DispatchKeyboard` target).
            Key::Character(c) if self.overlay == Some(crate::ui::Screen::Design) && !matches!(c, "1" | "2" | "3" | "p" | "P" | "m" | "M" | "o" | "O") => {}
            Key::Named(NamedKey::Tab | NamedKey::PageUp | NamedKey::PageDown | NamedKey::Delete) if self.overlay == Some(crate::ui::Screen::Design) => {}
            Key::Named(k @ (NamedKey::ArrowUp | NamedKey::ArrowDown)) if self.overlay == Some(crate::ui::Screen::Design) => {
                if let Some(d) = &mut self.design {
                    d.key(if k == NamedKey::ArrowUp { crate::design::Key::Up } else { crate::design::Key::Down });
                }
            }
            // Esc on the messages screen (`MainCallBackEventHandler` exe 0x4e2db0): when idle `AktIkonKell = 0x25`, a press
            // on hot-spot 7 (`IG2_gameUpdate` 0x4e0440), which leaves for `MessagesElottiScreen` as a click there does;
            // nothing while answers wait or the text types.
            Key::Named(NamedKey::Escape) if self.overlay == Some(crate::ui::Screen::Messages) => {
                if self.messages_idle() && !self.screen_changing() {
                    self.leave_messages();
                }
            }
            // `MessagesKeyboardHandler` (exe 0x507eb0): a..d answer.
            Key::Character(c @ ("a" | "b" | "c" | "d")) if self.overlay == Some(crate::ui::Screen::Messages) => {
                if let Some(h) = &mut self.hud {
                    h.messages.key(c);
                }
            }
            // The ground war's keys (`ColonyKeyboardHandler`, colony.md §11.2) while the shown planet is at war.
            Key::Character(ch) if self.colony_war_key(ch).is_some() => {
                if let Some(k) = self.colony_war_key(ch) {
                    self.colony_input(move |ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::War(k)));
                }
            }
            // Esc (`MainCallBackEventHandler` exe 0x4e2db0): on the starmap with a panel up the panel's exit (hot-spot
            // 9); on the colony in its planet info mode hot-spot 0x1f (`AktIkonKell` 0x3d), the panel's dead spot:
            // nothing; else `ESC_Pressed` (ELF 0x581440): Options (re/screens/options.md §5.1).
            Key::Named(NamedKey::Escape) if self.starmap_escape() => {}
            Key::Named(NamedKey::Escape) if self.colony.is_some() && self.overlay.is_none() && self.colony.as_ref().is_some_and(|c| c.ui.info.is_some()) => {}
            Key::Named(NamedKey::Escape) | Key::Character("o" | "O") => {
                if self.o_key_allowed() {
                    self.ask_options();
                }
            }
            // `DoKeyboard`'s M (exe 0x4db1a0 lines 92-114); refused, the character goes on to the screen (none uses it).
            Key::Character("m" | "M") => {
                self.m_key();
            }
            Key::Named(dir @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) if self.spy.is_some() => {
                let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
                if let Some(spy) = &mut self.spy {
                    spy.key(&d, dir == NamedKey::ArrowLeft);
                }
            }
            Key::Named(k) if self.overlay == Some(crate::ui::Screen::Research) && research_key(k).is_some() => {
                if let Some(k) = research_key(k) {
                    self.research_key(k);
                }
            }
            Key::Character("e" | "E") if self.colony.is_some() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Priority)),
            // Shift+Delete (vk 0x2e, `ColonyKeyboardHandler` exe 0x462a9c) never arrives: `remapKeyCodes` (exe 0x4e4670)
            // queues no key down for SDLK_DELETE. PageUp and PageDown come swapped out of it (SDLK_PAGEUP → vk 0x22,
            // SDLK_PAGEDOWN → 0x21): PageUp is the next own planet, PageDown the previous [obs: quick-1.sav planet 42,
            // PageUp → 43 → 46, PageDown → 43]; `TutorialCheckEvent(0x1b)` before the step (exe 0x462caa / 0x462df6).
            Key::Named(NamedKey::PageUp) if self.colony.is_some() => {
                self.world.game.tutorial_check_event(0x1b);
                self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Next));
            }
            Key::Named(NamedKey::PageDown) if self.colony.is_some() => {
                self.world.game.tutorial_check_event(0x1b);
                self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Prev));
            }
            // `ColonyKeyboardHandler`: I is the planet info mode on the colony screen (colony.md §12.1).
            Key::Character("i" | "I") if self.colony.is_some() => self.colony_input(|ui, ctx, _, _| ui.key(ctx, crate::colony_ui::Key::Info)),
            // `ColonyKeyboardHandler`: `c` (ch 99 only) cycles the camera presets on the colony screen, not under the
            // tutorial's colony lock (0xf66ff9).
            Key::Character("c") if self.colony.is_some() && self.overlay.is_none() && self.spy.is_none() => {
                if let Some(c) = self.colony.as_mut().filter(|c| !c.ui.all_off) {
                    c.ui.cam_keys.preset();
                }
            }
            // `DoKeyboard`: Space and P pause.
            // `DoKeyboard`: Space and P pause; MP on any screen (input.md §5.2).
            Key::Named(NamedKey::Space) | Key::Character("p" | "P") => {
                if self.time_keys_allowed() || self.multiplayer() {
                    self.toggle_pause();
                }
            }
            Key::Character(c @ ("1" | "2" | "3")) if self.multiplayer() => self.set_speed(c.parse().unwrap_or(1)),
            Key::Character(c @ ("1" | "2" | "3")) if self.time_keys_allowed() => {
                // `DoKeyboard` (ELF 0x5800d0): the keys sound whatever the speed was, single player only.
                self.sounds.push(crate::audio::SoundId::global(10));
                let level = c.parse().unwrap_or(1);
                if !self.turbo_step(level) && (self.world.paused() || self.world.game.speed_level() != level) {
                    self.send(StarmapOrder::Speed { level });
                    self.send(StarmapOrder::Pause { paused: false });
                }
            }
            _ => {}
        }
        false
    }
}
