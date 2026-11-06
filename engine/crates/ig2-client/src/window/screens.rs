//! The overlay screens (spy, diplomacy, messages, info, research, design, trade) entered and left from the game, and the top bar's menus.

use super::*;

impl App {
    /// Open the spy screen (`AktKepKell = 7`) or close it (`Spy_Kilep`: after the closing animation, which the
    /// redraw runs out).
    pub(super) fn toggle_spy(&mut self) {
        if let Some(s) = &mut self.spy {
            if s.leave() {
                self.spy = None;
            }
            return;
        }
        let (Some(g), Some(h)) = (&mut self.gpu, &self.hud) else { return };
        let d = crate::spy_screen::SpyData::read(&self.world.game.state, self.world.curr.human as usize, self.world.game.rules, self.tables);
        // Ours (ADR 0084): the modern screen opens with its own transition instead of the telescopes.
        let modern = self.display.modern;
        match crate::spy_screen::SpyView::open(&mut g.renderer, &self.install, h, &d, !modern, &mut self.spy_scene) {
            Ok(mut v) => {
                v.screen.modern = modern.then(crate::gui::Transition::opening);
                self.spy = Some(v);
            }
            Err(e) => eprintln!("ig2-client: spy screen: {e}"),
        }
    }

    /// The pointer on the spy screen's 640 × 480 virtual screen.
    pub(super) fn spy_virtual(&self, p: [f32; 2]) -> Option<[f32; 2]> {
        let g = self.gpu.as_ref()?;
        self.spy.as_ref()?;
        let (w, h) = g.band();
        Some(crate::font::Virtual::new([w as f32, h as f32]).virtual_of(p))
    }

    /// A starmap order or setting of the local player (`Multi_Send(.., EN, ..)`), applied in the next frame.
    /// Open or close a 2D screen (`I` info, `M` messages): `Info_Belep` starts on the human's empire and chart 0.
    pub(super) fn toggle_screen(&mut self, s: crate::ui::Screen) {
        // From the diplomacy screen (`Diplomacy_Kilep`): the closing first, except towards the messages screen.
        let closing = self.overlay == Some(crate::ui::Screen::Diplomacy) && !self.leave_diplomacy(Some(s));
        if !closing {
            self.overlay = if self.overlay == Some(s) { None } else { Some(s) };
            if self.overlay == Some(crate::ui::Screen::Messages) {
                // `Messages_Belep`'s saved part (re/rules/info-messages.md §7.3); `VoltUjValasz = −1`.
                self.world.send(ig2_sim::game::MessageOrder::Enter.into());
                if let Some(h) = &mut self.hud {
                    h.messages.pending = None;
                    h.messages.enter(true);
                }
            }
        }
        let next = if closing { Some(s) } else { self.overlay };
        if let (Some(crate::ui::Screen::Info), Some(h)) = (next, &mut self.hud) {
            // `InfoTriX/Y` keep the knob where it was left (`Info_Belep` only moves the model there).
            let tri = h.info.tri;
            h.info = crate::ui::InfoScreen::enter(self.world.ui.human);
            h.info.tri = tri;
        }
    }

    /// The diplomacy screen enters (`Diplomacy_Belep`; `animated` = with its opening, not back from the messages
    /// screen).
    pub(super) fn enter_diplomacy(&mut self, animated: bool) {
        if let (Some(g), Some(h)) = (&mut self.gpu, &self.hud) {
            let d = self.diplomacy.get_or_insert_with(|| crate::diplomacy::Open::new(&mut g.renderer, &self.install, self.tables, &h.texts, &self.world.game.state));
            d.enter(self.tables, &h.texts, &self.world.game.state, animated, self.display.modern);
            self.overlay = Some(crate::ui::Screen::Diplomacy);
        }
    }

    /// Leave the diplomacy screen for `next` (`None` = the starmap): true = gone now (towards the messages screen, or
    /// already closed); else its closing runs and the frame loop goes on to `next` when it ends.
    pub(super) fn leave_diplomacy(&mut self, next: Option<crate::ui::Screen>) -> bool {
        let quiet = next == Some(crate::ui::Screen::Messages);
        let Some(d) = &mut self.diplomacy else { return true };
        if d.screen.leave(quiet) {
            return true;
        }
        self.dip_next = next;
        false
    }

    /// A left press on the info or messages screen (`Info_Main`, `Messages_Main`).
    pub(super) fn screen_press(&mut self, screen: crate::ui::Screen) {
        let at = self.virtual_cursor();
        self.screen_press_at(screen, at);
    }

    /// A left press on the screen's hot-spots at the virtual point `at`.
    pub(super) fn screen_press_at(&mut self, screen: crate::ui::Screen, at: [f32; 2]) {
        let Some(h) = &mut self.hud else { return };
        let mut leave = false;
        match screen {
            crate::ui::Screen::Info => {
                let id = h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui);
                h.info.press(id, &self.world.ui.info, self.world.ui.human, at[1]);
                self.info_down = true;
                let b = crate::info_screen::Buttons { press: true, down: true, left: true, ..Default::default() };
                for c in h.info.orders(id, at, b, &self.world.ui.info, self.world.ui.human) {
                    self.world.send(c);
                }
            }
            // ADR 0040: the event log's label and, while it is up, its panel take the presses.
            crate::ui::Screen::Messages if self.display.event_log && !h.modern_messages && h.messages.list_up() && (h.messages.log.open || crate::event_log::Panel::over_label(at)) => {
                if crate::event_log::Panel::over_label(at) {
                    h.messages.log.toggle();
                    self.sounds.push(crate::audio::CLICK);
                } else if let Some(t) = h.messages.log.press(at, &self.world.log, h.texts.lang) {
                    self.log_jump(t);
                }
            }
            crate::ui::Screen::Messages => match h.messages.press(at, self.world.ui.inbox.len(), ig2_sim::game::inbox_arrows_allowed(&self.world.game.state)) {
                // The sim refuses the row while a dialogue or an automatic message holds the screen.
                crate::messages_screen::Press::Open(i) if ig2_sim::game::inbox_open_allowed(&self.world.game.state) => h.open_message(&self.install, &self.world.ui, i),
                crate::messages_screen::Press::Open(_) => {}
                crate::messages_screen::Press::Leave => leave = true,
                crate::messages_screen::Press::None => {}
            },
            _ => {}
        }
        if leave {
            self.leave_messages();
        }
    }

    /// Back to `MessagesElottiScreen` (hot-spots 7..=9, Esc): `Messages_Kilep`'s closing runs first.
    pub(super) fn leave_messages(&mut self) {
        if self.kep_kell.is_none() {
            self.request_screen(akt_kep(None, self.colony.is_some(), false));
        }
    }

    /// A right press on the info screen's buttons: `Info_Main` (exe 0x4e81c0) takes `Mouse_Lenyom`, either button's, on
    /// the rows 9..=12, the mode and chart buttons 14..=25, the arrows 30/31 and 35/36 and the back button 37 (FX3).
    pub(super) fn info_right_press(&mut self) {
        let at = self.virtual_cursor();
        let Some(h) = &mut self.hud else { return };
        let id = h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui);
        if matches!(id, Some(9..=12 | 14..=25 | 30 | 31 | 35 | 36 | 37)) {
            h.info.press(id, &self.world.ui.info, self.world.ui.human, at[1]);
        }
    }

    /// A right press, or a release, on the info screen (`Info_Main`'s triangle reset, drags, menus).
    pub(super) fn info_button(&mut self, press: bool, right: bool) {
        let at = self.virtual_cursor();
        let Some(h) = &mut self.hud else { return };
        let id = h.info.spot_at(&self.tables.race_name_x, at, &self.world.ui);
        self.info_down = press;
        if !press {
            h.info.release(id, &self.world.ui.info, self.world.ui.human);
        }
        let b = crate::info_screen::Buttons { press, release: !press, down: press, left: !right, right };
        for c in h.info.orders(id, at, b, &self.world.ui.info, self.world.ui.human) {
            self.world.send(c);
        }
    }

    /// `Research_Belep` from screen `from` (`OldKep`): animated (`KELLKINYITASDEFAULT`) unless from the messages, help
    /// or options screen.
    pub(super) fn enter_research(&mut self, from: i32) {
        if self.tech_infos.is_empty() {
            self.tech_infos = crate::research::tech_infos(&self.install);
        }
        if let (true, Some(h)) = (self.research_tips.is_empty(), &self.hud) {
            self.research_tips = crate::research::tips(&h.texts);
        }
        let place = self.research_place.unwrap_or_else(|| crate::research::Place::read(&self.world.game.state));
        // The modern screen (ADR 0071) has no 3D console to open.
        let animated = opens_animated(from) && !self.display.modern;
        self.research = Some(crate::research::Research::open_at(&self.world.game.state, self.tables, animated, place));
        self.overlay = Some(crate::ui::Screen::Research);
    }

    /// The research screen goes; its place stays for the next visit (`Research_Kilep` leaves the globals as they are).
    pub(super) fn drop_research(&mut self) {
        if let Some(r) = self.research.take() {
            self.research_place = Some(r.place());
        }
    }

    /// Run `f` on the design screen with what it reads ([`crate::design_edit::Ctx`]) and its hot-spot mask, then send
    /// the orders it made.
    pub(super) fn design_do<R>(&mut self, f: impl FnOnce(&mut crate::design::Design, &crate::design_edit::Ctx, &[u8]) -> R) -> Option<R> {
        let state = &self.world.game.state;
        let techs = state.tech();
        let d = self.design.as_mut()?;
        let c = crate::design_edit::Ctx {
            install: &self.install,
            state,
            techs: &techs,
            rules: self.tables,
            sim: &self.world.game.rules.ai,
            debug: state.mem.i32(ig2_sim::save::va::DESIGN_DEBUG) != 0,
            texts: self.hud.as_ref().map(|h| &h.texts),
        };
        let mask = self.design_scene.as_ref().map_or(&[][..], |s| &s.mask[..]);
        // `Design_Main`'s delete (spot 4) waits for `!TutorialPaused`.
        d.tutorial_paused = state.mem.u8(ig2_sim::save::va::TUTORIAL_PAUSED) != 0;
        let r = f(d, &c, mask);
        let events = std::mem::take(&mut d.tutorial);
        for o in std::mem::take(&mut d.orders) {
            self.world.send(Command::from(o));
        }
        for id in events {
            self.world.game.tutorial_check_event(id);
        }
        Some(r)
    }

    /// The design screen goes; the orders it sent that are still on their way are delivered.
    pub(super) fn drop_design(&mut self) {
        if let Some(mut d) = self.design.take() {
            for o in d.flush_orders() {
                self.world.send(Command::from(o));
            }
        }
    }

    /// A key in the design screen's name input; the viewing mode's input writes the stored design's name in place
    /// (re/deviations.md).
    pub(super) fn design_entry_key(&mut self, k: &Key<&str>) {
        use crate::design_edit::EntryKey;
        let key = match k {
            Key::Named(NamedKey::Enter) => {
                if let Some(d) = &mut self.design {
                    d.entry = None;
                }
                return;
            }
            Key::Named(NamedKey::Backspace) => EntryKey::Back,
            Key::Named(NamedKey::Delete) => EntryKey::Delete,
            Key::Named(NamedKey::Home) => EntryKey::Home,
            Key::Named(NamedKey::End) => EntryKey::End,
            Key::Named(NamedKey::ArrowLeft) => EntryKey::Left,
            Key::Named(NamedKey::ArrowRight) => EntryKey::Right,
            Key::Named(NamedKey::Space) => EntryKey::Char(b' '),
            Key::Character(c) => {
                let b = self.hud.as_ref().map_or_else(|| c.as_bytes().to_vec(), |h| h.texts.encode(c));
                match b.first().copied().filter(|&b| b >= 0x20 && b != 0x81) {
                    Some(b) => EntryKey::Char(b),
                    None => return,
                }
            }
            _ => return,
        };
        let Some(d) = &mut self.design else { return };
        let en = self.world.game.state.mem.i32(ig2_sim::save::va::EN).clamp(0, 18) as usize;
        if let Some((k, name)) = d.entry_key(self.world.game.state.designs.get(en).map_or(&[][..], |t| &t[..]), key) {
            // Multiplayer: the shared state changes only through the lockstep (deviation 96).
            if self.world.net.is_some() {
                let n = name.iter().take_while(|&&c| c != 0).take(0x15).copied().collect();
                self.world.send(Command::RenameDesign { index: k as u32, name: n });
                return;
            }
            let state = &mut self.world.game.state;
            let en = state.mem.i32(ig2_sim::save::va::EN).clamp(0, 18) as usize;
            if let Some(r) = state.designs.get_mut(en).and_then(|t| t.get_mut(k)) {
                let n = name.len().min(0x15);
                r[..n].copy_from_slice(&name[..n]);
                r[n] = 0;
            }
        }
    }

    /// `Design_Belep` from screen `from` (`OldKep`; `desKELLKINYITAS`, re/screens/design.md §3).
    pub(super) fn enter_design(&mut self, from: i32) {
        if let (true, Some(h)) = (self.design_tips.is_empty(), &self.hud) {
            self.design_tips = crate::design::tips(&h.texts);
        }
        self.drop_research();
        let mut d = crate::design::Design::open(&self.world.game.state, opens_animated(from) && !self.display.modern);
        if let Some(h) = &self.hud {
            d.names = crate::design_edit::Names::read(&h.texts);
        }
        self.design = Some(d);
        self.overlay = Some(crate::ui::Screen::Design);
    }

    /// `Trade_Belep` from screen `from` (`TradeElottiKep = OldKep`; `traKELLKINYITAS`, re/screens/trade.md §5).
    pub(super) fn enter_trade(&mut self, from: i32) {
        let Some(h) = &self.hud else { return };
        if self.trade_tips.is_empty() {
            self.trade_tips = crate::trade::tips(&h.texts);
        }
        let advance = h.font.as_ref().map_or(6.0, |f| f.advance(2));
        let lines = crate::trade::ticker_lines(&h.texts);
        self.drop_research();
        self.drop_design();
        // Ours (ADR 0084): the modern screen opens with its own transition instead of the frame's.
        let modern = self.display.modern;
        let mut t = crate::trade::Trade::open(&self.world.game.state, opens_animated(from) && !modern, from, lines, advance);
        t.modern = modern.then(crate::gui::Transition::opening);
        self.trade = Some(t);
        self.overlay = Some(crate::ui::Screen::Trade);
    }

    /// What the top bar's drop-downs show now (re/screens/hud.md §2.1).
    pub(super) fn top_menus_data(&self) -> Option<crate::topbar_menus::Data> {
        let h = self.hud.as_ref()?;
        let screen = akt_kep(if self.help.is_some() { Some(crate::ui::Screen::Help) } else if self.options.is_some() { Some(crate::ui::Screen::Options) } else { self.overlay }, self.colony.is_some(), self.spacewar.is_some());
        let gates = self.tutorial.as_ref().map(|t| &t.t.gates.screens);
        Some(crate::topbar_menus::Data::read(&self.world.game.state, self.tables, &h.texts, screen, gates, self.spacewar.is_some()))
    }

    /// Every frame a drop-down is open: its data again and `Selected` under the pointer (`MOUSE_SelectScreens` runs
    /// every held frame).
    pub(super) fn top_menus_frame(&mut self) {
        self.akt_planet();
        self.top_bar_frame();
        if self.hud.as_ref().is_none_or(|h| h.top.open == crate::topbar_menus::Open::None) {
            return;
        }
        // `MOUSE_SelectScreens`' tail (exe 0x5557ce..): the research menu (`e40c5d`) shuts on any frame with no button
        // down (`Mouse_Gomb` 0xdb4650), a release lost outside the window included; the screen menu waits for one.
        if self.mouse_held == 0 {
            if let Some(h) = self.hud.as_mut().filter(|h| h.top.open == crate::topbar_menus::Open::Research) {
                h.top.open = crate::topbar_menus::Open::None;
                return;
            }
        }
        let at = self.virtual_cursor().map(|c| c as i32);
        let d = self.top_menus_data();
        if let (Some(h), Some(d)) = (&mut self.hud, d) {
            h.top.moved(at, &d);
            h.top_data = d;
        }
    }

    /// A button on the top bar's drop-downs (hot-spots 6 and 8, `MOUSE_SelectScreens`): true = taken. The pick asks
    /// for a screen the way its key does, or sends the research order.
    pub(super) fn top_menus_button(&mut self, button: MouseButton, pressed: bool) -> bool {
        use crate::topbar_menus::{Open, Pick};
        if self.front.is_some() || self.hud.is_none() || !matches!(button, MouseButton::Left | MouseButton::Right) {
            return false;
        }
        let at = self.virtual_cursor().map(|c| c as i32);
        let open = self.hud.as_ref().is_some_and(|h| h.top.open != Open::None);
        if !open && (!pressed || crate::topbar_menus::spot(at).is_none()) {
            return false;
        }
        let Some(d) = self.top_menus_data() else { return false };
        let Some(h) = &mut self.hud else { return false };
        let left = button == MouseButton::Left || self.mouse_held & 1 != 0;
        let pick = if pressed { h.top.press(at, left, &d) } else { h.top.release(at, &d) };
        h.top_data = d;
        match pick {
            Pick::Pass => return false,
            Pick::Taken => {}
            // The arrow's right press asks for the starmap with no tutorial gate; the menu's pick has one.
            Pick::Screen(n) if pressed => self.request_screen(n),
            Pick::Screen(n) => self.top_menu_screen(n),
            Pick::Start(t) => self.quick_research(t),
            Pick::Stop(t) => self.world.send(Command::Research { order: ig2_net::ResearchCmd::Stop { tech: t } }),
        }
        true
    }

    /// The quick-research menu's start: `ResearchAtvaltTalalmanyra(t)`; on the research screen its start button goes
    /// in, with the screen down the place it opens at next moves (exe 0x521f20).
    pub(super) fn quick_research(&mut self, t: i32) {
        if let Some(r) = &mut self.research {
            r.quick_start(t);
        } else {
            let state = &self.world.game.state;
            self.research_place.get_or_insert_with(|| crate::research::Place::read(state)).jump_to(t, self.tables);
        }
        self.world.send(Command::Research { order: ig2_net::ResearchCmd::Start { tech: t } });
    }

    /// The screen menu's pick and the arrow's right press (`AktKepKell = n`, [`Self::request_screen`]). While the
    /// tutorial awaits event 0x1c the pick is refused on the main planet and raises the event elsewhere, whatever the
    /// screen (`MOUSE_SelectScreens` exe 0x55555f).
    pub(super) fn top_menu_screen(&mut self, n: i32) {
        if self.world.game.state.mem.i32(ig2_sim::save::va::TUTORIAL_EVENT) == 0x1c && !self.colony_tutorial_gate() {
            return;
        }
        self.request_screen(n);
    }

    /// A press on the research screen (`Research_Main`, the lists, the models).
    /// A press on the research screen (`left`: `Mouse_Bal`); the right button also grips the ball.
    pub(super) fn research_press(&mut self, left: bool) {
        let at = self.virtual_cursor();
        let (Some(r), Some(m)) = (&mut self.research, &self.research_models) else { return };
        if left {
            r.held = true;
        }
        r.buttons |= if left { 1 } else { 2 };
        // `Mouse_Bal`: the left button held, whichever went down.
        let bal = left || self.mouse_held & 1 != 0;
        for a in r.press(&self.world.game.state, self.tables, &m.mask, at, &m.centres, bal) {
            let crate::research::Act::Send(c) = a;
            self.world.send(c);
        }
    }

    /// `ResearchKeyboardHandler`'s `oldEnterlenyomva` follows every event the handler sees: every key up, and the key
    /// downs and characters the exe queues that aren't the screen's own keys (those go through `research_key`).
    pub(super) fn research_key_seen(&mut self, e: &winit::event::KeyEvent) {
        let Some(r) = self.research.as_mut().filter(|_| self.overlay == Some(crate::ui::Screen::Research)) else { return };
        let down = e.state == ElementState::Pressed;
        if down && (!do_keyboard::exe_queues_key_down(e) || matches!(e.logical_key.as_ref(), Key::Named(n) if research_key(n).is_some())) {
            return;
        }
        r.key_seen(down);
    }

    pub(super) fn research_key(&mut self, k: crate::research::Key) {
        let Some(r) = &mut self.research else { return };
        for a in r.key(&self.world.game.state, k) {
            let crate::research::Act::Send(c) = a;
            self.world.send(c);
        }
    }

    /// The trade screen's keyboard handler (exe 0x609ae0): numpad + / − and the arrows, by virtual key, down only.
    /// True when taken.
    pub(super) fn trade_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        self.trade_key_by(event.physical_key, event.logical_key.as_ref(), event.state == ElementState::Pressed)
    }

    pub(super) fn trade_key_by(&mut self, physical: winit::keyboard::PhysicalKey, _logical: Key<&str>, down: bool) -> bool {
        use crate::trade::Key as T;
        use winit::keyboard::{KeyCode, PhysicalKey};
        if self.overlay != Some(crate::ui::Screen::Trade) {
            return false;
        }
        let k = match physical {
            PhysicalKey::Code(KeyCode::NumpadAdd) => T::More,
            PhysicalKey::Code(KeyCode::NumpadSubtract) => T::Less,
            PhysicalKey::Code(KeyCode::ArrowUp) => T::Up,
            PhysicalKey::Code(KeyCode::ArrowDown) => T::Down,
            PhysicalKey::Code(KeyCode::ArrowLeft) => T::Left,
            PhysicalKey::Code(KeyCode::ArrowRight) => T::Right,
            _ => return false,
        };
        if let (true, Some(tr)) = (down, &mut self.trade) {
            tr.key(&self.world.game.state, k);
        }
        true
    }
}
