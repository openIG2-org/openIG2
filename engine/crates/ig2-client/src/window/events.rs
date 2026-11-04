//! The window's events from winit: resumed, the redraw and input events, the exit.

use super::*;

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        if self.gpu_wait.is_some() {
            return;
        }
        if self.gpu.is_none() {
            if let Err(e) = self.init_gpu(el) {
                eprintln!("ig2-client: {e}");
                el.exit();
            }
        }
    }

    /// The browser keeps the page after the game quits: it goes back to its start (web/main.js).
    #[cfg(target_arch = "wasm32")]
    fn exiting(&mut self, _: &ActiveEventLoop) {
        crate::web::exited();
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        #[cfg(target_arch = "wasm32")]
        if let Some(parts) = self.gpu_wait.as_ref().and_then(|w| w.borrow_mut().take()) {
            self.gpu_wait = None;
            if let Err(e) = parts.and_then(|p| self.finish_gpu(p)) {
                crate::web::failed(&e);
                _el.exit();
            }
        }
        if let Some(g) = &self.gpu {
            g.window.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        // `ig2screen::anyEvent` (exe 0x7329d0) passes a button's or a finger's release on to the game's handler
        // (`MainCallBackEventHandler` 0x4e2db0) also while a fullscreen video plays: after the video the game sees the
        // button up (a press that started the video isn't left held).
        if let (Some(v), WindowEvent::MouseInput { state: ElementState::Released, .. }) = (&mut self.video, &event) {
            v.release();
        }
        // ADR 0058: a bound key arrives as its action's default key (the screens match the original's keys).
        let mut event = event;
        if let WindowEvent::KeyboardInput { event: k, .. } = &mut event {
            if !self.rebind(k) {
                return;
            }
        }
        // The pointer is in the window on every screen (the main menu's, help's and options' arms below don't reach the
        // game's own; `frame_overlay` draws none while it is out).
        if let WindowEvent::CursorMoved { .. } | WindowEvent::CursorEntered { .. } = event {
            self.pointer_in = true;
        }
        // The help and options screens track their own pointer; the top bar's drop-downs read the band's one.
        if let (WindowEvent::CursorMoved { position, .. }, true) = (&event, self.help.is_some() || self.options.is_some()) {
            let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
            self.cursor = [position.x as f32, position.y as f32 - top];
        }
        // Ours (ADR 0062): an egui page up (the modern Options screen) takes the pointer; the keys stay the screen's.
        // The modern starmap HUD (ADR 0065) sees the pointer too; `mouse` gives it the buttons over its panels.
        // The multiplayer pages (ADR 0066) take it the same way.
        let mp_page = self.options.is_none() && self.front.as_ref().is_some_and(|f| f.mp.is_some());
        let mods_page = self.options.is_none() && self.front.as_ref().is_some_and(|f| f.mods.is_some());
        let page = mp_page || mods_page || self.options.as_ref().is_some_and(|o| o.s.modern.is_some());
        // The modern main menu's entries take either button, as the panel's and the exe's rings (`Mouse_Lenyom`).
        let either = self.main_menu_modern_up();
        if !page && (self.modern_up() || self.battle_modern_up() || self.main_menu_modern_up() || self.new_game_modern_up() || self.help_modern_up() || self.stats_modern_up() || self.colony_modern_up() || self.info_modern_up() || self.messages_modern_up() || self.pages_modern_up() || self.research_modern_up() || self.design_modern_up() || self.gui_held != 0) && let Some(g) = &mut self.gpu {
            let k = crate::gui::Gui::scale(g.band().1);
            let top = g.layout().frame[1] as f32;
            let over = g.gui.ctx.is_pointer_over_area();
            match &event {
                WindowEvent::CursorMoved { position, .. } => g.gui.pointer(Some([position.x as f32, position.y as f32 - top]), k),
                WindowEvent::CursorLeft { .. } => g.gui.pointer(None, k),
                WindowEvent::MouseInput { state, button, .. } => {
                    let b = match button {
                        MouseButton::Left => 0,
                        MouseButton::Right if either => 0,
                        MouseButton::Right => 1,
                        _ => 2,
                    };
                    // A release always reaches it, a press only over its panels.
                    if over || *state == ElementState::Released {
                        g.gui.button(b, *state == ElementState::Pressed);
                    }
                }
                WindowEvent::MouseWheel { delta, .. } if over => {
                    g.gui.wheel(match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                    });
                    return;
                }
                _ => {}
            }
        }
        if page && let Some(g) = &mut self.gpu {
            let k = crate::gui::Gui::scale(g.band().1);
            let top = g.layout().frame[1] as f32;
            match &event {
                WindowEvent::CursorMoved { position, .. } => g.gui.pointer(Some([position.x as f32, position.y as f32 - top]), k),
                WindowEvent::CursorLeft { .. } => g.gui.pointer(None, k),
                WindowEvent::MouseInput { state, button, .. } => {
                    let b = match button {
                        MouseButton::Left => 0,
                        MouseButton::Right => 1,
                        _ => 2,
                    };
                    g.gui.button(b, *state == ElementState::Pressed);
                    // The right button stays the menu's Back (ADR 0059); the multiplayer page acts on it itself, the
                    // Mods page is left by its release. A left release still reaches the menu under the multiplayer
                    // page (as before the page, which took presses only).
                    if mods_page && *button == MouseButton::Right {
                        if *state == ElementState::Released {
                            self.mods_act(crate::mods_page::Act::Back);
                        }
                        return;
                    }
                    if (*button == MouseButton::Left && !(mp_page && *state == ElementState::Released)) || (mp_page && *button != MouseButton::Left) {
                        return;
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    g.gui.wheel(match delta {
                        MouseScrollDelta::LineDelta(_, y) => *y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                    });
                    return;
                }
                _ => {}
            }
        }
        if let WindowEvent::KeyboardInput { event: ref k, is_synthetic, .. } = event {
            self.research_key_seen(k);
            // ADR 0030: a W/A/S/D up ends its pan whichever screen has the keyboard (help, a video, a battle…).
            if let (ElementState::Released, Some(i)) = (k.state, do_keyboard::wasd_key(k.physical_key)) {
                self.star_keys.pan[i] = false;
                if let Some(c) = &mut self.colony {
                    c.ui.cam_keys.pan[i] = false;
                }
            }
            if !is_synthetic {
                self.modifier_key(k);
            }
        }
        if let WindowEvent::MouseInput { device_id, state, button } = event {
            if self.middle_clears(el, device_id, state, button) {
                return;
            }
        }
        match event {
            // SDL_QUIT (`MainCallBackEventHandler`): `CloseBink` if a video plays, then the game quits.
            WindowEvent::CloseRequested => {
                self.video = None;
                el.exit();
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.reconfigure(),
            WindowEvent::RedrawRequested => {
                if let Some(due) = self.tour.as_mut().and_then(|t| t.due()) {
                    match due {
                        Some(k) => self.tour_key(&k),
                        None => el.exit(),
                    }
                }
                self.frame();
                crate::prof::frame(self.profile_screen());
                if self.front.as_ref().is_some_and(|f| f.quit) {
                    el.exit();
                }
            }
            // A fullscreen video takes the keys and buttons (`ig2screen::anyEvent`: a press arms it, the release
            // after it skips); the game under it waits.
            WindowEvent::KeyboardInput { event, .. } if self.video.is_some() => {
                if let Some(v) = &mut self.video {
                    if event.state == ElementState::Pressed {
                        v.press();
                    } else {
                        v.release();
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } if self.video.is_some() => {
                if let Some(v) = &mut self.video {
                    v.press();
                }
            }
            // The wait overlay's own frame loop (`MOUSE_DrawOver`): nothing under it takes the buttons (its items take a
            // held button each frame, `update_mp_wait`); the keys stay live (its `Fake_FRISSITES` runs `DoKeyboard`).
            WindowEvent::MouseInput { state, button, .. } if self.mp_wait.is_open() => {
                let bit = match button {
                    MouseButton::Left => 1,
                    MouseButton::Right => 2,
                    _ => 0,
                };
                self.mouse_held = if state == ElementState::Pressed { self.mouse_held | bit } else { self.mouse_held & !bit };
            }
            // The top bar's drop-downs take the buttons first on every screen with the bar (`MOUSE_SelectScreens`).
            WindowEvent::MouseInput { state, button, .. }
                if (self.help.is_some() || self.options.is_some() || (self.spacewar.is_some() && !self.battle_modern_up())) && !self.tutorial_owns_input() && self.top_menus_button(button, state == ElementState::Pressed) => {}
            WindowEvent::CursorMoved { position, .. } if self.help.is_some() => {
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let (bw, bh) = self.gpu.as_ref().map_or((640, 480), |g| g.band());
                let at = crate::font::Virtual::new([bw as f32, bh as f32]).virtual_of([position.x as f32, position.y as f32 - top]);
                if let Some(h) = &mut self.help {
                    h.s.hover(at);
                }
            }
            // The top bar stays live over Help (its spots are registered once, help.md §5).
            WindowEvent::MouseInput { state, button, .. } if self.help.is_some() && { let at = self.help.as_ref().map_or([0.0; 2], |h| h.s.at); self.top_bar_mouse(button, state == ElementState::Pressed, at) } => {}
            // Ours (ADR 0078): the modern help page has the buttons (the top bar took its own above).
            WindowEvent::MouseInput { .. } if self.help_modern_up() => {}
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.help.is_some() => {
                if let Some(h) = &mut self.help {
                    if state == ElementState::Pressed {
                        let at = h.s.at;
                        h.s.press(at);
                    } else {
                        h.s.release();
                    }
                }
            }
            WindowEvent::MouseWheel { .. } | WindowEvent::MouseInput { .. } if self.help.is_some() => {}
            // `remapKeyCodes` (exe 0x4e4670): the key downs the exe never queues reach nothing (the key ups go on); a
            // numpad digit's character still types into an entry up (its SDL_TEXTINPUT). Ours: the main menu's modern
            // Options screen (ADR 0053), the multiplayer pages (ADR 0066) and the Mods page (ADR 0068) take them all (Tab,
            // Delete, Home, End).
            WindowEvent::KeyboardInput { ref event, .. } if event.state == ElementState::Pressed && !do_keyboard::exe_queues_key_down(event)
                && !(do_keyboard::numpad_digit(event) && self.entry_up()) && !self.options.as_ref().is_some_and(|o| o.s.modern.is_some())
                && !(self.options.is_none() && self.front.as_ref().is_some_and(|f| f.mp.is_some() || f.mods.is_some())) => {}
            // Esc is the exit hot-spot (`MainCallBackEventHandler` on `AktKep` 1); F1 does nothing there. Help has no
            // keyboard handler of its own, but `DoKeyboard` stays live (exe 0x4db1a0): F2..F10, O and M leave it for
            // their screen (`AktKep` changes, `Help_Kilep`), Space, P and 1/2/3 pause and set the speed.
            WindowEvent::KeyboardInput { event, .. } if self.help.is_some() => {
                if event.state != ElementState::Pressed {
                    return;
                }
                // Ours (ADR 0078): the modern page's search field takes the keys while it is typed in.
                if self.help_modern_up() && self.help.as_mut().is_some_and(|h| h.page.key(&event.logical_key.as_ref())) {
                    return;
                }
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        if let Some(h) = &mut self.help {
                            h.s.escape();
                        }
                    }
                    // F-keys: `DoKeyboard`'s screen change (Help goes at once, `Help_Kilep`).
                    k @ Key::Named(_) if do_keyboard::f_key_screen(&k).is_some() => {
                        if self.key(k) {
                            el.exit();
                        }
                    }
                    // O: `Help_Kilep` closes the screen first when it opened animated, then Options enters over the
                    // screen before Help (`OptionsElottiKep = HelpElottiScreen`).
                    Key::Character("o" | "O") if self.o_key_allowed() => {
                        if let Some(h) = &mut self.help {
                            h.s.escape();
                            self.help_then_options = true;
                        }
                    }
                    Key::Character("o" | "O") => {}
                    k @ Key::Character("m" | "M") => {
                        let again = self.help_under_is(&k);
                        self.leave_help();
                        if !again && self.key(k) {
                            el.exit();
                        }
                    }
                    k @ (Key::Named(NamedKey::Space) | Key::Character("p" | "P" | "1" | "2" | "3")) => {
                        if self.key(k) {
                            el.exit();
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } if self.options.is_some() => {
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let (bw, bh) = self.gpu.as_ref().map_or((640, 480), |g| g.band());
                let at = crate::font::Virtual::new([bw as f32, bh as f32]).virtual_of([position.x as f32, position.y as f32 - top]);
                if let Some(f) = &mut self.front {
                    f.at = at;
                }
                if let Some(o) = &mut self.options {
                    o.s.hover(at);
                }
            }
            // Ours (ADR 0059): over the main menu the right button is Esc (Load, Settings).
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } if self.options.is_some() && self.front.as_ref().is_some_and(|f| f.s.flow) => {
                let audio = &mut self.audio;
                if let Some(o) = &mut self.options {
                    if o.s.modern.is_some() {
                        o.s.modern_key(crate::options::modern::Nav::Back, &mut |id| audio.play(id));
                    } else if o.s.entry.is_none() {
                        o.s.escape();
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } if self.options.as_ref().is_some_and(|o| o.s.modern.is_none()) && { let at = self.options.as_ref().map_or([0.0; 2], |o| o.s.at); self.top_bar_mouse(button, state == ElementState::Pressed, at) } => {}
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.options.is_some() => {
                let audio = &mut self.audio;
                if let Some(o) = &mut self.options {
                    if state == ElementState::Pressed {
                        let at = o.s.at;
                        o.s.press(at, &mut |id| audio.play(id));
                    } else {
                        o.s.release();
                    }
                }
            }
            WindowEvent::MouseWheel { .. } | WindowEvent::MouseInput { .. } if self.options.is_some() => {}
            // `DoKeyboard` on the options screen: the F-keys (also in the name entry) and M ask for their screen.
            WindowEvent::KeyboardInput { event, .. } if self.options.is_some() => {
                // Ours (ADR 0053): a held Esc, Enter, Space or O acts once on the modern screen.
                let once = event.repeat && self.options.as_ref().is_some_and(|o| o.s.modern.is_some())
                    && matches!(event.logical_key.as_ref(), Key::Named(NamedKey::Escape | NamedKey::Enter | NamedKey::Space) | Key::Character("o" | "O"));
                // ADR 0058: a key pressed for an action waiting for one is its new key (Esc gives up).
                let esc = matches!(event.logical_key.as_ref(), Key::Named(NamedKey::Escape));
                if let (ElementState::Pressed, false, Some(o), winit::keyboard::PhysicalKey::Code(code)) = (event.state, esc, &mut self.options, event.physical_key) {
                    if o.s.capturing() {
                        let audio = &mut self.audio;
                        o.s.modern_bind(code, &mut |id| audio.play(id));
                        return;
                    }
                }
                if event.state == ElementState::Pressed && !once {
                    let k = event.logical_key.as_ref();
                    let entry = self.options.as_ref().is_some_and(|o| o.s.typing());
                    if let Some(n) = do_keyboard::f_key_screen(&k) {
                        self.f_key(n);
                    } else if !(matches!(k, Key::Character("m" | "M")) && !entry && self.m_key()) {
                        self.options_key(&event.logical_key);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } if self.front.is_some() => {
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let (bw, bh) = self.gpu.as_ref().map_or((640, 480), |g| g.band());
                let at = crate::font::Virtual::new([bw as f32, bh as f32]).virtual_of([position.x as f32, position.y as f32 - top]);
                if let Some(f) = &mut self.front {
                    f.at = at;
                }
            }
            WindowEvent::KeyboardInput { event, .. } if self.front.is_some() => {
                // A held Esc's repeats would close the statistics screen and go on to the menu's own Esc; ours: a held Esc
                // or Enter that closed the Options screen (ADR 0053) goes no further.
                let closer = matches!(event.logical_key.as_ref(), Key::Named(NamedKey::Escape | NamedKey::Enter));
                if event.state == ElementState::Pressed && !(event.repeat && (closer || self.front.as_ref().is_some_and(|f| f.stats.is_some() || f.replays.is_some() || f.mods.is_some()))) {
                    self.front_key(&event.logical_key);
                }
            }
            // Ours (ADR 0078): the modern statistics page has the buttons.
            WindowEvent::MouseInput { .. } if self.stats_modern_up() => {}
            // Ours (ADR 0034): the statistics screen takes either button, acting on the release so the menu under it
            // gets neither half of the click.
            WindowEvent::MouseInput { state, button: button @ (MouseButton::Left | MouseButton::Right), .. } if self.front.as_ref().is_some_and(|f| f.stats.is_some()) => {
                let Some(f) = &mut self.front else { return };
                if state == ElementState::Released {
                    if std::mem::take(&mut f.opening) {
                        return;
                    }
                    let at = f.at;
                    // Ours (ADR 0059): the right button on empty space is Esc.
                    if button == MouseButton::Right && f.s.flow && f.stats.as_ref().is_some_and(|st| !st.on_spot(at)) {
                        if f.stats.as_mut().and_then(|st| st.back()).is_some() {
                            f.stats = None;
                        }
                        return;
                    }
                    if f.stats.as_mut().and_then(|st| st.press(at)).is_some() {
                        f.stats = None;
                        self.audio.play(crate::audio::CLICK);
                    }
                }
            }
            // Ours (ADR 0038 §6): the replays list, as the statistics screen (either button, on the release).
            WindowEvent::MouseInput { state, button: button @ (MouseButton::Left | MouseButton::Right), .. } if self.front.as_ref().is_some_and(|f| f.replays.is_some()) => {
                let Some(f) = &mut self.front else { return };
                if state == ElementState::Released {
                    if std::mem::take(&mut f.opening) {
                        return;
                    }
                    let at = f.at;
                    if button == MouseButton::Right && f.s.flow && f.replays.as_ref().is_some_and(|rl| !rl.on_spot(at)) {
                        f.replays = None;
                        return;
                    }
                    match f.replays.as_mut().and_then(|rl| rl.press(at)) {
                        Some(crate::replay_viewer::ListAct::Close) => f.replays = None,
                        Some(crate::replay_viewer::ListAct::Open(path)) => self.watch_replay(&path),
                        None => return,
                    }
                    self.audio.play(crate::audio::CLICK);
                }
            }
            // The main menu's "Replays" item (ours, ADR 0038 §6).
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.front.as_ref().is_some_and(|f| front_history_shown(f) && crate::kit::inside_f(crate::replay_viewer::REPLAYS_ITEM, f.at)) =>
            {
                self.open_replays();
            }
            // The replay viewer's bar takes the presses on it (ours, ADR 0038 §6).
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.replay_bar_up() && crate::replay_viewer::Bar::covers(self.virtual_cursor()) => {
                if state == ElementState::Pressed {
                    if let Some(a) = crate::replay_viewer::Bar::press(self.virtual_cursor()) {
                        self.audio.play(crate::audio::CLICK);
                        self.replay_act(a);
                    }
                }
            }
            // The main menu's "History" item (ours, ADR 0034).
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.display.statistics && self.front.as_ref().is_some_and(|f| front_history_shown(f) && crate::kit::inside_f(crate::stats_screen::HISTORY_ITEM, f.at)) =>
            {
                self.open_history();
            }
            // Ours (ADR 0078): a press over the modern main menu's panel is the page's (a right one is no Esc there);
            // the new game page takes them all.
            WindowEvent::MouseInput { .. } if self.main_menu_modern_up() && self.gpu.as_ref().is_some_and(|g| g.gui.ctx.is_pointer_over_area()) => {}
            WindowEvent::MouseInput { .. } if self.new_game_modern_up() => {}
            // The main menu's boxes, race spheres and difficulty rings take either button (`Mouse_Lenyom`, FX3).
            WindowEvent::MouseInput { state, button: MouseButton::Right, .. } if self.front.is_some() => {
                let Some(f) = &mut self.front else { return };
                let at = f.at;
                if state == ElementState::Pressed {
                    if let Some(a) = f.s.press_right(at) {
                        self.front_act(a);
                    }
                } else {
                    f.s.release_right();
                }
            }
            // The wheel is no event of `MainCallBackEventHandler`'s (exe 0x4e2db0); the middle button drops the held ones.
            WindowEvent::MouseWheel { .. } if self.front.is_some() => {}
            WindowEvent::MouseInput { button: MouseButton::Middle, .. } if self.front.as_ref().is_some_and(|f| f.stats.is_none()) => {
                if let Some(f) = &mut self.front {
                    f.s.middle();
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } if self.front.is_some() => {
                let Some(f) = &mut self.front else { return };
                let at = f.at;
                let act = if state == ElementState::Pressed { f.s.press(at) } else { f.s.release(at) };
                if let Some(a) = act {
                    self.front_act(a);
                }
            }
            // While the script plays (`MainCallBackEventHandler`): the live pointer and buttons are dropped; `t` shows or
            // hides the window, `o` opens the options screen.
            // The OS pointer is still tracked (`MainCallBackEventHandler` stores the raw position first).
            WindowEvent::CursorMoved { position, .. } if self.tutorial_owns_input() => {
                self.cursor_px = [position.x as i32, position.y as i32];
                self.pointer_seen = true;
                self.pointer_in = true;
            }
            WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } | WindowEvent::ModifiersChanged(_) if self.tutorial_owns_input() => {}
            WindowEvent::KeyboardInput { event, .. } if self.tutorial_owns_input() => {
                if event.state == ElementState::Pressed {
                    match event.logical_key.as_ref() {
                        // Esc keeps its live path (not tutorial-gated in the exe).
                        Key::Named(NamedKey::Escape) => {
                            if self.key(event.logical_key.as_ref()) {
                                el.exit();
                            }
                        }
                        Key::Character("t" | "T") if !self.entry_up() => {
                            if let Some(t) = &mut self.tutorial {
                                t.t.gates.text_shown ^= true;
                            }
                        }
                        Key::Character("o" | "O") if self.tutorial_o_allowed() => self.ask_options(),
                        _ => {}
                    }
                }
            }
            // Paused (live input): `t` still shows or hides the window and `o` opens the options screen, then the key goes
            // on to the screen.
            WindowEvent::KeyboardInput { ref event, .. }
                if event.state == ElementState::Pressed && matches!(event.logical_key.as_ref(), Key::Character("t" | "T" | "o" | "O")) && self.tutorial.as_ref().is_some_and(|t| t.running()) && self.options.is_none() && !self.entry_up() =>
            {
                if matches!(event.logical_key.as_ref(), Key::Character("o" | "O")) {
                    if self.tutorial_o_allowed() {
                        self.ask_options();
                    }
                    return;
                }
                if let Some(t) = &mut self.tutorial {
                    t.t.gates.text_shown ^= true;
                }
                if self.spacewar.is_some() {
                    self.spacewar_key(event);
                } else if !self.wasd_key(event) && !self.colony_camera_key(event) && !self.starmap_key(event) && self.key(event.logical_key.as_ref()) {
                    el.exit();
                }
            }
            // A modifier is set by its own key's down (`modifier_key`), as the exe's are; the OS state only clears.
            WindowEvent::ModifiersChanged(m) => self.modifiers &= m.state(),
            WindowEvent::CursorLeft { .. } => self.pointer_in = false,
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_px = [position.x as i32, position.y as i32];
                self.pointer_seen = true;
                self.pointer_in = true;
                // The band's pixels: the mouse goes through the frame (`IG2_VIRTUAL_POSY`, re/screens/widescreen.md §4).
                let top = self.gpu.as_ref().map_or(0, |g| g.layout().frame[1]) as f32;
                let p = [position.x as f32, position.y as f32 - top];
                self.cursor_moved(p);
            }
            WindowEvent::MouseWheel { .. } if self.spacewar.is_some() => {}
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 60.0,
                };
                self.wheel(steps);
            }
            WindowEvent::MouseInput { state, button, .. } if self.spacewar.is_some() => {
                let at = self.virtual_cursor();
                let down = state == ElementState::Pressed;
                let mut first = false;
                if let Some(sw) = &mut self.spacewar {
                    // `IG2_gameUpdate` (exe 0x4e0440) acts on the press edge of "any button held" (`Mouse_Valtozas`
                    // 0xDB4653 with 0xDB4650): a second button pressed while one is held changes nothing.
                    first = !(sw.left || sw.right);
                    match button {
                        MouseButton::Left => sw.left = down,
                        MouseButton::Right => sw.right = down,
                        _ => {}
                    }
                }
                // The top bar's plain spots as on every screen: pause and the speeds on either button's press, the
                // message line (7) with its battle gate (`DEBUGX`) and the right button's history.
                // Under the modern battle HUD its own bar (egui) has them.
                if (!down || first) && !self.battle_modern_up() {
                    self.top_bar_mouse(button, down, at);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if self.spacewar.is_some() => self.spacewar_key(&event),
            WindowEvent::KeyboardInput { event, .. } if self.fleet_group_key(&event) || self.wasd_key(&event) || self.colony_camera_key(&event) || self.trade_key(&event) || self.starmap_key(&event) => {}
            WindowEvent::MouseInput { state, button, .. } => self.mouse(button, state == ElementState::Pressed),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let quit = self.key(event.logical_key.as_ref());
                if quit {
                    el.exit();
                }
            }
            _ => {}
        }
    }
}
