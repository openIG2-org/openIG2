//! Ours (re/deviations.md, text fields): the editing keys every text field takes on top of the original's
//! (`DoKeyboard`'s `IOMode`: characters, Backspace, Delete, the arrows, Home, End): a selection (Shift with the
//! arrows, Home and End; a press and drag; a double-click picks a word), Ctrl+A, Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+Z,
//! Ctrl with the arrows and the erase keys a word at a time. A field keeps its own rules for what a typed byte
//! does ([`Edit::put`]); this module only moves the cursor, removes a selection and replays pasted text through them.

use ig2_formats::text::Lang;
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// A field's selection and undo history.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sel {
    /// The selection's other end (the cursor is one end); equal to the cursor or `None`: nothing selected.
    pub anchor: Option<usize>,
    undo: Vec<(Vec<u8>, usize)>,
    /// The last change was a typed byte: a run of them undoes at once.
    typing: bool,
}

/// The undo steps a field keeps.
const UNDO: usize = 64;

/// A text field the editing keys work on.
pub trait Edit {
    fn text(&self) -> &[u8];
    fn cursor(&self) -> usize;
    fn place(&mut self, cursor: usize);
    fn sel(&mut self) -> &mut Sel;
    /// A typed byte at the cursor, by the field's own rules (its length, the bytes it takes).
    fn put(&mut self, b: u8);
    /// The text and the cursor replaced (a removed selection, an undo).
    fn set(&mut self, text: Vec<u8>, cursor: usize);
    /// A password's: Copy and Cut take nothing out of it.
    fn secret(&self) -> bool {
        false
    }
}

/// [`Edit`] for a field with `text`, `cursor` and `sel`: `put` types a byte by its own rules; a `clear` flag
/// (`IOGombnyomasraTorles`, the first key replaces the text) ends with a cursor move or a replaced text.
macro_rules! edit_field {
    ($t:ty, $put:expr $(, $clear:ident)?) => {
        impl crate::text_edit::Edit for $t {
            fn text(&self) -> &[u8] {
                &self.text
            }
            fn cursor(&self) -> usize {
                self.cursor
            }
            fn place(&mut self, c: usize) {
                self.cursor = c.min(self.text.len());
                $(self.$clear = false;)?
            }
            fn sel(&mut self) -> &mut crate::text_edit::Sel {
                &mut self.sel
            }
            fn put(&mut self, b: u8) {
                let put: fn(&mut Self, u8) = $put;
                put(self, b);
            }
            fn set(&mut self, text: Vec<u8>, cursor: usize) {
                (self.text, self.cursor) = (text, cursor);
                $(self.$clear = false;)?
            }
        }
    };
}
pub(crate) use edit_field;

/// Where a cursor move goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum To {
    Left,
    Right,
    WordLeft,
    WordRight,
    Home,
    End,
}

/// An editing key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    SelectAll,
    Copy,
    Cut,
    Paste,
    Undo,
    Move { to: To, extend: bool },
    Erase { back: bool, word: bool },
    /// A character the field's own key handling types.
    Type,
    /// Another Ctrl chord: a field with the focus keeps it from typing its letter.
    Chord,
}

/// What a key did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    /// Not handled: the field's own key handling goes on (a selection it would act on is already gone).
    Pass,
    /// Handled; the text stayed.
    Done,
    /// Handled; the text changed.
    Changed,
    /// These bytes to the clipboard (the text changed too after a cut).
    Copy(Vec<u8>, bool),
    /// The clipboard's text wanted: [`paste`] it.
    Paste,
}

/// The editing key `key` is, with Ctrl (or the Mac's Command) and Shift as held.
pub fn cmd(key: &Key<&str>, ctrl: bool, shift: bool) -> Option<Cmd> {
    let mv = |w: To, c: To| Some(Cmd::Move { to: if ctrl { w } else { c }, extend: shift });
    match key {
        Key::Character(c) if ctrl => Some(match c.to_ascii_lowercase().as_str() {
            "a" => Cmd::SelectAll,
            "c" => Cmd::Copy,
            "x" => Cmd::Cut,
            "v" => Cmd::Paste,
            "z" => Cmd::Undo,
            _ => Cmd::Chord,
        }),
        Key::Character(_) | Key::Named(NamedKey::Space) => Some(Cmd::Type),
        Key::Named(NamedKey::ArrowLeft) => mv(To::WordLeft, To::Left),
        Key::Named(NamedKey::ArrowRight) => mv(To::WordRight, To::Right),
        Key::Named(NamedKey::Home) => mv(To::Home, To::Home),
        Key::Named(NamedKey::End) => mv(To::End, To::End),
        Key::Named(NamedKey::Insert) if ctrl => Some(Cmd::Copy),
        Key::Named(NamedKey::Insert) if shift => Some(Cmd::Paste),
        Key::Named(NamedKey::Delete) if shift && !ctrl => Some(Cmd::Cut),
        Key::Named(NamedKey::Backspace) => Some(Cmd::Erase { back: true, word: ctrl }),
        Key::Named(NamedKey::Delete) => Some(Cmd::Erase { back: false, word: ctrl }),
        _ => None,
    }
}

/// The selected range, if any (clamped to the text).
pub fn range(e: &mut impl Edit) -> Option<std::ops::Range<usize>> {
    let (len, c) = (e.text().len(), e.cursor().min(e.text().len()));
    let a = e.sel().anchor?.min(len);
    (a != c).then(|| a.min(c)..a.max(c))
}

/// A byte of a word (double-click, Ctrl with the arrows).
fn wordy(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

fn motion(t: &[u8], c: usize, to: To) -> usize {
    let c = c.min(t.len());
    match to {
        To::Left => c.saturating_sub(1),
        To::Right => (c + 1).min(t.len()),
        To::Home => 0,
        To::End => t.len(),
        To::WordLeft => {
            let k = t[..c].iter().rposition(|&b| wordy(b)).map_or(0, |i| i + 1);
            t[..k].iter().rposition(|&b| !wordy(b)).map_or(0, |i| i + 1)
        }
        To::WordRight => {
            let k = t[c..].iter().position(|&b| wordy(b)).map_or(t.len(), |i| c + i);
            t[k..].iter().position(|&b| !wordy(b)).map_or(t.len(), |i| k + i)
        }
    }
}

/// The current text kept for Ctrl+Z.
fn remember(e: &mut impl Edit) {
    let (t, c) = (e.text().to_vec(), e.cursor());
    let s = e.sel();
    if s.undo.last().is_some_and(|(u, _)| *u == t) {
        return;
    }
    if s.undo.len() == UNDO {
        s.undo.remove(0);
    }
    s.undo.push((t, c));
}

/// The selection removed (kept for Ctrl+Z); false when there was none.
fn erase_selection(e: &mut impl Edit) -> bool {
    let Some(r) = range(e) else {
        e.sel().anchor = None;
        return false;
    };
    remember(e);
    let mut t = e.text().to_vec();
    t.drain(r.clone());
    e.set(t, r.start);
    e.sel().anchor = None;
    true
}

/// An editing key on `e`.
pub fn apply(e: &mut impl Edit, k: Cmd) -> Out {
    let typing = std::mem::take(&mut e.sel().typing);
    match k {
        Cmd::SelectAll => {
            e.sel().anchor = Some(0);
            e.place(e.text().len());
            Out::Done
        }
        Cmd::Copy | Cmd::Cut if e.secret() => Out::Done,
        Cmd::Copy => range(e).map_or(Out::Done, |r| Out::Copy(e.text()[r].to_vec(), false)),
        Cmd::Cut => match range(e) {
            Some(r) => {
                let bytes = e.text()[r].to_vec();
                erase_selection(e);
                Out::Copy(bytes, true)
            }
            None => Out::Done,
        },
        Cmd::Paste => Out::Paste,
        Cmd::Undo => match e.sel().undo.pop() {
            Some((t, c)) => {
                e.set(t, c);
                e.sel().anchor = None;
                Out::Changed
            }
            None => Out::Done,
        },
        Cmd::Move { to, extend } => {
            let c = e.cursor().min(e.text().len());
            let at = match (range(e), to) {
                // A selection's arrow without Shift: to its end on that side.
                (Some(r), To::Left) if !extend => r.start,
                (Some(r), To::Right) if !extend => r.end,
                _ => motion(e.text(), c, to),
            };
            let s = e.sel();
            s.anchor = if extend { Some(s.anchor.unwrap_or(c)) } else { None };
            e.place(at);
            Out::Done
        }
        Cmd::Erase { back, word } => {
            if range(e).is_none() && word {
                let c = e.cursor().min(e.text().len());
                e.sel().anchor = Some(motion(e.text(), c, if back { To::WordLeft } else { To::WordRight }));
            }
            if erase_selection(e) {
                return Out::Changed;
            }
            remember(e);
            Out::Pass
        }
        Cmd::Type => {
            if !erase_selection(e) && !typing {
                remember(e);
            }
            e.sel().typing = true;
            Out::Pass
        }
        Cmd::Chord => Out::Done,
    }
}

/// Clipboard text (as the field's bytes) at the cursor, over the selection: its first line, each byte by the
/// field's own rules; true when the text changed.
pub fn paste(e: &mut impl Edit, bytes: &[u8]) -> bool {
    let before = e.text().to_vec();
    if !erase_selection(e) {
        remember(e);
    }
    let line = bytes.split(|&b| b == b'\n' || b == b'\r').find(|l| l.iter().any(|b| !b.is_ascii_whitespace())).unwrap_or(&[]);
    for &b in line.iter().filter(|&&b| b >= 0x20 && b != 0x7f) {
        e.put(if b == b'\t' { b' ' } else { b });
    }
    e.text() != before
}

/// The pointer on a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mouse {
    /// A left press at a byte (Shift held: the selection grows to it).
    Press(usize, bool),
    /// A drag to a byte.
    Drag(usize),
    /// A double-click at a byte.
    Word(usize),
}

/// The pointer's act on `e`.
pub fn mouse(e: &mut impl Edit, m: Mouse) {
    match m {
        Mouse::Press(at, extend) => press(e, at, extend),
        Mouse::Drag(at) => drag(e, at),
        Mouse::Word(at) => word(e, at),
    }
}

/// A left press at byte `at` (Shift: the selection grows to it); a drag goes on with [`drag`].
pub fn press(e: &mut impl Edit, at: usize, extend: bool) {
    let (c, at) = (e.cursor(), at.min(e.text().len()));
    let s = e.sel();
    s.typing = false;
    s.anchor = Some(if extend { s.anchor.unwrap_or(c) } else { at });
    e.place(at);
}

/// The pointer dragged to byte `at` with the button down.
pub fn drag(e: &mut impl Edit, at: usize) {
    e.place(at.min(e.text().len()));
}

/// A double-click at byte `at`: the word there (or the run of other bytes).
pub fn word(e: &mut impl Edit, at: usize) {
    let t = e.text();
    let at = at.min(t.len());
    let kind = t.get(at).or_else(|| at.checked_sub(1).and_then(|i| t.get(i))).is_some_and(|&b| wordy(b));
    let start = t[..at].iter().rposition(|&b| wordy(b) != kind).map_or(0, |i| i + 1);
    let end = t[at..].iter().position(|&b| wordy(b) != kind).map_or(t.len(), |i| at + i);
    e.sel().anchor = Some(start);
    e.place(end);
}

/// A key on `e` with the modifiers held: the editing keys done here (the clipboard's text in `lang`'s code page),
/// [`Out::Pass`] for the field's own handling. AltGr (Ctrl+Alt on Windows) types, it is no chord.
pub fn key(e: &mut impl Edit, key: &Key<&str>, m: ModifiersState, lang: Lang) -> Out {
    let ctrl = (m.control_key() || m.super_key()) && !m.alt_key();
    let Some(c) = cmd(key, ctrl, m.shift_key()) else { return Out::Pass };
    match apply(e, c) {
        Out::Copy(b, cut) => {
            clipboard_set(ig2_formats::text::decode(&b, lang));
            if cut { Out::Changed } else { Out::Done }
        }
        Out::Paste => match clipboard_get() {
            Some(t) if paste(e, &ig2_formats::text::encode(&t, lang)) => Out::Changed,
            _ => Out::Done,
        },
        o => o,
    }
}

/// The system clipboard (the page's in the browser, where a read arrives later through [`crate::web::take_pasted`]).
pub fn clipboard_set(text: String) {
    #[cfg(not(target_arch = "wasm32"))]
    use arboard::Clipboard;
    #[cfg(target_arch = "wasm32")]
    use crate::web::arboard::Clipboard;
    if let Err(e) = Clipboard::new().and_then(|mut c| c.set_text(text)) {
        eprintln!("ig2-client: clipboard: {e}");
    }
}

/// The clipboard's text now (the browser's: none yet, it is read in the background).
pub fn clipboard_get() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    use arboard::Clipboard;
    #[cfg(target_arch = "wasm32")]
    use crate::web::arboard::Clipboard;
    #[cfg(target_arch = "wasm32")]
    if let Some(t) = crate::web::take_pasted() {
        return Some(t).filter(|t| !t.is_empty());
    }
    match Clipboard::new().and_then(|mut c| c.get_text()) {
        Ok(t) => Some(t).filter(|t| !t.is_empty()),
        Err(e) => {
            eprintln!("ig2-client: clipboard: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A field with a length limit that takes letters only (as the name entries' own rules).
    #[derive(Default)]
    struct F {
        text: Vec<u8>,
        cursor: usize,
        sel: Sel,
    }

    impl Edit for F {
        fn text(&self) -> &[u8] {
            &self.text
        }
        fn cursor(&self) -> usize {
            self.cursor
        }
        fn place(&mut self, c: usize) {
            self.cursor = c;
        }
        fn sel(&mut self) -> &mut Sel {
            &mut self.sel
        }
        fn put(&mut self, b: u8) {
            if b.is_ascii_alphabetic() && self.text.len() < 8 {
                self.text.insert(self.cursor, b);
                self.cursor += 1;
            }
        }
        fn set(&mut self, t: Vec<u8>, c: usize) {
            (self.text, self.cursor) = (t, c);
        }
    }

    fn f(s: &str) -> F {
        F { text: s.as_bytes().to_vec(), cursor: s.len(), ..F::default() }
    }

    /// A typed key as a site does it: the editing key, then the field's own handling when it passes.
    fn typed(e: &mut F, b: u8) {
        if apply(e, Cmd::Type) == Out::Pass {
            e.put(b);
        }
    }

    #[test]
    fn select_all_copy_cut_paste() {
        let mut e = f("hello");
        apply(&mut e, Cmd::SelectAll);
        assert_eq!(apply(&mut e, Cmd::Copy), Out::Copy(b"hello".to_vec(), false));
        assert_eq!(apply(&mut e, Cmd::Cut), Out::Copy(b"hello".to_vec(), true));
        assert_eq!((e.text.as_slice(), e.cursor), (&b""[..], 0));
        assert_eq!(apply(&mut e, Cmd::Paste), Out::Paste);
        // The first non-blank line, by the field's rules (letters, 8 at most).
        assert!(paste(&mut e, b"\r\nab1cdefghijk\nzz"));
        assert_eq!(e.text, b"abcdefgh");
        // Nothing selected: Copy and Cut leave the clipboard.
        assert_eq!(apply(&mut e, Cmd::Copy), Out::Done);
    }

    #[test]
    fn shift_arrows_select_and_typing_replaces() {
        let mut e = f("abcdef");
        apply(&mut e, Cmd::Move { to: To::Home, extend: false });
        apply(&mut e, Cmd::Move { to: To::Right, extend: true });
        apply(&mut e, Cmd::Move { to: To::Right, extend: true });
        assert_eq!(range(&mut e), Some(0..2));
        typed(&mut e, b'x');
        assert_eq!((e.text.as_slice(), e.cursor), (&b"xcdef"[..], 1));
        // Shift+End, then Backspace takes the selection only.
        apply(&mut e, Cmd::Move { to: To::End, extend: true });
        assert_eq!(apply(&mut e, Cmd::Erase { back: true, word: false }), Out::Changed);
        assert_eq!(e.text, b"x");
        // A plain arrow with a selection lands on its side and drops it.
        let mut e = f("abcd");
        apply(&mut e, Cmd::SelectAll);
        apply(&mut e, Cmd::Move { to: To::Left, extend: false });
        assert_eq!((e.cursor, range(&mut e)), (0, None));
    }

    #[test]
    fn undo_steps_back_a_typed_run_then_a_paste() {
        let mut e = f("ab");
        for b in *b"cd" {
            typed(&mut e, b);
        }
        apply(&mut e, Cmd::Move { to: To::Left, extend: false });
        apply(&mut e, Cmd::SelectAll);
        paste(&mut e, b"zz");
        assert_eq!(e.text, b"zz");
        assert_eq!(apply(&mut e, Cmd::Undo), Out::Changed);
        assert_eq!(e.text, b"abcd");
        assert_eq!(apply(&mut e, Cmd::Undo), Out::Changed);
        assert_eq!(e.text, b"ab");
        assert_eq!(apply(&mut e, Cmd::Undo), Out::Done);
    }

    #[test]
    fn words_by_ctrl_and_double_click() {
        let t = b"192.168.0.10:port";
        assert_eq!(motion(t, 0, To::WordRight), 3);
        assert_eq!(motion(t, 3, To::WordRight), 7);
        assert_eq!(motion(t, t.len(), To::WordLeft), 13);
        assert_eq!(motion(t, 13, To::WordLeft), 10);
        let mut e = f("hello big world");
        word(&mut e, 7);
        assert_eq!(range(&mut e), Some(6..9));
        // Ctrl+Backspace: the word before the cursor.
        let mut e = f("hello big world");
        apply(&mut e, Cmd::Erase { back: true, word: true });
        assert_eq!(e.text, b"hello big ");
        // A press and a drag select; Shift+press grows it.
        press(&mut e, 1, false);
        drag(&mut e, 4);
        assert_eq!(range(&mut e), Some(1..4));
        press(&mut e, 6, true);
        assert_eq!(range(&mut e), Some(1..6));
    }

    #[test]
    fn keys_map_to_commands() {
        assert_eq!(cmd(&Key::Character("V"), true, false), Some(Cmd::Paste));
        assert_eq!(cmd(&Key::Character("q"), true, false), Some(Cmd::Chord));
        assert_eq!(cmd(&Key::Character("q"), false, false), Some(Cmd::Type));
        assert_eq!(cmd(&Key::Named(NamedKey::ArrowLeft), true, true), Some(Cmd::Move { to: To::WordLeft, extend: true }));
        assert_eq!(cmd(&Key::Named(NamedKey::Insert), false, true), Some(Cmd::Paste));
        assert_eq!(cmd(&Key::Named(NamedKey::Enter), false, false), None);
    }
}
