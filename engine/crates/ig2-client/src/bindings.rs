//! Ours (ADR 0058): the rebindable keys. Every action is one of the original's hotkeys (re/screens/input.md and the
//! screens' keyboard handlers) or one of the remake's, with the original's key as its default. A key the player bound
//! is turned into the action's default key before the screens see it, so every handler keeps matching the original's
//! keys; an action's default key that was bound away does nothing where the action applies.

use ig2_formats::options::Options as File;
use winit::keyboard::{Key, KeyCode, NamedKey, SmolStr};

/// Where an action's key is read: everywhere in the game, or on one screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ctx {
    Global,
    Starmap,
    Colony,
    Research,
    Design,
    Messages,
    Battle,
}

impl Ctx {
    /// Two actions with the same key clash where both apply.
    pub fn overlaps(self, other: Ctx) -> bool {
        self == other || self == Ctx::Global || other == Ctx::Global
    }

    /// The screen `AktKep` stands for (input.md §2), the game's other screens only the global keys.
    pub fn of_screen(akt_kep: i32) -> Ctx {
        match akt_kep {
            2 => Ctx::Starmap,
            3 => Ctx::Colony,
            4 => Ctx::Research,
            5 => Ctx::Design,
            10 => Ctx::Messages,
            0xb => Ctx::Battle,
            _ => Ctx::Global,
        }
    }

    /// The Controls tab's heading (the remake text table's key).
    pub fn title(self) -> &'static str {
        ["Everywhere", "Starmap", "Colony", "Research", "Design", "Messages", "Space battle"][self as usize]
    }
}

/// A rebindable action: its settings key (`Bind` + `id`), English name (the remake text table's key), where it
/// applies and its default key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    pub id: &'static str,
    pub name: &'static str,
    pub ctx: Ctx,
    pub default: KeyCode,
}

const fn a(id: &'static str, name: &'static str, ctx: Ctx, default: KeyCode) -> Action {
    Action { id, name, ctx, default }
}

/// The actions in the Controls tab's order. `DoKeyboard` (exe 0x4db1a0): F1..F10 the screens, Space and P pause, 1..3
/// the speeds, O options, M messages, ` the chat; F12 the quick save (ADR 0032). The screens' handlers (input.md §2):
/// `StarmapKeyboardHandler` 0x5aeab0, `ColonyKeyboardHandler` 0x4628c0, `ResearchKeyboardHandler` 0x523020,
/// `DesignKeyboardHandler` 0x4b1840, `MessagesKeyboardHandler` 0x507eb0, `SpaceWarKeyboardHandler` 0x561ef0. Esc,
/// Enter, the arrows and the group digits stay fixed (entries, menus and Ctrl+digit share them).
pub const ACTIONS: [Action; 45] = [
    a("Help", "Help", Ctx::Global, KeyCode::F1),
    a("Starmap", "Starmap screen", Ctx::Global, KeyCode::F2),
    a("Colony", "Colony screen", Ctx::Global, KeyCode::F3),
    a("Research", "Research screen", Ctx::Global, KeyCode::F4),
    a("Design", "Design screen", Ctx::Global, KeyCode::F5),
    a("Diplomacy", "Diplomacy screen", Ctx::Global, KeyCode::F6),
    a("Spy", "Spy screen", Ctx::Global, KeyCode::F7),
    a("Trade", "Trade screen", Ctx::Global, KeyCode::F8),
    a("Info", "Info screen", Ctx::Global, KeyCode::F9),
    a("MessagesF", "Messages screen", Ctx::Global, KeyCode::F10),
    a("Messages", "Messages (second key)", Ctx::Global, KeyCode::KeyM),
    a("Options", "Options", Ctx::Global, KeyCode::KeyO),
    a("Pause", "Pause", Ctx::Global, KeyCode::Space),
    a("Pause2", "Pause (second key)", Ctx::Global, KeyCode::KeyP),
    a("Speed1", "Normal speed", Ctx::Global, KeyCode::Digit1),
    a("Speed2", "Fast speed", Ctx::Global, KeyCode::Digit2),
    a("Speed3", "Fastest speed", Ctx::Global, KeyCode::Digit3),
    a("QuickSave", "Quick save (Shift: load)", Ctx::Global, KeyCode::F12),
    a("Chat", "Chat", Ctx::Global, KeyCode::Backquote),
    a("StarNext", "Next planet or fleet", Ctx::Starmap, KeyCode::PageUp),
    a("StarPrev", "Previous planet or fleet", Ctx::Starmap, KeyCode::PageDown),
    a("Colonise", "Colonise", Ctx::Starmap, KeyCode::KeyC),
    a("StarZoomIn", "Zoom in", Ctx::Starmap, KeyCode::NumpadAdd),
    a("StarZoomOut", "Zoom out", Ctx::Starmap, KeyCode::NumpadSubtract),
    a("PanUp", "Camera up", Ctx::Starmap, KeyCode::KeyW),
    a("PanLeft", "Camera left", Ctx::Starmap, KeyCode::KeyA),
    a("PanDown", "Camera down", Ctx::Starmap, KeyCode::KeyS),
    a("PanRight", "Camera right", Ctx::Starmap, KeyCode::KeyD),
    a("ColNext", "Next planet", Ctx::Colony, KeyCode::PageUp),
    a("ColPrev", "Previous planet", Ctx::Colony, KeyCode::PageDown),
    a("Camera", "Camera views", Ctx::Colony, KeyCode::KeyC),
    a("PlanetInfo", "Planet info", Ctx::Colony, KeyCode::KeyI),
    a("Power", "Building on or off", Ctx::Colony, KeyCode::KeyE),
    a("SelectAll", "Select all units", Ctx::Colony, KeyCode::KeyA),
    a("Stop", "Stop units", Ctx::Colony, KeyCode::KeyS),
    a("ResNext", "Next category", Ctx::Research, KeyCode::PageUp),
    a("ResPrev", "Previous category", Ctx::Research, KeyCode::PageDown),
    a("Redesign", "Redesign", Ctx::Design, KeyCode::KeyR),
    a("EventLog", "Event log", Ctx::Messages, KeyCode::KeyL),
    a("Answer1", "First answer", Ctx::Messages, KeyCode::KeyA),
    a("Answer2", "Second answer", Ctx::Messages, KeyCode::KeyB),
    a("Answer3", "Third answer", Ctx::Messages, KeyCode::KeyC),
    a("Answer4", "Fourth answer", Ctx::Messages, KeyCode::KeyD),
    a("WarAll", "Select the fleet", Ctx::Battle, KeyCode::KeyA),
    a("Chase", "Chase camera", Ctx::Battle, KeyCode::KeyC),
];

/// The keys a binding may take, with their settings and screen names (in the game's font: it has no backquote).
const KEYS: &[(KeyCode, &str)] = &[
    (KeyCode::KeyA, "A"), (KeyCode::KeyB, "B"), (KeyCode::KeyC, "C"), (KeyCode::KeyD, "D"), (KeyCode::KeyE, "E"),
    (KeyCode::KeyF, "F"), (KeyCode::KeyG, "G"), (KeyCode::KeyH, "H"), (KeyCode::KeyI, "I"), (KeyCode::KeyJ, "J"),
    (KeyCode::KeyK, "K"), (KeyCode::KeyL, "L"), (KeyCode::KeyM, "M"), (KeyCode::KeyN, "N"), (KeyCode::KeyO, "O"),
    (KeyCode::KeyP, "P"), (KeyCode::KeyQ, "Q"), (KeyCode::KeyR, "R"), (KeyCode::KeyS, "S"), (KeyCode::KeyT, "T"),
    (KeyCode::KeyU, "U"), (KeyCode::KeyV, "V"), (KeyCode::KeyW, "W"), (KeyCode::KeyX, "X"), (KeyCode::KeyY, "Y"),
    (KeyCode::KeyZ, "Z"), (KeyCode::Digit0, "0"), (KeyCode::Digit1, "1"), (KeyCode::Digit2, "2"), (KeyCode::Digit3, "3"),
    (KeyCode::Digit4, "4"), (KeyCode::Digit5, "5"), (KeyCode::Digit6, "6"), (KeyCode::Digit7, "7"), (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"), (KeyCode::F1, "F1"), (KeyCode::F2, "F2"), (KeyCode::F3, "F3"), (KeyCode::F4, "F4"),
    (KeyCode::F5, "F5"), (KeyCode::F6, "F6"), (KeyCode::F7, "F7"), (KeyCode::F8, "F8"), (KeyCode::F9, "F9"),
    (KeyCode::F10, "F10"), (KeyCode::F11, "F11"), (KeyCode::F12, "F12"), (KeyCode::Space, "Space"),
    (KeyCode::PageUp, "PgUp"), (KeyCode::PageDown, "PgDn"), (KeyCode::Home, "Home"), (KeyCode::End, "End"),
    (KeyCode::Insert, "Ins"), (KeyCode::Delete, "Del"), (KeyCode::Tab, "Tab"), (KeyCode::Backquote, "Backquote"),
    (KeyCode::Minus, "-"), (KeyCode::Equal, "="), (KeyCode::BracketLeft, "["), (KeyCode::BracketRight, "]"),
    (KeyCode::Semicolon, ";"), (KeyCode::Quote, "'"), (KeyCode::Backslash, "\\"), (KeyCode::Comma, ","),
    (KeyCode::Period, "."), (KeyCode::Slash, "/"), (KeyCode::NumpadAdd, "Num +"), (KeyCode::NumpadSubtract, "Num -"),
    (KeyCode::NumpadMultiply, "Num *"), (KeyCode::NumpadDivide, "Num /"), (KeyCode::Numpad0, "Num 0"),
    (KeyCode::Numpad1, "Num 1"), (KeyCode::Numpad2, "Num 2"), (KeyCode::Numpad3, "Num 3"), (KeyCode::Numpad4, "Num 4"),
    (KeyCode::Numpad5, "Num 5"), (KeyCode::Numpad6, "Num 6"), (KeyCode::Numpad7, "Num 7"), (KeyCode::Numpad8, "Num 8"),
    (KeyCode::Numpad9, "Num 9"),
];

/// A key's name on screen and in the settings file; `None` for a key no binding takes.
pub fn key_name(k: KeyCode) -> Option<&'static str> {
    KEYS.iter().find(|(c, _)| *c == k).map(|(_, n)| *n)
}

fn key_named(name: &str) -> Option<KeyCode> {
    KEYS.iter().find(|(_, n)| *n == name).map(|(c, _)| *c)
}

/// The logical key and text a default key arrives with on a US layout (what the screens match); `upper` for a
/// letter typed with Shift or Caps Lock.
pub fn logical(k: KeyCode, upper: bool) -> (Key, Option<SmolStr>) {
    let named = |n| (Key::Named(n), None);
    let ch = |s: &str| (Key::Character(SmolStr::new(s)), Some(SmolStr::new(s)));
    match k {
        KeyCode::Space => (Key::Named(NamedKey::Space), Some(SmolStr::new(" "))),
        KeyCode::PageUp => named(NamedKey::PageUp),
        KeyCode::PageDown => named(NamedKey::PageDown),
        KeyCode::F1 => named(NamedKey::F1),
        KeyCode::F2 => named(NamedKey::F2),
        KeyCode::F3 => named(NamedKey::F3),
        KeyCode::F4 => named(NamedKey::F4),
        KeyCode::F5 => named(NamedKey::F5),
        KeyCode::F6 => named(NamedKey::F6),
        KeyCode::F7 => named(NamedKey::F7),
        KeyCode::F8 => named(NamedKey::F8),
        KeyCode::F9 => named(NamedKey::F9),
        KeyCode::F10 => named(NamedKey::F10),
        KeyCode::F12 => named(NamedKey::F12),
        KeyCode::NumpadAdd => ch("+"),
        KeyCode::NumpadSubtract => ch("-"),
        KeyCode::Backquote => ch("`"),
        _ => match key_name(k) {
            Some(n) if n.len() == 1 && upper => ch(n),
            Some(n) if n.len() == 1 => ch(&n.to_ascii_lowercase()),
            _ => (Key::Unidentified(winit::keyboard::NativeKey::Unidentified), None),
        },
    }
}

/// What a pressed key becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remap {
    /// Unchanged (no action of the place has it, or it is the action's own default).
    Keep,
    /// The default key of the action bound to it.
    To(KeyCode),
    /// The default of an action bound to another key: nothing.
    Drop,
}

/// Each action's key, in [`ACTIONS`]' order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindings(pub Vec<KeyCode>);

impl Default for Bindings {
    fn default() -> Self {
        Bindings(ACTIONS.iter().map(|a| a.default).collect())
    }
}

impl Bindings {
    /// The settings file's keys (`Bind<id>` = a key's name); a missing or unknown one is the default.
    pub fn from_settings(f: &File) -> Self {
        Bindings(ACTIONS.iter().map(|a| f.get(&format!("Bind{}", a.id)).and_then(key_named).unwrap_or(a.default)).collect())
    }

    pub fn store(&self, f: &mut File) {
        for (a, &k) in ACTIONS.iter().zip(&self.0) {
            f.set(&format!("Bind{}", a.id), key_name(k).unwrap_or_default());
        }
    }

    /// Another action with action `i`'s key where both apply.
    pub fn clash(&self, i: usize) -> Option<usize> {
        (0..ACTIONS.len()).find(|&j| j != i && self.0[j] == self.0[i] && ACTIONS[j].ctx.overlaps(ACTIONS[i].ctx))
    }

    /// The key pressed at `ctx`: an action of the place bound to it gives its default key; the default of an action
    /// of the place bound elsewhere gives nothing.
    pub fn remap(&self, ctx: Ctx, pressed: KeyCode) -> Remap {
        let here = |a: &Action| a.ctx == Ctx::Global || a.ctx == ctx;
        if let Some((a, _)) = ACTIONS.iter().zip(&self.0).find(|(a, k)| here(a) && **k == pressed) {
            return if a.default == pressed { Remap::Keep } else { Remap::To(a.default) };
        }
        if ACTIONS.iter().zip(&self.0).any(|(a, &k)| here(a) && a.default == pressed && k != pressed) {
            return Remap::Drop;
        }
        Remap::Keep
    }

    /// Every key the original's.
    pub fn is_default(&self) -> bool {
        *self == Bindings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are the original's keys and clash nowhere; every default has a name, every action a unique id.
    #[test]
    fn defaults_are_the_originals_and_clash_nowhere() {
        let b = Bindings::default();
        for i in 0..ACTIONS.len() {
            assert_eq!(b.clash(i), None, "{}", ACTIONS[i].id);
            assert!(key_name(ACTIONS[i].default).is_some(), "{}", ACTIONS[i].id);
            assert!(ACTIONS[..i].iter().all(|a| a.id != ACTIONS[i].id), "{}", ACTIONS[i].id);
        }
        assert_eq!(b.remap(Ctx::Starmap, KeyCode::KeyC), Remap::Keep);
        assert_eq!(b.remap(Ctx::Global, KeyCode::KeyQ), Remap::Keep);
    }

    /// A key bound in one place clashes with an action of the same place or a global one, not with another screen's.
    #[test]
    fn clashes_by_place() {
        let at = |id| ACTIONS.iter().position(|a| a.id == id).unwrap();
        let mut b = Bindings::default();
        b.0[at("Colonise")] = KeyCode::KeyI;
        assert_eq!(b.clash(at("Colonise")), None, "I is the colony's");
        b.0[at("Colonise")] = KeyCode::KeyP;
        assert_eq!(b.clash(at("Colonise")), Some(at("Pause2")));
        assert_eq!(b.clash(at("Pause2")), Some(at("Colonise")));
        b.0[at("Colonise")] = KeyCode::PageUp;
        assert_eq!(b.clash(at("Colonise")), Some(at("StarNext")));
    }

    /// A bound key arrives as the action's default key, the default does nothing where the action applies and stays
    /// itself elsewhere; the settings file keeps the binding.
    #[test]
    fn remap_and_settings() {
        let at = |id| ACTIONS.iter().position(|a| a.id == id).unwrap();
        let mut b = Bindings::default();
        b.0[at("Pause")] = KeyCode::KeyK;
        b.0[at("Colonise")] = KeyCode::KeyX;
        assert_eq!(b.remap(Ctx::Colony, KeyCode::KeyK), Remap::To(KeyCode::Space));
        assert_eq!(b.remap(Ctx::Colony, KeyCode::Space), Remap::Drop);
        assert_eq!(b.remap(Ctx::Starmap, KeyCode::KeyX), Remap::To(KeyCode::KeyC));
        assert_eq!(b.remap(Ctx::Starmap, KeyCode::KeyC), Remap::Drop);
        // The colony's C (camera views) is another action.
        assert_eq!(b.remap(Ctx::Colony, KeyCode::KeyC), Remap::Keep);
        assert_eq!(b.remap(Ctx::Colony, KeyCode::KeyX), Remap::Keep);
        let mut f = File::default();
        b.store(&mut f);
        assert_eq!(f.get("BindPause"), Some("K"));
        assert_eq!(Bindings::from_settings(&f), b);
        assert!(Bindings::from_settings(&File::default()).is_default());
        f.set("BindPause", "nonsense");
        assert_eq!(Bindings::from_settings(&f).0[at("Pause")], KeyCode::Space);
    }

    #[test]
    fn defaults_arrive_as_the_screens_match_them() {
        assert_eq!(logical(KeyCode::KeyC, false).0, Key::Character(SmolStr::new("c")));
        assert_eq!(logical(KeyCode::KeyP, true).0, Key::Character(SmolStr::new("P")));
        assert_eq!(logical(KeyCode::F4, false).0, Key::Named(NamedKey::F4));
        assert_eq!(logical(KeyCode::Digit3, false), (Key::Character(SmolStr::new("3")), Some(SmolStr::new("3"))));
        for a in ACTIONS {
            assert!(!matches!(logical(a.default, false).0, Key::Unidentified(_)), "{}", a.id);
        }
    }
}
