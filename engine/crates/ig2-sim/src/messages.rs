//! The human's messages: the scrolling message line (`ScrollUzenet`, exe 0x556b50) with its history, the texts
//! of `uzenet.txt` (`ScrollUzenetId`, exe 0x556dc0) and the inbox's expiry (`FRISSIT_Messages`, exe 0x5062d0).
//! Spec: `re/rules/info-messages.md` §2–§4.
//!
//! The lists are kept as the original's raw 0x40-byte entries: a text is copied up to its NUL, so the bytes after
//! it are whatever the slot held before, and a save round-trips only with them. The sim posts; the UI takes the
//! next text off the line at its own pace ([`Ticker::pop`]).

/// One list entry.
pub const ENTRY: usize = 0x40;
/// Entries per list (1..=10; entry 0 is never written).
pub const MAX: usize = 10;
/// Bytes of a list (`ScrollUzenetek` / `ScrollUzenetekHistory` up to the next global).
pub const LIST: usize = (MAX + 1) * ENTRY;
/// `ScrollSzoveg` (0xE41230 .. 0xE412F8).
pub const ON_SCREEN: usize = 0xc8;
/// A posted text keeps this many bytes.
const TEXT: usize = 0x35;
/// Entry offsets: the message id (main list only) and the important flag.
const ID: usize = 0x38;
const IMPORTANT: usize = 0x3c;

/// The scrolling message line's state (spec §2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticker {
    /// `ScrollUzenetek` and `UzenetekDb`.
    pub main: Vec<u8>,
    pub count: i32,
    /// `ScrollUzenetekHistory` and `UzenetekDbHistory`.
    pub history: Vec<u8>,
    pub history_count: i32,
    /// `ScrollSzoveg`, `ScrollFontos`: the text on the line now and its flag (UI state, unsaved).
    pub on_screen: Vec<u8>,
    pub on_screen_important: u8,
    /// The tutorial is running or paused (`0xF6915D`, `0xF66F2A`): nothing is posted.
    pub tutorial: bool,
}

impl Default for Ticker {
    fn default() -> Self {
        Self { main: vec![0; LIST], count: 0, history: vec![0; LIST], history_count: 0, on_screen: vec![0; ON_SCREEN], on_screen_important: 0, tutorial: false }
    }
}

/// The C string at the start of `b`.
pub fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

/// Copy `text` and its NUL to the start of `b` (`strcpy`: the rest of `b` keeps its bytes).
fn put_cstr(b: &mut [u8], text: &[u8]) {
    b[..text.len()].copy_from_slice(text);
    b[text.len()] = 0;
}

impl Ticker {
    fn entry(list: &[u8], k: usize) -> &[u8] {
        &list[k * ENTRY..(k + 1) * ENTRY]
    }

    /// `ScrollUzenet(text, id, important, interrupt)` (spec §2.2).
    pub fn post(&mut self, text: &[u8], id: i32, important: bool, interrupt: bool) {
        if self.tutorial {
            return;
        }
        let t = &cstr(text)[..cstr(text).len().min(TEXT)];
        if self.history_count > 9 {
            self.history_count -= 1;
            let n = self.history_count.max(0) as usize;
            self.history.copy_within(2 * ENTRY..(n + 2) * ENTRY, ENTRY);
        }
        self.history_count += 1;
        let h = self.history_count as usize * ENTRY;
        put_cstr(&mut self.history[h..h + ENTRY], t);
        self.history[h + IMPORTANT] = important as u8;

        let db = self.count;
        let duplicate = cstr(&self.on_screen) == t || (1..=db.max(0) as usize).any(|k| cstr(Self::entry(&self.main, k)) == t);
        if duplicate {
            return;
        }
        let mut db = db;
        if important && db == MAX as i32 {
            db = MAX as i32 - 1;
        } else if db > MAX as i32 - 1 {
            return;
        }
        let n = db + 1;
        let mut slot = n as usize;
        if important {
            let mut k = db.max(0) as usize;
            while k > 0 && self.main[k * ENTRY + IMPORTANT] == 0 {
                self.main.copy_within(k * ENTRY..(k + 1) * ENTRY, (k + 1) * ENTRY);
                slot = k;
                k -= 1;
            }
        }
        let at = slot * ENTRY;
        put_cstr(&mut self.main[at..at + ENTRY], t);
        self.main[at + IMPORTANT] = important as u8;
        self.main[at + ID..at + ID + 4].copy_from_slice(&id.to_le_bytes());
        self.count = n;
        if (self.on_screen_important == 0 && important) || interrupt {
            self.on_screen[0] = 0;
        }
    }

    /// `FRISSIT_Scroll2` taking the next text once the line is free (spec §2.3; the UI's step, not the sim's):
    /// entry 1 goes on the line and the rest move up. `None` when nothing is waiting (the line is cleared).
    pub fn pop(&mut self) -> Option<i32> {
        if self.count < 1 {
            self.on_screen[0] = 0;
            return None;
        }
        let first = Self::entry(&self.main, 1).to_vec();
        put_cstr(&mut self.on_screen, cstr(&first));
        self.on_screen_important = first[IMPORTANT];
        self.count -= 1;
        let n = self.count as usize;
        self.main.copy_within(2 * ENTRY..(n + 2) * ENTRY, ENTRY);
        Some(i32::from_le_bytes(first[ID..ID + 4].try_into().unwrap()))
    }
}

/// `atoi`: leading white space, a sign, digits.
fn atoi(s: &[u8]) -> i32 {
    let s = &s[s.iter().position(|c| !matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')).unwrap_or(s.len())..];
    let (neg, s) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let v = s.iter().take_while(|c| c.is_ascii_digit()).fold(0i32, |a, &c| a.wrapping_mul(10).wrapping_add((c - b'0') as i32));
    if neg { v.wrapping_neg() } else { v }
}

/// The lines of `uzenet.txt` (raw bytes in the game's code page), spec §3.1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Texts {
    pub lines: Vec<Vec<u8>>,
}

impl Texts {
    pub fn parse(raw: &[u8]) -> Self {
        Self { lines: raw.split(|&c| c == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l).to_vec()).collect() }
    }

    /// `ScrollUzenetId`'s text: the first line whose number is `id`, from byte 5 on, with `%1`, `%2`, … replaced
    /// by `params` (each once, in order). `None` without such a line.
    pub fn format(&self, id: i32, params: &[&[u8]]) -> Option<Vec<u8>> {
        let line = self.lines.iter().find(|l| atoi(l) == id)?;
        let mut s = line.get(5..).unwrap_or_default().to_vec();
        for (k, p) in params.iter().enumerate() {
            let key = [b'%', b'1' + k as u8];
            if let Some(i) = s.windows(2).position(|w| w == key) {
                s.splice(i..i + 2, cstr(p).iter().copied());
            }
        }
        Some(s)
    }
}

/// The inbox (`Uzenetek`): 100 entries of 0x88 bytes (`+0` block, `+4` kept, `+8` expiry `Ido`), with the
/// parallel `UzenetekRandomKitol` (i32) and `UzenetPlaybackek` (0x54) records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inbox {
    pub messages: Vec<u8>,
    pub random_from: Vec<u8>,
    pub playbacks: Vec<u8>,
    /// `UzenetekSzama`.
    pub count: i32,
}

/// Inbox record sizes.
pub const MESSAGE: usize = 0x88;
pub const PLAYBACK: usize = 0x54;

impl Inbox {
    /// `FRISSIT_Messages` (spec §4): the first message past its expiry, unless kept or showing (`showing` =
    /// `UAktUzenetFajta`), is removed; its block for `Story_MessageElevules`.
    pub fn expire(&mut self, now: i32, showing: i32) -> Option<i32> {
        let n = self.count.max(0) as usize;
        let rd = |b: &[u8], o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let i = (0..n).find(|&i| {
            let m = &self.messages[i * MESSAGE..];
            let t = rd(m, 8);
            t < now && t != 0 && m[4] == 0 && rd(m, 0) != showing
        })?;
        let block = rd(&self.messages[i * MESSAGE..], 0);
        let last = n - 1;
        if i < last {
            self.random_from.copy_within((i + 1) * 4..n * 4, i * 4);
            self.playbacks.copy_within((i + 1) * PLAYBACK..n * PLAYBACK, i * PLAYBACK);
            self.messages.copy_within((i + 1) * MESSAGE..n * MESSAGE, i * MESSAGE);
        }
        self.count = last as i32;
        Some(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(list: &[u8], n: i32) -> Vec<(String, u8)> {
        (1..=n as usize).map(|k| (String::from_utf8_lossy(cstr(&list[k * ENTRY..])).into_owned(), list[k * ENTRY + IMPORTANT])).collect()
    }

    #[test]
    fn posts_dedupe_and_put_important_messages_after_the_last_important_one() {
        let mut t = Ticker::default();
        t.post(b"a", 1, false, false);
        t.post(b"b", 2, false, false);
        t.post(b"a", 1, false, false);
        t.post(b"C", 3, true, false);
        assert_eq!(texts(&t.main, t.count), [("C".into(), 1), ("a".into(), 0), ("b".into(), 0)]);
        assert_eq!(t.history_count, 4);
        t.post(b"D", 4, true, false);
        assert_eq!(texts(&t.main, t.count), [("C".into(), 1), ("D".into(), 1), ("a".into(), 0), ("b".into(), 0)]);
        assert_eq!(t.pop(), Some(3));
        assert_eq!(cstr(&t.on_screen), b"C");
        // On the line now: posting it again is a duplicate (history only).
        t.post(b"C", 3, true, false);
        assert_eq!(t.count, 3);
    }

    #[test]
    fn full_lists_drop_and_keep_ten() {
        let mut t = Ticker::default();
        for k in 0..12 {
            t.post(format!("m{k}").as_bytes(), k, false, false);
        }
        assert_eq!(t.count, 10);
        assert_eq!(t.history_count, 10);
        assert_eq!(cstr(&t.history[ENTRY..]), b"m2");
        t.post(b"urgent", 99, true, false);
        assert_eq!(t.count, 10);
        assert_eq!(cstr(&t.main[ENTRY..]), b"urgent");
        assert_eq!(cstr(&t.main[10 * ENTRY..]), b"m8");
    }

    #[test]
    fn texts_take_the_first_line_and_fill_the_parameters() {
        let t = Texts::parse(b"0001 Welcome\r\n0006 %1 declared war on the %2!\r\n\r\n0020 Income %1.\r\n0020 second\r\n");
        assert_eq!(t.format(6, &[b"A\0junk", b"B"]).unwrap(), b"A declared war on the B!");
        assert_eq!(t.format(20, &[b"40965"]).unwrap(), b"Income 40965.");
        assert_eq!(t.format(7, &[]), None);
    }

    #[test]
    fn one_message_expires_per_tick() {
        let mut inbox = Inbox { messages: vec![0; 100 * MESSAGE], random_from: vec![0; 400], playbacks: vec![0; 100 * PLAYBACK], count: 3 };
        for (i, (block, until)) in [(10i32, 0i32), (11, 500), (12, 400)].into_iter().enumerate() {
            inbox.messages[i * MESSAGE..i * MESSAGE + 4].copy_from_slice(&block.to_le_bytes());
            inbox.messages[i * MESSAGE + 8..i * MESSAGE + 12].copy_from_slice(&until.to_le_bytes());
        }
        assert_eq!(inbox.expire(600, 0), Some(11));
        assert_eq!(inbox.count, 2);
        assert_eq!(inbox.expire(600, 12), None);
    }
}
