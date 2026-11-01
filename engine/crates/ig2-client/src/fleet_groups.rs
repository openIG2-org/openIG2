//! ADR 0035: fleet groups on the starmap. Ctrl+1..9 stores the selected fleet, Alt+1..9 selects it, again centres the
//! camera on it. Client-side only (no order, nothing in the shared state); kept beside the save as `save<n>.groups`.
//! The original has none (`StarmapKeyboardHandler` exe 0x5aeab0 takes no digit, re/screens/starmap.md §10.6).

use std::path::{Path, PathBuf};

use winit::keyboard::{KeyCode, PhysicalKey};

/// The fleet (`Flottak` index) bound to groups 1..=9, 0 = none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Groups(pub [usize; 9]);

/// What a digit with a modifier asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKey {
    Store(usize),
    Recall(usize),
}

/// The top row's 1..9 by place (any layout) with Ctrl (store) or Alt (recall); 0-based group. Ctrl+Alt (AltGr) is
/// neither, it types.
pub fn group_key(physical: PhysicalKey, ctrl: bool, alt: bool) -> Option<GroupKey> {
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6,
        KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
    ];
    let PhysicalKey::Code(c) = physical else { return None };
    let g = DIGITS.iter().position(|&d| d == c)?;
    match (ctrl, alt) {
        (true, false) => Some(GroupKey::Store(g)),
        (false, true) => Some(GroupKey::Recall(g)),
        _ => None,
    }
}

/// The groups' file beside a save slot's file.
pub fn path(save: &Path) -> PathBuf {
    save.with_extension("groups")
}

impl Groups {
    /// `group fleet` lines (1-based group), only the bound ones.
    pub fn to_text(self) -> String {
        self.0.iter().enumerate().filter(|(_, f)| **f > 0).map(|(g, f)| format!("{} {f}\n", g + 1)).collect()
    }

    /// Lines that don't parse are skipped.
    pub fn parse(text: &str) -> Self {
        let mut out = Groups::default();
        for line in text.lines() {
            let mut w = line.split_whitespace().map(str::parse::<usize>);
            if let (Some(Ok(g @ 1..=9)), Some(Ok(f))) = (w.next(), w.next()) {
                out.0[g - 1] = f;
            }
        }
        out
    }

    /// The groups saved beside `save`, none without the file.
    pub fn load(save: &Path) -> Self {
        std::fs::read_to_string(path(save)).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    /// Written beside `save`; no groups removes the file.
    pub fn store(&self, save: &Path) -> std::io::Result<()> {
        match self.to_text() {
            t if t.is_empty() => std::fs::remove_file(path(save)).or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) }),
            t => std::fs::write(path(save), t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_by_place_and_modifier() {
        let k = PhysicalKey::Code(KeyCode::Digit3);
        assert_eq!(group_key(k, true, false), Some(GroupKey::Store(2)));
        assert_eq!(group_key(k, false, true), Some(GroupKey::Recall(2)));
        assert_eq!(group_key(k, false, false), None);
        assert_eq!(group_key(k, true, true), None);
        assert_eq!(group_key(PhysicalKey::Code(KeyCode::Digit0), true, false), None);
        assert_eq!(group_key(PhysicalKey::Code(KeyCode::Numpad1), true, false), None);
    }

    #[test]
    fn text_round_trip() {
        let mut g = Groups::default();
        g.0[0] = 12;
        g.0[8] = 3;
        assert_eq!(g.to_text(), "1 12\n9 3\n");
        assert_eq!(Groups::parse(&g.to_text()), g);
        assert_eq!(Groups::parse("0 5\n10 4\nx 1\n2\n4 7\r\n"), Groups([0, 0, 0, 7, 0, 0, 0, 0, 0]));
    }

    #[test]
    fn file_beside_the_save() {
        let dir = std::env::temp_dir().join(format!("ig2-groups-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let save = dir.join("save4.ig2s");
        assert_eq!(path(&save), dir.join("save4.groups"));
        assert_eq!(Groups::load(&save), Groups::default());
        let g = Groups([5, 0, 0, 0, 0, 0, 0, 0, 0]);
        g.store(&save).unwrap();
        assert_eq!(Groups::load(&save), g);
        Groups::default().store(&save).unwrap();
        assert!(!path(&save).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
