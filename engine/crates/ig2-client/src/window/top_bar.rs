//! The top bar's plain hot-spots on every game screen (re/screens/input.md §6): `Mouse_Init` (exe 0x555df0) registers
//! them once and the hit test (exe 0x510390, the lowest index wins) puts them over every screen's own: pause and the
//! three speeds (2..5, on the press), the message line (7: the left button asks for the messages screen while a message
//! is unread, the right one holds the history open). The screen menu (6) and the quick-research menu (8) are the
//! drop-downs' (`crate::topbar_menus`).

use ig2_sim::save::va;
use winit::event::MouseButton;

use super::App;

/// `Mouse_Szoveg_Set(id − 0x1e, 0, x, y, w, h, ..)` of `Mouse_Init`: the plain spots (`AktIkon`, x, y, w, h).
const SPOTS: [(u32, [i32; 4]); 5] = [(2, [0, 0, 15, 20]), (3, [16, 0, 14, 20]), (4, [30, 0, 21, 20]), (5, [50, 0, 23, 20]), (7, [270, 0, 323, 20])];

/// The plain spot under the virtual point (the pointer clamped to the screen, `x0 ≤ x ≤ x0 + w − 1`, the lowest id).
pub(super) fn bar_spot(at: [f32; 2]) -> Option<u32> {
    let (x, y) = (at[0].clamp(0.0, 639.0) as i32, at[1].clamp(0.0, 479.0) as i32);
    SPOTS.iter().filter(|(_, r)| r[0] <= x && x < r[0] + r[2] && r[1] <= y && y < r[1] + r[3]).map(|s| s.0).min()
}

impl App {
    /// `VanUzenet` (exe 0x509e00): an inbox entry not read yet (`+4 == 0`).
    fn unread_message(&self) -> bool {
        let b = self.world.game.state.inbox();
        (0..b.count.max(0) as usize).any(|i| b.messages.get(i * ig2_sim::messages::MESSAGE + 4) == Some(&0))
    }

    /// A button on the top bar's plain spots (`IG2_gameUpdate` exe 0x4e0440 lines 147-181, `MOUSE_SelectScreens`
    /// 0x555470 lines 90-121) at the virtual point `at`. True when the bar takes it (the screen under it sees nothing);
    /// a press it took also takes that button's release.
    pub(super) fn top_bar_mouse(&mut self, button: MouseButton, pressed: bool, at: [f32; 2]) -> bool {
        if button == MouseButton::Left {
            self.bar_free_hold = pressed && bar_spot(at).is_none() && !self.screen_spot_at(at);
        }
        if !pressed {
            if self.bar_held != Some(button) {
                return false;
            }
            self.bar_held = None;
            self.bar_line_held = false;
            // The history goes with the release (`ScreensScrollHistoryMode = 0`).
            if let Some(h) = &mut self.hud {
                h.history_open = false;
            }
            return true;
        }
        let Some(spot) = bar_spot(at) else { return false };
        if !matches!(button, MouseButton::Left | MouseButton::Right) {
            return false;
        }
        self.bar_held = Some(button);
        self.bar_line_held = spot == 7 && button == MouseButton::Left;
        match (spot, button) {
            // Pause and the speeds (`Mouse_FelengedesMajdMegvar`: once, on the press): single player not on the messages
            // or options screen, nor while the time can't be set; the speeds only when paused or at another speed.
            // Either button (`Mouse_Lenyom`; exe trace `rightbutton`: a right press on ×1 and ×3 sounds and sets the speed).
            (2, _) if self.time_keys_allowed() || self.multiplayer() => self.toggle_pause(),
            (n @ 3..=5, _) if self.time_keys_allowed() || self.multiplayer() => self.set_speed(n as i32 - 2),
            // The message line's left button: [`Self::top_bar_frame`] on this frame and every held one.
            (7, MouseButton::Left) => self.messages_from_bar(),
            // The right button: the history while it is held (`GFX_ScrollHistory`).
            (7, MouseButton::Right) => {
                if let Some(h) = &mut self.hud {
                    h.history_open = true;
                }
            }
            _ => {}
        }
        true
    }

    /// Every frame: the left button held over the message line asks for the messages screen (`MOUSE_SelectScreens`
    /// exe 0x555470 lines 90-121 test `Mouse_Bal`, no press edge; `IG2_gameUpdate` 0x4e0440 hit-tests `AktIkon` again
    /// each held frame unless a spot holds the pointer, `Mouse_Megfogva` 0xdb466a), so a message that arrives while
    /// the line is held, or the pointer coming back onto it, asks too. A hold that began elsewhere stays with that
    /// spot (the screens' mains grab the pointer, the bar's flag-1 spots wait for the release); a hold that began over
    /// no hot-spot (`AktIkon` 0: no `_Main` runs, so nothing sets `Mouse_Megfogva`) asks once the pointer reaches the
    /// line.
    pub(super) fn top_bar_frame(&mut self) {
        if !(self.bar_line_held || self.bar_free_hold) || self.mouse_held & 1 == 0 || self.front.is_some() {
            return;
        }
        let Some(h) = &self.hud else { return };
        if h.history_open || h.top.open != crate::topbar_menus::Open::None {
            return;
        }
        if bar_spot(self.virtual_cursor()) == Some(7) {
            self.messages_from_bar();
        }
    }

    /// The message line's left button: the messages screen while a message is unread, not from it; the space battle,
    /// a colony at war and the options screen when load-only or in a battle only with `DEBUGX`.
    fn messages_from_bar(&mut self) {
        let s = self.akt_kep_now();
        let debug = self.world.game.state.mem.u8(va::DEBUGX) != 0;
        let pass = match s {
            11 => debug,
            3 if self.colony_war_up() => debug,
            0xd => self.options.as_ref().is_some_and(|o| !(o.s.load_only || o.s.battle) || debug),
            _ => true,
        };
        if s != 10 && pass && self.kep_kell.is_none() && self.unread_message() {
            self.request_screen(10);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_spots_take_their_pixels() {
        // `FUN_00510390`: x0 ≤ x ≤ x0 + w − 1; x 15 is between pause and ×1; x 50 is ×2's (the lower id).
        assert_eq!(bar_spot([0.0, 0.0]), Some(2));
        assert_eq!(bar_spot([14.9, 19.0]), Some(2));
        assert_eq!(bar_spot([15.0, 5.0]), None);
        assert_eq!(bar_spot([29.0, 5.0]), Some(3));
        assert_eq!(bar_spot([50.0, 5.0]), Some(4));
        assert_eq!(bar_spot([72.0, 5.0]), Some(5));
        assert_eq!(bar_spot([73.0, 5.0]), None);
        assert_eq!(bar_spot([270.0, 5.0]), Some(7));
        assert_eq!(bar_spot([592.0, 5.0]), Some(7));
        assert_eq!(bar_spot([593.0, 5.0]), None);
        assert_eq!(bar_spot([300.0, 20.0]), None);
        assert_eq!(bar_spot([-5.0, -5.0]), Some(2));
    }
}
