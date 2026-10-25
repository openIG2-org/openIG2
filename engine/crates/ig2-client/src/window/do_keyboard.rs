//! `DoKeyboard` (exe 0x4db1a0, re/screens/input.md §1): the keys every game screen shares, and the screen change they
//! ask for (`AktKepKell`), carried out the way `IG2_gameUpdate` does it: the screen up leaves (its `_Kilep`, closing
//! animation included), then the screen asked for enters.

use ig2_sim::save::va;
use ig2_sim::time::{TickClock, SIM_TICK_MS};
use winit::keyboard::{Key, NamedKey};

use super::App;
use crate::ui::Screen;

/// A screen change asked for (`AktKepKell`), and the screen (`AktKep`) it was asked from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct KepKell {
    pub to: i32,
    pub from: i32,
}

/// F1..F11 (vk 0x70..0x7a): the screen they ask for, `vk − 0x6f` (1 help … 10 messages, 11 the space battle).
pub(super) fn f_key_screen(k: &Key<&str>) -> Option<i32> {
    const F: [NamedKey; 11] = [
        NamedKey::F1, NamedKey::F2, NamedKey::F3, NamedKey::F4, NamedKey::F5, NamedKey::F6, NamedKey::F7, NamedKey::F8, NamedKey::F9,
        NamedKey::F10, NamedKey::F11,
    ];
    match k {
        Key::Named(n) => F.iter().position(|f| f == n).map(|i| i as i32 + 1),
        _ => None,
    }
}

/// `remapKeyCodes` (exe 0x4e4670, FX5 decompile): the keys whose key down `MainCallBackEventHandler` queues
/// at all: Backspace, Enter, Esc, Space, the top row's 1..9, F1..F12, PgUp/PgDn (as each other's code), the arrows,
/// numpad +/−, Ctrl, Shift and Alt. Delete, Home, End, Insert, Tab, numpad Enter, the numpad digits and the rest reach
/// no handler (also not the text entries); the characters come from the text events (SDL_TEXTINPUT), which a numpad
/// digit's key event is taken for here (the client's keys are both). False = the client drops the key down.
pub(super) fn exe_queues_key_down(event: &winit::event::KeyEvent) -> bool {
    use winit::keyboard::{KeyCode, PhysicalKey};
    if let PhysicalKey::Code(c) = event.physical_key {
        let numpad = matches!(c, KeyCode::Numpad0 | KeyCode::Numpad1 | KeyCode::Numpad2 | KeyCode::Numpad3 | KeyCode::Numpad4
            | KeyCode::Numpad5 | KeyCode::Numpad6 | KeyCode::Numpad7 | KeyCode::Numpad8 | KeyCode::Numpad9 | KeyCode::NumpadEnter);
        if numpad {
            return false;
        }
    }
    match event.logical_key.as_ref() {
        Key::Named(n) => matches!(n, NamedKey::Backspace | NamedKey::Enter | NamedKey::Escape | NamedKey::Space | NamedKey::F1 | NamedKey::F2
            | NamedKey::F3 | NamedKey::F4 | NamedKey::F5 | NamedKey::F6 | NamedKey::F7 | NamedKey::F8 | NamedKey::F9 | NamedKey::F10
            | NamedKey::F11 | NamedKey::F12 | NamedKey::PageUp | NamedKey::PageDown | NamedKey::ArrowLeft | NamedKey::ArrowRight
            | NamedKey::ArrowUp | NamedKey::ArrowDown | NamedKey::Control | NamedKey::Shift | NamedKey::Alt),
        Key::Character(_) => true,
        _ => false,
    }
}

/// A numpad digit (with Num Lock on, a `Key::Character`): the exe queues no key down for it, but its SDL_TEXTINPUT
/// character still types into a text entry (`remapKeyCodes` exe 0x4e4670; deviations #253).
pub(super) fn numpad_digit(event: &winit::event::KeyEvent) -> bool {
    numpad_digit_key(event.physical_key, event.logical_key.as_ref())
}

fn numpad_digit_key(physical: winit::keyboard::PhysicalKey, logical: Key<&str>) -> bool {
    use winit::keyboard::{KeyCode, PhysicalKey};
    matches!(physical, PhysicalKey::Code(KeyCode::Numpad0 | KeyCode::Numpad1 | KeyCode::Numpad2 | KeyCode::Numpad3
        | KeyCode::Numpad4 | KeyCode::Numpad5 | KeyCode::Numpad6 | KeyCode::Numpad7 | KeyCode::Numpad8 | KeyCode::Numpad9))
        && matches!(logical, Key::Character(c) if c.bytes().all(|b| b.is_ascii_digit()))
}

impl App {
    /// A text entry has the keys: the characters a key event carries type into it.
    pub(super) fn entry_up(&self) -> bool {
        self.front.as_ref().is_some_and(|f| f.mp.is_some() || f.s.name.is_some())
            || self.options.as_ref().is_some_and(|o| o.s.entry.is_some())
            || self.hud.as_ref().is_some_and(|h| h.chat.entry.is_some())
            || self.menus.rename.is_some()
            || (self.overlay == Some(Screen::Design) && self.design.as_ref().is_some_and(|d| d.entry.is_some()))
    }

    /// `AktKep`: the screen up.
    pub(super) fn akt_kep_now(&self) -> i32 {
        match () {
            _ if self.front.is_some() => 0xf,
            _ if self.options.is_some() => 0xd,
            _ if self.help.is_some() => 1,
            _ if self.spacewar.is_some() => 11,
            _ if self.spy.is_some() => 7,
            _ => super::akt_kep(self.overlay, self.colony.is_some(), false),
        }
    }

    /// A screen change is under way (`Belep`, `Atlepes`, `AktKepKell` 0xbae24c / 0xbae24d / 0xbae250): one asked for,
    /// or a screen's closing running.
    pub(super) fn screen_changing(&self) -> bool {
        self.kep_kell.is_some()
            || self.research.as_ref().is_some_and(|r| r.closing)
            || self.design.as_ref().is_some_and(|d| d.closing)
            || self.trade.as_ref().is_some_and(|t| t.changing())
            || self.spy.as_ref().is_some_and(|s| s.screen.closing)
            || (self.overlay == Some(Screen::Diplomacy) && self.diplomacy.as_ref().is_some_and(|d| d.screen.closing))
            || (self.overlay == Some(Screen::Messages) && self.hud.as_ref().is_some_and(|h| h.messages.closing))
    }

    /// `Messages_Kilep` towards another screen: true = gone now (its closing has run out, or there is none).
    fn leave_messages_screen(&mut self) -> bool {
        self.overlay != Some(Screen::Messages) || self.hud.as_mut().is_none_or(|h| h.messages.leave())
    }

    fn debugx(&self) -> bool {
        self.world.game.state.mem.u8(va::DEBUGX) != 0
    }

    /// `KepernyokLetiltva[n]` (0xf66f2c + n): the tutorial has turned screen `n` off.
    /// A battle (on screen or parked under Options) locks every screen but Options: its opening zoom sets all of
    /// `KepernyokLetiltva`, its end restores them (`SpaceWar_Kilep` exe 0x562b50), the zoom's end only [0xd].
    fn screen_off(&self, n: i32) -> bool {
        (n != 0xd && (self.spacewar.is_some() || self.spacewar_parked.is_some()))
            || self.tutorial.as_ref().is_some_and(|t| t.t.gates.screens.get(n as usize).copied().unwrap_or(false))
    }

    /// The messages screen is idle (`DAT_00dabe3c == 0 && DAT_00da6607`): no answers wait and the text is typed out.
    pub(super) fn messages_idle(&self) -> bool {
        self.hud.as_ref().is_none_or(|h| h.messages.open.as_ref().is_none_or(|d| d.answers.is_empty() && d.typed_all))
    }

    /// The colony screen of a planet whose ground war runs (`help_new_available(AktPlanet) == 1`).
    pub(super) fn colony_war_up(&self) -> bool {
        self.colony.is_some() && self.overlay.is_none() && self.world.colony_curr.as_ref().is_some_and(|c| c.war_flag)
    }

    /// F*n* (`DoKeyboard` exe 0x4db1a0 lines 191-265, key down, also while a text entry is up): `AktKepKell = n` unless
    /// a change is under way, the screen is `n` already or the tutorial turned `n` off (`DAT_00f66ebd[vk]`, not with
    /// `DEBUGX`); the space battle and a colony at war pass only with `DEBUGX`, the options screen not when load-only
    /// and in a battle only with `DEBUGX`, the messages screen only when idle, the main menu never (`AktKep < 0xe`).
    pub(super) fn f_key(&mut self, to: i32) {
        let s = self.akt_kep_now();
        let debug = self.debugx();
        let gate = match s {
            11 => debug,
            3 if self.colony_war_up() => debug,
            0xd => self.options.as_ref().is_some_and(|o| !o.s.load_only && (!o.s.battle || debug)),
            10 => self.messages_idle(),
            _ => s < 0xe,
        };
        if !gate || self.screen_changing() || s == to || (self.screen_off(to) && !debug) {
            return;
        }
        // F11 asks for the space battle only while one is pending (bytes from 0xe436ee, stride 0x2d9c); the client's
        // battles start by themselves (re/deviations.md). From Options over a parked battle (`DEBUGX`) `SpaceWar_Belep`
        // resumes it at once (exe 0x562680: no redirect with `OldKep == 0xd` and the battle alive).
        if to == 11 {
            if s == 0xd && self.spacewar_parked.is_some() {
                self.leave_options();
                self.resume_parked_battle();
            }
            return;
        }
        // F3 while the tutorial awaits 0x1c (exe 0x4db3eb): the main planet stays shut, any other raises the event.
        if to == 3 && self.colony.is_none() && !self.colony_tutorial_gate() {
            return;
        }
        self.request_screen(to);
    }

    /// `AktKepKell = to`, carried out at once as far as it can be (the rest when the closing has run out).
    pub(super) fn request_screen(&mut self, to: i32) {
        self.kep_kell = Some(KepKell { to, from: self.akt_kep_now() });
        self.screen_change_step();
    }

    /// `IG2_gameUpdate`'s screen change: the screen up leaves (a closing animation stops the step until it has run
    /// out), down to the starmap or the colony under it, then the screen asked for enters. Help goes over the screen
    /// up (`HelpElottiScreen`).
    pub(super) fn screen_change_step(&mut self) {
        let Some(k) = self.kep_kell else { return };
        if k.to == 1 {
            // From the messages screen `Messages_Kilep`'s closing first.
            if !self.leave_messages_screen() {
                return;
            }
            self.kep_kell = None;
            self.open_help();
            return;
        }
        loop {
            let s = self.akt_kep_now();
            if s == k.to {
                self.kep_kell = None;
                return;
            }
            match s {
                1 => self.leave_help(),
                0xd => self.leave_options(),
                4 => {
                    if !self.research.as_mut().is_none_or(|r| r.leave()) {
                        return;
                    }
                    self.drop_research();
                    self.overlay = None;
                }
                5 => {
                    if !self.design.as_mut().is_none_or(|d| d.leave()) {
                        return;
                    }
                    self.drop_design();
                    self.overlay = None;
                }
                // `Diplomacy_Kilep`: towards the messages, help or options screen at once and silently.
                6 => {
                    let quiet = matches!(k.to, 1 | 10 | 0xd);
                    let closed = self.diplomacy.as_ref().is_some_and(|d| d.screen.closed());
                    if !closed && !self.diplomacy.as_mut().is_none_or(|d| d.screen.leave(quiet)) {
                        return;
                    }
                    self.dip_next = None;
                    self.overlay = None;
                }
                7 => {
                    if !self.spy.as_mut().is_none_or(|sp| sp.leave()) {
                        return;
                    }
                    self.spy = None;
                }
                8 => {
                    if !self.trade.as_mut().is_none_or(|t| t.leave(k.to)) {
                        return;
                    }
                    self.trade = None;
                    self.overlay = None;
                }
                9 => self.overlay = None,
                10 => {
                    if !self.leave_messages_screen() {
                        return;
                    }
                    self.overlay = None;
                }
                3 if k.to == 2 => self.toggle_colony(),
                2 | 3 => break,
                // `SpaceWar_Kilep` (exe 0x562b50): `Starmapvisszaallit` cancels the change and the zoom-out runs;
                // what follows is in `Window::spacewar_out`.
                11 => {
                    self.kep_kell = None;
                    self.leave_battle(k.to);
                    return;
                }
                // The space battle and the main menu aren't left by a key.
                _ => {
                    self.kep_kell = None;
                    return;
                }
            }
        }
        self.kep_kell = None;
        match k.to {
            3 => {
                if let Some(p) = self.planet_kell.take().or_else(|| self.colony_target()) {
                    self.open_colony(p);
                }
            }
            4 => self.enter_research(k.from),
            5 => self.enter_design(k.from),
            // `Diplomacy_Belep`: no opening (and no sound) from the messages, help or options screen.
            6 => self.enter_diplomacy(super::opens_animated(k.from)),
            7 => self.toggle_spy(),
            8 => self.enter_trade(k.from),
            9 => self.toggle_screen(Screen::Info),
            10 => self.toggle_screen(Screen::Messages),
            0xd => self.open_options(false),
            _ => {}
        }
    }

    /// `Options_Kilep` towards another screen: the settings kept, the time as it was (`SetIdoPause(OptionsOldIdoPause)`);
    /// the screen it was opened over isn't entered again.
    pub(super) fn leave_options(&mut self) {
        let Some(o) = self.options.take() else { return };
        self.store_settings_from(&o.s);
        if !o.was_paused {
            self.send(ig2_sim::game::StarmapOrder::Pause { paused: false });
        }
        self.clock = TickClock::new(SIM_TICK_MS, self.now_ms());
        self.trade_under_options = None;
        self.overlay = None;
        self.drop_research();
        self.drop_design();
        self.trade = None;
        self.spy = None;
    }

    /// M (`DoKeyboard` lines 92-114): the messages screen, not from itself or the main menu, not while a change is
    /// asked for or the tutorial turned it off (`DAT_00f66f36`, also with `DEBUGX`); the space battle, a colony at war
    /// and the options screen when load-only or in a battle only with `DEBUGX`. True when taken (else the character goes
    /// on to the screen's handler).
    pub(super) fn m_key(&mut self) -> bool {
        let s = self.akt_kep_now();
        let debug = self.debugx();
        let pass = match s {
            // The battle's screen locks hold M off even with `DEBUGX` (`screen_off`).
            11 => debug,
            3 if self.colony_war_up() => debug,
            0xd => self.options.as_ref().is_some_and(|o| !(o.s.load_only || o.s.battle) || debug),
            _ => true,
        };
        if s == 10 || s >= 0xe || !pass || self.kep_kell.is_some() || self.screen_off(10) {
            return false;
        }
        self.request_screen(10);
        true
    }

    /// O (`DoKeyboard` lines 54-76): the options screen (on it, back), not on the messages screen unless idle, not when
    /// the tutorial turned it off (`DAT_00f66f39`), not while a change is under way.
    pub(super) fn o_key_allowed(&self) -> bool {
        let s = self.akt_kep_now();
        (s != 10 || self.messages_idle()) && !self.screen_off(0xd) && s < 0xe && !self.screen_changing()
    }

    /// Space, P and 1..3 (`DoKeyboard` lines 80-90, 301-331): single player, not on the messages screen, the options
    /// screen or the main menu (`AktKep < 0xd`), not while the time can't be set (`nemlehetidotallitani`).
    pub(super) fn time_keys_allowed(&self) -> bool {
        let s = self.akt_kep_now();
        s != 10 && s < 0xd && !self.speed_locked() && self.spacewar.as_ref().is_none_or(|sw| sw.clock_free())
    }

    /// Esc on the starmap with a panel up (`MainCallBackEventHandler` exe 0x4e2db0: the production view, the transfer,
    /// the fleet's or vehicles' panel, the planet's long info): `AktIkonKell = 0x27`, hot-spot 9 (the exit) as if
    /// pressed. True when taken.
    pub(super) fn starmap_escape(&mut self) -> bool {
        if self.akt_kep_now() != 2 {
            return false;
        }
        if self.menus.info_on() {
            let cam = self.camera3();
            self.world.game.tutorial_check_event(0x16);
            self.sounds.push(crate::audio::SoundId::global(5));
            self.menus.leave_info(cam, false);
            return true;
        }
        if !(self.panels.product || self.panels.transfer.is_some() || self.panels.info.is_some()) {
            return false;
        }
        let sel = self.selection();
        if let Some(acts) = self.panels.press_id(&self.world.game.state, &sel, 9, [0.0; 2], true) {
            self.apply(acts);
        }
        true
    }
}

/// The MP sender's gate on 0x406/0x407 (`DoKeyboard` exe 0x4db484/0x4db535/0x4dbee9, `IG2_gameUpdate` 0x4e0831/
/// 0x4e08e9): `OnlyServerCanModifyTime == 0 || SuperVisor`.
pub(super) fn mp_time_gate(only_server: i32, host: bool) -> bool {
    only_server == 0 || host
}

/// `ColonyKeyboardHandler`'s held camera keys (exe 0x4628c0: vk 0x25..=0x28 by the logical key, so a NumLock-off
/// numpad's arrows too, deviation 375; numpad + / − by the physical one): left, right, up, down, +, − as 0..=5.
pub(super) fn colony_camera_key(physical: winit::keyboard::PhysicalKey, logical: Key<&str>) -> Option<usize> {
    use winit::keyboard::{KeyCode, PhysicalKey};
    Some(match (physical, logical) {
        (PhysicalKey::Code(KeyCode::NumpadAdd), _) => 4,
        (PhysicalKey::Code(KeyCode::NumpadSubtract), _) => 5,
        (_, Key::Named(NamedKey::ArrowLeft)) => 0,
        (_, Key::Named(NamedKey::ArrowRight)) => 1,
        (_, Key::Named(NamedKey::ArrowUp)) => 2,
        (_, Key::Named(NamedKey::ArrowDown)) => 3,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_host_sets_the_time_when_the_room_says_so() {
        assert!(mp_time_gate(0, false) && mp_time_gate(0, true));
        assert!(!mp_time_gate(1, false));
        assert!(mp_time_gate(1, true));
    }

    #[test]
    fn numpad_digits_type_into_entries() {
        use winit::keyboard::{KeyCode, PhysicalKey};
        // With Num Lock on a numpad digit is its character; off, it is a navigation key and types nothing.
        assert!(numpad_digit_key(PhysicalKey::Code(KeyCode::Numpad5), Key::Character("5")));
        assert!(!numpad_digit_key(PhysicalKey::Code(KeyCode::Numpad5), Key::Named(NamedKey::Clear)));
        assert!(!numpad_digit_key(PhysicalKey::Code(KeyCode::Digit5), Key::Character("5")));
        assert!(!numpad_digit_key(PhysicalKey::Code(KeyCode::NumpadAdd), Key::Character("+")));
    }

    #[test]
    fn colony_arrows_come_by_their_virtual_key() {
        use winit::keyboard::{KeyCode, PhysicalKey};
        assert_eq!(colony_camera_key(PhysicalKey::Code(KeyCode::ArrowLeft), Key::Named(NamedKey::ArrowLeft)), Some(0));
        // A NumLock-off numpad 8 sends VK_UP.
        assert_eq!(colony_camera_key(PhysicalKey::Code(KeyCode::Numpad8), Key::Named(NamedKey::ArrowUp)), Some(2));
        assert_eq!(colony_camera_key(PhysicalKey::Code(KeyCode::Numpad8), Key::Character("8")), None);
        assert_eq!(colony_camera_key(PhysicalKey::Code(KeyCode::NumpadSubtract), Key::Character("-")), Some(5));
    }

    #[test]
    fn f_keys_ask_for_their_screen() {
        // `AktKepKell = vk − 0x6f` for vk 0x70..0x7a.
        assert_eq!(f_key_screen(&Key::Named(NamedKey::F1)), Some(1));
        assert_eq!(f_key_screen(&Key::Named(NamedKey::F10)), Some(10));
        assert_eq!(f_key_screen(&Key::Named(NamedKey::F11)), Some(11));
        assert_eq!(f_key_screen(&Key::Named(NamedKey::F12)), None);
        assert_eq!(f_key_screen(&Key::Character("o")), None);
    }

    #[test]
    fn screens_open_animated_except_from_messages_help_and_options() {
        // FX1 trace `fkeys`: research from the options screen has no opening sound; `KELLKINYITASDEFAULT` otherwise.
        assert!(super::super::opens_animated(2));
        assert!(super::super::opens_animated(3));
        assert!(!super::super::opens_animated(1));
        assert!(!super::super::opens_animated(10));
        assert!(!super::super::opens_animated(0xd));
    }
}
