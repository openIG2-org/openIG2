//! The human's messages: the scrolling message line (`ScrollUzenet`, exe 0x556b50) with its history, the texts
//! of `uzenet.txt` (`ScrollUzenetId`, exe 0x556dc0), the story scripts' lines (`_s(5, n)` through [`printf`]) and
//! the inbox's expiry (`FRISSIT_Messages`, exe 0x5062d0). Spec: `re/rules/info-messages.md` §2–§4.
//!
//! The lists are kept as the original's raw 0x40-byte entries: a text is copied up to its NUL, so the bytes after
//! it are whatever the slot held before, and a save round-trips only with them. The sim posts; the UI takes the
//! next text off the line at its own pace ([`Ticker::pop`]).

use std::collections::BTreeMap;

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

/// The line's motion (`FRISSIT_Scroll2` exe 0x556210, spec §2.3): `UzenetStart` 0xE412FC, `UzenetSebesseg`
/// 0xE41300, `UzenetHatar` 0xE41304 (f32, unsaved UI state), in characters. A text comes in from the right
/// (`start` −53), slows to a stop at `limit`, holds 400 steps, then speeds away; one step per 10 ms UI tick.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Motion {
    pub start: f32,
    pub speed: f32,
    pub limit: f32,
}

/// The step's f64 constant (exe 0x859a78): one ulp below 0.0146.
const STEP: f64 = f64::from_bits(0x3f8d_e69a_d42c_3c9e);
/// The line's width in characters (exe 0x859a84: the start, −53).
const WIDTH: f32 = 53.0;

impl Motion {
    /// A text of `len` bytes taken onto the line (exe 0x556418..0x5564a0).
    pub fn begin(len: usize) -> Self {
        let n = len as i32;
        Self {
            start: -WIDTH,
            speed: ((n + 53) as f64 * STEP).sqrt() as f32,
            limit: -(((53 - n) as f64) as f32 * 0.5),
        }
    }

    /// One step of a text on the line (exe 0x55658f..0x556688).
    pub fn advance(&mut self) {
        if self.limit > self.start {
            self.start += self.speed;
            self.speed = (self.speed as f64 - STEP) as f32;
            if 0.02f64 > self.speed as f64 {
                self.speed = 0.02;
            }
            if self.start >= self.limit {
                self.start = self.limit;
                self.speed = 400.0;
            }
        } else if self.start == self.limit {
            self.speed -= 1.0;
            if self.speed == 0.0 {
                self.speed = 0.2;
                self.start = (self.start as f64 + 0.01) as f32;
            }
        } else {
            self.start += self.speed;
            self.speed = (self.speed as f64 + STEP) as f32;
        }
    }
}

impl Ticker {
    /// `FRISSIT_Scroll2`'s step once its gate lets it run (the screen, no dialog: the caller's): when the text
    /// has left the line (`start ≥ len`) or there is none, the next entry goes on it ([`Ticker::pop`],
    /// [`Motion::begin`]); then the text moves. Returns the id of a text taken on (its sound plays, spec §2.3).
    pub fn scroll(&mut self, m: &mut Motion) -> Option<i32> {
        let len = cstr(&self.on_screen).len();
        let mut popped = None;
        if len == 0 || len as f32 <= m.start {
            popped = self.pop();
            if popped.is_some() {
                *m = Motion::begin(cstr(&self.on_screen).len());
            }
        }
        if !cstr(&self.on_screen).is_empty() {
            m.advance();
        }
        popped
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

/// The texts the sim posts or writes, raw bytes in the game's code page: the lines of `uzenet.txt` (spec §3.1),
/// the story table `_s(5, n)` (`s_story.txt`, spec §3.4) and the tutorial's name for a sighted planet
/// (`GetString(2, 4, 6)`, `re/rules/tutorial.md` §4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Texts {
    pub lines: Vec<Vec<u8>>,
    pub story: BTreeMap<i32, Vec<u8>>,
    pub new_planet: Vec<u8>,
}

impl Texts {
    pub fn parse(raw: &[u8]) -> Self {
        Self { lines: raw.split(|&c| c == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l).to_vec()).collect(), ..Self::default() }
    }

    /// Add the story table (`s_story.txt`, keyed table 5) and the UI table's `2 4 6` line (`txt02.txt`), both in
    /// the game's language.
    pub fn with_story(mut self, s_story: &[u8], txt02: &[u8], lang: ig2_formats::text::Lang) -> Self {
        use ig2_formats::{strings, text};
        let mut keyed = strings::KeyedStrings::default();
        keyed.add_file(5, &text::decode(s_story, lang));
        self.story = keyed.entries.iter().map(|(k, s)| ((k & 0xffff) as i32, text::encode(s, lang))).collect();
        let mut ui = strings::UiStrings::default();
        ui.add_file(&text::decode(txt02, lang));
        self.new_planet = text::encode(ui.get(2, 4, 6), lang);
        self
    }

    /// `_s(5, n)` (exe 0x542ce0): a missing key gives "".
    pub fn story_text(&self, n: i32) -> &[u8] {
        self.story.get(&n).map_or(&[], Vec::as_slice)
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

/// An argument of [`printf`].
#[derive(Debug, Clone, Copy)]
pub enum Arg<'a> {
    Int(i32),
    Str(&'a [u8]),
}

/// C `printf` for the story texts (`stringhelper::format` = `vsnprintf`, and `sprintf`; spec §3.4): `%d`/`%i`, `%s`
/// (up to the NUL) and `%%`, the conversions the texts use. Any other `%` sequence is copied as it is; an argument
/// past the last given reads as an empty string or 0.
pub fn printf(fmt: &[u8], args: &[Arg]) -> Vec<u8> {
    let fmt = cstr(fmt);
    let mut out = Vec::with_capacity(fmt.len());
    let mut args = args.iter();
    let mut i = 0;
    while i < fmt.len() {
        match (fmt[i], fmt.get(i + 1)) {
            (b'%', Some(b'%')) => out.push(b'%'),
            (b'%', Some(b'd' | b'i')) => match args.next() {
                Some(Arg::Int(n)) => out.extend_from_slice(n.to_string().as_bytes()),
                Some(Arg::Str(_)) | None => out.push(b'0'),
            },
            (b'%', Some(b's')) => {
                if let Some(Arg::Str(s)) = args.next() {
                    out.extend_from_slice(cstr(s));
                }
            }
            (c, _) => {
                out.push(c);
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    out
}

/// The names a story line fills in: `Talalmanyok[t]`, `Bolygok[p]`, `BirodalomNev[e]` (C strings).
pub trait LineNames {
    fn tech(&self, t: i32) -> Vec<u8>;
    fn planet(&self, p: i32) -> Vec<u8>;
    fn empire(&self, e: i32) -> Vec<u8>;
}

/// A story script's own line on the scrolling ticker (spec §3.4): its text and important flag (posted with id 1,
/// no interrupt). `None` for the other story events.
pub fn story_line(ev: &crate::story::StoryEvent, texts: &Texts, names: &dyn LineNames) -> Option<(Vec<u8>, bool)> {
    use crate::story::StoryEvent;
    let t = |n| texts.story_text(n);
    Some(match *ev {
        StoryEvent::ScrollText { text, important } => (cstr(t(text)).to_vec(), important),
        // `sprintf(_s(5, 191), EpuletekLeiro[kind]'s tech name, planet name)`.
        StoryEvent::ScrollSabotage { tech, planet } => (printf(t(191), &[Arg::Str(&names.tech(tech)), Arg::Str(&names.planet(planet))]), false),
        StoryEvent::ScrollNumber { text, a } => (printf(t(text), &[Arg::Int(a)]), false),
        StoryEvent::ScrollNumbers { text, a, b } => (printf(t(text), &[Arg::Int(a), Arg::Int(b)]), false),
        StoryEvent::ScrollEmpirePlanet { text, empire, planet } => {
            (printf(t(text), &[Arg::Str(&names.empire(empire)), Arg::Str(&names.planet(planet))]), false)
        }
        _ => return None,
    })
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

/// The inbox's static data from the install (`LoadMessagesFile`, `BetoltFajlok`; re/screens/messages.md §5):
/// `uzenetek.txt`'s kinds (`UzenetFajtak` exe 0xD90DF0) and each conversation's raw `msg%04d.txt`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InboxRules {
    /// `(id, conversation +4, automatic +5)` in file order (`KerUz` takes the first match).
    pub kinds: Vec<(i32, bool, bool)>,
    /// `msg%04d.txt` by id, each made to end in `\r\n` (`BetoltFajlok`).
    pub scripts: BTreeMap<i32, Vec<u8>>,
}

impl InboxRules {
    pub fn from_install(install: &ig2_formats::Install) -> Self {
        let raw = install.read("dataraw/uzenetek/uzenetek.txt").unwrap_or_default();
        let kinds: Vec<(i32, bool, bool)> = ig2_formats::dialogue::parse_message_index(&String::from_utf8_lossy(&raw))
            .into_iter()
            .map(|k| (k.id as i32, k.conversation, k.automatic))
            .collect();
        let scripts = kinds
            .iter()
            .filter(|k| k.1)
            .filter_map(|&(id, ..)| {
                let mut b = install.read(&format!("dataraw/uzenetek/msg{id:04}.txt")).ok()?;
                if b.last() != Some(&b'\n') {
                    b.extend_from_slice(b"\r\n");
                }
                Some((id, b))
            })
            .collect();
        Self { kinds, scripts }
    }

    /// The kind's flags for `id`: the first with this id; none reads the slot past the table (zeros in the exe's
    /// inlined loops).
    pub fn kind(&self, id: i32) -> (bool, bool) {
        self.kinds.iter().find(|k| k.0 == id).map_or((false, false), |k| (k.1, k.2))
    }
}

/// One `- PP NN label` line of a block (`Valasztasok`, 0x5c: `+0x50` part, `+0x54` next block).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub part: i32,
    pub next: i32,
    /// The line's offset (its label starts 8 bytes in).
    pub at: i32,
}

/// A two-character `atoi` (the exe copies two bytes and a NUL).
fn atoi2(b: &[u8]) -> i32 {
    let s: String = b.iter().take(2).map(|&c| c as char).collect();
    ig2_formats::dialogue::atoi(&s)
}

/// `KeresBlokk(n)` (exe 0x505930): the offset of the line after the `%02d` header that starts a line, past empty
/// lines (lines end in `\r` + one byte), or −1.
pub fn find_block(msg: &[u8], n: i32) -> i32 {
    let len = msg.len() as i32;
    let key = format!("{n:02}").into_bytes();
    let at = |i: i32| msg.get(i as usize).copied().unwrap_or(0);
    let mut start = 0;
    while start < len {
        if msg[start as usize..].starts_with(&key) {
            let mut i = start;
            loop {
                while i < len && at(i) != b'\r' {
                    i += 1;
                }
                let next = i + 2;
                if !(next < len && at(i + 2) == b'\r') {
                    return next;
                }
                i = next;
            }
        }
        while start < len && at(start) != b'\r' {
            start += 1;
        }
        start += 2;
    }
    -1
}

/// `KiertekelBlokk`'s parse (exe 0x505c20): the block's `- PP NN` lines from offset `i`.
pub fn block_choices(msg: &[u8], mut i: i32) -> Vec<Choice> {
    let len = msg.len() as i32;
    let at = |i: i32| msg.get(i as usize).copied().unwrap_or(0);
    let mut out = Vec::new();
    while i >= 0 && at(i) == b'-' {
        let field = |k: i32| msg.get((i + k) as usize..).unwrap_or(&[]);
        out.push(Choice { part: atoi2(field(2)), next: atoi2(field(5)), at: i });
        loop {
            let mut j = i;
            while j < len && at(j) != b'\r' {
                j += 1;
            }
            i = j + 2;
            if !(i < len && at(j + 2) == b'\r') {
                break;
            }
        }
    }
    out
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
    fn blocks_and_choices_follow_keres_and_kiertekel_blokk() {
        let msg = b"00\r\n\r\n- 01 02 Hello #1#\r\n- -1 03 x\r\n02\r\n+ text\r\n";
        let off = find_block(msg, 0);
        // Past the header and the empty line.
        assert_eq!(off, 6);
        assert_eq!(block_choices(msg, off), [Choice { part: 1, next: 2, at: 6 }, Choice { part: -1, next: 3, at: 25 }]);
        assert_eq!(block_choices(msg, find_block(msg, 2)), []);
        assert_eq!(find_block(msg, 7), -1);
        assert_eq!(block_choices(msg, -1), []);
    }

    #[test]
    fn printf_fills_the_story_texts() {
        let f = printf(b"%d tons of food has been sold at %d price", &[Arg::Int(120), Arg::Int(-7)]);
        assert_eq!(f, b"120 tons of food has been sold at -7 price");
        let f = printf(b"Building %s damaged on %s", &[Arg::Str(b"Park\0junk"), Arg::Str(b"Orion 1")]);
        assert_eq!(f, b"Building Park damaged on Orion 1");
        assert_eq!(printf(b"100%% %x", &[]), b"100% %x");
        assert_eq!(printf(b"", &[Arg::Int(5)]), b"");
    }

    #[test]
    fn story_lines_render_their_texts() {
        use crate::story::StoryEvent;
        struct N;
        impl LineNames for N {
            fn tech(&self, t: i32) -> Vec<u8> {
                format!("tech{t}").into_bytes()
            }
            fn planet(&self, p: i32) -> Vec<u8> {
                format!("planet{p}").into_bytes()
            }
            fn empire(&self, e: i32) -> Vec<u8> {
                format!("empire{e}").into_bytes()
            }
        }
        let mut t = Texts::default();
        t.story.insert(191, b"Building %s damaged on %s".to_vec());
        t.story.insert(3081, b"Sta Kirin has been captured!".to_vec());
        t.story.insert(8242, b"Reinfocement arrived from %s to %s".to_vec());
        let line = |ev| story_line(&ev, &t, &N);
        assert_eq!(line(StoryEvent::ScrollText { text: 3081, important: false }), Some((b"Sta Kirin has been captured!".to_vec(), false)));
        assert_eq!(line(StoryEvent::ScrollSabotage { tech: 7, planet: 3 }), Some((b"Building tech7 damaged on planet3".to_vec(), false)));
        assert_eq!(line(StoryEvent::ScrollEmpirePlanet { text: 8242, empire: 2, planet: 9 }).unwrap().0, b"Reinfocement arrived from empire2 to planet9");
        // 814's key isn't in table 5: an empty line.
        assert_eq!(line(StoryEvent::ScrollNumber { text: 8140, a: 15000 }), Some((Vec::new(), false)));
        assert_eq!(line(StoryEvent::Won), None);
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
