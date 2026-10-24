//! The multiplayer chat (re/screens/input.md §DoKeyboard, deviations 157 and 163): `DoKeyboard`'s text entry (Enter
//! or the backquote in multiplayer, exe 0x4db1a0), `ExecuteString`'s chat line (exe 0x4dc140: `Multi_Send(0x3fc,
//! EN, …)`), `MULTIGET_Starmap`'s receiver (exe 0x59d930) into `AddSzoveg`'s ring (exe 0x591360) and its draw in
//! `StarmapGfx2D_3` (exe 0x5aaba0). The entry is drawn in the top bar's line (`GFX_IdospeedPenzDatum` exe 0x554990),
//! which the message line leaves free while it is open.

use crate::font::{Align, Font};
use crate::kit::Virtual;
use crate::render::{Frame, Instance};

/// `IOMaxLen` 0x34: the entry's cells (the last one the NUL's).
pub const ENTRY_LEN: usize = 0x34;
/// `AddSzoveg`'s slots.
const SLOTS: usize = 8;
/// Palette indices: a player's line (`AddSzoveg(.., 3)`; 7 is a system line's), the entry (`IOColor` 3), the cursor (1).
pub const CHAT_COLOUR: usize = 3;

/// A key in the entry (`DoKeyboard`'s `IOMode` branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKey {
    Char(u8),
    Backspace,
    Delete,
    Home,
    End,
    Left,
    Right,
    Enter,
    Escape,
}

/// The open text entry (`IOMode` 1, `IOAzonosito` 1, buffer `EnterChatText` 0xb772c8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub text: Vec<u8>,
    /// `IOKurzor`.
    pub cursor: usize,
}

impl Entry {
    /// A key; `Some(line)` once Enter ends a non-empty entry (`ExecuteString(buf)`), `Some(empty)` for an empty one
    /// or Esc (the entry closes either way: `None` = it stays open).
    pub fn key(&mut self, k: EntryKey) -> Option<Vec<u8>> {
        let max = ENTRY_LEN - 1;
        match k {
            // Codes above 0x1f but 0x81, while the cursor is inside; at full length the last one is pushed out.
            EntryKey::Char(c) if c > 0x1f && c != 0x81 && self.cursor < max => {
                self.text.insert(self.cursor, c);
                self.text.truncate(max);
                self.cursor += 1;
            }
            EntryKey::Char(_) => {}
            EntryKey::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.text.remove(self.cursor);
            }
            EntryKey::Backspace => {}
            EntryKey::Delete if self.cursor < self.text.len() => {
                self.text.remove(self.cursor);
            }
            EntryKey::Delete => {}
            EntryKey::Home => self.cursor = 0,
            EntryKey::End => self.cursor = self.text.len(),
            EntryKey::Left => self.cursor = self.cursor.saturating_sub(1),
            EntryKey::Right => self.cursor = (self.cursor + 1).min(self.text.len()),
            EntryKey::Enter => return Some(std::mem::take(&mut self.text)),
            EntryKey::Escape => return Some(Vec::new()),
        }
        None
    }

    /// `KiirStrAlap(IOString, 0x112, 6, 0x34, 2, 4, IOColor)` and, while `AktIdo200` is on, the cursor:
    /// `FillToScreen(IOX + min(IOKurzor, 0x33)·CharXMeret[2], IOY + CharYMeret[2], CharXMeret[2], 2, 1)`.
    pub fn draw(&self, f: &mut Frame, font: &Font, white: crate::render::TexId, v: &Virtual, palette: &[[f32; 3]], time: f32) {
        let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
        font.text(f, v, &self.text, 274.0, 6.0, ENTRY_LEN, 2, Align::Left, col(CHAT_COLOUR), None);
        if (time * 1000.0 / 200.0) as i64 % 2 == 0 {
            let w = font.advance(2);
            let x = 274.0 + self.cursor.min(ENTRY_LEN - 1) as f32 * w;
            let y = 6.0 + font.height(2);
            let c = col(1);
            f.push(white, crate::render::Blend::Alpha, Instance::new(crate::render::kind::SPRITE, v.px(x + w / 2.0, y + 1.0), [w * v.k / 2.0, v.k], [c[0], c[1], c[2], 1.0]));
        }
    }
}

/// `AddSzoveg`'s ring (`Szovegek`, `SzovegSzin`, `UtolsoSzovegIdo`, `SzovegEltolas`): eight lines, the newest at the
/// bottom, gliding up a pixel per 100 ms after the last change; ten pixels push the top one out.
#[derive(Debug, Clone, Default)]
pub struct Ring {
    lines: [(Vec<u8>, usize); SLOTS],
    last_ms: f64,
    shift: i32,
}

impl Ring {
    /// `AddSzoveg(text, colour)`: an empty text shifts every line up a slot; a line shifts only when the bottom
    /// slot is taken; then it takes the bottom slot.
    pub fn add(&mut self, text: Vec<u8>, colour: usize, now_ms: f64) {
        if text.is_empty() || !self.lines[SLOTS - 1].0.is_empty() {
            self.lines.rotate_left(1);
            self.shift = 0;
        }
        self.lines[SLOTS - 1] = (text, colour);
        self.last_ms = now_ms;
    }

    /// `StarmapGfx2D_3`'s timer: 100 ms after the last step the lines move up a pixel; at ten the top one goes
    /// (`AddSzoveg("", 3)`). A clock that went back restarts the count.
    pub fn tick(&mut self, now_ms: f64) {
        if now_ms < self.last_ms {
            self.last_ms = now_ms;
        }
        if self.lines.iter().all(|l| l.0.is_empty()) {
            return;
        }
        if self.last_ms + 100.0 < now_ms {
            self.shift += 1;
            self.last_ms = now_ms;
            if self.shift >= 10 {
                self.add(Vec::new(), CHAT_COLOUR, now_ms);
            }
        }
    }

    #[cfg(test)]
    fn lines(&self) -> impl Iterator<Item = &[u8]> {
        self.lines.iter().map(|l| l.0.as_slice()).filter(|l| !l.is_empty())
    }

    /// Slot i at (0x14, i·10 + 0x32 − shift), 100 cells of size 2, clipped to y ≥ 0x32; slot 0 at (9 − shift)·10 %.
    pub fn draw(&self, f: &mut Frame, font: &Font, v: &Virtual, palette: &[[f32; 3]]) {
        for (i, (text, c)) in self.lines.iter().enumerate() {
            let y = (i as i32 * 10 + 0x32 - self.shift) as f32;
            if text.is_empty() || y < 0x32 as f32 {
                continue;
            }
            let mut rgb = palette.get(*c).copied().unwrap_or([1.0; 3]);
            if i == 0 {
                let k = ((9 - self.shift) * 10).clamp(0, 100) as f32 / 100.0;
                rgb = rgb.map(|x| x * k);
            }
            font.text(f, v, text, 20.0, y, 100, 2, Align::Left, rgb, None);
        }
    }
}

/// The chat on this machine: the open entry and the ring.
#[derive(Debug, Clone, Default)]
pub struct Chat {
    pub entry: Option<Entry>,
    pub ring: Ring,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_edits_like_dokeyboard() {
        let mut e = Entry::default();
        for c in b"hllo" {
            assert_eq!(e.key(EntryKey::Char(*c)), None);
        }
        e.key(EntryKey::Home);
        e.key(EntryKey::Right);
        e.key(EntryKey::Char(b'e'));
        assert_eq!(e.text, b"hello");
        e.key(EntryKey::End);
        e.key(EntryKey::Backspace);
        e.key(EntryKey::Left);
        e.key(EntryKey::Delete);
        assert_eq!((e.text.as_slice(), e.cursor), (&b"hel"[..], 3));
        e.key(EntryKey::Char(0x81));
        e.key(EntryKey::Char(0x1f));
        assert_eq!(e.text, b"hel");
        for _ in 0..60 {
            e.key(EntryKey::Char(b'x'));
        }
        assert_eq!(e.text.len(), ENTRY_LEN - 1);
        assert_eq!(e.key(EntryKey::Enter).map(|t| t.len()), Some(ENTRY_LEN - 1));
        assert_eq!(Entry::default().key(EntryKey::Escape), Some(Vec::new()));
    }

    #[test]
    fn the_ring_fills_from_the_bottom_and_glides_out() {
        let mut r = Ring::default();
        r.add(b"one".to_vec(), CHAT_COLOUR, 0.0);
        r.add(b"two".to_vec(), CHAT_COLOUR, 0.0);
        assert_eq!(r.lines().collect::<Vec<_>>(), [&b"one"[..], b"two"]);
        assert_eq!(r.lines[SLOTS - 1].0, b"two");
        // Ten steps of 100+ ms move the lines up a slot; "one" (slot 6) leaves the top after seven.
        let mut t = 0.0;
        while r.lines().count() == 2 {
            t += 10.0;
            r.tick(t);
            assert!(t < 20_000.0);
        }
        assert_eq!(r.lines().collect::<Vec<_>>(), [&b"two"[..]]);
        assert_eq!(r.lines[0].0, b"two");
        assert!((7 * 1100) as f64 == t, "{t}");
    }
}
